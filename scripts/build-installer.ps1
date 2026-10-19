param([switch]$SkipBuild, [switch]$TestVariant)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (!$SkipBuild) { & (Join-Path $PSScriptRoot 'package.ps1') }
foreach ($file in @('straja.exe','straja-agent.exe','restic.exe','rclone.exe','README.md','LICENSE','THIRD-PARTY-NOTICES.txt')) {
    if (!(Test-Path -LiteralPath (Join-Path $projectRoot "dist\StrajaBackup\$file"))) { throw "Fișier lipsă din distribuție: $file" }
}
& (Join-Path $PSScriptRoot 'get-inno.ps1')
& (Join-Path $PSScriptRoot 'installer-assets.ps1')
$version = [regex]::Match((Get-Content -LiteralPath (Join-Path $projectRoot 'Cargo.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (!$version) { throw 'Versiunea aplicației nu a putut fi citită.' }
$compilerArgs = @("/DAppVersion=$version")
if ($TestVariant) { $compilerArgs += '/DInstallerTest=1' }
$compilerArgs += (Join-Path $projectRoot 'installer\straja.iss')
& (Join-Path $projectRoot '.tools\inno\ISCC.exe') @compilerArgs
if ($LASTEXITCODE -ne 0) { throw 'Compilarea instalatorului a eșuat.' }
$suffix = if ($TestVariant) { '-test' } else { '' }
$setup = Join-Path $projectRoot "dist\installer\StrajaBackup-$version-windows-x64$suffix-setup.exe"
$hash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath ($setup + '.sha256') -Value ($hash + '  ' + (Split-Path $setup -Leaf)) -Encoding ascii
Write-Host "Instalator: $setup"
