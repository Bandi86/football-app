# SwiftUI akadálymentességi és kézi UI-ellenőrzési lista

A lista a spec §3.6/§3.7 és §17 (Definition of Done) teljesülését szolgálja.

## Automatikusan ellenőrzött (2026-11-20)
- `AccessibilityLabelsTests`: 6/6 (`swift test --filter AccessibilityLabelsTests` a `FootballAppCore`-ban).
- Teljes Core teszt: 32/32. App teszt: 24/24 (Xcode, `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test -project apps/FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS'`).
- A napi lista és a meccs-sor VoiceOver-címkéi közös, tesztelt forrásból jönnek (`FootballAppCore/AccessibilityLabels.swift`).
- A hiányzó eredményt a címke kimondja (`eredmény még nincs`), nem `0 – 0`.

## S30g felmérés (2026-11-20, állapot a DSH-újraindítás után)

- Daemon: `launchctl print gui/$(id -u)/hu.bandi86.football.v2` → `state = running`, pid 962;
  `GET http://127.0.0.1:49387/v1/health` → `{"status":"ok","service":"footballd"}` (port a
  `~/Library/Application Support/FootballAppV2/runtime.json`-ból).
- Futó kliens: `FootballApp`, pid 30733, bundle:
  `/private/tmp/footballapp-xcode-fresh/Build/Products/Debug/FootballApp.app`.
- **Képernyő-olvasás működik:** `screencapture -x /tmp/dsh-sc-test.png` → 1920×1080 kép
  (2 114 749 bájt). A korábbi `could not create image from display` hiba megszűnt.
- **Vezérlés (AX) még nem:** `osascript -e 'tell application "System Events" to get name of
  frontmost application'` → `execution error: Can’t get name of frontmost application. (-1728)`;
  `… get name of every window` → `System Events got an error: osascript is not allowed assistive
  access. (-1728)`. A System Settings „Device Control and Data Access” listájában a
  `DeepSeek Harness` kapcsoló KI van kapcsolva (`osascript` be van kapcsolva, de a felelős
  folyamat a harness) → **egyetlen emberi lépés: a DeepSeek Harness bekapcsolása**, utána a
  harness képes az appot olvasni/vezérelni.
- Vizualisan igazolt (képernyőkép, 11:46): a DSH jobb oldali Tasks oldala a csapat-táblával
  (`lead`, `probe-swift`, `probe-rust`, `probe-swift2`, `probe-roster`) hiba nélkül renderelődik,
  tehát a `dsh-better-sidebar` 0.32.1 javítása él.

## Kézzel elvégzendő (a gép előtt, a futó daemonal)
1. Indítsd az alkalmazást: `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test …` után
   `open /tmp/footballapp-xcode-fresh/Build/Products/Debug/FootballApp.app` (vagy a saját DerivedData alól).
2. VoiceOver (Cmd+F5): végiglépni a dátumválasztón, a szűrőkön, egy liga-fejlécen, egy meccskártyán
   és a publikációs badge-en. A felolvasott szöveg legyen magyar, tartalmazzon csapatokat, időt,
   státuszt, eredményt vagy a „nincs eredmény” kimondását, és a minőségi állapotot.
3. Billentyűzet: Tab-ciklus a navigációban; Cmd+←/→ (előző/következő nap), Cmd+T (Ma), Cmd+R (frissítés).
   A fókusz látható, a dialógusokban nincs fókuszcsapda.
4. Világos/sötét mód, Reduced Motion, rendszerkontraszt: a szöveg és a badge-ek olvashatók, a szín nem egyedüli jel.
5. Állapotok: betöltés, üres szűrő, hiba, daemon-offline és részleges adat külön szöveggel jelenik meg.
6. A magyar felületi szövegeket olvasd vissza (ékezetek, dátum- és odds-formátum).

## Státusz (2026-11-20)

- Felhasználói visszajelzés (m00389): az app fut, a kezdőlapon adatok vannak, hibát nem lát;
  a 2–6. pontot (VoiceOver-felolvasás, billentyűzet, megjelenés/kontraszt, állapotok, magyar
  visszaolvasás) még nem próbálta.
- A kézi lista 2–6. pontja tehát **továbbra sem igazolt**. A vizuális igazolás (képernyőkép)
  most már lehetséges, az AX-vezérléshez a `DeepSeek Harness` Accessibility-kapcsolója kell.
- Terv (S30h): automatizált XCUITest-harness a `apps/`-ban (új `FootballAppUITests` cél), amely a
  VoiceOver által használt akadálymentességi címkéket olvassa, billentyűparancsokat küld, és
  fixture `runtime.json`-nal determinisztikus betöltés/üres/hiba/offline állapotot állít elő:
  ezzel a 2–6. pontok ismételhetően, emberi kézi ellenőrzés nélkül futnak.
- A SwiftUI fázis DoD-je **még nem zárt**; a kód-oldali címkék és billentyűparancsok megvannak.
