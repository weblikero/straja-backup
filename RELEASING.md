# Publishing Straja Backup

Source repository and update channel: https://github.com/weblikero/straja-backup.
The product portal is https://strajabackup.com/. Other applications on that site must use their own repositories and update channels.

## What installed clients do

Starting with 0.4.1, the desktop interface checks the public GitHub Releases API at startup and every 24 hours while open. A successful result is cached per Windows account for 24 hours. About includes a manual check, with a one-minute cooldown. No GitHub account or token is needed on users' PCs. Requests contain the application version in the User-Agent, not backup paths, plans, passwords or history. GitHub receives the normal network request, including the public IP address.

Only the repository's latest published stable release is considered. Drafts, prereleases, older versions and releases without a correctly named Windows x64 asset are ignored or reported as invalid. Versions are compared numerically using SemVer. Successful checks with no published stable release show that no newer stable release is available. Offline/API errors appear in About without affecting backups.

A notification offers **View release and download**, opening the release page in the default browser. Release notes appear in About as plain text, in the publisher's language. No installer is downloaded or run by the application. The tray and Windows service do not independently check updates; open the interface to see notifications. Older installed versions (0.4.0 and earlier) need one manual upgrade to gain this feature.

## Prepare a version

1. Update the package version in `Cargo.toml`, regenerate `Cargo.lock`, and update the guides and installer test default. Keep GUI and agent versions identical.
2. Run `scripts/test.ps1`, Clippy and formatting checks. Build with `scripts/package.ps1`. Test the actual installer lifecycle on a disposable Windows account before distributing an installer.
3. Package the entire `dist/StrajaBackup` folder as `dist/StrajaBackup-X.Y.Z-windows-x64-portable.zip`. Include notices and dependency licenses. Generate a SHA-256 sidecar using the same filename plus `.sha256`.
4. Optionally build `dist/installer/StrajaBackup-X.Y.Z-windows-x64-setup.exe` and its SHA-256 sidecar using `scripts/build-installer.ps1 -SkipBuild`. Do not upload the test installer.
5. Commit the reviewed sources, tag the commit `vX.Y.Z`, and push the branch and tag. Exclude local toolchains, build output, logs, databases, keys and personal data.
6. Write release notes describing changes, validation and limitations, then run:

```powershell
.\scripts\prepare-release.ps1 -NotesFile .\release-notes.txt
# Add -IncludeInstaller only after validating the installer.
```

This creates a **draft** with checksum-verified assets. Review the draft and publish it as a stable release, marking it **Latest** in GitHub. Publishing, rather than merely uploading a file to the website, makes it discoverable by installed clients. Use a prerelease for builds that should not be offered through the stable channel. Never replace assets under an existing published tag; issue a new version.

The draft helper validates tag/HEAD, a clean working tree and asset hashes. Build provenance, functional installer validation and signing remain release responsibilities; checksums alone do not prove publisher identity. Code signing is not configured yet.

## Website links

The Straja product page can link to https://github.com/weblikero/straja-backup/releases/latest. This stays valid across version changes. Separate product pages for other apps should point to those apps' own release URLs. No website credentials or changes are required for Straja's GitHub-based check.

GitHub API reference: https://docs.github.com/en/rest/releases/releases#get-the-latest-release.

## Current release limitation

The 0.4.0 installer lifecycle stalled locally, including a minimal Inno installer unrelated to Straja. The cause is unresolved. Do not treat installer compilation as a successful install/upgrade/uninstall test for 0.4.1. Keep releases as drafts until the intended distribution artifacts have been reviewed and tested. See `VALIDATION.md`.
