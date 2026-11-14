# S3b-1 — Observáció-vezérelt kanonikus út + identity bootstrap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (subagent-típusok nem elérhetők ebben a sessionben → inline execution). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `FixtureSource`-újraolvasás kivezetése: közös `NormalizedDay` + `from_observations` builder + a Canonicalizer átállítása, a fixture és a CLI ugyanazon az úton; élő forrásokhoz `identity approve --create` bootstrap. Demo-regresszió változatlan.

**Architecture:** `NormalizedDay` a domain/ingest határon (forrás-azonosítók + opcionális kanonikus id-hintek); `From<&FixtureSource>` és `from_observations(&[Observation])` ugyanazt a napot adják; `Canonicalizer::materialize(&NormalizedDay)`; a tick live ága parse → observationök → builder → materialize/verify; a CLI ugyanezt.

**Tech Stack:** Rust (edition 2024), sqlx 0.8, serde/serde_json.

**Spec:** `docs/superpowers/specs/2026-11-14-s3b-footballdata-design.md` (1. szekció; a bootstrap a terv-jegyzet kiegészítése)

## Global Constraints

- AGENTS.md §2: **minden task végén STOP**; **commit zöld task után** (Bandi86 identitás, push nélkül).
- Munkaág: `feat/s3b1-observer-path` a main (a6ea92d) csúcsáról.
- Magyar doksi/hibaszöveg, angol azonosítók; visszaolvasás + glyph-scan.
- **Demo-regresszió:** a meglévő `canonicalize_fixture`, `tick_e2e`, `trust_e2e`, `identity_e2e`, `day_query`, `display_*`, `worker_loop` tesztek **változatlan elvárásokkal** zöldek (a refaktor nem változtat viselkedést).
- Nincs migráció; 0 élő kérés.

## Fájlstruktúra

| Fájl | Felelősség |
|---|---|
| `rust/crates/ingest/src/normalized.rs` (új) | `NormalizedDay` + al-struktúrák + `From<&FixtureSource>` + `from_observations` |
| `rust/crates/ingest/src/canonicalize.rs` (módosul) | `materialize(&NormalizedDay)` (a test portja) |
| `rust/crates/worker/src/tick.rs` (módosul) | live ág: observations → `NormalizedDay` → verify |
| `rust/bins/footballctl/src/main.rs` (módosul) | IngestFixture: `NormalizedDay`-út |
| `rust/crates/identity/src/review.rs` + CLI (módosul) | `approve_create` (entitás-létrehozás + kötés) |
| Tesztek | `ingest/tests/normalized.rs` (új), `identity/tests/review.rs`, meglévő regressziók |

---

### Task 1: NormalizedDay + materializer-port (viselkedés-változás nélkül)

**Files:**
- Create: `rust/crates/ingest/src/normalized.rs`
- Modify: `rust/crates/ingest/src/canonicalize.rs`, `rust/crates/ingest/src/lib.rs`, `rust/bins/footballctl/src/main.rs`

**Interfaces:**
- Produces: `NormalizedDay { source_code, captured_at, competition, season, teams, matches, standings }` + al-struktúrák (`NormalizedCompetition`, `NormalizedSeason`, `NormalizedTeam`, `NormalizedMatch`, `NormalizedResult`, `NormalizedOdds`, `NormalizedQuote`, `NormalizedStanding`); `impl From<&FixtureSource> for NormalizedDay`; `Canonicalizer::materialize(&PgPool, &NormalizedDay)`.
- A kanonikus id-hintek: `NormalizedTeam.id: Option<Uuid>`, `NormalizedMatch.id: Option<Uuid>` — a fixture-út ezeket kitölti (a demo id-stabilitás), az élő út `None`-t ad (a materializer generál, natural-key upserttel).

- [ ] **Step 1: Bukó teszt** — `ingest/tests/normalized.rs`:

```rust
use football_ingest::fixture::FixtureSource;
use football_ingest::normalized::NormalizedDay;

#[test]
fn fixture_source_converts_losslessly() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let fixture: FixtureSource = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let day = NormalizedDay::from(&fixture);
    assert_eq!(day.competition.name, fixture.competition.name);
    assert_eq!(day.teams.len(), fixture.teams.len());
    assert_eq!(day.matches.len(), fixture.matches.len());
    assert_eq!(day.matches[0].id, Some(fixture.matches[0].id));
    assert_eq!(
        day.matches[0].home_source_entity_id,
        fixture.matches[0].home_source_entity_id
    );
    let quotes = day.matches.iter().flat_map(|m| m.odds.as_ref()).flat_map(|o| &o.quotes).count();
    assert!(quotes >= 7 * day.matches.iter().filter(|m| m.odds.is_some()).count());
}
```

- [ ] **Step 2: Futtatás — bukás.** `cargo test -p football-ingest --test normalized` → FAIL (modul nincs).

- [ ] **Step 3: Implementáció — `normalized.rs`**

```rust
//! Forrás-független normalizált nap: a kanonikus út közös bemenete.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::fixture::FixtureSource;

#[derive(Debug, Clone)]
pub struct NormalizedDay {
    pub source_code: String,
    pub captured_at: DateTime<Utc>,
    pub competition: NormalizedCompetition,
    pub season: NormalizedSeason,
    pub teams: Vec<NormalizedTeam>,
    pub matches: Vec<NormalizedMatch>,
    pub standings: Option<Vec<NormalizedStanding>>,
}

#[derive(Debug, Clone)]
pub struct NormalizedCompetition {
    pub name: String,
    pub country_code: String,
    pub country_name: String,
    pub tier: i16,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct NormalizedSeason {
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedTeam {
    pub id: Option<Uuid>,
    pub source_entity_id: String,
    pub name: String,
    pub country_code: String,
    pub gender: String,
    pub age_group: String,
    pub branch: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedMatch {
    pub id: Option<Uuid>,
    pub source_match_id: String,
    pub home_source_entity_id: String,
    pub away_source_entity_id: String,
    pub round_label: Option<String>,
    pub kickoff_at: DateTime<Utc>,
    pub competition_timezone: String,
    pub status: String,
    pub result: Option<NormalizedResult>,
    pub odds: Option<NormalizedOdds>,
}

#[derive(Debug, Clone)]
pub struct NormalizedResult {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
}

#[derive(Debug, Clone)]
pub struct NormalizedOdds {
    pub market_count: Option<i32>,
    pub quotes: Vec<NormalizedQuote>,
}

#[derive(Debug, Clone)]
pub struct NormalizedQuote {
    pub bookmaker: String,
    pub market: String,
    pub outcome: String,
    pub line: f64,
    pub phase: String,
    pub price: f64,
}

#[derive(Debug, Clone)]
pub struct NormalizedStanding {
    pub team_source_entity_id: String,
    pub position: i16,
}

/// A demó flat odds → kanonikus quote-ok (default bookmaker, closing) —
/// ugyanaz a leképezés, mint a canonicalize::odds_quotes-ban.
fn default_quotes(odds: &crate::fixture::FixtureOdds) -> Vec<NormalizedQuote> {
    let q = |market: &str, outcome: &str, line: f64, price: f64| NormalizedQuote {
        bookmaker: "default".into(),
        market: market.into(),
        outcome: outcome.into(),
        line,
        phase: "closing".into(),
        price,
    };
    vec![
        q("1x2", "home", 0.0, odds.home),
        q("1x2", "draw", 0.0, odds.draw),
        q("1x2", "away", 0.0, odds.away),
        q("btts", "yes", 0.0, odds.btts_yes),
        q("btts", "no", 0.0, odds.btts_no),
        q("ou", "over", 2.5, odds.over25),
        q("ou", "under", 2.5, odds.under25),
    ]
}

impl From<&FixtureSource> for NormalizedDay {
    fn from(fixture: &FixtureSource) -> Self {
        Self {
            source_code: fixture.source_code.clone(),
            captured_at: fixture.captured_at,
            competition: NormalizedCompetition {
                name: fixture.competition.name.clone(),
                country_code: fixture.competition.country_code.clone(),
                country_name: fixture.competition.country_name.clone(),
                tier: fixture.competition.tier,
                priority: fixture.competition.priority,
            },
            season: NormalizedSeason {
                label: fixture.season.label.clone(),
            },
            teams: fixture
                .teams
                .iter()
                .map(|t| NormalizedTeam {
                    id: Some(t.id),
                    source_entity_id: t.source_entity_id.clone(),
                    name: t.name.clone(),
                    country_code: t.country_code.clone(),
                    gender: t.gender.clone(),
                    age_group: t.age_group.clone(),
                    branch: t.branch.clone(),
                })
                .collect(),
            matches: fixture
                .matches
                .iter()
                .map(|m| NormalizedMatch {
                    id: Some(m.id),
                    source_match_id: m.source_match_id.clone(),
                    home_source_entity_id: m.home_source_entity_id.clone(),
                    away_source_entity_id: m.away_source_entity_id.clone(),
                    round_label: m.round_label.clone(),
                    kickoff_at: m.kickoff_at,
                    competition_timezone: m.competition_timezone.clone(),
                    status: m.status.clone(),
                    result: m.result.as_ref().map(|r| NormalizedResult {
                        home_ft: r.home_ft,
                        away_ft: r.away_ft,
                        home_ht: r.home_ht,
                        away_ht: r.away_ht,
                        home_et: r.home_et,
                        away_et: r.away_et,
                        home_pens: r.home_pens,
                        away_pens: r.away_pens,
                    }),
                    odds: m.odds.as_ref().map(|o| NormalizedOdds {
                        market_count: m.market_count,
                        quotes: default_quotes(o),
                    }),
                })
                .collect(),
            standings: fixture.standings.as_ref().map(|rows| {
                rows.iter()
                    .map(|s| NormalizedStanding {
                        team_source_entity_id: s.team_source_entity_id.clone(),
                        position: s.position,
                    })
                    .collect()
            }),
        }
    }
}
```

`lib.rs`: `pub mod normalized;`.

- [ ] **Step 4: Materializer-port** — `canonicalize.rs`:

  - `pub async fn materialize(pool: &PgPool, day: &NormalizedDay) -> Result<MaterializedDay>`.
  - A függvény törzsét mechanikusan átnevezni: `fixture.competition` → `day.competition`,
    `fixture.season` → `day.season`, `fixture.teams` → `day.teams`,
    `fixture.matches` → `day.matches`, `fixture.standings` → `day.standings`,
    `fixture.captured_at` → `day.captured_at`, `fixture.source_code` → `day.source_code`.
  - Ahol canonical UUID kell: `let id = day_match.id.unwrap_or_else(Uuid::now_v7);` (a fixture-út hintjeit használja → a demo id-k változatlanok).
  - A team-hivatkozások: a match home/away source id → a day.teams közötti keresés (mint eddig).
  - Az odds-írás a `NormalizedOdds.quotes`-ból (a `default_quotes` már ugyanazt adja, mint a régi `odds_quotes` — a régi fn törölhető, a quote-insert a `quotes` listán iterál).
  - Megtartandó: tranzakció, upsert-ek, observation-refek (`fixture_seed`), trust_state reset, minden viselkedés — **a meglévő tesztek a specifikáció**.

- [ ] **Step 5: Hívók átállítása**

  - `worker/src/tick.rs` `verify_and_publish(pool, &fixture)` → `verify_and_publish(pool, day: &NormalizedDay)`; a hívó a fixture-ágból `let day = NormalizedDay::from(&fixture);` (a Task 2-ben ez observations-alapúra vált).
  - `bins/footballctl` IngestFixture: `let day = NormalizedDay::from(&fixture); Canonicalizer::materialize(&pool, &day).await?;` — a többi hívás (`materialized.matches`, `materialized.snapshot` stb.) változatlan; a `resolve_team_identity(…, &fixture, …)` a `day`-t kapja (a függvény szignatúrája `&NormalizedDay`-re vált: a `fixture.teams` → `day.teams`; a `NormalizedTeam` azonos mezőkkel bír).

- [ ] **Step 6: Futtatás — zöld + regresszió.** `cargo test -p football-ingest --test normalized && cargo test --workspace -- --test-threads=1` → PASS (a demo-regressziós tesztek változatlanul).

- [ ] **Step 7: Teljes kapu + commit** (`refactor: introduce NormalizedDay canonical path`).

- [ ] **Step 8: STOP — jelentés.**

---

### Task 2: `from_observations` + a tick live-ág átállása

**Files:**
- Modify: `rust/crates/ingest/src/normalized.rs`, `rust/crates/worker/src/tick.rs`, `rust/crates/ingest/src/fixture.rs` (az observation-payloadok dokumentálása)
- Test: `rust/crates/ingest/tests/normalized.rs` (builder-egyenlőség), `worker/tests/tick_e2e.rs` (live ág)

**Interfaces:**
- Produces: `NormalizedDay::from_observations(&[Observation]) -> anyhow::Result<NormalizedDay>`.
- A payload-sémák (verziózott, dokumentált): `team_identity` = `FixtureTeam`
  mezők; `match_core` = `{canonical_match_id?, home_team_id?, away_team_id?,
  source_home_id, source_away_id, kickoff_at, status, result?, round_label?,
  competition_timezone}`; `match_odds` = `{canonical_match_id?, odds? (flat 7)
  VAGY quotes[{bookmaker,market,outcome,line,phase,price}], market_count?}`;
  `standing` = `{team_id?, season_id?, position, team_source_entity_id?}`.

- [ ] **Step 1: Bukó tesztek**

```rust
#[tokio::test]
async fn observations_build_the_same_day_as_the_fixture() {
    let pool = test_pool().await;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let fixture: FixtureSource = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let observations = football_ingest::fixture::build_observations(&fixture).unwrap();
    let from_obs = NormalizedDay::from_observations(&observations).unwrap();
    let direct = NormalizedDay::from(&fixture);
    assert_eq!(from_obs.matches.len(), direct.matches.len());
    assert_eq!(from_obs.teams.len(), direct.teams.len());
    assert_eq!(from_obs.matches[0].id, direct.matches[0].id);
    assert_eq!(from_obs.matches[0].kickoff_at, direct.matches[0].kickoff_at);
}
```

Megjegyzés: a `FixtureAdapter::ingest` `IngestReport`-ot ad vissza; a
`build_observations(&fixture) -> Result<Vec<Observation>>` a `fixture.rs`-ből
kiszervezett tiszta függvény (a persist és a builder ugyanazt használja;
a teszt nem igényel DB-t).

- [ ] **Step 2: Futtatás — bukás.**

- [ ] **Step 3: Implementáció — builder**

  - `fixture.rs`: `pub fn build_observations(fixture: &FixtureSource) -> anyhow::Result<Vec<Observation>>` (a jelenlegi payload-építő kód kiemelve, persist nélkül); `ingest` ezt hívja + persistálja.
  - `normalized.rs`: `from_observations` — az aspketek szerint csoportosít:
    - `team_identity` → `NormalizedTeam` (id = payload `id` ha van),
    - `match_core` → `NormalizedMatch` (meta a payloadból; a home/away source id-k),
    - `match_odds` → `NormalizedOdds` (flat → default quotes; `quotes` tömb → közvetlen),
    - `standing` → `NormalizedStanding`,
    - ismeretlen aspect → hiba (fail-closed, verziózott séma).
  - A competition/season meta a payloadokból nem vezethető le a fixture-út nélkül → a `match_core` payload egészüljön ki: `competition{name,country_code,country_name,tier,priority}` + `season{label}` (a fixture.rs payload-bővítése; a régi payloadok olvasásakor ezek hiányozhatnak → hibás builder? A demo a saját íróját használja, nincs backward-compat igény; a payload Séma v1 ezekkel kerül rögzítésre).

- [ ] **Step 4: Tick live-ág átállítás**

  ```rust
  // 8–9. Shadow ágban nincs kanonikus írás; live/canary ágban a teljes
  // verification + gate lánc fut az observationökből épített napon.
  if policy.mode != SourceMode::Shadow {
      let day = NormalizedDay::from_observations(&observations)
          .context("observationökből normalizált nap")?;
      verify_and_publish(pool, &day).await?;
  }
  ```

- [ ] **Step 5: Futtatás — zöld + teljes kapu + commit** (`feat: build canonical day from observations`).

- [ ] **Step 6: STOP — jelentés.**

---

### Task 3: Identity bootstrap — `approve --create`

**Files:**
- Modify: `rust/crates/identity/src/review.rs`, `rust/crates/identity/src/error.rs` (ha kell), `rust/bins/footballctl/src/main.rs`
- Test: `rust/crates/identity/tests/review.rs`

**Interfaces:**
- Produces: `review::approve_create(pool, queue_id, decided_by, note) -> Result<Uuid, IdentityError>`; CLI: `identity approve-create <queue_id> [--note]`.
- Kind-tudatos létrehozás: `team` → `core.entity` (kind, `proposed_name`) **+ `core.team`** (entity_id, country_id a sor kontextusának `country_code`-jából — ismeretlen ország → `InvalidInput`, name, gender/age_group/branch a kontextusból vagy `'male'/'senior'/'senior'` default); `referee` → csak `core.entity` (nincs külön tábla). A kapcsolás/döntés/audit a meglévő approve úton, egy tranzakcióban.

- [x] **Step 1: Bukó teszt** (review.rs):

```rust
#[tokio::test]
async fn approve_create_makes_entity_and_links() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let queue_id = pending_queue(&pool, source_id, "rev-new", &ctx_hu()).await;
    let entity_id = review::approve_create(&pool, queue_id, "cli", Some("új klub"))
        .await
        .unwrap();
    let (kind, name): (String, String) =
        sqlx::query_as("SELECT kind, canonical_name FROM core.entity WHERE id=$1")
            .bind(entity_id).fetch_one(&pool).await.unwrap();
    assert_eq!(kind, "team");
    assert_eq!(name, "Ismeretlen Klub");
    let status: String = sqlx::query_scalar("SELECT status FROM identity.review_queue WHERE id=$1")
        .bind(queue_id).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "approved");
    assert_eq!(ref_count(&pool, source_id, "rev-new").await, 1);
}
```

- [x] **Step 2: Futtatás — bukás.**

- [x] **Step 3: Implementáció**

  - `review::approve_create(pool, queue_id, decided_by, note) -> Result<Uuid, IdentityError>`:
    - betölti a pending sort (`entity_kind`, `proposed_name`, `context`);
    - kind-tudatos létrehozás a tranzakcióban (team → entity + team; referee → entity), a kontextus `country_code`-jából oldott országgal;
    - a meglévő approve-lánc (kind-ellenőrzés a frissen létrehozott entitással, ref + döntés + audit) ugyanabban a tranzakcióban.
  - CLI: `IdentityAction::ApproveCreate { id, note }` → kiírja az új entity id-t.

- [x] **Step 4: Zöld + kétfázisú bootstrap dokumentálása** a checkpointban (1. élő kör → review-sorok; `approve-create`/`approve`; 2. kör → kanonikus meccsek).

- [x] **Step 5: Teljes kapu + commit** (`feat: add identity approve-create bootstrap`).

- [x] **Step 6: STOP — jelentés.**

---

### Task 4: Zárás — checkpoint + S3b-2 előkészítés

- [x] **Step 1: Teljes kapu** (check.sh, demo 20/20 változatlan, nincs migráció).
- [x] **Step 2: Checkpoint** — S3b-1 szekció (refaktor, builder, bootstrap, bizonyítékok; a kétfázisú indulás leírása; S3b-2 következik).
- [x] **Step 3: Commit** (`docs: close S3b-1 checkpoint`) + STOP.

---

## Önreview-jegyzet

- **Spec-lefedettség:** S3b spec 1. szekció (observáció-vezérelt út) → Task 1–2; az identity bootstrap a terv kiegészítése (a spec review-hivatkozása ezt feltételezi, a CLI-ben eddig nem volt) → Task 3; S3b-2 (CSV-adapter, odds-lánc, élő kör) külön ciklus.
- **Típus-konzisztencia:** `NormalizedDay`/`NormalizedMatch`/`NormalizedQuote` mezőnevek Task 1–2-ben azonosak; `from_observations` a Task 1 payload-sémáit használja.
- **Kockázat:** a materializer-port mechanikus, de széles felületű — a teljes regressziós suite a biztosíték; a payload-séma bővítése (competition/season a match_core-ban) a Task 2-ben történik, a demó nem használ régi payloadot.
