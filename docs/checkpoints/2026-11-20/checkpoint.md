# 2026-11-20 — napi checkpoint

## Induló állapot (read-only ellenőrzés)
- 255 749 meccs, publish_gate: 208 377 provisional / 47 372 verified, withheld 0.
- **555 függő review** (tegnap 0): 399 soccerway + 156 openfoot, 07:31–07:33 között,
  `no_exact_match` 548 / `fuzzy_candidate` 7. Integritás egyébként OK.
- launchd daemon fut (pid 962), napi jobok 16 done.

## S30a — review bootstrap
- [x] Triage: a 7 fuzzy jelöltből 3 egyértelmű alias (Almere City FC → Almere City,
      FC Den Bosch → Den Bosch, Roda JC Kerkrade → Roda JC) kézi `identity approve`;
      a tartalékcsapatok (SK Rapid II, Jong AZ Alkmaar, Sporting CP B, Shakhtar
      Donetsk) külön entitások maradtak, mert a jelölt nem azonos klub.
- [x] `identity approve-create-all` 536 + `--allow-unknown-country` 16 sor; pending 0.
      Összes csapat: 2 702. Az új csapatok főleg Indonézia, Ausztrália, Vietnam,
      Irán és Wales új ligáiból jöttek — valódi új entitások, nem mapping-hiba.
- [x] A mai 4 day-job újrafuttatása (budget módosítás nélkül): footballdata,
      openfoot, tipplap done; soccerway 3× timeout (45 s) → `cancelled`, auditálva
      (az eredeti 07:31-i fetch sikeres volt, további retry nem indokolt).
- [x] Végállapot: **255 829 meccs** (+80), ma 118 meccs, pending review 0,
      withheld 0, integritás OK.

## S30b — integritás az API-ban és az Üzemeltetés nézetben
- [x] `GET /v1/ops/status` most `integrity` blokkot ad: verdict (OK/FAIL),
      duplikátum kickoff/fixture, árva provenance, összevont entitásra mutató
      aktív ref, pending review, withheld gate, failed job, nyitott breaker.
      Read-only, a meglévő `IntegrityReport` alapján.
- [x] Swift `OpsIntegrity` modell + Üzemeltetés nézet integritás-kártya
      (verdict színnel, 4 fő számláló); régi API-válasz esetén opcionális.
- [x] Tesztek: API `ops_status` 9/9 (új integritás-assertel),
      `FootballAppCore` 26/26. Kapu-lánc: db-reset → check-foundation →
      `cargo test --workspace -- --test-threads=1` → clippy `-D warnings` →
      fmt check → build `--features obscura` → db-reset: **EXIT 0**, 118 zöld
      tesztcsoport, 0 hiba.

## Nyitott / döntési pont
- A gateway blokkolta az `install-footballd-agent.sh` közvetlen futtatását
  („cannot restart … the gateway from inside the gateway process”), ezért a futó
  daemon még a régi binárist használja: az `integrity` mező a élő API-ban csak
  a következő kézi agent-telepítés után jelenik meg. A blokkot nem kerültem meg.
- Következő szelet: soccerway JS dayPicker (múltbeli nap fetch) vagy teljes
  (top-300-nál szélesebb) coverage-riport.

## S30c — 2006–2026 os gap-fill + eredményrevíziók

- [x] A `coverage --limit 5 / --competition` feltárta, hogy a korábbi gap-fill
  csak 1993–2005-öt pótolt: a v1-ben 2006–2026-ra ~40 ezer meccs volt, a v2-ben
  szinte semmi (pl. NB I 2020+ 3 meccs). A CLI most `--limit` (alap 300) és
  `--competition` szűrőt kapott, így a top-300 riport nem rejti el a réseket.
- [x] 11 verseny 2006–2026-os importja két körben (`--target-competition`-
  vel): NB I, Eliteserien, Ekstraklasa, Superliga, 2. Liga, Persha Liga,
  2. Bundesliga, Süper Lig, Primeira Liga, Serie B, Scottish Premiership.
  110 új csapat jóváhagyva (`approve-create-all`), 2 tartalékcsapat külön
  entitás maradt. Nettó **+31 835 meccs**, összesen **287 664**.
- [x] `dedupe fixtures`: 2 851 biztonságos merge + **32 kockázatos pár**,
  mindegyiket 4 párhuzamos kutató 2-2 forrással ellenőrizte — mind a 32-ben a
  **v1 eredmény volt a helyes**, az FD sor tévesen volt hozzárendelve. A
  kanonikus eredményt auditált revízióval javítottam, így a párok biztonságosan
  merge-elve lettek.
- [x] A reverify **115 withheld gate**-et talált (113 hiányzó eredmény, 4
  natural-key duplikátum, 2 anyagi konfliktus). 5 párhuzamos kutató ~9 perc
  alatt mind a 115-öt két forrással ellenőrizte: techinikai/walkower eredmények
  (Ankaraspor 34× 0-3, Obolon/Stal Alchevsk/Persha, FC Sopron, Reus Deportiu,
  Brescia, Salernitana, Magdeburg), 2 elhalasztott meccs (Chernihiv–Ahrobiznes,
  Hibernian–Dundee) `postponed`, 2 fordított hazai/vendég pár javítva.
  Minden módosítás `ops.audit_event`-tel rögzítve.
- [x] 266 konfliktus `resolved_manual` (web-igazolt vagy elavult, kanonikus
  érték megmaradt): 0 feloldatlan. A 43+43 orientációs konfliktus álhiba volt
  (a FD slug és a v1 `v1t-…` ID ugyanarra a kanonikus csapatra képez).
- [x] Végső kapu: `reverify 2006–2027` → **193 658 meccs, 0 hiba**;
  `integrity` → **0 duplikátum, 0 árva, 0 review, 0 withheld, 0 failed job,
  0 breaker, verdict OK**. `coverage 2006–2026 --limit 2000` → **189 310
  meccs, 96,7% eredmény, 86,1% odds**.
- [x] A soccerway bootstrap-timeoutokból nyitott breaker lezárva audittal
  (a job `cancelled`, további retry nem indokolt).
- Nyitott: a régi daemon-bináris miatt az API `integrity` mezője csak kézi
  agent-telepítés után él (lásd S30b); soccerway JS dayPicker; asset-licence.

## S30d — soccerway dayPicker-probe (backlog, nem szelet)

- [x] A mai napi oldal élő probe: a `www.soccerway.com` a `hu.soccerway.com`-ra
  irányít, a cookie-hözszólás elfogadása után sem jelenik meg
  `[data-testid=wcl-dayPickerButton]` és `div.event__match[data-event-row]`
  (25 s várakozás után sem). `TZ=UTC obscura fetch … --wait 25` 622 KB-os
  héjat ad, 0 `data-event-row`; a reggel 07:31-i sikeres fetch 1,79 MB volt
  (`ops.fetch_ledger`), tehát az oldal kiszolgálása idő/session-függő.
- [x] A `?date=` paraméter továbbra sem használható (dokumentált), a
  `/tegnap/` és `/matches/` útvonal nem ad match-sort. A dayPicker-alapú
  múltbeli nap fetch ezért **nem ellenőrizhető** jelenleg — backfill továbbra
  is a lokális v1 import útján megy. Nyitott marad, nem improvizáltam parsert
  nem látható DOM-ra.
- [x] Asset-licence: mind a 53 `ops.asset` sor `cleared` (49 csapatlogo + 4
  versenylogo), külön döntés nem kell.

## S30e — SwiftUI akadálymentességi szelet (P0, TDD)

- [x] Skill-/dokumentáció-audit: `development-readiness.md`, `references/roadmap.md`,
  `rust-backend.md`, `swiftui.md`; `AGENTS.md` §11 kijavítva; a fő skill 26,7 KB → 12,6 KB.
- [x] A katalógus community SwiftUI/Rust skilljeit a felhasználó `continue`-val zárta,
  **nem lettek telepítve** — nem kínáltam újra. A projekt-specifikus referenciák fedik a munkát.
- [x] TDD RED→GREEN: `AccessibilityLabelsTests` 6 teszt (státusz-, publikáció-, sor- és
  kártya-címke, hiányzó eredmény nem `0 – 0`, stabil identifier), közös, tesztelt forrás
  `FootballAppCore/AccessibilityLabels.swift`. Core **32/32**.
- [x] A címkéket használja a `MatchItemRow` (napi lista), `MatchRowView`, `StatusBadge`,
  liga-fejléc és üres állapot; `MatchListView`-be hiányzott az `import FootballAppCore` — javítva.
- [x] Billentyű-parancsok: Cmd+←/→ (előző/következő nap), Cmd+T (Ma), Cmd+R (frissítés),
  VoiceOver-címkék a navigációs ikonokon és a naptáron.
- [x] Valódi Xcode kapu (`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`):
  `xcodebuild test -project apps/FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS'`
  → **24/24, 0 failure**. `swiftc -parse` az appforráson: OK.
- [ ] **Nem ellenőrzött:** a tényleges VoiceOver-felolvasás és Tab-ciklus. Az app folyamat futott
  (pid 30733) és a System Events egy `FootballApp` ablakot jelzett, de a képernyőképen a chat
  alkalmazás maradt elöl → vizuális igazolás nincs. A teendőket az
  `docs/operations/ui-accessibility-checklist.md` rögzíti; ezért a SwiftUI DoD **még nem zárt**.
- Nyitott maradt: CI (`.github/workflows`), OpenAPI↔Swift kontrakt, futó daemon kézi frissítése,
  soccerway dayPicker, 2026/27 odds.

## S30f — Projekt-skillek és ágens-csapat (infrastruktúra, tudás)

Cél: a projekt tudása és a hozzá tartozó szerepek a repóban éljenek, ne egy másik harness
profiljában; utána jöhet a következő kód-szelet.

- [x] Megállapítva: a DSH a repó `.dsh/skills/` mappáját automatikusan, élőben katalógusba veszi
  (filesystem skill provider, rank 100). A projekt skillek eddig csak a hermes harness alatt
  (`~/.hermes/skills/`) léteztek, ezért voltak a DSH-ban láthatatlanok.
- [x] `.dsh/skills/football-app-v2/` átemelve (SKILL.md + 6 reference + `scripts/team_dup_groups.py`);
  a session katalógus azonnal jelezte.
- [x] Két új, projekt-specifikus skill: `swiftui-macos-client` (XcodeGen, `DEVELOPER_DIR`-es build/teszt
  parancsok, téma-tokenek, kötelező UI-állapotok, akadálymentességi szerződés, Swift 6) és
  `rust-workspace-backend` (crate-határok, thiserror/anyhow, sqlx, teszt-layout, ismert csapdák).
- [x] Bizonyíték a tudásra: `probe-rust` a meglévő referenciából 7/7 helyes választ adott
  fájlhivatkozásokkal; a baseline `probe-swift` csak checkpointokból tudott válaszolni (szétszórt
  tudás) → ezt a `swiftui-macos-client` zárta; a green `probe-swift2` mind a 6 kérdésre
  fájlhivatkozással, csak rövid ellenőrző olvasással válaszolt.
- [x] Ágens-csapat: `agents/README.md` (szereptábla, kötelező skillek, dispatch, handoff-formátum,
  topológiák, tiltások) + hat szerepkártya: `lead`, `rust-backend`, `swiftui-client`, `data-ops`,
  `researcher`, `verifier`. Alapelvek: builder ≠ verifier, diszjunkt írási hatókör,
  bizonyíték a futtatott kimenet.
- [x] `AGENTS.md`: §3 szerkezet-fa kiegészítve (`.dsh/skills/`, `agents/`), új §12 a skillekről,
  szerepekről és a handoffról; a skill nem írja felül az `AGENTS.md`-t.
- [x] `docs/operations/development-readiness.md`: a skill-fedettség hiány sora lezárva; community
  bundle telepítése nem szükséges.
- [x] Futtatott ellenőrzés: mindhárom skill YAML frontmatterje parse-olva, a kártyákban hivatkozott
  fájlok léteznek, `git status` átnézve (nincs titok, nincs generált fájl).
- [x] Független roster-próba (`probe-roster`, csak olvasás): a verifier-kártya + skill alapján
  helyesen adta vissza a szerep tiltásait, a teljes kapu-láncot, a handoff-mezőket és a
  bizonyíték-szabályt, és 7 apró következetlenséget jelzett. Javítva: a handoff egységes
  (`SZELET` + ZÖLD/PIROS, külön `KOCKÁZAT` és `VISSZAÁLLÁS`), a verifier diff-bázisa és teszt-DB
  határa tisztázva, a `references/rust-backend.md` kapu-lánca kiegészítve a hiányzó `source .env`
  lépéssel.
- [ ] Nem ellenőrzött: a DSH-n kívüli motorok `.dsh/skills` olvasása (a protokoll szerint bármelyik
  töltheti, de nem teszteltem); a szerepkártyák valódi több-ágenses futásban a következő szeletben
  bizonyítódnak.
- Következő döntési pont: a nyitott P0/P1 közül választani — VoiceOver/Tab kézi futás, CI,
  OpenAPI↔Swift kontrakt, vagy a futó daemon frissítése.

## S30g — VoiceOver/Tab kézi ellenőrzés: állapot és akadály

- Cél: a SwiftUI kliens kézi akadálymentességi/viselkedési ellenőrzése (a
  `docs/operations/ui-accessibility-checklist.md` 2–6. pontja), hogy a fázis DoD zárható legyen.
- [x] Daemon- és kliensállapot rögzítve: `GET /v1/health` → `{"status":"ok","service":"footballd"}`
  (port 49387), `FootballApp` pid 30733.
- [x] Képernyő-olvasás (screencapture) működik: 1920×1080 PNG, 2 114 749 bájt; a korábbi
  `could not create image from display` hiba megszűnt.
- [x] A DSH oldalsáv `Cannot read properties of undefined (reading 'map')` hibája javítva
  (`dsh-better-sidebar` 0.32.1); a Tasks oldal a csapat-táblával hiba nélkül renderel (képernyőkép).
- [x] Felhasználói visszajelzés (m00389): az app fut, a kezdőlapon adatok vannak, hibát nem lát;
  a 2–6. pontot még nem próbálta.
- [ ] VoiceOver-felolvasás, Tab-ciklus, Cmd+←/→/T/R, világos/sötét, Reduced Motion, kontraszt,
  állapotok, magyar visszaolvasás: **NEM igazolt**.
- Akadály: az AX-vezérléshez (System Events, kattintás, billentyű) a `DeepSeek Harness`
  Accessibility-kapcsolója kell a System Settings „Device Control and Data Access” panelen;
  amíg ki van kapcsolva, `osascript` → `not allowed assistive access. (-1728)`.
- Döntés: a kézi ellenőrzés kiváltására S30h szeletben automatizált XCUITest-harness épül
  (VoiceOver által használt címkék olvasása, billentyűparancsok, determinisztikus
  betöltés/üres/hiba/offline állapotok fixture `runtime.json`-nal). A VoiceOver tényleges
  beszédhangja továbbra is emberi ellenőrzés marad.
- Futtatott kapu (2026-11-20 11:52): **GATE RESULT: GREEN** a `/tmp/gate-s30g.sh` lánccal:
  db-reset (test DB), `check-foundation.sh`, `cargo test --workspace -- --test-threads=1`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`,
  `cargo build -p footballd -p footballctl --features obscura`, Swift Core **32/32**,
  Xcode app **24/24**, záró db-reset.
- Következő döntési pont: S30h (automatizált UI-harness); ehhez egyetlen emberi lépés az
  AX-kapcsoló bekapcsolása, utána a harness önállóan fut.

## S30h — Automatizált UI-ellenőrzés (XCUITest harness)

- Cél: a `docs/operations/ui-accessibility-checklist.md` 2–6. pontjának kiváltása ismételhető,
  emberi kézi ellenőrzés nélküli automatizált UI-teszttel, hogy a SwiftUI fázis DoD zárható legyen.
- [x] Új `FootballAppUITests` cél (`bundle.ui-testing`, `TEST_TARGET_NAME: FootballApp`) a
  `apps/project.yml`-ban és a séma teszt-céljai között; `xcodegen generate` zöld.
- [x] `FOOTBALL_APP_RUNTIME_FILE` környezeti felülírás a `RuntimeInfoLoader`-ben
  (`resolveDefaultURL(environment:)`), hogy a teszt determinisztikus `runtime.json`-t adjon az appnak.
- [x] `scripts/ui-fixture-server.py` + `scripts/run-ui-tests.sh` + `tests/fixtures/ui/{loaded-day,empty-day}.json`:
  a fixture-szerver külön folyamat (a UI-test runner sandboxja `socket()` → EPERM miatt nem nyithat
  socketet), a runner a saját temp könyvtárába írja a `runtime.json`-t.
- [x] `AccessibilityLabels.matchCardLabel` megkapja a `publishState` paramétert, és kimondja a
  publikációs minőségi állapotot; `MatchCardModel.publishState` hozzáadva (DayMatch-ból, mintaadatnál
  `.unknown`); a `MatchListView` átadja. Unit-tesztek frissítve.
- [x] Futott UI-tesztek (2026-11-20 12:09): `** TEST SUCCEEDED **`, **4/4 zöld** —
  betöltött nap címkéi (csapatok, idő, státusz, eredmény, publikációs állapot), üres nap,
  daemon-offline hiba, billentyűparancsok (Cmd+→ / Cmd+T / Cmd+← valódi napi váltást okoz).
- [x] Swift Core tesztek: **34/34 zöld** (új `RuntimeInfoLoaderTests` esetekkel).
- [x] Vizuális igazolás harness oldalról: `CGWindowList` + `screencapture -l<windowID>` működik
  (a futó app ablaka elmentve, kezdőlap valós adattal: 41 meccs, Ligue 1/Bundesliga/La Liga).
- [x] AX-vezérlés igazolva: `scripts/ax-press.swift FootballApp "Következő nap"` → `AXPress result: 0`,
  a fejléc „november 21.” / „HOLNAP” / „90 meccs” lett; az AX-fa (gombok, kártyacímkék) olvasható.
  A felhasználó kérése (m00389: „tudnad az appot vezérelni látni használni”) ezzel teljesült.
- [ ] Nem ellenőrzött: a teszt-runnerből készített screenshot TCC miatt nem működik
  („Failed to create screenshot”); a tényleges VoiceOver-hang, Tab-fókusz és fókuszcsapda,
  világos/sötét + Reduced Motion + kontraszt, részleges adat állapot továbbra is nyitott.
- Tanulságok (dokumentálva a checklistben): a `CODE_SIGNING_ALLOWED=NO` megöli a UI-test runnert;
  a SwiftUI `Text` AX-ban üres `label`, a szöveg a `value` attribútumban van (`value CONTAINS`).
- Futtatott kapu: lásd a szelet záró ellenőrzését (Rust kapu-lánc + Core 34/34 + app 24/24 + UI 4/4).
- Következő döntési pont: a fennmaradó akadálymentességi pontok automatizálása (megjelenés/fókusz),
  CI workflow, OpenAPI↔Swift kontrakt-teszt, vagy a futó daemon frissítése.

## S31a — a fennmaradó akadálymentességi pontok automatizálása

Induló állapot: az S30h harness 4 UI-tesztje zöld; nyitott a részleges adat, a megjelenés/kontraszt
és a Tab-fókusz pont.

- [x] Részleges adat fixture (`tests/fixtures/ui/partial-day.json`) és új UI-teszt
  (`testPartialDataSpeaksStatusScoreAndPublishState`): élő/elhalasztott/visszatartott/karanténos
  állapotokat és a hiányzó eredmény kimondását ellenőrzi. Futott: `TEST SUCCEEDED`, 5/5 UI-teszt zöld.
- [x] Kontraszt-teszt (`apps/FootballAppTests/ThemeContrastTests.swift`): WCAG 2.1 arány minden
  szöveg-tokenre mindhárom felületen + OLED-fekete háttér. Futott: 26/26 app unit teszt zöld.
- [x] `scripts/ax-focus.sh` elkészült (Tab-lépkedés + AX-fókusz kiírás); eredmény: az
  `AXFocusedUIElement` az app processzén mindig `AXWindow`, Tab/`AXPress`/`AppleKeyboardUIMode 3`
  mellett sem változik → a Tab-fókusz automatizálása app-oldali fókuszállítás nélkül nem mérhető.
- [ ] Nem ellenőrzött: tényleges VoiceOver-hang, Tab-fókusz láthatósága/fókuszcsapda, Reduced Motion,
  Cmd+R vizuális frissítés. (A megjelenés/kontraszt pontot a fenti teszt fedi.)
- Következő döntési pont: a felhasználó kérésére a 4 irány sorrendben — következik a CI workflow,
  majd az OpenAPI↔Swift kontrakt-teszt, végül a futó daemon/kliens frissítése.

## S31b — CI workflow

Induló állapot: a kapu-lánc csak helyben futott, `.github/workflows` nem létezett (spec 0. fázis nyitott).

- [x] `.github/workflows/ci.yml`: Rust feladat (postgres:18 szerviz, migrációk dev+test, check-foundation,
  teljes teszt, clippy, obscura build) és Swift feladat (xcodegen, Core `swift test`, app unit tesztek).
- [x] `actionlint -no-color .github/workflows/ci.yml` → exit 0 (a shellcheck-jelzést `find`-re cseréléssel javítottuk).
- [x] A Swift feladat parancsai helyben lefuttatva: Core 34/34, app 26/26 zöld, `TEST SUCCEEDED`.
- [ ] Nem ellenőrzött: az első tényleges GitHub Actions futás (a repó távoli pipeline-ja még nem futott).
- Következő döntési pont: OpenAPI↔Swift kontrakt-teszt (S31c).

## S31c — OpenAPI↔Swift kontrakt-teszt

Induló állapot: semmilyen gépi kontrakt nem kötötte össze az API-t és a Swift DTO-kat.

- [x] `tests/fixtures/contract/day-response.json`: a `GET /v1/day/{date}` válasz kanonikus payloadja
  (teljes és minimális meccs, blockers, odds, publish_state, időbélyegek).
- [x] `rust/crates/api/tests/day_contract.rs`: a `DayResponse` szerializált JSON-ja pontosan a fixture
  (include_str!, hogy hiányzó fájl fordítási hiba legyen). Futott: 1/1 zöld.
- [x] `apps/FootballAppCore/Tests/FootballAppCoreTests/DayContractTests.swift`: ugyanazt a fájlt
  dekódolja a Swift DTO-kkal, mezőnként ellenőriz (3 teszt). Futott: Core 37/37 zöld.
- [x] `cargo fmt --all --check` és `cargo clippy -p football-api --all-targets -- -D warnings`: tiszta.
- [ ] Nem ellenőrzött: teljes OpenAPI-dokumentum minden végpontra (generálási mód: utoipa vs. kézi —
  külön döntés), és a CI első valós futása.
- Következő döntési pont: a futó daemon/kliens frissítése új buildre (S31d).

## S31d — a futó daemon és kliens frissítése

Induló állapot: a launchd daemon a korábbi buildet futtatta (pid 962, port 49387), a kliens a
`/private/tmp/footballapp-xcode-fresh` alatti régi buildből indult, ezért a kártyacímkékből hiányzott
a publikációs állapot.

- [x] `cargo build -p footballd -p footballctl --features obscura` (12,3 s).
- [x] `./scripts/install-footballd-agent.sh run` → „agent telepítve és elindítva”; a telepített
  `agent/bin/footballd` sha1-je azonos a repó buildjével (`121af2d0…`), pid 76003, `state = running`.
- [x] Új port a logból: **62923**; `runtime.json` frissült; `GET /v1/health` → `{"status":"ok","service":"footballd"}`;
  `GET /v1/day/2026-11-20` → 41 meccs.
- [x] `footballctl integrity` → `OK` (0 duplikált, 0 árva provenance, 0 függő review, 0 withheld, 0 failed job).
- [x] Kliens újraépítve (`** BUILD SUCCEEDED **`), a régi példány kilépett, az új elindult; a kezdőlap
  41 meccs / 4 kiemelt, és a kártyacímkék már kimondják a publikációs állapotot
  (pl. „…, Ütemezett, eredmény még nincs, Ellenőrzött publikáció”).
- [x] Teljes kapu (`/tmp/gate-s31.sh` → `/tmp/s31-gate.log`): `GATE RESULT: GREEN`; Rust 415 teszt / 0 hiba,
  Swift Core 37/37, app 26/26, UI 5/5, clippy `-D warnings` és `fmt --check` tiszta.
- [ ] Nem ellenőrzött: a GitHub Actions első valós futása (a workflow lokálisan, actionlinttel és a lépések
  helyi futtatásával ellenőrzött, de a repóban még nem futott le).

## S32a — billentyűzet-fókusz a nap listáján

Induló állapot: az appban sehol nem volt `.focusable()` / `@FocusState`; a Tab-fókusz harness oldali
mérése (S31a) azért nem látott fókuszt, mert nem volt fókuszálható elem. A felhasználó döntése
(m01055): a valódi VoiceOver-felolvasás kimarad, a többi irány mehet sorban.

- [x] `apps/FootballApp/KeyboardNavigation.swift`: `KeyboardMonitor` (`NSEvent` lokális keyDown-monitor,
  csak a főablakban aktiválódik, nyitott sheet felett nem nyeli el a billentyűket).
- [x] `MatchListView`: `@FocusState` + Tab/Shift+Tab (körbeérő), ↓/↑ (fókusz után), Return/Enter
  (részletező), Escape (fókusz törlése); a fókuszált kártya 2 pt borostyán keretet kap
  (`focusEffectDisabled()` mellett), a listán belül középre gördül (Reduced Motion esetén azonnal).
- [x] A fókuszált kártya `selected` AX-traitet kap — ezt hirdeti a VoiceOver, és az XCUITest ebből tudja
  ellenőrizni a fókuszt (`hasFocus` a kombinált AX-csoporton nem elérhető).
- [x] Új UI-teszt `testKeyboardFocusWalksMatchCardsAndOpensDetail` (Tab → 1. kártya, ↓ → 2., ↑ → 1.,
  Return → `match.detail`, Escape → zár, Shift+Tab → 2.). Futott: **6/6 UI-teszt zöld**
  (`./scripts/run-ui-tests.sh` → `** TEST SUCCEEDED **`).
- [x] Élő igazolás a futó appon: `/tmp/s32a-ring.png` (keret a Toulouse–Marseille kártyán),
  `/tmp/s32a-sheet.png` (Return megnyitotta a részletezőt), `/tmp/s32a-after-escape.png`
  (Escape bezárta, a keret a kártyán maradt).
- [x] Tanulságok rögzítve: a nyilak/Tab `.function` modifierje; a sheet ablakának `attachedSheet == nil`
  csapdája (`sheetParent == nil` kell); `NSEvent` nem `Sendable` (csak keyCode + flags mehet át).
- [ ] Nem ellenőrzött: `shift+Tab` körbeérése csak UI-tesztben futott (élőben nem), és a fókuszgyűrű
  megjelenése világos/sötét témában (a kliens szándékosan csak sötét palettát használ).
- Következő döntési pont: push + a CI első valós futása (S32b).

## S32b — a CI első valós futása

Induló állapot: `06da09a` push után elindult az első valós GitHub Actions futás; a workflow eddig
csak actionlinttel és helyi lépésfuttatással volt ellenőrizve.

- [x] Run `44730897063`: a **Swift feladat zöld** (Core tesztek, `xcodegen`, app unit tesztek) — ez az
  első valós CI-siker a kliens oldalon.
- [x] Ugyanabban a futásban a **Rust feladat piros** lett a `sqlx-cli` telepítésénél:
  `error: invalid value '0.8' for '--version <VERSION>': unexpected end of input while parsing minor
  version number` (a cargo a `0.8`-at nem fogadja el, csak `^0.8`-at).
- [x] `8dc5a97` javítás: `cargo install sqlx-cli --version ^0.8 --no-default-features --features
  rustls,postgres --locked`, és a `Swatinem/rust-cache` `workspaces: rust -> target` beállítása (a
  workspace a `rust/` alatt van, ezért a cache eddig nem találta a `Cargo.toml`-t).
  `actionlint -no-color .github/workflows/ci.yml` → exit 0.
- [x] `77c3138` javítás: a nap-lista két rendezése `($0.time, $0.id) < ($1.time, $1.id)` tiebreak-et
  kapott, mert a Swift rendezés nem stabil, és az azonos kezdésű meccsek sorrendje UI-teszt-flake-et
  okozott. Újrafuttatott UI-tesztek: **6/6 zöld** (`** TEST SUCCEEDED **`).
- [x] Push `06da09a..77c3138`; a második futás (`44731298004`) Swift feladata zöld, a Rust feladat
  viszont a storage-tesztekben elhasalt (lásd S32c).
- [x] Harmadik futás (`44733358476`, head `aed79c9`): **Rust és Swift feladat is zöld**, `status:
  completed, conclusion: success` — ez a pipeline első teljes zöld futása (a `44731298004` Rust
  feladata a S32c időbélyeg-javítása előtt hasalt el).

## S32c — időbélyeg-pontosság a storage-tesztekben

Induló állapot: a második CI-futás Rust feladata 6 storage-tesztet buktatott el:
`left: Some(2026-11-20T11:55:44.689002Z)` vs `right: Some(...689002024Z)`.

- [x] Ok: a PostgreSQL `timestamptz` **mikroszekundum** felbontású, a `chrono::Utc::now()` viszont
  nanoszekundumot ad; a `trunc_subsecs` nélküli érték a DB-n át kerekítve jön vissza. macOS-en az óra
  gyakorlatilag µs-felbontású, ezért helyben zöld volt, a Linux-os CI nanoszekundummal buktatta el.
- [x] Javítás: `rust/crates/storage/tests/worker_repos.rs` mind a 13 `let now = Utc::now();` sora
  `let now = Utc::now().trunc_subsecs(6);`, import: `use chrono::{Duration, SubsecRound, Utc};`.
- [x] Átnézve a többi pontos időbélyeg-egyenlőség: `rust/crates/worker/tests/scheduler.rs:173`
  `base_time()`-ja egész másodperc (`.nanosecond() == 0`), ezért érintetlen; más ilyen assertion nincs.
- [x] `./scripts/db-reset.sh` (csak `TEST_DATABASE_URL`, 13 migráció) exit 0 → `./scripts/check-foundation.sh`
  exit 0: **114 domain+storage teszt zöld**, `clippy -D warnings` és `fmt --check` tiszta.
- [x] `cargo build -p footballd -p footballctl --features obscura` → `Finished ... in 13.41s`.
- [x] Teljes munkaterület: `cargo test --workspace -- --test-threads=1` → **307 teszt, 0 hiba**
  (`/tmp/ws-tests.log`); `cargo test -p football-api --test openapi` → 4/4.
- [x] A javított tesztek Linuxon is zöldek: a `44733358476` futás Rust feladata success (a
  `record_auth_failure…`, `robots_cache…`, `breaker…`, `ledger…` teszteket is beleértve).

## S33 — teljes OpenAPI-dokumentum

Induló állapot: a nap-végpont kontraktja kész volt (S31c), de teljes OpenAPI-dokumentum nem létezett,
és a generálás módja nyitott döntés volt.

- [x] Döntés: **utoipa 6** derive-alapú dokumentum a `GET /openapi.json` útvonalon, token nélkül
  (séma, nem adat); nincs generált artifact a repóban. Indoklás és következmények:
  `docs/decisions/2026-11-20-openapi.md`.
- [x] `rust-version` 1.85 → **1.88** (az utoipa 6 MSRV-je); a `utoipa` workspace-függőség a
  `domain`/`storage`/`api` crate-ekben (a domain csak fordítás idejű séma-annotációt kap, I/O-t nem).
- [x] `#[utoipa::path(...)]` mind a 11 handleren (tagek: health/nap/meccs/verseny/csapat/asset/ops;
  `security(("bearer" = []))` a health kivételével), `ToSchema` a válasz- és sor-típusokon.
  A `serde_json::Value` mezők `#[schema(value_type = ...)]`-ot kaptak (Finding/Severity/OddsValues),
  így nem maradt üres `{}` séma.
- [x] `rust/crates/api/src/openapi.rs`: `ApiDoc` (info, 11 teljes modulútvonalú path, components,
  `BearerAuth` modifier), `document()` handler; `lib.rs`-ben `pub mod openapi;` és a route.
- [x] Tesztek: `cargo test -p football-api --lib openapi` → 2/2; `rust/crates/api/tests/openapi.rs`
  4/4 (minden dokumentált útvonal routol negatív kontrollal, minden `$ref` feloldható, a nap-séma
  mezői pontosan a `tests/fixtures/contract/day-response.json` kulcsai, `GET /openapi.json` token
  nélkül 200).
- [x] A munka közben a clippy 1.99 `collapsible_if` lintjei miatt let-chain átírások történtek
  (`domain/match_state.rs`, `identity/linker.rs`, `ingest/demo.rs`, `ingest/http.rs`,
  `worker/scheduler.rs`, `worker/tick.rs`, `bins/footballctl/main.rs`); viselkedés nem változott,
  a teljes tesztkészlet zöld.
- [x] README: új „## OpenAPI” szakasz és frissített állapotsor; döntési dokumentum a `docs/decisions/` alatt.
- [ ] Nem ellenőrzött: a **futó** daemon még a korábbi buildet futtatja, ezért az élő `GET /openapi.json`
  válasz ebből a buildből nem lett ellenőrizve; a daemon újratelepítése külön döntés.
- Következő döntési pont: push után a zöld CI-futás igazolása, majd Tab-fókusz app-oldali mérés.

## S34 — app-szintű Tab-sorrend (2026-11-20)

Induló állapot: az S33 zöld CI-vel lezárult, a következő nyitott pont a Tab-fókusz volt. Az S32a
fókuszmodellje mérten **billentyűzet-csapdát** okozott: a `KeyboardMonitor` minden Tabot elfogyasztott,
így Escape után a következő Tab visszavitte a fókuszt az első kártyára, a fejléc/szűrők/dátum-nyilak
billentyűzettel elérhetetlenek voltak (mérés: `/tmp/s34-tab-after-escape.png`). A felhasználó a
bemutatott bizonyíték alapján a **B — app-szintű Tab-sorrend** irányt választotta.

- [x] Új `apps/FootballApp/AppFocus.swift`: `AppFocus` állomások, körbeérő `focusedTarget`, fókuszgyűrű
  és AX `selected` trait segédek.
- [x] `apps/FootballApp/HomeView.swift`: a billentyű-monitor a gyökérre került, `focus`/`showCalendar`
  állapot, `matchIDs`, `focusOrder`, `handle`/`stepMatch`/`activate`; felső navigáció, fejléc-gombok
  és szűrők azonosító + fókuszgyűrűvel; naptár-popover nyitva nem fogyaszt billentyűt.
- [x] `apps/FootballApp/MatchListView.swift`: a lista csak `focusedMatch` azonosítót kap, saját
  billentyűkezelés nélkül; `TeamSheet`/`CompetitionSheet` hívása `focusedMatch: nil`.
- [x] UI-tesztek: frissített `testKeyboardFocusWalksMatchCardsAndOpensDetail` (15. Tab = első kártya)
  és új `testKeyboardTabOrderCoversHeaderFiltersAndList` (navigáció → fejléc → szűrők → kártya →
  Escape → Shift+Tab az utolsó kártyára).
- [x] Futott: `./scripts/run-ui-tests.sh` → **26/26 app unit, 7/7 UI teszt, 0 failure**, `** TEST SUCCEEDED **` (UI 45,161 s).
- [x] Élő ellenőrzés a futó builddel fixture szerveren: `/tmp/s34-v2-tab1.png`, `/tmp/s34-v2-tab9.png`,
  `/tmp/s34-v2-tab14.png`, `/tmp/s34-v2-return.png`, `/tmp/s34-v2-shifttab2.png` — a csapda megszűnt.
- [x] Dokumentáció: `docs/operations/ui-accessibility-checklist.md` új „S34” szakasszal (sorrend,
  billentyűk, AX-jelzés, dokumentált korlátok); README állapotsor frissítve.
- [ ] Nem ellenőrzött: a saját Tab-körön kívül maradt natív ország/osztály `Menu`, a naptár-popover
  tartalma és a részletező sheet vezérlői (VoiceOverrel/egérrel elérhetők, külön szelet); a valódi
  VoiceOver-felolvasás emberi fület igényel.
- [x] Push `1e8edf4..a19ded3`; CI-futás **44736745275**: „Rust (domain, storage, API, worker)” success, „Swift kliens (Core + app unit tesztek)” success.
- Következő döntési pont: a soccerway JS dayPicker (továbbra is ellenőrizhetetlen a forrás oldalán)
  és a Tab-kör kiterjesztése a kimaradt vezérlőkre (natív menük, naptár-popover, részletező sheet).

## S35 — helyi AI-réteg (Magnitude gemma 4 + helyi SearXNG)

- Kiindulás: a felhasználó leállította a billentyűzet-fókuszes munkát, és a források / napi
  működés, illetve a shared task (`task-1`: helyi modell bekötése háttérfeladatokhoz,
  webes kereséssel) felé irányított. A gép helyi stackje (Magnitude 0.2.17 a
  `127.0.0.1:10100/inference/v1` címen gemma 4 E4B Artemis modellel, SearXNG a
  `127.0.0.1:8888` címen) élőben ellenőrizve: chat-completion és tool-call is működik,
  a SearXNG JSON-t ad, külső kulcs nélkül.
- [x] Új crate `rust/crates/ai` (`football-ai`): `config` (env + defaultok),
  `searxng` (JSON keresés), `magnitude` (OpenAI-kompatibilis chat kliens),
  `agent` (tool-call hurok: `web_search` → SearXNG → `role: tool` üzenet → végleges
  válasz, források deduplikálva); `AiError` tipizált hibák.
- [x] `bins/footballctl`: `ai search` és `ai ask` alparancs, a DB-pool létrehozása
  **előtti** dispatch-csel (a réteg nem igényel adatbázist); `football-ai` path-függőség.
- [x] Tesztek: `cargo test -p football-ai` → **3 config unit + 3 wiremock integráció
  zöld** (SearXNG mapping/limit, üres kifejezés = nincs hálózat, agent hurok forrásokkal).
- [x] Élő smoke: `footballctl ai search "Debrecen Paks mérkőzés" --limit 3` → 3 valódi
  találat; `footballctl ai ask "…következő mérkőzés…" --max-steps 2` → gemma 4 válasz
  ~6,4 s alatt, 1 tool-lépés, 6 találat; a modell kimondta, hogy nem találta meg a
  dátumot (nem hallucinált).
- [x] Dokumentáció: `docs/decisions/2026-11-20-local-ai.md` (döntés, miért külön crate,
  miért nincs kliens-oldali integráció, korlátok), README „Helyi AI (S35)” szakasz és
  állapotsor, `.env.example` AI-változók (kommentelve).
- [ ] Nem ellenőrzött: a worker-oldali háttér-job bekötése (napi forrás-összefoglaló)
  még nem készült el; a kis E4B modell összetett több-lépéses kérdésnél bizonytalan
  választ adhat (a hurok 4 lépésre / 700 tokenre korlátoz).
- Következő döntési pont: a helyi AI-t a napi üzemeltetésbe kötni (pl. reggeli
  forrás-egészség összefoglaló a worker tickből), vagy előbb a forrás-frissítési
  policy-k (napi egyszeri discovery helyett napon belüli frissítés) javítása.

## S36a — Tipplap liga-lefedettség bővítése (napi odds)

- Kiindulás: a felhasználó a források/napi működés felé irányított; mérés szerint a nap
  124 v2-meccséből csak 4 kapott oddsot, mert a Tipplap-adapter 16 ligacímet ismert
  (`rows=118 skipped=114`), a v1-ben viszont napi 100–220 Tipplap-esemény van.
- [x] Döntés rögzítve: `docs/decisions/2026-11-20-tipplap-league-coverage.md`
  (adatvezérelt cím-bővítés ≥2 szavazat + ≥80% többség; GB→„Scotland” név kezelése;
  nincs deployment ebben a szeletben).
- [x] `rust/crates/ingest/src/tipplap.rs`: `GENERAL_LEAGUES` (26 új cím) + `league_for`
  fallback ékezet/kisbetű-toleráns pontos egyezéssel; ismeretlen cím továbbra is kimarad.
- [x] Tesztek: `cargo test -p football-ingest --lib tipplap` → 2 zöld;
  `cargo test -p football-ingest --test tipplap` → 5 zöld (új integrációs teszt
  „Angol Liga 1.” szakasszal, League One/GB tiert3 + odds-observáció).
- [x] Élő parse a daemon által mentett napi oldalon (629 961 byte, 60 ligaszakasz):
  új kóddal `match_core=44, odds=44, competitions=19`; régi kóddal ugyanaznap
  `rows=118 skipped=114` → 4 meccs. A 44-ből 24 csapatpár egyezik a nap v2-meccseivel.
- [ ] Nem ellenőrzött: a tényleges betöltés az új binárison (a launchd-daemon a régi
  binárist futtatja; újratelepítés külön döntés), és a csapat-identitás feloldás
  valós aránya a 24-ből.
- Következő döntési pont: (a) daemon újraépítése + újratelepítése, hogy a lefedettség
  élesben is megjelenjen, (b) napon belüli frissítés policy-k (`offset_minutes` események
  a kickoffhoz), (c) csapat-alias/identitás lefedettség.

## S36a — daemon újratelepítés és éles mérés (2026-11-20 14:2x)

- Felhasználói döntés: „Telepítsd újra a daemont most” — `scripts/install-footballd-agent.sh`
  lefutott: új bináris (`cargo build -p footballd --features obscura`), run.sh/plist újraírva,
  `launchctl kickstart` → az új worker 2026-11-20T13:21:25Z-kor indult (port 64087);
  a run.sh most a Keychainből olvassa az OpenFoot-kulcsot, nem fixen a fájlból.
- Kényszerített Tipplap-discovery (kézi `ops.job`, `entity_key=all-manual-s36a-2`) a daemon
  dolgozta fel 60 s alatt: log `tipplap nap parse rows=118 skipped=74` (előtte `skipped=114`),
  majd `napi versenyek materializálva competitions=19`.
- [x] Éles mérés a mai napra: oddsos v2-meccs **4 → 29** (forrás: tipplap, 12 → 87 quote),
  19 verseny; a 44 parsolt meccsből 29 materializálódott.
- [x] `identity.review_queue` 17 új nyitott sor (főleg Cymru Premier: fuzzy_candidate /
  no_exact_match) — a maradék ~15 meccs a csapat-identitás feloldásán múlik, nem a liga-fedésen.
- Következő döntési pont: a 17 review rendezése (kurált alias-seed a v1 adatokból), illetve a
  napon belüli frissítés policy-k (kickoff előtti `offset_minutes` események).

## S36b — napon belüli frissítés (intraday refresh slotok)

- Kiindulás: minden engedélyezett forrásnak egyetlen napi `discovery` eseménye volt
  (1440 perc), ezért a nap folyamán nem frissült sem az odds, sem a meccsállapot;
  a felhasználó külön engedélyezte a napon belüli frissítést.
- [x] Scheduler: a `supported_interval` mostantól a napi (1440) és heti (10080) mellett
  a napot egyenlő sávokra osztó intervalt is engedi (30..1439, ha osztja az 1440-et);
  a napon belüli ág sávonként egy jobot generál (`slot-NN`, `due_at` = sáv kezdete,
  prioritás = esemény-alap + sávindex), a jövőbeli sávok pendingben várnak.
- [x] Tesztek: `cargo test -p football-worker --test scheduler -- --test-threads=1` → 6 zöld
  (új: `subdaily_interval_generates_slot_jobs_with_due_times`); `--test scheduler_validation`
  → 4 zöld (új: `subdaily_interval_generates_one_job_per_slot`).
- [x] Dev DB policy: tipplap `[discovery 1440, refresh 360]` → napi 5 kérés a 6-os
  keretből; openfoot (keret elfogyott), footballdata (napi CSV), soccerway (dayPicker
  ellenőrizhetetlen), fotmob/demo (kikapcsolva) szándékosan nem kapott sávot.
- [x] Döntés: `docs/decisions/2026-11-20-intraday-refresh.md`; üzemeltetési leírás:
  `docs/operations/tipplap.md` (policy + S36a szótár + S36b mérés).
- [x] Daemon újratelepítés (felhasználói döntés: „Telepítsd újra most”) —
  `scripts/install-footballd-agent.sh` lefutott; az új worker első tickje 4 sáv-jobot
  generált (slot-00…slot-03), a slot-00 lefutott (processed=1, 18 verseny materializálva),
  a slot-01/02 `quota_blocked` lett a napközbeni kézi kérések miatt.
- [x] Ezért a tipplap `daily_budget` 6 → 8 (1 discovery + 4 refresh + 1 robots + 2 tartalék);
  a döntés-dokumentum és az üzemeltetési leírás frissítve.
- [ ] Nem ellenőrzött: teljes napos sáv-futás (a holnapi napon mérhető, hogy mind a 4 sáv
  lefut-e a kereten belül), és a csapat-identitás review-k hatása az odds-lefedettségre.

## S37 — forrás-összehangolás: v1-referencia felmérése és az első javítás

- Kiindulás (felhasználói kérés): a v1-ben a napi meccslista és az odds-párosítás
  jól működött; a v2-ben a forrásokat össze kell hangolni, hogy minden nap minden
  liga/osztály (férfi/női/ifjúsági) meccse meglegyen, ellenőrizhetően, hitelesen
  és duplikációmentesen. A felmérés + terv készült el, majd az első konkrét javítás.
- [x] v1 referencia felmérése (read-only): `football_app` DB és repo; napi
  lánc (`scrappers/tipplap/daily_update.py` 474. sor sorrendje), meccs-szintű
  linkelés (`link_events.py`), odds-párosítás, duplikáció-őrök, identitás-rétegek
  és az `automation_decisions` ledger. v1 mai nap: 163 kanonikus meccs, ebből
  109 oddsos (67 %), nap-lista 201 sor, 0 napi duplikátum.
- [x] v2 mért hiányok: 131 mai meccs, 29 oddsos (22 %); meccs-szintű forrás-híd
  nincs (0 sor); 8 valódi közeli duplikátum-pár a mai napon (pl. SV Darmstadt 98 /
  Darmstadt, Llandudno FC / Llandudno, TNS / The New Saints, FC Gagra / Gagra);
  a beépített dedupe csak 1 pontos-kulcsú csoportot talál; soccerway: 252 parse-olt
  meccsből 47 van a listán; tipplap→meccs linkelés nincs (ezért 29 vs 109 odds).
- [x] Duplikált ország-rekordok a `core.country`-ban: FR/FRA, DE/GER, HU/HUN,
  IT/ITA, ES/ESP, GB/GB-SCT — a v1-import ISO3, az új források ISO2 kódot hoznak.
- [x] Terv: `docs/superpowers/plans/2026-11-20-forras-osszehangolas.md` — S37a–S37g
  szeletek, mért kiindulással, elfogadási feltételekkel.
- [x] **S37a javítás**: `football_domain::identity::normalize_country_code` mostantól
  ISO3 → ISO2 alias-táblát alkalmaz (FRA→FR, GER→DE, HUN→HU, ITA→IT, ESP→ES,
  NED→NL, POR→PT, …), a GB-alrégió szabály változatlan; az ENG szándékosan külön
  marad. Új domain-teszt: `iso3_country_codes_are_equivalent_to_iso2`.
- [x] Kapuk: `cargo fmt --all --check` OK; `cargo clippy --workspace --all-targets
  -- -D warnings` tiszta; `./scripts/db-reset.sh` (15 migráció) + `check-foundation`
  exit 0 (33 „test result: ok”); `cargo test --workspace -- --test-threads=1` →
  lásd a záró jelentést (S37a futtatás).
- [ ] Nem ellenőrzött: a S37a éles hatása (a daemon új binárison fut-e); a
  `core.country` duplikált sorainak összevonása adatművelet — külön döntés; a
  soccerway-veszteség (H4) diagnózisa és a tipplap-linkelés (S37b) még nyitott.

## S37b — meccs-szintű forrás-híd és név-variáns linkelés

- Kiindulás: a v2-ben a források csak csapat-szintű híddal rendelkeztek; ugyanaz
  a meccs két forrásból külön kanonikus sorként jött létre, ha a csapatnév
  variáns (pl. `SV Darmstadt 98` / `Darmstadt`), ezért a tipplap oddsai sem
  találtak a meccshez (29/131 odds a v1 109/163-ával szemben).
- [x] `identity.source_entity_ref` mostantól **meccs-szintű** hídat is ír
  (`entity_kind='match'`, `source_entity_id` = a forrás meccs-azonosítója,
  `method='exact_external_id'`, név-variáns linknél `confidence=0.9`).
- [x] Név-variáns meccs-link a kanonikalizálóban
  (`rust/crates/ingest/src/canonicalize.rs`): a natural-key keresés után, azonos
  verseny + ±12 órás kickoff-ablakon belül csapatnév-hasonlítás
  (`football_domain::identity::names_likely_same`); pontosan egy jelöltnél a
  meglévő meccshez linkel (a kanonikus sor adatai érintetlenek, az odds és a
  forrás-híd oda kerül), több jelöltnél a legkorábbihoz (a merge az S37c).
- [x] Új domain-segédfüggvények: `name_tokens` (klub-szuffixek levágása:
  fc/cf/sc/afc/club/cd/ud/sv/ac/…), `names_likely_same` (foldolt egyezés,
  token-subset, monogram, egy elgépelés hosszú tokenen).
- [x] Tesztek: `rust/crates/domain/tests/identity_rules.rs`
  `club_name_variants_are_likely_the_same_team` (8 pozitív/negatív eset);
  `rust/crates/ingest/tests/live_materialize.rs`
  `variant_team_names_link_to_the_existing_canonical_match` (két forrás,
  `Újpest` / `Újpest FC`: 1 kanonikus meccs, 2 meccs-híd, az odds a meglévőhöz).
- [x] Kapuk: `cargo fmt --all --check` OK; `cargo clippy --workspace
  --all-targets -- -D warnings` tiszta; `./scripts/db-reset.sh` (15 migráció) +
  `./scripts/check-foundation.sh` exit 0 (33 „test result: ok”);
  `cargo test --workspace -- --test-threads=1` → **320 passed, 0 failed**.
- [ ] Nem ellenőrzött: az éles hatás a daemon újratelepítése előtt (a felhasználó
  kérésére egy telepítés jön az S37b után); a meglévő variáns-duplikátumok
  merge-elése (S37c); a tipplap-események csapat-identitás feloldása (S37d).

### S37b — éles mérés (daemon újratelepítve)

- Az `install-footballd-agent.sh` a `rust/target/debug/footballd`-t másolja; a
  S37b kód csak új `cargo build -p footballd --features obscura` + telepítés
  után került a daemonba (a 15:25-ös telepítés még a régi binárist vitte ki).
- Kényszerített tipplap refresh (kézi `ops.job`, `entity_key='s37b-check-3'`;
  a napi keret ehhez átmenetileg 10 volt, utána visszaállt 8-ra):
  **24 meccs-szintű forrás-híd** íródott (`entity_kind='match'`, mind
  `confidence=1.0`, azaz natural-key egyezés), a kanonikus meccsszám nem nőtt
  (131), azaz a linkelés nem gyártott duplikátumot.
- **A várttól elmaradó hatás, mérve**: a mai odds-lefedettség változatlan
  **29/131**, mert a tipplap-sorok többsége még a csapat-identitásnál elakad
  (`ismeretlen csapat` → review, a meccs kimarad), és így el sem jut a
  meccs-linkelésig. A v1 ezt esemény-szintű linkeléssel kerülte meg (a tipplap
  esemény kanonikus csapat nélkül is a meccshez kerül). Ezért a következő,
  lefedettséget ténylegesen emelő szelet: **esemény-szintű tipplap-linkelés**
  (a találati szabályok a v1 `link_events.py`-ból) + **identitás-seed** a v1
  crosswalkból (S37d).

## S37c — név-variáns duplikátumok felismerése és merge-elése

- [x] `variant_duplicate_groups` + `is_strict_variant_same_match` a storage-ban,
      `names_strictly_same` / `strip_age_markers` a domainben.
- [x] `merge_duplicate` átviszi a meccs-szintű forrás-hidat, és archiválja.
- [x] Új parancs: `footballctl dedupe variants --date … [--apply]`.
- [x] Tesztek: `dedupe_variants` 2/2, domain identity 9/9, storage dedupe
      suite-ok zöldek.
- [x] Éles futás a fejlesztői DB-n: 2026-11-20 → 9 merge (131 → 122 meccs,
      odds 29 megmaradt, integrity OK); 11-14…11-19 → további 70 merge;
      összesen **79** archív bejegyzés, 1 pár kézi döntésre vár.
- [ ] A 2026-11-17-i tükrözött pár (áldozatnál eredmény) kézi eldöntése.
- [ ] Rendszeres napi futtatás beépítése a napi láncba (a keletkezés
      megelőzése az S37d identitás-seed feladata).
- [x] CI: `82cdf86` push után a **44757839540** futás mindkét jobja zöld
      (Rust domain/storage/API/worker + Swift kliens).

## R2 — a football-research forrás bekötése (S38)

- [x] Adatkönyvtár átköltöztetve `~/Library/Application Support/FootballResearch`
      alá, Desktopon szimlinkkel (TCC-hiba megszűnt, launchd újra betölthető).
- [x] `local_file` transzport + `LocalFileFetcher`; a worker helyi fájl-forrásnál
      nem futtat robots- és kvóta-kaput.
- [x] `research` adapter: napi envelope → `team_identity` / `match_core` /
      `match_referee`; a `coverage.odds`-szal jelölt meccsek detail-fájljaiból
      `match_odds` (`1x2`, `btts`, `dc`, `ou`, opening/closing fázis).
- [x] Migráció `0016`: `local_file` a transport CHECK-ben + `ops.source.identity_mode`
      (`fail_closed` / `create_on_miss`); a research forrás create-on-miss
      (új csapat név+ország+forrás-ID alapján, ütközésnél review).
- [x] `scripts/seed-research-source.sh`: idempotens forrás+policy regisztráció.
- [x] Tesztek: `cargo test -p football-ingest --test research` 5/5,
      `--test live_materialize` 11/11 (benne a create-on-miss eset).
- [x] Éles futás a fejlesztői DB-n (kézi `ops.job`, daemon leállítva a verseny
      elkerülésére): 285 research meccs, 177 oddsos; napi meccsszám 122 → 311,
      oddsos meccsek 29 → 163, publikált feed 46 → 95; 1 696 quote, 348 új
      csapat, 38 függő review.
- [x] A research verseny-név-variánsai miatt keletkezett 22 pontos duplikátumot
      az S37c dedupe motor merge-elte: `dedupe matches`/`dedupe variants` → 0,
      `integrity` → OK.
- [x] A daemon új binárisának telepítése (`scripts/install-footballd-agent.sh`,
      worker újraindult); a kényszerített `deploy-check-1120` jobot a daemon a
      új binárissal dolgozta fel: 134 verseny-csoport materializálva, majd a
      **napi automatikus dedupe-pass 22 duplikátumot merge-elt** (S37e a
      research forrásra bekötve).
- [ ] S38b: verseny-alias réteg (szponzor-szuffixek, walesi „Premier League”),
      hogy a duplikátumok ne is keletkezzenek; S37e: napi automatikus
      dedupe-pass a láncban.
- [x] Kapu: fmt OK, clippy `-D warnings` tiszta, db-reset + check-foundation
      exit 0 (34 `test result: ok`), workspace **329 teszt, 0 hiba**.
- [x] Commit `ae3293f` (S38) pusholva; az automatikus dedupe-pass külön
      commitban.
- [x] API-ellenőrzés a daemonon: `/v1/day/2026-11-20` → **95 meccs, 64 oddsos,
      54 verseny** (előtte 46 / 21).

## S38b — verseny-aliasok és betöltés előtti ellenőrzés

- [x] Az adapter a rövid `name`-et használja elsődlegesen (a szponzoros
      `display_name` csak fallback): a „Ligue 1 McDonald's” típusú
      verseny-duplikátumok forrása megszűnt.
- [x] `COMPETITION_ALIASES`: brit hazai nemzetek (ENG/WAL/SCO/NIR) ligái a v2
      kanonikus neveire és GB alá; spanyol LaLiga forma; ország nélküli
      bejegyzések → INT.
- [x] Női verseny-felismerés kiegészítve jelöletlen ligákkal (SWPL, WE League,
      Liga F, NWSL, WSL, Toppserien, NCAAW, Damallsvenskan, …).
- [x] `scripts/precheck-research-days.py`: betöltés előtti ellenőrzés
      (meccsek, eredmények, oddsok, napon belüli duplikátumok, kanonizált
      versenyek, v2-hiányok).
- [x] Tesztek: 6 új unit teszt az aliasokra/női felismerésre; a research
      integrációs teszt (5) és a live_materialize (11) zöld.
- [ ] A három backfill-nap (2026-11-18, -19, -21) ellenőrzött betöltése.

## S37e+ — automatikus dedupe kiterjesztés és a backfill betöltése

- [x] A napi automatikus dedupe pass (S37e) kiterjesztve: név-variáns
      csoportok (szigorú ellenőrzéssel) és fixture-szintű csoportok
      (azonos verseny+szezon+csapat-pár+UTC-nap, eltérő kickoff) is
      merge-elődnek; fordított párnál a szerepek cserélődnek, hogy az
      eredmény ne vesszen el.
- [x] A research `cancelled` státusza `canceled`-ra normalizálva (a kanonikus
      szótár és a DB CHECK írásmódja).
- [x] Ország nélküli research versenyek `INT` besorolása (a korábbi `XX`
      helyett), a precheck is ezt tükrözi.
- [x] Backfill betöltés ellenőrzött sorrendben (a daemon áll, kézi job +
      `worker tick`):
      - 2026-11-18: 75 meccs, 17 oddsos; duplikátum 0; integrity OK.
      - 2026-11-19: 137 meccs, 38 oddsos; 1 pontos + 1 név-variáns duplikátum
        a passzal lezárva; 1 név-variáns pár (Stars League) kézi döntésre vár.
      - 2026-11-21: 817 meccs, 252 oddsos; a research napi fájl saját 8 fuzzy
        duplikátum-párja (1 óra kickoff-eltérés) a fixture-dedupe-pal lezárva.
- [x] Integritás a betöltés után: `verdict: OK` (duplikált kickoff 0,
      duplikált fixture 0, árva provenance 0).
- [x] Publikációs kapuk: 11-18 21 verified / 54 provisional; 11-19 16/121;
      11-20 95/216; 11-21 223/594.
- [ ] A daemon visszaállítása és az API ellenőrzése a backfill napokra.
- [ ] 2026-11-17 (a research-nek nincs napi fájlja) külön döntés.

## Nyitott kérdések lezárása (S38c)

- [x] Korábbi éjszakai research-fájlok nélküli 2026-11-17 pótlása a research
      saját pipeline-jával: `football.py sync/render/verify 2026-11-17` →
      94 meccs (91 lejátszott), `api-export.py 2026-11-17` → napi fájl.
      Közben javított research-hiba: `tools/odds.py` `raw` változó
      tipplap nélküli napokon `UnboundLocalError`-t dobott (inicializálva).
- [x] 2026-11-17 betöltése: 277 meccs, 40 oddsos, 62 verseny; a napi pass
      4 pontos duplikátumot zárt le; 1 fordított AFCON U23 pár kézi
      döntéssel (`--allow-loose`) merge-elve; integrity OK.
- [x] Stars League fordított név-variáns pár (11-19) kézi döntéssel merge-elve
      → nincs nyitott név-variáns pár egyik backfill napon sem.
- [x] Új CLI-kapcsoló: `footballctl dedupe variants --allow-loose` a kézi
      döntéshez (lazább név/kickoff-ellenőrzés, külön merge-note).
- [x] Nemzetközi sorozatnevek (AFCON selejtező csoportok, Men's International
      Friendly) kanonikus v2-versenyre képezve (1 csoport helyett 1 verseny).
- [x] Backfill összesítés: 11-17 277/40, 11-18 75/17, 11-19 137/38,
      11-20 311/163, 11-21 817/252 meccs/oddos; integrity OK.


## S39 — az app vizuális újratervezése („éjféli stadion")

- [x] Design-rendszer: `apps/FootballApp/Theme.swift` tokenekkel (háttér, grafit
      felületek, borostyán hangsúly, élő zöld, hajszálvonalak, 4 pt-os rács,
      kártya/panel/chip sugarak) és tipográfiai rétegekkel (`scoreboard`,
      `appText`, `appLabel`, `.sectionLabel()`), `.appCard()` felülettel.
- [x] Új komponensek `apps/FootballApp/Scoreboard.swift`: `DayMasthead`,
      `ScoreboardHero` (nagy eredmény, címer/monogram, ELO, gate-jelzés),
      `OddsMatrix` (1X2 / BTTS / 2,5 gól, forrás + időbélyeg), `StatsDuel`
      (arányos borostyán sávok), `VerificationStrip` (gate, szabálykészlet,
      blokkolók), `CrestLogo` monogram-fallbackkel.
- [x] `MatchDetailSheet.swift` új felépítés: hero → meta-rács → periódus-chipek
      → statisztika-párbaj → odds-mátrix → ellenőrzési sáv; 720×620 panel,
      `match.detail.stats|odds|verification` azonosítókkal.
- [x] Nap-nézet és kártyák tipográfiája átvéve (nagy lekerekített dátum,
      táblázatos időpont/eredmény, ELO-chipek, csapatmonogramok; `LiveDot`
      újrahasznosíthatóvá téve).
- [x] Fixture: `tests/fixtures/ui/match-detail.json` (lejátszott meccs
      statisztikával, 7 piacos oddsszal, gate-blokkolóval);
      `scripts/ui-fixture-server.py --detail-fixture` a `/v1/match/...`
      végpontot is kiszolgálja; `run-ui-tests.sh` ezt használja.
- [x] Új UI-teszt `testMatchDetailShowsRichSections`: kártyára kattintás →
      `match.detail` + odds/statisztika/ellenőrzés szakasz látható → Escape zár.
- [x] Bizonyíték: `./scripts/run-ui-tests.sh` → **TEST SUCCEEDED**, app unit
      26/26, UI 8/8 (új teszt 9,387 s); build figyelmeztetés nélkül; élő
      képernyőképek `/tmp/design-day3.png`, `/tmp/design-detail2.png`,
      `/tmp/design-detail-odds.png`.
- [x] Dokumentáció: `docs/superpowers/specs/2026-11-20-app-design.md`.
- [ ] Commit + push + CI (folyamatban).

## S40 — a maradék nézetek átemelése, csapatlogók és teljesítmény-fix

- [x] A három maradék nézet (Versenyek, Csapatok, Üzemeltetés) átvéve az
      S39-es design-rendszerbe: közös `PageHeader`, `SearchField`,
      `RoundIconButton` komponensek a `Scoreboard.swift`-ben; egységes
      tipográfia (`.appText`, `.appLabel`, `.scoreboard`), kártya-hátterek.
- [x] Csapatlogók: `scripts/import-v1-team-logos.py` (v1 WebP-készlet
      tartalom-címzett másolása + `logo_url` referenciák) és
      `scripts/import-luukhopman-logos.py` (GitHub repó, pontos + rövidítés-tűrő
      név-egyezés). Eredmény: **566** team-logo sor az `ops.asset`-ben; a napi
      feedben 104 meccsből **20-nál** van legalább egy logó, 7-nél mindkettő.
      A nézetek a `CrestLogo`-t használják, így a hiányzó kép monogramot rajzol.
- [x] Teljesítmény: a `/v1/teams` végpont korábban **>60 s** (timeout) volt,
      mert a 60 napos ablakot csapatonként pásztázta; egyetlen aggregátumra
      írva **0,19 s** (400 csapat). A lista aktivitás szerint rendez
      (`recent_matches DESC, name`).
- [x] Nap-nézet: a „Kiemelt kör” szűrő alapból kikapcsolva, így nyitáskor a
      teljes napi lista látszik (104 meccs; Következik 83, Élő 4,
      Befejezett 17).
- [x] Dokumentáció: `docs/operations/team-logos.md`,
      `docs/decisions/2026-11-20-csapatlogok.md`.
- [x] Napi feed odds-fix: a nap-query a `default`/`B365`/`Tippmix`/`Avg`/`Max`
      könyvmesterekre szűrt, ezért a research-forrás (bookmaker = a forrás
      neve) oddsai null-ként jelentek meg, holott a quote-ok megvoltak.
      Könyvmester-független tartalék bekerült a COALESCE-láncba: a napi
      feedben **104 meccsből 73-nál** jelennek meg most odds-értékek
      (korábban a felületen 0). A képernyőképen a La Liga: 1 1,48 / X 4,30 /
      2 5,75.
- [x] Bizonyíték: build zöld; `./scripts/run-ui-tests.sh` → **TEST SUCCEEDED**
      (app unit 26/26, UI 8/8); `cargo fmt --all --check` OK; clippy
      `-D warnings` tiszta; `db-reset.sh` + `check-foundation.sh` exit 0;
      `cargo test --workspace -- --test-threads=1` → exit 0, **92 teszt-blokk
      zöld**; a daemon az új binárissal fut (telepítve).
- [x] Élő ellenőrzés: az app a valódi daemonhoz csatlakozva fut, a nap-nézet
      (104 meccs), a Versenyek, a Csapatok és az Üzemeltetés nézet is a helyes
      adatot mutatja; képernyőképek `/tmp/live-day4.png`,
      `/tmp/live-versenyek.png`, `/tmp/live-teams3.png`, `/tmp/live-ops3.png`.

## S41 — napi lista adatminőség + kártya újratervezés

Induló állapot: a 2026-11-20-i napi feed 104 meccse tele volt hibával (FA Cup
alatt kínai csapatok, ugyanaz a fixture két verseny-entitás alatt, „Bahrain–
Bahrain U17" önmaga-meccsek, ISO3 kódok és fehér zászlók, kettévált liganevek),
a kártya pedig logót, pozíciót, ELO-t, BTTS/2,5 piacot és forrásjelzést is
mutatott. Cél: csak kezdési dátum, csapatnevek, eredmény és 1X2 maradjon a
kártyán, minden más a részletes nézetben; az adathibák rendszerszintű javítása.

### Kártya és nap-nézet (SwiftUI)

- [x] `MatchCardModel.swift` új modell: `MatchScore` (ET → FT → nil, „–"),
      `OddsPill` (1/X/2 magyar vesszős formátum), `dayLabel`/`timeLabel`
      (hu_HU, fix szélesség), `isLive`/`isFinished`/`phaseLabel`, `hasOdds`,
      `oddsPills`, `oddsAccessibilityText`. A logó/pozíció/ELO/BTTS/2,5/piacok/
      forrás mezők lekerültek a kártyáról.
- [x] `MatchListView.swift` új kártya: dátum + idő, két csapatnév, eredmény,
      1/X/2 három pill (a nyertes kiemelve); `GeometryReader` + `isWide ≥ 800`
      px és `ViewThatFits` a tördelő/széles elrendezéshez; hover, fókuszgyűrű,
      `match.card.<id>` azonosító, `AccessibilityLabels.matchCardLabel`
      (opcionális dátum/odds) megmaradt.
- [x] `HomeView.swift` reszponzív héj: `GeometryReader`, `TopNavigationBar`
      `compact` mód (<760 px), `FilterBar` két sorba törő chipek + legördülők
      (<1000 px); 640×900 ablakban a kártya tördelt, 1400×940 ablakban egy
      soros. Képernyőképek: `/tmp/card_narrow2.png`, `/tmp/card_wide2.png`,
      `/tmp/s41_final_wide.png`.
- [x] Bizonyíték: Core `swift test` → **54 teszt / 12 suite zöld**; app unit
      **43/43 zöld**; UI **8/8 PASSED**; `xcodebuild build` → BUILD SUCCEEDED.

### Zászlók, országok, liganevek

- [x] Új `CountryCatalog.swift` (Core): ISO3 → ISO2 teljes készlet + FIFA-
      aliasok, ISO2 → zászló regional indicator párokból, GB-ENG/GB-SCT/
      GB-WLS tag-zászló, GB-NIR → 🇬🇧, ISO2 → magyar országnév, fallback.
      `CountryDisplay` a GB + hazai nemzetnév (Wales/Skócia/Anglia/Észak-
      Írország) esetet a hazai nemzet zászlajára oldja.
- [x] Új `CompetitionCatalog.swift` (Core): ország + név alapú alias-tábla
      (J. League/Japanese J1 League → J1 League, 1st Lig → 1. Lig,
      1. osztály [PL] → I Liga, 1st Division [DK], walesi Premier League →
      Cymru Premier, First Division B → Challenger Pro League, ukrán Premier
      League, Taça de Portugal, Primera Federación csoportok, argentin
      Clausura Playoff). A `LeagueGroup.fromDay` e kulcs szerint csoportosít,
      a csoport a legjobb tier/priority értéket örökli, a használhatatlan
      versenynév (üres / csak országkód) országnévre esik vissza.
- [x] `core.country` revízió (auditált SQL, `identity_country_revision`):
      GB neve „Egyesült Királyság" (korábban tévesen „Észak-Írország"),
      GB-WLS → „Wales", GB-SCT → „Skócia", új GB-NIR sor „Észak-Írország".
- [x] 11 brit verseny átsorolása a helyes hazai nemzet alá
      (`identity_competition_country`): 7 skót (Scottish Premiership/
      Championship/League One/League Two, Lowland League East/West,
      SWPL 1 - női), 2 walesi (Cymru Premier, Cymru - dél), 2 észak-írországi
      (NIFL Premiership, NIFL Championship). A napi feed most GB-SCT/GB-WLS/
      GB-NIR kódot ad, a szekciók fejléce „SKÓCIA", „WALES",
      „ÉSZAK-ÍRORSZÁG".

### Adatminőség (Rust)

- [x] Verseny-ország guard (`competitions.rs`): a szótári ország csak akkor írja
      felül a megfigyeltet, ha az üres, a normalizált kód egyezik, vagy az
      ország-név ugyanarra oldódik; CN FA Kupa így CN marad, nem GB.
- [x] `canonical_country_code` (domain): ISO3 → ISO2, ENG/SCO/WAL/NIR →
      GB-ENG/GB-SCT/GB-WLS/GB-NIR, INT → XX; az ingest `upsert_country` és
      `team_country_id` ezt használja, így nem keletkeznek új ISO3-sorok.
- [x] Cross-competition név-variáns dedupe (`dedupe.rs`): a pár-egyezés
      versenyhatáron át is megy, de normalizált ország + gender/age_group/
      branch egyezés kell; női-jelölő guard (`name_marks_womens`) kizárja a
      férfi–női téves párokat. CLI:
      `footballctl dedupe variants --date <D> --cross-competition [--apply]`.
- [x] Identity repair (`rust/crates/identity/src/repair.rs`):
      `rebuild_source_refs` (dry-run riport: 1318 kód, 357 átkötés, 51 új
      csapat, 20 bizonytalan) és `repair_soccerway_matches` capture-scope-olt
      team_identity observation alapján; approve_create kontextus a névből
      vezeti le az age_group/gender értéket (U17 nem olvad felnőttbe);
      oldalankénti részleges javítás; dry-run írásvédett (regressziós teszt),
      age_token guard (U17 ≠ U23).
- [x] Új verification szabályok (`verification-1.2`): SAME_TEAM_BOTH_SIDES
      (korosztály-jelölő nélkül azonos csapat a két oldalon → blocker) és
      FOREIGN_TEAMS_DOMESTIC_COMPETITION (hazai bajnokságban mindkét csapat
      külföldi → blocker). Rule set verzió 1.2 a workerben, a repair-ben és a
      footballctl-ben.

### Alkalmazott adatjavítások (visszaállítható)

- [x] Mentések: `/tmp/football_v2_dev_20261120_pre_s41.dump` és
      `/tmp/football_v2_dev_20261120_pre_dedupe.dump` (542 MB,
      `pg_dump -Fc -Z 5`).
- [x] `footballctl identity repair-matches --from 2026-11-13 --to 2026-11-22
      --apply` → 370 vizsgált, 351 már helyes, 3 átállítva, 0 merge,
      0 visszatartva, 16 feloldhatatlan (0 hiba). Az AFC U17-es kör mind a
      9 sora helyes, nincs önmaga-meccs.
- [x] Cross-competition dedupe: 11-18: 1, 11-19: 13 szigorú + 3 loose,
      11-20: 33 szigorú + 8 loose, 11-16: 2 loose, 11-17: 2 loose merge;
      utána 11-13..11-25 dry-run → nincs név-variáns duplikátum.
- [x] Maradék azonos csapatpárú, csak lokalizált névben eltérő duplikátumok
      (Primera RFEF - 2. csoport / Primera Federacion - Group 2,
      IWL - női / IWL Women, Kings Cup - Thaiföld / Kings Cup (Thailand)):
      a `variant_pair` cross-competition módban mostantól a csapat-identitást is
      bizonyítéknak fogadja el (azonos home/away team ID, ≤12 óra, egyező
      ország + gender/age/branch), a tükrözött + eredményes eset továbbra is
      kézi döntés. Mindhárom pár merge-elve; utána 11-13..11-27 között nulla
      cross-competition duplikátum (SQL-ellenőrzés). Az IWL magyar neve a
      CompetitionCatalogban „IWL Women"-re oldódik.
- [x] `footballctl reverify --from 2026-11-13 --to 2026-11-27` → 1793 meccs,
      0 hiba; gate-állapotok: 590 verified, 1190 provisional, 13 withheld.
      A 11-20-i feed 91 meccs, 11-21-i 223 meccs; nincs önmaga-meccs és nincs
      azonos csapatpár + kickoff duplikátum a verified ablakban.
- [x] Két szándékos withheld a 11-20-i napon: FA Cup Beijing Guoan–Shanghai
      Port (kínai csapatok a GB versenyben) és La Liga Cobresal–Audax Italiano
      (chilei csapatok a spanyol ligában) — a FOREIGN szabály fogta meg őket.
- [x] Nemzeti U17 csapatok névrevíziója (auditált SQL): Bhután U17 →
      „Bhutan U17" (BT), Mongólia U17 → „Mongolia U17" (MN), „Új-Zéland U17"
      beolvasztva a „New Zealand U17" entitásba (alias + merged_into_id,
      ország NZ).

### Kapuk

- [x] `cargo fmt --all --check` → OK; `cargo clippy --workspace
      --all-targets -- -D warnings` → tiszta; `cargo test --workspace --
      --test-threads=1` → exit 0 (minden blokk zöld).
- [x] Swift: Core 54/54, app unit 43/43, UI 8/8, `xcodebuild build` OK.
- [x] Daemon: `cargo build -p footballd -p footballctl --features obscura` →
      OK; `./scripts/install-footballd-agent.sh` → új bináris fut
      (port 60881), `/v1/health` → 200, `/v1/day/2026-11-20` → 91 sor; az app
      az új runtime.json-nal elindítva a valódi daemonra csatlakozik.

### Nyitott / következő szelet

- [ ] Globális `rebuild_source_refs` élesítése (create nélküli móddal) — a
      94 markánsan eltérő ref továbbra is kézi döntést vár.
- [ ] `core.country` ISO3/ISO2 duplikátumok összevonása (S41d) — a mostani
      szelet csak a látható zászlókat és a brit hazai nemzeteket rendezte.
- [ ] Történelmi competition-ország újracímkézés (CN/SK FA Kupa) és a
      csapat-ország INT/rokon hibák szélesebb revíziója.

## S42 — a napi lista alapjai: teljes nap, verseny-ország, forduló, logók

A felhasználó visszajelzése (m01633) szerint az alapok még hiányosak: „első
osztály" mindenhol, invalid kezdésidő, egyiptomi kupa-duplikátum, hiányzó
meccsek, logók, országok szétválasztása, kínai FA Kupa, fordulójelzés. Ez a
szelet ezeket a rendszerszintű okokat javítja.

### Változások

- [x] Napi feed: a `day` lekérdezés alapból a `verified_publish` **és**
      `provisional_publish` sorokat adja (`rust/crates/storage/src/
      repositories/day.rs`); a `?include=all` továbbra is mindent mutat.
      2026-11-20: 267 sor (90 verified + 177 provisional; a 2 withheld
      szándékosan kimarad).
- [x] `date_only` kezdés: `kickoff_precision` bekerült a day API-ba; a kártya
      ilyenkor nem mutat hamis időt, hanem „időpont egyeztetés alatt" jelzést
      (Fluminense–São Paulo 12:00Z placeholdere).
- [x] Forduló: `round_label` a day API-ban és a kártyán („13. forduló",
      „Döntő", „Nyolcaddöntő"); 2026-11-20-on 148/267 meccsnél van forrásadat.
- [x] Osztálycímke: a research adapter most `tier=0`-t ad (ismeretlen), a
      kanonikus szótár adja az ismert tier/priority értékeket; a UI a hamis
      „1. osztály" feliratot elhagyta — a fejléc csak az országot mutatja.
- [x] Országok: `canonical_country_code_with_name` (domain) + ingest
      kanonizálás GB-ENG / GB-SCT / GB-WLS / GB-NIR; a GB versenyek forrás-
      országneve (Anglia / Skócia / Wales) alapján sorolódnak be; a DB-ben az
      `ENG` sor átkódolva `GB-ENG`-re (auditált SQL, `identity_country_revision`).
- [x] Verseny-ország javítás: új `footballctl competitions repair-countries`
      CLI (szavazat + brit hazai nemzet guard); alkalmazva 2026-11-13..27:
      40 audit sor, Chinese FA Cup [CN] (Beijing Guoan–Shanghai Port),
      Slovak Cup [SK] 2, Primera División [BO] 2 / [CL] 6, FA Cup [GB] 0,
      27 angol verseny GB→GB-ENG.
- [x] Egyiptomi kupa duplikátum: 3 csapat-entitás összevonva (auditált SQL,
      `identity_team_merge`), majd `dedupe variants --date 2026-11-20 --apply`
      → 1 merge; a 16:00-s sor maradt.
- [x] Dedupe: a szigorú név-variáns most ≤120 perc kickoff-eltérést fogad el
      (a források 30–60 percet kerekítenek); azonos csapatpár azonos versenyen
      belül eltérő kickoffal is merge-elhető.
- [x] Logók: `scripts/import-v1-megapack-logos.py` (v1 megapack csapat- és
      ligalogók, ISO3→ISO2 ország-család, rövidített név egyeztetés,
      content-addressed másolás, `ops.asset`). Eredmény: 1955 csapat + 560
      legacy + 75 verseny illesztve; a napi feedben 270/521 csapat és
      64/268 meccs verseny-logóval; a fájlok a daemon adatkönyvtárába is
      átmásolva (`~/Library/Application Support/FootballAppV2/agent/data`).
- [x] Kártya/UI: csapatcímer a csapatnév mellett, ligalogó a szekció-
      fejlécben (fallback zászló), forduló és date_only jelzés; a
      `LeagueGroup` a csoport logóját bármely tagból választja.
- [x] Tesztek: a `day-response.json` kontrakt-fixture és a Rust DTO teszt
      frissítve (`kickoff_precision`, `round_label`, `competition_logo`);
      a UI fixture valós állapotot tükröz (date_only, final, GB-ENG szekció);
      a UI-tesztek Tab-sorrendje robusztus (nem fix Tab-szám), új assertion a
      date_only jelzésre és a fordulóra.

### Adatműveletek és visszaállítás

- [x] Mentés az apply előtt: `/tmp/football_v2_dev_20261120_pre_s42_apply.dump`
      (543 MB, `pg_dump -Fc -Z 5`); visszaállítás `pg_restore`-ral.
- [x] Auditált írások: `competition_country_repair` (40), `identity_team_merge`
      (1), `identity_country_revision` (1); a logó-import `ops.asset` sorokat
      szúr be (forrás `v1-megapack`, licence ismeretlen).

### Kapuk

- [x] `cargo fmt --all --check` OK; `cargo clippy --workspace --all-targets
      -- -D warnings` tiszta; `cargo test --workspace -- --test-threads=1`
      exit 0.
- [x] Swift Core 55/55, app unit 43/43, UI 8/8 PASSED, `xcodebuild build`
      BUILD SUCCEEDED.
- [x] Daemon újraépítve + telepítve (port 65493), `/v1/health` 200,
      `/v1/day/2026-11-20` 267 sor, 0 önmaga-meccs, 0 azonos csapatpárú
      duplikátum.

### Nyitott / következő

- [ ] Csapat-entitás duplikátumok összevonása (research vs v1-import; pl.
      „Ath Bilbao" / „Athletic Bilbao") — ez emeli a logó-lefedettséget.
- [ ] `core.country` ISO3→ISO2 sorok összevonása (S41d); addig az API ISO3
      kódot ad, a UI katalógus fordítja zászlóra/névre.
- [ ] Verseny-logó forrás bővítése (a v1-nek csak 96 ligája van; a La Liga
      logója hiányzik), és a fordulójelzés kitöltése a hiányzó meccseknél.

## S43 — napi lista: valós ligák, forduló a fejlécben, duplikátum- és logó-kör

Induló állapot: a felhasználó szerint az angol Championship alatt skót/NIFL
csapatok voltak, a League One és a National League duplikált, sok ligában
maradt duplikáció, a fordulószám többször szerepelt, Bulgáriának nem volt
zászlaja. Cél: a 2026-11-20-i 246–267 soros napi lista teljes auditja
(liga, zászló, név, időpont), a forduló egyszer, a szekciófejlécben.

### Változások

- [x] Championship szétvágása: a 3 kevert sor átmozgatva a Scottish
      Championship (GB-SCT) és a NIFL Championship (GB-NIR) alá; a GB-ENG
      Championship alatt 0 nem angol csapat maradt.
- [x] Duplikált verseny-entitások merge-e (audit `competition_merge`):
      CZ FNL, DK 1. Division, DK A-Liga Women, DK Superliga, IN Indian Super
      League, MY Super League, NIFL Premiership + 45 azonos (ország, név)
      verseny; a maradék GB kódú angol versenyek GB-ENG-re állítva,
      a téves „Premiership" (Cliftonville–Glentoran) NIFL Premiership alá.
- [x] Csapat-merge 2 körben (123 + 162 merge, seed kizárva; survivor: több
      meccs → logó → korábbi): `core.team` 4781 → 4497; önmaga-meccs 0.
- [x] Ország-merge: ISO3→ISO2 52 pár + INT→XX; `core.country` 250 → 201;
      magyar országnevek 131 UPDATE; GB-ENG sor („Anglia") létrejött.
- [x] Meccs-duplikátumok: 12 általános + 5 kézi pár merge (`dedupe pair` CLI),
      köztük a hét angol/olasz/ukrán/orosz/cseh/dán példányai; a férfi–női
      Toulouse-pár szándékosan érintetlen (hamis pozitív).
- [x] Kickoff: a research ±1 órás eltéréseit a hét 16 meccsénél openfoot
      időre igazítottuk, a kickoff-provenance kiválasztást átbillentve
      (audit `match_kickoff_revision`); a két önmaga-meccs csapata javítva
      (Jordan U23 létrehozva, Switzerland U17 → Czech Republic U17).
- [x] Anyagi konfliktus szabály: elavult státusz/üres eredmény nem konfliktus,
      kickoff-eltérés csak >120 percnél; RULE_SET_VERSION → `verification-1.3`
      (worker, identity, footballctl); reverify 1890 meccs, withheld 3 (CD/CG,
      forrás-instabil országkód — helyesen visszatartva).
- [x] UI: a forduló a szekciófejlécbe került („ORSZÁG · 13. forduló"), a
      kártyáról lekerült; a csapatnevek mellett címer, a fejlécben ligalogó
      vagy zászló; 4 új ISO3-alias (BUL→BG, ROM→RO, KSA→SA, KVX→XK).
- [x] Tesztek: domain 9, storage 14→18, verification 11, identity 8; a
      rövidítés-feloldás („Sheffield Weds" = „Sheffield Wednesday") nem nyeli
      el az országneveket („Dominica" ≠ „Dominican Republic"); a hétvégi
      név-variáns dry-run minden napra tiszta.

### Adatműveletek és visszaállítás

- [x] Mentések: `/tmp/football_v2_dev_20261120_pre_s43.dump` (543 MB),
      `/tmp/football_v2_dev_20261120_pre_s42_apply.dump`; visszaállítás
      `pg_restore`-ral. Minden írás tranzakciós és auditált
      (`identity_team_merge`, `competition_merge`, `identity_country_merge`,
      `match_kickoff_revision`, `identity_match_revision`,
      `competition_match_reassign`).

### Kapuk

- [x] `cargo fmt --all --check` OK; `cargo clippy --workspace --all-targets
      -- -D warnings` tiszta; `cargo test --workspace -- --test-threads=1`
      exit 0.
- [x] Swift Core 57/57, app unit 50/50, UI 8/8 PASSED; `xcodebuild build`
      BUILD SUCCEEDED; a futó app a friss binárisból (`/tmp/s43_final1.png`,
      `/tmp/s43_gb3.png`).
- [x] Daemon újraépítve + telepítve (port 56709), `/v1/health` 200,
      `/v1/day/2026-11-20` 246 sor, 121 szekció, 0 önmaga-meccs,
      0 azonos csapatpárú duplikátum, 138 forduló, 83/246 meccsnél mindkét
      csapatlogó, 11 date_only.

### Nyitott / következő

- [ ] A research +1 órás kickoff-eltolásának gyökér-oka az ingest adapterben
      (a napi DB-javítás megtörtént, a forrás-oldali normalizálás még nyitott).
- [ ] A CD/CG 3 withheld forrás-instabil országkódja (soccerway kódváltás).
- [ ] A hét többi napjának (11-21…11-27) egyenkénti duplikátum-auditja.
- [ ] Verseny-logó lefedettség (a v1-nek csak 96 ligája van; a napi 121
      szekcióból 64 kap logót) és a csapatlogó-lefedettség további emelése.

## S43b — ország-semleges duplikátumok és archív-restore

### Változások

- [x] `rust/crates/storage/src/repositories/dedupe.rs`: a cross-competition
      párosítás mostantól a semleges verseny-országot (NULL / `XX` / `INT`)
      wildcardként kezeli, és a korosztályt/ágat a csapatnév jelölője alapján
      egyezteti (a v1-import `senior`/hiányos metaadata miatt eddig kimaradtak
      a nemzeti csapatos párok). Az „egyik oldal egyezik" szabály kikerült: a
      merge csak akkor biztonságos, ha mindkét oldal ugyanaz (normál vagy
      tükrözött), különben két különböző meccs olvadhatott össze.
- [x] `rust/bins/footballctl/src/main.rs`: a `dedupe variants` lekérdezése
      ±1 napos ablakkal dolgozik (a helyi nap és a UTC-nap eltérése miatt a
      23:xx-kor kezdődő meccsek kimaradtak); új `dedupe pair --survivor …
      --duplicate … --note …` parancs az ellenőrzött kézi merge-hez.
- [x] Visszaállítás: a tévesen összevont „FK Skopje–Arsimi" meccs teljes
      archív-payloadból (match + provenance + gate) visszakerült; az „East
      Bengal FC–SC Delhi" sor ellenfele Hyderabad FC-re javítva (mindkettő
      auditált).

### Adatműveletek

- [x] Ország-semleges cross-competition dedupe a 11-13…11-29 ablakon:
      51 + 8 + 7 + 10 merge (főleg v1 „Friendly International", AFCON/UEFA
      Nations League/World Cup U17 párhuzamos entitások); a maradék jelöltek
      hamis pozitívok (U17/U18, férfi/női, U17/női).
- [x] Verseny-merge-ek: DZ „1. divízió" → „Ligue 1"; „Copa de la Reina - női"
      → „Copa de la Reina"; „Women's Champions League" és „UEFA Women's
      Champions League" → „UEFA Champions League Women - League phase"
      (ország XX). Audit: `competition_merge`, `match_dedupe_merge`.
- [x] Mentés az ország-semleges dedupe előtt:
      `/tmp/football_v2_dev_20261120_pre_neutral_dedupe.dump` (571 MB).

### Eredmény (2026-11-20 / 11-21)

- [x] 11-20: 245 meccs, 121 szekció, 0 önmaga-meccs, 0 valódi duplikátum,
      0 withheld, 138 forduló, 83/245 mindkét csapatlogó, 59 verseny-logó,
      11 date_only.
- [x] 11-21: 805 meccs; a maradék fuzzy jelöltek hamis pozitívok
      (Esbjerg U17 / AaB W / Esbjerg fB, Toulouse férfi/női).
- [x] Hétvégi feed-audit: 1830 publishable sor, 0 valódi duplikátum,
      0 önmaga-meccs.

### Kapuk

- [x] `cargo fmt --all --check` OK; clippy `-D warnings` tiszta;
      `cargo test --workspace -- --test-threads=1` exit 0.

### Nyitott

- [ ] 11-17/11-19: 8 withheld a merge-ek után (a két forrás eredménye eltér:
      Aruba–Martinique, Guatemala–Canada, Cook Islands–Tahiti, Oman–Brunei,
      Yemen–Malaysia, CD/CG Ligue 1 sorok) — forrás-oldali eredmény-konfliktus,
      kézi ellenőrzés kell.
- [ ] A hét többi napjának (11-22…11-29) egyenkénti tematikus auditja.

## S44 — női ligák, lezáratlan/kezdés nélküli meccsek, napi hiány-panel

Bemenet: a felhasználó észrevétele (2026-11-20 este) — a Toulouse–Marseille
kétszer szerepelt (férfi és női), a női bajnokság kezelése nem egyértelmű;
korábbi napokon lezáratlan és kezdésidő nélküli meccsek vannak; kér egy
napi „hiány" panelt a v1 mintájára.

### Változások

- [x] Női csapat-identitás javítása (auditált SQL, `/tmp/s44_women3.sql`):
      13 érintett meccs a női versenyekben; a férfi entitást használó női
      csapatok cseréje (Toulouse → „Toulouse (W)", Brighton → meglévő női
      entitás, új „West Ham (W)" és „Saint Malo (W)"), a W-jelölésű csapatok
      `gender='female', branch='women'`. Eredmény: női versenyben nem-female
      csapat 0.
- [x] Női verseny-merge-ek + egységes név: „A-League (női)",
      „Bundesliga (női)", „IWL (női)", „Liga MX (női) - Apertura - Play Offs",
      „Première Ligue (női)", „WE League (női)"; a tévesen összevont
      AFC Asian Cup U17 visszaállítva (22 meccs, audit
      `competition_merge_restore`).
- [x] Lezáratlan meccsek: 22 meccs lezárva a forrás `finished`
      megfigyeléséből (`core.match.status`, `core.match_result`; audit
      `match_status_revision`). A többi lejárt meccsnél a forrás nem adott
      eredményt (a footballdata napi kvótája kimerült; a budget visszaállítva
      60-ra).
- [x] `date_only` kezdésidők felmérése: 11-20: 11, a héten összesen 90;
      pontosabb (exact) forrás-observation nincs, ezért a UI helyesen
      „időpont egyeztetés alatt" jelzést ad.
- [x] API: `DayResponse.gaps` — `fetch_day_gaps` (storage) számolja a
      kezdésidő nélküli, 3 órán túl lejárt-lezáratlan, eredmény nélküli,
      csapat-/verseny-logó nélküli, odds nélküli és withheld sorokat;
      kontrakt-fixture frissítve.
- [x] Swift: `DayGaps` modell + „Hiányzó adatok" kártya a nap fejlécében
      (accessibility: `day.gaps`), és a női ligák UI-kanonizálása
      (`CompetitionCatalog.womensCanonical`: „Women's …"/„… - női"/„… Femenine"
      → „… (női)"; a férfi és női liga külön kulcs).

### Adatműveletek / visszaállítás

- [x] Mentés: `/tmp/football_v2_dev_20261120_pre_neutral_dedupe.dump` (571 MB).
- [x] Minden DB-írás `ops.audit_event` sorral: `identity_team_revision`,
      `identity_match_revision`, `match_status_revision`,
      `competition_merge`, `competition_merge_restore`.

### Eredmény / bizonyíték

- [x] `/v1/day/2026-11-20`: 245 meccs; `gaps`: 11 kezdésidő nélkül,
      106 lejárt-lezáratlan (3 órán túl), 0 eredmény nélküli, 165 csapatlogó
      nélkül, 186 ligalogó nélkül, 97 odds nélkül, 0 withheld.
- [x] Női szekciók a napon: „A-League (női)", „IWL (női)",
      „Première Ligue (női)" (a maradék „A-Liga Women" a UI-kanonizálással
      „A-Liga (női)" lesz).
- [x] Képernyőkép: `/tmp/s44_gaps.png` — a „Hiányzó adatok" kártya a nap
      fejlécében, alatta a szokásos lista.

### Kapuk

- [x] `cargo fmt --all --check` OK; `cargo clippy --workspace --all-targets
      -- -D warnings` tiszta; `cargo test --workspace -- --test-threads=1`
      exit 0.
- [x] `cargo test -p football-api --test day_contract` 1 passed.
- [x] Swift Core: 59 teszt / 12 suite zöld; app build BUILD SUCCEEDED;
      UI-tesztek 8/8 PASSED.

### Nyitott

- [ ] 8 withheld (11-17/11-19): a két forrás eredménye eltér — kézi ellenőrzés.
- [ ] A lejárt, eredmény nélküli meccsek pótlása csak forrásból lehetséges
      (footballdata napi kvóta); újrapróbálás a kvóta reset után.
- [ ] Logó-lefedettség (165/245 csapatlogó nélküli, 186 ligalogó nélküli):
      a v1 megapack csak 96 ligát fed; további forrás/egyeztetés kell.
- [ ] 11-22…11-29 napok egyenkénti tematikus auditja.

### S44b — női duplikátumok utótisztítása

- [x] Cross-competition dedupe a 11-18/11-19 napokra: 2 merge —
      „UEFA Champions League Women - League phase" Paris FC (W)–HB Køge (W)
      (a v1-es „Paris FC W–Koge W" sor beolvasztva), és
      „Women's League Cup" ← „WSL Players Cup" London City Lionesses–Brighton.
- [x] Női verseny-duplikátumok merge-elve: „UEFA U17 (női) - Qualification -
      League A" (két néven élt, 4 meccs) és „WE League (női)" (3 meccs).
- [x] `reverify --from 2026-11-13 --to 2026-11-29`: 1836 meccs, 0 hiba;
      gate-ek: verified 754, provisional 1074, withheld 8.
- [x] 11-19 feed: 98 meccs, női szekciók duplikátum nélkül; 11-20: 245 meccs,
      „Première Ligue (női)" alatt Toulouse (W)–Marseille W egyszer szerepel,
      a férfi Ligue 1 külön.
- [x] Képernyőkép: `/tmp/s44_final.png` (a hiány-panel a végleges
      „lejárt, nincs eredmény" címkével).

### S45 — nyitva maradt meccsek, napi kereső/filterek, női bajnokságok

- [x] Research napi fájlok frissítése 11-16…11-20 (`tools/football.py sync` +
      `docs/api-export.py`), majd repair: 12 kickoff-pótlás, 63 státusz, 64
      eredmény; livescore public API-ból további 92 + 6 eredmény; 21 meccs
      státusza eredményhez igazítva; `merge_duplicate` mostantól viszi a
      lezárt státuszt a túlélőre.
- [x] Automatizálás: `scheduler.rs` research catch-up (7 napra visszamenőleg,
      naponta egyszer) + teszt.
- [x] UI: „Idő nélkül" és „Eredmény nélkül" hiány-szűrők, csapat/liga/ország
      kereső, külön női szűrő, mélyebb osztályok, kiemelt prioritás (5 topliga
      + NB I/NB II) — a filter-sáv két sorban, hogy 1512 pt-on se csússzon el.
      A kiemelt kör a magyar bajnokságokat prioritástól függetlenül mutatja.
- [x] Női bajnokságok: a női csapatok gender/branch javítása; férfi csapatok
      cseréje női meccsekben; francia, német és japán női verseny-duplikátumok
      merge-elve; WE League és A-Liga duplikált fixture-ök merge-elve; férfi
      kupa-meccs áthelyezve a Copa del Rey alá. 11-20-on a Toulouse–Marseille
      női meccs egyszer szerepel, „Ligue 1 · női" felirattal.
- [x] `reverify --from 2026-11-13 --to 2026-11-29`: 1850 meccs, 0 hiba;
      gate-ek: verified 747, provisional 1092, withheld 11.
- [x] 11-20 API: 257 meccs (219 finished, 33 scheduled, 3 live, 2 postponed);
      gaps: 10 kezdésidő nélkül, 21 lejárt eredmény nélkül, 176 csapatlogó,
      189 ligalogó, 88 odds, 3 withheld nélkül.
- [x] Kapuk: `cargo fmt --check`, clippy `-D warnings`, `cargo test --workspace
      -- --test-threads=1` (EXIT=0), Swift Core 59/59, app unit 50/50,
      xcodebuild build, UI 8/8 — mind zöld.
- [ ] Maradék 21 lejárt, eredmény nélküli meccs (főleg U17-selejtezők és kis
      ligák) — a research/livescore nem fedi, a hiány-panelben látszik.
- [ ] 11 withheld (CD/CG forrás-instabil országkód, hiányzó eredmény) kézi
      felülvizsgálata.
- [ ] Logó-lefedettség és a research +1 órás kickoff-eltolás gyökér-okának
      javítása; 11-22…11-29 tematikus audit.

## S46 — +1 órás kickoff gyökér-ok, eredménypótlás, napi lefedettség

- [x] A +1 órás kickoff gyökér-oka: a research `docs/livescore-fetch.py` a
      LiveScore API-tól `tz=1`-gyel kérte az `Esd` mezőt, majd UTC-ként
      értelmezte. Javítás: a fetch URL mindig `/0` (igazi UTC), a helyi időt
      magunk számoljuk. A research export 9 napra újragenerálva.
- [x] `/tmp/s45_repair3.py`: 737 kickoff (−60), 9 date_only pótlás, 13
      státusz, 16 eredmény; a `fact.provenance` kiválasztás javítva
      (nincs `id` oszlop → `observation_id` + `field_key` + `subject_id`).
- [x] Kézi kickoff-ellenőrzés az in-play observationök + LiveScore UTC oldal
      + openfoot alapján: 6 további javítás (Al Zawraa 15:30Z, Shakhtar és
      Prishtina és Ivory Coast U17 16:00Z, Velez 23:00Z, Bihor 12:30Z).
- [x] Új eredmény-forrás: FotMob nyilvános API
      (`/api/data/matches?date=YYYYMMDD`) — 7 + 2 eredmény, 13 kickoff.
      Research JSON-ból 12 további eredmény. Összesen 37 eredmény pótolva.
- [x] JSON→DB teljesség: `Ath Bilbao–Espanol` 0-1 egyezik a FotMob
      `Athletic Club–Espanyol` 0-1-gyel; a FotMob-vs-DB „hiányzó" lista
      névvariáns false negative.
- [x] `reverify --from 2026-11-13 --to 2026-11-29`: 3150 meccs, 0 hiba;
      gate-ek: verified 1099, provisional 2047, withheld 18.
- [x] Napi lefedettség (meccs / lezárt / eredmény / odds / csapatlogó /
      idő nélkül / withheld / verified):
      11-13: 108/106/106/1/19/0/1/48; 11-14: 608/585/585/45/196/8/8/164;
      11-15: 420/416/416/4/53/0/0/86; 11-16: 158/124/124/2/16/8/0/33;
      11-17: 260/210/210/20/52/29/7/155; 11-18: 105/101/101/17/23/0/0/33;
      11-19: 122/89/89/42/23/13/1/45; 11-20: 269/249/249/178/90/10/1/116;
      11-21: 811/1/1/253/434/9/0/221; 11-22: 62/0/0/0/35/0/0/28.
- [x] 11-20 API: 268 meccs; gaps: 10 kezdésidő nélkül, 15 lejárt nyitott,
      0 eredmény nélküli, 186 csapatlogó, 199 ligalogó, 91 odds, 1 withheld.
- [ ] 117 meccs >24 órával kezdés után eredmény nélkül (Friendly
      International 22, AFC Asian Cup U17 11, Persha Liga 11, AFCON U23 10,
      Ligue 1 7, Kings Cup Italy 6, …) — a research/livescore/fotmob nem
      fedi; a hiány-panelben látszik, a napi research catch-up újrapróbálja.
- [ ] Részletes meccsoldal (következő nagy szelet).

## S46b — heti duplikátum- és női-liga audit (a felhasználó visszajelzése után)

Indok: a felhasználó szerint a női BL-fordulóban 3× szerepelt ugyanaz a meccs, a
Toulouse–Marseille női meccs 3× (2 duplikátum + férfi Ligue 1 alá sorolva), és
„amíg nincs minden ki-bugolva, nem mehetünk tovább"; legalább 1 hétre visszamenőleg
minden napot újra kell ellenőrizni.

Vizsgálat:
- [x] Backup a merge-ek előtt: `/tmp/football_v2_dev_20261121_pre_week_dedupe.dump` (548 MB).
- [x] Exact klaszter-audit (11-13…11-21, 2861 meccs; fold, (W)/W/Ladies/női
      markerek levágása, utd/weds/ath feloldás, ±12 óra): 98 klaszter
      (1 mixed = Toulouse, 97 azonos nemű).
- [x] Toulouse-klasztter: a férfi Ligue 1 Toulouse–Marseille 11-20-án nem
      létezik (research JSON + FotMob csak a női Première Ligue-et adja) → a
      `01a1f7a8-e2bc` férfi ál-sor és a `01a1fb4d-a64c` duplikátum beolvasztva a
      `01a1fb22-2cf3` (Première Ligue (női), Toulouse (W)–Marseille (W) 1-1) alá.
- [x] 97 azonos-nemű klaszter merge-elve `footballctl dedupe pair`-ral
      (ok 97, fail 0), survivor: több provenance → logó → verified.
- [x] Fuzzy auditok kézi ellenőrzéssel: szigorú (mindkét oldal ≥0.86) 16 merge;
      plan_fuzzy3 26+5; laza (≥0.72 + token-guard) 24 merge; egyoldalas-kanonikus
      (azonos kickoff + egyik oldal pontos egyezés + másik ≥0.45) 23 merge.
      Az első laza tervet elvetettük: union-find tranzitív láncolás hamis
      párokat kevert (U17/U19 és különböző nemzetek).
- [x] Egy hamis merge javítva: Denmark U23–UAE U23 ↔ UAE U23–Jordan U23; a
      soccerway kódok instabilak (OxxDcngs: Jordánia→Denmark→UAE; 88mfeE0h:
      EAE→Jordan→UAE), ezért a túlélő csapatai auditáltan UAE U23–Jordan U23-ra
      állítva.
- [x] ASEAN Club Championship: ugyanaz a fixture két verseny-entitás alatt
      (`Cong An Ha Noi–Johor DT` gender-flag hibával vs `Công An Hà Nội–Johor
      Darul Ta'zim`) → match merge; a 0 meccses csapat-duplikátumok
      (Cong An Ha Noi, Johor DT) beolvasztva ékezetes/tele nevű párjukba.
- [x] Verseny-duplikátumok: azonos ország + azonos foldolt név (NIFL
      Championship, Eerste Divisie, Eredivisie, Liga Nacional Apertura, USL
      Championship Playoff, Division Profesional PY), nemzetközi NULL/XX párok
      (Africa Cup of Nations - Qualification, Club Friendly), valamint a négy
      WE League entitás (`WE League`, `WE League - női`, `WE League (női)`,
      `Women's WE League`) egy `WE League (női)` alá vonva.

Ellenőrzés:
- [x] Exact klaszter: 0; laza pár: 0; egyoldalas kanonikus pár: 0.
- [x] API-napi duplikátum-audit 11-13…11-22 minden napra: 0 azonos sor.
      (11-13: 107 meccs, 11-14: 600, 11-15: 420, 11-16: 109, 11-17: 206,
      11-18: 97, 11-19: 105, 11-20: 247, 11-21: 843, 11-22: 62.)
- [x] `reverify --from 2026-11-13 --to 2026-11-29`: 3042 meccs, 0 hiba;
      gate-ek: verified 1075, provisional 1962, withheld 19.
- [x] 11-20: a női Première Ligue egyszer (Toulouse (W)–Marseille (W) 1-1);
      a férfi Ligue 1 Toulouse-meccs külön napon (11-21, Lens–Toulouse).
- [x] App-ellenőrzés: `/tmp/s46b_final.png` (11-21, 843 meccs, hiány-panel:
      27 kezdésidő nélkül, 422 csapatlogó, 659 ligalogó, 579 odds, 2 withheld).

Nyitott:
- [ ] 116 meccs >24 órával kezdés után eredmény nélkül (U17/kis ligák; a
      research/livescore/fotmob nem fedi) — hiány-panelben látszik.
- [ ] 19 withheld (forrás-instabil CD/CG országkód, hiányzó eredmény).
- [ ] Logó-lefedettség (422/840 csapat, 659 liga a látható napon).
- [ ] A heti duplikátum-audit automatizálása (a mostani szkriptek /tmp-ben).

### S46b folytatás — második audit-kör (névvariánsok, verseny-entitások)

- [x] Aggresszívabb duplikátum-szkennelés (azonos kickoff + egyik oldal normalizált
      egyezése + másik oldal hasonlóság ≥0.28, W-jelölő egyezéssel): 8 valódi
      névvariáns pár a hétre, mind merge-elve:
      NEC Nijmegen–PSV / PSV Eindhoven; Naft Masjed Soleyman–Shenavarsazi Qeshm /
      Qeshm Island; East Bengal–Hyderabad / East Bengal–SC Delhi (a FotMob a
      rossz ellenfelet adja, a livescore a helyes); Floridsdorfer AC–Young Violets /
      Austria Wien II; Benfica–Roma, Rostov–Krylia, USL New Mexico–Sacramento,
      LPF Independiente–Tauro, Suchitepequez–Antigua, Cusco–AD Tarma (3 sor → 1).
- [x] A maradék 0.28-as jelöltek hamis pozitívok (különböző meccsek: Coton Sport /
      Loto FC, Cavese / Monopoli / Sorrento, Mauritania / China stb.) — nem merge-elve.
- [x] Verseny-entitás merge-ek: azonos ország + azonos foldolt név (NIFL
      Championship, Eerste Divisie, Eredivisie, Liga Nacional Apertura, USL
      Championship Playoff, Division Profesional PY, Premiership → Scottish
      Premiership, LPF Apertura), nemzetközi NULL/XX párok (Africa Cup of Nations
      Qualification, Club Friendly), 4 WE League entitás, 3 női BL entitás
      (`Women's Champions League`, `UEFA Women's Champions League` →
      `UEFA Champions League Women - League phase`).
- [x] Csapat-duplikátumok: Cong An Ha Noi, Johor DT (0 meccses v1-import sorok) a
      tele/ékezetes párjukba olvasztva, aliasszal.
- [x] Végső ellenőrzés: exact klaszter 0, laza pár 0, kanonikus egyoldalas pár 0,
      API-napi duplikátum 10 napra 0 (11-13: 107, 11-14: 600, 11-15: 420,
      11-16: 105, 11-17: 205, 11-18: 97, 11-19: 103, 11-20: 245, 11-21: 841,
      11-22: 62).
- [x] `reverify --from 2026-11-13 --to 2026-11-29`: 3031 meccs, 0 hiba;
      gate-ek: verified 1072, provisional 1954, withheld 19.
- [x] Női BL 11-19: 5 meccs, egy verseny alatt, duplikátum nélkül; Toulouse
      11-20: csak a női Première Ligue sor.
- [x] Audit-események: 1640 match_dedupe_merge, 19 competition_merge, 34
      status/kickoff revision, 765 kickoff revision, 135 result fill (utolsó 2 óra).
- [x] App-ellenőrzés: `/tmp/s46b_final2.png` (11-21, 841 meccs, hiány-panel).

## S47 — duplikátum- és konzisztencia-audit a CLI-ben

- [x] Új `football_storage::repositories::audit` modul: `audit_range(pool, from, to)`
      — pontos/név-variáns klaszterek (union-find, ±12 óra, nem/korosztály-guard),
      verseny-duplikátumok (azonos foldolt név + ország, semleges ország wildcard),
      dupla szereplés (ugyanaz a csapat 6 órán belül két meccsen), valamint
      konzisztencia (lezárt eredmény nélkül, eredmény nem terminális státusszal,
      visszatartva, ország nélküli csapattal).
- [x] `footballctl audit --from <D> --to <D> [--json]`: magyar riport, találat
      esetén exit 1 (kapuként használható). Unit tesztek: 7 (pontos egyezés,
      névvariáns mindkét orientációban, korosztály/nem kizárás, dupla szereplés,
      klaszterezés, 12 órás ablak).
- [x] Első futás 11-13…11-22: 2804 meccs, 5 név-variáns klaszter (Moquegua–Grau,
      Rathfriland–Moyola, Armagh–Rathfriland, Dunfermline–Morton, Aachen–Mannheim),
      25 dupla szereplés, 0 verseny-duplikátum a név-egyezés szerint.
- [x] Javítások: 3 verseny-merge (DE „3. osztály" → „3. Liga", PE „Liga 1 -
      Clausura" és „Peru Liga 1" → „Liga 1"), 5 meccs-merge (`dedupe pair`).
- [x] Kapuk: `cargo fmt --all --check` OK, clippy `-D warnings` tiszta,
      `cargo test -p football-storage --lib` 25 passed, footballctl integrációs
      teszt 1 passed.
- [ ] Nyitott (következő szelet): a 25 dupla szereplés valós adathibáinak javítása
      (többnyire soccerway/v1 félre-mappelt csapatok: W-, B-, U23-csapatok a
      felnőtt entitáson, illetve spurious sorok, amelyeket a research nem ismer).

## S48 — reconcile-alapú adatjavítás (B-/női csapatok, spurious sorok)

- [x] Reconcile a research napi JSON-okkal (11-13…11-23, 2807 meccs): 2567 pár,
      240 DB-oldali pár nélküli sor, 152 verseny-név eltérés; a valódi hibák
      csoportjai: B-csapatok a felnőtt entitáson, női meccsek férfi versenyben,
      forrás által nem ismert spurious sorok.
- [x] Csapat-javítás: 8 új entitás (Eibar B, Girona B, Tenerife B, Las Palmas B,
      Castellon B, Real Sociedad B, Le Havre W) + meccs-relink; a két francia női
      meccs (Lens–Montpellier, Le Havre–Nantes) a „Première Ligue (női)" alá.
- [x] Francia női versenyek összevonása „Première Ligue (női)" alá; alias-csapatok
      merge-elése (H&W Welders → Harland & Wolff Welders FC, Reus → Reus FC Reddis,
      Wolves → Wolverhampton Wanderers, Senegal XX → Senegal SN); az AFCON U23
      South Sudan–Senegal sor a helyes U23 entitásokra kötve.
- [x] Research/FotMob ellentmondás miatt visszatartva 10 spurious sor (Ards–H&W
      Welders, Cavese–Sorrento, Savoia–Monopoli, Atletico Huila–Envigado,
      South Africa–Madagascar, China–Mauritania, a National 1 Group C két 16:00-s
      sora, Hyeres–Canet, Hauts Lyonnais–Etoile Frejus), a 21 szintetikus demó-sor
      szintén withheld.
- [x] Audit-finomítás: a visszatartott és a le nem játszott (canceled/postponed/
      abandoned) sorok nem számítanak duplikátumnak/dupla szereplésnek.
- [x] 11-01…11-12 és 11-24…11-30 korábbi duplikátumai merge-elve
      (`dedupe variants` 6+3+1, `dedupe matches` 2), a hétvégi hibaosztályok
      javítva; ország nélküli csapatok (12) országot kaptak.
- [x] Audit-eredmény: 11-01…11-12 TISZTA, 11-13…11-23 TISZTA, 11-24…11-30 TISZTA
      (a decemberi ablakban 1 kategória maradt, következő szelet).
- [x] Kapuk: `cargo fmt --all --check` OK, clippy `-D warnings` tiszta,
      `cargo test -p football-storage --lib` 25 passed.
- [ ] Nyitott: decemberi ablak auditja, verseny-név aliasok beépítése az ingestbe
      (hogy az újraingest ne hozza vissza a duplikátumokat), 30 visszatartott sor
      felülvizsgálata, 116 forrás nélküli eredmény, logó/odds-lefedettség, UI.

## S49 — logó-lefedettség és részletes meccsnézet odds-javítás

### Változások
- `scripts/import-fotmob-logos.py`: a FotMob nyilvános napi meccs-JSON-jaiból név alapján párosítja a hiányzó logójú csapatokat (pontos fold, majd egyértelmű token-részhalmaz; női jelölő egyezés kötelező), letölti a `teamlogo` képet, tartalom-címzett útvonalra menti a `DATA_DIR/raw/assets` alá, és `ops.asset` sort ír. Dry-run alapértelmezett, `--apply` ír.
- Alkalmazva 2026-11-21…11-30 között játszó csapatokra: 273 új csapatlogó (215 + 58); a 11-21-i feed `missing_team_logo` 412 → 230.
- `rust/crates/storage/src/repositories/match_detail.rs`: a részletes meccsnézet odds-laterálja eddig kizárólag `phase='closing'` idézetet fogadott el, ezért a nap-listán látszó odds a részletes nézetben `null` volt. Most a napi nézettel azonos módon `DISTINCT ON (bookmaker, market, outcome, line)` + `(phase='closing') DESC` preferencia, és a könyvmester-lánc végén catch-all fallback (pl. `tipplap` könyvmesterű idézetekre is).

### Ellenőrzések
- `cargo fmt --all --check` OK; `cargo clippy -p football-storage -p football-api -p footballctl --all-targets -- -D warnings` tiszta.
- `cargo test -p football-storage --lib` 25 passed; `cargo test -p football-api --test routes` 9 passed.
- Élő API: `/v1/match/01a1fa06-82c3-7208-93c9-8cd3065fcab7` → odds {home 3.60, draw 3.40, away 1.73, btts 1.65/1.99, over/under 2.5 1.66/1.97}; a daemon új porton fut (runtime.json), `/v1/day/2026-11-21` 828 meccs, 230 hiányzó csapatlogó, 0 duplikált csapatpár.

### Nyitott
- Verseny-logók továbbra is hiányosak (648 a látható napon); a FotMob league-logo végpont 403-at ad, más forrás kell.
- A 2026-os történelmi v1-duplikátumok (346 klaszter) külön adósság; a 2027-es két azonos idejű, ütköző sor forrás nélkül nem dönthető el.

## S50 — 2026-os történelmi duplikátumok és tartós editorial hold

### Változások
- Adat: a 2026-os audit 346 név-variáns klaszteréből 65 pontos duplikátum-pár és 152 azonos csapatpárú sor `footballctl dedupe pair`-ral merge-elve; 141 globális csapat-duplikátum (`pg_temp.merge_team`) összevonva (match/standing/stats/club_elo/ref/alias/asset/entity+team), demo-seed kizárva.
- Új migráció `db/migrations/0017_editorial_hold.sql`: `ops.editorial_hold(match_id PK, reason, note, created_at)` — ember által ellenőrzött, ellentmondó forrás-fixture-ök tartós kizárása, mert a `reverify` a publish gate-et újragenerálja.
- `rust/crates/storage/src/repositories/day.rs`: a napi lista és a hiány-mutatók is kiszűrik a holdolt meccseket (`NOT EXISTS ops.editorial_hold`).
- `rust/crates/storage/src/repositories/audit.rs`: a klaszter- és dupla-szereplés-vizsgálat kizárja a holdolt sorokat.
- Tesztek: `rust/crates/storage/tests/audit_hold.rs` (holdolt sor kimarad az auditból), `day_query.rs::editorial_hold_hides_match_from_day`, `connection.rs` táblaszám 47→48.

### Adatművelet (visszaállítás: dumpból)
- Backup: `/tmp/football_v2_dev_20261121_pre_s50.dump` (551 MB).
- 12 novembrei ellentmondó sor editorial holddal kizárva (Ards, Bourges 18, Canet Roussillon, Cavese, FC Limonest, Harland & Wolff Welders, Hauts Lyonnais, Envigado stb.); a research JSON maga is ellentmondó ezeken a napokon, ezért a forrás nem dönthető el.
- November (11-01…11-30) és 11-13…12-01 audit: **TISZTA**; 2026 egész éve: 301 klaszter marad, klubonkénti kézi döntést igényel (Jove Español → Espanyol, Res./Under 19/CC utótagok, LaLiga 2 tartalék-nevek) — dokumentált adósság.

### Ellenőrzések
- `cargo fmt --all --check` OK; `cargo clippy --workspace --all-targets -- -D warnings` tiszta; `cargo test --workspace -- --test-threads=1` minden `test result: ok`.
- `footballctl reverify --from 2026-11-01 --to 2026-12-01`: provisional 2161, verified 1185, withheld 24.
- Új daemon (port 50084), `/v1/day/2026-11-21` → 827 meccs (a holdolt sorok nélkül), gaps: 21 kezdésidő nélkül, 229 csapatlogó, 647 ligalogó, 565 odds, 7 visszatartva; `/v1/day/2026-11-20` → 244 meccs.
- Futó app a DerivedData binárisból, kép `/tmp/s50_final.png` (827 meccs, hiány-panel, La Liga/LaLiga 2, Real Sociedad B helyes kickofftal).

### Nyitott
- 2026-os 301 klaszter kézi osztályozása (klubonkénti tudás); 2027-es 2 ütköző sor; verseny-logó lefedettség (FotMob 403); 116 forrás nélküli eredmény; részletes meccsoldal és további UI-fejlesztések.

## S51 — verseny-logók, 2027-es ütközés, audit-állapot

### Változások
- `scripts/import-fotmob-competition-logos.py`: a FotMob napi JSON `leagues[]` (id, name, ccode) alapján párosítja a logó nélküli v2 versenyeket (pontos fold, majd egyértelmű hasonlóság ≥0.82), a `leaguelogo/{id}.png` képet böngésző User-Agent + Referer fejléccel tölti le (enélkül 403), tartalom-címzett útvonalra menti és `ops.asset` sort ír. Dry-run alapértelmezett.
- Alkalmazva: 80 párosított verseny, 19 új verseny-logó (a többi FotMob-azonosító képe 403); a 11-21-i feed `missing_competition_logo` 647 → 616.
- 2027-es ütköző sor (Brentford–Ipswich 2027-02-20 12:00, mert Brentford és Ipswich is más meccsen szerepel ugyanakkor) editorial holddal kizárva; a 2027-es audit így **TISZTA**.

### Ellenőrzések
- Audit: 11-01…11-30 TISZTA, 11-13…12-01 TISZTA, 11-22…11-30 TISZTA, 2026-12 TISZTA, 2027 TISZTA. 2026 egésze: 301 klaszter (történelmi v1-adathibák, klubonkénti kézi döntés).
- `cargo test --workspace -- --test-threads=1` zöld; fmt/clippy tiszta.
- Élő API: `/v1/day/2026-11-21` 827 meccs, `/v1/day/2026-11-20` 244 meccs; app újraindítva, kép `/tmp/s50_final.png`.

### Blokkolt / nem ellenőrzött
- UI-tesztek futtatása ebben a környezetben nem sikerült: a runner „Timed out while enabling automation mode", majd „hung before establishing connection" hibát adott (a Swift kód az utolsó zöld futás óta nem változott; a Rust-oldali S50/S51 változásokat a Rust kapuk fedik).
- Soccerway visszamenőleges eredménypótlás nem lehetséges: a route csak a mai és a tegnapi napot szolgálja ki (`route_for`), a korábbi napokra „üres fetch-terv"; a tegnapi rescan breakeren blokkolt.
- Verseny-logók: a FotMob 403 miatt csak részleges (19/80); további forrás kell.

## S52 — verseny-név egységesítés és román Liga II helyreállítás

### Változások
- `apps/FootballApp/HomeView.swift`: a `DayGapsCard` chipjei külön AX-gombokként
  elérhetők (`.accessibilityElement(children: .contain)`), kattintásra a megfelelő
  hiány-szűrőt aktiválják (`day.gaps.kickoff` → „Idő nélkül", `day.gaps.open` →
  „Eredmény nélkül"), aktív állapotban akcent színnel.
- Adat: a tévesen „Liga 1"-be olvasztott román „Liga II" helyreállítva (új
  competition + 2026/27 szezon, 9 meccs visszahelyezve a provenance-observációk
  alapján), a román „Superliga" 2 meccse a „Liga 1" alá merge-elve.
- Verseny-név egységesítés (audit `competition_merge`/`competition_rename`):
  DK „1. osztály" → „1. Division"; EG „Egyiptomi Kupa" → „Egyptian Cup";
  ES „La Liga 2" (205 meccs) → „LaLiga 2"; ES „Liga F - női" → „Liga F";
  ES Primera RFEF / Segunda Federacion csoport-variánsok → kanonikus
  „Primera Division RFEF: Group 2", „Segunda Division RFEF: Group 1/5";
  PT „Portugál Kupa" + „Taca de Portugal" → „Taça de Portugal";
  UZ „Szuperliga"/„Superliga" → „Super League"; ZM „Szuperliga" →
  „Super League"; JO „Jordán Kupa" → „Jordan Cup"; BR „Brazil Kupa U20" →
  „Copa do Brasil U20".

### Ellenőrzések
- `footballctl audit --from 2026-11-13 --to 2026-11-30`: 3032 meccs,
  0 dupla szereplés, 0 verseny-duplikátum, 0 konzisztencia-hiba → **TISZTA**.
- `footballctl reverify --from 2026-11-13 --to 2026-11-30`: 3032 meccs,
  0 hiba; gate: provisional 1950, verified 1076, withheld 24.
- `/v1/day/2026-11-21`: 827 meccs, 207 szekció; a kanonikus nevek élnek
  (1. Division [DK], LaLiga 2 [ES], Liga II [RO], Super League [UZ],
  Primera Division RFEF: Group 1/2 [ES]).
- `xcodebuild build` a HomeView-változás után BUILD SUCCEEDED; AX-press
  igazolva: a „21, kezdésidő nélkül" chip aktiválja az „Idő nélkül 21" szűrőt.

### Blokkolt / nem ellenőrzött
- UI-tesztek ebben a környezetben továbbra sem indultak el (automation mode
  timeout / runner nem csatlakozott); a Swift-kód változása csak a
  `DayGapsCard` AX-felépítése, a zöld build és az AX-press igazolja.
- FotMob verseny-logók: 19/80 letöltve, a többi FotMob-azonosító 403.

## S53 — részletes meccsnézet: forma és egymás elleni mérleg

### Változások
- `rust/crates/storage/src/repositories/match_detail.rs`: új `FormEntry`,
  `HeadToHeadEntry`, `MatchExtras` típusok és `fetch_team_form`,
  `fetch_head_to_head`, `fetch_match_extras` lekérdezések. A forma az utolsó
  5 lejátszott meccset adja a csapat szemszögéből (W/D/L + ellenfél + eredmény),
  az egymás elleni lista a két csapat utolsó 5 találkozóját. Minden lekérdezés
  a meccs kezdete előtti állapotra szűr (`kickoff_at < before`), így jövőbeli
  információ nem szivárog vissza.
- `MatchDetailRow` három új mezővel bővült: `home_form`, `away_form`,
  `head_to_head` (üres listával, ha nincs korábbi adat).
- `rust/crates/api/src/routes/match_detail.rs`: a `/v1/match/{id}` válasz
  kitölti a három új mezőt; hiba esetén a meccs részletei akkor is megjelennek,
  csak a kiegészítések maradnak üresek.
- Swift: `MatchDetailModels.swift` új `FormEntry`/`HeadToHeadEntry` modellek és
  opcionális mezők (a régi fixture-ök is dekódolhatók); `MatchDetailSheet.swift`
  új „FORMA · UTOLSÓ 5 MECCS" szekció színezett W/D/L chipekkel (akadálymentes
  felolvasással) és „EGYMÁS ELLEN" lista.

### Ellenőrzések
- `cargo test -p football-storage --test match_detail_extras -- --test-threads=1`
  → 1 passed (a jövőbeli meccs nem szerepel sem a formában, sem a H2H-ban).
- `cargo fmt --all --check` OK; `cargo clippy -p football-storage -p football-api
  --all-targets -- -D warnings` tiszta.
- Swift Core tesztek: 60/60 (új `decodesFormAndHeadToHead`); `xcodebuild build`
  BUILD SUCCEEDED.
- Élő API (daemon, port 53686): `/v1/match/01a1e9e8-d362-75bd-91b3-93993a9942dc`
  (Real Madrid–Celta) → home_form 5 meccs (Valencia 3–0 W, Santander 3–0 W,
  Barcelona 2–1 W, Sevilla 4–1 W, Villarreal 1–0 W), away_form 5, head_to_head 5
  (Celta–Real Madrid 1–2 stb.).
- Élő UI: a Sevilla–Real Betis részletező a formát és az egymás elleni
  mérleget is mutatja (`/tmp/s53_detail2.png`).

### Nyitott
- A forma/H2H csak lejátszott, eredménnyel rendelkező meccsekből számol; a
  116 forrás nélküli eredmény továbbra is hiányzik.
- A részletes nézet további bővítése (sérülések, kezdőcsapatok, események)
  csak akkor lehetséges, ha a források ilyen adatot adnak.

## S54 — következő meccsek a részletes nézetben

### Változások
- `rust/crates/storage/src/repositories/match_detail.rs`: új `NextMatchEntry`
  (`match_id`, `kickoff_at`, `competition_name`, `opponent_name`, `is_home`) és
  `fetch_team_next(pool, team_id, after, limit)` — a csapat következő, még le nem
  játszott meccsei a kezdés után; `MatchExtras` most `home_next`/`away_next`
  mezőkkel is rendelkezik, `fetch_match_extras` legfeljebb 3-3 következő meccset ad.
- `MatchDetailRow` + `home_next`/`away_next` (`#[serde(default)]`), a `From` impl
  üres vektorral tölt; `rust/crates/api/src/routes/match_detail.rs` a válaszban
  kitölti őket.
- Swift: `NextMatchEntry` modell, `MatchDetail.homeNext/awayNext`, a
  `MatchDetailSheet`-ben „KÖVETKEZŐ MECCS" szekció csapatonként (dátum, ellenfél,
  H/V jelzés).
- `rust/crates/storage/tests/match_detail_extras.rs` kibővítve: a jövőbeli
  meccsek nem szivárognak a formába/H2H-ba, a következő meccsek listája a kezdés
  utáni, nem lejátszott sorokat adja, csapatonként legfeljebb 3-at.

### Ellenőrzések
- `cargo test -p football-storage --test match_detail_extras -- --test-threads=1`
  → 1 passed; `cargo fmt --all --check` OK; `cargo clippy --workspace
  --all-targets -- -D warnings` tiszta.
- Swift Core tesztek: 61/61 (új `decodesNextMatches`); `xcodebuild build`
  BUILD SUCCEEDED.
- Élő API (daemon, port 54966): `/v1/match/01a1e9e8-d362-75bd-91b3-93993a9942dc`
  → home_next 3 (Alaves H, Ath Bilbao V, Osasuna H), away_next 3 (Villarreal H,
  Barcelona V, Espanol V).
- Élő UI: a Sevilla–Real Betis részletező „KÖVETKEZŐ MECCS" szekciója látszik
  (`/tmp/s54_detail.png`).

### Nyitott
- A 140 eredmény nélküli meccs FotMob-alapú pótlása alias-tábla nélkül nem
  biztonságos (0 strict találat; a laza találatok hamisak) — dokumentálva.

## S55 — tabella a verseny-adatlapon

### Változások
- `rust/crates/storage/src/repositories/competition.rs`: új
  `CompetitionStandingRow` (position, team, logo, played/wins/draws/losses,
  goals_for/against, goal_diff, points) és `fetch_competition_standings`.
  A tabella a verseny legfrissebb meccsű szezonjából (max kickoff) számol, csak
  lejátszott, érvényes eredményű (`status ∈ finished/awarded`, `validity='valid'`)
  meccsekből; pont 3–1–0, sorrend pont → gólkülönbség → lőtt gól → név.
- `/v1/competition/{id}` válasz új `standings` mezővel (a meccs- és csapatblokkok
  mellett; a kliens opcionálisan dekódolja).
- Swift: `CompetitionStanding` modell, `CompetitionDetail.standings`, a
  `CompetitionSheet` új „Tabella" füle: #, csapat (címer + név), M, GY, D, V,
  LG, KG, GK, P; a top 4 akcent színnel; üres állapot szöveggel.
- Tesztek: `rust/crates/storage/tests/competition_query.rs
  ::standings_are_computed_from_played_matches` (3 csapat, pont/ Sorrend/GK
  ellenőrzés), Core `decodesStandingsTable`.

### Ellenőrzések
- `cargo test -p football-storage --test competition_query -- --test-threads=1`
  → 5 passed; `cargo fmt --all --check` OK; clippy `-D warnings` tiszta;
  `cargo test --workspace -- --test-threads=1` zöld.
- Swift Core: 62/62; `xcodebuild build` BUILD SUCCEEDED.
- Élő API: `/v1/competition/01a1d9dd-4963-72b2-854e-01d76519be27?date=2026-11-21`
  → 2026/27 tabella 24 csapattal (Barcelona 33 pont, Real Madrid 33, Atlético 31).
- Élő UI: a La Liga adatlap „Tabella" füle (`/tmp/s55_standings.png`).

### Megjegyzés
- A tabella a jelenlegi szezont a legfrissebb kickoff szerint választja; a
  szezonon belüli meccsszám a forrásadatoktól függ (a synthetic v1-adatokban
  néhol 24 csapat szerepel egy ligában).

## S56 — kedvenc versenyek (csillag + szűrő)

### Változások
- Új `apps/FootballAppCore/Sources/FootballAppCore/LeagueFavorites.swift`: a
  kedvenc versenyek a kanonikus liga-kulccsal (`LeagueGroup.id`, pl. `"ES|la liga"`)
  a UserDefaults-ban (`day.favoriteLeagues`) tárolódnak, így liga-átnevezés és
  entitás-összevonás után is megmaradnak; `load/contains/toggle/save` API.
- `apps/FootballApp/MatchListView.swift`: a szekciófejléc opcionális csillag
  gombot kapott (`favorites`, `onToggleFavorite` paraméterek; a többi hívó
  változatlan, mert a paraméterek defaulttal rendelkeznek); a csillag AX-címkéje
  és azonosítója (`league.favorite.<id>`) a be-/kikapcsolást mondja.
- `apps/FootballApp/HomeView.swift`: `favorites` állapot a UserDefaults-ból, új
  „Kedvencek" chip (`filter.favorites`) a női szűrő mellett; a csoportosítás a
  kedvencekre is szűr, a `toggleFavorite` azonnal ment.

### Ellenőrzések
- Core tesztek: 64/64 (új `LeagueFavoritesTests`: toggle/mentés, rendezett
  kulcsok izolált UserDefaults suite-dal).
- `xcodebuild build` BUILD SUCCEEDED.
- Élő UI: a La Liga csillagozva (`/tmp/s56_star.png`), a „Kedvencek" szűrő
  aktiválva csak a La Liga 4 meccsét mutatja, a chip-számlálók is igazodnak
  (`/tmp/s56_favorites_filter.png`).

## S57 — csapat-adatlap: forma és következő meccsek

### Változások
- `rust/crates/api/src/routes/team.rs`: a `/v1/team/{id}` válasz két új mezőt
  kapott: `form` (utolsó 5 lejátszott meccs a csapat szemszögéből, W/D/L) és
  `next` (következő legfeljebb 3 kijelölt meccs). A két lekérdezés a meccs-
  részletezőből már meglévő `fetch_team_form` / `fetch_team_next`; nem
  kritikusak, hiba esetén üres listával jelenik meg az adatlap.
- Swift: `TeamDetail.form`/`next` opcionális mezők (a meglévő `FormEntry` /
  `NextMatchEntry` dekódoló típusokkal), `TeamSheet` két új kártyát kapott a
  fejléc alatt: „FORMA · UTOLSÓ 5 MECCS" (színezett W/D/L chipek, akadálymentes
  felolvasással) és „KÖVETKEZŐ MECCS" (dátum, verseny, ellenfél, H/V).

### Ellenőrzések
- Core: 65/65, benne `decodesTeamDetailWithFormAndNext`.
- `cargo fmt --all --check` OK, clippy `-D warnings` tiszta, workspace tesztek
  zöldek.
- `xcodebuild build` BUILD SUCCEEDED; daemon újratelepítve (port 56916).
- Élő API: `/v1/team/01a1da40-…(Real Madrid)` → form 5 (mind W), next 3.
- Élő UI: a Csapatok → Real Madrid adatlap – forma-chipek és következő meccsek
  (`/tmp/s57_team.png`).

## S58 — tabella-pozíció a meccs-részletezőben

### Változások
- `rust/crates/storage/src/repositories/match_detail.rs`: `MatchExtras` +
  `home_position`/`away_position`; a `fetch_match_extras` a meccs versenyének
  aktuális tabellájából (`fetch_competition_standings`) oldja fel a két csapat
  helyezését. A `MatchDetailRow` is megkapta a két opcionális mezőt; hiba esetén
  `None` (a részletező ugyanúgy működik).
- `/v1/match/{id}` válasz: `home_position` / `away_position`.
- Swift: `MatchDetail.homePosition/awayPosition`, a `ScoreboardHero` a csapatnév
  alatt az ELO mellé helyezés-jelvényt tesz (borostyán kapszula, pl. „2.",
  „13."), akadálymentes címkével.

### Ellenőrzések
- Core: 65/65 (`decodesNextMatches` kiegészítve a helyezés-dekódolással).
- `cargo fmt --all --check` OK, clippy `-D warnings` tiszta, workspace tesztek
  zöldek; `xcodebuild build` BUILD SUCCEEDED; daemon újratelepítve (port 57830).
- Élő API: `/v1/match/01a1e9e8-…` → Real Madrid 2., Celta 13.
- Élő UI: a Real Madrid–Celta részletezőn a helyezés-jelvények, forma, egymás
  elleni mérleg és következő meccsek együtt (`/tmp/s58_detail.png`).

## S59 — odds-frissítés a következő hétre + napi frissítés dokumentálása

### Adatműveletek
- A research odds pipeline lefutott 2026-11-21…28-ra (`football fetch`, `odds`,
  `api-export`); a 11-21-i napra 266 meccshez került piac (≈3 900 `fact.odds_quote`
  research forrásból), 11-22-re 41-hez, a többi napra 0–7-hez (a fogadóirodák
  távolabbra még nem adnak oddsot).
- A hét napjaira `odds-resync-<dátum>` research refresh jobok futottak
  (feldolgozva 7/7); a beragadt, lejárt lease-eket visszaállítottam, a hibás
  `rescan-*` soccerway jobokat lezártam (az „üres fetch-terv" és a hiányzó
  obscura-feature miatt).
- Új üzemeltetési útmutató: `docs/operations/daily-refresh.md` (research napi
  futás, v2 catch-up, odds-lefedettség értelmezése, hiány-panel/audit).

### Ellenőrzések
- Odds-lefedettség (publishable, 2026-11-21…28): 271/844, 34/396, 6/101, 1/149,
  1/147, 0/78, 1/141, 1/621 — a 11-21-i nap a legjobban fedett, a jövőbeli
  napok a források publikálási ütemét követik.
- A daemon a legfrissebb binárison fut (S58 végi állapot).

## S60 — élő meccsek csendes háttér-frissítése

### Változások
- `apps/FootballApp/DayModel.swift`: a betöltés két módra vált —
  `load()` (betöltés-jelzővel) és `refresh()` (háttér). A háttér-frissítés nem
  villantja fel a `loading` állapotot, hiba esetén pedig **megtartja** a már
  megjelenített listát, csak a `lastRefreshFailed` jelzőt állítja (a következő
  kör újrapróbálja).
- `apps/FootballApp/HomeView.swift`: ha a megjelenített napon van élő meccs, a
  nézet 60 másodpercenként csendben frissít (`model.refresh()`); a `.task(id:)`
  kulcsa a nap + az élő meccsek száma, így napváltáskor vagy élő→vége váltáskor
  a kör újraindul, illetve leáll.
- Tesztek: `apps/FootballAppTests/DayModelTests.swift` —
  `testRefreshKeepsLoadedContentAndSwallowsErrors` (háttér-hiba nem dobja el a
  listát), `testRefreshReplacesContent` (sikeres frissítés cseréli a tartalmat).

### Ellenőrzések
- `xcodebuild build` BUILD SUCCEEDED; `./scripts/run-ui-tests.sh` (unit + UI
  tesztek, fixture-szerverekkel) exit 0.
- Élő állapot: a 2026-11-21-i napon 3 élő meccs van, tehát a háttér-kör aktív.
- Megjegyzés: a frissítés ütemezése SwiftUI `.task`-ban fut, közvetlen
  vizuális bizonyíték (pl. hálózati számláló) nincs rá; a frissítés
  szemantikáját a fenti unit tesztek fedik.

## S61 — 2026-os történelmi duplikátumok és csapatnevek

### Változások
- **Történelmi audit:** a teljes 2026-os évre 12 905 meccs, 326 név-variáns
  klaszter, 200 dupla szereplés. A klaszterek 329 párja automatizálva merge-elve
  (`footballctl dedupe pair --survivor … --duplicate …`) használtság → verified
  gate → eredmény → provenance sorrendű túlélő-választással.
- **Csapat-alias merge-ek:** a maradék dupla szereplésekből 9 klubpár derült ki,
  ahol ugyanaz a klub két entitásként élt (pl. „1. FC Frankfurt (Oder)" ↔
  „Ein Frankfurt", „Jagiellonia II" ↔ „Jagiellonia Białystok"). A merge-ek
  alias-bejegyzést, standing/statisztika-átkötést, meccs-átsorolást és
  `core.entity.merged_into_id`-t írnak, audit `identity_team_merge`.
- **Kanonikus nevek:** Eintracht Frankfurt, 1. FC Nürnberg, Termalica Nieciecza,
  Paris Saint-Germain (a magyaros/rövid alakok alias-ként maradnak).
- **75 további meccs-merge** a csapat-merge-ek után azonossá vált sorokra.
- **Forrás-ellentmondások dokumentálva:** CR Bélouizdad (Ligue 1 vs CAF Confed
  Cup), KFUM Oslo (két Eliteserien-fixture), Schwarz-Weiß Bregenz (Austria vs
  First Vienna ellenfél ugyanabban az időben) — ezeknél a források maguk
  mondanak ellent, ezért merge nem biztonságos.
- **Hibás tartalék-bekötések javítva:** Eibar B, Las Palmas B, Śląsk Wrocław II
  entitás létrehozása/relink; seed „FA Kupa" demo sor editorial hold alá.
- **Hibás merge visszaállítva:** egy `--allow-loose` kör felnőtt/youth párt
  olvasztott össze (Real Madrid–PSV vs Real Madrid U19–PSV U19); a
  `ops.dedupe_archive` payloadból teljes visszaállítás történt (match +
  provenance + gate), audit `dedupe_merge_restore`.

### Ellenőrzések
- `footballctl audit --from 2026-01-01 --to 2026-12-31`: klaszter 0, dupla
  szereplés 9 → a javítások után 2–3, verseny-duplikátum 0 (ország-tudatos
  csoportosítás után).
- `reverify --from 2026-11-13 --to 2026-11-30`: 2612 provisional, 1779 verified,
  19 withheld.
- `cargo test -p football-storage --lib` 29 passed; `cargo test -p football-domain
  --lib` 9 passed.

## S62 — ország-tudatos audit, kupa-kivétel, holland duplikátumok

### Változások
- **Audit ország-tudatos verseny-duplikátum:** a `cluster_competition_rows`
  tiszta függvény országonként particionál; két valódi országot egy semleges sor
  nem köthet össze. Így a Ligue 1 FR/DZ/CD/BJ/SN nem hamis pozitív, viszont a
  XX (nemzetközi) duplikátumok továbbra is jelződnek.
- **Ország-kód kanonizálás bővítése:** `HOL → NL`, `SIN → SG`, `KVX → XK`,
  `CAN → CA`; a `core.country` 3 betűs sorai (CAN/HOL/KVX/SIN) auditáltan
  merge-elve a kétbetűs párba, a holland sor neve „Hollandia" (a forrás „NED"
  kód-neve nem írja felül többé).
- **Kupa-kivétel:** a `FOREIGN_TEAMS_DOMESTIC_COMPETITION` szabály nem tüzel
  kupa-versenyekben (Coupe de France, Taça de Portugal stb. tengerentúli
  résztvevői). A kupa-felismerés `football_domain::identity::looks_like_cup_competition`,
  a verseny betöltése (`worker/tick.rs`, `footballctl`) ad `None` országot.
- **Ingest név-védelem:** ha a forrás ország-neve kód alakú (pl. „NED"), az
  `upsert_country` nem írja felül a meglévő megjelenített nevet.
- **Holland duplikátumok:** az ország-merge után felszínre jött 3 duplikált
  verseny (Eerste Divisie, Eredivisie, Tweede Divisie) összevonva, 18 duplikált
  csapatpár merge-elve (De Graafschap, FC Den Bosch, FC Dordrecht, FC Emmen,
  FC Twente, FC Utrecht, FC Volendam, Helmond Sport, Heracles Almelo, HHC
  Hardenberg, MVV Maastricht, NAC Breda, RKC Waalwijk, Roda JC Kerkrade,
  TOP Oss, Vitesse, VVV-Venlo, Almere City FC) — a Jong- és W-entitások
  érintetlenek. A holland nap duplikált meccsei (`dedupe matches`) merge-elve.
- **Korosztály-guard szigorítása:** ha csak az egyik csapatnév hordoz
  korosztály-jelölőt („U19"), a másik oldal `age_group` mezőjének kell
  igazolnia ugyanazt — felnőtt és utánpótlás meccs soha nem olvad össze. A
  csapat-kontextus ellenőrzés tükrözött orientációban is helyes.

### Ellenőrzések
- `cargo test -p football-storage --lib` 29 passed (új teszt:
  `one_sided_age_marker_never_matches_a_senior_team`, flipped-orientációs
  fixture-ök javítva).
- `footballctl audit --from 2026-11-13 --to 2026-11-30`: klaszter 0, dupla
  szereplés 2 (CR Bélouizdad, SW Bregenz — forrás-ellentmondás), verseny-
  duplikátum 0, konzisztencia tiszta (0 eredmény nélküli lezárt, 0 ország
  nélküli csapat).
- `reverify` ugyanarra: 2612 provisional, 1779 verified, 19 withheld.

### Nyitott
- A 2 forrás-ellentmondásos dupla szereplés dokumentálva, további javítás csak
  új forrásból lehetséges.
- A 2026-os év 301-es „maradék" klaszter-listája (Jove Español, Res./Under 19/
  CC nevek) klubonkénti kézi döntést igényel — az automatikus tömeges merge
  nem biztonságos.
- 19 withheld (főleg CD/CG instabil forrás-országkód és hiányzó eredmény).

## S63 — történelmi évek duplikátum-tisztítása (2019–2025)

### Változások
- **Konzervatív marker-párok:** a 2025-ös audit 583 klaszteréből kiindulva a
  felnőtt csapatra tévesen felvitt tartalék-/utánpótlás-jelölős sorok
  („X Res.", „X II", „X Under 19", „X CC") duplikátumait a marker nélküli
  felnőtt sorba olvasztottuk (156 merge), majd a maradék azonos-fixture párokat
  (329 merge, majd 1) — 2025 így **0 klaszter / 0 dupla szereplés**.
- **2020–2024:** ugyanaz a pipeline (marker → azonos fixture → azonos név →
  azonos verseny+kickoff+egyező oldal): 2020: 330→3 klaszter, 2021: 513→6,
  2022: 539→12, 2023: 628→16, 2024: 537→11; a dupla szereplések 46/97/118/122/
  118 → 0/2/17/18/17.
- **Csapat-alias normalizálás:** 150 további azonos-klub pár merge-elve, ahol a
  különbség csak klub-prefix/szuffix (FC/CF/SC/SK/VfL…), ékezet, vagy
  tartalék-jelölés mindkét oldalon (pl. „Villarreal II" ↔ „Villarreal B",
  „Annecy" ↔ „FC Annecy", „Beveren" ↔ „SK Beveren"). A korosztály- és
  női-jelölők egyezése kötelező, a senior ↔ U17/U19/U21 és senior ↔ „X B/II"
  párok **nem** olvadnak össze. Az érintett 93 nap meccs-duplikátumai külön
  `dedupe matches` körrel merge-elve.

### Ellenőrzések
- 2025: `audit` → 0 klaszter, 0 dupla szereplés, 0 verseny-duplikátum.
- 2026: 0 klaszter, 3 dupla szereplés (forrás-ellentmondás), 0 verseny-duplikátum.
- 2027: 0 klaszter, 0 dupla szereplés.
- `reverify 2026-11-20…22`: 960 provisional, 521 verified, 6 withheld; a napi
  feed `open_past` 6 → 2 (a hátralévő kettő valóban most kezdődött/élő).

### Nyitott
- A 2020–2024 években maradt néhány tucat olyan klaszter, ahol a v1-adat két
  különböző klubot nevezett meg ugyanarra a fixture-re (pl. „Jove Español" /
  „Espanol", „Quevilly-Rouen Métropole" / „Rouen") — ezek klubonkénti,
  forrás-alapú döntést igényelnek; a látható (2026-os) adatot nem érintik.
- A történelmi „dupla szereplés" maradékok (más-más ellenfél ugyanazon a napon)
  forrás nélkül nem dönthetők el, dokumentálva.
