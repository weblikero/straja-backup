$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$toolsDir = Join-Path $projectRoot '.tools'
New-Item -ItemType Directory -Path $toolsDir -Force | Out-Null
$env:CARGO_HOME = Join-Path $toolsDir 'cargo'
$env:RUSTUP_HOME = Join-Path $toolsDir 'rustup'
$rustup = Join-Path $toolsDir 'rustup-init.exe'
if (!(Test-Path -LiteralPath (Join-Path $env:CARGO_HOME 'bin\cargo.exe'))) {
    Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $rustup
    & $rustup -y --no-modify-path --profile minimal --default-toolchain stable
    if ($LASTEXITCODE -ne 0) { throw 'Instalarea Rust a eșuat.' }
}
& (Join-Path $PSScriptRoot 'get-restic.ps1')
Write-Host 'Uneltele sunt pregătite. Compilare: .\scripts\package.ps1'
