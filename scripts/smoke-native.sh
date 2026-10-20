#!/usr/bin/env bash
# Disposable native CI machines only; never run against a user's installation.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" == Linux ]]; then
  sudo dpkg -i dist/native/*.deb
  /opt/straja-backup/straja --version
  /opt/straja-backup/straja-agent --version
  /opt/straja-backup/restic version
  /opt/straja-backup/rclone version
  test -f /usr/share/applications/straja-backup.desktop
  # A software renderer makes this independent of a GPU on the hosted runner.
  set +e
  ICED_BACKEND=tiny-skia timeout 12s xvfb-run -a /opt/straja-backup/straja > /tmp/straja-gui.log 2>&1
  status=$?
  set -e
  cat /tmp/straja-gui.log
  test "$status" = 124 # The event loop stayed alive until the smoke-test deadline.
  sudo dpkg -r straja-backup
else
  mount="$(mktemp -d)"
  hdiutil attach dist/native/*.dmg -nobrowse -mountpoint "$mount"
  app="$mount/Straja Backup.app"
  "$app/Contents/MacOS/straja" --version
  "$app/Contents/MacOS/straja-agent" --version
  codesign --verify --deep --strict "$app"
  # Test an installed copy, not an executable inside a read-only mounted image.
  ditto "$app" "/Applications/Straja Backup.app"
  hdiutil detach "$mount"
  ICED_BACKEND=tiny-skia "/Applications/Straja Backup.app/Contents/MacOS/straja" > /tmp/straja-gui.log 2>&1 &
  gui=$!
  sleep 10
  kill -0 "$gui"
  kill "$gui"
  wait "$gui" || true
  cat /tmp/straja-gui.log
fi
