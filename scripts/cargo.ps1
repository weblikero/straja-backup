param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$env:CARGO_HOME = Join-Path $projectRoot '.tools\cargo'
$env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
$cargoExe = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
if (!(Test-Path -LiteralPath $cargoExe)) { throw 'Rulează scripts\setup.ps1 înainte de compilare.' }
Push-Location $projectRoot
try { & $cargoExe @CargoArgs; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } }
finally { Pop-Location }
