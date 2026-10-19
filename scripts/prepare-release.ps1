param([Parameter(Mandatory=$true)][string]$NotesFile, [switch]$IncludeInstaller)
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
    if ($IncludeInstaller) { $artifacts += "dist\installer\StrajaBackup-$version-windows-x64-setup.exe" }
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
    Write-Host 'Draft created. Review the files and notes on GitHub before publishing.'
} finally { Pop-Location }
