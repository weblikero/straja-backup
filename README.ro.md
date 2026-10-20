Versiunea 0.7.0 adaugă destinații FTP/FTPS, Google Drive, Dropbox și Amazon S3 pentru backup criptat incremental. Ghid: [REMOTE.md](REMOTE.md). Formatele necriptate și arhivele rămân disponibile pentru destinațiile locale/de rețea.

Din versiunea 0.8.0 sunt disponibile și kituri native pentru Ubuntu 22.04+ (x64) și
macOS 14+ (Apple Silicon și Intel). Instalare, agent la autentificare și diferențe:
[NATIVE.md](NATIVE.md). Kiturile Mac nu sunt încă semnate Developer ID/notarizate.


> Actualizare 0.6.2: manifestele pentru fișiere necriptate și arhive acceptă până la 512 MiB. Citirea, scrierea și verificarea metadatelor folosesc buffere limitate; dimensiunea estimată se verifică înainte de copiere. Copiile întrerupte de limita veche rămân disponibile în `.in-progress-.../Fisiere`, dar necesită o rulare nouă pentru un backup complet validat.

> Actualizare 0.5.3: „Planurile mele” afișează carduri compacte cu sursa, destinația, frecvența, ultima stare și următoarea rulare. Excluderile, mesajele și acțiunile secundare sunt în „Detalii și acțiuni”.

> Actualizare 0.5.0: aplicația poate descărca versiuni noi din GitHub și propune „Închide și instalează”, cu confirmare și așteptarea operațiilor active. Versiunile până la 0.4.2 necesită o primă instalare manuală pentru a primi această funcție. Ghidul principal actualizat este [README.md](README.md); limitele de validare sunt în [VALIDATION.md](VALIDATION.md). Textul de mai jos este ghidul românesc anterior.

# Straja Backup — Windows, Rust și Restic

Creată de **[Weblike PRO](https://weblike.ro/)** — un proiect **Open-Source**, sub licență MIT.

Aplicație desktop în limba română, cu interfață Iced, agent separat și suport pentru Windows Service. Licență MIT. Versiunea 0.3 este o previzualizare funcțională pentru Windows x64, cu destinații locale și foldere de rețea. Backupul criptat folosește Restic 0.19.1, distribuit separat sub BSD-2-Clause. Motorul Rust poate salva fișiere direct accesibile sau arhive necriptate ZIP, TAR.GZ și 7z; nu necesită instalarea unui arhivator separat.

## Instalare Windows

Rulează `dist\installer\StrajaBackup-0.3.3-windows-x64-setup.exe`. Instalatorul include interfața, agentul și Restic, fără descărcări în timpul instalării. Are identitate vizuală proprie, interfață în română/engleză, alegerea folderului și opțiuni pentru scurtătura desktop și pornirea agentului la autentificare. Editorul afișat în Windows este **Weblike PRO**, cu link către **https://weblike.ro/**.

Logo-ul oficial Weblike PRO este inclus local și apare fără conexiune la internet. În bara laterală și în „Despre aplicație”, apăsarea logo-ului sau a linkului **weblike.ro** deschide site-ul în browserul implicit. Aplicația nu accesează site-ul automat.

Implicit instalează pentru utilizatorul curent, fără drepturi de administrator, în `%LOCALAPPDATA%\Programs\StrajaBackup`. Aplicația apare ca **Straja Backup** în **Add or remove programs / Installed apps**, cu versiune, pictogramă, dimensiune și comandă de dezinstalare. Există și scurtătură în meniul Start. Pentru instalare administrativă în Program Files, pornește instalatorul cu `/ALLUSERS`; serviciul se configurează separat, conform secțiunii de mai jos.

La actualizare se folosește aceeași identitate de produs. Înainte de actualizare/dezinstalare, așteaptă terminarea backupului și oprește agentul sau serviciul; instalatorul refuză modificarea executabilelor în uz. Dezinstalarea elimină fișierele instalate și scurtăturile, păstrând `%LOCALAPPDATA%\StrajaBackup`, parolele protejate, depozitele și fișierele utilizatorului. Un serviciu oprit este eliminat numai dacă aparține exact acestei locații; o copie de serviciu instalată separat în Program Files se dezinstalează separat.

Instalatorul și executabilele aplicației nu sunt încă semnate digital; semnarea pentru distribuție publică necesită un certificat al editorului.

## Pornire

### Excluderi direct din eroare — 0.3.3

Erorile de backup pentru junction/reparse point și fișier imposibil de citit oferă **Copiază calea** și, dacă acea cale este în interiorul unei surse actuale, **Adaugă la excluderi…**. Acțiunile apar în mesajul operației manuale, pe cardul planului și în **Istoric**, inclusiv pentru erorile salvate de versiunile anterioare. **Copiază mesajul** este disponibil și pentru erorile fără o cale recunoscută. Nu se preia calea copiei temporare din începutul mesajului.

**Adaugă la excluderi…** deschide planul cu calea completată în câmpul manual. Poți modifica liber textul înainte de **Adaugă excluderea** (sau **Salvează calea**, dacă era deja exclusă), apoi continui pașii și salvezi planul. Poți anula fără schimbări. Excluderile existente rămân editabile prin **Editează**; sunt acceptate `/` și `\`. Agentul trebuie să fie conectat și liber pentru editarea planului. Copierea textului rămâne disponibilă și fără agent sau dacă planul a fost șters.

Excluderea se aplică numai după salvarea planului, la backupurile viitoare. Nu pornește automat un backup și nu modifică fișierele, junction-ul sau copiile anterioare. Pentru alte tipuri de erori, copiază mesajul și introdu manual calea dorită în **Editează → Ce salvezi → Excluderi**.

### Introducerea și editarea manuală a excluderilor — 0.3.2

În **Plan nou / Editează planul → Ce salvezi → Ce NU se include în backup**, câmpul **Sau introdu manual calea completă** acceptă căi precum `C:/laragon/www/example-suite/public/build`. Apasă **Adaugă excluderea** sau Enter. Sunt acceptate atât `/`, cât și `\`, inclusiv o cale copiată între ghilimele duble. Calea este păstrată literal, fără rezolvarea junction-urilor către folderul țintă și fără a cere existența folderului în acel moment.

Fiecare cale existentă are **Editează**: modifică textul în același câmp, apoi apasă **Salvează calea**. **Anulează** abandonează textul nesalvat și păstrează excluderea anterioară. **Include din nou** elimină excluderea. După modificarea listei, continuă pașii și salvează planul. Formularul cere salvarea sau anularea textului introdus înainte de a trece la pasul următor; o cale invalidă sau duplicată nu înlocuiește excluderea existentă.

### Selectarea fișierelor și subfolderelor excluse — 0.3.1

În **Plan nou** sau **Editează planul**, la pasul **Ce salvezi**, adaugă mai întâi sursele. În secțiunea **Ce NU se include în backup**, folosește **+ Exclude foldere** sau **+ Exclude fișiere**, apoi selectează elementele din fereastra Windows. Lista arată căile complete; **Include din nou** elimină numai excluderea respectivă. Excluderile apar și pe cardul planului și în confirmarea finală.

Exemplu: sursa este `C:\FolderexempluA`, iar excluderea selectată este `C:\FolderexempluA\FolderB`. Se omite întregul `FolderB`, inclusiv subfolderele și fișierele adăugate ulterior. `C:\FolderexempluA\AltFolder\FolderB` și `FolderB-copie` rămân incluse, dacă nu există alte reguli care să le excludă. Se pot selecta și fișiere individuale, de exemplu `C:\FolderexempluA\privat.txt`.

Căile selectate sunt tratate literal, inclusiv când numele conține paranteze drepte sau `;`; comparația ignoră diferențele dintre litere mari și mici. Sunt acceptate cel mult 100 de căi selectate. O excludere trebuie să fie în interiorul unei surse; dacă ar elimina o sursă întreagă, elimină acea sursă din lista de backup. Dacă elimini sursa unui folder exclus, elimină și excluderea rămasă fără sursă. O cale exclusă poate lipsi temporar de pe disc și rămâne exclusă dacă reapare.

**Reguli avansate** păstrează tiparele existente, separate prin `;`, de exemplu `*.tmp; node_modules; target`. O regulă simplă precum `FolderB` poate omite orice folder cu acel nume; folosește selecția din Explorer pentru o singură cale exactă. Ambele tipuri de excluderi se aplică backupurilor viitoare, manuale sau programate, în toate cele cinci moduri. Nu modifică sursele sau conținutul copiilor deja create. Retenția configurată separat continuă să se aplice.

### Număr de copii și compresie — 0.3.0

În **Plan nou → destinație**, alege **Arhivă ZIP (necriptată)**, **Arhivă TAR.GZ (necriptată)** sau **Arhivă 7z (necriptată)**. Fiecare rulare produce o arhivă completă în `Backup-data-id/Backup.zip`, `Backup.tar.gz` sau `Backup.7z`. Arhiva conține folderul `Fisiere`, cu sursele numerotate, și manifestul Straja. Se deschide sau extrage cu orice program compatibil cu formatul ales, fără parolă și fără Straja. Pentru copy/paste direct fără extragere rămâne disponibil modul **Fișiere necriptate**.

Formatul unui plan existent rămâne neschimbat; pentru alt format creează un plan nou, cu alt folder gol. Nu există conversie a versiunilor vechi. Modurile de arhivare sunt necriptate; Restic rămâne opțiunea de backup criptat, cu compresia proprie.

În pasul **programare**, secțiunea **Câte copii păstrezi?** oferă **Păstrează toate copiile** (implicit) sau un număr între **1 și 10000**. Numărul include cea mai nouă copie: de exemplu 7 înseamnă copia nouă plus cel mult 6 anterioare. Limita poate fi modificată prin **Editează planul**, inclusiv pentru planurile existente. Schimbarea limitei nu șterge imediat nimic; se aplică la următorul backup manual sau programat reușit.

Arhivele sunt decomprimate în flux și verificate SHA-256 înainte de publicare. Când există copii de eliminat, Straja verifică toate copiile înainte de curățare; pentru Restic verifică integral depozitul, elimină numai snapshoturile acestui plan și recuperează spațiul prin `prune`. Erorile de backup nu pornesc curățarea. Un eșec la verificare/curățare lasă backupul nou salvat și apare ca **Reușit cu avertisment**, cu explicația în istoric. Copiile parțiale și fișierele străine nu se elimină automat. Verificarea și curățarea pot prelungi rularea.

Compresia folosește temporar pe destinație spațiul unei copii complete necomprimate plus arhiva nouă, pe lângă copiile deja păstrate. Fișierele temporare necomprimate se elimină după verificarea arhivei. La întrerupere poate rămâne un folder `.in-progress-...`, care se poate șterge manual după oprirea operațiilor. Nu modifica metadatele sau arhivele din depozit; pentru lucru extrage într-un folder separat.

### Meniu lizibil lângă ceas — 0.2.4

Meniul clic dreapta al pictogramei Straja transmite corect mesajele native Windows pentru desenare, temă și accesibilitate. Opțiunile active sunt lizibile, iar cele indisponibile folosesc aspectul dezactivat al sistemului. Fereastra monitorului rămâne receptivă cât timp meniul este deschis. Nu sunt schimbate tema sau culorile sistemului.

După instalarea acestei actualizări trebuie repornit și monitorul pictogramei. Dacă meniul vechi este ilizibil, închide Straja și, după terminarea operațiilor, închide procesele `straja-agent.exe` din Task Manager; alternativ, repornește Windows. Instalează noua versiune și pornește aplicația din Start. Un serviciu Windows se oprește din Services.

### Ștergerea unui plan — 0.2.3

În **Planurile mele**, apasă **Șterge planul…** și apoi **Șterge doar planul** în confirmarea afișată pe card. **Anulează** închide confirmarea fără modificări. Agentul trebuie să fie conectat, la aceeași versiune cu interfața, și să nu execute un backup, o restaurare sau o verificare. Dacă o operație începe după afișarea confirmării, agentul refuză ștergerea până la terminarea ei.

Se elimină planul, programarea și parola salvată pentru acel plan. Istoricul operațiilor rămâne, cu numele planului. Niciun fișier sursă și nicio copie de backup nu se șterg de pe disc. Ștergerea funcționează și când discul sursă sau destinația nu sunt conectate.

Planul eliminat nu mai apare la Restaurare și nu există încă funcție de import/anulare a ștergerii. Copiile necriptate rămân accesibile direct în Explorer. Pentru copiile criptate rămase pe disc ai nevoie de **Restic și de parola depozitului**; confirmarea cere să fi păstrat parola separat înainte de ștergerea planului. Dacă vrei doar să suspenzi rulările automate, folosește **Oprește programarea**.

### Fișiere necriptate, accesibile direct — 0.2.0

În **Plan nou**, la alegerea destinației, selectează **Fișiere necriptate (copy/paste)**. Nu se cere parolă. Alege un folder gol dedicat, de exemplu `E:\Straja-documente`, apoi programarea dorită. **Backup acum**, programarea, istoricul, verificarea și restaurarea funcționează pentru toate modurile.

Fiecare rulare produce un folder `Backup-20261018-...`, în care `Fisiere` conține sursele numerotate: `01-Documente`, `02-Poze`, `03-fisier.txt`. Numerotarea separă sursele cu același nume. Fișierele au formatul și numele originale; subfolderele și folderele goale se păstrează. Poți folosi **Deschide copiile în Explorer**, apoi deschide fișierele sau copia cu Ctrl+C/Ctrl+V, inclusiv pe alt PC fără Straja și fără parolă. Metadatele `.straja-*.json` sunt folosite doar de aplicație pentru identificare și verificare.

Fiecare versiune este o copie completă, fără compresie sau deduplicare, și ocupă spațiul tuturor fișierelor incluse. Modificarea sau ștergerea sursei nu afectează versiunile deja terminate. Implicit se păstrează toate copiile; o limită configurată în plan activează curățarea automată. Nu modifica fișierele dintr-o copie dacă vrei să treacă verificarea ulterioară; copiază-le întâi într-un folder de lucru.

Conținutul copiat este recitit și verificat prin SHA-256 înainte de finalizare. Un fișier blocat sau o eroare lasă un folder `.in-progress-...`, care nu apare ca versiune reușită. După oprirea operațiilor poți șterge manual aceste copii parțiale. Restaurarea verifică toate fișierele și creează un folder nou; întreruperea poate lăsa un folder `.Straja-Restaurare-incompleta-...`.

În acest mod sunt păstrate conținutul obișnuit al fișierelor și data ultimei modificări a fișierelor. Nu sunt copiate ACL-uri, fluxuri NTFS alternative (ADS) sau alte metadate speciale. Legăturile, junction-urile și reparse points (inclusiv unele fișiere cloud) sunt refuzate explicit: selectează fișiere locale obișnuite sau exclude legăturile. Din versiunea 0.6.0, VSS poate fi activat separat în plan pentru surse locale NTFS. Excluderile acceptă tipare precum `*.tmp`, `cache`, `subfolder/*.log`, fără diferențiere între litere mari și mici; prefixul `!` nu este acceptat.

Planurile existente rămân criptate. Pentru copii necriptate creează un plan nou și o destinație nouă; nu există conversie automată a depozitelor vechi. Oricine are acces la discul cu copii necriptate poate citi conținutul lor.

### Pictograma de lângă ceas — 0.1.2

Interfața și agentul interactiv pornesc un monitor separat în zona de notificări Windows. Pictograma rămâne după închiderea ferestrei. Scutul are un indicator verde pentru agent disponibil, galben pentru operație în curs, roșu pentru eroare/agent vechi și gri pentru agent deconectat. Textul afișat la trecerea cursorului precizează starea și progresul. Windows poate grupa pictograma sub săgeata **^** a zonei de notificări.

Un clic deschide aplicația. Meniul clic dreapta oferă **Deschide Straja Backup**, **Pornește agentul**, **Oprește agentul și pictograma** și **Închide doar pictograma (agentul continuă)**. Oprirea este refuzată și de agent dacă între timp a început un backup, o restaurare sau o verificare. Un serviciu se oprește din Windows Services. Monitorul nu execută backupuri și închiderea lui nu le întrerupe.

Instalatorul oferă separat afișarea pictogramei la autentificare și pornirea agentului la autentificare. Pentru serviciul Windows, pictograma este un proces al utilizatorului conectat, nu al serviciului din sesiunea 0. Aplicația trebuie deschisă după autentificare sau opțiunea de pornire a pictogramei trebuie activată. Există o singură pictogramă per utilizator/sesiune; ea se reînregistrează când Explorer repornește.

Înainte de o actualizare viitoare, alege **Oprește agentul și pictograma**, apoi închide fereastra. Dacă folosești serviciul, oprește-l și închide separat pictograma. Din versiunile fără pictogramă (0.1.0/0.1.1), agentul se oprește din Task Manager după terminarea operațiilor.

### Corecții în 0.1.1

Planul afișează căile complete ale surselor, excluderile și rezultatul ultimei încercări de backup. Butonul **Editează** permite schimbarea numelui, surselor, excluderilor și programării, păstrând ID-ul, parola protejată și istoricul. Destinația poate fi corectată înainte de inițializarea depozitului; după inițializare se folosește un plan nou pentru altă destinație.

Un folder nevid este respins înainte de salvarea unui plan nou. Alege un subfolder gol dedicat, de exemplu `E:\Straja-test`, nu direct `E:\`. Poți crea folderul din dialogul de alegere a destinației. Pentru un plan vechi cu destinație nevalidă, apasă **Editează**, păstrează sursele, alege folderul nou și salvează.

**Backup acum** urmărește operația până la rezultat, inclusiv dacă eșuează înainte de prima actualizare a progresului. Mesajul de acceptare este înlocuit cu rezultatul final și eroarea exactă.

La actualizarea din 0.1.0, așteaptă terminarea operațiilor, închide interfața și oprește agentul vechi (Task Manager → `straja-agent.exe`; pentru serviciu, Windows Services). Instalează 0.1.2, apoi deschide aplicația din Start și pornește agentul. Interfața detectează un agent vechi și blochează operațiile incompatibile. Datele existente sunt păstrate.

Deschide **Straja Backup** din Start (sau `dist\StrajaBackup\straja.exe` pentru varianta portabilă), apasă **Pornește agentul**, apoi **Creează primul plan**. Agentul rămâne activ după închiderea interfeței, până la oprirea sesiunii Windows sau a procesului. Dacă ai bifat pornirea la autentificare în instalator, agentul pornește automat la următoarea autentificare.

1. Alege numele planului și fișierele/folderele. Subfolderele și fișierele noi sunt incluse automat. Excluderile sunt separate prin `;`; modul criptat folosește sintaxa Restic.
2. Alege un folder gol, dedicat planului, pe alt disc sau într-o locație de rețea accesibilă contului. Pentru serviciu folosește căi UNC, nu litere de disc mapate în sesiunea interactivă.
3. Alege modul criptat sau necriptat. Pentru cel criptat, introdu și repetă parola depozitului. Păstreaz-o separat: DPAPI ajută rulările automate pe acest PC, dar nu înlocuiește parola de recuperare pe alt PC.
4. Alege rulare manuală, interval (minimum 15 minute), zilnică sau săptămânală la ora locală.
5. Salvează planul și pornește **Backup acum**. Depozitul este inițializat la prima rulare, numai dacă folderul este gol.
6. În **Restaurare**, alege planul, versiunea și un folder-destinație. Se creează un subfolder unic `Straja-Restaurare-...`; motorul ales verifică conținutul recuperat. Sursele existente nu sunt suprascrise.

**Verifică** citește și verifică toate datele depozitului. Istoricul diferențiază `Reușit`, `Reușit cu avertisment`, `Incomplet`, `Eroare` și `Întrerupt`. Fișierele blocate nu sunt raportate ca un backup complet.

## Compilare și teste

Necesare: Windows x64, PowerShell și Visual Studio Build Tools cu C++/Windows SDK. Scriptul de pregătire instalează Rust numai în `.tools`, fără modificarea PATH-ului sistemului, și descarcă Restic din release-ul oficial GitHub, verificând SHA-256 față de `SHA256SUMS`.

```powershell
.\scripts\setup.ps1
.\scripts\test.ps1
.\scripts\package.ps1
.\scripts\build-installer.ps1 -SkipBuild
```

Pentru compilare rapidă de dezvoltare: `scripts\package.ps1 -DebugBuild`. Executabilele și motorul Restic se găsesc împreună în `dist\StrajaBackup`. Păstrează întregul folder, nu doar interfața. Pachetul de previzualizare nu este semnat digital.

Instalatorul se compilează cu Inno Setup 7.1.0, pregătit portabil în `.tools\inno`, cu verificarea semnăturii editorului. `build-installer.ps1` generează și suma SHA-256 a pachetului. Identitatea vizuală este generată local prin `installer-assets.ps1`. Pentru utilizări comerciale ale compilatorului, verifică termenii Inno Setup: https://jrsoftware.org/isinfo.php.

Testul real de instalare folosește un AppId, un nume și un folder distincte și elimină la final propria înregistrare Windows:

```powershell
.\scripts\build-installer.ps1 -SkipBuild -TestVariant
.\scripts\test-installer.ps1
```

Comenzi Cargo cu unelte locale:

```powershell
.\scripts\cargo.ps1 check --all-targets --locked
.\scripts\cargo.ps1 test --all-targets --locked
.\scripts\cargo.ps1 -CargoArgs @('test', '--test', 'restic_roundtrip', '--', '--ignored', '--nocapture')
.\scripts\cargo.ps1 -CargoArgs @('test', '--test', 'worker_lifecycle', '--', '--ignored', '--nocapture')
```

`--` trebuie transmis prin tabloul `-CargoArgs`, pentru a fi păstrat de PowerShell. Testele Restic și de agent folosesc exclusiv fișiere temporare; testul agentului are un named pipe unic, separat de aplicația utilizatorului.

## Serviciu Windows

Distribuția include suport SCM real prin `windows-service`. Agentul și interfața trebuie să ruleze sub **același cont Windows**, deoarece parolele sunt protejate cu DPAPI în domeniul utilizatorului.

Închide agentul manual înainte de pornirea serviciului (Task Manager → procesul `straja-agent.exe`). Așteaptă finalizarea oricărei operații de backup. Din PowerShell **ca administrator**, sub același cont folosit pentru interfață:

```powershell
.\dist\StrajaBackup\scripts\install-service.ps1
```

Scriptul cere parola contului Windows, nu PIN-ul Windows Hello; acordă contului dreptul „Log on as a service”, copiază executabilele în `Program Files\StrajaBackup` și înregistrează serviciul `StrajaBackup` cu pornire automată. Parola contului este gestionată de Windows Service Control Manager, nu salvată în fișierele aplicației. Conturile fără parolă sau politicile de domeniu pot împiedica autentificarea ca serviciu. Schimbarea parolei contului necesită actualizarea credențialelor serviciului.

Interfața se poate deschide apoi din `Program Files\StrajaBackup\straja.exe`. Datele rămân în `%LOCALAPPDATA%\StrajaBackup`, explicit configurat pentru serviciu. Scriptul refuză instalarea peste un serviciu existent.

Dezinstalare:

```powershell
.\dist\StrajaBackup\scripts\uninstall-service.ps1
```

Se elimină înregistrarea serviciului; executabilele, configurația, dreptul de logare al contului și depozitele de backup sunt păstrate. Pentru actualizare, oprește/dezinstalează serviciul, pregătește noua distribuție și instalează din nou.

## Comportament și limite ale versiunii 0.3

- Planurile, programările și istoricul sunt persistate în SQLite. O singură operație de backup/restaurare/verificare rulează la un moment dat.
- Programarea este verificată la 15 secunde. O execuție ratată este recuperată o singură dată la revenire; erorile sunt reîncercate după 15 minute. PC-ul nu este trezit din sleep.
- Orele zilnice/săptămânale folosesc fusul local Windows. O oră inexistentă la trecerea la ora de vară este omisă; ora repetată toamna rulează o singură dată.
- Implicit se păstrează toate versiunile. Limita configurată în plan se aplică după un backup nou reușit și verificat. Notificările de sistem nu sunt încă implementate. Planurile pot fi editate sau șterse fără eliminarea copiilor; programarea poate fi oprită/reactivată din interfață.
- Restaurarea este pentru întreaga copie selectată, cu eliminarea structurii de părinți comuni unde este posibil. Nu există încă navigator pentru selecția fișierelor individuale. La surse de pe volume diferite sau cu permisiuni speciale, Windows/Restic poate refuza restaurarea unor metadate; rezultatul este raportat ca eroare, nu ascuns.
- VSS este opțional, dezactivat implicit. Fără VSS, fișierele blocate pot produce copii incomplete. Vezi [VSS.md](VSS.md) pentru activare și limite. Bazele de date active necesită export consistent înainte de backup. Aceasta nu este o soluție de imagine bootabilă a sistemului.
- Închiderea forțată poate lăsa un lock Restic. Aplicația nu execută automat `unlock`; verifică întâi că nu există alte procese Restic care folosesc depozitul, apoi folosește uneltele oficiale Restic.
- Destinațiile cloud, importul depozitelor existente după reinstalare, MSI, auto-update și semnarea executabilelor nu sunt implementate. Copiile criptate rămân accesibile cu Restic și parola depozitului, inclusiv fără Straja; cele necriptate sunt accesibile direct în Explorer.
- Nu s-a instalat automat niciun serviciu pe calculator în timpul dezvoltării. Integrarea SCM necesită validare pe o mașină de test cu un cont Windows și parolă potrivite; testele automate acoperă agentul, programarea și instalatorul, nu logarea SCM.

## Structură și protecția datelor

`src/ui.rs` — interfață; `src/worker.rs` — agent și scheduler; `src/service.rs` — Windows SCM; `src/engine.rs` — procese Restic și selecția motorului; `src/plain.rs` — versiuni necriptate, verificare SHA-256 și curățare; `src/archive.rs` — compresie și restaurare ZIP/TAR.GZ/7z; `src/store.rs` — SQLite; `src/security.rs` — DPAPI și identificarea contului; `src/protocol.rs` — mesaje IPC cu lungime limitată.

Named pipe-ul permite acces numai contului curent și SYSTEM, respinge clienții remote și previne două instanțe de agent. Parolele depozitelor sunt criptate pe disc, iar către Restic sunt transmise pe stdin, fără shell, argumente cu parolă sau fișiere temporare cu parole. Sursa și destinația sunt verificate pentru suprapunere, inclusiv prin rezolvarea junction-urilor Windows la momentul rulării. Datele interne ale aplicației sunt excluse din backup.

Codul este open-source; proiectul nou nu are încă o comunitate proprie. Licențele dependențelor trebuie reunite complet înainte de distribuția publică. Vezi `THIRD-PARTY-NOTICES.txt` și `Cargo.lock`.
