$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$version = '1.75.2'
$name = "rclone-v$version-windows-amd64"
$archive = Join-Path $projectRoot ".tools\$name.zip"
New-Item -ItemType Directory -Path (Split-Path $archive) -Force | Out-Null
Invoke-WebRequest -Uri "https://github.com/rclone/rclone/releases/download/v$version/$name.zip" -OutFile $archive
# Pinned from the official GitHub release asset digest, reviewed on 2026-10-19.
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne '8a9db2bad1dc79a65b3348fd857069598e2aab7cf4d118b77b1df23055aff818') { throw 'Rclone SHA-256 mismatch; refusing extraction.' }
$extract = Join-Path $projectRoot '.tools\rclone'
Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
Copy-Item -LiteralPath (Join-Path $extract "$name\rclone.exe") -Destination (Join-Path $projectRoot '.tools\rclone.exe') -Force
Write-Host "Rclone $version downloaded and SHA-256 verified."
