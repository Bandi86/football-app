# 2026-11-12 — Napi checkpoint

**Állapot:** review-kapu lezárva (16/16 döntés), rögzítés kész; S2 brainstorming indul
**Előző nap:** 2026-11-11
**Kapcsolódó dokumentumok:** [S1 döntések](../../decisions/2026-11-12-s1-review-dontesek.md),
[forráskatalógus](../../research/source-catalog.md),
[S1 spec](../../superpowers/specs/2026-11-11-real-data-sources-research-design.md)
**Branch:** `feat/source-research` (a review előtt tiszta, commitolva: 10e4a11)

## Napi cél

Az S1 záró review-kapu lezárása: a katalógus 16 döntési pontjának
egyesével történő megválaszolása a felhasználóval, a döntések rögzítése
`docs/decisions/`-be, a katalógus státuszainak átvezetése, majd az
S2 spec ciklus indítása (a 16. pont jóváhagyása szerint).

## Kiinduló állapot

- [x] S1 (W1–W4, Task 1–15) lezárva, commitolva + pusholva (10e4a11);
      a munkafa tiszta volt (`git status --short --branch`).
- [x] A review-kapu nyitott: bizalmi mátrix + adoptációs sorrend
      jóváhagyásra, 16 döntési pont megválaszolatlan.
- [x] Kötelező olvasási sorrend felvéve: AGENTS.md, README, S1 spec/terv,
      2026-11-11 checkpoint, git állapot.

## Elvégzett szelet: 16 döntési pont + rögzítés

- [x] Mind a 16 pont egyesével végigjárva a felhasználóval (helyzet +
      opciók + javaslat minden pontnál; a felhasználó döntött).
      Eredmények röviden:
      1. TheSportsDB elvetett; 2. Livescore MCP elvetett;
      3. logók: a v1 réteges megoldás auditált átvétele (MIT zászlók,
      v1 asset-bázis lokális tárolóba + audit, FotMob liga-logók,
      új forrás csak mért résekre); 4. Wikidata nem kell most;
      5. OpenFoot read-only folytatás (worker-ledger az S2-ben);
      6. ClubElo helyett saját Elo-számítás; 7. FM27 adat kihagyás;
      8. Guardian ToS-review kapu; 9. API-Football elvetett;
      10. reep stamp-guard követelmény; 11. worldfootball kupa-szerep
      átadva a Sky/ESPN/thefa hármasnak; 12. Soccerway tartalék;
      13. FA Kupa: Sky + ESPN + thefa, 90 perces skip-őr;
      14. football-data.org / The Odds API regisztráció nem kell most;
      15. ESPN ToS-review kapu; 16. mátrix + sorrend jóváhagyva, S2 indul.
- [x] A 3. ponthoz a v1 logó-megoldás read-only átnézése
      (`../football-app`: logo-audit, migrációs riport, manifestek,
      flag-icons MIT LICENSE) — a döntés ezt támasztja alá.
- [x] Döntési dokumentum: `docs/decisions/2026-11-12-s1-review-dontesek.md`
      (16 pont, indoklások, újrabírálási feltételek).
- [x] Katalógus-frissítés: 5 bejegyzés státusz-sora frissítve
      (thesportsdb, logos-github, livescore-mcp, clubelo, api-football →
      `elvetett`, döntés-hivatkozással) + új „Felhasználói döntések
      (2026-11-12, S1 záró review)” fejezet a 16 pont átvezetésével.
      A „Döntési pontok” fejezet történeti előkészítés maradt (nem szerkesztett).

## Futtatott ellenőrzések

- [x] Magyar visszaolvasás (Read tool, szó szintű): döntési dokumentum
      teljes egészében + a katalógus új fejezete — ékezetek épek,
      hiányzó szóköz / idegen szó nem fordul elő.
- [x] Idegen-glyph scan (python3, nem-ASCII karakterek): döntési
      dokumentum 0 találat; katalógus 6 találata mind korábbi, szándékos
      jelölés (Køge, CÍMKE−1, ©, ≈, 90’, ≠) — az új szakaszban 0.
- [x] Záró újsor: döntési dokumentum (pótolva) és katalógus OK.
- [x] `git status --short --branch` a rögzítés után: új fájl
      `docs/decisions/2026-11-12-s1-review-dontesek.md`, módosult
      `docs/research/source-catalog.md` + ez a checkpoint.

## Ellenőrizetlen / nem történt

- Új hálózati kérés NEM történt (a review a meglévő spike-bizonyítékokra épült).
- DB-írás NEM történt; kód NEM módosult; tesztek nem futottak (csak dokumentum).
- Commit NEM készült (felhasználói engedélyhez kötött).
- A ToS-reviewk (Guardian, ESPN) és a logó/asset átvétel NEM indult el —
      külön szeletek kapui.

## Következő döntési pont

1. **S2 brainstorming → spec → terv** (a 16. pont szerint jóváhagyott
   indítás): ingest keretrendszer — Obscura-integráció, worker
   kvóta/breaker/ledger, bizalmi szintes gate, fail-closed identitás;
   bemenet: jóváhagyott mátrix + adoptációs sorrend + 18 tételes
   séma-rés lista (a 18. tétel saját Elo-feature-re módosul).
2. Commit engedélyezése a mai dokumentum-változásokra (külön kérésre).

## Visszaállítási út

- Új fájl: `docs/decisions/2026-11-12-s1-review-dontesek.md` — törlése
  csak kifejezett engedéllyel.
- Módosult: `docs/research/source-catalog.md` (5 státusz-sor + új fejezet) —
  a review előtti állapot a git munkafa-diffből visszaállítható
  (`git restore` csak kifejezett engedéllyel).
- DB és /tmp/s1 érintetlen.

## S2a — spec + megvalósítási terv (2026-11-12, brainstorming → writing-plans)

- [x] S2 dekompozíció jóváhagyva: S2a (keretrendszer), S2b (bizalmi gate +
      identitás), S2c (Obscura), S2d (séma-rések) — mindegyik saját ciklus.
- [x] S2a brainstorming: 3 tisztázó kérdés (tiszta keretrendszer; worker a
      footballd-ben; teljes §7.3 eseménymodell) + 3 megközelítés (A: trait +
      worker crate — jóváhagyva) + 5 design szekció egyenként jóváhagyva.
- [x] Spec: `docs/superpowers/specs/2026-11-12-s2a-ingest-framework-design.md`
      — felhasználói review: „Spec jó". Önreview: 2 javítás (ops.job
      time_window, async-trait függőség).
- [x] Terv: `docs/superpowers/plans/2026-11-12-s2a-ingest-framework.md`
      — 12 task, TDD-lépések, valódi kód, commit nélkül (AGENTS §9).
      Önreview-javítások: DemoAdapter TZ-kezelés, `runner` modulnév
      (a `loop` foglalt szó), reserve-elszámolás egyértelműsítése,
      `url` függőség, fixture-útvonalak a CARGO_MANIFEST_DIR konvencióra.
- [x] Magyar visszaolvasás + glyph-scan: spec és terv tiszta (a terv
      szkennjén 0 idegen glyph; a diagram-karakterek szándékosak),
      záró újsor mindkettőn megvan.
- [ ] Végrehajtás: NEM indult — a felhasználó a végrehajtási módról
      (subagent-driven vs inline) és a `feat/s2a-ingest-framework`
      branch létrehozásáról még nem döntött.

Következő döntési pont: végrehajtási mód + branch-engedély; utána Task 1
(domain típusok) indul, taskonkénti STOP-pontokkal. Commit nem készült.

## S2a — záró checkpoint (Task 12, 2026-11-12)

**Branch:** `feat/s2a-ingest-framework` (bázis: bf96ff4). Végrehajtás:
subagent-driven, taskonkénti STOP + controller review; Task 11–12 a
felhasználó delegálása szerint STOP nélkül, engedélyezett kézi dev körrel.
Commit NEM készült — a teljes S2a munka a munkafán él (review-csomag:
munkafa-diff + untracked fájlok).

### A 12 task eredménye (egy sor / task, tesztbizonyítékkal)

- [x] **Task 1 — domain típusok:** `SourcePolicy`/`FetchJob`/ledger/breaker/
      coverage enumok + `DbValueError` tipizált hiba; 31 variáns db_string
      roundtrip (domain suite 8 zöld, ebből policy_types 2).
- [x] **Task 2 — 0004 migráció:** 6 új ops tábla (source_policy, fetch_ledger,
      breaker_state, job, coverage_slot, robots_cache), tranzakciós;
      worker_schema 1/1, connection 29 táblás assert; tesztDB 29/6/0.
- [x] **Task 3 — storage repository-k:** worker repo-függvények (policy,
      ledger, breaker, job, coverage, robots); worker_repos suite zöld
      (jelenleg 16 teszt, a Task 10 ordering-tesztjével bővülve).
- [x] **Task 4 — Fetcher réteg:** `Fetcher` trait + `FixtureFetcher`
      (hiányzó könyvtár → üres map, hiányzó kulcs → `NotFound`); fetcher 3/3.
- [x] **Task 5 — RustHttpFetcher:** streaming méretplafon letöltés közben,
      content-length előellenőrzés, timeout/transport tipizált hibák;
      http_fetcher 5/5 (wiremock).
- [x] **Task 6 — SourceAdapter + DemoAdapter:** decoy/dátum-guard/séma-drift
      őrök, AET/pens fail-closed karantén; demo_adapter 5/5.
- [x] **Task 7 — robots-kapu:** fail-closed (Transport → blocked, fixture
      NotFound → no_robots), számjegymentes UA-token (`football-app`);
      robots_gate 7/7 (negatív tesztekkel).
- [x] **Task 8 — kvóta + breaker:** napi kvóta (UTC, számláló outcome-ok),
      min-delay guard, breaker állapotgép + auth-azonnali open;
      quota_breaker 5/5 + worker unit 4/4.
- [x] **Task 9 — scheduler:** idempotens job-generálás (interval/offset),
      prioritás (esemény + verseny + közelség); SQL a storage-ba került
      (insert_job_counted, kickoff_window); scheduler 5/5.
- [x] **Task 10 — tick + E2E:** teljes kapu-sorrend, shadow/canary ágak,
      ledger-elszámolás; DelayBlocked-livelock zárva (szűrt last_attempt_at),
      forrás-identitás a job.slug autoritatív (impersonáció-teszt),
      claim_due_jobs Rust-oldali rendezés (flake-gyökér zárva); tick_e2e 6/6.
- [x] **Task 11 — footballd + footballctl:** worker loop graceful shutdown +
      lease-release, WORKER_TICK_SECS clamp+warn, `worker status/tick`,
      doctor új mezők; worker_loop 2/2, footballd unit 4/4. Kézi dev kör
      (engedélyezett): dev 0004 migráció 29 tábla, shadow tick
      `generált=1 feldolgozott=1`, core.match=21 VÁLTOZATLAN, leased=0.
- [x] **Task 12 — zárás:** `docs/operations/worker.md` elkészült; takarítás
      (db-check.sh dev 23→29 + worker-lekérdezés a táblaszám-ellenőrzés után +
      üzleti összeg worker nélkül, külön „worker sorok” mező + záró újsor;
      ingest/Cargo.toml záró újsor; quota.rs stale komment frissítve);
      záró ellenőrzések lefutnak (lásd alább).

### Futtatott záró ellenőrzések (pontos kimenetek)

- [x] `./scripts/check.sh` (db-reset tesztDB + fmt + clippy + workspace
      tesztek, `--test-threads=1`): **minden zöld**. 37 `test result: ok`
      sor, összesen **86 passed; 0 failed** (api 2, domain 8, ingest 17,
      storage 21, verification 5, worker 29, footballd 4; a többi suite 0).
      A fmt/clippy lépések hibátlanul lefutottak (a script set -e mellett
      jutott a tesztekig).
- [x] `./scripts/db-reset.sh && ./scripts/db-check.sh`:
      ```text
      OK: football_v2_dev|football_v2, táblák: 29, szabályok: 6, üzleti sorok: 986, worker sorok: 6
      OK: football_v2_test|football_v2, táblák: 29, szabályok: 6, üzleti sorok: 0, worker sorok: 0
      ```
      A dev **986** üzleti sor = 979 (Task 2 bázis) + 7: a Task 11 kézi
      shadow tickje 1 `ops.source` (demo-synthetic) + 1 `raw.artifact` +
      1 `raw.capture` + 4 `stage.observation` sort írt (read-only
      lekérdezésekkel igazolva, created_at 2026-11-12 14:29). A 6 worker
      sor (policy/ledger/breaker/job/coverage/robots 1-1) NEM része az
      üzleti összegnek — a db-check.sh mostantól külön mezőben jelzi, a
      tesztDB zéró-ellenőrzése mindkettőt fedezi. `core.match=21` változatlan.
- [x] Demó pipeline változatlan: `rust/target/debug/footballctl day
      --date 2026-11-11` (dev DB, read-only) → **20 meccs, mind a 20
      `verified_publish`** (a JSON kimenet publish_state mezője megszámolva).
- [x] `cd rust && cargo fmt --all --check` → tiszta;
      `cargo clippy --workspace --all-targets -- -D warnings` → tiszta,
      0 figyelmeztetés (a check.sh mellett külön is megerősítve).
- [x] Magyar visszaolvasás + glyph-scan: `docs/operations/worker.md` és ez
      a záró szekció visszaolvasva (ékezetek épek), python3 nem-ASCII scan
      a két dokumentumon + a 3 takarított fájlon: csak szándékos magyar
      ékezetek és a meglévő stílusú `→`/`—` jelölések, idegen (CJK) glyph
      nincs; minden érintett fájl záró újsorral végződik.

### Ellenőrizetlen / nem történt

- **Élő forrásadapter nincs** — a négy katalógus-forrás seed SQL-je
  (enabled=false, shadow) az útmutatóban van, élesítés csak az S3
  minősítési folyamatában; hálózati fetch élő forrásról NEM futott.
- **bind/serve hibaági lease-release** (footballd): kódút-indoklással és
  fordítással igazolt, automata teszt nélkül (a shutdown→release ágat a
  worker_loop integrációs teszt fedi) — ellenőrizetlen ág, külön kimondva.
- Canary élesítési folyamat, explicit ET/pens mezőkezelés: S3.
- A halasztott minor tételek listája a SDD ledgerben él:
  `.superpowers/sdd/2026-11-12-s2a-ingest-framework/progress.md`
  (taskonkénti „minor (deferred)” sorok; a Task 12-es takarítás a ledger
  5 érintett tételét zárta: db-check dev-flip, worker_rows sorrend,
  db-check újsor, ingest/Cargo.toml újsor, quota.rs komment).
- Dev DB írása a Task 12-ben NEM történt (csak read-only lekérdezések);
  a tesztDB resetje a check.sh/db-reset.sh része volt.

### Visszaállítási út

- Worker azonnali leállítás: `WORKER_ENABLED=false` + újraindítás;
  graceful leállításkor a lease-ek elengedődnek (részletek:
  `docs/operations/worker.md` 8. fejezet).
- 0004 drop-sorrend (FK-sorrendben, CASCADE nélkül, csak kifejezett
  engedéllyel): `ops.robots_cache`, `ops.coverage_slot`, `ops.job`,
  `ops.breaker_state`, `ops.fetch_ledger`, `ops.source_policy`.
- Kód: a munkafa-diff a review-csomag; a taskonkénti prefix-fájlok a
  `.superpowers/sdd/` alatt őrzik a bázisállapotokat. Új fájlok törlése /
  `git restore` csak kifejezett engedéllyel.

### Következő döntési pont

- Commit: a felhasználó a záró git rendezést kérte (S2a munka commitja
  engedélyezve a delegálásban) — **függő controller-lépés**, ebben a
  taskban commit NEM készült. Push NEM része — külön kérdés marad.
- Következő szelet: S2b (bizalmi gate + identitás) indítása a felhasználó
  engedélyével; az S3 forrás-élesítés a minősítési kapu után.

## S2a — final whole-branch review + S3 belépési kapuk (2026-11-12)

- [x] Final reviewer (teljes branch diff, 8848 sor): **merge engedélyezhető**,
      Critical 0, must-fix-before-merge 0; DoD minden tétele teljesült
      (86 teszt zöld, 0 élő kérés, 0 nyers payload, demó 20/20 verified,
      operations doc, 0004 test→dev + rollback).
- [x] Controller záró kapu: `./scripts/check.sh` → 86 teszt, 0 failed;
      `./scripts/db-reset.sh && ./scripts/db-check.sh` (tesztDB reset
      felhasználói engedéllyel) → dev 29/6/986+6 worker, test 29/6/0/0;
      `footballctl day --date 2026-11-11` → 20/20 verified_publish.
- [x] Ruling: a final review 2 Important tétele + a kapus Minorok
      PARKOLVA S3-belépési kapuként (nem merge-blokkolók, fail-closed
      irányúak, S2a-ban latensek a 0 élő kérés miatt).

### S3 belépési kapuk (éles forrás előtt KÖTELEZŐ zárni)

1. **429 → Retry-After** tisztelete (a tick minden HttpStatus-t generikus
   HttpError-ként kezel; spec §5.2 ígéret).
2. **jitter_ms** valódi használata + **exponenciális backoff** (failed job
   nem requeue-zódik; a breaker-cooldown az egyetlen fék).
3. **Half-open breaker beragadás**: sikeres fetch + parse-karantén/infra-hiba
   után a breaker nem változik, az allow HalfOpen-ban örökre false →
   állapotátmenet kell (vagy a documented kézi reset ops-eljárás).
4. Robots: direktíva-kapu lazítása (Sitemap-only/BOM legitim esetek),
   port/séma cache-kulcs, blocked-TTL felülvizsgálat átmeneti hibánál.
5. Scheduler: interval-validáció (1440/10080) + ismeretlen event_type hiba.
6. persist_raw media_type a payload content_type-jából (most JSON hardcode).
7. Robots-fetch ledger-sor (spec §3.1 „minden kísérlet”; napi 1/host cache).
8. footballctl worker status/doctor nyers SQL → storage repo-függvények.
9. Live ág FixtureSource-huzalozás + identity_exact=true → S2b fail-closed
   linker (a shadow ág érintetlen).
10. OpenFoot havi számláló (5000/hó) a worker-ledgerben (most 166/nap közelítés).
11. Többworker-bevezetéskor: complete_job fencing-guard + half_open
    get+set atomitás (feltételes UPDATE).

A deferred minorok teljes listája az SDD ledgerben
(`.superpowers/sdd/2026-11-12-s2a-ingest-framework/progress.md`);
a final review triázs szerint egyik sem merge-blokkoló.
