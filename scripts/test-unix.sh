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
  # Separate unsigned test executables need separate items: even a permissive
  # ACL can prompt for consent across code identities on current macOS.
  # This namespace is honored only in Mac debug builds on GitHub Actions.
  # Keychain entries disappear with the disposable runner, never in artifacts.
  export STRAJA_CI_KEYCHAIN_NAMESPACE="$(uuidgen)"
fi
cargo test --locked --all-targets --no-fail-fast
cargo test --locked --test restic_roundtrip -- --ignored
cargo test --locked --test selected_exclusions selected_exclusions_restic -- --ignored
cargo test --locked --test remote_destinations ftp_backup_restore_retention_and_failure_preserve_other_files -- --ignored
