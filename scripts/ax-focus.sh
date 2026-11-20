#!/usr/bin/env bash
# A futó app fókusz-láncának ellenőrzése: Tab-bal lépked, és minden lépésnél
# kiírja az AXFocusedUIElement szerepét/leírását. Ehhez Accessibility-engedély kell.
# Használat: ./scripts/ax-focus.sh FootballApp [tablépések=6] [screenshot-előtag]
set -euo pipefail
app="${1:-FootballApp}"
steps="${2:-6}"
shot_prefix="${3:-}"

osascript - "$app" "$steps" "$shot_prefix" <<'APPLESCRIPT'
on run argv
    set appName to item 1 of argv
    set steps to (item 2 of argv) as integer
    set shotPrefix to item 3 of argv
    tell application appName to activate
    delay 0.4
    tell application "System Events"
        repeat with i from 1 to steps
            key code 48 -- Tab
            delay 0.25
            set focusedElement to value of attribute "AXFocusedUIElement" of application "System Events"
            set roleText to ""
            set descText to ""
            try
                set roleText to role of focusedElement as text
            end try
            try
                set descText to description of focusedElement as text
            end try
            try
                set descText to descText & " | " & (value of attribute "AXTitle" of focusedElement as text)
            end try
            log "tab " & i & ": " & roleText & " — " & descText
            if shotPrefix is not "" then
                do shell script "screencapture -x " & quoted form of (shotPrefix & i & ".png")
            end if
        end repeat
    end tell
    return "ok"
end run
APPLESCRIPT
