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

## S198 — Predikció P1b: EB-illesztés és shadow predikciók

- [x] **Döntés:** a felhasználó a v1-szerű empirikus-Bayes zsugorítást választotta
      (K=4, hazai/vendég bontás, 365 napos ablak, 180 napos felezési idő).
- [x] **Tiszta illesztő:** `football-prediction/src/fit.rs` — súlyozott
      gól-for/against gyűjtés kizárólag a `cutoff` **előtti** meccsekből,
      `GF + K·bázis / (játszott + K)` zsugorítás, támadási/védekező erősség
      a ligaátlaghoz képest, `lambda_home/away` clamp 0,05–10; hibás/jövőbeli
      adat fail-closed. 5 új teszt.
- [x] **Storage/CLI:** `scored_matches_before` + `upcoming_matches` +
      `upsert_prediction`; `footballctl prediction-fit --date [--apply]` —
      Dixon–Coles gólmátrix (rho = −0,05), 1X2/OU/BTTS + fair odds,
      modellverzió `dixon-coles-eb-v1`, `as_of` és `inputs_hash` minden sorban.
      Alapból dry-run; nem ad fogadási tanácsot, nem ír kanonikus adatot.
- [x] **Éles mérés (2026-11-28, dev DB):** 40 905 as-of eredménysor,
      582 jövőbeli meccs; `--apply` **1746 predikciós sor** (582 × 3 kimenetel),
      átlagos `home/draw/away` = **0,437 / 0,252 / 0,311**, fair odds
      **2,50 / 4,12 / 3,73**. Minden sor `as_of` = 2026-11-27T11:36:55Z,
      modellverzióval és input-hash-sel.
- [x] **Kapuk:** teljes workspace **669 passed, 0 failed** · clippy 0 · fmt 0 ·
      build OK · `daily-check 11-26` EXIT=0 · `integrity` OK · `git diff --check` 0.
- [ ] **Nem ellenőrzött**: a modell **még nincs letesztelve** a P0 mérési
      szerződéssel (a `prediction-fit` és a `prediction-measure` még nincs
      összekapcsolva); nincs v1 `predictions.ts`-vel való szám szerinti
      ütköztetés; a `rho = −0,05` fix érték, nincs illesztve; az OU/BTTS
      predikciók nem íródnak a `analytics.prediction`-be (csak 1X2);
      582 meccsre 1746 sor — a 2026-11-28-i eredmények még nincsenek meg,
      így nincs hitelesített predikciós teljesítmény.
- [ ] A modell **shadow** marad: aktív modellé csak a P0 szerződés szerinti
      szignifikáns javulás után lehet.

### Visszaállás (S198)

- `git revert <S198 commit>` + `DELETE FROM analytics.prediction WHERE
  model_version='dixon-coles-eb-v1'` (a raw/audit sorok megőrzendők).
      A kanonikus adatot a szelet nem írta.

## S199 — Predikció P1c: backtest mód és shadow-modell mérése

- [x] **Döntés:** a felhasználó a backtest módot választotta (a 11-28-i
      eredményekre várás helyett).
- [x] **CLI:** `prediction-fit --date D --as-of D2 [--apply]` — a cutoff `D2`
      00:00 UTC, a modellverzió `-bt-{D2}` utótagot kap, hogy ne írja felül az
      éles shadow-predikciókat; `prediction-measure --model-version V` — a
      rögzített predikciókat méri a tényleges eredményre. Mindkettő alapból
      dry-run. A `upcoming_matches` a jelenlegi státusz helyett a `kickoff >
      cutoff` feltételt használja (különben a már lezárt meccsek kimaradtak a
      backtestből — javítva).
- [x] **Éles backtest (2026-11-24 cutoff → 2026-11-25 meccsei):** 522 predikciós
      sor (174 meccs × 3), ebből **146 lezárt meccs** mérhető. Eredmény:
      Brier/3 **0,2213** vs bázis **0,2222** (különbség **−0,0010**, bootstrap CI
      **[−0,0174; +0,0126]** → **nem szignifikáns**), de a de-vigelt piac
      Brier-e **0,1408** — a piac messze jobb. Log-loss **1,0928**, top-1
      **64/146**, Wilson CI [0,3605; 0,5194]. A mérés rögzítve
      (`analytics.measurement` id `01a21cd6-61cf-71c7-8486-57da7f5c216f`).
- [x] **Kapuk:** teljes workspace **669 passed, 0 failed** · clippy 0 · fmt 0 ·
      build OK · `daily-check 11-26` EXIT=0 · `integrity` OK · `git diff --check` 0.
- [x] **Döntés a kapuban:** a modell **nem aktiválható** — a P0 szerződés
      szerint a CI átmetszi a 0-t, és a piac is jobb nála. Shadow marad.
- [ ] **Nem ellenőrzött**: a backtest csak egy napon (146 minta) futott;
      hosszabb ablak, `rho` illesztése, OU/BTTS predikciók rögzítése,
      és a `home_ft`-hiányos meccsek kiszűrésének oka nincs kivizsgálva.
      Egy korábbi, hibás backtest-futtatás 75 sorát töröltük az analytics
      táblából (csak saját generált adat).
- [ ] A 2 névreview és az S9 AI-gomb továbbra is külön döntés.

### Visszaállás (S199)

- `git revert <S199 commit>`; a backtest sorok törlése:
      `DELETE FROM analytics.prediction WHERE model_version LIKE 'dixon-coles-eb-v1-bt-%'`
      és `DELETE FROM analytics.measurement WHERE model_version LIKE '...'`.
      A kanonikus adatot a szelet nem írta.

## S200 — Predikció megjelenítése a kliensben + lemeztakarítás

- [x] **Lemez:** a `rust/target/debug/incremental` (6,9 GB) törlése — a szabad
      hely **1,7 GB → 8,6 GB**; a telepített daemon/app binárisok és a
      `target/debug/deps` build-cache megmaradt, így a munka folytatható.
- [x] **API:** `GET /v1/match/{id}/prediction` (read scope, 400/401/404/500),
      a legfrissebb modellverzió 1X2 sorait adja `as_of`, `inputs_hash`,
      `quality` és fair odds mezőkkel; OpenAPI **28 → 29** útvonal. A `numeric`
      DB-oszlopok `::float8` castet kaptak (élő 500 hiba javítva).
- [x] **Core/UI:** `ShadowPrediction` (a meglévő `MatchInsightModels.MatchPrediction`
      névütközés elkerülésére), `MatchPredictionService`, `PredictionCard` az
      Odds fülben: százalékos 1X2, fair odds, és kifejezett **„NEM ELLENŐRZÖTT ·
      SHADOW"** + **„Nem fogadási tanács"** jelzés, modellverzió és as-of idővel.
      AX-azonosítók és magyar címkék.
- [x] **Tesztek/kapuk:** +2 Core teszt, +1 API teszt (auth/404/legfrissebb
      verzió), +1 XCUITest; workspace **670 passed, 0 failed** · clippy 0 ·
      fmt 0 · build OK · Core **165/0** · app unit **101/0** ·
      `run-ui-tests.sh` **34/34** (egy régi Ops-dátumválasztó-teszt továbbra is
      piros, A/B-vel alapon is — környezeti) · `daily-check` ZÖLD ·
      `integrity` OK · `git diff --check` 0.
- [x] **Éles:** daemon + app újratelepítve, `/v1/health` OK; a predikció-végpont
      valódi meccsen 3 sort ad fair odds-szal. A kliensben az új kártyát az
      XCUITest igazolja (fixture-tel, valódi macOS-en).
- [ ] **Nem ellenőrzött**: a kártya görgetéssel való élő kézi végignézése
      (az AX-harness nem nyit részletezőt a napi kártyáról); VoiceOver-hang;
      a százalékok a modell saját termeiből jönnek, összevetve a piaci
      szorzókkal még nincs UI-szinten edge-mutatás.
- [ ] A modell shadowban marad (P1c mérése szerint nem jobb a piacnál).

### Visszaállás (S200)

- `git revert <S200 commit>` — az új végpont, Core modell és kártya megszűnik;
      az analytics adatok és a kanonikus entitások változatlanok.
- Az inkrementális build-cache nem állítható vissza, de `cargo build` újra
      létrehozza (a `deps` cache megmaradt).

## S201 — OU/BTTS predikciók és megjelenítésük

- [x] **Backend:** a `prediction-fit` most 7 sort ír meccsenként (1X2 + OU 2,5
      + BTTS), a `analytics.prediction` `market`/`line` dimenzióit használva;
      a `/v1/match/{id}/prediction` válasz és a Core `ShadowPrediction` is
      hordozza ezeket. Éles mérés: **4151 sor**, ebből 1X2 1779, OU 1186,
      BTTS 1186 — minden predikált meccsen 7 kimenetel.
- [x] **UI:** a `PredictionCard` az 1X2 sorok alatt megjeleníti a gólszám 2,5
      és a BTTS valószínűségeket is, ugyanazzal a „NEM ELLENŐRZÖTT · SHADOW"
      és „Nem fogadási tanács" jelzéssel; AX-címkék magyarul.
- [x] **Kapuk:** workspace **670 passed, 0 failed** · clippy 0 · fmt 0 · build OK ·
      Core **165/0** · app unit **101/0** · `run-ui-tests.sh` **34/34** (egy régi
      Ops-dátumválasztó A/B-vel alapon is piros) · `daily-check` ZÖLD ·
      `integrity` OK · `git diff --check` 0.
- [ ] **Nem ellenőrzött**: az OU/BTTS predikciók hitelességét a P0 szerződés
      még nem mérte (a `predicted_outcomes` csak 1X2-t olvas); a kártya élő
      kézi görgetése; VoiceOver-hang.
- [ ] A modell shadowban marad (P1c: nem jobb a piacnál).

### Visszaállás (S201)

- `git revert <S201 commit>`; a már írt OU/BTTS sorok törlése:
      `DELETE FROM analytics.prediction WHERE market IN ('ou','btts')`.

## S202–S203 — UI-hibajavítás, mérési kiterjesztés, dokumentáció

**S202 (UI):** a tartósan piros `testOperationsSummaryDatePicker...` oka valódi
layout-hiba volt: a hosszú hiányjel-sorok intrinsic szélessége szétfeszítette
az Üzemeltetés ScrollView-ét, és a dátumválasztó a látható területen kívülre
került. `GeometryReader` + `lineLimit(1)` javítás; a teszt-harnessben rögzített
**1400 px** ablakszélesség (a macOS window restoration 1800 px-es ablakot hozott
vissza, ami a képernyőn túlra nyúlt). Eredmény: **`run-ui-tests.sh` 34/34 ZÖLD**
— először hibátlan teljes futás. A nem tőlem származó `HomeHeader.swift`
(naptár gombok AX-címkéi) átvizsgálva és commitolva.

**S203 (mérés):** a `prediction-measure` most **1X2 + OU 2,5 + BTTS** piacokra
mér külön-külön; `prediction_market_samples` storage-lekérés, piac-szintű
`analytics.measurement` sorok. Backtest (2026-11-24 cutoff, 11-25 meccsei,
146 lezárt mérhető meccs):

| piac | modell | bázis | piac | különbség | bootstrap CI |
|---|---|---|---|---|---|
| 1x2 | 0,2213 | 0,2222 | 0,1408 | −0,0010 | [−0,0174; +0,0126] |
| btts | 0,2527 | 0,2500 | — | +0,0027 | [−0,0137; +0,0208] |
| ou | 0,2381 | 0,2500 | — | −0,0119 | [−0,0289; +0,0059] |

Egyik piac sem szignifikáns → a modell **shadowban marad**. Három mérési sor
rögzítve. `docs/operations/remaining-gaps.md` frissítve (S131/S190 megoldva,
VoiceOver SKIP, Tipplap és predikció állapota).

**Kapuk:** workspace **670/0**, clippy 0, fmt 0, build OK, Core 165/0,
app unit 101/0, UI **34/34**, `daily-check` ZÖLD, `integrity` OK, `git diff --check` 0.

### Visszaállás (S202–S203)

- `git revert` a két commitra; a mérési sorok törlése:
      `DELETE FROM analytics.measurement WHERE model_version LIKE 'dixon-coles-eb-v1-bt-%'`.

## S204 — Függő Tipplap-névreviewök rendezése (adatművelet)

- [x] **Felhatalmazás:** a felhasználó 2026-11-27-ben minden további munkára
      és javításra általános engedélyt adott; a VoiceOver-hangot kifejezetten
      **skip**-elte (nincs mikrofon).
- [x] **Review-átvizsgálás:** 10 pending Tipplap team-sor. Egyik sem fuzzy
      összevonás: `Inter Kashi FC` → kanonikus `Inter Kashi` (IND) reviewed
      alias (`manual_review` ref, csak az „FC" utótag eltér), a másik 9-nek
      **egyáltalán nem volt jelöltje** — valódi, új klubok (Al-Gharraf SC,
      Al-Mudhaibi, Brickfield Rangers, Denbigh Town, Fanja, Gas Alshimal,
      IMT Novi Belgrád, Nacional Madeira, Swansea Univ.).
- [x] **Végrehajtás:** 1 `footballctl identity approve` + 9 `approve-create`
      (élő forrás bootstrap, mindegyik auditált megjegyzéssel). A Tipplap
      review queue pending száma **10 → 0**. Egy új, idempotens Tipplap
      discovery job (`manual-review-unlock-2026-11-27`) újraingeste a mai
      napot: a napi lista meccsszáma **232 → 245**, a linkelt mai Tipplap-
      események **81 → 171**.
- [x] **Ellenőrzés:** `integrity 11-25…11-27` **OK** mind az ingest előtt,
      mint utána (0 duplikátum/árva); `daily-check 11-26` EXIT=0, failed job 0;
      workspace **670/0**, clippy 0, fmt 0, `git diff --check` 0.
- [ ] **Nem ellenőrzött**: a 9 új entitás független forrás-keresztellenőrzése
      (egyforrású bootstrap; a nav review queue jelölt nélkül volt); a
      visszaállás csak a review sorok `pending`re állítása + az új entitások
      törlése (audit megőrzendő).
- [ ] A modell shadowban marad; a BIZONYOS forrású új csapatok mostantól
      feloldódnak, de a kanonikus adatot a szelet nem írta át kézzel.

### Visszaállás (S204)

- `UPDATE identity.review_queue SET status='pending' WHERE id IN (…10 UUID…);`,
      `DELETE FROM identity.source_entity_ref WHERE method='manual_review' AND
      source_entity_id='inter-kashi-fc'`, és a 9 létrehozott `core.entity`/
      `core.team` sor archiválása (nem törlése) az audit megőrzésével.

## S210 — Döntések: UI-tesztelés, forduló-hézag, 25/75 terv

- [x] **XCUITest-gyakorlat leállítása:** a felhasználó kérésére a teljes
      `run-ui-tests.sh` (15 perc, mindig ugyanazt kattintja, blokkolja a gépet)
      **nem része a rutin-kapunak**. Ellenőriztem: nem fut xcodebuild folyamat.
      Helyette gyors app unit + Core teszt, valamint célzott 1–2 UI-eset csak
      akkor, ha a szelet azt tényleg érinti. A script megmarad kérésre.
- [x] **Fordulóadat-hézag kivizsgálása (read-only):**
  - A research napi JSON **tartalmaz** fordulót: 11-27-en Premier League
    **15/17**, Eerste Divisie 7/7, de Ligue 1 FR csak 1/3.
  - A DB viszont kevesebbet tart: novemberi RTL-adatok alapján
    `Premier League 60%` (21/35), `La Liga 60,5%`, `Bundesliga 61,3%`,
    `Ligue 1 (FR) 68,6%`, `Serie A 74,4%`, `Coupe de France 28,5%`,
    `NCAAW/NCAAM Soccer`, `Club Friendly`, `FA Trophy`, `Friendlies`: **0%**.
  - **Gyökér:** `canonicalize.rs:501` a `round_label`-t feltétel nélkül írja
    át `EXCLUDED.round_label`-re — a források, amelyek nem adnak fordulót
    (`footballdata`, `soccerway`, `fotmob`, `tipplap`), **NULL-lal felülírják**
    a research/openfoot által korábban rögzített számot. A `fact.provenance`
    ráadásul nem követi ezt a mezőt (0 sor), ezért a hiba csendes.
  - Javaslat: `COALESCE(EXCLUDED.round_label, core.match.round_label)` +
    provenance-bejegyzés + visszajátszható backfill a research obszervációkból.
    Új külső forrás csak azoknak a ligáknak kell, ahol a research sem ad
    fordulót (pl. Ligue 1 FR 1/3, kupa-/barátságos meccsek).
- [x] **25/75 terv:** `docs/superpowers/specs/2026-11-27-fooldal-reszletezo-panel-design.md`
      — bal lista 25%, jobb részletező panel 75%, kattintásra ott jelenik meg
      a meccs (nem sheetben), widget-sorrend az áttekintéstől a jegyzetekig,
      keskeny ablaknál sheet fallback, S210–S212 szeletek.
- [ ] A 25/75 arány értelmezése (bal 25 / jobb 75) és a `round_label`-javítás
      elindítása — felhasználói döntés.

### Visszaállás (S210)

- Csak dokumentáció és read-only vizsgálat; kód/adat nem változott.

## S211 — Fordulószám megőrzése és visszajátszható backfill

- [x] **Döntés:** a felhasználó a 25/75 arányt bal 25 / jobb 75-ként erősítette
      meg, és a fordulójavítást most kérte.
- [x] **Kódjavítás:** `canonicalize.rs` — `round_label = COALESCE(EXCLUDED.
      round_label, core.match.round_label)`; a források, amelyek nem adnak
      fordulót, nem törölhetik a már rögzítettet. Új regressziós teszt:
      `source_without_round_does_not_erase_stored_round`.
- [x] **Backfill:** új storage-modul (`round_labels.rs`) + `footballctl
      rounds-backfill [--apply]` (alapból dry-run, idempotens). storage-teszt:
      csak NULL-t ír, második futás 0 sort. Éles: **1059** meccs pótlása
      (dry-run 1059 → apply 1059 → újra 0). Integritás OK, daily-check ZÖLD.
- [x] **Mérés:** a mai napon a kanonikus nagy ligák sorai fordulóval rendelkeznek
      (Bundesliga DE 1/1, La Liga ES 1/1, Ligue 1 FR 1/1, NB I HU 1/1,
      Premier League GB-ENG 1/1). A „ Premier League" többi országú sora
      (AM/AZ/BH/LY/MT/SG/TZ/UA) külön liga — ezért jelenik meg a főoldalon
      több „1 meccs" csoport, nem hiba.
- [x] **Kapuk:** workspace **672 passed, 0 failed**, clippy 0, fmt 0, build OK,
      `integrity` OK, `daily-check` EXIT=0, `git diff --check` 0.
- [ ] **Nem ellenőrzött**: a research által egyáltalán nem adott fordulók
      (pl. barátságosok, kupák) továbbra is hiányoznak — új forrás kellene;
      a főoldali `~` jelölésű többségi forduló megjelenítése a preview ágon
      már kész, de ezen az ágon nincs.

### Visszaállás (S211)

- `git revert <S211 commit>`; a backfill soraihoz: az érintett `core.match`
      `round_label` mezőjét a research obszervációkból újra lehet generálni;
      destruktív törlés nem történt. Az új daemon a régi commitból újraépíthető.

## S212–S214 — Elhalasztott meccsek, pontos oszlopszélesség, kártyaállapotok

- [x] **S212 — elhalasztottak:** új `Elhalasztott` szűrő (`feat/postponed-handling`
      @ `51b51ee`) — a `postponed` státusz kikerült a `Következik` és a
      `Lejárt, nyitott` szűrőből; a kártyán `ELHALASZTOTT` felirat jelenik meg
      idő helyett. Mért adat: novemberben **107** elhalasztott meccs (100 múltbeli,
      7 mai); a teljes audit `docs/operations/postponed-audit.md`-ben, újra
      futtatható SQL-lel. Egységteszt: `testPostponedIsSeparateFromUpcoming`.
- [x] **S213 — pontos szélesség:** `feat/panel-exact-width` @ `73725aa`.
      A bal oszlop nem százalék, hanem a kártya tényleges tartalmi minimuma:
      `76 + 190 + 56 + 180 + 4×10 + 4 + 2×14 = **574 pt**`. 1440 pt ablomnál
      lista **574 pt**, jobb panel **754 pt**; 1200 pt-nél 552/536; 900 pt-nél
      egyoszlopos (804 pt). A `columnWidths()` tiszta függvény, 5 pixelpontos
      egységteszttel (`MatchListLayoutTests`). A kártya `isWide` döntése most
      a saját oszlopán alapul, nem a teljes ablakon.
- [x] **S214 — minden állapot látható:** a kártya bal felső sorában
      `KÖVETKEZIK` / `ÉLŐ` / `LEZÁRT` / `VÁRHATÓ` / `ELHALASZTOTT` jelenik meg.
      A hosszú felirat 7,5 pt betűvel elfér a 76 pt-es oszlopban (képi ellenőrzés).
- [x] **Integráció:** `feat/homepage-preview` @ `621f436` — a három feature ág
      és az S211 backend ág merge-elve, pusholva. App unit **zöld**; a teljes
      15 perces UI-harness **nem futott** (felhasználói döntés). Helyette valódi
      macOS ellenőrzés: AX-probe `home.sidepanel` és `filter.postponed` jelen van,
      az `Elhalasztott: 7` chipre kattintva 7 elhalasztott kártya látszik AX-ben
      (pl. `FC Dordrecht – Vitesse, Elhalasztott, 17. forduló`), képek:
      `/tmp/fb-homepage-preview3.png`, `/tmp/fb-postponed-filter2.png`.
      Az app újratelepítve, futó PID **6153**-ból újraindítva; a szűrő visszaállítva `Mind`re.
- [x] A backfill után a ligafejlécekben ténylegesen megjelennek a fordulószámok
      (12., 15., 11., 13., 14. forduló a képen) — nem találgatás, a research-adatból.

### Visszaállás (S212–S214)

- `git revert` a feature ágak commitjaira; a preview ág újraépíthető a feature
      ágak merge-éből. Az adatot ezek a szeletek nem írják. A szűrő és a badge
      visszaállítása külön revertelhető.

## S215 — Adatminőségi hibák gyökéroka és kemény kapu

- [x] **Kivizsgálás (read-only):** a felhasználó által jelzett három eset
      részletes gyökérok-lemutatása `docs/operations/2026-11-27-data-bug-audit.md`.
  - Paraguayan Primera 11-25: ugyanaz a három kanonikus csapatpár három sorban
    (egy `exact` 20:00 UTC + két `date_only` 00:00 placeholder), a research
    két külön uid alatt (`fm:` és `espn:`). A `find_cross_source_fixture` nem
    köt hidat: az ország XX és 20 óra a kezdés-különbség. A forrás mind a hat
    sorban `scheduled`/null eredmény — nem volt mit visszajátszani.
  - Ciprus-kupa 11-25: 8 meccs, mind `scheduled`/null eredmény a researchben
    még 11-27-en is; más forrás nem rögzített hozzájuk adatot.
  - Algéria 2 meccs: **nem rendszerhiba** — az `ops.audit_event` szerint a
    felhasználó maga állította át 11-27 16:58-kor `postponed`-ra,
    „el lett halasztva" indoklással, teljes observation/provenance/audit lánccal.
- [x] **Kapu:** az `IntegrityReport` két új mutatója:
    `unresolved_duplicate_fixture_groups` (**kemény hiba**, ID-alapú, nem
    fuzzy), `stale_scheduled_without_result` + `missing_finished_result`
    (adathiány-jelzés, `verdict: OK — de ADATHIÁNY`). A korábbi riport az
    `azonos csapatpár azonos napon = 5` mutatót csak figyelmeztetésnek kezelte,
    ezért volt `OK` — ezért nem derült ki 11-25-én.
- [x] **API-szűrés:** a `fetch_day` és a `fetch_day_gaps` determinisztikusan
    egy sorra szűri az ilyen csoportot (pontos kezdésű sor marad). Mért eredmény:
    11-25-re a napi API **3** paraguayi meccset ad vissza 6 helyett, mind
    20:00 UTC-vel; `missing_kickoff` 7 → 4. Az adatot nem írtuk át.
- [x] **Kapuk:** workspace teszt **0 failed**, clippy 0, fmt 0, build OK.
    Új daemon telepítve; `integrity --from 2026-11-25 --to 2026-11-27` most
    `FELOLDATLAN cross-source duplikátum: 3`, `lejárt, scheduled, eredmény
    nélkül: 36`, **`verdict: FAIL`** — ez a kívánt, korai jelzés.
- [ ] **Nem ellenőrzött / nyitott:** a Ciprus-kupa és a paraguayi meccsek
    eredménye a forrásban nincs meg — új forrás kell hozzá (S216 már nem nyitott).

## S216 — Duplikátumok adattisztítása (jóváhagyás után)

- [x] **Döntés:** a felhasználó engedélyezte a placeholder sorok `canceled`re
    állítását. A dry-run során kiderült, hogy páronként **egy `date_only` +
    két `exact`** sor van — ezért csak a 3 placeholder érintett, pontos sor
    nem lett leállítva vagy törölve.
- [x] **Eszköz:** új `footballctl duplicates-resolve [--from --to] [--apply]`
    (dry-run alapból, egy tranzakció, `ops.audit_event` minden sorhoz).
    Idempotens: első apply 3 sor, második 0. Új storage-teszt igazolja,
    hogy csak `date_only` sorokat érint és auditot ír.
- [x] **Kapu most zöld:** az integritási lekérések kizárják a `canceled`
    sorokat; `duplikátum: 0`, `verdict: OK — de ADATHIÁNY` (35 lejárt,
    eredmény nélküli meccs — forráshiány, nem találunk ki eredményt).
    `daily-check 2026-11-26` **EXIT=0 / ZÖLD**.
- [x] **API:** 11-25-re továbbra is 3 paraguayi sor, mind 20:00 UTC-vel;
    a két pontos sor közül determinisztikusan egyet jelenítünk meg
    (`duplicate_instant_losers`), `missing_kickoff` 4. Új daemon telepítve.
- [x] **Kapuk:** workspace teszt 0 failed, clippy 0, fmt 0, build OK.
    Részletek: `docs/operations/2026-11-27-data-bug-audit.md`.

### Visszaállás (S216)

- A 3 `canceled` sor `status`ja az audit eventek alapján `scheduled`re
    állítható; sor nem lett törölve. A kód `git revert`-tel visszaállítható.

### Visszaállás (S215)

- `git revert` a `feat/duplicate-fixture-gate` commitjaira; az API-szűrés nem
    ír adatot, így a régi 6 sor újra megjelenik. A kézi algériai szerkesztések
    az audit eventből visszaállíthatók.

## S217 — Külső források alapján végzett adatkorrekciók

- [x] **Döntés:** a felhasználó engedélyezte a teljes korrekciót (dátumok,
    státuszok, hiányzó meccsek letöltése) auditlánccal.
- [x] **Külső bizonyíték:** `docs/operations/2026-11-27-data-bug-audit.md` 4. szakasza.
  - Paraguay: az ESPN ugyanazzal a játékID-vel 11-27/11-28-ra tünteti fel a
    három meccset, amelyek nálunk 11-25-ön `scheduled`-ek maradtak.
  - Ciprus-kupa: a CFA hivatalos programja szerint a nyolc nyolcaddöntő
    12-09 és 2027-01-27 között van — a 11-25-i dátum a research feedben
    placeholder volt („legkorábban ekkor kezdődhetnek").
  - Holland 17. forduló: a Voetbalkrant pontosan az öt elhalasztott és három
    lejátszott meccset mutatja, aminek megfelel a DB; a két 11-29-i meccs
    azért hiányzott, mert nem volt 11-29-i napi JSON.
- [x] **Végrehajtás:** `scripts/corrections-2026-11-27.sql` — 6 elavult
    paraguayi sor canceled, 8 ciprusi kickoff javítva, Emmen–Helmond
    postponed; minden sorhoz `ops.audit_event` (**15** event). Újrafuttatása
    idempotens. A 11-29-i research feed `scripts/research-sync.sh 2026-11-29`
    segítségével letöltve (2,1 MB) és feldolgozva — a két hiányzó holland
    meccs most a megfelelő adatcsatornából származik. Az új feed által
    hozott 2 paraguayi placeholder duplikátum a meglévő
    `duplicates-resolve` paranccsal rendezve (2 sor, audit event).
- [x] **Kapu:** a `fetch_day` és `fetch_day_gaps` most kiszűri a `canceled`
    sorokat (korábban a leállított meccsek még megjelentek az API-ban).
    `integrity --from 2026-11-25 --to 2026-11-29`: duplikátum 0,
    `verdict: OK — de ADATHIÁNY`; `daily-check` **EXIT=0 / ZÖLD**.
    Workspace teszt 0 failed, clippy 0, fmt 0, build OK; új daemon telepítve,
    `/v1/health` OK. API: 11-25-en 0 canceled sor, 11-27-n 1 paraguayi meccs,
    11-29-en 2 holland meccs.
- [x] **Migráció:** a `feat/ops-triage-console` ág 0032-es migrációja
    (capture request_key index) bekerült erre az ágra is — a dev DB már
    alkalmazta, ezért a régi bináris nem tudott indítani. Forrás és indok
    rögzítve; másik ág munkáját nem írtuk át.
- [ ] **Nem ellenőrzött:** a javított ciprusi időpontokat a UI-ban nem
    néztük meg (jövőbeli meccsek, nincs AX/képi teszt); a researchben továbbra
    is 2876 függő review van (a 11-29-i sync növelte meg).

### Visszaállás (S217)

- A `scripts/corrections-2026-11-27.sql` minden UPDATE-je audit eventtel
    írt; az `ops.audit_event` `detail` mezője tartalmazza a régi értékeket.
    A `git revert` a kódot állítja vissza, az adatkorrekció az auditból
    visszafordítható.

## S219 — Élő percek: a research-betöltés újra működik

- [x] **Jelzés:** a Győr–Debrecen kártyája 42'-t mutatott, miközben a meccs
    később járt; a német meccsek rendben voltak. A kliens óraszámítása nem
    volt hibás — a `minute` közvetlenül a research obszervációból jön.
- [x] **Gyökérok:** a 0032-es migráció eltörölte a
    `raw.capture (source_id, request_key)` egyedi kényszert, de a worker és a
    fixture-adapter még `ON CONFLICT (source_id, request_key)`-t használt —
    ezért minden research job a `capture beszúrás` lépésben failed, és az élő
    adat a 20:24-i snapshoton ragadt. A research JSON közben már 88'-t tartalmazott.
- [x] **Javítás:** a capture INSERT nem hivatkozik törölt kényszerre; a
    felhasznált capture a legfrissebb `received_at` szerint választódik
    (a migráció szándékának megfelelően). Ugyanez a fixture-adapterben.
    Storage-teszt frissítve: artifact dedup, capture-ek száma 3, legújabb nyer.
- [x] **Mérés:** a failed job újraütemezése után Győr–Debrecen
    **42' → 88'**; a 11-27-i API 47 élő meccsére **0 perc-, 0 státusz- és
    0 eredményeltérés** a research JSON-hoz képest. `ops.job`: done 71,
    failed 0. Workspace teszt 0 failed, clippy 0, fmt 0, build OK; daemon
    újratelepítve, health OK. Részletek: `docs/operations/2026-11-27-data-bug-audit.md` 5. szakasz.
- [ ] **Nyitott:** a `daily-check` csak az előző nap failed jobjait nézte,
    ezért zöld maradt a 11-27-i hibás betöltés mellett — a job-ellenőrzést
    minden napra kell terjeszteni (külön szelet).

### Visszaállás (S219)

- `git revert` a két capture-javításra; a 0032-es migráció visszaállítása
    külön döntés (az új capture-ek miatt nem önmagában visszafordítható).

## Következő döntési pont

- A predikció shadowban marad; nyitva: a 3 duplikátumcsoport adattisztításának
    jóváhagyása, a 25/75 részletező panel widgetjei (S210–S212 terv), hosszabb
    backtest/rho-illesztés, S9 AI-gomb. VoiceOver skip.
