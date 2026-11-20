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
