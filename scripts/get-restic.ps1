$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$toolsDir = Join-Path $projectRoot '.tools'
New-Item -ItemType Directory -Path $toolsDir -Force | Out-Null
$version = '0.19.1'
$asset = "restic_${version}_windows_amd64.zip"
$baseUrl = "https://github.com/restic/restic/releases/download/v$version"
$archive = Join-Path $toolsDir $asset
$checksumFile = Join-Path $toolsDir 'restic-SHA256SUMS'
Invoke-WebRequest -Uri "$baseUrl/$asset" -OutFile $archive
Invoke-WebRequest -Uri "$baseUrl/SHA256SUMS" -OutFile $checksumFile
$line = Get-Content -LiteralPath $checksumFile | Where-Object { $_ -match ([regex]::Escape($asset) + '$') } | Select-Object -First 1
if (!$line) { throw 'Arhiva nu apare în lista oficială de checksumuri.' }
$expected = ($line -split '\s+')[0]
$actual = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw 'Checksum Restic invalid. Arhiva nu va fi extrasă.' }
$extractDir = Join-Path $toolsDir 'restic'
Expand-Archive -LiteralPath $archive -DestinationPath $extractDir -Force
Copy-Item -LiteralPath (Join-Path $extractDir "restic_${version}_windows_amd64.exe") -Destination (Join-Path $toolsDir 'restic.exe') -Force
Write-Host "Restic $version descărcat și verificat SHA-256."
