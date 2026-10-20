# SignPath application preparation

Prepared 2026-10-20. Not submitted. This file is a working application packet,
not a statement that every admission requirement has already been satisfied.

## Project information

| Field | Value |
| --- | --- |
| Project | Straja Backup |
| Maintainer | Weblike PRO / GitHub account `weblikero` |
| Repository | https://github.com/weblikero/straja-backup |
| Website | https://strajabackup.com/ |
| Downloads | https://github.com/weblikero/straja-backup/releases |
| License | MIT; bundled components have their own notices |
| Requested scope | Windows GUI, agent and setup; not Apple signing |
| Contact email | Owner must supply; do not infer from Git history |
| Account MFA | Owner confirmation required; not verified |

## Project description for the application

Straja Backup is an actively developed open-source desktop backup application
written in Rust, maintained by Weblike PRO. It provides manual and scheduled
file/folder backups, exclusions, retention, verification and restore. It supports
Windows, Ubuntu and macOS, with seven interface languages.

Windows downloads include an installer and a portable ZIP. Restic and Rclone
are bundled with upstream notices for encrypted backups and configured online
destinations. Local backups can also use ordinary files or ZIP, TAR.GZ and 7z
archives. The Windows application checks GitHub Releases for new versions and
offers a user-confirmed upgrade.

We are seeking free signing for the Windows executables and installer. The
project is new; we do not claim an established user base or a security audit.
Current downloads are unsigned. We are preparing the verified build and
approval process required for onboarding.

Public documents:

- [Code signing policy](CODE_SIGNING.md)
- [Desktop privacy policy](PRIVACY.md)
- [Validation evidence and limitations](VALIDATION.md)
- [Release process](RELEASING.md)
- [Component notices](THIRD-PARTY-NOTICES.txt)

## Readiness and remaining work

1. **Owner details:** confirm the contact email, MFA and signing approver.
   Do not put credentials, recovery codes or signing tokens in this repository.
2. **Windows CI provenance:** current Windows kits were built locally. Only
   Linux/macOS have a checked-in GitHub Actions workflow. Establish and validate
   a Windows CI build plus SignPath's accepted origin-verification integration
   before requesting production signing. A locally uploaded ZIP is not a
   substitute for verified build provenance.
3. **Executable metadata:** `build.rs` currently compiles the VSS helper but does
   not add Windows VERSIONINFO resources. Add and verify product/version metadata
   for both Straja executables before configuring signing restrictions.
4. **Privacy controls:** 3.0.5 checks GitHub automatically and has no opt-out.
   Add an installation-time disclosure and an effective update-check opt-out,
   including the portable app, before claiming the provider's privacy condition
   is met. Preserve user-triggered checks. Test that the disabled state prevents
   startup and periodic requests.
5. **Distribution notices:** publish links to the signing and privacy policies
   on the product download page and future release notes. Include the policies
   in future packages. The website and existing installers have not been changed
   by this documentation preparation.
6. **Component review:** reconcile the Rust dependency inventory, Restic and
   Rclone with shipped notices and the provider's licensing rules. Do not
   request individual signatures for upstream executables as our own code.
7. **Provider acceptance:** apply at https://signpath.org/apply. Admission depends
   on their review, including project reputation; do not promise acceptance
   or an approval date.
8. **After acceptance:** configure the assigned project/policy and protected
   signing credentials, verify signed binaries and installer/uninstaller,
   test install/upgrade/uninstall, then publish a new version with final checksums.
   Production signing must have a maintainer approval step.

## Submission boundary

The prepared project description may be copied into the official application
after the owner supplies the missing contact details and reviews outstanding
items. Do not send an application that states unfinished work is complete.
No SignPath account, subscription or signature has been created by these changes.

Provider references: [application](https://signpath.org/apply),
[conditions](https://signpath.org/terms).
