#!/usr/bin/env bash
# A napi worker-daemon launchd agent telepítése.
#
# macOS TCC miatt a launchd NEM futtathat a Documents mappából, ezért az
# agent mindent a ~/Library/Application Support/FootballAppV2/agent alatt
# kap: bináris, Obscura, env (600), adatkönyvtár. A repó buildje a forrás;
# frissítéshez futtasd újra ezt a scriptet.
#
# Visszavonás: scripts/install-footballd-agent.sh --uninstall
set -euo pipefail
cd "$(dirname "$0")/.."
REPO="$PWD"
LABEL="hu.bandi86.football.v2"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
SUPPORT="$HOME/Library/Application Support/FootballAppV2"
AGENTDIR="$SUPPORT/agent"
LOGDIR="$HOME/Library/Logs/FootballAppV2"

if [ "${1:-}" = "--uninstall" ]; then
  launchctl bootout "gui/$UID" "$PLIST" 2>/dev/null || true
  rm -f "$PLIST"
  echo "agent eltávolítva (az adatok megmaradnak: $AGENTDIR)"
  exit 0
fi

BIN="$REPO/rust/target/debug/footballd"
if [ ! -x "$BIN" ]; then
  echo "hiányzó bináris — építsd: cd rust && cargo build -p footballd --features obscura" >&2
  exit 1
fi
if [ ! -f "$REPO/.env" ]; then
  echo "hiányzó .env a repó gyökerében" >&2
  exit 1
fi

OBSCURA="${OBSCURA_BIN:-$REPO/../football-app/tools/obscura/obscura}"
if [ ! -x "$OBSCURA" ]; then
  echo "hiányzó Obscura bináris: $OBSCURA (OBSCURA_BIN)" >&2
  exit 1
fi

mkdir -p "$AGENTDIR/bin" "$AGENTDIR/data" "$LOGDIR"
cp "$BIN" "$AGENTDIR/bin/footballd"
cp "$OBSCURA" "$AGENTDIR/bin/obscura"
cp "$REPO/.env" "$AGENTDIR/env"
chmod 600 "$AGENTDIR/env"

# Meglévő helyi adatok (nyers objektumtár, fixture-ök) átemelése egyszer.
if [ -d "$REPO/data" ] && [ -z "$(ls -A "$AGENTDIR/data" 2>/dev/null)" ]; then
  cp -R "$REPO/data/." "$AGENTDIR/data/"
  echo "adatok átemelve: $REPO/data → $AGENTDIR/data"
fi

cat > "$AGENTDIR/run.sh" <<RUN_EOF
#!/usr/bin/env bash
set -euo pipefail
set -a
. "$AGENTDIR/env"
set +a
export DATA_DIR="$AGENTDIR/data"
export WORKER_ENABLED=true
export RUST_LOG="\${RUST_LOG:-info}"
export OBSCURA_BIN="$AGENTDIR/bin/obscura"
exec "$AGENTDIR/bin/footballd"
RUN_EOF
chmod 700 "$AGENTDIR/run.sh"

cat > "$PLIST" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$AGENTDIR/run.sh</string>
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
echo "agent könyvtár: $AGENTDIR"
echo "log: $LOGDIR/footballd.log"
