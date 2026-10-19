#Requires -RunAsAdministrator
param([string]$DataDirectory = (Join-Path $env:LOCALAPPDATA 'StrajaBackup'))
$ErrorActionPreference = 'Stop'
$distribution = Split-Path $PSScriptRoot -Parent
foreach ($file in @('straja.exe','straja-agent.exe','restic.exe')) {
    if (!(Test-Path -LiteralPath (Join-Path $distribution $file))) { throw 'Rulează acest script din dist\StrajaBackup\scripts, după package.ps1.' }
}
if (Get-Service -Name 'StrajaBackup' -ErrorAction SilentlyContinue) { throw 'Serviciul există deja. Oprește și dezinstalează serviciul înainte de actualizare; datele rămân păstrate.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$credential = Get-Credential -UserName $identity.Name -Message 'Contul Windows pentru serviciul Straja. Introdu parola contului, nu PIN-ul Windows Hello.'
if (!$credential) { throw 'Instalare anulată.' }
$account = [Security.Principal.NTAccount]::new($credential.UserName)
$accountSid = $account.Translate([Security.Principal.SecurityIdentifier])
if ($accountSid.Value -ne $identity.User.Value) { throw 'Serviciul trebuie să ruleze sub același cont ca interfața, pentru a accesa parolele DPAPI.' }

# Grant only this account the Windows right required by the Service Control Manager.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class StrajaServiceRights {
    [StructLayout(LayoutKind.Sequential)] struct ObjectAttributes {
        public uint Length; public IntPtr RootDirectory, ObjectName; public uint Attributes;
        public IntPtr SecurityDescriptor, SecurityQualityOfService;
    }
    [StructLayout(LayoutKind.Sequential)] struct LsaString {
        public ushort Length, MaximumLength; public IntPtr Buffer;
    }
    [DllImport("advapi32.dll")] static extern uint LsaOpenPolicy(IntPtr system, ref ObjectAttributes attrs, uint access, out IntPtr policy);
    [DllImport("advapi32.dll")] static extern uint LsaAddAccountRights(IntPtr policy, byte[] sid, LsaString[] rights, uint count);
    [DllImport("advapi32.dll")] static extern uint LsaNtStatusToWinError(uint status);
    [DllImport("advapi32.dll")] static extern uint LsaClose(IntPtr policy);
    public static void Grant(byte[] sid) {
        var attrs = new ObjectAttributes(); attrs.Length = (uint)Marshal.SizeOf(attrs);
        IntPtr policy; uint result = LsaOpenPolicy(IntPtr.Zero, ref attrs, 0x810, out policy);
        if (result != 0) throw new System.ComponentModel.Win32Exception((int)LsaNtStatusToWinError(result));
        string name = "SeServiceLogonRight"; IntPtr buffer = Marshal.StringToHGlobalUni(name);
        try {
            var right = new LsaString { Length = (ushort)(name.Length * 2), MaximumLength = (ushort)((name.Length + 1) * 2), Buffer = buffer };
            result = LsaAddAccountRights(policy, sid, new[] {right}, 1);
            if (result != 0) throw new System.ComponentModel.Win32Exception((int)LsaNtStatusToWinError(result));
        } finally { Marshal.FreeHGlobal(buffer); LsaClose(policy); }
    }
}
'@
$sidBytes = New-Object byte[] $accountSid.BinaryLength
$accountSid.GetBinaryForm($sidBytes, 0)
[StrajaServiceRights]::Grant($sidBytes)
$installDirectory = Join-Path $env:ProgramFiles 'StrajaBackup'
New-Item -ItemType Directory -Path $installDirectory -Force | Out-Null
foreach ($file in @('straja.exe','straja-agent.exe','restic.exe','LICENSE','THIRD-PARTY-NOTICES.txt','README.md')) {
    $source = [IO.Path]::GetFullPath((Join-Path $distribution $file))
    $destination = [IO.Path]::GetFullPath((Join-Path $installDirectory $file))
    if ($source -ne $destination) { Copy-Item -LiteralPath $source -Destination $destination -Force }
}
New-Item -ItemType Directory -Path $DataDirectory -Force | Out-Null
$agentPath = Join-Path $installDirectory 'straja-agent.exe'
if ($agentPath.Contains('"') -or $DataDirectory.Contains('"')) { throw 'Calea nu poate conține ghilimele.' }
$binaryPath = '"' + $agentPath + '" --service --data-dir "' + $DataDirectory + '"'
New-Service -Name 'StrajaBackup' -DisplayName 'Straja Backup' -Description 'Backup programat și restaurare cu Restic.' -BinaryPathName $binaryPath -StartupType Automatic -Credential $credential | Out-Null
try {
    Start-Service -Name 'StrajaBackup'
    (Get-Service -Name 'StrajaBackup').WaitForStatus('Running', [TimeSpan]::FromSeconds(30))
    Start-Sleep -Seconds 2
    if ((Get-Service -Name 'StrajaBackup').Status -ne 'Running') { throw 'Serviciul s-a oprit. Verifică dacă agentul manual este deja pornit.' }
} catch { throw "Serviciul a fost înregistrat, dar pornirea a eșuat: $_" }
Write-Host "Serviciul rulează sub $($identity.Name). Interfață: $installDirectory\straja.exe"
