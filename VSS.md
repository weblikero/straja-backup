# Backing up files in use

Version 0.6.0 adds a per-plan **Back up files in use (Windows VSS)** option in the schedule/retention step. Existing plans keep VSS disabled until edited. It works with encrypted Restic backups, ordinary files, ZIP, TAR.GZ and 7z. Restore and verification do not require VSS privileges.

## Enable it

1. Edit a plan, enable the VSS checkbox and save.
2. If the agent lacks the required privileges, click **Enable VSS agent** and approve the Windows UAC prompt using the **same Windows account**. The GUI stays unelevated. A busy agent is never stopped; finish its current operation first.
3. Run a manual backup, check its final status and test a restore.

The elevated interactive agent lasts until that agent process exits. After restarting it or upgrading the app, enable VSS again if needed. This does not create a scheduled task, change UAC settings or make ordinary sign-in startup elevated. For schedules that must run while signed out, use the existing `scripts/install-service.ps1` setup under the same administrator account, with that account's password (not Windows Hello PIN). Do not switch to LocalSystem or another account: saved repository passwords are protected with per-user DPAPI. The service and interactive agent are alternatives; do not run both for the same plan store.

## Supported scope and limitations

- Sources must be absolute paths on fixed local NTFS drives with drive letters. Network shares, mapped drives, removable media, ReFS and volumes mounted in folders are outside this initial implementation. The destination does not need VSS.
- A snapshot captures data saved to disk, including files whose live handles deny read sharing. Unsaved editor buffers are not backed up. Directory junctions/symlinks/reparse points still need to be excluded or replaced with their real source path.
- Unencrypted formats use a file-level, crash-consistent snapshot without VSS writers. Encrypted mode uses Restic's VSS copy-backup implementation and its Windows `FilesNotToSnapshot` exclusions. Neither mode claims application-consistent database recovery, System State or bare-metal recovery. Use native SQL/database backups and verify restoration separately.
- Snapshot creation needs free space for Windows' copy-on-write data. Straja does not resize shadow storage, delete restore points or delete snapshots belonging to other applications. A concurrent VSS operation, provider error or insufficient storage can cause a backup to fail; the existing schedule retries failed runs after 15 minutes.
- Plain/archive mode publishes a completed version only after source copying and snapshot cleanup succeed. A failed copy can leave an `.in-progress-*` folder. Restic may create a partial backup after a VSS failure; Straja marks reported VSS errors as incomplete even when Restic exits with code zero. Such runs do not advance last-success or trigger retention.

## Implementation

`src/vss.rs` owns path validation, source-to-shadow mapping and the snapshot lease. A dedicated thread owns COM for the entire snapshot lifetime. `native/vss.cpp` is a small Windows SDK bridge for `IVssBackupComponents`, which is absent from the Windows metadata bindings used here. It uses `VSS_CTX_FILE_SHARE_BACKUP`, `VSS_BT_COPY` and a 120-second asynchronous creation deadline. Snapshots are nonpersistent and auto-release; explicit cleanup addresses only the newly created snapshot-set ID. Cancellation drops the lease and the COM thread cleans up. Plain snapshots are released before archive compression begins.

Logical source paths remain in manifests, exclusions and progress messages. They are mapped to read-only shadow device paths only when reading. Original source bytes are never written by VSS or the backup engine. Restic receives `--use-fs-snapshot -o vss.timeout=120s`; structured error messages are tracked independently of its bounded diagnostic tail. Included mounted folders are rejected before invoking Restic to avoid its unsupported-mount fallback.

The optional elevation flow checks the expected Windows user SID, reserves the idle agent against new scheduled jobs, preserves its data directory, requests a graceful shutdown and starts the same installed agent elevated. It does not force-kill processes. The administrator approval grants the agent backup/restore privileges for that session; enable it only for trusted plans and destinations.

## Developer validation

Build `cargo test --test vss --no-run` using `scripts/cargo.ps1`, with `CARGO_TARGET_DIR` set to `target/verification`. Ordinary tests cover old-plan defaults, persistence, path mapping and refusing backup without sufficient privileges. In an elevated PowerShell window, run `scripts/test-vss.ps1` with the resulting `target/verification/debug/deps/vss-*.exe` path and a new `test-output/vss-...` output directory. Elevated tests exercise an exclusively locked file, point-in-time stability, explicit cleanup, all five backup/restore formats, logical-path exclusions and failure isolation from another snapshot set. They use temporary test data and do not use installed plans or agents.

References: [Microsoft VSS contexts](https://learn.microsoft.com/en-us/windows/win32/api/vss/ne-vss-vss_snapshot_context), [Restic filesystem snapshots](https://restic.readthedocs.io/en/stable/040_backup.html#creating-snapshots).
