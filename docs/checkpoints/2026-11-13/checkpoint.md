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
