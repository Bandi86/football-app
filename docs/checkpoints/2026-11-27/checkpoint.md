# Checkpoint — 2026-11-27

## Induló állapot

- A stabil állapot a `docs/checkpoints/2026-11-26/checkpoint.md`-ben (S185 — az
  adatmegjelenítési terv S0–S8 szeletei kész, S9 külön döntésre vár) és a
  `README.md`-ben olvasandó.
- `git status`: `main...origin/main`, branch `main`. Az RFC-spec
  (`2026-11-23-modern-predikcios-rendszer-terv.md`) továbbra is nem commitolt,
  érintetlen.
- Felhasználói észrevétel: „a filter nem jelzi rendesen, ha hiányzik az eredmény"
  — a tegnapi (11-26) és mai (11-27) adat, illetve a szűrők átvizsgálata.

## S186 — Napi check + szűrővizsgálat (read-only, adatot nem írt)

- [x] `./scripts/daily-check.sh 2026-11-26` → **EXIT=0, ZÖLD**.
  - 11-26: 104 meccs / 101 lezárt / 101 eredmény / 91 odds; hiányok:
    `missing_team_logo 16`, `missing_competition_logo 48`, `missing_odds 13`;
    `open_past 0`, `missing_result 0`, `missing_kickoff 0`. Integritás OK,
    reverify 102 meccs / 0 hiba, verified gate 14.
  - 11-27: 232 meccs / 4 lezárt / 4 eredmény / 93 odds; hiányok:
    `missing_team_logo 66`, `missing_competition_logo 166`, `missing_odds 139`;
    `open_past 0`, `missing_result 0`. A legkorábbi kezdés 08:30Z (10:30 Budapest),
    tehát a méréskor (07:52Z) még egy meccs sem járt le — a 227 „Következik"
    rendben.
- [x] A `gaps` mező és a kliens szűrői **meg egyeznek** mindkét napon. Az API
  `/v1/day` adatából a Swift `MatchFilter` szabályait utánozva számolt darabszámok
  és a futó app AX-címkéi azonosak: 11-26 → Mind 104 / Befejezett 101 / Eredmény
  nélkül 0 / Odds nélkül 13; 11-27 → Mind 232 / Következik 227 / Befejezett 4 /
  Eredmény nélkül 0 / Odds nélkül 139.
- [x] **A gyanús filter-hiba nem igazolódott**: a `missingResult` szűrő definíciója
  (`finished` és nincs végeredmény) következetesen van implementálva a kliensben
  (`MatchListView.swift`, `MatchCardModel.swift`) és a backendben
  (`storage/repositories/day.rs`: `status='finished' AND home_ft IS NULL`); a
  `home_score` API-mező az `r.home_ft`-ből jön. A 11-26-os hat 0:0-s lezárt meccs
  eredménye a research forrásban is 0:0 (FT + félidő adattal), nem placeholder.
- [x] **Amit a felhasználó látott — elavult pillanatkép, nem szűrőhiba**: a futó app
  eleinte 103 meccset / 100 lezártat / 1 „Idő nélkül"-t mutatott, miközben az API
  már 104 / 101 / 0-t. A „Frissítés" gomb AXPress-szel azonnal az API-val egyező
  számokra állt. A napi lista 5 perces csendes háttérkörét a `.task(id:)`
  indítja; a képernyő gyors ellenőrzése nem mutatott újraindulást a régi napon —
  **nem zártuk le**, hogy a timer háttérben/szuszpendálva miért nem frissített.
- [x] Kapuk (mért): `MatchFilterTests` + `MatchCardModelTests` → **27 teszt,
  0 hiba, TEST SUCCEEDED**; `daily-check 11-26` EXIT=0; `psql` read-only
  (1 `abandoned` meccs 2026-12-09-re, nem a két érintett napon; a 2 failed job
  a régi `catchup-2026-11-22` és `slot-03`, nem mai keletkezésű).
- [ ] **Nem ellenőrzött**: az 5 perces háttér-timer viselkedése hosszabb
  háttérben tartózkodás után; a tényleges VoiceOver-hang (a korábbi checkpointok
  óta nyitott); a `missing_result` hiányjel-kód az `ops.issue` detektorban
  jelenleg **nincs** (`OpsIssueModels`/`detect.rs` — ha az eredményhiányt is a
  hiánypanelbe szeretnénk, ez külön szelet).

## Döntések / korlátok

- Az `Eredmény nélkül` szűrő a lezárt, de eredmény nélküli meccseket jelenti —
  **nem** a még el nem kezdett vagy lejárt, nyitott meccseket (azok a
  `Következik`, illetve `Lejárt, nyitott` szűrőben vannak, S163). Ez a
  megkülönböztetés tudatos, mind a backend, mind a kliens így viselkedik.
- A napi check a hiányjeleket írja (`ops.issue`); 11-26-ra 61 jel rögzítve
  (48 logó / 13 odds), 0 lezárás. Ez nem szűrőhiba, hanem a detektor normal
  működése.

## S187 — Aktíváláskori frissítés (a felhasználó által választott szelet)

- [x] **Döntés:** a felhasználó a mai első szeletnek az „elavult kliensfrissítést"
      választotta. A kivizsgálás szerint a kliensnek **nincs** alkalmazás-aktíválódási
      frissítése: csak a `.task(id: autoRefreshKey)` 5/1 perces `Task.sleep` köre
      fut, ami háttérben/szuszpendálva nem garantált. A felhasználó által látott
      elavult számok (103/100/1 ↔ API 104/101/0) pontosan egy ilyen kihagyott
      körnek felelnek meg; kézi „Frissítés" után azonnal korrigált.
- [x] **Core:** `AutoRefreshPolicy` kibővül `AppLifecyclePhase`-szel és a
      `shouldRefreshOnActivation(from:to:)` tiszta döntéssel (background/inactive →
      active = igaz; minden más = hamis). 2 új teszt; Core **155 passed, 0 failed,
      27 suite** (előtte 153/26).
- [x] **Kliens:** `HomeView` figyeli a `@Environment(\.scenePhase)`-t, és
      aktíválódáskor `model.refresh()`-t indít (nem villant betöltőt, nem dobja el
      a meglévő listát); a SwiftUI `ScenePhase` → Core `AppLifecyclePhase`
      leképezés privát extensionben, `@unknown default`-del. Az eredeti 5/1 perces
      kör változatlanul fut.
- [x] **Kapuk (mért):** `db-reset.sh` (TEST_DATABASE_URL-t sourcolva) →
      `check-foundation.sh` EXIT=0 · Core **155/0** · app unit **94/0 TEST
      SUCCEEDED** · `run-ui-tests.sh` **31/31, 0 hiba** · Rust `cargo test
      --workspace -- --test-threads=1` **635 passed, 0 failed** · clippy
      `-D warnings` **0** · `fmt --all --check` **0** · build `--features obscura`
      **OK** · `daily-check 2026-11-26` **EXIT=0 ZÖLD** · `git diff --check` **0**.
- [x] **Valódi macOS:** `install-football-app.sh` → **BUILD SUCCEEDED**; az app
      újraindítva (PID **11199**); Finder→FootballApp háttér/előtér ciklus után
      PID stabil, AX: `Mind 232 / Következik 227 / Befejezett 4 / Eredmény nélkül 0`
      — változatlan, egyezik az API-val.
- [ ] **Nem ellenőrzött**: az aktíválás által kiváltott hálózati kérés **közvetlenül
      nem lett mérve** (a daemon nem naplózza a kéréseket, a fejléc nem mutat
      „utolsó frissítés" időt); a döntést a Core egységtesztje és a bekötött kód
      igazolja. A 11-26-i adat időzítése (103→104 a daemon indulása után) nem
      lett pontosan audit-eseményhez kötve — a javítás a tünetet függetlenül
      orvosolja. A tényleges VoiceOver-hang és az 5 perces kör hosszú háttérben
      tartózkodás utáni viselkedése továbbra is nyitott.

### Visszaállás (S187)

- `git revert <S187 commit>` — a Core döntés és a `HomeView`-bekötés megszűnik;
      az API, az adatbázis és a research slotjai érintetlenek.
- Az app bármely commitból újraépíthető az `install-football-app.sh`-vel.

## S188 — `missing_result` hiányjel (a felhasználó által választott szelet)

- [x] **Kivizsgálás**: a detektor már jelzett `finished` + nincs eredmény esetet,
      de a félrevezető `stale_status` ("Elavult státusz") kód alatt; az
      `ops.issue.code` CHECK-je nem engedte a `missing_result` értéket. A napi gap
      és a kliens szűrője viszont `missing_result` / „Eredmény nélkül" néven
      szerepel — ez volt a következetlenség.
- [x] **Migráció `0030_missing_result_issue_code.sql`**: a `issue_code_check`
      kibővül `missing_result`-tal; a régi `stale_status` a visszaféle
      kompatibilitás miatt engedélyezett marad, de a detektor nem írja.
      Migráció **előbb a teszt-, majd a dev DB-re**; a constraint_def ellenőrizve.
- [x] **Backend**: `domain/ops.rs` új `IssueCode::MissingResult` (`as_db_str`,
      `parse`, `label_hu` = „Eredmény nélkül"); `detect.rs` ezt írja Hard
      súlyossággal, `kickoff_at`-tal a detailben. Az API-tesztben a
      `code=missing_result` most érvényes kód (200), a hibás kód `not_a_gap`.
- [x] **Kliens**: `OpsIssueModels` új `.missingResult`, `OpsFormatting` magyar
      címke, `OperationsView` hiány-szűrője és `issueCode(from:)` bővítve.
- [x] **Tesztek**: új storage-integrációs teszt (lezárt meccs eredmény nélkül →
      `MissingResult`, nem `StaleStatus`, más napra nem), új Core dekódolós-
      és címketest, domain round-trip bővítve.
- [x] **Kapuk (mért)**: `db-reset` → `check-foundation` EXIT=0 · Rust
      **636 passed, 0 failed**, clippy 0, fmt 0, build `--features obscura` OK ·
      Core **156/0 (27 suite)** · app unit **94/0** · `run-ui-tests.sh` **31/31,
      0 hiba** · `daily-check 11-26` **EXIT=0 ZÖLD** · `integrity 11-25…11-26`
      **verdict OK** · `git diff --check` 0.
- [x] **Éles**: `install-footballd-agent.sh` → `/v1/health` OK; a futó daemon
      `GET /v1/issues?code=missing_result` → **200, 0 jel**; érvénytelen kód →
      **400**. `footballctl issues detect --date 2026-11-26 --apply` (repo CLI)
      61 jel, 0 lezárás; a DB-ben továbbra is 0 `stale_status` és 0
      `missing_result` — a két napon nincs eredmény nélküli lezárt meccs, ezért
      élesben nem keletkezett új jel.
- [ ] **Nem ellenőrzött**: a `missing_result` jel **éles adaton** nem volt
      megfigyelhető (a teljes DB-ben 0 `finished` eredmény nélkül); a kódot az
      integrációs teszt fedi. A kliens szűrőt az XCUITest nem kattintja végig az
      új kódra (csak a dekódolás és a címke van tesztelve); a tényleges
      VoiceOver-hang nyitott marad.

### Visszaállás (S188)

- `git revert <S188 commit>` + a `0030` migráció visszaállítása: a CHECK
  visszaállítása a hét elemű listára, a `_sqlx_migrations` 0030 sorának törlése;
  `missing_result` jelek esetén előbb `resolved_at` (jelenleg nincs ilyen sor).

## S189 — Becsült óra átvétele a worktree-ből

- [x] **Döntés:** a felhasználó a következő szeletnek a
      `feat/estimated-match-clock` worktree becsült óráját választotta.
- [x] **Worktree-review (csak olvasás):** a branch a `main` régebbi bázisáról
      készült (`openPastThreshold` 3 óra, `liveWindow` 6 óra), ezért a kódot
      **nem másoltuk át**: a `main` 10 perces / 150 perces szabályaira és az
      `isResultPending` / „EREDMÉNY VÁRHATÓ" viselkedésre adaptáltuk. A worktree
      más, nem kapcsolódó változásait (11-24 checkpoint, sortMenü-teszt,
      `app.activate()` — utóbbi már a `main`-ben van) nem vettük át.
- [x] **Core/UI:** `MatchCardModel.ClockDisplay` + `clock(at:)` (forrás minute-ja
      vagy 45+15+45 becslés, csak a 150 perces ablakban), `phaseLabel(at:)`,
      `displayStatus(at:)`; a `main` stale-live szabálya változatlan. A kártya
      kezdési ideje helyett becsült órát és „becsült" jelzést mutat; az
      `AccessibilityLabels.matchCardLabel` „becsült 10. perc" / „becsült félidő"
      / „becsült 90. perc vagy ráadás" alakban olvassa fel. 30 mp-es
      `Timer.publish` frissíti a kártyákat.
- [x] **Fixture/teszt:** új `clock-day.json` + `--relative-kickoffs` a fixture
      szerverben, új 18083-as port, 1 XCUITest; Core +3 AX-teszt, app unit +7
      óra-teszt (határok 0/44/45/59/60/104/105/149/150 perc, félidő, forrás-minute,
      date_only, lezárt státuszok, stale-live megőrzése).
- [x] **Kapuk (mért):** `db-reset` → `check-foundation` EXIT=0 · Core **159/0
      (27 suite)** · app unit **101/0** · `run-ui-tests.sh` **32/32, 0 hiba**
      (az új óra-teszt 5,4 mp alatt zöld) · Rust **636 passed, 0 failed**,
      clippy 0, fmt 0, build OK · `daily-check 11-26` **EXIT=0 ZÖLD** ·
      `integrity` **OK** · `git diff --check` 0.
- [x] **Valódi macOS:** `install-football-app.sh` BUILD SUCCEEDED, app újraindítva
      (PID **18591**), AX: `Mind 232 / Következik 227 / Befejezett 4 / Eredmény
      nélkül 0`. A képernyőn látható kártyákon jövőbeli kezdések vannak, ezért
      óra nem jelent meg rajtuk; a becsült óra viselkedését az XCUITest ellenőrzi
      valódi macOS-en (fixture-szerverrel).
- [ ] **Nem ellenőrzött**: a becsült óra **élő, lejárt-startú meccsen** nem
      lett szemmel/AX-szel végignézve (a megfelelő kártyák a listában görgetéssel
      elérhetők, az AX-fa csak a láthatókat tartalmazza); a Timer 30 mp-es
      újraértékelését nem vártuk végig; VoiceOver-hang emberi füllel nyitott.
- [ ] A worktree három másik mappája (`estimated-match-clock` kivételével) és a
      worktree saját, nem commitolt fájljai érintetlenek — a forrás worktree-be
      nem írtunk.

### Visszaállás (S189)

- `git revert <S189 commit>` — a becsült óra és a fixture/harness-változások
      megszűnnek; a `main` 10/150 perces és stale-live szabályai változatlanok,
      adatot a szelet nem írt.

## S190 — Két régi failed job diagnosztikus lezárása

- [x] **Döntés:** a felhasználó a napi munka lezárása előtt a két régi failed
      job rendezését választotta (újraingestelés **nélkül**).
- [x] **Diagnózis (read-only):**
  - `slot-03` (created 11-24 07:31, 4 kísérlet, `time_window=2026-11-24`):
        „dátum-guard eltérés: kért 2026-11-24, kapott 2026-11-25". A 11-24-i
        napi JSON azonban helyes (`meta.date=2026-11-24`) — a régi kód a
        `time_window` helyett a mai dátumot ellenőrizte. Az **S179** óta a slot
        job a `time_window`-ból olvas (regressziós teszttel).
  - `catchup-2026-11-22` (created 11-25 07:31, 1 kísérlet,
        `entity_key=catchup-2026-11-22`): a hivatkozott
        `matches/2026/11/22/2026-11-22_bendel-insurance-vs-doma-united_fm6103183.json`
        a forrásban **nem létezik** (a meccs maga a DB-ben van: `scheduled`).
        A régi kód egy hiányzó detail miatt az egész kört megbuktatta; az
        **S179** óta warning + skip (forrás: `ingest/research.rs:420-427`).
- [x] **Végrehajtás (tudatos, auditált revízió — nem migráció, nem törlés):**
      a két job státusza `failed → cancelled`; az eredeti `last_error` megmarad,
      hozzáfűzve az S190-es ok és a visszaállítási út. Két `ops.audit_event`
      (`actor=ops`, `action=job_cancel_stale`, `subject_kind=job`,
      `data_changed=false`) ugyanabban a tranzakcióban. Kanonikus adatot,
      `ops.issue`-t vagy research fájlt a szelet **nem írt**.
- [x] **Kapuk (mért):** `daily-check 2026-11-26` **EXIT=0 ZÖLD**, `failed job:
      0` (előtte 2, majd 1), `pending job: 8`; `integrity 11-25…11-26`
      **verdict OK**; `git diff --check` 0; a repo munkaterülete tiszta.
- [ ] **Nem ellenőrzött**: a hiányzó 11-22-i detail-fájl forráshoz kötődő
      eredete (miért nem lett letöltve) nem nyomoztuk ki; új forrásengedély
      nélkül nem pótolható. A 11-22-i meccs eredménye továbbra is hiány —
      ez nem a job-státusz változtatásának mellékhatása.

### Visszaállás (S190)

- `UPDATE ops.job SET status='failed' WHERE id IN ('01a20c40-…','01a21166-…');`
  és a két `job_cancel_stale` audit-esemény törlése (vagy megőrzése — a
  visszaállás nem követeli meg). A kanonikus adatot a szelet nem érintette.

## S191 — Tipplap S1: négy versenycím + piac-katalógus és parser

- [x] **Előzmény:** a felhasználó kérésére a Tipplapot kiegészítő forrásként
      kezeljük. Három döntés: (1) csak a mért hiány versenyei, (2) kvóta emelése
      és minden linkelt meccs markets-lekérése, (3) review-alapú, fail-closed
      alias-kezelés. Rögzítve: `docs/decisions/2026-11-27-tipplap-kiegeszito-forras.md`
      (`73609e3`); spec + terv ugyanott jól láthatóan (`S0`).
- [x] **Adapter:** `GENERAL_LEAGUES` 26 → **30**: `Walesi 2., Déli csoport` →
      `Cymru - dél` (GB-WLS), `Walesi 2., Északi csoport` → `Cymru - észak`
      (GB-WLS), `Szerb bajnokság` → `Super Liga` (RS), `Ománi Szultán Kupa` →
      `Sultan Cup` (OM). A GB/Scotland ghost-párt nem másoltuk. A két walesi és
      a szerb entitás már létezik a DB-ben; az ománi kupa új, reviewed verseny,
      amelyet az első sikeres ingest hoz létre a szokásos `find_or_create`
      úton — nem fuzzy, nem `create_on_miss` találgatás.
- [x] **Piac-katalógus:** `MarketKind` + `market_for()` + `market_line()` +
      `market_outcome()` — 12 piacfajta (1x2, dupla esély, DNB, BTTS, OU,
      gólszám-sáv, hendikep, hazai/vendég clean sheet, pontos eredmény, félidő
      1X2/dupla, other). A „Kétesély" **nem** 1X2, a dupla esély kimenetelei
      `1x/12/2x`; a hazai/vendég clean sheet külön market, hogy ne ütközzön.
      Ismeretlen piac → `other` + eredeti név (adatvesztés nincs).
- [x] **`parse_markets()`**: a markets JSON `desktop` fragmentjéből olvas,
      `<table><tbody>` burkolatban (különben a HTML5 parser a `<td>`-ket
      eldobja), soronkénti kód/kimenetel testvér-cellákkal, `data-outcome-id`
      szerinti deduplikációval; hiányzó/üres `desktop` vagy 0 odds →
      `SchemaDrift` (fail-closed). A `mobile` fragment duplikátum, nem adatforrás.
- [x] **Tesztek (mért):** +4 unit teszt (4 új cím GB-WLS/RS/OM ellenőrzéssel,
      14 piac-leképezési eset, magyar tizedesvessző/hendikep-vonal, parser +
      dedup + fail-closed). `cargo test -p football-ingest --lib` **69/0**;
      teljes workspace **640 passed, 0 failed**; clippy 0, fmt 0, build OK.
- [x] **Valódi formátum ellenőrzése:** egy élő markets JSON
      (`event 5621252`, read-only) **14 piaccsoportot / 36 kimenetelt** tartalmaz
      (Kétesély, DNB, BTTS, Gólszám 1,5/2,5/3,5, Hendikep 0:1/1:0/2:0,
      clean sheet hazai/vendég, 1. félidő 1X2/Kétesély) — mindet felismeri a
      katalógus. A nyers payload nem került a repóba, csak kézzel írt fixture.
- [x] **Kapuk:** `check-foundation` EXIT=0 · `daily-check 11-26` EXIT=0 ZÖLD ·
      `integrity 11-25…11-27` OK · `git diff --check` 0.
- [ ] **Nem ellenőrzött:** az új címek még **nem élnek a futó daemonban**
      (S2-ben telepítjük újra); a 4 új verseny kanonikus entitását a következő
      ingest fogja létrehozni/frissíteni, éles mérés S2/S3; a markets jobok és
      a kvóta emelése még nem készült.
- [ ] A `Sultan Cup` létrehozásának döntése dokumentált, de a verseny-entitás
      csak az első sikeres forrás-sorral jön létre; ha a linker közben review-t
      nyit, a meccs kimarad (fail-closed) — ez a kívánt viselkedés.

### Visszaállás (S191)

- `git revert <S191 commit>` — a szótár és a piac-katalógus visszaáll; a
  kanonikus adatot a szelet nem írta, a daemon még nem fut új kóddal.

## S192 — Tipplap S2: markets jobok, kvóta és odds-írás

- [x] **Policy (dev DB, SQL):** `tipplap.daily_budget` **8 → 150**, adapter
      `1.0 → 1.1`, parser `1.1 → 1.2`; `domains` változatlan (`["day"]`),
      mert a markets domain sablonjait szándékosan nem generáltatjuk a
      `generate_jobs` általános ágával. Visszaállás:
      `UPDATE ops.source_policy SET daily_budget=8, adapter_version='1.0',
      parser_version='1.1' WHERE source_slug='tipplap'`.
- [x] **Adapter:** `TipplapAdapter::plan()` a `markets-{event_id}` jobra
      `/kalkulator/markets/{event_id}` URL-t ad (`request_key=
      tipplap:markets:{id}`), `parse()` a markets JSON `desktop` fragmentjéből
      egy `match_odds` observationt emitál, ugyanazzal a `source_record_key`-vel
      mint a napi 1X2. A piac-kulcsok: az API által használt kanonikus piacok
      (`1x2/btts/ou/dnb/double_chance/clean_sheet`) változatlanok, minden más
      piac a forrás `outcome-id`-jével egyedített (`other:<id>`, `handicap_-1:<id>`),
      hogy a `fact.odds_quote` PK-ja ne ütközzön. A negatív hendikep-vonal
      előjele a marketbe kerül (DB CHECK: line >= 0). Magyar tizedesvesszőből
      bármely vonal (`0,5 / 2,5 / 10,5`) felismerhető.
- [x] **Scheduler:** `repo::linked_tipplap_event_ids()` kizárólag a mai napon
      megfigyelt ÉS aktív kanonikus meccs-reffel rendelkező eseményeket adja;
      a `tipplap` policy-n `markets-{event_id}` jobok keletkeznek
      (`domain=markets`, `event_type=refresh`, 5 mp-es szétütemezéssel).
      A review-ra várók kimaradnak (fail-closed).
- [x] **Worker:** a markets domain nem megy át a napi verifikációs úton;
      `repo::active_match_ref()` oldja fel a meccs-reffel (a meccsek nem
      `core.entity` sorok, ezért az `identity::active_ref` nem használható),
      majd `write_market_odds()` írja az egész odds-blokkot. A trust
      provisional marad; a verifikáció és a publish gate többi forrásra
      változatlan.
- [x] **Tesztek (mért):** +3 új teszt (adapter plan és markets parse,
      scheduler only-linked), teljes workspace **643 passed, 0 failed**,
      clippy 0, fmt 0, build OK, check-foundation 0, `daily-check 11-26`
      EXIT=0 ZÖLD, `integrity 11-25…11-27` OK, `git diff --check` 0.
- [x] **Éles mérés:** daemon újratelepítve (`footballd` 09:52-től); a logistics
      látott hibák után (negatív vonal / PK-ütközés) a javított kóddal a markets
      jobok `written=1`-gyel futnak; a Tipplap odds-snapshotok száma
      **1921**, legnagyobb `market_count` **651**, 805 külön piac;
      `btts yes/no`, `ou 1,5/2,5/3,5` és számos további piac bekerült.
      `failed job: 0` a napi ellenőrzésben. A korábban elbukott 12 markets
      job `pending → újra feldolgozva` (auditált SQL, nem törlés).
- [ ] **Nem ellenőrzött:** a 18 még pending markets job végigfutása (kb.
      18 perc, 1 kérés/perc a delay miatt); a 4 új versenycím éles betöltése
      (a mai `discovery` még a régi binárissal futott, ezért holnap 07:30
      után mérhető); a `btts` piac néhány összetett neve (pl. „BTTS + gólszám
      2,5") a `btts` kategóriába került eltérő outcome-kóddal — adatvesztés
      nincs, de a katalogizálás finomítható; a photon/odds UI nem mutatja
      az új piacokat (csak 1x2/btts/ou jelenik meg) — külön UI-szelet.
- [ ] A normál napi kvóta most 150 requests/nap; a 429/breaker viselkedést
      nem provokáltuk (nem szabad), a meglévő fail-closed őrök változatlanok.

### Visszaállás (S192)

- `git revert <S192 commit>` + a fenti policy-SQL visszaállítása; a már írt
      odds-sorok a `write_odds()` upsertjével újrajátszhatók, a raw/observation
      és audit sorok nem törlődnek. A markets jobok `cancelled` státuszba
      tétele leállítja a további kéréseket.

## S193 — Tipplap S3: napi ellenőrzés és üzemeltetési dokumentáció

- [x] **`daily-check.sh`** új, read-only szakasza: `linkelt mai esemény`,
      `markets job (done/failed/pending)`, `gazdag odds-snapshot (>1 piac)` és
      `függő tipplap névreview`. A markets sikertelensége figyelmeztetés, nem
      kemény kapu (429/breaker esetén újraütemezés), a review-szám megjelenik.
- [x] **Dokumentáció:** `docs/operations/tipplap.md` (kvóta 150, markets domain,
      4 új versenycím, piac-leképezési tábla, visszaállás) és
      `docs/research/source-catalog.md` (S191–S192 státusz) frissítve.
- [x] **Éles mérés (napi check, 2026-11-27):** linkelt mai esemény **81**,
      markets job **15 done / 0 failed / 12 pending**, gazdag Tipplap
      odds-snapshot **8**, függő névreview **2**. `daily-check 11-26` EXIT=0;
      `integrity 11-25…11-27` OK; teljes workspace **643 passed, 0 failed**,
      clippy 0, fmt 0, build OK, `check-foundation` 0, `git diff --check` 0.
- [ ] **Nem ellenőrzött:** a 12 pending markets job végigfutása; a 4 új
      versenycím első éles betöltése holnap 07:30 után; a 2 review-ra váró
      név emberi eldöntése (`footballctl identity review/approve`); a részletes
      piacok UI-megjelenítése és a VoiceOver-hang külön szelet/döntés.
- [ ] A markets piacok száma egy meccsen 651 is lehet (ismeretlen piacok
      outcome-ID szerint egyedítve) — ez adatvesztés nélküli, de a
      `market_count` nem azonos a klasszikus „piacfajták" számával.

### Visszaállás (S193)

- `git revert <S193 commit>` — a napi check és a docok visszaállnak; adatot,
      jobot, odds-ot a szelet nem írt.

## S194 — Részletes odds-piacok a meccsoldalon

- [x] **Döntés:** a felhasználó a predikció előtt az új Tipplap-piacok UI-ját
      választotta (a mai checkpoint zárópontja). A predikciós RFC
      (`2026-11-23-modern-predikcios-rendszer-terv.md`) és a v1 referencia
      áttekintve: a v1 aktív v4 modell Brier/3 ~0,20, de **minden piacon a
      piac volt jobb**, a `value` ROI negatív, a mérőréteg pedig hibás —
      ezért a predikció Phase 0 előfeltétele a mérési szerződés.
- [x] **API/storage:** új `GET /v1/match/{id}/markets` (read scope, 400/401/
      404/500, OpenAPI 27 → **28** útvonal), `repositories/match_markets.rs`
      a teljes `fact.odds_quote` blokkot adja piaconként legfrissebb
      `captured_at`-tal, forrás-kóddal. A fő `/v1/match` válasz változatlan.
- [x] **Core/UI:** `MarketQuoteDTO`, `MarketsResponseDTO`, `MarketDisplay`
      (12 magyar piaccímke + ismeretlen piac megőrzése), `HTTPMatchMarketService`;
      a `MatchDetailModel` a fő adattól függetlenül tölti. Az Odds fülben
      `MarketOddsList` csoportosítva, keresővel és 10 csoportos kezdeti
      korláttal („Többi piac megjelenítése") mutatja a sorokat; AX-címkék
      magyarul, forrással.
- [x] **Tesztek (mért):** +2 API-teszt (auth/404/üres + valódi piacok),
      +4 Core-teszt (dekódolás, csoportosítás, címkék), +1 XCUITest;
      teljes workspace **645 passed, 0 failed** · Core **163/0 (28 suite)** ·
      app unit **101/0** · `run-ui-tests.sh` **33/33** (az új teszt zöld;
      egy régi Ops-dátumválasztó-teszt piros — lásd lent) · clippy 0, fmt 0,
      build OK · `daily-check 11-26` EXIT=0 · `integrity` OK · `git diff --check` 0.
- [x] **Éles:** daemon + app újratelepítve, `/v1/health` OK; egy valódi meccs
      `/markets` válasza **676 quote / 653 külön piac**, első sor:
      `1x2 / away / 4,10 / forrás research`. A korábban láthatatlan 651 piac
      most elérhető a kliensben is.
- [ ] **PIROS kapu — nem regresszió (A/B-vel igazolva):** a teljes UI-futtatás
      33 tesztből 1 piros: `testOperationsSummaryDatePickerHasHungarianLabelAndOptions`
      („Not hittable: ops.summary.date"). Ugyanaz a hiba a **`21f4224` alap-commit
      worktreejében** is reprodukálható (A/B), tehát környezeti/XCUITest-
      geometria, nem az S194 okozza. Egy másik régi teszt
      (`testCompetitionTeamRowsOpenTeamSheet`) az egyik futtatásban flaky
      volt, izoláltan zöld.
- [ ] **Nem ellenőrzött:** a 676 soros lista görgetéssel/keresővel való
      emberi végigkattintása; VoiceOver tényleges hang; a `btts` összetett
      piacnevek katalogizálása; a 2 függő névreview és a 9 pending markets job.

### Visszaállás (S194)

- `git revert <S194 commit>` — az új végpont, Core modell és UI-lista
      megszűnik; a fő meccs-válasz, az odds-adatok és a daemon/worker
      változatlanok.

## S195 — Predikció P0a: Shin + mérési szerződés + as-of őr

- [x] **Döntés:** a felhasználó a piacok UI (S194) után a **mérési szerződés +
      Shin** szeletet választotta, a Dixon–Coles modell elé. Indoklás: a v1
      saját auditja szerint a modell minden piacon rosszabb volt a piacnál, a
      `value` ROI negatív és a mérőréteg (CI nélküli verdikt) nem tudta hitelesen
      megítélni az új modelleket. Terv:
      `docs/superpowers/plans/2026-11-27-predikcio-phase0.md`.
- [x] **Új crate:** `football-prediction` (workspace `crates/*` automatikusan),
      adatbázis-/hálózatmentes, tiszta Rust.
  - `margin.rs`: determinisztikus Shin-biszekció a `p_i = [√(z²+4(1-z)p_i²/S)−z] /
    (2(1-z))` formulára, `fair_odds`, `edge = p·odds−1`; üres/nem pozitív odds
    fail-closed `None`.
  - `metrics.rs`: multi-class Brier (opcionális `/klass`, a v1 `Brier/3`
    konvenciója), log-loss, Wilson-intervallum, **klaszterezett bootstrap**
    páros Brier-különbségre (fix xorshift seed, 2,5/97,5 percentilis).
  - `as_of.rs`: szigorú `captured_at < kickoff_at` őr, `date_only` placeholder
    elutasítása, teljes feature+predikció-idő ellenőrzés.
- [x] **Tesztek (mért):** 13 új unit teszt (Shin-összeg=1, favorite-longshot
      torzítás, 2 utas/nincs margin, hibás odds, Brier/log-loss értékek,
      Wilson, bootstrap szignifikáns és zajos eset, as-of). Teljes workspace
      **658 passed, 0 failed**, clippy 0, fmt 0, build OK.
- [x] **Kapuk:** `check-foundation` érintetlen · `daily-check 11-26` EXIT=0 ZÖLD ·
      `integrity 11-25…11-27` OK · `git diff --check` 0. A szelet nem írt
      adatot, nem módosított sémát, nem indított modellt.
- [ ] **Nem ellenőrzött**: a Shin-implementációütköztetés a v1 `marketcalib.ts`-vel
      (csak a matematikai tulajdonságok vannak tesztelve); valódi odds-blokkon
      mért margin/edge; a bootstrap statisztikai kalibrálása szimulált adaton
      túl; a `football-prediction` még nincs bekötve CLI-be vagy API-ba.
- [ ] A P0b (analytics séma + `footballctl prediction measure`) és a P1
      (Dixon–Coles) nem indult — külön szeletek.

### Visszaállás (S195)

- `git revert <S195 commit>` — a `football-prediction` crate és a Phase 0 terv
      megszűnik; a meglévő crate-ek, adatok és API változatlanok.

## Szakasz zárása — 2026-11-27 (S186–S195)

A reggeli adat- és filter-ellenőrzéstől a predikciós alapkönyvtárig tartó
tíz szelet mindegyik saját kapulánccal zöld, commitolva és pusholva:

| Szelet | Eredmény | Commit |
|---|---|---|
| S186 — napi check + szűrővizsgálat | a feltételezett filterhiba nem igazolódott; elavult klienskép | — (vizsgálat) |
| S187 — aktíváláskori frissítés | `scenePhase` → azonnali refresh | `121bfd0` |
| S188 — `missing_result` hiányjel | detektor + API + Üzemeltetés egységes | `256db0d` |
| S189 — becsült óra | worktree-funkció a `main` 10/150 perces szabályaira adaptálva | `141ace7` |
| S190 — régi failed jobok | 2 job diagnosztikusan lezárva, auditált | `61ed775` |
| S0 → S191–S193 — Tipplap kiegészítő forrás | 4 versenycím, 653 piac, kvóta 150, napi check, doc | `73609e3`, `77fcfde`, `c52a586`, `21f4224` |
| S194 — részletes piacok UI | `/v1/match/{id}/markets`, Odds fül, 676 quote mérve | `870dce6` |
| S195 — predikció P0a | `football-prediction`: Shin + mérési szerződés + as-of | `820e3a1`, `362d1f9` |

**Záró kapuk (utolsó mérés):** Rust **658 passed, 0 failed** · clippy 0 · fmt 0 ·
build OK · Core **163/0** · app unit **101/0** · UI **33/33** (egy régi Ops-
dátumválasztó-teszt A/B-vel alapon is piros, környezeti) · `daily-check 11-26`
EXIT=0 ZÖLD · `integrity 11-25…11-27` OK · `git diff --check` 0.

**Nyitva maradt (tudatosan):** P0b analytics/mérő CLI és P1 Dixon–Coles; 2
függő Tipplap-névreview; 9 pending markets job; VoiceOver-hang emberi füllel;
egy nem tőlem származó `HomeHeader.swift` módosítás a munkaterületen
(érintetlenül hagyva); a három másik worktree.

## S196 — Predikció P0b: analytics séma + offline mérő CLI

- [x] **Migráció `0031_prediction_analytics.sql`** (előbb teszt-, majd dev DB):
      `analytics.market_line`, `analytics.prediction`, `analytics.measurement`
      — minden sor `model_version`, `as_of`, `inputs_hash` és forrással; a
      kanonikus `core`/`fact` táblákat nem módosítja. Táblaszám **57 → 60**
      (`db-check.sh`, `connection.rs`, `test_support.rs` TRUNCATE).
- [x] **Storage:** `repositories/analytics.rs` — as-of 1X2 minták (csak kickoff
      előtt rögzített snapshotok, legfrissebb), idempotens `upsert_measurement`
      és `list_measurements`.
- [x] **CLI:** `footballctl prediction-measure --from --to [--apply]` — Shin
      de-vigelés, Brier/3, log-loss, Wilson-intervallum és **klaszterezett
      bootstrap** (versenyenként, 2000 iteráció, fix seed); alapból dry-run,
      `--apply` esetén auditált `analytics.measurement` sor `inputs_hash`-sel.
- [x] **Éles mérés (2026-11-20…26, dev DB):** **247 as-of minta**, 0 kiszűrt;
      Brier/3 **0,1945** vs egyenletes bázis **0,2222**, különbség **−0,0277**,
      bootstrap CI **[−0,0438; −0,0130]** (a piac szignifikánsan jobb a bázisnál);
      log-loss **0,9795**, top-1 **129/247**, Wilson CI [0,4601; 0,5837].
      A mérés `--apply`-vel rögzítve (`analytics.measurement` id
      `01a21cbd-61ce-7132-a84e-77830b15ce32`). A dry-run és apply hash-e eltér,
      mert közben új odds-sorok érkeztek — az `inputs_hash` a tényleges inputé.
- [x] **Tesztek/kapuk:** +1 storage integrációs teszt (upsert idempotencia),
      teljes workspace **659 passed, 0 failed** · clippy 0 · fmt 0 · build OK ·
      `check-foundation` OK (dev/test 60 tábla) · `daily-check 11-26` EXIT=0 ·
      `integrity` OK · `git diff --check` 0.
- [ ] **Nem ellenőrzött**: a `market_line`/`prediction` táblák még nem íródnak
      (csak a mérés); a Shin-féle de-vigelés összevetése a v1 `marketcalib.ts`-vel;
      a 653 piac közül a mérés most csak az 1X2-t fedi; a bootstrap statisztikai
      kalibrálása szimuláción túl. A CLI neve kebab-case (`prediction-measure`).
- [ ] A P1 (Dixon–Coles / Poisson gólmodell shadow módban) nem indult.

### Visszaállás (S196)

- `git revert <S196 commit>`; a `0031` migráció visszaállítása:
      `DROP TABLE analytics.measurement, analytics.prediction, analytics.market_line;
       DROP SCHEMA analytics;` + a `_sqlx_migrations` 0031-es sorának törlése.
      A kanonikus adatot a szelet nem írta.

## S197 — Predikció P1a: Poisson + Dixon–Coles gólmatematika

- [x] **Tiszta modul:** `football-prediction/src/goals.rs` — Poisson PMF,
      Dixon–Coles `tau` (csak az 0-0 / 0-1 / 1-0 / 1-1 cellákra), 20×20
      (konfigurálható) gólmátrix normalizálással, és a mátrixból származtatott
      piacok: 1X2, O/U 2,5, BTTS + fair odds mindegyikre. Nincs adatbázis,
      illesztés vagy kanonikus írás; a `rho = 0` visszaadja a független Poisson-t.
- [x] **Tesztek (mért):** +5 unit teszt (PMF értékek, rho=0 ekvivalencia,
      tau csak alacsony cellák, piac-összegek és fair odds, hibás bemenet
      fail-closed). `football-prediction` **18/0**; teljes workspace
      **664 passed, 0 failed**, clippy 0, fmt 0, build OK.
- [x] **Kapuk:** `daily-check 11-26` EXIT=0 ZÖLD, `integrity 11-25…11-27` OK,
      `git diff --check` 0. A modell még shadow módban sincs bekötve — ez a
      szelet csak a matematikát adja.
- [ ] **Nem ellenőrzött**: a modell paramétereinek (`lambda_home/away`, `rho`)
      as-of illesztése és éles predikciók; összevetés a v1 `predictions.ts`
      20×20 mátrixával; a P0 mérési kapu szerinti elfogadás nem történt meg.
- [ ] A P1b (paraméter-illesztés + `analytics.prediction` + shadow CLI) nem indult.

### Visszaállás (S197)

- `git revert <S197 commit>` — a `goals.rs` modul megszűnik; a P0 modulok,
      az analytics séma és az adatok változatlanok.

## Következő döntési pont

- Predikció **P1b**: as-of paraméter-illesztés (csapat támadás/védelem, liga
      bázis, `rho`) + predikciók `analytics.prediction`-be shadow módban,
      majd mérésük a P0 szerződéssel (csak szignifikáns javulásnál lehet aktív).
      Külön döntés marad a 2 névreview és az S9 AI-gomb.
