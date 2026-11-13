# 2026-11-13 — Napi checkpoint

**Állapot:** S2b-1 design kész, spec review-kapu előtt
**Előző nap:** 2026-11-12
**Kapcsolódó dokumentumok:** [S2b-1 spec](../../superpowers/specs/2026-11-13-s2b1-identity-layer-design.md),
[S1 döntések](../../decisions/2026-11-12-s1-review-dontesek.md),
[tegnapi checkpoint](../2026-11-12/checkpoint.md)
**Branch:** `main` (szinkronban originennel, 07b1936); az S2b-1 implementáció
a `feat/s2b1-identity-layer` ágon indul majd (létrehozás a terv jóváhagyása után).

## Napi cél

S2b-1: fail-closed identitás-réteg (linker, crosswalk/alias/review-sor táblák,
CLI review, tick-integráció) — brainstorming → spec → terv → implementáció.
Az S2b-2 (bizalmi szintes gate) külön ciklusban követi.

## Kiinduló állapot

- [x] Az S2a (ingest keretrendszer) tegnap lezárult és merge-elve a main-be
      (44fc15c + 07b1936); 86 teszt zöld, demo 20/20 verified.
- [x] A tegnapi checkpoint S3-belépési kapui rögzítve (11 tétel).
- [x] Kötelező olvasási sorrend felvéve: AGENTS.md, README, S1/S2a spec,
      tegnapi checkpoint, git állapot (main, tiszta).
- [x] A felhasználó a mai szeletnek az S2b-t választotta; felbontás:
      S2b-1 identitás → S2b-2 gate (jóváhagyva).

## Elvégzett szelet: S2b-1 brainstorming + spec

- [x] Kontextus: verification gate/rules (6 szabály, evaluate_publish_state),
      identity.source_entity_ref séma, katalógus identitás-híd sora,
      S2a live ág identity_exact hardcode.
- [x] Tisztázó kérdések (egyesével, jóváhagyva): két alszelet; tiszta keret
      (szintetikus fixture, importok S3); review csak CLI.
- [x] Megközelítés: A — új identity crate + 0005 (crosswalk, alias,
      review_queue), domain-pure szabályok, fail-closed linker (B: meglévő
      tábla bővítés elvetve; C: fuzzy automatizmus tiltott).
- [x] Design 5 szekció egyenként jóváhagyva (architektúra, adatmodell +
      feloldási sorrend, adatfolyam/review-életciklus/CLI, hibakezelés +
      6 fail-closed garancia, tesztelés + DoD).
- [x] Spec: `docs/superpowers/specs/2026-11-13-s2b1-identity-layer-design.md`
      — önreview: 3 javítás (ManualReview method-leképezés, fuzzy-küszöb
      explicit 0.6, elírás); glyph-scan tiszta (diagram-karakterek
      szándékosak), magyar visszaolvasás kész, záró újsor megvan.
- [ ] Felhasználói spec-review: FÜGGŐBEN.
- [ ] writing-plans: NEM indult (a review-kapu után).
- [ ] Implementáció: NEM indult.

## Ellenőrizetlen / nem történt

- Kód NEM változott; DB NEM érintett; hálózati kérés NEM történt.
- Commit NEM készült (a spec a munkafán, felhasználói engedélyhez kötött).

## Következő döntési pont

1. A felhasználó review-zza a specet; jóváhagyás után writing-plans
   (terv: `docs/superpowers/plans/2026-11-13-s2b1-identity-layer.md`).
2. A terv jóváhagyása után implementáció a `feat/s2b1-identity-layer` ágon,
   taskonkénti STOP-pontokkal (SDD).

## Visszaállítási út

- Új fájl: a spec (munkafa) — elvetés `git restore`/törlés csak kifejezett
  engedéllyel. DB és kód érintetlen.

## S2b-1 — megvalósítási terv elkészült (writing-plans)

- [x] Terv: `docs/superpowers/plans/2026-11-13-s2b1-identity-layer.md`
      — 7 task (domain szabályok → 0005 migráció → storage repo-k →
      identity crate + linker → review + CLI → tick E2E → zárás),
      TDD-lépések, valódi kód, commit nélkül.
- [x] Önreview: spec-lefedettség teljes, placeholder nincs, típus-konzisztens;
      javítások: context() helper-ek a Task 3 interfészben, Task 3 teszt
      valódi kódja, linker unwrap-mentes, weak-rung egységesítve (kihagyott).
- [x] Spec-jegyzet: a resolve szignatúra source_slug-gal bővül (backward
      kompatibilis, a crosswalk szöveges kulcsa miatt) — a spec §2.2-ben rögzítve.
- [x] Magyar visszaolvasás + glyph-scan: terv és spec tiszta, záró újsor megvan.
- [ ] Végrehajtás: NEM indult — a felhasználó a végrehajtási módról és a
      `feat/s2b1-identity-layer` branch létrehozásáról még nem döntött.

## S2b-1 — záró checkpoint

**Állapot:** mind a 7 task kész, a workspace gate zöld (121 teszt), a dev DB
a 0005 migrációval 32 táblán áll. Commit NEM készült — a felhasználó
kifejezett kérésére vár.

### Elvégzett taskok (bizonyítékkal)

- [x] Task 1 — `domain::identity` tiszta szabályok (normalizálás, kontextus,
      token-átfedés): `cargo test -p football-domain -- --test-threads=1`
      zöld (6+4+2 teszt).
- [x] Task 2 — `0005_identity_layer.sql` (crosswalk, alias, review_queue +
      uq_review_pending NULLS NOT DISTINCT): tesztDB 32 tábla,
      `identity_schema` tesztek zöldek.
- [x] Task 3 — `storage::repositories::identity` repo-k: `identity_repos`
      15/15 zöld, storage suite 0 FAILED.
- [x] Task 4 — `football-identity` crate + fail-closed linker:
      football-identity 8/8 zöld (6 feloldási út + pending-dedup + magyar
      hibaüzenetek).
- [x] Task 5 — review-életciklus (approve/reject tranzakcióban, audit) +
      `footballctl identity` CLI: review tesztek zöldek, InvalidInput
      fix-kör ratifikálva (üres jegyzet / NULL forrás magyar üzenettel).
- [x] Task 6 — tick live ág linkerrel + E2E: workspace gate 120 zöld;
      exact fixture → verified, bizonytalan identitás → withheld → approve
      → Exact (resolve-szinten, controller-ruling), shadow érintetlen.
- [x] Task 7 — zárás: dev 0005 migráció (engedélyezett kézi kör),
      db-check flip 29→32, operations doc, hibakezelési pontosítások,
      záró ellenőrzések (lent).

### Task 7 kiegészítő javítások (controller-döntések)

- [x] `identity/src/error.rs` Db-ág doc pontosítva: a hívó (tick) naplóz és
      false flaggel folytatja (fail-closed) — a korábbi „nem folytatja a
      feldolgozást” fogalmazás pontatlan volt.
- [x] Új worker E2E teszt `db_error_is_fail_closed`: lezárt pool →
      `resolve_team_identity` false, pánik nélkül (a linker-szintű Db-teszt
      mellett a tick-ági hívó viselkedése is bizonyított).
- [x] `identity/Cargo.toml` takarítás: chrono + tracing + tempfile eltávolítva
      (grep-pel igazoltan használatlanok), a tokio a dev-dependencies
      szekcióba került; clippy `-D warnings` zöld.

### Záró ellenőrzések (futtatott parancs + pontos eredmény)

1. `./scripts/check.sh` (reset + fmt + clippy + workspace tesztek
   single-threaded): EXIT=0, fmt/clippy figyelmeztetés nélkül,
   **121 passed / 0 failed** (45 suite; a Task 6-os 120 + 1 új Db-teszt).
2. `./scripts/db-reset.sh && ./scripts/db-check.sh`: EXIT=0,
   `OK: football_v2_dev|football_v2, táblák: 32, szabályok: 6, üzleti sorok: 986, worker sorok: 6`
   és `OK: football_v2_test|football_v2, táblák: 32, szabályok: 6, üzleti sorok: 0, worker sorok: 0`.
3. `cd rust && cargo run -q -p footballctl -- migrate` (dev, külön
   felhasználói engedéllyel): `migráció kész`; `_sqlx_migrations`
   `5|identity layer|t`; az új identity táblák üresek (0/0/0), a meglévő
   84 fixture_seed ref és a 986 üzleti demósor érintetlen.
4. `cargo run -q -p footballctl -- day --date 2026-11-11`: EXIT=0,
   **20 meccs, mind `verified_publish`, blockers: []** (a magyar nevek —
   Ferencváros, Újpest, Győr, Debrecen — épségben).
5. `cargo run -q -p footballctl -- identity review`: EXIT=0,
   `id | forrás | external ID | név | ok | jelöltek száma | létrehozva` +
   `(nincs felülvizsgálati sor)`; `identity list-aliases`: `(nincs alias)`.

### Elkészült dokumentum

- [x] `docs/operations/identity.md` (magyar): feloldási sorrend (6 lépés),
      review menete CLI-példakimenetekkel, append-only audit, visszavonás
      csak új döntéssel, fail-closed garanciák, fixture re-seed korlát,
      visszaállítási út (0005 drop sorrend + tick revert).

### Ami ellenőrizetlen maradt

- Valódi crosswalk-import (reep strong rung, ESPN/v1 aliasok) — S3 szelet.
- Tick-szintű withheld gate E2E nem-fixture forrással — S3 (a fixture
  pipeline canonicalize lépése mindig újraéleszti a fixture_seed ref-eket;
  a ciklus resolve-szinten bizonyított).
- Fuzzy `all_team_entities` skálakorlát: teljes entitás-listán fut, S2b-1
  szintetikus skálán mérve; S3-ban előszűrő kell.
- Valódi párhuzamos approve-verseny (in-tx decided==0 rollback ág) és az
  approve entity-kind validáció: a ledger Task 5 deferred minorjai, a
  Task 7 controller-döntés hatókörén kívül maradtak.

### Deferred minorok

A teljes lista az SDD ledgerben:
`.superpowers/sdd/2026-11-13-s2b1-identity-layer/progress.md`
(Task 1: 3, Task 2: 2, Task 3: 3, Task 4: 3, Task 5: 2, Task 6: 4 tétel;
a Task 7-re jelölt hármas javítás fent elvégezve).

### Visszaállítási út

- DB: a 0005 additív; drop sorrend `review_queue → alias → crosswalk`
  (dev DB-n csak kifejezett engedéllyel); tesztDB: `./scripts/db-reset.sh`.
- Kód: `feat/s2b1-identity-layer` branch, minden változás a munkafán;
  a tick linker-cseréje git reverttel az S2a hardcode-ra állítható.
- Dokumentumok: új `docs/operations/identity.md`, módosult
  `scripts/db-check.sh` + ez a checkpoint (append-only).

### Következő döntési pont

1. Commit a felhasználó kifejezett kérésére (munkafa-review után).
2. S2b-2 (bizalmi szintes gate) külön brainstorming ciklusban.

## Final review ratifikációk + S3 kapuk

**Ratifikált eltérések (a controller döntése, viselkedés változatlan):**

- §3.3 rendezés: S2b-1 FIFO (`created_at DESC`); a prioritás-rendezés
  (kickoff-közelség + verseny) S3/S8-ra halasztva, queue→meccs kontextus-kapcsolattal.
- §2.2/§4.1 DB-hiba: Db-hibánál a queue-írás best-effort (teljes DB-kiesésnél
  lehetetlen); a fail-closed withheld érvényesül, a következő tick gyógyítja a queue-t.

**Final fix-hullám (3 kódjavítás, a munkafán, commit nélkül):**

- [x] SQL a binből a storage-ba (AGENTS §5): `footballctl` review-listájának
      forrás-lekérdezése új `identity::list_sources` repository-függvénybe
      került, storage-teszttel.
- [x] Linker hibalánc: a `db` helper `format!("{e:#}")`-ra váltva
      (a review.rs-sel azonos formátum, a sqlx gyökérok megmarad).
- [x] Audit detail spec-sync (spec §3.2): approve és reject detail JSON-ja
      kiegészült az `ok` mezővel (a queue-sor reason-je); tesztek frissítve.

**Új S3-belépési kapuk (a tegnapi 11 tétel mellé):**

- [ ] Cross-kind approve-validáció: az approve ellenőrizze a választott
      entitás kind-ját a queue-sor `entity_kind`-jével szemben.
- [ ] Crosswalk-uniqueness validáció importkor: az importáló ellenőrizze a
      crosswalk-kötések egyértelműségét (nem ütköző/ellentmondó kötések).

## S2b-2 — bizalmi szintes gate: brainstorming + spec (2026-11-13)

- [x] Kontextus: verification engine/gate (6 szabály, SINGLE_SOURCE warning),
      fact.odds_snapshot PK=match_id (egy sor/meccs), jóváhagyott bizalmi mátrix.
- [x] Tisztázó kérdések (egyesével, jóváhagyva): új ops.source_trust tábla
      (provider-csoportosítással); teljes odds-szabály MOST (0006 composite PK
      + trust_state); trust default medium; odds tolerancia paraméter default 0.0.
- [x] Megközelítés: A — TRUST_LEVEL a meglévő motorban + külön
      evaluate_odds_trust (mezőszintű, NEM Finding); B (pre-gate) és
      C (field_trust mátrix) elvetve.
- [x] Design 5 szekció egyenként jóváhagyva.
- [x] Spec: `docs/superpowers/specs/2026-11-13-s2b2-trust-gate-design.md`
      — önreview: CJK-injektálás javítva (idegen „same” jelentésű szó → „ugyanazon”), csoport-képviselő
      definíció pontosítva; glyph-scan tiszta, magyar visszaolvasás kész.
- [x] Demo odds-kockázat ellenőrizve: mind a 20 pár egzakt egyező (0 eltérés,
      canonical id join) — a default toleranciával verified odds zöld.
- [ ] Felhasználói spec-review: FÜGGŐBEN.
- [ ] writing-plans + implementáció: NEM indult.

## S2b-2 — megvalósítási terv elkészült (writing-plans)

- [x] Terv: `docs/superpowers/plans/2026-11-13-s2b2-trust-gate.md` — 7 task
      (domain trust típusok → 0006 migráció → storage trust/odds/LATERAL →
      verification TRUST_LEVEL + evaluate_odds_trust → hívók + E2E →
      API + Swift → zárás), TDD, valódi kód, commit nélkül.
- [x] Önreview: spec-lefedettség teljes, placeholder nincs, típus-konzisztens;
      javítások: display_only-only domain nem ad warningot, unused változó,
      load_match_odds unwrap-mentes (ok_or_else).
- [x] Magyar visszaolvasás + glyph-scan: terv tiszta, záró újsor megvan.
- [ ] Végrehajtás: NEM indult — a felhasználó a végrehajtási módról és a
      `feat/s2b2-trust-gate` branch létrehozásáról még nem döntött.

## S2b-2 — záró checkpoint

**Dátum:** 2026-11-13 (Task 7, a szelet zárása)
**Branch:** `feat/s2b2-trust-gate`, minden változás a munkafán, commit NEM
történt (csak kifejezett felhasználói kérésre).

### A 7 task eredménye (bizonyítékkal)

1. **Domain trust típusok** — `TrustClass`/`SourceTrust`/`OddsValues`/
   `SourceOdds`/`OddsTrustState` + `ODDS_TOLERANCE_DEFAULT`; mikro-epszilon
   ruling (1e-9). Bizonyíték: trust_types 3/3, domain suite 15/15 zöld.
2. **0006 migráció** — `ops.source_trust`, composite odds PK
   `(match_id, source_id)`, `trust_state` CHECK/default. Bizonyíték:
   trust_schema séma-tesztek (23514 CHECK, PK exact-equality), tesztDB 33
   tábla, db-check 33/6/0/0.
3. **Storage trust repo + LATERAL day join** — upsert/trust_for_sources/
   load_match_odds/update_odds_trust_state; a day repo LATERAL joinja nem
   duplikál a composite PK-val. Bizonyíték: storage suite zöld (0 FAILED).
4. **TRUST_LEVEL + evaluate_odds_trust** — a SINGLE_SOURCE szemantikáját
   átvevő szabály (provider-csoportosítás, domainenként) + mezőszintű
   odds-kiértékelés. Bizonyíték: verification suite 18/18 zöld.
5. **Hívók + E2E** — tick + footballctl trust/odds betöltés, gate utáni
   trust_state UPDATE; `RULE_SET_VERSION = verification-1.1`. Bizonyíték:
   worker suite 38/38 zöld (0 failed), demo E2E 20/20 verified + verified odds.
6. **API + Swift** — `/v1/day` top-level `odds_trust_state` (additív),
   Swift `DayMatch.oddsTrustState` decode. Bizonyíték: api suite 3/3 zöld,
   Swift Core decode tesztek zöldek.
7. **Zárás (ez a task)** — `docs/operations/trust.md` (upsert, demo +
   katalógus seed, provider/odds szabály, 0006 rollback); dev migráció
   felhasználói engedéllyel; db-check dev 32→33 + `ops.source_trust` az
   üzleti összegben; routes.rs null-assert `contains_key` erősítés;
   záró ellenőrzések lent.

### Dev DB változások (felhasználói engedéllyel)

- `cargo run -q -p footballctl -- migrate` → `migráció kész`; dev 33 tábla,
  a 20 demo odds sor megmaradt (trust_state default provisional), composite
  PK éles, `_sqlx_migrations` 1–6.
- Takarítás: a stale `SINGLE_SOURCE` validation_rule sor `TRUST_LEVEL`-re
  átnevezve (UPDATE 1, description „Bizalmi szint” — a tesztDB-vel azonos).
- Trust seed: 15 sor (5 demo forrás × 3 domain: fixture/kickoff, result,
  odds; medium, NULL provider) az operations doc 2. szekciójának SQL-jével.
- Demo újratöltés (a dokumentált, visszajátszható `ingest-fixture` út, 26
  fájl): a composite PK miatt mindkét forrás odds sora megmarad (2×20),
  a gate + odds trust újraszámolva. Eredmény: 40 odds sor mind verified,
  21 publish_gate verified_publish. Ez a lépés a runtime bizonyíték
  előfeltétele volt (a régi PK csak egyforrású odds sorokat hagyott).

### Záró ellenőrzések (futtatott parancsok + pontos kimenetek)

1. `./scripts/check.sh` → reset + fmt + clippy + workspace tesztek zöld.
   Összesen **152 passed / 0 failed** (50 test bináris, mind ok). Megjegyzés:
   az első futás a routes.rs új assertjének formázásán bukott (fmt),
   javítás után a második futás zöld.
2. `./scripts/db-reset.sh && ./scripts/db-check.sh` →
   `OK: football_v2_dev|football_v2, táblák: 33, szabályok: 6, üzleti sorok: 1061, worker sorok: 6`
   `OK: football_v2_test|football_v2, táblák: 33, szabályok: 6, üzleti sorok: 0, worker sorok: 0`
   Próba: 1 source + 1 source_trust sor a tesztDB-n → a zero-row gate
   HIBA-t ad (üzleti: 2), db-reset után ismét 0/0.
3. `cd rust && cargo run -q -p footballctl -- day --date 2026-11-11` →
   20 meccs, mind `publish_state: verified_publish` és
   `odds_trust_state: verified` (20/20, Counter-rel mérve).
4. `cd apps/FootballAppCore && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test`
   → `Test run with 18 tests in 4 suites passed` (0 hiba).

### Ami ellenőrizetlen maradt

- `footballctl IngestFixture` identity_exact=true hardcode (pre-existing,
  a tick linkerével szemben) — szándékosan NEM változott; S3 consistency
  tétel (a linker élesben a tickben fut).
- Valódi crosswalk/odds import: nincs (S3 fázis, 13 belépési kapu).
- SwiftUI app-szintű teszt ebben a taskban nem futott (a brief Core-t
  ír elő); a 2026-11-09-i fixture meccs gate-e a régi szabálykészlettel
  marad (nem része a runtime bizonyítéknak).

### Deferred minorok

A szelet deferred minorjait az SDD ledger tartja nyilván:
`.superpowers/sdd/2026-11-13-s2b2-trust-gate/progress.md` (Task 1
epszilon/from_db_str, Task 3 created_at/diagnosztika/day-teszt, Task 4
odds teszthézag, Task 6 seed-duplikáció). Task 7-ben pótolva: db-check
`ops.source_trust` üzleti összeg, dev SINGLE_SOURCE takarítás, routes.rs
`contains_key` erősítés.

### Visszaállítási út

- Kód: a változások commitolatlanok a `feat/s2b2-trust-gate` ágon;
  elvetés `git restore` csak kifejezett engedéllyel.
- Dev DB: 0006 rollback az operations doc 7. szekciója szerint (odds
  dedup + PK visszaállítás match_id-re, trust_state drop, source_trust
  drop, `_sqlx_migrations` 6 törlése); a demo adat visszajátszható
  `ingest-fixture`-rel. TesztDB: `./scripts/db-reset.sh` bármikor 33/6/0/0.
- Gate: TRUST_LEVEL → SINGLE_SOURCE git revert + a validation_rule sor
  visszanevezése (operations doc 7. szekció).

### Következő döntési pont

1. Commit a felhasználó kifejezett kérésére (munkafa-review után).
2. S3 belépési kapuk + a deferred minorok ütemezése.
