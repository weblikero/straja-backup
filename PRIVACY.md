# Straja Backup desktop application privacy policy

Last reviewed: 2026-10-20. Describes the published 3.0.5 desktop application.
This document describes software behavior; it does not cover browsing
strajabackup.com or weblike.ro, website analytics, or contact forms.

Straja Backup is an open-source application maintained by Weblike PRO.
It does not require a Straja account. The reviewed application code has no
usage-analytics or automatic crash-report upload feature. Backup files and
credentials are not sent to Weblike PRO by the desktop application.

## Information stored on your computer

Straja stores plan names, source and destination paths, exclusions, schedules,
run history, diagnostics, interface preferences and update-cache information
locally. Plan metadata and history are not encrypted as a whole. Diagnostics
can contain paths, filenames and details returned by backup tools.

Saved backup passwords and provider credentials are protected with Windows
DPAPI on Windows. Ubuntu and macOS use authenticated encryption with a wrapping
key held by the user's Secret Service or login Keychain. These mechanisms do
not protect against every process running as the same user or an administrator.
Credentials must be available to the backup processes while operations run.

Unencrypted file copies and ZIP, TAR.GZ and 7z archives can be read by anyone
with access to the destination. Encrypted plans use Restic repositories.
Retain your repository password separately for recovery.

## Automatic update checks

While the desktop interface is open, it checks the public GitHub release API
at startup and approximately every 24 hours, with a local cache. A manual check
is also available. Requests go to:

`https://api.github.com/repos/weblikero/straja-backup/releases/latest`

They identify the application version through `StrajaBackup/<version>` in the
User-Agent. GitHub also receives normal connection information, such as the
public IP address. The update request does not upload plans, backup content,
paths, credentials or run history.

Windows installer downloads, initiated by the user, contact GitHub and its
release asset hosts. Native editions open the release page for manual upgrades.
The agent and tray do not independently check for updates.

Version 3.0.5 has no preference to disable automatic checks in the interface
and no installer opt-out. Do not describe this version as making network
requests only after explicit user action.

GitHub handles requests under its
[privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Backup destinations and account connections

Local backups write to the folders you select. A selected network share involves
the server hosting that share. Online plans send encrypted repository data to
the FTP/FTPS server, Google Drive, Dropbox or Amazon S3 destination configured
by you. Connection tests, backups, restores, verification and retention contact
that destination. Scheduled operations can run without the interface open.

Restic and Rclone perform the configured transfers. Providers see account
access, connection details, repository object sizes and transfer activity.
Google Drive and Dropbox authorization opens the provider's consent flow
through Rclone; the resulting tokens are stored as protected credentials.
FTP and S3 credentials are supplied by the user. Plain FTP does not encrypt
the transport, including login credentials; explicit FTPS is the default.

Provider policies:

- [Google](https://policies.google.com/privacy)
- [Dropbox](https://www.dropbox.com/privacy)
- [Amazon Web Services](https://aws.amazon.com/privacy/)
- For FTP/FTPS and network shares, consult the operator you selected.

See [REMOTE.md](REMOTE.md) for connection behavior, credential handling and
limitations.

## External links, support and deletion

Opening a product, creator or GitHub link launches a browser; the destination's
own policy then applies. Nothing automatically attaches your logs to an issue.
If you choose to share diagnostics, remove private paths, tokens and personal
data first. GitHub issues are public.

Deleting a plan removes its schedule and saved credentials, but preserves
history and backup data. Uninstalling preserves application data and backups
for recovery. Retention removes eligible old backups after a successful run;
it is not a request to erase all stored data. Revoking cloud authorization is
done through the relevant provider.

For public questions about application behavior, use the
[project issue tracker](https://github.com/weblikero/straja-backup/issues).
Do not post confidential information there.
