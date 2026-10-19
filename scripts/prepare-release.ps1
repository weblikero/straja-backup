param([Parameter(Mandatory=$true)][string]$NotesFile, [switch]$PortableOnly)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    $version = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected a stable major.minor.patch version.' }
    if (!(Test-Path -LiteralPath $NotesFile -PathType Leaf)) { throw 'Release notes file not found.' }
    $dirty = git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Commit the reviewed source before preparing a release.' }
    $tagCommit = git rev-list -n 1 "v$version"
    if ($LASTEXITCODE -ne 0) { throw 'Create and push the version tag first.' }
    $headCommit = git rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $tagCommit -ne $headCommit) { throw 'The version tag must point to HEAD.' }
    $portable = "dist\StrajaBackup-$version-windows-x64-portable.zip"
    $artifacts = @($portable)
    if (!$PortableOnly) { $artifacts += "dist\installer\StrajaBackup-$version-windows-x64-setup.exe" }
    $assets = @()
    foreach ($artifact in $artifacts) {
        $sidecar = "$artifact.sha256"
        if (!(Test-Path -LiteralPath $artifact -PathType Leaf) -or !(Test-Path -LiteralPath $sidecar -PathType Leaf)) {
            throw "Missing artifact or SHA-256 file: $artifact"
        }
        $expected = ((Get-Content -LiteralPath $sidecar -Raw).Trim() -split '\s+')[0]
        if ($expected -notmatch '^[a-fA-F0-9]{64}$' -or (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash -ne $expected) {
            throw "Invalid SHA-256: $artifact"
        }
        $assets += @($artifact, $sidecar)
    }
    & gh release create "v$version" @assets --repo weblikero/straja-backup --verify-tag --draft --title "Straja Backup $version" --notes-file $NotesFile
    if ($LASTEXITCODE -ne 0) { throw 'GitHub draft release creation failed.' }
    if (!$PortableOnly) {
        $releaseJson = & gh api "repos/weblikero/straja-backup/releases/tags/v$version"
        if ($LASTEXITCODE -ne 0) { throw 'Cannot verify uploaded installer metadata. Leave the release as a draft.' }
        $release = ($releaseJson -join "`n") | ConvertFrom-Json
        $installerName = "StrajaBackup-$version-windows-x64-setup.exe"
        $installerAsset = @($release.assets | Where-Object { $_.name -eq $installerName })
        $localInstaller = Join-Path 'dist\installer' $installerName
        $digest = 'sha256:' + (Get-FileHash -LiteralPath $localInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($installerAsset.Count -ne 1 -or $installerAsset[0].digest -ne $digest -or $installerAsset[0].size -ne (Get-Item -LiteralPath $localInstaller).Length) {
            throw 'GitHub installer size/digest mismatch or missing digest. Keep the release as a draft; in-app updates are unavailable.'
        }
    }
    Write-Host 'Draft created. Review the files and notes on GitHub before publishing.'
} finally { Pop-Location }
