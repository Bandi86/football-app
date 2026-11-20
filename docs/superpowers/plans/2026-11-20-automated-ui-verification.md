# S30h — Automatizált macOS UI-ellenőrzés (XCUITest)

## Cél

A `docs/operations/ui-accessibility-checklist.md` 2–6. pontjának (VoiceOver-szövegek, billentyűparancsok,
megjelenési módok, állapotok, magyar visszaolvasás) ismételhető, emberi kézi ellenőrzés nélkül futó
igazolása. A cél nem a VoiceOver beszédhang helyettesítése, hanem az, hogy a felolvasott **tartalom**
ugyanabból az akadálymentességi fából legyen ellenőrizve, amit a VoiceOver használ, és hogy a
billentyűzetes/állapot-viselkedés minden szeletnél automatikusan zöld legyen.

## Megközelítés

1. **Launch-hook (app)**: a `RuntimeInfoLoader.defaultURL` tiszteletben tartja a
   `FOOTBALL_APP_RUNTIME_FILE` környezeti változót, ha be van állítva. Ez teszi lehetővé, hogy a
   teszt saját `runtime.json`-t adjon az appnak (determinisztikus port/token), a napi fejlesztői
   futást nem érinti, és unit-teszttel védett.
2. **UI-teszt cél**: `FootballAppUITests` (`bundle.ui-testing`, `TEST_TARGET_NAME: FootballApp`) a
   `apps/project.yml`-ben, a `FootballApp` scheme tesztlistájára fűzve.
3. **Fixture szerver a tesztben**: a teszt a `Network` keretrendszerrel indít egy eldobható HTTP
   szervert a `127.0.0.1:0` címen, a `v1/day/<dátum>` végpontra szcenárió-specifikus JSON-t ad
   (`loaded`, `empty`, `error`), és a temp könyvtárba írt `runtime.json`-nal indítja az appot.
   Nincs külső függőség, nincs futó daemon-kötelezettség.
4. **Állítások**:
   - a meccssor/kártya `accessibilityLabel`-je tartalmazza a csapatokat, az időt, a státuszt és a
     „eredmény még nincs” szöveget hiányzó eredmény esetén;
   - a publikációs badge címkéje a minőségi állapotot mondja ki;
   - üres szűrő → `match.list.empty`; hiányzó/hibás runtime → hibaállapot szövege;
   - Cmd+←/→/T/R megváltoztatja a fejlécet, illetve frissít;
   - minden szcenárióról képernyőkép-attachment készül (vizualitás visszaolvasásához).
5. **Bizonyíték**: `xcodebuild test -only-testing:FootballAppUITests` kimenete + a kapu-lánc.

## Elfogadás

- [x] `xcodegen generate` hibátlan a két új célfüggőséggel.
- [x] `FOOTBALL_APP_RUNTIME_FILE` unit-tesztje zöld (Core, 34/34).
- [x] Az UI-tesztek zöldek (`loaded`, `empty`, `error` szcenárió és a billentyű-parancsok):
      4/4, `** TEST SUCCEEDED **` (2026-11-20 12:09).
- [ ] A teljes kapu-lánc zöld; a checklist 2–6. pontja automatizált bizonyítékot kap (a VoiceOver
      beszédhangja továbbra is emberi lépés, ezt a dokumentum kimondja). — a kapu futtatása a
      szelet zárásakor történik.

## Kockázatok

- A macOS UI-teszt futtatásához a tesztfolyamatnak Accessibility-engedély kellhet (TCC); ha a rendszer
  megtagadja, a szelet addig „blokkolt”, amíg a `DeepSeek Harness`/Xcode kapcsoló be nem kapcsol.
- Az XCUITest macOS-en lassabb; a teszt csak a kritikus, stabil azonosítókat (`match.card.*`,
  `publish.badge.*`, `match.list.empty`) használja, a nem determinisztikus tartalomtól független.
## Eredmény (2026-11-20)

- Megvalósult: `FootballAppUITests` cél (4 teszt), `FOOTBALL_APP_RUNTIME_FILE` felülírás + Core
  unit-tesztek, `scripts/ui-fixture-server.py`, `scripts/run-ui-tests.sh`, `tests/fixtures/ui/*`.
- Futott: UI 4/4 zöld; Core 34/34; app unit 24/24; a fixture-szerver nélkül a runner socketet sem
  tud nyitni (sandbox EPERM), ezért külső folyamat szolgálja ki az adatot.
- Eltérés a tervhez: a teszten belüli screenshot (XCTAttachment) TCC miatt nem működik; a vizuális
  bizonyítékot a harness `CGWindowList` + `screencapture -l<windowID>` úton készíti.
- A `publish.badge.*` azonosító a kártya kombinált AX-elemében nem külön elem, ezért a publikációs
  állapotot a kártya címkéje mondja ki (`AccessibilityLabels.matchCardLabel` + `publishState`).
