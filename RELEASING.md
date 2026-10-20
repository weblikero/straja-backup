# Publishing Straja Backup

Source repository and update channel: https://github.com/weblikero/straja-backup.
The product portal is https://strajabackup.com/. Other applications on that site must use their own repositories and update channels.

## What installed clients do

Starting with 0.4.1, the desktop interface checks the public GitHub Releases API at startup and every 24 hours while open. A successful result is cached per Windows account for 24 hours. About includes a manual check, with a one-minute cooldown. No GitHub account or token is needed on users' PCs. Requests contain the application version in the User-Agent, not backup paths, plans, passwords or history. GitHub receives the normal network request, including the public IP address.

Only the repository's latest published stable release is considered. Drafts, prereleases, older versions and releases without a correctly named Windows x64 asset are ignored or reported as invalid. Versions are compared numerically using SemVer. Successful checks with no published stable release show that no newer stable release is available. Offline/API errors appear in About without affecting backups.

From 0.5.0, a notification offers **Download update**, then **Close and install** with a confirmation or an option to wait for the current backup/restore/verification. The GUI streams the installer to a unique per-user update folder, checks its size and GitHub SHA-256 digest, and hands off to a copy of itself outside the installation directory. The helper revalidates the published asset, waits for the GUI to exit, requests a token-protected idle-agent/service shutdown, closes the matching tray, and waits for executables to be released. The installer keeps the existing path, per-user/all-users scope, language and previously selected tasks. No forced termination or automatic reboot is requested. Straja reopens after completion; UAC approval may be required. A failed handoff keeps the GUI open; later failures are reported and reopening is attempted.

In-app installation requires `StrajaBackup-X.Y.Z-windows-x64-setup.exe`, an uploaded asset of 1–256 MiB, and a GitHub `digest` of `sha256:<64 hex digits>`. The digest is obtained from the public release API over HTTPS, not from an adjacent checksum file or release notes. Redirects are restricted to GitHub and its release asset hosts. A portable-only release can be discovered but cannot be installed automatically. SHA-256 validates integrity; the binaries are not yet code-signed.

The tray and service do not independently check for updates; open the interface to see notifications. Installed versions up to 0.4.2 need one manual upgrade to gain in-app installation. Release notes appear in About as plain text in the publisher's language. Automatic checks use the 24-hour cache, so publication is not an instant push notification; a manual check bypasses that cache. Validate installer/runtime recovery and service/UAC flows on disposable Windows installations and disclose untested paths in release notes.

## Prepare a version

1. Update the package version in `Cargo.toml`, regenerate `Cargo.lock`, and update the guides and installer test default. Keep GUI and agent versions identical.
2. Run `scripts/test.ps1`, `scripts/test-remote.ps1`, Clippy and formatting checks. Build with `scripts/package.ps1`. Test the actual installer lifecycle on a disposable Windows account before distributing an installer. See `REMOTE.md` for the isolated remote test prerequisites and real-account validation limitations.
3. Package the entire `dist/StrajaBackup` folder as `dist/StrajaBackup-X.Y.Z-windows-x64-portable.zip`. Include notices and dependency licenses. Generate a SHA-256 sidecar using the same filename plus `.sha256`.
4. Build `dist/installer/StrajaBackup-X.Y.Z-windows-x64-setup.exe` and its SHA-256 sidecar using `scripts/build-installer.ps1 -SkipBuild`. Do not upload the test installer.
5. Commit the reviewed sources, tag the commit `vX.Y.Z`, and push the branch and tag. Exclude local toolchains, build output, logs, databases, keys and personal data.
6. Write release notes describing changes, validation and limitations, then run:

```powershell
.\scripts\prepare-release.ps1 -NotesFile .\release-notes.txt
# Only for an explicitly requested review build: add -DraftOnly.
# Only for a deliberately portable-only preview: add -PortableOnly (no in-app installation).
```

By default this uploads a temporary draft, verifies its installer digest, then calls `scripts/publish-release.ps1` to publish **Stable / Latest** and verify the anonymous `/releases/latest` endpoint used by installed clients. It checks all four uploaded assets against local hashes and sizes, validates final download URLs, and refuses to make an older version Latest. A failed check is not a completed launch. `-DraftOnly` and `-PortableOnly` leave an explicitly requested draft invisible to installed clients. Never replace assets under an existing published tag; issue a new version.

To finish an already uploaded draft (or verify an already published release without replacing it), use `scripts/publish-release.ps1 -Version X.Y.Z`. Keep the matching local artifacts and checksum sidecars available. If public verification fails after publication, inspect the existing release and retry verification; do not recreate or overwrite its assets.

The project owner requested on 2026-10-19 that launching a new version includes GitHub publication and automatic in-app discovery. Do not describe a local installer or draft as a launched update. After publishing, run the ignored `updates::tests::live_public_repository_and_release_api` test with `STRAJA_UPDATE_TEST_INSTALLED` set to an older installed version to exercise the actual Rust discovery logic without changing user settings.

The draft helper validates tag/HEAD, a clean working tree and asset hashes. Build provenance, functional installer validation and signing remain release responsibilities; checksums alone do not prove publisher identity. Code signing is not configured yet.

## Website links

The Straja product page can link to https://github.com/weblikero/straja-backup/releases/latest. This stays valid across version changes. Separate product pages for other apps should point to those apps' own release URLs. No website credentials or changes are required for Straja's GitHub-based check.

GitHub API reference: https://docs.github.com/en/rest/releases/releases#get-the-latest-release.

## Current validation status

Version 0.7.1 passed 80 regression tests, the three isolated online tests, strict Clippy, formatting and the isolated installer lifecycle. The exclusion-removal and first-step save changes were tested automatically; the native UI was not visually exercised. Earlier cloud-account and full update/UAC/SCM limitations remain as described below.

Version 0.7.0 passed 79 regression tests, isolated FTP and HTTPS S3-compatible roundtrips, strict Clippy and the isolated installer lifecycle including the new Rclone file lock. Real Google Drive/Dropbox consent and refresh, external FTPS and Amazon account permissions/quotas still need account-specific validation. The native form could not be visually inspected in the available desktop session. See `REMOTE.md` and `VALIDATION.md`.

The 0.6.2 isolated installer lifecycle and 74 Rust tests passed. Installation, upgrade, uninstall, registration, locked-file protection, updater installer arguments and data preservation were exercised. The full GitHub download/helper/restart path, all-users elevation and actual installed SCM service restart have not yet been exercised end to end. After these limitations were explained, the project owner directed stable publication and automatic discovery on 2026-10-19; the earlier draft-only publication hold is superseded. Keep the limitations visible and complete those disposable-machine checks in future validation. See `VALIDATION.md`.

## Native Linux and macOS release assets (0.8.0+)

Run `.github/workflows/native.yml` on the exact release commit. Download all three
successful runner artifacts into `dist/native` (flatten their contents). Verify the
workflow head SHA, the version in each filename, and all SHA-256 sidecars before
publishing. Native compilation, regression tests, package installation and GUI startup
smoke tests must pass. The scripts now require all six native packages and their
sidecars, in addition to the Windows installer/portable ZIP, before stable publication.
Do not publish a Windows-only Latest release that breaks native update discovery.

Native asset names: `StrajaBackup-X.Y.Z-ubuntu-x64.deb`, `-linux-x64.tar.gz`,
`-macos-arm64.dmg`, `-macos-arm64.tar.gz`, `-macos-x64.dmg`, `-macos-x64.tar.gz`.
The matching OS/architecture is selected by native clients. Initial native clients
notify and link to the release page; Windows automatic installer handoff is unchanged.
Mac packages are ad-hoc signed only until Developer ID and notarization are configured.
See NATIVE.md for keychain, snapshots, autostart and manual validation limitations.
