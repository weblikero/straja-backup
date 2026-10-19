param([Parameter(Mandatory=$true)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$repository = 'weblikero/straja-backup'
$tag = "v$Version"
$expected = @{}
foreach ($relative in @("dist\installer\StrajaBackup-$Version-windows-x64-setup.exe", "dist\StrajaBackup-$Version-windows-x64-portable.zip")) {
    $path = Join-Path $projectRoot $relative
    $sidecar = "$path.sha256"
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if (((Get-Content -LiteralPath $sidecar -Raw).Trim() -split '\s+')[0] -ne $hash) { throw "Invalid local checksum: $relative" }
    foreach ($file in @($path, $sidecar)) {
        $item = Get-Item -LiteralPath $file
        $expected[$item.Name] = @{ size = $item.Length; digest = 'sha256:' + (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
}
function Assert-Release($release) {
    if ($release.tag_name -ne $tag -or $release.prerelease) { throw 'Unexpected tag or prerelease; refusing stable publication.' }
    foreach ($name in $expected.Keys) {
        $assets = @($release.assets | Where-Object { $_.name -ceq $name })
        if ($assets.Count -ne 1 -or $assets[0].state -ne 'uploaded' -or $assets[0].size -ne $expected[$name].size -or $assets[0].digest -ne $expected[$name].digest) {
            throw "Missing or mismatched uploaded asset: $name"
        }
        # GitHub assigns temporary untagged URLs to drafts; clients need the final tag URL.
        if (!$release.draft -and $assets[0].browser_download_url -cne "https://github.com/$repository/releases/download/$tag/$name") {
            throw "Unexpected public download URL: $name"
        }
    }
    $installer = $expected["StrajaBackup-$Version-windows-x64-setup.exe"]
    if ($installer.size -lt 1 -or $installer.size -gt 256MB) { throw 'Installer exceeds the in-app updater size limit.' }
}
# Drafts are not available through the public tag endpoint.
$filter = '.[] | select(.tag_name == "' + $tag + '") | .id'
$ids = @(& gh api "repos/$repository/releases" --paginate --jq $filter)
if ($LASTEXITCODE -ne 0 -or $ids.Count -ne 1 -or $ids[0] -notmatch '^\d+$') { throw 'Cannot resolve exactly one release for this tag.' }
$json = & gh api "repos/$repository/releases/$($ids[0])"
if ($LASTEXITCODE -ne 0) { throw 'Cannot read the release.' }
$release = ($json -join "`n") | ConvertFrom-Json
Assert-Release $release
$headers = @{ 'User-Agent' = 'StrajaBackup-release-validation'; Accept = 'application/vnd.github+json'; 'X-GitHub-Api-Version' = '2022-11-28' }
$latestUrl = "https://api.github.com/repos/$repository/releases/latest"
try {
    $latest = Invoke-RestMethod -Uri $latestUrl -Headers $headers -TimeoutSec 20
    if ([version]($latest.tag_name -replace '^v','') -gt [version]$Version) { throw 'A newer stable version already exists; refusing to roll back Latest.' }
} catch {
    if ($null -eq $_.Exception.Response -or [int]$_.Exception.Response.StatusCode -ne 404) { throw }
}
if ($release.draft) {
    & gh release edit $tag --repo $repository --draft=false --prerelease=false --latest
    if ($LASTEXITCODE -ne 0) { throw 'Publication failed. Inspect the existing release before retrying; never replace its assets.' }
}
# Use the anonymous endpoint used by installed clients, not the authenticated draft API.
for ($attempt = 1; $attempt -le 6; $attempt++) {
    try {
        $latest = Invoke-RestMethod -Uri $latestUrl -Headers $headers -TimeoutSec 20
        Assert-Release $latest
        if ($latest.draft -or !$latest.published_at) { throw 'The release is not published.' }
        Write-Host "Published and publicly discoverable: https://github.com/$repository/releases/tag/$tag"
        return
    } catch {
        if ($attempt -eq 6) { throw "Publication may have succeeded, but public update verification failed: $_" }
        Start-Sleep -Seconds 5
    }
}
