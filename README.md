# Straja Backup

Open-source backup and restore for Windows, created by [Weblike PRO](https://weblike.ro/). Licensed under MIT. Product website: [strajabackup.com](https://strajabackup.com/).

Version **0.5.3** is a preview for Windows x64 (Windows 10 version 1809 or later, including Windows 11). The desktop app and background agent are written in Rust. Encrypted backups use the bundled Restic 0.19.1. Ordinary files and ZIP, TAR.GZ and 7z archives do not require Restic or a separate archive utility.

My plans uses compact cards with source/destination, frequency, latest status and next run. Exclusions are summarized as counts; **Details & actions** opens the full paths, exclusions, backup format, retention, diagnostics and secondary actions. Only one plan expands at a time. Details remain readable while a backup is running.

The title bar displays the Straja icon and the running executable version, for example **Straja Backup — v0.5.3**.

## Install or update

Run `StrajaBackup-0.5.3-windows-x64-setup.exe`. All runtime components are included; installation needs no downloads. The installer offers English, German, Spanish, Portuguese (Portugal), French, Italian and Romanian. English is the initial default; a previous installer language may be remembered.

Before updating, wait for backup/restore/verification operations to finish. Choose **Stop agent and tray icon** in the notification icon menu, then close the app. If using the Windows service, stop the service and close the tray icon separately. The installer refuses to overwrite running executables.

The default installation is per-user in `%LOCALAPPDATA%\Programs\StrajaBackup`. Straja appears in **Settings > Apps > Installed apps / Add or remove programs**. Optional installer tasks create a desktop shortcut or start the agent and notification icon at sign-in. Installation with `/ALLUSERS` targets Program Files; service installation is separate.

Uninstalling removes application files and shortcuts, while keeping plans, protected passwords and backup data. A service is removed only when it belongs to that installation. The application and installer are not digitally signed yet.

## Portable ZIP and release validation

A portable package, `StrajaBackup-0.5.3-windows-x64-portable.zip`, is also provided. Extract the whole archive to a writable folder and run `StrajaBackup\straja.exe`; keep the agent and Restic next to it. Close older instances first. It uses the same per-user plan database as the installed app and does not register a new Add/remove Programs entry.

For 0.5.0, 64 application tests passed, plus strict static checks. The isolated per-user installer passed installation, upgrade, uninstall, Windows registration, file-in-use rejection and data preservation. The invocation used by in-app updates also passed with the installer held under a read-sharing lock, retaining the existing folder and startup tasks. These checks do not constitute a complete published-release-to-restarted-app test. The live GitHub handoff, all-users UAC and actual Windows service restart still require end-to-end validation on disposable installations. See `VALIDATION.md`.

## Resource use

Version 0.5.2 enables GPU rendering with vsync for scrolling and interaction, with software rendering as a fallback when GPU initialization fails. Static translations are indexed and the logo handle is reused. Rendering occurs on demand; idle windows do not run the synthetic scroll probe. GPU rendering can use more graphics memory than software rendering.

Version 0.4.2 reduces idle polling, uses compact tray status and bounded worker pools, and gives background work lower Windows resource priority. Restic uses reduced parallelism and a soft memory target. ZIP, TAR.GZ and 7z favor faster, lighter compression; archives may be larger. Integrity checks remain enabled. See [PERFORMANCE.md](PERFORMANCE.md) for measurements, tradeoffs and limits.

## Update notifications

The interface checks [GitHub Releases](https://github.com/weblikero/straja-backup/releases) at startup and every 24 hours, using a per-user cache. About offers **Check for updates**. Version 0.5.0 adds **Download update**, followed by **Close and install** and explicit confirmation. No browser download is needed. The installer is streamed to disk with a progress indicator, checked against GitHub's SHA-256 digest and size, and rechecked immediately before use. Cancelling a download keeps the application open.

If a backup, restore or verification is running, choose **Install after the current operation**. Straja waits, temporarily blocks new operations, closes its own interface and tray, asks its idle agent/service to exit, and runs the installer in the same folder and installation scope. It never forcibly terminates an operation. The interface and previously running agent/service reopen afterward, unless Windows requires a restart. Windows may ask for UAC approval. Plans, passwords and backups are kept. Save or close any plan form before updating. **Later** postpones installation; a failed handoff leaves the interface open. Installer failure is reported and Straja attempts to reopen.

Automatic installation requires a correctly named Windows x64 setup asset with a valid GitHub SHA-256 digest in a published stable release. No GitHub account is needed, and backup data is not sent. The interface must be open; the service and tray do not independently check for releases. Existing versions up to 0.4.2 need one manual upgrade to gain in-app installation. Updating a portable copy through this flow installs/registers Straja in its current folder. See [RELEASING.md](RELEASING.md) for publishing instructions and validation limits.

## Choose your language

Use **Language** in the sidebar to switch instantly between **English, Deutsch, Español, Português, Français, Italiano and Română**. English is the default for new users and upgrades without a saved app-language preference. Installer language and app language are separate choices.

The preference is saved per Windows account in `%LOCALAPPDATA%\StrajaBackup\language.txt`. The tray companion picks it up on its next refresh. No account, network connection or online translation service is required. Changing language preserves an open form, existing plan names, paths, exclusions, passwords and backup history. Dates use `YYYY-MM-DD` and the PC's local time.

Navigation, plan setup, validation, backup statuses, known historical errors, restore, exclusion actions and the tray menu are localized. Raw diagnostics supplied by Windows, Restic or other libraries are kept as supplied by those components. User-provided text and paths are never translated. The installer branding image uses English; the installer controls use the selected language.

## Create a backup plan

1. Start the agent from the app, then choose **New plan**.
2. Name the plan and add files or whole folders. Folders include descendants and newly added files.
3. Optionally exclude individual files/subfolders. Choose them in a picker or enter a full path manually. `/` and `\` are accepted. Each exclusion has **Edit** and **Include again** actions. Save the entered path before continuing.
4. Choose the backup mode and a dedicated empty destination folder, on a local disk or accessible network share.
5. Choose manual, interval (at least 15 minutes), daily or weekly runs. Set **Keep all backups**, or a total from 1 to 10000 including the newest backup.
6. Review and save the plan. Use **Back up now** to run it immediately.

The app can close while the agent continues working. Missed schedules run when the agent returns. Failed schedules retry after 15 minutes. A Windows service can run at system startup under the same Windows account; use `scripts\install-service.ps1` from an elevated PowerShell after reading its parameters. Use `scripts\uninstall-service.ps1` to remove it. Keep access to the destination under that account in mind, especially for network locations.

## Backup modes and retention

- **Encrypted (Restic):** password-protected, incremental repositories with deduplication. Keep the repository password separately; you need it to recover on another PC. Windows DPAPI protects the locally saved password for your account.
- **Unencrypted files (copy/paste):** a full dated copy per run. Open and copy files directly in Explorer without Straja or a password.
- **ZIP, TAR.GZ or 7z (unencrypted):** a complete compressed archive per run. Open/extract it with compatible software without Straja or a password. ZIP uses Deflate; 7z uses LZMA2.

Unencrypted copies are verified with SHA-256. Archives are read back and verified before publication. Compression temporarily requires space for both the complete uncompressed copy and its archive, in addition to retained backups.

Retention removes old copies only after a successful, verified new backup. A retention failure reports **Successful with warning** and keeps the new backup. Failed/incomplete backups do not trigger cleanup. Unknown files and incomplete versions are not silently deleted. Changing retention applies to the next backup, not immediately. Backup format cannot be changed in an existing plan; create a new plan and destination.

## Exclusions and failed backups

Exact excluded paths must be inside a source and cannot remove an entire source. Up to 100 are allowed. Matching ignores case and slash style, keeps the literal path (without resolving junctions), and covers descendants of excluded folders. A path may temporarily be absent. Advanced rules such as `*.tmp; node_modules; target` remain available separately.

Unencrypted backup modes reject links, junctions and reparse points rather than following them. For these errors, and known file-read errors, use **Copy path** or **Add to exclusions…** on the plan, completion message or History. The latter opens an editable draft; save the exclusion and then the plan to apply it. **Copy message** is available for other errors. Historical errors from previous versions are supported too.

If you exclude a junction, include its real target separately when you need its contents. An `.in-progress-...` folder is an incomplete copy, not a successful backup. Previous completed copies stay intact. Locked source files are reported; this version does not use VSS.

## Restore, verify and delete plans

Choose **Restore**, select a version, and choose an existing parent folder. Straja creates a new subfolder and verifies restored files. It never overwrites your existing originals. Restore currently recovers the entire selected version. Use **Verify** to check stored data.

**Delete plan…** removes the plan, schedule and saved password only. Files, backups and history stay on disk. Encrypted plans require confirmation that you retained the repository password. Deleted plans no longer appear in Restore; importing deleted plans is not available. Unencrypted copies remain accessible in Explorer, and encrypted repositories can be opened with Restic and their password.

## Build and validate

Use the scripts from this repository on Windows:

```powershell
.\scripts\setup.ps1
.\scripts\test.ps1
.\scripts\package.ps1
.\scripts\build-installer.ps1 -SkipBuild
```

`setup.ps1` prepares the local Rust toolchain. The scripts fetch and verify bundled tools as needed. Release output is in `dist\StrajaBackup`; the installer and SHA-256 file are in `dist\installer`. Dependencies and their licenses are documented in `THIRD-PARTY-NOTICES.txt` and `ARCHIVE-LICENSES.txt`. See `VALIDATION.md` for test evidence and remaining limits.

Translations are embedded from `locales\catalog.json`, generated with Python 3 using `scripts\build-locales.py` from the UTF-8 `locales\messages.txt` table. Its first column retains legacy Romanian message templates so stored histories remain readable; the next columns are English, German, Spanish, Portuguese, French and Italian. Translation placeholders are positional (`{0}`, `{1}`, etc.) and validated by the generator and Rust tests. New translations do not change persisted statuses, JSON schemas, plan identities or file formats.

A debug build supports `straja.exe --preview` (or `--preview-updates --preview-update-confirmation` for the update confirmation) for visual review with fictitious data. It does not connect to an agent, save preferences, open file pickers or run backups. These options are excluded from ordinary release builds. For realistic visual/performance checks, build the isolated fixture with `scripts\cargo.ps1 build --release --features ui-preview --bin straja`; ordinary unoptimized debug rendering is not representative of the installer.

## Scope

This preview supports local and accessible network folders. It does not yet provide cloud-provider integrations, VSS, backup of NTFS ACLs/alternate data streams, automatic junction traversal, format conversion, or selective restore through the app. Use the original source files for production work, and periodically test recovery from your backups.

The earlier Romanian guide is available as `README.ro.md`.
