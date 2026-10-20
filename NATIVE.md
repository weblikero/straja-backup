# Straja Backup for Ubuntu and macOS

Native desktop interface and a per-user background agent. These are file backups,
not bootable system images. All seven interface languages are included.

## Ubuntu 22.04 or newer (x86-64)

Install the `.deb` with the Software installer or:

    sudo apt install ./StrajaBackup-0.8.0-ubuntu-x64.deb

Open **Straja Backup** from Applications. Click **Start agent** if disconnected.
The portable `.tar.gz` contains the same binaries; extract it before running `straja`.
A graphical session, X11/Wayland libraries and a Secret Service such as GNOME Keyring
are required. Unlock the login keyring to save/access encrypted repository passwords
or cloud credentials. There is no plaintext credential fallback.

In **About**, choose **Start agent automatically at login**, or run as your normal user:

    /opt/straja-backup/enable-autostart.sh

Disable autostart with `systemctl --user disable straja-backup.service`.
Wait for active operations to finish before stopping the agent or upgrading.
Uninstall with `sudo apt remove straja-backup`; user data and backups are retained.
Disable the user service before uninstalling.

## macOS 14 or newer (Apple Silicon / Intel)

Choose `macos-arm64` for Apple Silicon or `macos-x64` for Intel.
Open the DMG, drag **Straja Backup.app** into **Applications**, eject the image,
then launch the installed copy. Do not run the agent from the mounted DMG.
The package has an ad-hoc signature, not an Apple Developer ID signature, and is
not notarized. Gatekeeper may block first launch. Only approve a package whose
origin and SHA-256 you have verified, using macOS Privacy & Security.
Do not disable Gatekeeper globally.

Passwords use the login Keychain. macOS may ask you to authorize access. Protected
folders may require Files & Folders or Full Disk Access permission for the app/agent
in System Settings. Grant only the access needed for your selected sources.

Optional startup at login: use **About → Start agent automatically at login**, or run after installation:

    "/Applications/Straja Backup.app/Contents/MacOS/enable-autostart.sh"

This creates a LaunchAgent for the current user. To disable it:

    launchctl bootout "gui/$(id -u)" "$HOME/Library/LaunchAgents/com.weblike.straja-backup.plist"

Remove that plist to disable future logins. Wait for backups to finish before
stopping the agent, upgrading or moving the application to Trash.
User data and backups are not removed.

## Platform differences

- Agent continues after closing the window; scheduling requires the agent and an
  unlocked keychain. Default autostart is per login, not a root/system service.
- Windows tray integration, VSS and the Windows automatic installer are not used.
  Native releases are detected for the current OS/CPU; these first native kits
  link to the release page for installation. Windows keeps its in-app upgrade flow.
- Files changing while read can fail the backup. No filesystem snapshot guarantee
  on these platforms; use database-native exports for live databases.
- Plain/archive modes reject symbolic links, special files and unsupported names;
  exclude them or use encrypted Restic mode, which preserves symbolic links.
- Plain/archive copies are intended for documents; they are not an ACL/xattr,
  resource-fork or ownership-preserving system backup. Use Restic when metadata
  preservation matters and validate restore on your target filesystem.
- Unix paths and exclusions are case-sensitive. Keep destinations outside sources.
- Credentials are tied to the login keychain. Keep repository passwords separately
  so a lost computer/keychain does not prevent recovery.

Data: Ubuntu `~/.local/share/straja-backup` (or `$XDG_DATA_HOME/straja-backup`);
macOS `~/Library/Application Support/StrajaBackup`.
Do not synchronize a live database between operating systems.

## Build and validation

`.github/workflows/native.yml` builds on real Ubuntu, macOS Apple Silicon and Intel
runners. It runs native regression tests, Restic backup/restore and strict Clippy,
then packages checksum-verified Restic/Rclone with the app and license notices.
`scripts/test-unix.sh` is for disposable CI accounts only (it starts a test keyring).
On Mac CI each unsigned test executable uses its own ephemeral Keychain item,
with normal ACLs. Release builds always use the stable production account.
User consent prompts and access after upgrading a signed application still
need interactive validation.
`python3 scripts/package-unix.py` builds the package on its target OS after
`cargo build --locked --release`; `--tools-only` downloads the backup engines.

Signing/notarization and interactive usability on physical Mac/Ubuntu machines
remain separate release checks; a successful CI build does not certify them.

Developer references: [Apple distribution and notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution),
[keyring native stores](https://docs.rs/keyring/3.6.3/keyring/),
[GitHub hosted runner platforms](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
