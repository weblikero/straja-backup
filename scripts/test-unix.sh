#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${GITHUB_ACTIONS:-}" != true ]]; then
  echo "This script requires a disposable GitHub Actions runner." >&2
  exit 1
fi
if [[ "$(uname -s)" == Linux && "${STRAJA_TEST_DBUS:-}" != 1 ]]; then
  export STRAJA_TEST_DBUS=1
  exec dbus-run-session -- bash "$0"
fi
if [[ "$(uname -s)" == Linux ]]; then
  # Disposable CI login keyring. Never run this script in a user's login session.
  export GNOME_KEYRING_CONTROL="$(mktemp -d)"
  printf 'ci-only-password' | gnome-keyring-daemon --unlock --components=secrets
else
  # Each Rust integration test is a different executable. Without an explicit
  # test ACL, Keychain prompts for consent when the next test accesses the key
  # created by the previous binary, blocking an unattended runner.
  # No -U: refuse to replace an existing item. This ephemeral CI key is never
  # included in a package, and production code keeps the normal Keychain ACL.
  security add-generic-password -a database-wrapping-key-v1 \
    -s com.weblike.straja-backup -w "$(openssl rand -hex 16)" -A
  trap 'security delete-generic-password -a database-wrapping-key-v1 -s com.weblike.straja-backup >/dev/null 2>&1' EXIT
fi
cargo test --locked --all-targets --no-fail-fast
cargo test --locked --test restic_roundtrip -- --ignored
cargo test --locked --test selected_exclusions selected_exclusions_restic -- --ignored
cargo test --locked --test remote_destinations ftp_backup_restore_retention_and_failure_preserve_other_files -- --ignored
