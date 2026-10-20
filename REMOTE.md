# Online backup destinations

Straja 0.7.0 adds FTP/explicit FTPS, Google Drive, Dropbox and Amazon S3 to the destination step of a backup plan. Local/network folders retain all existing formats. Online plans currently use encrypted, incremental Restic repositories through the bundled Rclone 1.75.2 binary. Plain files and ZIP/TAR.GZ/7z archives remain available for local/network destinations.

## Create a plan

1. Select files/folders and exclusions as usual.
2. Choose the destination provider. For FTP, enter hostname (without `ftp://`), port, username and password. Explicit FTPS is enabled by default; disable it only for a server that requires plain FTP. Plain FTP sends login credentials without transport encryption even though backup contents remain encrypted.
3. For Google Drive or Dropbox, select **Connect account in browser** and complete the provider's consent screen. This uses Rclone's OAuth application, not a Straja password form. The authorization process expires after three minutes if unfinished. No account authorization is performed by opening the destination screen alone.
4. For Amazon S3, enter an existing bucket, its AWS region, Access Key ID and Secret Access Key. Use an identity with list/read/write/delete access to the backup prefix, including permissions needed for multipart uploads. This version uses Amazon's normal HTTPS endpoints and static credentials; custom S3 endpoints, role profiles and temporary session tokens are not exposed in the UI. Straja does not create a bucket for you.
5. Choose a relative online folder, for example `StrajaBackup`. Each plan creates its own `plan-<UUID>` subfolder. Do not move or rename these folders independently of the plan.
6. Set a transfer limit in KiB/s (default 10240; 0 means unlimited). **Test connection** checks listing/access and configuration; write access and available quota are checked by the actual backup. A successful connection test is not a completed backup.
7. Enter and separately save the backup encryption password. It is different from the provider's login or access key and is essential for recovery on another PC. Then set schedule and retention and save.

Backup, history, verification, restoration, VSS source reading and retention use the existing plan workflow. Backup data is streamed to the provider; a complete local staging copy is not required. Restic may keep its normal metadata cache locally. There is no promise of constant RAM usage for arbitrarily large sources.

## Editing and recovery

Editing preserves saved provider credentials when the credential fields are left untouched. To replace FTP/S3 credentials, supply the complete replacement credential set; reconnect Google Drive/Dropbox through the browser. Bandwidth, sources, exclusions, schedule and retention can be edited. A different destination/account location requires a new plan so existing copies stay reachable.

Deleting a plan removes its saved credentials and schedule but does not delete its online repository. Keep the repository password, provider access and full repository path separately; **Details & actions → Copy path** copies the complete remote path, including the plan subfolder. This version does not add an import-existing-repository wizard. External recovery can use Restic and Rclone with the same provider and encryption password. Do not initialize a new plan over an old repository or treat a new plan as importing old history.

Windows DPAPI encrypts stored provider credentials for the current account. Plaintext credentials/tokens are passed only in the environment of Straja's own transfer processes, not command arguments or an `rclone.conf` file. Processes running as the same user or an administrator can still inspect process memory/environment. Run the service under the same Windows account as the interface. Reconnect after revoking provider access or moving to another account/PC.

## Limits and operational notes

- FTP hostnames/IPv4 and explicit FTPS are supported. SFTP, implicit FTPS and IPv6 literal configuration are not included in this release.
- Provider capacity, throttling, network outages and S3 request/storage costs still apply. Verification and retention can read remote data and incur traffic. Interrupted Restic uploads can leave unreferenced encrypted objects for later maintenance; Straja never claims success after a failed transfer.
- Existing repositories are checked before use. A nonempty invalid destination is not reinitialized. Retention runs only after a successful new backup and the existing integrity checks; deleting a plan never runs remote deletion.
- Transfers use two Rclone transfers/checkers, a small buffer and no multithread file transfers. Restic-to-Rclone requests are also limited to two. FTP allows five pooled connections so metadata requests do not block behind transfers; this is connection headroom, not five parallel file transfers. The speed setting affects transfers, not all network/CPU activity. It does not alter other applications' settings or prevent every possible resource contention.
- Google Drive/Dropbox account consent and real Amazon IAM/quota behavior require validation with the user's account. Automated local FTP/S3-compatible tests cannot certify those account-specific conditions.

References: [Restic Rclone backend](https://restic.readthedocs.io/en/stable/030_preparing_a_new_repo.html#other-services-via-rclone), [Rclone FTP](https://rclone.org/ftp/), [Rclone Google Drive](https://rclone.org/drive/), [Rclone Dropbox](https://rclone.org/dropbox/), [Rclone S3](https://rclone.org/s3/).

## Developer verification

Run `scripts/test.ps1` for the regression suite and `scripts/test-remote.ps1` for isolated FTP and HTTPS S3-compatible roundtrips. The latter requires bundled `.tools/restic.exe` and `.tools/rclone.exe`, plus Git for Windows OpenSSL (or `STRAJA_TEST_OPENSSL` pointing to OpenSSL). Tests use temporary folders, synthetic credentials and loopback listeners. The HTTPS fixture creates a short-lived test certificate, trusts it only in the child process and does not change the Windows certificate store. No cloud account is used. FTP tests include retention, interrupted connectivity, safe refusal of a nonempty target and executable paths containing spaces.
