# 2026-11-22 — napi checkpoint

## Induló állapot és kérés

- A felhasználó kérése: „nézzük meg a mai adatokat, indítsuk be az appot,
  kezdjük meg a mai ellenőrzéseket és fejlesztéseket".
- Kiindulás a `2026-11-21/checkpoint.md` S90-es záró állapota:
  `core.match` 283 611; 11-21 incidents 5 893 / 487 meccs; reverify 11-21:
  verified 515 / provisional 430 / withheld 4; `integrity` OK; git tiszta,
  `main` ágon, utolsó commit `df2442b` (docs S90).
- A környezet futott: daemon pid 911 (port 49355, 07:30-tól), app pid 425
  (07:30-tól, de a bináris **2026-11-21 17:08-as**, az utolsó app-commit
  17:21-es volt — tehát elavult build), Obscura pid 1483.

## Bizonyíték-alapú induló diagnosztika (read-only)

- [x] API: `/v1/health` 200; `GET /v1/day/2026-11-22` 200, 632 sor,
      `refreshed_at=2026-11-22T06:44:48Z`; gaps: lejárt eredmény nélkül 13,
      withheld 0, idő nélkül 0.
- [x] DB (Budapest nap): 11-22 → **638 meccs / 34 lezárt / 37 eredmény**;
      11-23 → 98 meccs. `fact.match_incidents` 11-22-re **436 sor / 33 meccs**.
- [x] Research envelope 11-22: `generated_at=2026-11-22T08:07:04Z`, rev 20,
      **725 meccs / 49 terminális / 676 nyitott / 411 detail**, `data_status=partial`.
- [x] `ops.job`: 5 pending (soccerway discovery 126 próbálkozással, kvóta
      kimerült; research + tipplap refresh 13:00 és 19:00-ra ütemezve);
      nyitott breaker: soccerway (pre-existing).
- [x] `identity.review_queue`: **45 pending** (27 `no_exact_match`,
      18 `fuzzy_candidate`), túlnyomóan openfoot/tipplap/research csapatok.
- [x] `integrity --from 2026-11-22 --to 2026-11-22`: mind a 8 mutató 0
      kivéve 45 függő review és 1 nyitott breaker, **verdict OK**.
- [x] `reverify --from 2026-11-22 --to 2026-11-22`: **633 meccs, 0 hiba**;
      238 verified / 395 provisional / 0 withheld.
- [x] `coverage-day --from 2026-11-22 --to 2026-11-22`: 638/34/37, odds 198,
      logó 363, idő nélkül 0, withheld 0, verified 239.

## S91 — app friss buildje és alapkapu

- [x] `cd apps && xcodegen generate && xcodebuild … build` → **BUILD SUCCEEDED**;
      az app újraindítva a friss binárissal (pid 69982, build 10:47:25),
      ami tartalmazza az S84–S87 idővonal-üres-állapotot is.
- [x] **Valódi macOS viselkedésellenőrzés**: `swift scripts/ax-probe.swift
      FootballApp match.card` → a mai nap kártyái magyar VoiceOver-címkékkel
      (pl. „Club Santos Laguna – Leon, nov. 22. 00:00, Lezárt, 1 – 2,
      Ideiglenes publikáció, 17. forduló, odds 1: 2,19, X: 3,40, 2: 2,50");
      a `day` szűrő a hiány-panelt és a napi navigációt is visszaadta.
- [x] `./scripts/check.sh` → **EXIT=0**, **102 „test result: ok" blokk**, 0 FAILED.
- [x] `./scripts/check-foundation.sh` kezdetben **EXIT=1**: a `db-check.sh`
      47 elvárt táblát ellenőrzött, de a mindkét DB-n alkalmazott 0017-es
      migráció (`ops.editorial_hold`) óta 48 tábla van. Az 0018–0020 csak
      függvényt/indexet ad. Javítva: `expected_tables=48` + magyarázó komment.
- [x] `./scripts/db-reset.sh` (csak teszt-DB) után `./scripts/db-check.sh` →
      **EXIT=0**, mindkét DB: 48 tábla / 6 szabály; dev 15 945 658 üzleti sor,
      test 0/0. `./scripts/check-foundation.sh` → **EXIT=0**
      (`/tmp/foundation_1122_fixed2.log`).

## S92 — napi review-triage (45 → 0)

- [x] **Gyökérok**: a reggeli ingest 07:31–07:34 között 45 új csapat-review-t
      hozott (27 `no_exact_match`, 18 `fuzzy_candidate`). A vak
      `approve-create-all` itt duplikátumokat hozna létre (pl. „Manchester
      Utd." → „Manchester United", „Nyíregyháza" → „Nyíregyháza Spartacus",
      „ZTE" → „Zalaegerszeg"), ezért minden sort név + ország + kontextus
      alapján párosítottam a meglévő entitáshoz (scratch: `/tmp/triage_reviews.py`,
      száraz tervvel: 45/45 lefedve, 0 hiányzó cél).
- [x] **41 review** meglévő entitáshoz kötve (`identity approve`, mind
      `identity_approve` audittal); **3 review** új entitást kapott
      (`approve-create`): AC Milan U20/U23 → `youth/youth`, Peterborough
      United W → `female/women` — a névjelölő-konverzió a kódból jött, nem
      kézi találgatás.
- [x] **Madrid CFF**: a tipplap `gender=male` adathibája (a klub női:
      Copa de la Reina, Liga F, és már volt `Madrid CFF W` entitás). Auditalt
      kontextus-korrekció (`review_context_gender_correction`: male→female,
      senior→women), majd `approve --entity` a meglévő női entitásra.
- [x] Eredmény: **45 → 0 pending review**; 44 döntés 0 hibával, összesen
      46 friss audit-esemény (41 approve + 3 create + 1 korrekció + 1 approve).
- [x] Ellenőrzési lánc: `integrity --from 2026-11-22 --to 2026-11-22` →
      **OK**, mind a 8 mutató 0 (0 duplikált kickoff/fixture, 0 árva
      provenance, 0 review, 0 withheld, 0 failed job), kivéve 1 pre-existing
      nyitott soccerway breaker; `reverify 11-22` → **633 meccs, 0 hiba**
      (238 verified / 395 provisional); `dedupe matches --date 2026-11-22`
      → 0 duplikátum; `coverage-day 11-22` → 638/34/37. `core.match`
      **283 859** (a reggeli ingest +248).

## S93 — női versenyű meccsek férfi csapat-hozzárendelése

- [x] **Gyökérok**: 12 női versenyű meccsover férfi csapat: (a) 8
      `Women's International Friendly` (11-27/28), ahol a research/ESPN a női
      nemzeti csapatokat `(W)` jelölő nélkül küldte, így a linker a férfi
      entitáshoz kötötte őket; (b) 4 klubmeccs, ahol ugyanaz a fixture két
      forrásból jött — a FotMob-sor női csapatokkal, az ESPN-sor férfi
      entitásokkal. A férfi entitásoknak valós férfi meccsei vannak, ezért
      nem lehetett a nemüket átállítani.
- [x] **17 új női entitás** auditalt SQL-lel (`team_create_female_counterpart`,
      17 audit-esemény): 15 nemzeti (China (W), Thailand (W), San Marino (W),
      Liechtenstein (W), Malawi (W), Albania (W), Australia (W), Colombia (W),
      South Korea (W), Peru (W), Dominican Republic (W), Northern Ireland (W),
      Greece (W), Bolivia (W), Uruguay (W)) + 2 svéd klub (Elfsborg W,
      Enskede IK W). Mindegyik kapott `espn:<id>` research forrás-refet
      (`manual_review`, active), női kontextusú aliast és országkódot — a
      következő ingest így nem hozza létre újra a férfi párt.
- [x] **10 meccs átrendelése** (`match_team_reassign`, 17 audit-esemény):
      a 8 barátságos + 2 svéd klubmeccs most női csapatokra mutat.
- [x] **5 duplikátum archiválása** (`dedupe pair`, teljes payload az
      `ops.dedupe_archive`-ban): Everton–Brighton & Hove Albion → Everton W–
      Brighton W; De Graafschap–Ajax W és De Graafschap–Ajax Amsterdam →
      De Graafschap (W)–Ajax W; Feyenoord Rotterdam–FC Twente → Feyenoord W–
      FC Twente (W); PSV–PEC Zwolle → PSV W–PEC Zwolle (W).
- [x] Eredmény: a bővített szűrővel (**women/női/(W)/femenin/vrouwen/frauen/
      damallsvenskan/elitettan/toppserien**) **0** női versenyű meccs maradt
      férfi csapattal. Az API 11-22-re egy sor Everton W–Brighton W és egy
      De Graafschap (W)–Ajax W (633 sor).
- [x] Ellenőrzési lánc: `reverify 11-14…11-28` → **4 585 meccs, 0 hiba**
      (1 998 verified / 2 583 provisional / 4 withheld — a 4 ismert hiányzó
      eredmény 11-21-ről); `integrity 11-13…11-28` → **OK**, 0 duplikált
      kickoff/fixture, 0 árva provenance, 0 review, 0 failed job (1 pre-existing
      soccerway breaker); `dedupe matches` dry-run 11-14/22/27/28 → 0;
      `coverage-day 11-22` → 633/34/37.

## S94 — a 4 hiányzó eredmény pótlása (11-21)

- [x] A 11-21-i research envelope 06:42-re frissült (rev 76) és **mind a 4**
      korábban withheld meccs eredményét tartalmazza, két független forrás
      (FotMob + LiveScore) egybehangzóan: Östersunds FK **0–2** Varbergs BoIS
      (HT 0–1); SV Rödinghausen **2–1** Schalke 04 II (HT 1–0); Como **2–0**
      Cagliari (HT 1–0); Ascoli **2–2** Vicenza (HT 1–1).
- [x] `scripts/research-sync.sh 2026-11-21 --enqueue-only` → discovery job
      `research-sync-2026-11-21-1117` a workernek átadva, `done` (1 próba,
      0 hiba); a 4 eredmény `fact.match_result`-be került HT-vel, kiválasztott
      `fact.provenance` (`subject_kind=match`, `field_key=result`) sorral.
- [x] `reverify 11-21` → **950 meccs, 0 hiba**, 507 verified / 443 provisional /
      **withheld 0**; `integrity 11-13…11-28` → **OK**, mind a 8 mutató 0
      (1 pre-existing soccerway breaker). Az API `/v1/day/2026-11-21` mind a
      4 meccset lezártként, eredménnyel és publikációs állapottal adja.

## S98 — daemon-frissítés, research-ütemezés, validációs retenció

- [x] **Daemon frissítése**: `cargo build -p footballd -p footballctl
      --features obscura` → EXIT=0; `./scripts/install-footballd-agent.sh`
      → EXIT=0. Az új daemon pid 81213, port **56995**, `/v1/health` 200;
      a launchd `state=running`. Ez a bináris már tartalmazza az S90-es
      forrás-ref ország-guardot. A kliens újraindítva (pid 81263), az AX-probe
      újra a mai kártyákat mutatja.
- [x] **Research-szinkron automatizálása**: új launchd agent
      `hu.bandi86.football.researchsync` (5 slot: 14:15/16:15/18:15/20:15/
      22:15, plist telepítve, `launchctl print` 5 calendar-intervallumot mutat).
      A script és a `.env` a Library alá került (Documents/TCC elkerülése),
      `FOOTBALL_RESEARCH_ROOT` a valódi könyvtárra mutat. A próbafuttatás
      (`kickstart`) és az azt követő slotok is működnek: 11:38, 12:48, 14:28,
      14:44, 14:51, 15:14-i discovery jobok mind `done` (1 próba, 0 hiba).
      Dokumentáció: `docs/operations/research-sync.md` frissítve.
- [x] **Validációs retenció**: a `scripts/validation-retention.sh` első
      futtatása **4 óra 18 percig** akadt (a `fact.verification.last_run_id`
      FK-nak nem volt indexe, így a cascade-check 1,8 M soros seq-scant végzett
      törlésenként; a mozgó `now()-3 days` cutoff közben új sorokat is átlökött
      a határon: 890 ezről 1,75 M-re nőtt a halmaz). `pg_cancel_backend`-tel
      leállítva, teljes rollback (0 sor, 0 NULL hivatkozás). Új migráció
      `db/migrations/0023_retention_indexes.sql` (két index, 2,9 s) + a script
      rögzített cutoffot használ. Újrafuttatás: **16 s**, **1 794 798 run** és
      **2 728 968 finding** törölve, 265 610 `verification.last_run_id`
      NULL-ra állítva, mind auditálva (`validation_retention`,
      actor `ops-retention`). `ops.validation_run` 3 191 903 → 1 397 105;
      `ops.validation_finding` 4 838 544 → 2 109 576. A fájlméret 1,55 GB
      marad (plain `VACUUM`), de a felszabadított hely újrahasznosítható —
      a napi ~130 MB növekedés megállt. Új dokumentáció:
      `docs/operations/validation-retention.md`.
- [x] Ellenőrzés a retenció után: `integrity 11-19…11-22` → **OK**, minden
      mutató 0 (1 pre-existing soccerway breaker).

## Következő döntési pont

- [x] A daemon frissítése megtörtént (S98, port 56995, health 200).
- [x] A research szinkron automatizálása megtörtént (S98, launchd 5 slot).
- [x] A validációs retenció megtörtént (S98, 1,79 M run + 2,73 M finding).
- [x] A 45 pending review triage-a megtörtént (S92, 45 → 0).
- [x] A női versenyű meccsek férfi hozzárendelése megtörtént (S93, 12 → 0).
- [x] A 4 hiányzó eredmény pótlása megtörtént (S94, withheld 4 → 0).
- Nyitott: a párhuzamos munkamenet S96–S97c kódjai után a futó daemon már
  elavult lehet (a mi telepítésünk 11:23-as build volt); a frissítést az a
  munkamenet végzi, amelyik a kódot írta — ne tapossunk egymás buildjébe.
- A mai nap lejárt, eredmény nélküli meccsei (pl. mexikói Liga Premier
  `date_only` placeholderek, dél-koreai J/K-league élő meccsek) — a research
  13:00-s és 19:00-s refresh jobjai ütemezve vannak, nem kézi beavatkozás.

## S95 — felhasználói hibacsomag diagnózisa és javítása (nap-lista, szűrők, részletek)

A felhasználó 2026-11-22-i panasza (szó szerinti tartalommal): a tegnapi
(2026-11-21) 936 meccses napon duplikált meccsek és csapatok, hiányzó
eredmények, a La Liga fejlécben nincs forduló, az Alavés kétszer szerepel egy
„Logroño" csapattal, hamis „élő" állapot tegnapra, a Real Sociedad B és a
Real Sociedad 2 ugyanaz, a teljes algériai első osztály elmaradt, az
„eredmény nélkül" szűrő eredményes meccseket is mutat, és a részletes
meccsadatok (időjárás, stadion, bíró) hiányosak. Kérés: az agent maga találja
meg a gyökereket, ne a felhasználó nézze át a 936 meccset.

### Diagnózis (mért gyökerek)

- **Hamis „élő" és nyitott eredmények** — a research pipeline
  (`docs/api-export.py:1454-1482`) a múltbeli scheduled/live sorokat „stale"
  listára tette, majd `terminal=closed+len(stale)`-lel `data_status=complete`-et
  írt, miközben 136 sor valójában nyitott volt. A 11-21-es envelope egy
  fagyott `matches.json`-ból készült (a finalization fetch elhalt a
  `.football.lock`-on, a pipeline továbbment). Emellett a friss LiveScore
  eredményeket a dup-detektálás eldobta (`:1335-1337`), a scheduled soroknál
  minden score-t nullázott (`:791`).
- **Scheduled + mégis eredmény** — a v2 ingestion a Variant/Bridge linknél nem
  írta a kanonikus státuszt, de az eredményt igen
  (`rust/crates/ingest/src/canonicalize.rs`), ezért 26 lezáratlan sornak volt
  score-ja; a Real Sociedad B–Cádiz sor a két forrás egyesítése után is
  `scheduled` maradt 0-3-mal.
- **Bíró nem linkelődött** — a research referee megfigyelés
  `source_record_key`-e `"<uid>:referee"` volt, a linkelő viszont bare
  `<uid>`-ot vár (`rust/crates/ingest/src/research.rs`,
  `rust/crates/ingest/src/normalized.rs`), ezért a research játékvezetők
  egyáltalán nem linkelődtek.
- **Stadion/időjárás** — a research envelope 1024-ből 1024 venue és 398
  weather mezőt hozott, de a v2 adapter csak a bírót dolgozta fel, és nem is
  volt tábla a tárolásukra (`core.match.venue_entity_id` kitöltetlen).
- **Női meccsek férfi versenyben** — a tipplap `league_for`
  (`rust/crates/ingest/src/tipplap.rs`) a ligacím-prefix után a maradékot
  („női") tette `round_label`-nek, a férfi ligára képezte a versenyt, a
  csapatokat pedig fixen `gender=male`-ként küldte. 8 ilyen sor volt, ebből
  a „Logroño" és a „Lens/Montpellier", „Le Havre", „Eibar", „Real Sociedad",
  „Granada" párok.
- **La Liga fejléc és a „La Liga" név** — két külön ok: (a) a kevert
  `round_label` („13" + „női" + ∅) miatt a kliens fejléc-logikája nem adott
  feliratot; (b) a `rust/crates/ingest/src/competitions.rs` szótára a
  **nevet országtól függetlenül** írta át, ezért a venezuelai
  „Primera División" (VE) „La Liga" nevet kapott, és a kliens a spanyol
  bajnokság alá csoportosította.
- **Algéria** — a nyers FotMob `cancelled:true` + `reason=postponed` párost
  hozott; a research a booleanből `cancelled`-et, a labelből „Postponed"-ot
  csinált, ezért 8/8 meccs `cancelled` + 0-0 lett.
- **Szűrők** — a kliens „eredmény nélkül" szűrője a szerver `open_past`
  definícióját másolta (nem a `missing_result`-ot), és nem nézte a score-t;
  a gap panel „open" chipje is erre mutatott. Az „élő" szűrő/számláló
  időkorlát nélkül számolt, ezért egy múltbeli nap örökre élőnek látszott.
  A női verseny-felismerés csak a „(női)" markert ismerte.

### Kódváltozások

- `db/migrations/0021_match_environment.sql` — új `fact.match_environment`
  tábla (PK `(match_id, source_id)`, venue- és weather-oszlopok, CHECK-ek).
- `rust/crates/ingest/src/research.rs` — venue/weather megfigyelés a
  research envelope-ból; a referee `source_record_key` javítása bare uid-ra.
- `rust/crates/ingest/src/normalized.rs` — `NormalizedEnvironment`, referee
  link-kulcs.
- `rust/crates/ingest/src/canonicalize.rs` — environment perzisztálás és
  monoton `merge_status` (`Unchanged`/`Write`/`Frozen`): a Variant/Bridge
  linknél is frissül a státusz, de terminális→nyitott irány nem íródik felül.
- `rust/crates/ingest/src/tipplap.rs` — `WOMEN_LEAGUES` tábla, `LeagueAssignment`,
  női forduló-szűrés, `gender=female`/`branch=women` payload, parser 1.1.
- `rust/crates/ingest/src/competitions.rs` — `COUNTRY_SCOPED_OBSERVED_TITLES`
  (`laliga`, `primeradivision`): ország-ütközésnél nincs szótári átnevezés.
- `rust/crates/storage/src/repositories/match_detail.rs` + `openapi.rs` —
  a meccs-részletező válasz `venue` és `weather` mezője (LATERAL join, a
  research forrást preferálva; adat híján `null`).
- `scripts/db-check.sh` — várt táblaszám 48 → 49.
- Kliens: `MatchListView`, `MatchCardModel`, `HomeView`, `LeagueGroup`,
  `MatchDetailSheet`, `CompetitionCatalog`, `MatchDetailModels`,
  `VenueWeatherModels` (új) — szűrők, 6 órás élő-kapu, forduló-fejléc,
  gap panel, női markerek, venue/weather megjelenítés.

### Adatrevíziók (nem migrációval; dokumentált, visszajátszható)

- `round_label='női'` → `NULL` 4 soron (Ferencváros–Győri ETO 11-18,
  Real Sociedad–Sevilla, Granada–Real Madrid), a Le Havre W–Nantes W soron
  `'10'`; a „női" nem fordulószám.
- 144 női csapat `branch` mezője `senior` → `women` (a merge-team
  kontextus-guard miatt); Real Sociedad 2 / Real Sociedad B senior →
  `reserve`; Logroño → női; Real Madrid (INT) → ES.
- A 3 nem-spanyol „La Liga" verseny átnevezve: VE → „Primera Division:
  Clausura", CR/CL → „Primera División"; a VE példány beolvasztva a valódi
  VE versenybe (`identity merge-competition`).

### Összeolvasztott duplikátumok (mind `ops.dedupe_archive` /
`ops.entity_merge_archive` alatt, audit-azonosítóval)

- Meccsek: Lens (W)–Montpellier FC (W) ← tipplap Ligue 1 + Lens Féminin;
  Eibar W–Atlético W ← tipplap La Liga; Logroño W–Deportivo Alavés W ←
  tipplap La Liga + elavult espn sor; Real Sociedad B–Cádiz ← Real Sociedad 2;
  Al Jadida–Darnes ← tükrözött Group B sor; Recoleta–Guaraní ← scheduled sor;
  Pineto–Pescara, Chernomorets–Rilski, Gimnasia Jujuy–Madryn, NWSL döntő,
  Borussia M'gladbach II–Bonner SC, Eltersdorf–Bayern II,
  Levante Badalona–Depor, Burnley–Blackburn, Real Sociedad W–Sevilla W,
  Granada W–Real Madrid W, Le Havre W–Nantes W.
- Csapatok: Real Sociedad 2 + Real Sociedad B senior → Real Sociedad B
  (reserve); Logrono + Dux Logroño → Logroño W; Montpellier W →
  Montpellier FC (W); Real Madrid (INT) → Real Madrid W.

### Mérés a javítás után

- Research envelope 11-21 újragyártva (rev 83): 1024 → **897** sor,
  `stale_no_result` 136 → **7**, `data_status=complete` → **complete_with_gaps**,
  `day_closed=true`, live sor 0, scheduled+score 0, a pipeline saját 0,82-es
  duplikátum-kulcsával a duplikált párok **99 → 0**; az algériai 8 meccs
  `postponed` + score `null`, a Logroño egyetlen soron, Dundee–Hearts /
  Utrecht–Cambuur / Feyenoord eredménnyel; 897/897 detail-fájl konzisztens.
- `/v1/day/2026-11-21` (három nap újraingestje után): 936 → **923** sor;
  finished 739 → **863**, scheduled 133 → 29, live 41 → **1**, postponed 30;
  `open_past` 169 → **30**; `missing_result` 0; eredménnyel rendelkező
  nem-lezárt sor **0** (volt 29).
- `/v1/day/2026-11-20`: 269 sor (finished 262, postponed 4, scheduled 3),
  0 névpár-duplikátum, 0 eredményes nem-lezárt sor.
- `/v1/day/2026-11-22`: 630 sor (finished 50, scheduled 521, live 57, canceled 2),
  `open_past` 7, `missing_result` 0, 0 névpár-duplikátum, 0 eredményes
  nem-lezárt sor.
- `fact.match_environment`: 469 sor (468 venue, 390 weather a 11-21-re);
  a `/v1/match/{id}` válaszban a `venue` és `weather` objektum él, pl.
  „Estadio Ricardo Gregor", Luque, 4000 néző, `Thunder`, 27 °C, 86% pára.
- 11-22 indonéz félrekötés javítva: 3 fixture (Isenmulang Kalteng–Borneo
  Samarinda, Persib Bandung–Bali United, Borneo Samarinda–Persita) az UZ
  „Super League" alól az ID „Super League" alá került; az UZ verseny így
  15/15 üzbég, az ID 14 ID + 2 ismeretlen országú csapat. Az utolsó 40 napra
  futó rendszeres ellenőrzés (azonos országú csapatpár idegen versenyben,
  ≥3 találat) ezen kívül **nem talált** klasztert.

### Futott ellenőrzések

- `cargo fmt --all --check` → OK (FMT_OK_AFTER).
- `cargo clippy --workspace --all-targets -- -D warnings` → Finished,
  **0 figyelmeztetés**.
- `cargo test --workspace -- --test-threads=1` (env betöltve,
  `/tmp/s95-workspace-test.log`) → **512 passed, 0 failed**.
- `./scripts/db-reset.sh` → EXIT=0; `./scripts/db-check.sh` → **dev 49 tábla /
  15 985 744 üzleti sor**, test 0 sor, EXIT=0.
- `./scripts/check-foundation.sh` (db-reset után) → **EXIT=0**, 39 teszt-bináris.
- Swift: `swift test` (FootballAppCore) → **105 teszt / 15 suite passed**;
  `xcodebuild test -only-testing:FootballAppTests` → **TEST SUCCEEDED, 69 teszt,
  0 failure**; `xcodebuild build -scheme FootballApp` → **BUILD SUCCEEDED**.
- Daemon újratelepítve a javított binárissal (`install-footballd-agent.sh`,
  pid 36029, port 64426); a kliens újraépítve és újraindítva; a képernyőképen
  (`/tmp/s95-app-window.png`) a fejlécben „BOLÍVIA · 26. forduló" látszik, az
  AX-próba a La Liga kártyán „13. forduló"-t olvas.

### Nyitott tételek (döntést igényelnek, nem lettek elfedve)

1. **Bíró-entitás feloldás**: a research 11-21-re 294 meccshez ad bíró-nevet,
   de a 293 pending review-ból csak 1 egyezik pontos névvel a meglévő 1195
   bíró-entitással (a névformátum eltér a footballdata-étól). Auto-create
   naponta ~293 entitást hozna létre, ezért ez külön identitás-politikai
   döntés; addig a részletező a linked bírót mutatja, a research nevét nem.
2. **Női tipplap csapat-feloldás**: a mai (11-22) női tipplap sorok
   (Granada, Sevilla, Tenerife, …) csapatnevei nem oldhatók fel a kanonikus
   „… W" entitásokra, ezért review-sor nyílik és a meccs kimarad.
3. **Verseny-duplikátumok országonként**: több országban 2-3 azonos nevű
   „Primera División" entitás él (CL 2, BO 2, CR 3); a szélesebb
   competition-dedupe nem része ennek a szeletnek.
4. **Forrás-konfliktus a női Liga F 12. fordulójában**: a soccerway
   „Eibar N–Levante N" / „Cacereno N–Alavés N" sorokat ad, amelyek a valós
   fordulóval ütköznek; automatikus feloldás nincs, a „Lejárt, nyitott"
   szűrő mutatja őket.
5. **Tipplap múltbeli napok**: a tipplap oldal csak a mai napot szolgálja ki
   (dátum-guard), ezért a múltbeli tipplap sorok javítása csak merge/revízió
   útján lehetséges — új funkció (replay) nélkül.
6. **Történelmi verseny-ország újrafeloldás**: 6030 research megfigyelés
   háromjegyű országkóddal (`IDN`, `VEN`, `BUL`, …) készült; a mostani kód
   (`canonical_country_code`) ezeket ISO2-re oldja, de a korábban létrehozott
   verseny-hozzárendeléseket nem írja felül automatikusan. A mért klasztert
   (indonéz sorok az UZ „Super League" alatt) kézzel javítottuk; szélesebb,
   visszajátszható újrafeloldás külön szelet.

## S96 — nyitott kérdések és projekt-előrelépés (kontrakt, identitás, dedupe-parancs, CI)

Induló állapot: a felhasználó kérése („vizsgáld tovább a bugokat, de közben
haladj a projekttel és a nyitott kérdésekkel is"). A szelet a nyitott tételek
közül a kódoldalon megoldhatókat zárta le, és a maradékot döntési pontokra
bontotta.

### Megvalósított szeletek

- [x] **Match-detail kontrakt-teszt** (P1 roadmap): új fixture
  `tests/fixtures/contract/match-detail-response.json` a futó daemon valós
  válaszából (66 felső kulcs, venue/weather/incidents/tabella), Rust
  round-trip teszt a storage-structokon (`match_detail.rs` contract_tests), és
  Swift dekódoló teszt ugyanarra a fixture-re
  (`DetailContractTests.swift`, 4 teszt). A kulcshalmaz-teszt azonnal talált
  egy valódi driftet: a backend `competition_id`-t küld, a kliens nem
  dekódolta — javítva (`MatchDetail.competitionId`, opcionális a régi
  fixture-ök miatt).
- [x] **Bíró-entitás feloldás (gyökér)**: a linker 4–5. lépése fixen
  `kind='team'`-re szűrt, ezért referee-re az egzakt-név és a fuzzy lépcső
  strukturálisan nem működött (minden research bíró `NotFound` → review).
  Most a név/fuzzy lépcső `entity_kind`-ra szűr, a fuzzy jelöltlista
  fajtaszűrt (team már nem lehet referee-jelölt), a referee hívó a meccs
  versenyének országát adja kontextusként, és referee-re csak TELJES név
  egzakt egyezése linkel automatikusan (rövid alak → review, AGENTS.md §6).
- [x] **Női csapatnév-feloldás**: női kontextusban a linker a jelölő nélküli
  névre a `fold+név+"w"` / `+"women"` egzakt kulcsokat is kipróbálja (nem
  fuzzy merge), így a tipplap „Granada" a kanonikus „Granada W"-ra linkel.
  Mért hatás a jelenlegi queue-n: 26 pending team-sorból 13 egyértelműen
  linkelhető, 3 ambiguous marad review, 10 hosszabb katalógusnév miatt
  továbbra is nyitott.
- [x] **`footballctl dedupe aliases --from … --to … [--apply]`**: a cross-source
  névváltozat-duplikátumok (FC Cologne / FC Koln, Kuching FA / Kuching City)
  bizonyíték-alapú felismerése és merge-elése: pontos kickoff + foldolt
  versenynév + azonos státusz/eredmény + közös csapat-entitás horgony + a
  névváltozat-oldalon kontextus-egyezés és ≥0,55 név-hasonlóság (2·LCS/alapú).
  A parancs a meglévő `merge_duplicate` + `merge_team_via_alias` utakat
  használja; dry-run a 11-19..11-23 tartományra 0 csoportot ad (a kézi
  takarítás után ez a várt érték).
- [x] **CI**: a workflow megmaradt (nem hiányzott), a valódi hiány a magyar
  üzemeltetési leírás volt → `docs/operations/ci.md` (209 sor). A Rust job
  PostgreSQL image-e a helyi PG18-hoz igazítva (`postgres:18-alpine`),
  a két job neve változatlan (branch protection), a lánc a helyi kapukat
  futtatja 1:1.

### Adattisztítás ebben a szeletben

- 11 „ghost" (0 meccses) verseny-entitás beolvasztva a több meccses párjába
  (`identity merge-competition`, mind „merge utáni duplikált meccscsoportok: 0");
  2 kivétel (`Group K`, `Group L`): az entitásnak nincs verseny-sora.
- Cross-source duplikátumok: 28 csapat-entitás merge (`identity merge-team
  --via-alias`, bizonyíték-szöveggel) + 27 fixture-merge (`dedupe pair`).
  A mai archívum-számlálók: `ops.dedupe_archive` 184 sor,
  `ops.entity_merge_archive` 48 sor (2026-11-22).
- Ellenőrzés: `dedupe matches --date 2026-11-19 --to 2026-11-23` →
  „nincs duplikált meccs"; `dedupe variants --date 2026-11-21` →
  „nincs név-variáns duplikátum"; a nap-listákban 0 névpár-duplikátum,
  0 nem-lezárt-eredményes sor.

### Mért napi állapot (a szelet végén, `/v1/day`)

| nap | sor | finished | scheduled | live | postponed | dup | nonterminal+eredmény |
|---|---|---|---|---|---|---|---|
| 11-19 | 102 | 69 | 31 | 0 | 2 | 0 | 0 |
| 11-20 | 257 | 250 | 3 | 0 | 4 | 0 | 0 |
| 11-21 | 909 | 850 | 28 | 1 | 30 | 0 | 0 |
| 11-22 | 629 | 50 | 520 | 57 | 0 | 0 | 0 |
| 11-23 | 98 | 0 | 98 | 0 | 0 | 0 | 0 |

(A 11-21-i sorszám a korábbi 923-hoz képest a 27 duplikátum-merge miatt csökkent;
a `live` 1 sor az Independiente Medellin–Santa Fe, amelyre nincs frissítő
forrás — a kliens 6 órás `isLiveNow` kapuja kezeli.)

### Futott ellenőrzések (mért)

- `cargo fmt --all --check` → OK; `cargo clippy --workspace --all-targets -- -D warnings` → 0 warning.
- `cargo test --workspace -- --test-threads=1` (db-reset után) → **539 passed / 0 failed** (105 test-binary).
- `./scripts/db-reset.sh` → exit 0 (21 migráció); `./scripts/check-foundation.sh` → **exit 0**; `./scripts/db-check.sh` → dev 49 tábla / test 0 sor.
- Swift: `swift test --package-path apps/FootballAppCore` → **109 teszt / 16 suite passed**;
  `xcodebuild test -only-testing:FootballAppTests` → **69 teszt / 0 failure**.
- `dedupe aliases` dry-run (11-19..11-23) → 0 csoport; a dev DB írás nélkül.

### Nyitott tételek (döntést igényelnek)

1. **Bíró auto-create vs review**: a research napi ~168 új bírónévhez nincs
   katalógus (477 nem-GB név); a mai szabály szerint mind review marad. Döntés
   kell: auto-create (napi ~168 entitás) vagy szűkebb, országos katalógus.
2. **Bíró-katalógus névformátuma**: 983/1195 entitás rövid alakú
   (`M Oliver`), a research teljes nevet ad (`Michael Oliver`) — a rövid alak
   csak review lehet; a katalógus normalizálása/alias-seedelése külön szelet.
3. **Referee-ország tárolása**: nincs `core.referee` / `core.entity.country_id`,
   ezért az „ország-egyezés" a referee-ágon ma látens (a review-kontextusba
   viszont bekerül a verseny országa).
4. **A 26 meglévő pending team-review sor takarítása** (13 linkelhető, 3
   ambiguous, 10 hosszú név) — az Exact út nem zárja le a queue-sort.
5. **`dedupe aliases --apply` éles futtatása** csak a következő valós
   cross-source hullámnál lesz értelmes; a hamis-pozitív arány élesben még
   nem mért.
6. Korábbi tételek változatlanok: országonkénti competition-duplikátumok,
   Liga F forrás-konfliktus, tipplap múltbeli napok, történelmi verseny-ország
   újrafeloldás.

### Visszaállás

- Kód: a szelet commitja egyben visszafordítható (`git revert`); a
  `dedupe aliases` és a linker-változások migrációt nem hoztak.
- Adat: minden entitás/fixture merge az `ops.entity_merge_archive` és
  `ops.dedupe_archive` táblában payload-dal együtt visszajátszható.

## S97 — retry-policy, játékvezető megjelenítés, verseny-duplikátumok és női csapatmarkerek

Induló állapot: a napi lista adatszinten tiszta volt (0 névpár-duplikátum a
11-19..11-23 tartományban), de három nyitott osztály maradt: a worker hot
loopja, a játékvezető hiánya a részletezőben, és a forrásokon átnyúló
verseny-/csapatnév-duplikátumok. Cél: ezeket gyökerüknél javítani, mérni, és a
szeletet zöld kapuval lezárni.

### Kódváltozások

- `rust/crates/worker/src/tick.rs`: a kvóta/delay-blokk nem a változatlan
  `due_at`-tel állítja vissza a jobot `Pending`-be, hanem
  `repo::requeue_with_delay`-jel — kvóta esetén a következő UTC-éjfélre, delay
  esetén `now + max(min_delay_ms, 1000) + jitter`-re. Új
  `MAX_TRANSIENT_ATTEMPTS = 8`: ennyi tranziens hiba után a job `Failed`
  státuszt és magyar hibaüzenetet kap. 4 új unit teszt.
- `db/migrations/0022_match_official.sql` + `canonicalize.rs`: új
  `fact.match_official` tábla (match, source, role, name, captured_at). A
  bíró neve akkor is megjeleníthető, ha nincs entitás-link; a link továbbra is
  fail-closed (nincs automatikus fuzzy). A `link_referee` paraméterlistája
  `RefereeLink` structba került (clippy).
- `rust/crates/storage/src/repositories/match_detail.rs`: a részletező
  `referee` mezője `COALESCE(referee_entity.canonical_name, official.name)`,
  LATERAL joinnal a `fact.match_official`-ra (research elsőbbség).
- `rust/crates/ingest/src/competitions.rs`: szlovák élvonal
  (`Super Liga` / `Niké Liga` / `1. Liga` → **1. Liga**) és görög élvonal
  (`Super League` / `Greek Super League` / `Super League Greece` →
  **Super League**) szótári egységesítés, ország-kötött feloldással
  (`COUNTRY_SCOPED_OBSERVED_TITLES`), hogy más ország azonos nevű versenyét ne
  nevezze át. Új teszt: SK/GR egységesítés + RS/MD „Super Liga" marad +
  görög másodosztály nem olvad.
- `rust/crates/storage/src/repositories/competition_merge.rs`: a beolvadó
  `core.competition` sor a merge tranzakcióban **törlődik** (a meccsek és
  szezonok már a survivor alatt vannak). Enélkül a megmaradó sor tovább élt, és
  egy későbbi ingest újra rá köthette a meccseket — a kliensben ugyanaz a liga
  két csoportban jelent meg.
- `rust/crates/storage/src/repositories/integrity.rs`: új
  `same_pair_same_day_groups` metrika — ugyanaz a csapatpár ugyanazon az
  UTC-napon több sorban, akkor is, ha a két forrás eltérő csapat-entitást
  használt (cross-source névváltozat). A `footballctl integrity` kiírja; nem
  kemény hiba, hanem review-jelzés.
- `scripts/db-check.sh`, `rust/crates/storage/tests/connection.rs`: 49 → **50**
  terméktábla (0001–0022 lánc).

### Adatrevíziók (mind dokumentált, visszajátszható)

- 4 „N" utótagú női csapat átnevezése „(W)" alakra (Beleza, C-Osaka, Saitama,
  Kickstart), alias-bejegyzéssel.
- 13 női liga-verseny egységesítése (Liga F, Frauen-Bundesliga, Eredivisie
  Vrouwen, WE League, A-League Women, A-Liga/B-Liga Women, Première Ligue,
  IWL Women, Liga MX Femenil, Liga FPD, Women's FA Cup, Women's Serie A);
  4 esetben az ország-kódot előbb a survivor országára kellett írni.
- ~101 női csapat-merge (marker-variánsok: „X W", „X (W)", „X N", marker
  nélküli) + 17 azonos klubú iker merge (Monaco, St. Truiden, Göztepe,
  Barcelona (W), Sporting CP, Vitesse stb.) — a `merge-team` guard a
  kontextust (ország/nem/korosztály/ág) nézi, ezért ország-revízió kellett.
- Duplikált verseny-entitások: SK „Super Liga" → „1. Liga", görög
  „Greek Super League" + duplikált „Super League" → „Super League", LV „Cup"
  duplikátum; a 101 merge-ölt entitású, meccs/szezon nélküli verseny-sor
  törlése; két téves, országokon átnyúló korábbi merge (SK 1. Liga → CZ,
  LV Cup → SI) visszaállítása `merged_into_id = NULL`-lal.
- Fixture-duplikátumok: SWF Scottish Cup Rossvale–Gartcairn (date_only 00:00 vs
  14:15 pontos kezdés) és Segunda División (VE) Deportivo Lara–Aragua FC
  (a forrás 21:22Z-ről 22:00Z-re pontosította a kezdést) — `dedupe pair`,
  a friss/points kezdésű sor maradt.
- `dedupe aliases --from 2026-11-01 --to 2026-11-25 --apply`: 11 csoport
  (1 csapat+fixture, 10 fixture-twin); utána dry-run 0.

### Mérés (dev DB, `/v1/day`, daemon 61615)

| nap | sor | státuszok | open_past | missing_result | dup | nonterm+eredmény |
|---|---|---|---|---|---|---|
| 11-20 | 255 | 248 finished, 4 postponed, 3 scheduled | 3 | 0 | 0 | 0 |
| 11-21 | 900 | 844 finished, 29 postponed, 26 scheduled, 1 live | 27 | 0 | 0 | 0 |
| 11-22 | 621 | 94 finished, 455 scheduled, 70 live, 2 canceled | 22 | 0 | 1 → 0 | 0 |
| 11-23 | 98 | 98 scheduled | 0 | 0 | 0 | 0 |

- `footballctl integrity --from 2026-11-01 --to 2026-11-30`: duplikált
  kickoff 0, duplikált fixture 0, azonos csapatpár azonos napon 0, árva
  provenance 0, aktív ref összevont entitásra 0, withheld gate 0, failed job 0
  → **verdict: OK** (függő review 602).
- `fact.match_official`: 347 sor / 347 meccs; a részletező API élőben:
  „Tromsø vs Viking | referee: Tore Hansen | venue: Romssa Arena";
  „Burton vs Luton | referee: Dean Whitestone | venue: Pirelli Stadium".
  `referee_entity_id` 1/1535, `venue_entity_id` 0/1535 — szándékosan szigorú
  identitás-policy, dokumentált korlát.
- Worker: a beragadt soccerway job `due_at`-je a következő UTC-éjfélre állt,
  az `attempts` 363-nál megállt (nincs több hot loop).

### Futott ellenőrzések

- `cargo fmt --all --check` → OK; `cargo clippy --workspace --all-targets -- -D
  warnings` → 0 figyelmeztetés.
- `./scripts/db-reset.sh` → 0; `cargo test --workspace -- --test-threads=1` →
  **561 passed / 0 failed / 105 teszt-bináris**.
- `./scripts/check-foundation.sh` → 0 (dev 50 tábla / 16,0 M üzleti sor, test 0).
- `sqlx migrate run --source db/migrations` → 0022 applied; `pg_tables` 50.
- Élő API-ellenőrzés a futó daemonon (port 61615) — lásd a mérésnél.

### Nyitott tételek

1. `Deportivo Lara`-féle „azonos csapatpár azonos napon" eset most 0, de a
   detektor csak jelent, nem javít; a javítás kézi `dedupe pair`.
2. A `same_pair_same_day_groups` hamis-pozitív aránya élesben még nem mért
   (ifjúsági tornák, ahol egy csapat naponta kétszer játszhat — jelenleg 0).
3. Bíró-identitás policy: auto-create (~168 új entitás/nap) vs review;
   referee-ország séma (migráció) továbbra is döntés előtt.
4. 602 függő identitás-review (főleg research bíró-nevek) takarítása.
5. Kliens: a Swift kód a HEAD-en van, de a futó app-bináris a S95-ös build —
   újrabuildelés és AX-ellenőrzés a következő szelet elején.
6. Korábbi tételek: országonkénti competition-duplikátumok maradéka,
   Liga F forrás-konfliktus, tipplap múltbeli napok, 565+ referee review.

### Visszaállás

- Kód: a szelet commitja `git revert`-tel visszafordítható; a 0022 migráció
  visszaállítása `DROP TABLE fact.match_official;` + migrációs sor törlése.
- Adat: minden entitás/fixture merge az `ops.entity_merge_archive` és
  `ops.dedupe_archive` payloadjaival visszajátszható; a 101 törölt verseny-sor
  a merge-archívum payloadjából újraépíthető.

## S97b — azonos pillanatú duplikátumok, bíró-review flood leállítása

### Kód

- `rust/crates/storage/src/repositories/integrity.rs`: új
  `same_pair_same_instant_groups` metrika — ugyanaz a csapatpár pontosan
  ugyanabban a kickoff-pillanatban, verseny-entitástól függetlenül. Egy klub
  nem játszhat két meccset ugyanabban a percben; ez a cross-competition
  duplikátum osztály, amit a versenyre szűkítő kulcsok nem látnak. Kiírva a
  `footballctl integrity`-ben (`azonos csapatpár azonos pillanatban`).
- `rust/crates/identity/src/linker.rs`: `ResolveOptions { open_review }` +
  `resolve_with_options(...)`; a `resolve` változatlan viselkedéssel hívja
  (default `open_review: true`). Re-export a `lib.rs`-ben.
- `rust/crates/ingest/src/canonicalize.rs`: a referee-ág `open_review:
  source_code != "research"` — a research bíró-neve megjelenítés-only (a
  forrás alakját a `fact.match_official` tárolja), ezért nem nyit
  feldolgozatlan review-sort. Ezzel a napi ~150-200 soros flood megszűnt.

### Adat

- U17-es világbajnokság csoport-duplikátumok mergelve (entity-id alapon):
  Group I `01a1fb4a-1c14` → `8c0327ff` (1 meccs, a Tanzania U17–Costa Rica U17
  duplikátum is összeolvadt), Group K `01a1fb4a-1c91` → `e6014534`,
  Group L `01a1fb4a-1c9f` → `531e3373`.
- `dedupe aliases --from 2026-11-01 --to 2026-11-30 --apply`: 4 csoport
  (2 csapat+fixture: Maghreb As De Fès/MAS Fès, Antigua GFC/Antigua Guatemala;
  2 fixture-twin: Bundesliga és Süper Lig), ismételt apply → 0 csoport.
- Az 573 research referee pending review-sor elutasítva
  (`decided_by='agent:S97'`, note: megjelenítés-only, policy döntés előtt);
  állomány most: team pending 29, referee pending 0.

### Mérés

- `cargo test --workspace -- --test-threads=1` → 562 passed / 0 failed /
  105 bináris; `cargo fmt --all --check` → OK; clippy 0 warning;
  `check-foundation.sh` → 0; db-check 50 tábla.
- `footballctl integrity --from 2026-11-01 --to 2026-11-30` → minden számláló
  0 (duplikált kickoff, duplikált fixture, azonos pillanat, azonos nap, árva
  provenance, összevont entitásra mutató ref, withheld gate, failed job),
  verdict OK.
- Újraingest (2026-11-22, job `fb45a029`) után: referee pending review **0**
  (előtte 573), a detail API továbbra is adja a bírót
  (`Allen Chapman`), a venue-t (`Nu Stadium`) és az időjárást.
- `/v1/day/2026-11-22` → 620 sor {scheduled 454, finished 94, live 70,
  canceled 2}, 0 névpár-duplikátum.

### Nyitott

1. Bíró-identitás policy: auto-create (~168 entitás/nap) vs review-only vs
   csak megjelenítés — a mostani állapot a „csak megjelenítés" (a katalógus
   bővítése külön döntés).
2. Referee-ország tárolása (séma-migráció) a pontosabb linkeléshez.
3. A 983 rövid alakú („M. Marchetti") bíró-entitás normalizálása.
4. 29 team pending review sorsa.
5. Korábbi tételek: competition-duplikátumok maradéka, Liga F
   forrás-konfliktus, tipplap múltbeli napok, detail-sheet vizuális ellenőrzés.

## S97c — szezonhatáron átnyúló fixture-duplikátum, review-queue tisztítás

### Kód

- `rust/crates/storage/src/repositories/dedupe.rs`:
  - `duplicate_fixture_groups` csoportkulcsa már NEM tartalmazza a `season_id`-t
    (verseny + csapatpár + UTC-nap). Ugyanaz a verseny + csapatpár + nap nem
    lehet két külön szezonban valódi; a szezon-eltérés forrás-feloldási hiba.
  - új `is_same_instant_fixture(...)`: azonos verseny + azonos orientációjú
    csapatpár + PONTOSAN azonos `kickoff_at` + egyező (vagy hiányzó) eredmény.
    A szezont nem nézi, a tükrözött párt továbbra is elutasítja.
- `rust/bins/footballctl/src/main.rs`: a `dedupe matches --apply` őr-lánca
  `is_same_fixture || is_same_instant_fixture` (a felülvizsgált `--review-note`
  út változatlan).
- Új teszt: `different_season_same_instant_is_one_group_and_mergeable`
  (`rust/crates/storage/tests/dedupe_fixtures.rs`) — bizonyítja, hogy a régi
  csoportosítás nem látta, az új igen, és a merge lefut.

### Adat

- `Lorient – Rennes` duplikátum (a `Stade Rennais` exonym entitás korábban
  Rennes-be merge-ölve, de a fixture külön soron maradt eltérő `season_id`-vel):
  `dedupe pair --survivor 01a1e9ef-9c3a-73c5-a828-869f323ecc63 --duplicate
  01a1fc59-c190-7711-b320-5c4fc2e5258d`, audit `01a203a4-8c94-76dd-bdd6-1d9f0b8a0997`.
- `identity.review_queue`: a 29 elavult tipplap team-sor elutasítva (a javított
  női marker-feloldás óta a forrás linkel; ha mégsem, a következő ingest újra
  nyit sort). Állomány: **0 pending** (team 0, referee 0).

### Mérés / ellenőrzés

- `cargo test --workspace -- --test-threads=1` → **564 passed / 0 failed**
  (105 bináris); `dedupe_fixtures` 8/8.
- `footballctl dedupe matches --date 2026-09-01 --to 2026-11-30` → nincs
  duplikált meccs; `integrity` 2026-09-01..11-30 → minden számláló 0,
  függő review 0, verdict OK.
- Élő nap (2026-11-22): 70 `live`, ebből 6 órán túli elavult **0**; 25 múltbeli
  `scheduled` — ellenőrizve a research envelope-ban (u.a. `scheduled`, score
  null, csak livescore id): a forrás még nem adott eredményt, nem ingest-hiba.
  A research envelope mai állapota `data_status=partial` (a nap még nyitva).

### Nyitott

1. Bíró-identitás policy (auto-create vs megjelenítés-only) — jelenleg
   megjelenítés-only, a `fact.match_official` hordozza a nevet.
2. Referee-ország séma + a 983 rövid alakú bíró-entitás normalizálása.
3. ~~`dedupe variants` csak egy napra fut~~ — kész: `--to` tartomány-flag
   (2026-06-01..11-30 vizsgálat: 0 név-variáns duplikátum).
4. Korábbi tételek: competition-duplikátumok maradéka, Liga F forrás-konfliktus,
   tipplap múltbeli napok, detail-sheet vizuális ellenőrzés.

## S99 — kezdőlap vizuális rendbetétele

### Induló állapot és tervezett szelet

- A felhasználó a megadott képernyő alapján a főoldali menüsor ismétlődésének
  megszüntetését, a logóhiány-számlálók elhagyását, a dátumfejléc csinosítását
  és a verseny-/meccskártyák nagyobb logóit kérte.
- A főoldal két helyen mutatta az adatminőség-szűrőket: a szűrősorban és a
  „Hiányzó adatok” sorban. A napi lista csapatcímerei 18 pt-osak, a versenycímerek
  32 pt-osak.
- Tervezett egyetlen szelet: a szűrők egyszeri, logó-számlálók nélküli
  megjelenítése; gazdagabb napfejléc; nagyobb címer és levegősebb meccssor.
- Kezdő branch: `feat/homepage-visual-refresh` (`main` érintetlen marad).
- [x] A kezdőlap vizuális módosításai és a szükséges UI-ellenőrzés.

### Záró eredmény

- `apps/FootballApp/HomeView.swift`: a napi főoldal kapott egy grafit/borostyán
  fejléckártyát napszakhoz illő ikonnal, nap-, élő- és odds-számlálóval,
  kiemelt napváltókkal; keskeny ablakban a fejléc egymás alá rendeződik. A felső
  „Ma” menüpont „Főoldal” lett, a hiány-szűrő ismétlődő második sora megszűnt.
- A „Logó nélkül” és „Ligalogó nélkül” szűrő megmaradt, de számláló nélkül.
  A versenycímerek 32→40 pt-ra, a csapatcímerek 18→26 pt-ra nőttek; a meccssor
  térköze és a kártyák íve is nagyobb lett.
- `apps/FootballAppUITests/FootballAppUITests.swift`: a fejléc-számlálókat és a
  számláló nélküli logószűrőket ellenőrző állítások; a csoportforduló lekérdezése
  a SwiftUI szöveg AX-elemét vizsgálja. A 11-20-i live fixture 11-22-én már
  helyesen „Ütemezett” az elavult-live védelem miatt; a teszt ezt méri.
- [x] `cd apps && xcodegen generate` → projekt sikeresen generálva.
- [x] `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path apps/FootballAppCore`
  → **109 teszt, 0 hiba**.
- [x] `./scripts/run-ui-tests.sh -parallel-testing-enabled NO` → **69 app teszt
  + 8 UI teszt, 0 hiba; TEST SUCCEEDED**.
- [x] `git diff --check` → **0 whitespace hiba**.
- [x] Valódi macOS-ablak ellenőrzése: a 2026-11-22-i nap betöltődött
  (**619 meccs, 70 élő, 283 oddsadattal**); a magyar feliratok, az egyszeri
  navigáció/szűrősor, a logó-szám nélküli szűrők és a nagyobb logók láthatók.
- Nem ellenőrzött: a VoiceOver beszédhangja és a kompakt ablakméret vizuális
  ellenőrzése (a valódi ablak teljes méretű maradt); a CUA-ablakkép nem került fájlba.
- Visszaállás: a változások a `feat/homepage-visual-refresh` branch-en vannak;
  a `main` változatlan.
- Commit: `ef9e6b1` (`feat: refresh homepage visual design`), felpusholva az
  `origin/feat/homepage-visual-refresh` ágra.

## S100 — élő, dinamikus fejléc (időjárás + napszak + visszaszámláló)

### Induló állapot és döntés

- A felhasználó kérése: „Most csak foglalkozzunk a megjelenitessel. A felső
  headert legyen szép dinamikus, live weather, animációk, színek napszaktól /
  időjárástól függően, ikonos statisztikák, visszaszámláló, fancy; a kártyán
  csak a hover változzon."
- Architektúrális egyeztetés (a kliens alapból csak `127.0.0.1` API-t használ):
  a felhasználó az **Open-Meteo, kulcs nélküli, read-only** forrást választotta.
  Ez az egyetlen külső hívás a kliensben; football-adat továbbra is csak a
  helyi API-ról jön. Hálózati hiba esetén a fejléc napszak-alapú palettára
  esik vissza, és ezt a felület szövegesen is jelzi („napszak alapján").
- Branch: `feat/homepage-visual-refresh` (változatlan).

### Változott

- Új Core-réteg (Swift 6, tesztelt, SwiftUI-mentes):
  `WeatherModels.swift` (CurrentWeather, OpenMeteoResponse, WMO → `WeatherKind`),
  `WeatherService.swift` (`WeatherService` protokoll + `OpenMeteoWeatherService`,
  5/8 mp időkorlát, Budapest alaphelyszín), `HeaderMood.swift`
  (`DayPhase`, `HeaderPalette`, `HeaderParticle`, determinisztikus `resolve`).
- Új app-réteg: `WeatherModel.swift` (@Observable, 15 perces háttér-ciklus,
  hiba esetén megtartja az utolsó megfigyelést), `HomeHeader.swift`
  (ikonos stat-téglák, időjárás-tégla, kattintható visszaszámláló, a régi
  `MonthCalendar` ide költöztetve), `HeaderAtmosphere.swift`
  (Canvas: csillagok, lebegő fények, sugarak, eső, hó, villám — teljesen
  statikus `reduceMotion` módban).
- `HomeView.swift`: a régi fejléc-blokk és a szűrők külön fájlokba kerültek
  (`FilterBar.swift`); a `FilterBar` publikus lett a más fájlból való használathoz,
  és kikerült belőle egy felesleges `?? 0`. Az app betölti az időjárást.
- `MatchListView.swift`: a kártya hover-e most enyhe borostyán tónus + hajszál
  szegély + 1,003-as skálázás (a billentyűfókusz vastag kerete változatlan).
- **Saját helyzet (CoreLocation)**: `UserLocationProvider.swift` egyszeri,
  „használat közbeni" engedéllyel kért pozíciót fordított geokódolással városnévre
  alakít; 12 mp időtúl, illetve engedély-megtagadás esetén `nil`-t ad, és ilyenkor
  a fejléc megmarad az alaphelyszínen. `WeatherModel.useLocation()` új szolgáltatást
  épít az új koordinátákkal, és a tégla a VO-címkében külön kimondja a
  „saját helyzet" / „alaphelyszín" eredetet. Az engedély szövege a generált
  `Info.plist`-be került a `project.yml`-en át. A koordinátákat nem tároljuk,
  nem naplózzuk és nem küldjük sehova — csak az Open-Meteo URL-ben használjuk.
- Megőrzött azonosítók: `home.stat.matches|live|odds`, `nav.today`, `nav.calendar`,
  `nav.*`, `filter.*`. Új: `home.stat.weather`, `home.stat.refresh`.

### Futott ellenőrzések (mért)

- `cd apps && xcodegen generate` → projekt generálva.
- `swift test --package-path apps/FootballAppCore` → **120 teszt / 18 suite, 0 hiba**
  (új: `HeaderMoodTests` 8, `WeatherServiceTests` 3).
- `xcodebuild build … -scheme FootballApp` → **BUILD SUCCEEDED**, 0 fordítási hiba.
- `xcodebuild test … -only-testing:FootballAppTests` → **73 teszt, 0 failure**
  (új: `WeatherModelTests` 4, köztük a saját-helyzet váltás).
- `./scripts/run-ui-tests.sh -parallel-testing-enabled NO` → **73 app + 8 UI teszt,
  0 failure; TEST SUCCEEDED**.
- `git diff --check` → **0 whitespace hiba**.
- Valódi macOS-ablak: az app új binárissal indítva, a napi lista betöltött
  (**619 meccs / 93 élő / 283 oddsadattal**). A képernyőképen
  (`/tmp/header-preview2.png`) a meleg „aranyóra" paletta, az élő időjárás
  („7 °C · Borult", Open-Meteo), a `00:03`-ról induló élő visszaszámláló és az
  ikonos téglák láthatók; a régi szöveges meta-sor a fejléc alatt eltűnt.
- Keskeny ablak (820×700) ellenőrzése: `/tmp/header-compact.png` — a dátumvezérlők
  egymás alá rendeződnek, a stat-téglák vízszintesen görgethetők, a szűrők is
  görgethetők. Az ablak visszaállítva 1480×1000-re.
- **Saját helyzet élőben**: az újraindított app a gép pozícióját feloldotta, és
  az AX-próba ezt adja: `Időjárás: 5,6 °C, Eső, Cserszegtomaj, saját helyzet,
  élő adat` (`swift scripts/ax-probe.swift FootballApp home.stat.weather`).
  A képernyőkép (`/tmp/location-preview.png`) az eső-időjárás kék palettáját és
  a lefutó eső-animációt is mutatja — ez az időjárásfüggő animáció élő bizonyítéka.

### Nem ellenőrzött

- Eső animációja élőben ellenőrizve; hó és zivatar (villanás) csak
  egység-teszttel (paletta/animáció-típus) lefedve. A VoiceOver beszédhangja és
  a `reduceMotion` vizuális képe nem lett emberileg lejátszatva.
- A macOS engedély-dialógus megjelenését a tesztelő gépén nem lehet
  képernyőképpel rögzíteni (a meglévő engedély miatt nem jelent meg újra);
  a `nil`-re eső, illetve időtúl-eső ágat unit teszt fedi le.
- A countdown a kliens 60 mp-es élő körét, illetve a worker 6 órás sávját
  becsüli; a backend nem közöl explicit „következő frissítés" időpontot.

### Kockázat / visszaállás / következő döntési pont

- Kockázat: egyetlen új külső, read-only HTTPS-hívás; ha a hálózat lassú,
  a fejléc 5 mp-ig a napszaki háttért mutatja. Open-Meteo downtownolás esetén
  nincs adat, de nincs hamis adat sem.
- Visszaállás: minden változás a branch-en van, `git revert`-tel visszafordítható;
  az új szolgáltatásnak nincs migrációja, kontraktja vagy adatírása.
- Következő döntési pont: az időjárás-hívás átkerülhet a backendbe (a kliens
  így teljes egészében a `127.0.0.1` API-ról kapna adatot); a countdown pedig
  valódi backend-ütemezési mezőre köthető.
- A mai munka áttekintése: az S91–S98 commitok (12 db) már a `main`-en vannak;
  az egyetlen nem mergelt mai munka ez az ág (S99 + S100). A `main` közvetlen
  őse ennek az ágnak, így a merge fast-forward, ütközés nélküli.
- Commit + merge: a felhasználó döntése szerint az S100 commitolásra kerül az
  `feat/homepage-visual-refresh` ágon, majd fast-forward merge a lokális
  `main`-be; az `origin/main` push nem történik meg (külön döntés).
