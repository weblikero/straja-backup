#ifndef AppVersion
  #define AppVersion "0.6.2"
#endif
#ifdef InstallerTest
  #define ProductName "Straja Backup Installer Test"
  #define ProductId "{{B42B0415-DA33-41E0-AFD3-B68EA7C05E13}"
  #define FolderName "StrajaBackupInstallerTest"
  #define ServiceName "StrajaBackupInstallerTest"
  #define Suffix "-test"
#else
  #define ProductName "Straja Backup"
  #define ProductId "{{D76D401F-7D3B-4B8A-9BF6-3491DE7C802D}"
  #define FolderName "StrajaBackup"
  #define ServiceName "StrajaBackup"
  #define Suffix ""
#endif

[Setup]
AppId={#ProductId}
AppName={#ProductName}
AppVersion={#AppVersion}
AppVerName={#ProductName} {#AppVersion}
AppPublisher=Weblike PRO
AppPublisherURL=https://weblike.ro/
AppSupportURL=https://strajabackup.com/
AppUpdatesURL=https://strajabackup.com/
AppComments=Created by Weblike PRO — an open-source project. Manual and scheduled backups with verified restore.
DefaultDirName={autopf}\{#FolderName}
DefaultGroupName={#ProductName}
DisableProgramGroupPage=yes
DisableWelcomePage=no
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir=..\dist\installer
OutputBaseFilename=StrajaBackup-{#AppVersion}-windows-x64{#Suffix}-setup
Compression=lzma2/ultra64
SolidCompression=yes
SetupIconFile=assets\straja.ico
UninstallDisplayIcon={app}\straja.ico
UninstallDisplayName={#ProductName}
Uninstallable=yes
CreateUninstallRegKey=yes
UninstallLogMode=append
VersionInfoVersion={#AppVersion}.0
VersionInfoCompany=Weblike PRO
VersionInfoDescription=Straja Backup for Windows Setup
VersionInfoProductName={#ProductName}
VersionInfoProductVersion={#AppVersion}
LicenseFile=..\LICENSE
WizardStyle=modern light windows11 includetitlebar
WizardSizePercent=120,115
WizardImageFile=assets\wizard.png
WizardSmallImageFile=assets\wizard-small.png
WizardImageBackColor=$30170C
WizardSmallImageBackColor=clWhite
SetupLogging=yes
CloseApplications=no
RestartApplications=no
ChangesAssociations=no
LanguageDetectionMethod=none
ShowLanguageDialog=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "portuguese"; MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "romanian"; MessagesFile: "Romanian.isl"

[LangOptions]
DialogFontName=Segoe UI
DialogFontSize=10
WelcomeFontName=Segoe UI
WelcomeFontSize=18

[Messages]
romanian.ArchiveIncorrectPassword=Parola este incorectă
romanian.ArchiveIsCorrupted=Arhiva este deteriorată
romanian.ArchiveUnsupportedFormat=Formatul arhivei nu este acceptat
romanian.ButtonStopExtraction=&Oprește extragerea
romanian.DownloadingLabel2=Se descarcă fișierele...
romanian.ErrorDownloading=A apărut o eroare la descărcarea unui fișier:
romanian.ErrorExtracting=A apărut o eroare la extragerea unei arhive:
romanian.ErrorExtractionAborted=Extragere anulată
romanian.ErrorExtractionFailed=Extragerea a eșuat: %1
romanian.ExtractingLabel=Se extrag fișierele...
romanian.RetryCancelCancel=Anulează
romanian.RetryCancelRetry=&Încearcă din nou
romanian.RetryCancelSelectAction=Alege o acțiune
romanian.SourceVerificationFailed=Verificarea fișierului sursă a eșuat: %1
romanian.StatusDownloadFiles=Se descarcă fișierele...
romanian.StopExtraction=Sigur dorești să oprești extragerea?
romanian.VerificationFileHashIncorrect=Suma de control a fișierului este incorectă
romanian.VerificationFileNameIncorrect=Numele fișierului este incorect
romanian.VerificationFileSizeIncorrect=Dimensiunea fișierului este incorectă
romanian.VerificationFileTagIncorrect=Eticheta fișierului este incorectă
romanian.VerificationKeyNotFound=Fișierul de semnătură "%1" folosește o cheie necunoscută
romanian.VerificationSignatureDoesntExist=Fișierul de semnătură "%1" nu există
romanian.VerificationSignatureInvalid=Fișierul de semnătură "%1" este nevalid
romanian.ButtonNext=&Continuă
romanian.ButtonBack=Îna&poi
romanian.ButtonWizardBrowse=&Alege folderul...
romanian.ClickNext=Apasă Continuă pentru a merge mai departe.
romanian.WizardLicense=Licență open-source
romanian.LicenseLabel=Citește termenii licenței MIT.
romanian.LicenseLabel3=Straja Backup este software open-source. Citește și acceptă licența pentru a continua.
romanian.WizardSelectDir=Locul instalării
romanian.SelectDirDesc=Unde vrei să instalezi Straja Backup?
romanian.SelectDirLabel3=Aplicația va fi instalată în folderul de mai jos. Pentru altă locație, apasă Alege folderul.
romanian.WizardSelectTasks=Preferințe
romanian.SelectTasksDesc=Alege cum vrei să folosești Straja.
romanian.SelectTasksLabel2=Poți activa opțiunile de mai jos:
romanian.WizardReady=Totul este pregătit
romanian.ReadyLabel1=Apasă Instalează pentru a continua.
romanian.WelcomeLabel1=Un loc sigur pentru%nfișierele tale
romanian.WelcomeLabel2=Instalează Straja Backup și păstrează controlul asupra datelor tale.%n%nBackup manual și programat%nCopii criptate sau fișiere normale, la alegere%nRestaurare verificată, într-un folder nou%n%nCreată de Weblike PRO — un proiect Open-Source.
romanian.FinishedHeadingLabel=Straja este pregătită.
romanian.FinishedLabel=Aplicația a fost instalată pe calculatorul tău.%n%nDeschide Straja, pornește agentul și creează primul plan de backup. Poți dezinstala aplicația oricând din Setări > Aplicații > Aplicații instalate.%n%nPlanurile și copiile tale sunt păstrate la dezinstalare.
romanian.BeveledLabel=Weblike PRO — un proiect Open-Source
english.BeveledLabel=Weblike PRO — an Open-Source project
english.WelcomeLabel1=A safer place for your files
english.WelcomeLabel2=Install Straja Backup and stay in control of your data.%n%nManual and scheduled backups%nEncrypted backups or ordinary files, your choice%nVerified restore to a new folder%n%nCreated by Weblike PRO — an Open-Source project.
english.FinishedHeadingLabel=Straja is ready.
english.FinishedLabel=Straja Backup has been installed.%n%nOpen Straja, start the agent and create your first backup plan. You can uninstall the app from Windows Settings > Apps > Installed apps.%n%nYour plans and backups are kept when uninstalling.

[CustomMessages]
romanian.DesktopShortcut=Creează o scurtătură pe desktop
romanian.StartAtLogin=Pornește agentul de backup la autentificarea în Windows
romanian.TrayAtLogin=Afișează pictograma Straja lângă ceas la autentificare
romanian.Shortcuts=Scurtături:
romanian.Background=Rulare în fundal:
romanian.LaunchApp=Deschide Straja Backup
romanian.AppBusy=Straja Backup sau agentul său rulează din acest folder. Așteaptă terminarea backupului, alege «Oprește agentul și pictograma» din meniul pictogramei de lângă ceas și închide aplicația, apoi încearcă din nou. Pentru serviciu, închide separat pictograma și oprește serviciul. Fișierele nu au fost modificate.
romanian.ServiceBusy=Serviciul Straja Backup este activ. Așteaptă finalizarea backupului și oprește serviciul din Windows Services înainte de a continua.
romanian.ServiceError=Serviciul Straja Backup nu a putut fi verificat sau eliminat. Rulează dezinstalarea ca administrator.
english.DesktopShortcut=Create a desktop shortcut
english.StartAtLogin=Start the backup agent when signing in to Windows
english.TrayAtLogin=Show the Straja notification icon when signing in
english.Shortcuts=Shortcuts:
english.Background=Background operation:
english.LaunchApp=Open Straja Backup
english.AppBusy=Straja Backup or its agent is running from this folder. Wait for the backup to finish, choose Stop agent and tray icon from the notification icon menu, and close the app. For a service, close the tray icon and stop the service separately. No application files were changed.
english.ServiceBusy=The Straja Backup service is running. Wait for the backup to finish and stop it in Windows Services before continuing.
english.ServiceError=The Straja Backup service could not be checked or removed. Run the uninstaller as administrator.

german.DesktopShortcut=Desktop-Verknüpfung erstellen
german.StartAtLogin=Backup-Agent bei der Windows-Anmeldung starten
german.TrayAtLogin=Straja-Symbol bei der Anmeldung im Infobereich anzeigen
german.Shortcuts=Verknüpfungen:
german.Background=Hintergrundbetrieb:
german.LaunchApp=Straja Backup öffnen
german.AppBusy=Straja Backup oder sein Agent läuft aus diesem Ordner. Warten Sie auf das Ende des Backups, wählen Sie im Symbolmenü „Agent und Tray-Symbol beenden“ und schließen Sie die App. Bei einem Dienst schließen Sie das Symbol und beenden den Dienst separat. Dateien wurden nicht geändert.
german.ServiceBusy=Der Straja-Backup-Dienst läuft. Warten Sie auf das Ende des Backups und beenden Sie ihn über Windows-Dienste.
german.ServiceError=Der Straja-Backup-Dienst konnte nicht geprüft oder entfernt werden. Starten Sie die Deinstallation als Administrator.
spanish.DesktopShortcut=Crear un acceso directo en el escritorio
spanish.StartAtLogin=Iniciar el agente de copias al iniciar sesión en Windows
spanish.TrayAtLogin=Mostrar el icono de Straja al iniciar sesión
spanish.Shortcuts=Accesos directos:
spanish.Background=Ejecución en segundo plano:
spanish.LaunchApp=Abrir Straja Backup
spanish.AppBusy=Straja Backup o su agente se está ejecutando desde esta carpeta. Espera a que termine la copia, elige «Detener agente e icono» en el menú del icono y cierra la aplicación. Si usas un servicio, cierra el icono y detén el servicio por separado. No se modificaron archivos.
spanish.ServiceBusy=El servicio Straja Backup está activo. Espera a que termine la copia y detén el servicio en Servicios de Windows.
spanish.ServiceError=No se pudo comprobar o eliminar el servicio Straja Backup. Ejecuta el desinstalador como administrador.
portuguese.DesktopShortcut=Criar um atalho no ambiente de trabalho
portuguese.StartAtLogin=Iniciar o agente de cópias ao iniciar sessão no Windows
portuguese.TrayAtLogin=Mostrar o ícone Straja ao iniciar sessão
portuguese.Shortcuts=Atalhos:
portuguese.Background=Execução em segundo plano:
portuguese.LaunchApp=Abrir Straja Backup
portuguese.AppBusy=A Straja Backup ou o agente está a executar nesta pasta. Aguarde o fim da cópia, escolha «Parar agente e ícone» no menu do ícone e feche a aplicação. Se usar um serviço, feche o ícone e pare o serviço separadamente. Os ficheiros não foram alterados.
portuguese.ServiceBusy=O serviço Straja Backup está ativo. Aguarde o fim da cópia e pare o serviço nos Serviços do Windows.
portuguese.ServiceError=Não foi possível verificar ou remover o serviço Straja Backup. Execute o desinstalador como administrador.
french.DesktopShortcut=Créer un raccourci sur le Bureau
french.StartAtLogin=Démarrer l’agent de sauvegarde à l’ouverture de session Windows
french.TrayAtLogin=Afficher l’icône Straja à l’ouverture de session
french.Shortcuts=Raccourcis :
french.Background=Exécution en arrière-plan :
french.LaunchApp=Ouvrir Straja Backup
french.AppBusy=Straja Backup ou son agent fonctionne depuis ce dossier. Attendez la fin de la sauvegarde, choisissez « Arrêter l’agent et l’icône » dans le menu de l’icône et fermez l’application. Pour un service, fermez l’icône et arrêtez le service séparément. Aucun fichier n’a été modifié.
french.ServiceBusy=Le service Straja Backup est actif. Attendez la fin de la sauvegarde et arrêtez-le dans Services Windows.
french.ServiceError=Impossible de vérifier ou supprimer le service Straja Backup. Exécutez la désinstallation en tant qu’administrateur.
italian.DesktopShortcut=Crea un collegamento sul desktop
italian.StartAtLogin=Avvia l’agente di backup all’accesso a Windows
italian.TrayAtLogin=Mostra l’icona Straja all’accesso
italian.Shortcuts=Collegamenti:
italian.Background=Esecuzione in background:
italian.LaunchApp=Apri Straja Backup
italian.AppBusy=Straja Backup o il suo agente è in esecuzione da questa cartella. Attendi la fine del backup, scegli «Ferma agente e icona» dal menu dell’icona e chiudi l’app. Per un servizio, chiudi l’icona e arresta il servizio separatamente. Nessun file è stato modificato.
italian.ServiceBusy=Il servizio Straja Backup è attivo. Attendi la fine del backup e arrestalo da Servizi di Windows.
italian.ServiceError=Impossibile verificare o rimuovere il servizio Straja Backup. Esegui la disinstallazione come amministratore.

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopShortcut}"; GroupDescription: "{cm:Shortcuts}"; Flags: unchecked
Name: "loginagent"; Description: "{cm:StartAtLogin}"; GroupDescription: "{cm:Background}"; Flags: unchecked; Check: not IsAdminInstallMode
Name: "logintray"; Description: "{cm:TrayAtLogin}"; GroupDescription: "{cm:Background}"; Check: not IsAdminInstallMode

[Files]
Source: "..\dist\StrajaBackup\straja.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\StrajaBackup\straja-agent.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\StrajaBackup\restic.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\straja.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\VSS.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.ro.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\ARCHIVE-LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\GRAPHICS-LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\install-service.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion
Source: "..\scripts\uninstall-service.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#ProductName}"; Filename: "{app}\straja.exe"; WorkingDir: "{app}"; IconFilename: "{app}\straja.ico"; AppUserModelID: "StrajaBackup.Desktop"
Name: "{autodesktop}\{#ProductName}"; Filename: "{app}\straja.exe"; WorkingDir: "{app}"; IconFilename: "{app}\straja.ico"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "{#FolderName}Tray"; Flags: deletevalue; Check: (not IsAdminInstallMode) and (not WizardIsTaskSelected('logintray'))
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#FolderName}Tray"; ValueData: """{app}\straja-agent.exe"" --tray"; Tasks: logintray; Flags: uninsdeletevalue; Check: not IsAdminInstallMode
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "{#FolderName}"; Flags: deletevalue; Check: (not IsAdminInstallMode) and (not WizardIsTaskSelected('loginagent'))
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#FolderName}"; ValueData: """{app}\straja-agent.exe"""; Tasks: loginagent; Flags: uninsdeletevalue; Check: not IsAdminInstallMode

[Run]
Filename: "{app}\straja.exe"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[Code]
type
  TServiceStatus = record
    ServiceType, CurrentState, ControlsAccepted, Win32ExitCode,
    ServiceSpecificExitCode, CheckPoint, WaitHint: LongWord;
  end;

function CreateFileW(Name: String; Access, Share: LongWord; Security: NativeUInt;
  Creation, Flags: LongWord; Template: NativeUInt): NativeUInt;
  external 'CreateFileW@kernel32.dll stdcall';
function CloseHandle(Handle: NativeUInt): Boolean;
  external 'CloseHandle@kernel32.dll stdcall';
function OpenSCManagerW(Machine, Database: NativeUInt; Access: LongWord): NativeUInt;
  external 'OpenSCManagerW@advapi32.dll stdcall';
function OpenServiceW(Manager: NativeUInt; Name: String; Access: LongWord): NativeUInt;
  external 'OpenServiceW@advapi32.dll stdcall';
function QueryServiceStatus(Service: NativeUInt; var Status: TServiceStatus): Boolean;
  external 'QueryServiceStatus@advapi32.dll stdcall';
function DeleteService(Service: NativeUInt): Boolean;
  external 'DeleteService@advapi32.dll stdcall';
function CloseServiceHandle(Handle: NativeUInt): Boolean;
  external 'CloseServiceHandle@advapi32.dll stdcall';

function FileIsLocked(Name: String): Boolean;
var Handle: NativeUInt;
begin
  Result := False;
  if not FileExists(Name) then exit;
  { Request write access without truncating: running executables reject this. }
  Handle := CreateFileW(Name, $40000000, 7, 0, 3, 0, 0);
  Result := Handle = NativeUInt(-1);
  if not Result then CloseHandle(Handle);
end;

function ApplicationBusy: Boolean;
begin
  Result := FileIsLocked(ExpandConstant('{app}\straja.exe')) or
            FileIsLocked(ExpandConstant('{app}\straja-agent.exe')) or
            FileIsLocked(ExpandConstant('{app}\restic.exe'));
end;

function OwnService: Boolean;
var ImagePath, Expected: String;
begin
  Expected := Lowercase(ExpandConstant('{app}\straja-agent.exe'));
  Result := RegQueryStringValue(HKLM64,
    'SYSTEM\CurrentControlSet\Services\{#ServiceName}', 'ImagePath', ImagePath);
  if Result then Result := Pos(Expected, Lowercase(ImagePath)) > 0;
end;

function CheckService(Remove: Boolean): String;
var Manager, Service: NativeUInt; Status: TServiceStatus; Access: LongWord;
begin
  Result := '';
  if not OwnService then exit;
  Manager := OpenSCManagerW(0, 0, 1);
  if Manager = 0 then begin Result := CustomMessage('ServiceError'); exit; end;
  Access := 4;
  if Remove then Access := Access or $10000;
  Service := OpenServiceW(Manager, '{#ServiceName}', Access);
  if Service = 0 then Result := CustomMessage('ServiceError')
  else begin
    if not QueryServiceStatus(Service, Status) then Result := CustomMessage('ServiceError')
    else if Status.CurrentState <> 1 then Result := CustomMessage('ServiceBusy')
    else if Remove and not DeleteService(Service) then Result := CustomMessage('ServiceError');
    CloseServiceHandle(Service);
  end;
  CloseServiceHandle(Manager);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := CheckService(False);
  if (Result = '') and ApplicationBusy then Result := CustomMessage('AppBusy');
end;

function InitializeUninstall: Boolean;
var Problem: String;
begin
  Problem := CheckService(False);
  if (Problem = '') and ApplicationBusy then Problem := CustomMessage('AppBusy');
  Result := Problem = '';
  if not Result then SuppressibleMsgBox(Problem, mbError, MB_OK, IDOK);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var Problem: String;
begin
  if CurUninstallStep = usUninstall then begin
    { Recheck after confirmation; never terminate a backup to remove the app. }
    if ApplicationBusy then RaiseException(CustomMessage('AppBusy'));
    Problem := CheckService(True);
    if Problem <> '' then RaiseException(Problem);
  end;
end;
