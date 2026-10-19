$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$compilerDir = Join-Path $projectRoot '.tools\inno'
$compiler = Join-Path $compilerDir 'ISCC.exe'
if (Test-Path -LiteralPath $compiler) { Write-Host "Compilator disponibil: $compiler"; return }
$download = Join-Path $projectRoot '.tools\innosetup-7.1.0-x64.exe'
New-Item -ItemType Directory -Path (Split-Path $download -Parent) -Force | Out-Null
Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_0/innosetup-7.1.0-x64.exe' -OutFile $download
$signature = Get-AuthenticodeSignature -LiteralPath $download
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike '*Pyrsys B.V.*') { throw 'Semnătura oficială Inno Setup nu este validă.' }
$setup = Start-Process -FilePath $download -ArgumentList @('/PORTABLE=1','/CURRENTUSER','/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/NOICONS',('/DIR="' + $compilerDir + '"')) -WindowStyle Hidden -PassThru -Wait
if ($setup.ExitCode -ne 0 -or !(Test-Path -LiteralPath $compiler)) { throw 'Pregătirea compilatorului portabil a eșuat.' }
Write-Host "Compilator pregătit: $compiler"
