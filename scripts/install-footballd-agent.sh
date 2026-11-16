#!/usr/bin/env bash
# A napi worker-daemon launchd agent telepítése (felhasználói LaunchAgent).
# Visszavonás: scripts/install-footballd-agent.sh --uninstall
set -euo pipefail
cd "$(dirname "$0")/.."
REPO="$PWD"
LABEL="hu.bandi86.football.v2"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
LOGDIR="$HOME/Library/Logs/FootballAppV2"

if [ "${1:-}" = "--uninstall" ]; then
  launchctl bootout "gui/$UID" "$PLIST" 2>/dev/null || true
  rm -f "$PLIST"
  echo "agent eltávolítva: $PLIST"
  exit 0
fi

if [ ! -x "$REPO/rust/target/debug/footballd" ]; then
  echo "hiányzó bináris — építsd: cd rust && cargo build -p footballd --features obscura" >&2
  exit 1
fi

mkdir -p "$HOME/Library/LaunchAgents" "$LOGDIR"
cat > "$PLIST" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$REPO/scripts/run-footballd.sh</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>ProcessType</key><string>Background</string>
  <key>StandardOutPath</key><string>$LOGDIR/footballd.log</string>
  <key>StandardErrorPath</key><string>$LOGDIR/footballd.log</string>
</dict>
</plist>
PLIST_EOF

launchctl bootout "gui/$UID" "$PLIST" 2>/dev/null || true
launchctl bootstrap "gui/$UID" "$PLIST"
launchctl kickstart -k "gui/$UID/$LABEL"
echo "agent telepítve és elindítva: $LABEL"
echo "log: $LOGDIR/footballd.log"
