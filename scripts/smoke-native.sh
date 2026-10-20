#!/usr/bin/env bash
# Disposable native CI machines only; never run against a user's installation.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${GITHUB_ACTIONS:-}" != true ]]; then
  echo "Installation smoke tests require a disposable GitHub Actions runner." >&2
  exit 1
fi
start_agent() {
  "$base/straja-agent" > /tmp/straja-agent.log 2>&1 &
  agent=$!
  for _ in $(seq 1 20); do
    if "$base/straja-agent" --status > /tmp/straja-status.json 2>/dev/null; then
      python3 -c 'import json; s=json.load(open("/tmp/straja-status.json"))["State"]; assert s["engine_available"] and s["active"] is None'
      return
    fi
    sleep 0.5
  done
  cat /tmp/straja-agent.log
  return 1
}
stop_agent() {
  "$base/straja-agent" --stop-if-idle
  wait "$agent"
}
if [[ "$(uname -s)" == Linux ]]; then
  sudo dpkg -i dist/native/*.deb
  /opt/straja-backup/straja --version
  /opt/straja-backup/straja-agent --version
  /opt/straja-backup/restic version
  /opt/straja-backup/rclone version
  test -f /usr/share/applications/straja-backup.desktop
  base=/opt/straja-backup
  start_agent
  # A software renderer makes this independent of a GPU on the hosted runner.
  set +e
  ICED_BACKEND=tiny-skia timeout 12s xvfb-run -a /opt/straja-backup/straja > /tmp/straja-gui.log 2>&1
  status=$?
  set -e
  cat /tmp/straja-gui.log
  test "$status" = 124 # The event loop stayed alive until the smoke-test deadline.
  stop_agent
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
  base="/Applications/Straja Backup.app/Contents/MacOS"
  start_agent
  ICED_BACKEND=tiny-skia "/Applications/Straja Backup.app/Contents/MacOS/straja" > /tmp/straja-gui.log 2>&1 &
  gui=$!
  sleep 10
  kill -0 "$gui"
  kill "$gui"
  wait "$gui" || true
  cat /tmp/straja-gui.log
  stop_agent
fi
