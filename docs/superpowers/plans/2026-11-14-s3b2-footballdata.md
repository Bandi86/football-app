# S3b-2 — Footballdata CSV-adapter: implementációs terv

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (subagent nélkül inline). Steps use checkbox (`- [ ]`) syntax.

**Goal:** Az S3b spec 3–7. szekciója: élő módú materializálás (natural-key upsert + linkeres csapatfeloldás), B365→Avg→Max odds-lánc, a footballdata CSV-adapter (5 liga), policy/trust seed és e2e — az élő kör külön engedéllyel.

**Architecture:** A `NormalizedDay` id-hintjei `None` élő módban; a materializer natural-key upserttel hozza létre a versenyt/szezont/meccset, a csapatokat a fail-closed linkeren át oldja fel (nem egzakt → a meccs kimarad, a linker review-sort nyit); az odds-lánc a storage olvasókban; a CSV-adapter observationöket ad (a közös séma szerint).

**Spec:** `docs/superpowers/specs/2026-11-14-s3b-footballdata-design.md`

## Global Constraints

- Taskonként STOP + **commit zöld task után**; `Bandi86` noreply identitás; push nélkül.
- Magyar doksi/hibaüzenet, angol azonosítók; visszaolvasás; glyph-scan.
- Demo-regresszió végig zöld (a fixture-út viselkedése változatlan); 0 élő kérés a default futásban.
- Branch: `feat/s3b2-footballdata` (main f58dbad).

## Terv-jegyzetek (döntések, 2026-11-14)

1. **Élő materializálás:** competition = find-or-create `(country_id, name)`; season = `(competition_id, label)`; match = natural-key find (NULL-safe `round_label`) + update, különben insert generált id-vel (egy-workeres, nincs verseny); a hint-es (fixture) út változatlan.
2. **Csapatfeloldás:** `None` hintnél a `linker::resolve` (team kind, kontextus a névből+országból); `Exact` → `core.team` biztosítása (entity_id alapján find-or-create); nem egzakt → a csapat kimarad + **minden érintett meccs kimarad** (fail-closed; a review-sort a linker nyitotta).
3. **Provenance:** a `match_core` observationöket `source_record_key = source_match_id` alapján kötjük a meccshez (a fixture-nél is működik).
4. **Odds-lánc:** kulcsonként `default` → `B365` → `Avg` → `Max` (closing); a `load_match_odds` és a day-LATERAL ugyanazt a láncot használja.
5. **Adapter:** `FootballdataAdapter` — `plan`: `https://www.football-data.co.uk/mmz4281/2627/{league}.csv` (5 liga); `parse`: CSV → observationök (`team_identity`, `match_core` meta-val, `match_odds` quotes-szal, `match_stats`, `match_referee`); ±7 napos ablak; Europe/London → UTC; BTTS nincs.

---

### Task 1: Élő módú materializálás + provenance-fix

**Files:** Modify: `rust/crates/ingest/src/canonicalize.rs`; Test: `rust/crates/ingest/tests/live_materialize.rs` (új).

**Interfaces:** `materialize(&NormalizedDay)` viselkedése hint nélkül (lásd Terv-jegyzet 1–3); `linker::ResolveMethod`.

- [ ] **Step 1: Bukó teszt** — seed: source `live-src`, country HU, 2 csapat entity+team+`source_entity_ref` (manual_review, active), NormalizedDay hint nélkül (competition/season None, teams None, 1 match None id, `round_label=None`) → materialize → rows; újrafuttatás → nincs duplikátum; egy ismeretlen csapatos nap → a meccs kimarad.
- [ ] **Step 2: Futtatás — bukás.**
- [ ] **Step 3: Implementáció** a Terv-jegyzet 1–3 szerint: `upsert_country` után `source_id` korai lekérés; `find_or_create_competition/season`; hint nélküli csapatok linkeres feloldása (a tx előtt, map-pel); hint nélküli meccs natural-key upsert (SELECT ... `round_label IS NOT DISTINCT FROM $n`); `observations_for_match` `source_record_key`-re; a hint-es út változatlan.
- [ ] **Step 4: Zöld + kapu + commit** (`feat: support live-mode canonical materialization`).
- [ ] **Step 5: STOP.**

### Task 2: Odds-lánc (B365→Avg→Max)

**Files:** Modify: `rust/crates/storage/src/repositories/trust.rs`, `day.rs`; Test: `trust_repos.rs`, `day_query.rs`.

- [ ] **Step 1: Bukó tesztek** — quote-ok három bookmakerrel: `default` nélkül B365 nyer; B365 hiányában Avg; majd Max; a day-JSON ugyanezt adja.
- [ ] **Step 2: Futtatás — bukás.**
- [ ] **Step 3: Implementáció** — a `load_match_odds`/day SQL a láncot használja: `COALESCE(max(price) FILTER (... bookmaker='default'), ... 'B365', ... 'Avg', ... 'Max')` kulcsonként.
- [ ] **Step 4: Zöld + kapu + commit** (`feat: add canonical odds bookmaker chain`).

### Task 3: FootballdataAdapter + CSV-fixture-ök

**Files:** Create: `rust/crates/ingest/src/footballdata.rs`, `rust/crates/ingest/tests/footballdata.rs`, `rust/crates/ingest/tests/fixtures/footballdata_e0.csv` (valós formátumú minta, BOM+CRLF); Modify: `ingest/src/lib.rs`; `ingest/Cargo.toml` (`csv = "0.3"`).

- [ ] **Step 1: Bukó tesztek** — BOM+CRLF parse; `Date`+`Time` Europe/London→UTC (nyári/téli); `date_only`; ±7 napos ablak; odds-map (B365/Avg/Max opening+closing, `>2.5`); stats/referee; hiányzó kötelező oszlop → hiba; observation-aspketek a közös séma szerint.
- [ ] **Step 2: Futtatás — bukás.**
- [ ] **Step 3: Implementáció** — liga-térkép (E0 England/Premier League, SP1 Spain/La Liga, I1 Italy/Serie A, D1 Germany/Bundesliga, F1 France/Ligue 1; tier 1; priority a demóhoz hasonló), `plan`/`parse`, observation-építés (source-entity id = normalizált név-azonosító: `slug(lower(name))`), `csv` crate.
- [ ] **Step 4: Pipeline-e2e** wiremock CSV-vel: seedelt csapatok/refek → canary tick → kanonikus meccs + verified_publish; seed nélkül → review + nincs meccs.
- [ ] **Step 5: Zöld + kapu + commit** (`feat: add footballdata CSV adapter`).

### Task 4: Policy/trust seed + operations doc + zárás

- [x] **Step 1:** `docs/operations/footballdata.md` — policy/trust seed SQL, kétfázisú bootstrap, canary→live, rollback.
- [x] **Step 2:** Teljes kapu + demo-regresszió; **checkpoint** S3b-2 szekció.
- [x] **Step 3:** Commit (`docs: close S3b-2 checkpoint`); **élő kör külön engedéllyel** (nem része a tasknak).
- [x] **Step 4: STOP — zárójelentés.**
