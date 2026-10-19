# Instalator Straja Backup

`straja.iss` este sursa instalatorului Inno Setup. AppId-ul de producție trebuie păstrat la actualizări. Definirea `InstallerTest` produce o identitate separată, folosită exclusiv de testul de instalare.

`Romanian.isl` provine din https://github.com/jrsoftware/issrc/blob/is-7_1_0/Files/Languages/Unofficial/Romanian.isl (traducător: Alexandru Bogdan Munteanu). Fișierul a fost convertit din Windows-1250 în UTF-8 și au fost eliminate trei mesaje depreciate. Mesajele noi din Inno Setup 7 și textele specifice Straja sunt în `straja.iss`.

Fișierele din `assets` sunt generate din `scripts/installer-assets.ps1`. Compilarea și testarea sunt descrise în README-ul proiectului.
