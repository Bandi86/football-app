#!/usr/bin/env bash
# S30h: determinisztikus fixture-szerverekkel futtatja a Swift UI-teszteket.
set -euo pipefail
cd "$(dirname "$0")/.."

PORT_LOADED="${FOOTBALL_UI_PORT_LOADED:-18080}"
PORT_EMPTY="${FOOTBALL_UI_PORT_EMPTY:-18081}"
PORT_PARTIAL="${FOOTBALL_UI_PORT_PARTIAL:-18082}"

pids=()
cleanup() {
    for pid in "${pids[@]:-}"; do
        kill "$pid" 2>/dev/null || true
    done
}
trap cleanup EXIT

python3 scripts/ui-fixture-server.py --port "$PORT_LOADED" \
    --fixture tests/fixtures/ui/loaded-day.json \
    --detail-fixture tests/fixtures/ui/match-detail.json \
    --detail-map-fixture tests/fixtures/ui/match-chain-details.json \
    --notes-fixture tests/fixtures/ui/match-notes.json \
    --search-fixture tests/fixtures/ui/search-results.json \
    --player-fixture tests/fixtures/ui/player-detail.json \
    --match-micro-fixture tests/fixtures/ui/match-micro.json \
    --team-fixture tests/fixtures/ui/team-detail.json \
    --competition-fixture tests/fixtures/ui/competition-detail.json \
    --ops-status-fixture tests/fixtures/ui/ops-status.json \
    --issues-fixture tests/fixtures/ui/ops-issues.json \
    --history-fixture tests/fixtures/ui/ops-history.json \
    --ops-summary-fixture tests/fixtures/ui/ops-summary.json \
    --ops-audit-fixture tests/fixtures/ui/ops-audit.json &
pids+=("$!")
python3 scripts/ui-fixture-server.py --port "$PORT_EMPTY" --fixture tests/fixtures/ui/empty-day.json \
    --ops-status-fixture tests/fixtures/ui/ops-status.json \
    --issues-fixture tests/fixtures/ui/ops-issues-empty.json &
pids+=("$!")
python3 scripts/ui-fixture-server.py --port "$PORT_PARTIAL" --fixture tests/fixtures/ui/partial-day.json \
    --team-fixture tests/fixtures/ui/team-detail.json \
    --competition-fixture tests/fixtures/ui/competition-detail.json \
    --ops-status-fixture tests/fixtures/ui/ops-status.json \
    --issues-fixture tests/fixtures/ui/ops-issues.json \
    --history-fixture tests/fixtures/ui/ops-history.json \
    --ops-summary-fixture tests/fixtures/ui/ops-summary.json \
    --ops-audit-fixture tests/fixtures/ui/ops-audit.json &
pids+=("$!")

for _ in $(seq 1 100); do
    if curl -sf -m 1 "http://127.0.0.1:${PORT_LOADED}/v1/health" >/dev/null \
        && curl -sf -m 1 "http://127.0.0.1:${PORT_EMPTY}/v1/health" >/dev/null \
        && curl -sf -m 1 "http://127.0.0.1:${PORT_PARTIAL}/v1/health" >/dev/null; then
        break
    fi
    sleep 0.1
done

export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"

# S173: a scheme unit + UI teszteket is futtatna; együttes futtatáskor a
# teszt-runner elveszíti az app ablakának vezérlését (6 régi teszt hamisan bukott).
# Az unit tesztek külön xcodebuild -only-testing:FootballAppTests paranccsal futnak.
has_only_testing=0
for arg in "$@"; do
    case "$arg" in
        -only-testing*) has_only_testing=1 ;;
    esac
done
# A macOS bash 3.2 `set -u` mellett nem engedi az üres tömb bontását, ezért
# szövegként építjük (a argumentumok űr nélküli tokenek).
extra_args="$*"
if [ "$has_only_testing" -eq 0 ]; then
    extra_args="-only-testing:FootballAppUITests $extra_args"
fi

xcodebuild test \
    -project apps/FootballApp.xcodeproj \
    -scheme FootballApp \
    -destination 'platform=macOS' \
    $extra_args
