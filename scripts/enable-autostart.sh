#!/usr/bin/env bash
# Run as the logged-in desktop user, never with sudo.
set -euo pipefail
if [[ "$(id -u)" == 0 ]]; then
  echo "Run this as your normal desktop user, without sudo." >&2
  exit 1
fi
base="$(cd "$(dirname "$0")" && pwd)"
if [[ "$(uname -s)" == Darwin ]]; then
  base="$(cd "$base/../MacOS" && pwd)"
fi
"$base/straja-agent" --stop-if-idle
if [[ "$(uname -s)" == Darwin ]]; then
  plist="$HOME/Library/LaunchAgents/com.weblike.straja-backup.plist"
  mkdir -p "$(dirname "$plist")"
  escaped="$(printf '%s' "$base/straja-agent" | sed 's/\&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g;')"
  cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.weblike.straja-backup</string>
<key>ProgramArguments</key><array><string>$escaped</string></array>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
<key>ProcessType</key><string>Background</string>
<key>Nice</key><integer>10</integer>
</dict></plist>
EOF
  chmod 600 "$plist"
  plutil -lint "$plist"
  if launchctl print "gui/$(id -u)/com.weblike.straja-backup" >/dev/null 2>&1; then
    launchctl kickstart "gui/$(id -u)/com.weblike.straja-backup"
  else
    launchctl bootstrap "gui/$(id -u)" "$plist"
  fi
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
