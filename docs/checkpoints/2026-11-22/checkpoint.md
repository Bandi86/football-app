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

## Következő döntési pont

- A futó daemon binárisa 2026-11-21 18:33-as, az utolsó Rust feat-commit
  18:41-es (S90 forrás-ref ország-guard) — a frissítés
  `cargo build … && ./scripts/install-footballd-agent.sh` útján launchd
  újraindítással jár: külön üzemeltetési döntés, nem lett elvégezve.
- [x] A 45 pending review triage-a megtörtént (S92, 45 → 0).
- [x] A női versenyű meccsek férfi hozzárendelése megtörtént (S93, 12 → 0).
- [x] A 4 hiányzó eredmény pótlása megtörtént (S94, withheld 4 → 0).
- A S90 nyitott tételei: research szinkron launchd-ütemezése; validációs
  retenció (1,47 GB, ~130 MB/nap) — mindkettő külön üzemeltetési döntés.
- A mai nap lejárt, eredmény nélküli meccsei (pl. mexikói Liga Premier
  `date_only` placeholderek, dél-koreai J/K-league élő meccsek) — a research
  13:00-s és 19:00-s refresh jobjai ütemezve vannak, nem kézi beavatkozás.
