#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" == Linux && "${STRAJA_TEST_DBUS:-}" != 1 ]]; then
  export STRAJA_TEST_DBUS=1
  exec dbus-run-session -- bash "$0"
fi
if [[ "$(uname -s)" == Linux ]]; then
  # Disposable CI login keyring. Never run this script in a user's login session.
  export GNOME_KEYRING_CONTROL="$(mktemp -d)"
  printf 'ci-only-password' | gnome-keyring-daemon --unlock --components=secrets
fi
cargo test --locked --all-targets
cargo test --locked --test restic_roundtrip -- --ignored
cargo test --locked --test selected_exclusions selected_exclusions_restic -- --ignored
cargo clippy --locked --all-targets --all-features -- -D warnings
