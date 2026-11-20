#!/usr/bin/env bash
# A futó FootballApp ablakának PNG-mentése (a harness vizuális bizonyítékához).
# Előfeltétel: a DeepSeek Harness (vagy a futtató terminál) rendelkezzen Screen Recording engedéllyel.
set -euo pipefail
cd "$(dirname "$0")/.."

output="${1:-/tmp/footballapp-window.png}"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
window_id="$(swift scripts/window-id.swift FootballApp)"
screencapture -l"$window_id" -x "$output"
echo "$output"
