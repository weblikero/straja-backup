# Validare — 19 octombrie 2026

Testat local pe Windows x64, cu Rust 1.99.1 și Restic 0.19.1. Datele de test au fost generate în directoare temporare, fără folosirea documentelor utilizatorului.

## Rezultate

### GitHub update discovery — 0.4.1

- Public source/update repository: `weblikero/straja-backup`. Read-only HTTPS check in the GUI, per-user 24-hour cache, manual check with a one-minute cooldown, release notification and browser download link. No installer execution and no change to backup operations. Seven-language UI.
- 53 distinct tests passed: 51 through `scripts/test.ps1`, plus an oversized-response test and a live read-only GitHub repository/releases API smoke test. Tests cover numeric version ordering, drafts/prereleases, wrong products/hosts, missing assets, cache expiry/corruption, HTTP failures, and preserving an active operation/form when updates complete. Native tray tests remain excluded. Logs: `test-output/tests-final-0.4.1.log`, `test-output/update-api-0.4.1.log`.
- Formatting and strict Clippy passed (`test-output/clippy-final-0.4.1.log`). Release compilation passed (`test-output/package-0.4.1.log`). The draft-release helper passed PowerShell syntax validation.
- The debug-only `--preview-updates` fixture displays a fictitious newer version and notes, with no network requests, IPC or saved preferences. English notification, release notes and manual-check controls were visually inspected at 1120 × 780.
- Installer lifecycle validation remains unresolved from 0.4.0; compilation is not an installation test. No automatic in-app installer is implemented. No public stable release is claimed by these checks; drafts are intentionally not offered by the updater. Release notes are displayed verbatim in the publisher's language. Updates require the GUI to be open; older installed versions require one manual upgrade.

### Interfață internațională — 0.4.0

- Engleză implicită și selector persistent pentru English, Deutsch, Español, Português (Portugalia), Français, Italiano și Română. Setarea per cont este separată de baza de date, în `language.txt`; monitorul tray o recitește la refresh. Navigarea, formularele, validările, stările, mesajele cunoscute din istoricul vechi și meniul tray sunt localizate. Numele planurilor, căile, regulile, parolele, valorile protocolului și formatele copiilor nu sunt traduse sau migrate. Diagnosticele brute Windows/Restic/biblioteci rămân în limba componentei respective.
- Catalog embedded, fără rețea, cu 377 mesaje în șapte limbi. Generatorul verifică prezența și numărul parametrilor; testul Rust verifică fiecare mesaj completat cu argumente în fiecare limbă. Teste dedicate pentru setare implicită, persistență, setare necunoscută/coruptă, eroare de scriere, neatingerea unui fișier canar de planuri, nume/Unicode/acolade păstrate, eroare junction cu două căi, programare săptămânală și stări din istoric.
- Suita completă a trecut: 42 teste (36 obișnuite și 6 Restic/agent rulate explicit), incluzând backup/restaurare pentru toate formatele, retenție, excluderi, programare, repornire și compatibilitate cu planuri vechi. Jurnal: `test-output\tests-0.4.0.log`. Testul adăugat ulterior pentru mesajele compuse de retenție și avertizare a trecut împreună cu celelalte teste lib/UI: 19 trecute, 2 native ignored, în `test-output\tests-locales-final-0.4.0.log`. În total, 43 teste distincte trecute. Clippy strict și formatarea au trecut (`test-output\clippy-final-0.4.0.log`). Motorul backup nu a fost modificat.
- Inspecție vizuală prin skill-ul Computer Use, pe `--preview` debug izolat: plan și eroare în engleză și germană, formular de excluderi în germană și franceză, selector cu toate șapte limbile, schimbare de limbă în formular fără pierderea căii. Textele și butoanele inspectate sunt lizibile și încap la 1120 × 780. Preview nu face IPC, nu lansează agent/tray, nu salvează setări și nu execută operații. Fereastra a fost închisă după verificare. Nu s-au folosit sau schimbat planurile reale.
- Instalatorul include cele șapte limbi (mesaje standard Inno și acțiuni proprii traduse), engleză ca alegere inițială și branding în engleză. README este în engleză, ghidul anterior rămâne în README.ro.md. Link de produs/asistență: `https://strajabackup.com/`; editorul și sigla rămân Weblike PRO. Datele vizibile folosesc YYYY-MM-DD și ora locală.
- Limite: traducerile nu au fost revizuite de vorbitori nativi externi; controalele Windows de alegere fișiere urmează limba Windows. Nu a fost verificată vizual fiecare pagină în fiecare limbă sau pe alte configurații DPI. Site-ul nu a fost publicat/modificat. Executabilele și instalatorul nu sunt semnate digital.


- Release 0.4.0 compilat; instalatorul multilingv a fost compilat fără avertismente. Testul automat al instalatorului **nu a putut fi finalizat**: atât /LANG=english, cât și /LANG=romanian au rămas în inițializare înainte de copierea fișierelor. Același comportament a fost reprodus cu un instalator Inno 7.1 minimal, fără cod Straja, fără fișiere și fără dezinstalator, inclusiv /NOSTYLE. Acest rezultat indică o problemă de mediu/runtime în sesiunea curentă, fără a stabili cauza exactă. Procesele de test identificate au fost oprite; instalarea reală și planurile nu au fost modificate. Nu se revendică un ciclu instalare/upgrade/dezinstalare reușit pentru 0.4.0. Jurnale: `test-output\installer-test-0.4.0-en.log`, `test-output\installer-test-0.4.0-ro.log`, `test-output\installer-0686425f68744cc19e512a9dcff48355\install.log`, `test-output\installer-e7e511408be54144bb211c29928af3a3\install.log`, `test-output\setup-diagnostic-run.log`, `test-output\setup-diagnostic-nostyle.log`.
- Este pregătită și o distribuție ZIP a executabilelor release, ghidurilor și licențelor, cu verificare de integritate a fișierelor. ZIP-ul nu execută instalarea și nu înregistrează Add/remove Programs. Publicarea instalatorului trebuie precedată de verificarea sa într-o sesiune Windows funcțională. Executabilele nu sunt semnate digital.

### Acțiuni pentru calea din eroare — 0.3.3

- „Copiază mesajul”, „Copiază calea” și „Adaugă la excluderi…” în notificarea operației, ultima încercare de pe card și Istoric. Recunoaște mesajele existente de junction/reparse point și fișier imposibil de citit; nu presupune că prima cale din mesaj este sursa. Pentru alte erori rămâne copierea mesajului și editarea manuală a planului.
- Butonul deschide un draft editabil, fără scriere imediată în plan. O excludere existentă se editează, fără duplicare; păstrează celelalte excluderi, programarea și retenția. Se aplică validarea normală la salvarea căii. Agentul ocupat/deconectat, un plan șters, o sursă modificată sau un formular deja deschis nu permit suprascrierea draftului. Copierea folosește API-ul clipboard Iced și păstrează mesajul erorii.
- 13 teste relevante trecute: 11 de interfață/stare și 2 de integrare/validare a excluderilor. Testele noi acoperă mesajul laragon cu destinație temporară înaintea sursei, UNC, căi extinse, spații, diacritice, delimitatoare, mesaje necunoscute/incomplete, operații de restaurare/verificare, editare înainte de adăugare, anulare, duplicate, protejarea draftului și asocierea corectă a notificării. Testele de integrare verifică backup și restaurare în cele patru moduri necriptate.
- Clippy cu avertismente refuzate și formatarea au trecut. Jurnale: `test-output\tests-0.3.3.log`, `test-output\clippy-0.3.3.log`. Motorul, protocolul și formatul planurilor nu s-au schimbat; testul Restic marcat ignored nu a fost repetat. Interfața și clipboardul Windows nu au fost inspectate/interacționate vizual în această sesiune. Nu s-au modificat planurile sau copiile utilizatorului.


- Compilarea release 0.3.3 și ciclul izolat instalare/actualizare/dezinstalare au trecut, inclusiv înregistrarea Windows, protecția executabilelor în uz și păstrarea datelor. Jurnale: `test-output\installer-2bc59c6bd05d475d9e5d725fa96b11db`, `test-output\installer-test-0.3.3.log`, `test-output\package-0.3.3.log`.

### Excluderi introduse și editate manual — 0.3.2

- Câmp manual comun pentru adăugare/editare, cu Enter, butoane Adaugă excluderea / Salvează calea / Anulează și Editează pentru fiecare cale existentă. Sunt acceptate căi cu `/`, `\` și ghilimele duble exterioare. Nu se rezolvă junction-urile; calea rămâne literală și poate lipsi temporar de pe disc.
- Validarea existentă se aplică înainte de modificarea listei. Un duplicat sau o cale invalidă păstrează excluderea veche și textul de corectat. Textul nesalvat blochează trecerea la pasul următor; editarea altei căi nu îl aruncă. Eliminarea unei căi actualizează corect indicele editat sau anulează editarea acelei căi.
- 10 teste relevante trecute: 8 de interfață/stare și 2 de integrare/validare a excluderilor, inclusiv backup și restaurare pentru fișiere simple, ZIP, TAR.GZ și 7z. Testele noi acoperă exemplul laragon, ghilimele, separatorii și majusculele diferite, duplicate, cale externă, editare invalidă fără pierderea originalului, anulare, nume cu `[ ]` și `;`, text nesalvat și indici după eliminare. Clippy cu avertismente refuzate și formatarea au trecut.
- Jurnale: `test-output\tests-ui-0.3.2.log`, `test-output\tests-exclusions-0.3.2.log`, `test-output\clippy-0.3.2.log`. Motorul, formatul planurilor și protocolul nu s-au schimbat; regresiile Restic/agent din 0.3.1 nu au fost repetate. Formularul nou nu a fost inspectat vizual în această sesiune. Nu s-au modificat planurile sau copiile utilizatorului.

- Compilarea release 0.3.2 și hash-urile executabilelor au trecut. Instalatorul a trecut ciclul izolat instalare/actualizare/dezinstalare, înregistrarea Windows, opțiunile de login, protecția fișierelor în uz și păstrarea datelor. Jurnale: `test-output\installer-47fd037c9a1f4f92ac5c38707df8a40e`, `test-output\installer-test-0.3.2.log`.

### Fișiere și subfoldere excluse prin selecție — 0.3.1

- Selectoare Windows pentru foldere și fișiere excluse în pasul „Ce salvezi”, listă cu căi complete și „Include din nou”; afișare pe cardul planului și în confirmarea finală. Sunt disponibile atât pentru plan nou, cât și pentru editare. Tiparele avansate existente rămân separate.
- Căile selectate sunt literale și ignoră diferențele de majuscule; excluderea unui folder acoperă descendenții săi. Nu se extinde la foldere cu același nume în alt loc sau cu prefix asemănător. Parantezele drepte sunt escapate pentru filtrul Restic; punctul și virgula nu separă căile selectate. Matcherul copiilor simple/arhivelor pregătește o singură dată lista și verifică strămoșii căii, fără recalcule pentru fiecare excludere și fișier.
- Validare înainte de salvare și backup: cel mult 100 de căi absolute din interiorul surselor, fără `..` sau wildcard. Excluderea unei surse întregi este refuzată. Căile excluse pot lipsi temporar; setarea rămâne activă dacă reapar. Planurile vechi au implicit lista nouă goală, iar regulile lor existente sunt păstrate.
- 34 de teste distincte trecute: 28 obișnuite și 6 teste Restic/agent executate explicit. Testele noi verifică toate cele cinci moduri prin backup și restaurare, surse cu spații/diacritice/`[ ]`/`;`, fișier și subfolder excluse, fișiere excluse blocate, alt folder cu același nume și un vecin cu prefix asemănător, fișiere noi sub folderul exclus, editare pentru reincludere, păstrarea versiunilor și surselor, persistența SQLite și compatibilitatea planurilor vechi. Testele UI verifică selecția, deduplicarea, respingerea unei căi externe, eliminarea excluderii și încărcarea ei la editare.
- Regresiile de compresie, retenție, Restic, programare și agent au trecut. Clippy cu avertismente refuzate și formatarea au trecut. Jurnale: `test-output\tests-0.3.1.log`, `test-output\tests-final-0.3.1.log`, `test-output\clippy-0.3.1.log`. Nu s-au folosit sau modificat planurile și copiile utilizatorului.
- Compilarea release și hash-urile executabilelor au trecut. Instalatorul 0.3.1 a trecut ciclul izolat instalare/actualizare/dezinstalare, înregistrarea Add/remove Programs, scurtăturile, opțiunile de login, refuzul modificării executabilelor în uz și păstrarea datelor. Jurnale: `test-output\installer-f25c124e948541ebac9699a05c5a4192`, `test-output\installer-test-0.3.1.log`.
- Limite: selectoarele și formularul nou nu au fost inspectate vizual în această sesiune; testele UI verifică starea și validarea. Căile UNC sunt acoperite la normalizare, fără test pe NAS real. Formatele de backup și limitările anterioare rămân neschimbate.

### Retenție și arhive — 0.3.0

- Limită opțională per plan, 1–10000 copii inclusiv cea mai nouă; implicit toate, inclusiv la deserializarea planurilor vechi. Editarea salvează limita fără a șterge fișiere. Aceeași regulă se aplică backupurilor manuale și programate.
- Arhive standard necriptate ZIP/Deflate, TAR.GZ și 7z/LZMA2, create în Rust fără proces extern de arhivare. Sursele sunt copiate și verificate SHA-256, arhiva este recitită/decomprimată și verificată înainte de publicare, apoi copia temporară necomprimată este eliminată. Restaurarea acceptă doar intrările din manifestul validat și verifică și fișierele scrise pe disc.
- Curățarea pornește doar după o copie completă, identificată separat de copiile anterioare. Verifică datele înainte de ștergere, păstrează copia nouă chiar dacă ceasul a mers înapoi, refuză conținutul necunoscut și legăturile, ignoră copiile parțiale. Restic folosește ID-uri complete ale planului și `prune`, fără gruparea implicită pe surse/gazdă. Eșecul curățării apare explicit ca „Reușit cu avertisment”, păstrând data backupului salvat.
- 30 de teste trecute: 25 obișnuite și 5 Restic/agent executate explicit. Cele trei arhive au fost citite și prin API-urile standard ale bibliotecilor, separat de funcțiile de restaurare Straja. Acoperire: compresie efectivă, trei rulări cu limită două, restaurare, nume Unicode, surse multiple, fișiere goale, foldere goale, excluderi, data modificării, lipsa Restic în modurile necriptate, copii vechi/politică implicită/editare, fișier blocat fără retenție, corupere, traversare de directoare, metadate străine, fișier suplimentar protejat și ceas dat înapoi.
- Testele Restic au verificat eliminarea versiunilor vechi, restaurarea copiei rămase, păstrarea snapshotului cu alt tag și absența ștergerilor la backup incomplet. Agentul izolat a executat și programarea, repornirea și backupul manual pentru fiecare format comprimat. Nu s-au modificat planurile, agentul sau backupurile utilizatorului.
- Clippy cu avertismente refuzate și formatarea au trecut. Jurnale: `test-output\tests-0.3.0.log`, `test-output\clippy-0.3.0.log`. Noile câmpuri sunt acoperite de teste de stare/validare UI; formularul nu a fost inspectat vizual în această sesiune. Testele native ale meniului, neschimbat, nu au fost repetate.
- Compilarea release 0.3.0 și verificarea hash-urilor executabilelor au trecut. Instalatorul a trecut ciclul izolat instalare/actualizare/dezinstalare, inclusiv licențele arhivării, înregistrarea Add/remove Programs, scurtăturile, opțiunile de login, protecția fișierelor în uz și păstrarea datelor. Jurnale: `test-output\installer-1b1c442f6ec44c0282599cd869c6c37b`, `test-output\installer-test-0.3.0.log`.
- Limite: arhive complete, fără deduplicare; necesită temporar spațiul copiei necomprimate plus arhiva nouă. Fără conversie între formatele planurilor existente. Nu s-au testat arhive >4 GB, întreruperea alimentării, NAS sau funcționarea prin SCM. Licențele bibliotecilor noi sunt incluse în `ARCHIVE-LICENSES.txt`.

### Meniu nativ lizibil — 0.2.4

- Cauză identificată în cod: `TrackPopupMenu` era apelat cu împrumutul mutabil al stării Tray încă activ. Mesajele recursive ale meniului întâlneau un RefCell ocupat și primeau `0`, fără a ajunge la procesarea implicită Windows. Starea este acum eliberată înainte de bucla meniului, iar mesajele netratate sunt trimise către `DefWindowProcW` în afara împrumutului.
- Prevenire a deschiderii recursive a meniului; timerul monitorului rămâne activ în bucla nativă. Închiderea cere `EndMenu` dacă este necesar, iar distrugerea ferestrei încheie bucla de mesaje. Comenzile de pornire/oprire și protecția operațiilor active rămân neschimbate.
- Test nativ nou: meniul se deschide într-o fereastră de test separată, fără executarea comenzilor către agent; verifică mesajul implicit recursiv `WM_GETTEXTLENGTH`, intrarea în `WM_INITMENUPOPUP` cu starea liberă și recepția timerului cât timp meniul rămâne deschis.
- 19 teste automate obișnuite și 2 teste native Windows au trecut; Clippy cu avertismente refuzate și formatarea au trecut. Jurnale: `test-output\tests-0.2.4.log`, `test-output\tray-native-0.2.4.log`, `test-output\clippy-0.2.4.log`. Testele Restic/IPC marcate explicit `ignored` nu au fost repetate pentru această modificare a pictogramei.
- Inspecție vizuală cu skill-ul Computer Use: meniul corectat arată texte negre pentru acțiunile active, texte gri pentru cele indisponibile și separatoare lizibile, pe configurația Windows curentă. Captura a fost făcută asupra meniului izolat de test, nu asupra monitorului instalat. Nu s-au schimbat tema sistemului, agentul instalat sau planurile utilizatorului.
- Compilarea release și hash-urile executabilelor au fost verificate. Instalatorul 0.2.4 a trecut ciclul izolat instalare/actualizare/dezinstalare, înregistrarea Windows și păstrarea datelor. Jurnale: `test-output\installer-10a4c6682461444f97818afce94ecf1f`.

### Ștergerea planurilor — 0.2.3

- Buton „Șterge planul…” cu confirmare pe card, anulare și confirmarea păstrării parolei pentru planurile criptate. Se elimină configurația, programarea și secretul salvat; fișierele sursă, copiile și istoricul rămân intacte. Se curăță selecția de restaurare după confirmarea succesului de către agent.
- Agentul refuză ștergerea în timpul oricărei operații. Store refuză eliminarea unui plan cu operații nefinalizate, într-o tranzacție SQLite. La repornire se recuperează toate operațiile întrerupte, inclusiv cele mai vechi decât cele 100 de intrări afișate în istoric.
- 23 de teste au trecut: cele existente plus teste pentru confirmare/anulare și răspuns de eroare în UI, refuz în timpul backupului/restaurării/verificării, păstrarea istoricului și fișierelor, eliminarea doar a secretului planului ales, alte planuri neafectate, discuri deconectate și recuperarea unui istoric vechi. Testele IPC existente verifică ștergerea după backupuri reale, în ambele moduri, comparând octet cu octet conținutul depozitului înainte și după ștergere.
- Regresiile de backup, restaurare, programare, SQLite și parole au trecut. Clippy cu avertismente refuzate și formatarea au trecut. Jurnale: `test-output\tests-0.2.3.log`, `test-output\clippy-0.2.3.log`. Noul panou de confirmare este acoperit de testele de stare UI, fără inspecție vizuală în această sesiune.
- Compilarea release și hash-urile executabilelor au fost verificate. Instalatorul 0.2.3 a trecut ciclul izolat instalare/actualizare/dezinstalare, înregistrarea Windows și păstrarea datelor. Jurnale: `test-output\installer-13b2d93fb47748e79a3ac2243a0a1526`. Niciun plan real al utilizatorului nu a fost șters sau modificat în teste.

### Logo și site Weblike PRO — 0.2.2

- Logo-ul SVG original din antetul https://weblike.ro/ este inclus în executabil și afișat pe fundal închis. Logo-ul și textul „weblike.ro” din bara laterală și „Despre aplicație” trimit URL-ul HTTPS fix către browserul implicit prin Windows ShellExecuteW, numai la apăsare.
- SVG-ul local a fost verificat pentru absența scripturilor și referințelor externe. Logo-ul rasterizat și bannerul instalatorului au fost inspectate vizual. Interfața rulată și deschiderea efectivă a browserului nu au fost testate în această sesiune.
- `cargo check --all-targets`, Clippy cu avertismente refuzate, formatarea și compilarea release au trecut. Motorul de backup nu a fost modificat; testele sale din 0.2.0 nu au fost repetate.
- Instalatorul a trecut ciclul izolat instalare/actualizare/dezinstalare. Au fost verificate explicit valorile `URLInfoAbout` și `HelpLink` cu adresa https://weblike.ro/, precum și editorul Weblike PRO. Jurnale: `test-output\installer-ceb6de947dc3402ab923af901a8f01bb`.

### Identitatea creatorului — 0.2.1

- Mențiunea „Creată de Weblike PRO — un proiect Open-Source” este inclusă în interfață, instalator și README. Editorul instalatorului și al înregistrării Windows este Weblike PRO. Identitatea AppId și licența MIT sunt păstrate.
- Compilarea release și formatarea au trecut. Testul existent al instalatorului verifică noul editor; ciclul izolat instalare/actualizare/dezinstalare a trecut, inclusiv protecția fișierelor în uz și păstrarea datelor. Jurnale: `test-output\installer-f569be2d5ce844e392e59381ff58d926`.
- Modificarea privește textele și metadatele de distribuție; motorul de backup nu a fost schimbat. Suita de backup din 0.2.0 nu a fost repetată. Textele noi nu au fost inspectate vizual în aplicația rulată.

### Backup necriptat — 0.2.0

- Mod distinct în plan: `Plain`, fără parolă sau secret DPAPI; planurile vechi, fără câmpul `mode`, se citesc ca `Encrypted`. Modificarea unui plan păstrează modul și versiunile existente.
- Copii complete în foldere datate, cu fișiere obișnuite, surse numerotate, foldere goale și data modificării fișierelor. Fiecare fișier copiat este recitit și verificat prin SHA-256. Copiile parțiale nu sunt publicate drept versiuni complete.
- Patru teste de integrare au trecut: citire directă din backup, surse multiple cu același nume și fișier individual, diacritice, excluderi și date interne omise, două versiuni după modificarea/ștergerea sursei, restaurare verificată fără suprascrierea sursei, editare și refuzul mutării depozitului inițializat, fișier blocat și reîncercare, detectarea coruperii, respingerea căilor nesigure și a unui depozit aparținând altui plan, compatibilitatea planurilor vechi.
- Testul agentului izolat a executat programarea necriptată restantă, repornirea fără dublarea rulării și backupul manual prin IPC, fără niciun executabil Restic disponibil.
- Testul formularului verifică lipsa cerinței de parolă/confirmare pentru modul necriptat. Formularul nou nu a fost inspectat vizual în această sesiune.
- 20 de teste trecute în total, incluzând regresiile Restic, agent, SQLite, programări și UI. Testul separat al pictogramei Windows nu a fost repetat pentru această modificare. `cargo clippy --all-targets -- -D warnings` și formatarea au trecut. Jurnale: `test-output\tests-0.2.0.log`, `test-output\clippy-0.2.0.log`.
- Compilarea release 0.2.0 și verificarea hash-urilor executabilelor au trecut. Instalatorul a trecut ciclul izolat instalare/actualizare/dezinstalare, înregistrarea Windows, opțiunile de autentificare, refuzul modificării fișierelor în uz și păstrarea datelor de test. Jurnale: `test-output\installer-58596e09ddf1494c9371ce367fff850d`. Instalația și planurile utilizatorului nu au fost modificate.
- Limite ale modului necriptat: fără VSS, ACL/ADS, deduplicare sau retenție automată; reparse points sunt refuzate explicit. Nu s-au testat un NAS, întreruperea alimentării sau milioane de fișiere. Modificările simultane ale întregului arbore de surse nu constituie un snapshot atomic al sistemului de fișiere.

### Pictogramă Windows — 0.1.2

- Monitor separat `straja-agent.exe --tray`: pictograma continuă după închiderea GUI, fără să dețină operațiile de backup. Instanța este limitată prin mutex per utilizator/sesiune. Serviciul SCM nu lansează UI în sesiunea 0.
- Stările agent oprit, disponibil, ocupat, eroare și agent vechi sunt reprezentate în pictogramă/tooltip. Oprirea din meniu cere confirmare agentului prin IPC și este refuzată pentru serviciu sau operație activă.
- Testele de stare/permisiune de oprire, text UTF-16 și refuzul opririi în backend au trecut. Testul Windows nativ a înregistrat o pictogramă temporară reală, apoi a eliminat-o. Oprirea agentului inactiv prin IPC a fost verificată în testul integrat cu pipe separat.
- Regresiile 0.1.1 și cele trei teste cu Restic au trecut și pentru 0.1.2. Restartul Explorer și autentificarea Windows nu au fost exercitate în această sesiune.
- Instalatorul 0.1.2 a trecut ciclul izolat instalare/actualizare/dezinstalare, verificarea înregistrării Windows, a scurtăturilor și a celor două intrări Run (agent și pictogramă); debifarea opțiunilor elimină intrările. Jurnale: `test-output\installer-0a835b5141cf4cd2940ac95745418db5`. Compilarea release și Clippy au trecut.

### Regresii 0.1.1

- 8 teste de bază/UI trecute: cele 5 inițiale, editare cu păstrarea ID-ului, secretului DPAPI, stării și ultimei reușite; completarea formularului fără expunerea parolei; înlocuirea mesajului de acceptare cu eroarea unei operații terminate rapid.
- 3 teste de integrare cu Restic trecutе: backup/restaurare existent; agent/programare existent; reproducerea unui plan vechi cu destinație nevidă, eroare identificată prin ID-ul operației, corectarea destinației prin IPC și backup manual reușit cu aceeași parolă și același istoric.
- `cargo clippy --all-targets -- -D warnings`: trecut. Formatarea Rust verificată.
- Diagnosticul aplicației instalate a fost numai de citire: planul indica `E:\`, iar istoricul raporta destinație nevidă. Agentul activ provenea din buildul de dezvoltare 0.1.0. Datele și planul utilizatorului nu au fost modificate.
- Instalatorul 0.1.1: compilare fără avertismente; testul izolat de instalare/actualizare/dezinstalare și păstrare fișiere a trecut. Jurnalele sunt în `test-output\installer-883426aa822c4dcb80b313614b4d74ee`.
- Interfața release 0.1.1 a fost lansată pentru inspecție: sursa, excluderile, ultima eroare și avertismentul de agent vechi au fost vizibile. Inspecția completă a formularului în UI nu a fost efectuată; formularul și salvarea au fost acoperite de testele unitare și IPC.

### Verificări inițiale 0.1.0

- `cargo clippy --all-targets -- -D warnings`: trecut, fără avertismente de cod.
- Formatare Rust și sintaxa scripturilor PowerShell: verificate.
- Helperul C# pentru dreptul de serviciu: compilat separat, fără apelarea funcției care modifică drepturile contului.
- 5 teste de bază: programări strict în viitor; tranziții DST București; delimitarea căilor; SQLite/DPAPI și păstrarea programării; respingerea mesajelor IPC supradimensionate.
- Test Restic real: backup inițial și incremental, două versiuni, nume cu spații și diacritice, excluderea `*.tmp` și a bazei/cache-ului aplicației dintr-o sursă-părinte, verificare integrală a depozitului, restaurarea și verificarea versiunii inițiale, păstrarea sursei modificate, respingerea unei parole greșite și a unui snapshot invalid. Un fișier blocat produce `Incomplet`.
- Test agent real: programare ratată executată fără GUI; comunicare Windows named pipe; respingerea unui al doilea agent pe același pipe; oprire și repornire fără dublarea backupului. Testul folosește un pipe unic, separat de aplicația utilizatorului.
- Interfața de dezvoltare a fost lansată și inspectată vizual: ecran principal, stare deconectată și primul pas al formularului. Utilizatorul a pornit agentul și a început un plan de probă; sesiunea a fost păstrată deschisă.

## Instalator Windows

- Inno Setup 7.1.0, instalator x64 complet, compilare fără avertismente. Suma SHA-256 este livrată lângă executabil.
- Test real, izolat prin AppId și nume separat: instalare pentru utilizatorul curent, verificarea fișierelor prin hash, înregistrare Add/remove Programs (nume, versiune, editor, pictogramă, dimensiune și comenzi de dezinstalare), scurtături Start/desktop și agent la login.
- Blocarea accesului de scriere la executabil: actualizarea refuzată cu exit 7 și dezinstalarea refuzată cu exit 1; înregistrarea rămâne intactă.
- Reinstalare peste aceeași versiune: reușită; debifarea opțiunii de login elimină intrarea Run. Fișierele neadministrate de instalator rămân nemodificate.
- Dezinstalare reală: exit 0; intrarea Windows, scurtăturile și executabilele eliminate; fișierele de test ale utilizatorului, din folder și din afara lui, păstrate cu hash identic. Nu au fost folosite backupurile utilizatorului.
- Paginile de bun venit, licență, folder și preferințe au fost inspectate vizual în Windows, în română. Previzualizarea a fost închisă fără instalarea produsului de producție.
- Sintaxa scripturilor PowerShell: verificată. Instalarea administrativă și eliminarea unui serviciu SCM nu au fost exercitate. Pachetul nu este semnat digital.

Comenzi reproductibile: `scripts\build-installer.ps1 -SkipBuild -TestVariant`, apoi `scripts\test-installer.ps1`. Logurile și valorile de înregistrare sunt păstrate în `test-output\installer-*`.

## Limite ale verificării aplicației

Instalarea serviciului în Windows SCM, autentificarea contului la boot și accesul la un NAS nu au fost exercitate. Acestea necesită o mașină/cont de test și credențiale adecvate. Nu s-a făcut un test de durată, un audit de securitate independent sau validare a tuturor combinațiilor de ACL și volume Windows.

Rularea finală a testelor și pachetul release folosesc `target\verification`, deoarece executabilele din `target\debug` erau deschise de utilizator. Distribuția pentru utilizare este în `dist\StrajaBackup`.
