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

## S30h — automatizált ellenőrzés (2026-11-20, futott)

Új, ismételhető harness: `apps/FootballAppUITests` (XCUITest) + `scripts/run-ui-tests.sh`
+ `scripts/ui-fixture-server.py` + `tests/fixtures/ui/*.json`. A szerver determinisztikus
napi adatot szolgál ki, a teszt `FOOTBALL_APP_RUNTIME_FILE` környezeti változóval erre
irányítja az appot, így futó daemon nélkül is ellenőrizhető a UI.

Futott eredmény (2026-11-20 12:09): `** TEST SUCCEEDED **`, 4/4 teszt zöld:

1. `testLoadedDaySpeaksTeamsStatusScoreAndPublishState` — a meccskártya akadálymentességi
   címkéje tartalmazza a csapatokat, az időt, a státuszt (Ütemezett/Lezárt), az eredményt
   (2 – 1), a hiányzó eredmény kimondását („eredmény még nincs”) és a publikációs
   minőségi állapotot (Ideiglenes/Ellenőrzött publikáció).
2. `testEmptyDayShowsEmptyState` — üres napon „Nincs meccs ezen a napon.” jelenik meg.
3. `testMissingRuntimeShowsDaemonOfflineError` — hiányzó `runtime.json` esetén a
   daemon-offline hibaüzenet jelenik meg.
4. `testKeyboardShortcutsMoveBetweenDays` — Cmd+→ (HOLNAP), Cmd+T (MA), Cmd+← (TEGNAP)
   valóban napi váltást okoz.

Tanulságok (a harness dokumentálva):
- A macOS UI-test runner sandboxja miatt a teszt nem nyithat saját socketet (`socket()` → EPERM),
  ezért a fixture-szerver külön folyamat; a `runtime.json`-t a runner a saját temp könyvtárába írja.
- A UI-teszteknél a `CODE_SIGNING_ALLOWED=NO` megöli a runnert — aláírás nélkül nem fut.
- A SwiftUI `Text` az AX-fában **üres `label`-lel, a `value` attribútumban** hordozza a szöveget,
  ezért a tesztek `value CONTAINS` predikátummal keresnek.
- A teszt-runnerből készített screenshot TCC miatt nem működik („Image creation failed”);
  a vizuális igazolás a harness oldaláról `CGWindowList` + `screencapture -l<windowID>` úton megy,
  kényelmesen: `./scripts/app-window-shot.sh [kimenet.png]`.

## Státusz (2026-11-20, S30h után)

- Automatikusan igazolt: 1. pont (VoiceOver-címkék tartalma, kártya + publikációs állapot),
  3. pont billentyűparancsai (Cmd+←/→/T), 5. pont három állapota (betöltött, üres, daemon-offline),
  valamint a magyar címkék visszaolvasása unit-tesztekkel.
- Továbbra is emberi/egyéb ellenőrzést igényel: tényleges VoiceOver-felolvasás (hang), Tab-fókusz
  láthatósága és fókuszcsapda, Reduced Motion, Cmd+R vizuális frissítés.
- Vizuális igazolás (2026-11-20 12:07): a futó app ablaka `screencapture -l442` módszerrel
  elmentve; a kezdőlap valós adattal (41 meccs, Ligue 1/Bundesliga/La Liga, odds) renderelődik.
- AX-vezérlés igazolva (2026-11-20 12:11): `scripts/ax-press.swift FootballApp "Következő nap"`
  → `AXPress result: 0`, utána a fejléc „november 21.” / „HOLNAP” / „90 meccs” lett
  (`/tmp/app-after-next.png`); az AX-fa olvasható (gombok magyar leírással, meccskártyák
  VoiceOver-címkéi). Ezzel a harness látja, olvassa és vezérli is az appot.
- A SwiftUI fázis DoD-je részben zárt; a fennmaradó pontok az S30h harness bővítésével
  (megjelenés, fókusz, részleges adat) zárhatók.

## S31a — részleges adat, kontraszt, fókusz (2026-11-20)

- Részleges adat automatizálva: `tests/fixtures/ui/partial-day.json` (élő meccs, elhalasztott
  eredmény nélkül, visszatartott publikációjú lezárt, félbeszakadt/karanténos meccs) + új UI-teszt
  `testPartialDataSpeaksStatusScoreAndPublishState`. Futott: **5/5 UI-teszt zöld**.
  Tanulság: a kártya `match.card.<uuidString>` azonosítója NAGYBETŰS UUID-t használ.
- Kontraszt automatizálva: `apps/FootballAppTests/ThemeContrastTests.swift` WCAG 2.1 arányt számol
  minden szöveg-tokenre mindhárom felületen (küszöb: `primaryText` 7:1, többi 4.5:1), és ellenőrzi,
  hogy a háttér valóban `#000000`. Futott: **26/26 app unit teszt zöld**. Mivel a kliens szándékosan
  egyetlen sötét paletta (nincs külön világos renderelési út), a „világos/sötét” pontot ez a
  kontraszt-teszt fedi le.
- Tab-fókusz: a harness oldali mérés (`scripts/ax-focus.sh`) ezen a builden nem mutat fókuszt —
  az app processzén az `AXFocusedUIElement` Tab és `AXPress` után is `AXWindow` marad, `-g
  AppleKeyboardUIMode -int 3` mellett is. Ezért a fókuszpont továbbra sem automatizált; következő
  lépés app-oldali fókuszálhatóság (`.focusable()` / `focusSection`) + `-AppleKeyboardUIMode 3`
  launch argumentumos XCUITest, vagy célzott vizuális ellenőrzés.
