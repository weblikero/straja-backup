#!/usr/bin/env bash
# Run as the logged-in desktop user, never with sudo.
set -euo pipefail
if [[ "$(id -u)" == 0 ]]; then
  echo "Run this as your normal desktop user, without sudo." >&2
  exit 1
fi
base="$(cd "$(dirname "$0")" && pwd)"
"$base/straja-agent" --stop-if-idle
if [[ "$(uname -s)" == Darwin ]]; then
  plist="$HOME/Library/LaunchAgents/com.weblike.straja-backup.plist"
  mkdir -p "$(dirname "$plist")"
  python3 - "$base" "$plist" <<'PY'
import plistlib,sys
from pathlib import Path
base, target = sys.argv[1:]
with open(target, "wb") as f:
    plistlib.dump({"Label":"com.weblike.straja-backup",
                  "ProgramArguments":[str(Path(base)/"straja-agent")],
                  "RunAtLoad":True, "KeepAlive":{"SuccessfulExit":False},
                  "ProcessType":"Background","Nice":10}, f)
PY
  launchctl bootstrap "gui/$(id -u)" "$plist"
else
  mkdir -p "$HOME/.config/systemd/user"
  # Installation paths with newlines/quotes/% require systemd escaping; refuse them.
  case "$base" in *$'\n'*|*'"'*|*'%'*|*'\'*) echo "Unsupported installation path"; exit 1;; esac
  cat > "$HOME/.config/systemd/user/straja-backup.service" <<EOF
[Unit]
Description=Straja Backup background agent
After=graphical-session.target
[Service]
Type=simple
ExecStart="$base/straja-agent"
Restart=on-failure
RestartSec=15
Nice=10
UMask=0077
[Install]
WantedBy=default.target
EOF
  systemctl --user daemon-reload
  systemctl --user enable --now straja-backup.service
fi
echo "Straja will start at login. Backups need your login keychain unlocked."
