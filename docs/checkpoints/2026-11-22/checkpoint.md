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

## Következő döntési pont

- A futó daemon binárisa 2026-11-21 18:33-as, az utolsó Rust feat-commit
  18:41-es (S90 forrás-ref ország-guard) — a frissítés
  `cargo build … && ./scripts/install-footballd-agent.sh` útján launchd
  újraindítással jár: külön üzemeltetési döntés, nem lett elvégezve.
- [x] A 45 pending review triage-a megtörtént (S92, 45 → 0).
- A S90 nyitott tételei: 10 női versenyű meccs férfi csapattal; 4 hiányzó
  eredmény forrás-oldali pótlása (11-21); research szinkron launchd-ütemezése;
  validációs retenció (1,47 GB, ~130 MB/nap).
- A mai nap lejárt, eredmény nélküli meccsei (pl. mexikói Liga Premier
  `date_only` placeholderek, dél-koreai J/K-league élő meccsek) — a research
  13:00-s és 19:00-s refresh jobjai ütemezve vannak, nem kézi beavatkozás.
