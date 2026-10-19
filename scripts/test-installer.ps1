param([string]$Version = '0.6.2', [ValidateSet('english','german','spanish','portuguese','french','italian','romanian')][string]$Language = 'english')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$setup = Join-Path $projectRoot "dist\installer\StrajaBackup-$Version-windows-x64-test-setup.exe"
if (!(Test-Path -LiteralPath $setup)) { throw 'Compilează mai întâi build-installer.ps1 -SkipBuild -TestVariant.' }
$testName = 'Straja Backup Installer Test'
$uninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{B42B0415-DA33-41E0-AFD3-B68EA7C05E13}_is1'
$runKey = 'Software\Microsoft\Windows\CurrentVersion\Run'
$runName = 'StrajaBackupInstallerTest'
$registry = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, [Microsoft.Win32.RegistryView]::Registry64)
$menuLink = Join-Path ([Environment]::GetFolderPath('Programs')) "$testName.lnk"
$desktopLink = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) "$testName.lnk"
if ($registry.OpenSubKey($uninstallKey) -or (Test-Path -LiteralPath $menuLink) -or (Test-Path -LiteralPath $desktopLink)) {
    throw 'Există deja o instalare/scurtătură de test. Nu va fi suprascrisă.'
}
$key = $registry.OpenSubKey($runKey)
try { if ($key -and (($null -ne $key.GetValue($runName)) -or ($null -ne $key.GetValue($runName + 'Tray')))) { throw 'Există deja o intrare Run de test.' } } finally { if ($key) { $key.Dispose() } }
$testRoot = Join-Path $projectRoot ('test-output\installer-' + [Guid]::NewGuid().ToString('N'))
$appDir = Join-Path $testRoot 'app'
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$canary = Join-Path $testRoot 'backup-canary.txt'
Set-Content -LiteralPath $canary -Value 'Date de test care trebuie păstrate.' -Encoding utf8
$canaryHash = (Get-FileHash -LiteralPath $canary).Hash
$common = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/CURRENTUSER',('/LANG=' + $Language),('/DIR="' + $appDir + '"'))

function Assert-Test([bool]$Condition, [string]$Message) {
    if (!$Condition) { throw $Message }
}
function Invoke-TestProcess([string]$Exe, [string[]]$ProcessArgs, [string]$LogName) {
    $processArgs += ('/LOG="' + (Join-Path $testRoot "$LogName.log") + '"')
    $process = Start-Process -FilePath $Exe -ArgumentList $processArgs -WindowStyle Hidden -PassThru -Wait
    Write-Host "$LogName : exit $($process.ExitCode)"
    return $process.ExitCode
}
function Read-UninstallEntry {
    $entry = $registry.OpenSubKey($uninstallKey)
    if (!$entry) { return $null }
    try {
        $values = @{}
        foreach ($name in $entry.GetValueNames()) { $values[$name] = $entry.GetValue($name) }
        return $values
    } finally { $entry.Dispose() }
}
function Read-StartupEntry([string]$Name = $runName) {
    $entry = $registry.OpenSubKey($runKey)
    if (!$entry) { return $null }
    try { return $entry.GetValue($Name) } finally { $entry.Dispose() }
}

$uninstaller = Join-Path $appDir 'unins000.exe'
try {
    Assert-Test ((Invoke-TestProcess $setup ($common + '/TASKS="desktopicon,loginagent,logintray"') 'install') -eq 0) 'Instalarea a eșuat.'
    Assert-Test ((Get-FileHash -LiteralPath (Join-Path $appDir 'ARCHIVE-LICENSES.txt')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $projectRoot 'ARCHIVE-LICENSES.txt')).Hash) 'Lipsesc licențele bibliotecilor de arhivare.'
    $entry = Read-UninstallEntry
    Assert-Test ($null -ne $entry) 'Lipsește intrarea Add/remove Programs.'
    Assert-Test ($entry.DisplayName -eq $testName) 'DisplayName incorect.'
    Assert-Test ($entry.DisplayVersion -eq $Version) 'DisplayVersion incorect.'
    Assert-Test ($entry.Publisher -eq 'Weblike PRO') 'Publisher incorect.'
    Assert-Test ($entry.URLInfoAbout -eq 'https://weblike.ro/') 'Linkul editorului este incorect.'
    Assert-Test ($entry.HelpLink -eq 'https://strajabackup.com/') 'Linkul de asistență este incorect.'
    Assert-Test ($entry.InstallLocation.TrimEnd('\') -eq $appDir) 'InstallLocation incorect.'
    Assert-Test ($entry.DisplayIcon -eq (Join-Path $appDir 'straja.ico')) 'Pictograma nu este înregistrată.'
    Assert-Test ($entry.UninstallString.Contains($uninstaller)) 'Comandă de dezinstalare incorectă.'
    Assert-Test ($entry.QuietUninstallString.Contains('/SILENT')) 'Lipsește dezinstalarea silențioasă.'
    Assert-Test ($entry.EstimatedSize -gt 0) 'Dimensiunea nu este înregistrată.'
    Assert-Test ((Test-Path -LiteralPath $menuLink) -and (Test-Path -LiteralPath $desktopLink)) 'Lipsesc scurtăturile.'
    Assert-Test ((Read-StartupEntry) -eq ('"' + (Join-Path $appDir 'straja-agent.exe') + '"')) 'Pornirea la autentificare nu este înregistrată.'
    Assert-Test ((Read-StartupEntry ($runName + 'Tray')) -eq ('"' + (Join-Path $appDir 'straja-agent.exe') + '" --tray')) 'Pictograma nu este înregistrată pentru autentificare.'
    foreach ($file in @('straja.exe','straja-agent.exe','restic.exe')) {
        $sourceHash = (Get-FileHash -LiteralPath (Join-Path $projectRoot "dist\StrajaBackup\$file")).Hash
        Assert-Test ((Get-FileHash -LiteralPath (Join-Path $appDir $file)).Hash -eq $sourceHash) "Conținut diferit: $file"
    }
    $entry | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'registration.json') -Encoding utf8
    $marker = Join-Path $appDir 'user-note.txt'
    Set-Content -LiteralPath $marker -Value 'Fișier neadministrat de instalator.' -Encoding utf8
    $markerHash = (Get-FileHash -LiteralPath $marker).Hash

    # Deny write access just as a running executable does, without launching an agent.
    $lockedFile = [IO.File]::Open((Join-Path $appDir 'straja.exe'), [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        Assert-Test ((Invoke-TestProcess $setup ($common + '/TASKS=""') 'locked-upgrade') -ne 0) 'Actualizarea nu a refuzat fișierul în uz.'
        Assert-Test ((Invoke-TestProcess $uninstaller @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') 'locked-uninstall') -ne 0) 'Dezinstalarea nu a refuzat fișierul în uz.'
        Assert-Test ($null -ne (Read-UninstallEntry)) 'Dezinstalarea blocată a eliminat înregistrarea.'
    } finally { $lockedFile.Dispose() }

    # Match the in-app updater's arguments and read-sharing lock on the installer.
    $inAppArgs = @('/SP-','/SILENT','/SUPPRESSMSGBOXES','/NORESTART','/NOCLOSEAPPLICATIONS','/NORESTARTAPPLICATIONS','/RESTARTEXITCODE=3010','/CURRENTUSER',('/LANG=' + $Language),('/DIR="' + $appDir + '"'))
    $installerReadLock = [IO.File]::Open($setup, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        Assert-Test ((Invoke-TestProcess $setup $inAppArgs 'in-app-upgrade') -eq 0) 'The in-app installer invocation failed.'
    } finally { $installerReadLock.Dispose() }
    Assert-Test ((Read-UninstallEntry).InstallLocation.TrimEnd('\') -eq $appDir) 'The update changed the installation folder.'
    Assert-Test ((Read-StartupEntry) -eq ('"' + (Join-Path $appDir 'straja-agent.exe') + '"')) 'The update did not preserve agent startup.'
    Assert-Test ((Read-StartupEntry ($runName + 'Tray')) -eq ('"' + (Join-Path $appDir 'straja-agent.exe') + '" --tray')) 'The update did not preserve tray startup.'
    Assert-Test ((Get-FileHash -LiteralPath $marker).Hash -eq $markerHash) 'The update changed the user canary.'

    Assert-Test ((Invoke-TestProcess $setup ($common + '/TASKS=""') 'upgrade') -eq 0) 'Actualizarea a eșuat.'
    Assert-Test ($null -ne (Read-UninstallEntry)) 'Actualizarea a pierdut înregistrarea.'
    Assert-Test ($null -eq (Read-StartupEntry)) 'Debifarea pornirii la autentificare nu a șters intrarea Run.'
    Assert-Test ($null -eq (Read-StartupEntry ($runName + 'Tray'))) 'Debifarea pictogramei nu a șters intrarea Run.'
    Assert-Test ((Get-FileHash -LiteralPath $marker).Hash -eq $markerHash) 'Actualizarea a modificat fișierele utilizatorului.'

    Assert-Test ((Invoke-TestProcess $uninstaller @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') 'uninstall') -eq 0) 'Dezinstalarea a eșuat.'
    Assert-Test ($null -eq (Read-UninstallEntry)) 'Intrarea Add/remove Programs a rămas după dezinstalare.'
    Assert-Test ($null -eq (Read-StartupEntry)) 'Intrarea Run a rămas după dezinstalare.'
    Assert-Test ($null -eq (Read-StartupEntry ($runName + 'Tray'))) 'Intrarea Run a pictogramei a rămas după dezinstalare.'
    Assert-Test (!(Test-Path -LiteralPath $menuLink) -and !(Test-Path -LiteralPath $desktopLink)) 'Scurtăturile au rămas după dezinstalare.'
    foreach ($file in @('straja.exe','straja-agent.exe','restic.exe','straja.ico')) {
        Assert-Test (!(Test-Path -LiteralPath (Join-Path $appDir $file))) "Fișier rămas: $file"
    }
    Assert-Test ((Get-FileHash -LiteralPath $marker).Hash -eq $markerHash) 'Dezinstalarea a șters fișierele utilizatorului.'
    Assert-Test ((Get-FileHash -LiteralPath $canary).Hash -eq $canaryHash) 'Dezinstalarea a modificat datele externe.'
    Write-Host "PASS: instalare, înregistrare Windows, scurtături, pornire la login, protecție fișiere în uz, actualizare, dezinstalare și păstrare date. Loguri: $testRoot"
} finally {
    # Only the unique test AppId is eligible for cleanup; production is never touched.
    $entry = Read-UninstallEntry
    if ($entry -and $entry.InstallLocation.TrimEnd('\') -eq $appDir -and (Test-Path -LiteralPath $uninstaller)) {
        $cleanupExit = Invoke-TestProcess $uninstaller @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') 'cleanup'
        if ($cleanupExit -ne 0) { Write-Warning "Curățarea testului necesită verificare: $uninstaller" }
    }
    $registry.Dispose()
}
