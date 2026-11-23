#!/usr/bin/env bash
# A telepitett FootballApp hozzadasa a Dock-hoz (idempotens).
# Visszavonas: a Dock ikont jobb kattintas → „Eltavolitas a Dockrol",
# vagy a persistent-apps tombol valo tile torlese.
set -euo pipefail

APP="$HOME/Applications/FootballApp.app"
if [ ! -d "$APP" ]; then
  echo "nincs telepitett app: $APP — eloszor: scripts/install-football-app.sh" >&2
  exit 1
fi

PLIST="$(mktemp -t footballdock.XXXXXX)"
trap 'rm -f "$PLIST"' EXIT
defaults export com.apple.dock "$PLIST"

python3 - "$PLIST" "$APP" <<'PY'
import plistlib, sys, uuid
plist_path, app_path = sys.argv[1], sys.argv[2]
with open(plist_path, "rb") as f:
    dock = plistlib.load(f)
apps = dock.setdefault("persistent-apps", [])
normal = lambda u: str(u).rstrip("/")
if any(normal(t.get("tile-data", {}).get("file-data", {}).get("_CFURLString", "")) == app_path for t in apps):
    print("FootballApp mar a Dock-on van")
else:
    apps.append({
        "GUID": str(uuid.uuid4()).upper(),
        "tile-data": {
            "file-data": {
                "_CFURLString": "file://" + app_path + "/",
                "_CFURLStringType": 15,
            },
            "file-label": "FootballApp",
        },
        "tile-type": "file-tile",
    })
    with open(plist_path, "wb") as f:
        plistlib.dump(dock, f, fmt=plistlib.FMT_XML)
    print("FootballApp hozzadva a Dock-hoz")
PY

defaults import com.apple.dock "$PLIST"
killall Dock
