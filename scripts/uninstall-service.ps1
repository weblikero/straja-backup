#Requires -RunAsAdministrator
$ErrorActionPreference = 'Stop'
$service = Get-Service -Name 'StrajaBackup' -ErrorAction SilentlyContinue
if (!$service) { Write-Host 'Serviciul nu este instalat.'; return }
if ($service.Status -ne 'Stopped') {
    Stop-Service -Name 'StrajaBackup'
    $service.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(45))
}
& sc.exe delete StrajaBackup
if ($LASTEXITCODE -ne 0) { throw 'Eliminarea serviciului a eșuat.' }
Write-Host 'Serviciul a fost eliminat. Configurația, parolele protejate și backupurile au fost păstrate.'
