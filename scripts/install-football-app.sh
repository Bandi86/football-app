#!/usr/bin/env bash
# A SwiftUI kliens leforditasa es telepitese a felhasznaloi Applications mappaba.
# A Dock-hoz adast a scripts/add-football-app-to-dock.sh vegzi (kulon felelosseg).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
APP_NAME="FootballApp"
BUNDLE_ID="hu.bandi86.FootballApp"
DERIVED="$ROOT/apps/.build/xcode"
PRODUCT="$DERIVED/Build/Products/Debug/$APP_NAME.app"
TARGET="$HOME/Applications/$APP_NAME.app"

cd "$ROOT/apps"
xcodegen generate
xcodebuild -project FootballApp.xcodeproj -scheme FootballApp \
  -derivedDataPath "$DERIVED" -destination 'platform=macOS' build

if [ ! -d "$PRODUCT" ]; then
  echo "hianyzó build eredmeny: $PRODUCT" >&2
  exit 1
fi

mkdir -p "$HOME/Applications"
if [ -d "$TARGET" ]; then
  EXISTING="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$TARGET/Contents/Info.plist" 2>/dev/null || true)"
  if [ "$EXISTING" != "$BUNDLE_ID" ]; then
    echo "A celhelyen mas app all: $TARGET — nem irjuk felul." >&2
    exit 1
  fi
  rm -rf "$TARGET"
fi
ditto "$PRODUCT" "$TARGET"

# A lokalisan epitett appnek nincs quarantine jelzese, ez ugy is indulo.
echo "app telepitve: $TARGET"
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$TARGET/Contents/Info.plist"
