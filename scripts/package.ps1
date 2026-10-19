param([switch]$DebugBuild)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$restic = Join-Path $projectRoot '.tools\restic.exe'
if (!(Test-Path -LiteralPath $restic)) { & (Join-Path $PSScriptRoot 'get-restic.ps1') }
$profile = if ($DebugBuild) { 'debug' } else { 'release' }
$buildArgs = @('build', '--locked', '--bins')
if (!$DebugBuild) { $buildArgs += '--release' }
& (Join-Path $PSScriptRoot 'cargo.ps1') @buildArgs
if ($LASTEXITCODE -ne 0) { throw 'Compilarea a eșuat.' }
$targetDirectory = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $projectRoot 'target' }
$dist = Join-Path $projectRoot 'dist\StrajaBackup'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
foreach ($file in @('straja.exe', 'straja-agent.exe')) { Copy-Item -LiteralPath (Join-Path $targetDirectory "$profile\$file") -Destination $dist -Force }
Copy-Item -LiteralPath $restic -Destination $dist -Force
foreach ($file in @('README.md','README.ro.md','RELEASING.md','PERFORMANCE.md','LICENSE','THIRD-PARTY-NOTICES.txt','ARCHIVE-LICENSES.txt','GRAPHICS-LICENSES.txt','VALIDATION.md')) { Copy-Item -LiteralPath (Join-Path $projectRoot $file) -Destination $dist -Force }
New-Item -ItemType Directory -Path (Join-Path $dist 'scripts') -Force | Out-Null
foreach ($file in @('install-service.ps1','uninstall-service.ps1')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $dist 'scripts') -Force }
Write-Host "Aplicația este pregătită: $dist\straja.exe"
$binaryHashes = foreach ($file in @('straja.exe','straja-agent.exe','restic.exe')) {
    [pscustomobject]@{ File = $file; Hash = (Get-FileHash -LiteralPath (Join-Path $dist $file) -Algorithm SHA256).Hash }
}
$binaryHashes | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dist 'SHA256.json') -Encoding utf8
