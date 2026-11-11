# Valós adat bekötése — megvalósítási terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A SwiftUI főoldal a SampleData helyett a footballd API valós válaszából épül fel; a backend új megjelenítési tényezőket kap (tier/prioritás, tabella, odds snapshot, HT/ET/pens bekötés).

**Architecture:** A `0003` migráció két új fact táblát és a competition tier/priority oszlopait adja hozzá. A fixture v2 formátumot az adapter observationökké alakítja, a kanonizáló idempotensen materializálja. A `/v1/day` válasz bővül (backward kompatibilis), a Swift Core opcionális mezőkkel dekódol, az app liga-csoportokat épít. A demóadat bővített fixture-párként töltődik a dev DB-re.

**Tech Stack:** Rust (sqlx, axum, serde), PostgreSQL 18, SwiftUI (Swift 6 app target, SwiftPM Core), XcodeGen.

**Spec:** `docs/superpowers/specs/2026-11-11-real-data-wiring-design.md`

## Global Constraints

- AGENTS.md érvényes: magyar dokumentáció és hibaüzenetek; Write/Edit után a magyar szöveg visszaolvasása kötelező.
- Commit/push CSAK kifejezett felhasználói kérésre — a terv lépései ezért nem tartalmaznak commitot; a szelet végén checkpoint-jelentés van.
- Branch: `feat/real-data-wiring` (nem a main).
- Tesztadatbázis-first: migráció és constraint-próba először a `football_v2_test`-en, dev csak zöld ellenőrzés után.
- Hiányzó adat NULL/„–”, soha nem nulla; minden időpont UTC.
- Production kódban nincs `unwrap()`/`expect()` (tesztekben megengedett).
- Új külső függőség nem kell; minden a workspace-ben már deklarálva van.
- Rust toolchain: `rust-toolchain.toml` (1.85); `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Swift: Core `cd apps/FootballAppCore && swift test`; app `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test -project apps/FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO`; új fájloknál `xcodegen generate` az `apps/` könyvtárban.
- Integrációs tesztek után a tesztDB-t `./scripts/db-reset.sh` állítja vissza (23/6/0).
- A `.env` betöltése: `set -a; . ./.env; set +a` a repo gyökerében.

---

### Task 1: 0003 migráció — tier/priority, standing, odds_snapshot

**Files:**
- Create: `db/migrations/0003_display_facts.sql`
- Modify: `rust/crates/storage/tests/connection.rs` (21 → 23)
- Modify: `scripts/db-check.sh` (21 → 23, üzleti számlálók + 2 tábla)
- Modify: `rust/crates/storage/src/test_support.rs` (TRUNCATE lista + 2 tábla)

**Interfaces:**
- Produces: `core.competition.tier smallint`, `core.competition.priority integer`, `core.standing(season_id, team_id, position, source_id, captured_at)`, `fact.odds_snapshot(match_id, source_id, captured_at, market_count, odds jsonb)` — a Task 2–4 ezekre épít.

- [ ] **Step 1: Bukó teszt — a connection teszt elvárása 23**

`rust/crates/storage/tests/connection.rs` assert módosítása:

```rust
    assert_eq!(
        tables, 23,
        "a 0001–0003 migráció után 22 terméktábla és a _sqlx_migrations várható"
    );
```

- [ ] **Step 2: Futtatás — piros**

Run: `cd rust && cargo test -p football-storage --test connection`
Expected: FAIL, `assertion left == right failed: left: 21, right: 23`.

- [ ] **Step 3: Migráció megírása**

`db/migrations/0003_display_facts.sql`:

```sql
-- 0003 — megjelenítési tények: competition tier/priority, tabella, odds snapshot.
-- A sqlx a public._sqlx_migrations táblában tartja nyilván; ismételt futtatása nem hajtódik végre újra.
-- Visszaállítási út: scripts/db-reset.sh (kizárólag a tesztadatbázison), amely minden sémát és
-- a migrációs nyilvántartást törli, majd a 0001–0003 migrációkat együtt újrafuttatja.

BEGIN;

ALTER TABLE core.competition
    ADD COLUMN tier smallint NOT NULL DEFAULT 1 CHECK (tier >= 0),
    ADD COLUMN priority integer NOT NULL DEFAULT 100;

CREATE TABLE core.standing (
    season_id uuid NOT NULL REFERENCES core.season(id),
    team_id uuid NOT NULL REFERENCES core.team(id),
    position smallint NOT NULL CHECK (position > 0),
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    PRIMARY KEY (season_id, team_id)
);

CREATE TABLE fact.odds_snapshot (
    match_id uuid PRIMARY KEY REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    market_count integer NOT NULL CHECK (market_count >= 0),
    odds jsonb NOT NULL
);

COMMIT;
```

- [ ] **Step 4: Migráció a tesztadatbázison + zöld teszt**

Run (gyökérben, .env betöltve):
`DATABASE_URL="$TEST_DATABASE_URL" sqlx migrate run --source db/migrations`
Expected: `Applied 3/migrate display facts`.
Run: `cd rust && cargo test -p football-storage --test connection` → PASS.

- [ ] **Step 5: Constraint-próbák a tesztadatbázison**

psql `$TEST_DATABASE_URL` — mindkettőnek BUKNIA kell:
```sql
-- position 0 → CHECK hiba
INSERT INTO core.standing (season_id, team_id, position, source_id, captured_at)
VALUES (gen_random_uuid(), gen_random_uuid(), 0, gen_random_uuid(), now());
-- nem létező meccs → FK hiba
INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, odds)
VALUES (gen_random_uuid(), gen_random_uuid(), now(), 18, '{}'::jsonb);
```
Expected: `violates check constraint "standing_position_check"`, illetve `violates foreign key constraint "odds_snapshot_match_id_fkey"`.

- [ ] **Step 6: db-check.sh és test_support igazítása**

`scripts/db-check.sh`: `[ "$tables" -ne 21 ]` → `-ne 23`; a HIBA üzenet „0002 migráció utáni” → „0003 migráció utáni”; az üzleti összeghez két új tag:
```bash
    (SELECT count(*) FROM ops.publish_gate) +
    (SELECT count(*) FROM core.standing) + (SELECT count(*) FROM fact.odds_snapshot);")"
```
`rust/crates/storage/src/test_support.rs` TRUNCATE elejére: `fact.odds_snapshot, core.standing,` a meglévő lista elé.

- [ ] **Step 7: Dev adatbázis migráció + teljes ellenőrzés**

Run: `sqlx migrate run --source db/migrations` (dev) → `Applied 3`.
Run: `./scripts/db-check.sh` → dev OK 23/6/43, teszt OK 23/6/0.
Run: `cd rust && cargo fmt --all --check && cargo test -p football-domain -p football-storage && cargo clippy -p football-domain -p football-storage --all-targets -- -D warnings` → zöld.
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh` → 23/6/0 teszt, dev változatlan.

---

### Task 2: Fixture v2 — odds és standing observationök

**Files:**
- Create: `tests/fixtures/fixture_d_display.json`
- Create: `rust/crates/ingest/tests/display_observations.rs`
- Modify: `rust/crates/ingest/src/fixture.rs`

**Interfaces:**
- Consumes: Task 1 sémája (az observationök még nem írnak kanonikus táblát).
- Produces: `FixtureCompetition { tier: i16, priority: i32 }` (serde default 1/100), `FixtureOdds { home, draw, away, btts_yes, btts_no, over25, under25: f64 }`, `FixtureMatch { odds: Option<FixtureOdds>, market_count: Option<i32> }`, `FixtureStanding { team_source_entity_id: String, position: i16 }`, `FixtureSource { standings: Option<Vec<FixtureStanding>> }`; observation aspectek: `match_odds` (entity_kind `match`, natural_key = kanonikus meccs UUID, payload: `canonical_match_id`, `odds`, `market_count`), `standing` (entity_kind `team`, natural_key = kanonikus csapat UUID, payload: `team_id`, `position`, `season_id`).

- [ ] **Step 1: Fixture és bukó teszt**

`tests/fixtures/fixture_d_display.json`:

```json
{
  "source_code": "fixture_d",
  "source_display_name": "Fixture D",
  "captured_at": "2026-11-11T08:00:00Z",
  "endpoint_key": "day-fixtures",
  "request_url": "fixture://fixture_d/day/2026-11-11",
  "adapter_version": "fixture-d-1.0.0",
  "parser_version": "fixture-parser-1.0.0",
  "competition": {
    "id": "30000000-0000-7000-8000-000000000001",
    "name": "Hungarian NB I",
    "country_code": "HUN",
    "country_name": "Hungary",
    "tier": 1,
    "priority": 6
  },
  "season": { "id": "30000000-0000-7000-8000-000000000002", "label": "2026/27" },
  "teams": [
    { "id": "30000000-0000-7000-8000-000000000011", "source_entity_id": "D-UJP", "name": "Újpest", "country_code": "HUN", "gender": "male", "age_group": "senior", "branch": "senior" },
    { "id": "30000000-0000-7000-8000-000000000012", "source_entity_id": "D-GYOR", "name": "Győr", "country_code": "HUN", "gender": "male", "age_group": "senior", "branch": "senior" }
  ],
  "matches": [
    {
      "id": "30000000-0000-7000-8000-000000000101",
      "source_match_id": "D-1001",
      "home_source_entity_id": "D-UJP",
      "away_source_entity_id": "D-GYOR",
      "round_label": "13",
      "kickoff_at": "2026-11-11T18:00:00Z",
      "competition_timezone": "Europe/Budapest",
      "status": "scheduled",
      "result": null,
      "odds": { "home": 2.20, "draw": 3.30, "away": 3.20, "btts_yes": 1.85, "btts_no": 1.95, "over25": 2.15, "under25": 1.70 },
      "market_count": 18
    }
  ],
  "standings": [
    { "team_source_entity_id": "D-UJP", "position": 8 },
    { "team_source_entity_id": "D-GYOR", "position": 6 }
  ]
}
```

`rust/crates/ingest/tests/display_observations.rs`:

```rust
use football_ingest::fixture::FixtureAdapter;
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn display_fields_become_observations() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_d_display.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &fixture_path)
        .await
        .unwrap();

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM stage.observation")
        .fetch_one(&pool).await.unwrap();
    let odds: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM stage.observation WHERE aspect='match_odds'")
        .fetch_one(&pool).await.unwrap();
    let standing: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM stage.observation WHERE aspect='standing'")
        .fetch_one(&pool).await.unwrap();
    let odds_home: String = sqlx::query_scalar(
        "SELECT normalized_payload->'odds'->>'home' FROM stage.observation WHERE aspect='match_odds'")
        .fetch_one(&pool).await.unwrap();
    let standing_position: i16 = sqlx::query_scalar(
        "SELECT (normalized_payload->>'position')::smallint FROM stage.observation
         WHERE aspect='standing' AND natural_key='30000000-0000-7000-8000-000000000011'")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(total, 6, "2 team_identity + 1 match_core + 1 match_odds + 2 standing");
    assert_eq!(odds, 1);
    assert_eq!(standing, 2);
    assert_eq!(odds_home, "2.2");
    assert_eq!(standing_position, 8);
}
```

- [ ] **Step 2: Futtatás — piros**

Run: `cd rust && cargo test -p football-ingest --test display_observations`
Expected: FAIL, `total: 3` (a serde a bővített mezőket még figyelmen kívül hagyja, csak 3 observation keletkezik).

- [ ] **Step 3: Implementáció a fixture.rs-ben**

`FixtureCompetition` bővítése (a `country_name` után):

```rust
    #[serde(default = "default_tier")]
    pub tier: i16,
    #[serde(default = "default_priority")]
    pub priority: i32,
```

Új típusok és függvények:

```rust
fn default_tier() -> i16 { 1 }
fn default_priority() -> i32 { 100 }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureOdds {
    pub home: f64,
    pub draw: f64,
    pub away: f64,
    pub btts_yes: f64,
    pub btts_no: f64,
    pub over25: f64,
    pub under25: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureStanding {
    pub team_source_entity_id: String,
    pub position: i16,
}
```

`FixtureMatch` bővítése (a `result` után):

```rust
    #[serde(default)]
    pub odds: Option<FixtureOdds>,
    #[serde(default)]
    pub market_count: Option<i32>,
```

`FixtureSource` bővítése (a `matches` után):

```rust
    #[serde(default)]
    pub standings: Option<Vec<FixtureStanding>>,
```

Az `ingest` függvényben a match-ciklus UTÁN, az `Ok(IngestReport …)` elé:

```rust
        for fixture_match in &fixture.matches {
            if let Some(odds) = &fixture_match.odds {
                let payload = serde_json::json!({
                    "canonical_match_id": fixture_match.id,
                    "odds": odds,
                    "market_count": fixture_match.market_count,
                });
                let observation_id = insert_observation(
                    pool,
                    source_id,
                    capture_id,
                    "match",
                    "match_odds",
                    &fixture_match.id.to_string(),
                    &fixture_match.source_match_id,
                    fixture.captured_at,
                    payload,
                )
                .await?;
                observation_ids.push(observation_id);
            }
        }

        if let Some(standings) = &fixture.standings {
            for standing in standings {
                let team = fixture
                    .teams
                    .iter()
                    .find(|team| team.source_entity_id == standing.team_source_entity_id)
                    .with_context(|| {
                        format!(
                            "ismeretlen csapatforrás a tabellában: {}",
                            standing.team_source_entity_id
                        )
                    })?;
                let payload = serde_json::json!({
                    "team_id": team.id,
                    "position": standing.position,
                    "season_id": fixture.season.id,
                });
                let observation_id = insert_observation(
                    pool,
                    source_id,
                    capture_id,
                    "team",
                    "standing",
                    &team.id.to_string(),
                    &team.source_entity_id,
                    fixture.captured_at,
                    payload,
                )
                .await?;
                observation_ids.push(observation_id);
            }
        }
```

- [ ] **Step 4: Zöld + teljes ellenőrzés**

Run: `cd rust && cargo test -p football-ingest --test display_observations` → PASS.
Run: `cargo test --workspace` → zöld (a régi fixture-ök változatlanul fogynak).
Run: `cargo fmt --all && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` → tiszta.
Run (gyökér): `./scripts/db-reset.sh && ./scripts/db-check.sh` → 23/6/0.
Magyar szöveg visszaolvasása: „ismeretlen csapatforrás a tabellában”.

---

### Task 3: Canonicalizer — odds_snapshot és standing materializálás

**Files:**
- Create: `rust/crates/ingest/tests/display_materialize.rs`
- Modify: `rust/crates/ingest/src/canonicalize.rs`

**Interfaces:**
- Consumes: Task 1 sémája, Task 2 fixture v2 mezői (`fixture.competition.tier/priority`, `fixture_match.odds/market_count`, `fixture.standings`).
- Produces: materializált `core.competition.tier/priority`, `fact.odds_snapshot` (meccsenként 1 sor, forrás + captured_at), `core.standing` (csapatonként 1 sor a szezonban). A `MaterializedDay` API változatlan.

- [ ] **Step 1: Bukó teszt**

`rust/crates/ingest/tests/display_materialize.rs`:

```rust
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn display_facts_materialize_idempotently() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_d_display.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &fixture_path).await.unwrap();
    let fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();

    Canonicalizer::materialize(&pool, &fixture).await.unwrap();
    Canonicalizer::materialize(&pool, &fixture).await.unwrap();

    let (tier, priority): (i16, i32) = sqlx::query_as(
        "SELECT tier, priority FROM core.competition WHERE id='30000000-0000-7000-8000-000000000001'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!((tier, priority), (1, 6));

    let odds_count: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.odds_snapshot")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(odds_count, 1, "ismételt materializálás nem duplikál");

    let (home, market_count): (String, i32) = sqlx::query_as(
        "SELECT odds->>'home', market_count FROM fact.odds_snapshot
         WHERE match_id='30000000-0000-7000-8000-000000000101'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(home, "2.2");
    assert_eq!(market_count, 18);

    let standing_count: i64 = sqlx::query_scalar("SELECT count(*) FROM core.standing")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(standing_count, 2);

    let position: i16 = sqlx::query_scalar(
        "SELECT position FROM core.standing WHERE team_id='30000000-0000-7000-8000-000000000011'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(position, 8);

    let source_code: String = sqlx::query_scalar(
        "SELECT s.code FROM fact.odds_snapshot o JOIN ops.source s ON s.id=o.source_id")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(source_code, "fixture_d");
}
```

- [ ] **Step 2: Futtatás — piros**

Run: `cd rust && cargo test -p football-ingest --test display_materialize`
Expected: FAIL, `odds_count: 0` (a kanonizáló még nem írja a fact táblákat).

- [ ] **Step 3: Implementáció a canonicalize.rs-ben**

A competition upsert cseréje:

```rust
        sqlx::query(
            "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
             VALUES ($1,$1,$2,$3,$4,$5)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               tier = EXCLUDED.tier,
               priority = EXCLUDED.priority",
        )
        .bind(fixture.competition.id)
        .bind(country_id)
        .bind(&fixture.competition.name)
        .bind(fixture.competition.tier)
        .bind(fixture.competition.priority)
        .execute(&mut *tx)
        .await?;
```

A match-ciklus UTÁN, a `tx.commit()` elé:

```rust
        let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
            .bind(&fixture.source_code)
            .fetch_one(&mut *tx)
            .await?;

        for fixture_match in &fixture.matches {
            let Some(odds) = &fixture_match.odds else { continue };
            let market_count = fixture_match
                .market_count
                .with_context(|| format!("hiányzó piacszám az odds mellé: {}", fixture_match.id))?;
            sqlx::query(
                "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, odds)
                 VALUES ($1,$2,$3,$4,$5)
                 ON CONFLICT (match_id) DO UPDATE SET
                   source_id = EXCLUDED.source_id,
                   captured_at = EXCLUDED.captured_at,
                   market_count = EXCLUDED.market_count,
                   odds = EXCLUDED.odds",
            )
            .bind(fixture_match.id)
            .bind(source_id)
            .bind(fixture.captured_at)
            .bind(market_count)
            .bind(serde_json::to_value(odds)?)
            .execute(&mut *tx)
            .await?;
        }

        if let Some(standings) = &fixture.standings {
            for standing in standings {
                let team_id = team_by_source_id(&mut tx, &standing.team_source_entity_id).await?;
                sqlx::query(
                    "INSERT INTO core.standing (season_id, team_id, position, source_id, captured_at)
                     VALUES ($1,$2,$3,$4,$5)
                     ON CONFLICT (season_id, team_id) DO UPDATE SET
                       position = EXCLUDED.position,
                       source_id = EXCLUDED.source_id,
                       captured_at = EXCLUDED.captured_at",
                )
                .bind(fixture.season.id)
                .bind(team_id)
                .bind(standing.position)
                .bind(source_id)
                .bind(fixture.captured_at)
                .execute(&mut *tx)
                .await?;
            }
        }
```

- [ ] **Step 4: Zöld + teljes ellenőrzés**

Run: `cd rust && cargo test -p football-ingest --test display_materialize` → PASS.
Run: `cargo test --workspace && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` → zöld.
Run (gyökér): `./scripts/db-reset.sh && ./scripts/db-check.sh` → 23/6/0.
Magyar szöveg visszaolvasása: „hiányzó piacszám az odds mellé”.

---

### Task 4: API — /v1/day bővítés

**Files:**
- Modify: `rust/crates/storage/tests/day_query.rs` (új teszt)
- Modify: `rust/crates/storage/src/repositories/day.rs`
- Modify: `rust/crates/api/src/routes/day.rs`

**Interfaces:**
- Consumes: Task 1 sémája.
- Produces: `DayMatchRow` új mezői: `competition_id: Uuid`, `competition_name: String`, `country_code: Option<String>`, `country_name: Option<String>`, `tier: i16`, `priority: i32`, `home_ht/away_ht/home_et/away_et/home_pens/away_pens: Option<i16>`, `home_position/away_position: Option<i16>`, `odds: Option<serde_json::Value>`, `market_count: Option<i32>`, `odds_source: Option<String>`, `odds_captured_at: Option<DateTime<Utc>>`. A JSON válasz ugyanezeket a snake_case kulcsokat adja (a Swift Core ezekre dekódol).

- [ ] **Step 1: Bukó storage-teszt**

Új teszt a `rust/crates/storage/tests/day_query.rs` végére (a meglévő seed helperek újrahasználva):

```rust
#[tokio::test]
async fn day_row_includes_display_fields() {
    let pool = test_pool().await;
    let base = seed_match_base(&pool).await;
    let match_id = seed_match(
        &pool,
        &base,
        "verified_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
    )
    .await;

    sqlx::query("UPDATE core.competition SET tier=1, priority=6 WHERE id=$1")
        .bind(base.competition_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO core.match_result (match_id, home_ft, away_ft, home_ht, away_ht)
         VALUES ($1,2,1,1,0)")
        .bind(match_id).execute(&pool).await.unwrap();
    let source_id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'demo','Demó')")
        .bind(source_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, odds)
         VALUES ($1,$2,now(),18,'{\"home\":2.2,\"draw\":3.3,\"away\":3.2}'::jsonb)")
        .bind(match_id).bind(source_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO core.standing (season_id, team_id, position, source_id, captured_at)
         VALUES ($1,$2,8,$3,now()),($1,$4,6,$3,now())")
        .bind(base.season_id).bind(base.home_team_id).bind(source_id).bind(base.away_team_id)
        .execute(&pool).await.unwrap();

    let rows = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.competition_name, "NB I");
    assert_eq!(row.country_code.as_deref(), Some("HU"));
    assert_eq!((row.tier, row.priority), (1, 6));
    assert_eq!((row.home_ht, row.away_ht), (Some(1), Some(0)));
    assert_eq!((row.home_position, row.away_position), (Some(8), Some(6)));
    assert_eq!(row.market_count, Some(18));
    assert_eq!(row.odds_source.as_deref(), Some("Demó"));
    assert_eq!(row.odds.as_ref().unwrap()["home"], 2.2);
    assert!(row.odds_captured_at.is_some());
}
```

- [ ] **Step 2: Futtatás — piros**

Run: `cd rust && cargo test -p football-storage --test day_query`
Expected: FAIL, fordítási hiba (`no field competition_name on DayMatchRow`).

- [ ] **Step 3: Implementáció — storage day.rs**

A `DayMatchRow` bővítése (a `blockers` után):

```rust
    pub competition_id: Uuid,
    pub competition_name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub tier: i16,
    pub priority: i32,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    pub home_position: Option<i16>,
    pub away_position: Option<i16>,
    pub odds: Option<serde_json::Value>,
    pub market_count: Option<i32>,
    pub odds_source: Option<String>,
    pub odds_captured_at: Option<DateTime<Utc>>,
```

Az SQL cseréje:

```rust
        "SELECT
            m.id AS match_id,
            m.kickoff_at,
            m.status,
            home.name AS home_name,
            away.name AS away_name,
            r.home_ft AS home_score,
            r.away_ft AS away_score,
            g.state AS publish_state,
            g.blockers,
            c.id AS competition_id,
            c.name AS competition_name,
            co.code AS country_code,
            co.name AS country_name,
            c.tier,
            c.priority,
            r.home_ht, r.away_ht, r.home_et, r.away_et, r.home_pens, r.away_pens,
            sh.position AS home_position,
            sa.position AS away_position,
            o.odds,
            o.market_count,
            os.display_name AS odds_source,
            o.captured_at AS odds_captured_at
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         LEFT JOIN core.country co ON co.id = c.country_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
         LEFT JOIN core.standing sh ON sh.season_id = m.season_id AND sh.team_id = m.home_team_id
         LEFT JOIN core.standing sa ON sa.season_id = m.season_id AND sa.team_id = m.away_team_id
         LEFT JOIN fact.odds_snapshot o ON o.match_id = m.id
         LEFT JOIN ops.source os ON os.id = o.source_id
         JOIN ops.publish_gate g ON g.match_id = m.id AND g.publish_date = $1
         WHERE (m.kickoff_at AT TIME ZONE $2)::date = $1
           AND ($3::boolean OR g.state = 'verified_publish')
         ORDER BY m.kickoff_at, home.name, away.name",
```

- [ ] **Step 4: Implementáció — api routes/day.rs**

A `DayMatch` struct bővítése (a `blockers` után, azonos mezőnevek/típusok mint a `DayMatchRow`-ban), és a mapping bővítése:

```rust
                competition_id: row.competition_id,
                competition_name: row.competition_name,
                country_code: row.country_code,
                country_name: row.country_name,
                tier: row.tier,
                priority: row.priority,
                home_ht: row.home_ht,
                away_ht: row.away_ht,
                home_et: row.home_et,
                away_et: row.away_et,
                home_pens: row.home_pens,
                away_pens: row.away_pens,
                home_position: row.home_position,
                away_position: row.away_position,
                odds: row.odds,
                market_count: row.market_count,
                odds_source: row.odds_source,
                odds_captured_at: row.odds_captured_at,
```

- [ ] **Step 5: Zöld + teljes ellenőrzés**

Run: `cd rust && cargo test --workspace` → zöld (18 + 3 új = 21 teszt).
Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` → tiszta.
Run (gyökér): `./scripts/db-reset.sh && ./scripts/db-check.sh` → 23/6/0.

---

### Task 5: Demo fixture-pár + dev pipeline E2E

**Files:**
- Create: `tests/fixtures/demo_day_a_<liga>.json` (13 fájl) és `demo_day_b_<liga>.json` (13 fájl) — ideiglenes generátorral
- Modify: nem szükséges termék-kód

**Interfaces:**
- Consumes: Task 2–3 fixture v2 pipeline, `footballctl ingest-fixture --path`.
- Produces: dev DB-ben 2026-11-11-i nap 20 meccsel, `verified_publish` gate-ekkel; a Swift kliens ezt fogyasztja.

- [ ] **Step 1: Generátor elkészítése (ideiglenes, /tmp-ben, nem commitolandó)**

`/tmp/gen-demo-fixtures.mjs` — Node script, amely a SampleData struktúráját tükröző adatból 13+13 fixture-t ír a `tests/fixtures/` alá. Adat (liga slug, ország kód/név, tier, prioritás, csapatok SampleData szerint):

| slug | verseny | ország | tier | prio | meccsek (helyi idő, státusz, FT, HT, ET, pens) |
|---|---|---|---|---|---|
| pl | Premier League | ENG/England | 1 | 1 | Arsenal–Chelsea 16:00 finished 2–1 HT 1–0; Manchester City–Liverpool 17:30 live 1–1 HT 1–0; Newcastle–Tottenham 18:30 scheduled |
| ll | La Liga | ESP/Spain | 1 | 2 | Real Madrid–Barcelona 21:00 finished 3–2 HT 2–1; Atlético Madrid–Sevilla 19:00 live 0–0 HT 0–0; Betis–Valencia 20:30 scheduled |
| sa | Serie A | ITA/Italy | 1 | 3 | Inter–Juventus 18:00 scheduled; Milan–Napoli 20:45 scheduled |
| bl | Bundesliga | GER/Germany | 1 | 4 | Bayern München–Dortmund 17:30 finished 4–2 HT 2–1; RB Leipzig–Leverkusen 18:30 scheduled |
| l1 | Ligue 1 | FRA/France | 1 | 5 | PSG–Marseille 21:00 scheduled |
| nb1 | NB I | HUN/Hungary | 1 | 6 | Ferencváros–Újpest 19:00 live 1–0 HT 1–0; Debrecen–Győr 17:00 scheduled |
| eng2 | Championship | ENG/England | 2 | 7 | Leeds United–West Brom 20:45 scheduled |
| esp2 | LaLiga 2 | ESP/Spain | 2 | 8 | Eibar–Levante 21:00 scheduled |
| ita2 | Serie B | ITA/Italy | 2 | 9 | Palermo–Sampdoria 20:30 scheduled |
| ger2 | 2. Bundesliga | GER/Germany | 2 | 10 | Hamburger SV–Schalke 04 18:30 scheduled |
| fra2 | Ligue 2 | FRA/France | 2 | 11 | Saint-Étienne–Bordeaux 20:00 scheduled |
| nb2 | NB II | HUN/Hungary | 2 | 12 | Vasas–Szentlőrinc 17:00 scheduled |
| facup | FA Kupa | ENG/England | 0 | 13 | Manchester United–Liverpool 16:30 finished 1–1 HT 1–0 ET 2–2 pens 4–3 |

Odds meccsenként a SampleData `o(...)` értékei sorrendben: pl (1.85,3.40,4.20,1.72,2.05,2.10,1.75), (2.10,3.20,3.40,1.80,1.95,1.85,1.95), (2.60,3.30,2.70,1.90,1.90,2.05,1.78); ll (2.30,3.50,3.00,1.55,2.35,1.70,2.15), (1.70,3.60,5.00,2.10,1.70,2.30,1.62), (2.40,3.20,2.90,1.85,1.95,2.20,1.65); sa (2.05,3.25,3.60,1.95,1.85,2.25,1.62), (2.50,3.30,2.75,1.80,2.00,2.05,1.78); bl (1.65,4.00,4.60,1.45,2.60,1.55,2.40), (2.70,3.40,2.55,1.75,2.05,1.95,1.88); l1 (1.55,4.20,5.50,1.60,2.30,1.75,2.05); nb1 (1.45,4.30,6.00,2.00,1.80,2.40,1.55), (2.20,3.30,3.20,1.85,1.95,2.15,1.70); eng2 (1.95,3.30,3.90,1.90,1.90,2.10,1.72); esp2 (2.30,3.10,3.10,1.88,1.92,2.20,1.65); ita2 (2.45,3.15,2.95,1.85,1.95,2.15,1.68); ger2 (2.15,3.40,3.25,1.80,2.00,2.05,1.78); fra2 (2.35,3.20,3.05,1.85,1.95,2.10,1.72); nb2 (1.80,3.50,4.10,1.95,1.85,2.30,1.62); facup (2.40,3.30,2.80,1.50,2.45,1.80,1.95).

Tabella (csak tier ≥ 1 ligák, SampleData.positions): Arsenal 1, Chelsea 5, Manchester City 2, Liverpool 3, Newcastle 6, Tottenham 7, Real Madrid 1, Barcelona 2, Atlético Madrid 4, Sevilla 9, Betis 7, Valencia 12, Inter 1, Juventus 3, Milan 2, Napoli 5, Bayern München 1, Dortmund 4, RB Leipzig 3, Leverkusen 2, PSG 1, Marseille 3, Ferencváros 1, Újpest 8, Debrecen 4, Győr 6, Leeds United 2, West Brom 9, Eibar 4, Levante 6, Palermo 5, Sampdoria 8, Hamburger SV 3, Schalke 04 7, Saint-Étienne 5, Bordeaux 10, Vasas 4, Szentlőrinc 12. (A Liverpool PL-ben 3, kupában nincs standing sor.)

Generálási szabályok:
- UUID séma, L = liga sorszám 01–0d (hex): competition `20000000-0000-7000-8000-00000000LL01`, season `…LL02`, csapat `…LL10+i`, meccs `…LL20+i` (LL = L két hex jegye, i = 0-tól).
- Csapat forrásentitás-ID: `D-<sorszám>` (demo_a) / `E-<sorszám>` (demo_b), a kanonikus UUID azonos a két forrásban.
- `kickoff_at`: `2026-11-11T<HelyiIdő>:00+01:00` (november = CET).
- `captured_at`: a = `2026-11-11T08:00:00Z`, b = `2026-11-11T09:00:00Z`.
- `source_code`: `demo_day_a` / `demo_day_b`; `source_display_name`: mindkettő `Demo odds feed`; `endpoint_key`: `day-fixtures`; `request_url`: `fixture://demo_day_a/day/2026-11-11` (b-nél b); `adapter_version`: `demo-1.0.0`; `parser_version`: `fixture-parser-1.0.0`.
- `market_count`: 12 + meccs sorszám (0-tól), meccsenként.
- Eredmény: finished/live → `result` FT + HT (+ ET + pens a kupánál); scheduled → `result: null`.
- `standings`: a liga tier ≥ 1 esetén a fenti pozíciók; kupánál üres tömb.
- A b fájlok az a fájlokból származnak: source_code/display/captured_at/request_url/csapat source_entity_id prefix cseréje, minden más azonos.

Run: `node /tmp/gen-demo-fixtures.mjs` → 26 fájl a `tests/fixtures/` alatt.
Ellenőrzés: `python3 -c "import json,glob; [json.load(open(f)) for f in glob.glob('tests/fixtures/demo_day_*.json')]"` → nincs hiba; `ls tests/fixtures/demo_day_*.json | wc -l` → 26.

- [ ] **Step 2: Betöltés a dev adatbázison**

Run (gyökér, .env betöltve, `DATA_DIR=data`):
```bash
for f in tests/fixtures/demo_day_a_*.json tests/fixtures/demo_day_b_*.json; do
  cargo run -p footballctl -- ingest-fixture --path "$f" || exit 1
done
```
Expected: minden sor `state=verified_publish` (két egyező forrás).

- [ ] **Step 3: E2E evidence**

Run: `cargo run -p footballctl -- day --date 2026-11-11` → 20 meccs, mind verified_publish.
Run: `cargo run -p footballctl -- doctor` → `db=1 gates=21` (1 régi + 20 új).
psql dev: `SELECT count(*) FROM core.match` → 21; `SELECT count(*) FROM fact.odds_snapshot` → 20; `SELECT count(*) FROM core.standing` → 38; `SELECT state, count(*) FROM ops.publish_gate GROUP BY state` → verified_publish 21.
Run: `cargo run -p footballctl -- day --date 2026-11-09` → 1 meccs (a régi nap megmaradt).

- [ ] **Step 4: TesztDB nyugalmi állapot**

Run: `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev OK 23/6/üzleti (nőtt), teszt OK 23/6/0.

---

### Task 6: Swift Core — DayMatch bővítés

**Files:**
- Modify: `apps/FootballAppCore/Sources/FootballAppCore/DayModels.swift`
- Modify: `apps/FootballAppCore/Tests/FootballAppCoreTests/DayModelsTests.swift`

**Interfaces:**
- Consumes: Task 4 JSON válasz (snake_case kulcsok).
- Produces: `OddsSnapshot { home, draw, away, bttsYes, bttsNo, over25, under25: Double }` (public, Decodable, Equatable, Sendable); `DayMatch` új opcionális mezői: `competitionId: UUID?`, `competitionName: String?`, `countryCode: String?`, `countryName: String?`, `tier: Int?`, `priority: Int?`, `homeHt/awayHt/homeEt/awayEt/homePens/awayPens: Int?`, `homePosition/awayPosition: Int?`, `odds: OddsSnapshot?`, `marketCount: Int?`, `oddsSource: String?`, `oddsCapturedAt: Date?`.

- [ ] **Step 1: Bukó tesztek**

Két új teszt a `DayModelsTests` suite-ba:

```swift
    @Test func decodesDisplayFields() throws {
        let json = """
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000201","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"finished","home":"Újpest","away":"Győr","home_score":2,"away_score":1,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-000000000001",
           "competition_name":"NB I","country_code":"HUN","country_name":"Hungary",
           "tier":1,"priority":6,
           "home_ht":1,"away_ht":0,"home_et":null,"away_et":null,"home_pens":null,"away_pens":null,
           "home_position":8,"away_position":6,
           "odds":{"home":2.2,"draw":3.3,"away":3.2,"btts_yes":1.85,"btts_no":1.95,"over25":2.15,"under25":1.7},
           "market_count":18,"odds_source":"Demo odds feed","odds_captured_at":"2026-11-11T09:00:00Z"}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.competitionName == "NB I")
        #expect(match.countryCode == "HUN")
        #expect(match.tier == 1)
        #expect(match.priority == 6)
        #expect(match.homeHt == 1)
        #expect(match.awayHt == 0)
        #expect(match.homeEt == nil)
        #expect(match.homePosition == 8)
        #expect(match.awayPosition == 6)
        #expect(match.odds?.home == 2.2)
        #expect(match.odds?.bttsYes == 1.85)
        #expect(match.odds?.under25 == 1.7)
        #expect(match.marketCount == 18)
        #expect(match.oddsSource == "Demo odds feed")
        #expect(match.oddsCapturedAt != nil)
    }

    @Test func missingDisplayFieldsDecodeAsNil() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.competitionId == nil)
        #expect(match.odds == nil)
        #expect(match.homePosition == nil)
        #expect(match.marketCount == nil)
    }
```

- [ ] **Step 2: Futtatás — piros**

Run: `cd apps/FootballAppCore && swift test`
Expected: FAIL, fordítási hiba (`value of type 'DayMatch' has no member 'competitionName'`).

- [ ] **Step 3: Implementáció a DayModels.swift-ben**

Új struktúra a `DayMatch` elé:

```swift
public struct OddsSnapshot: Decodable, Equatable, Sendable {
    public let home: Double
    public let draw: Double
    public let away: Double
    public let bttsYes: Double
    public let bttsNo: Double
    public let over25: Double
    public let under25: Double

    enum CodingKeys: String, CodingKey {
        case home, draw, away
        case bttsYes = "btts_yes"
        case bttsNo = "btts_no"
        case over25, under25
    }
}
```

A `DayMatch` bővítése (a `blockers` után) — minden új mező opcionális:

```swift
    public let competitionId: UUID?
    public let competitionName: String?
    public let countryCode: String?
    public let countryName: String?
    public let tier: Int?
    public let priority: Int?
    public let homeHt: Int?
    public let awayHt: Int?
    public let homeEt: Int?
    public let awayEt: Int?
    public let homePens: Int?
    public let awayPens: Int?
    public let homePosition: Int?
    public let awayPosition: Int?
    public let odds: OddsSnapshot?
    public let marketCount: Int?
    public let oddsSource: String?
    public let oddsCapturedAt: Date?
```

CodingKeys bővítése:

```swift
        case competitionId = "competition_id"
        case competitionName = "competition_name"
        case countryCode = "country_code"
        case countryName = "country_name"
        case tier, priority
        case homeHt = "home_ht"
        case awayHt = "away_ht"
        case homeEt = "home_et"
        case awayEt = "away_et"
        case homePens = "home_pens"
        case awayPens = "away_pens"
        case homePosition = "home_position"
        case awayPosition = "away_position"
        case odds
        case marketCount = "market_count"
        case oddsSource = "odds_source"
        case oddsCapturedAt = "odds_captured_at"
```

- [ ] **Step 4: Zöld**

Run: `cd apps/FootballAppCore && swift test` → 15 teszt / 4 suite PASS (a régi 13 + 2 új).

---

### Task 7: App — HomeView valós adat + vizuális ellenőrzés

**Files:**
- Create: `apps/FootballApp/MatchCardModel.swift`
- Create: `apps/FootballApp/LeagueGroup.swift`
- Create: `apps/FootballAppTests/LeagueGroupTests.swift`
- Modify: `apps/FootballApp/MatchListView.swift`
- Modify: `apps/FootballApp/HomeView.swift`
- Modify: `apps/FootballApp/SampleData.swift` (MatchOdds/Outcome átköltözik, Prediction törölve)
- Regenerate: `apps/project.yml` → `xcodegen generate`

**Interfaces:**
- Consumes: Task 6 `DayMatch` mezői; meglévő `DayModel`, `EmptyStateView`, `ErrorStateView(message:retry:)`, `APIError.userMessage`, `MatchFilter`.
- Produces: `MatchCardModel` (kártya-display modell, `from(_ match: DayMatch)` és `from(_ match: SampleMatch)` építőkkel), `LeagueGroup` (`fromDay(_ matches: [DayMatch]) -> [LeagueGroup]`, `fromSample(_ leagues: [SampleLeague]) -> [LeagueGroup]`, `filtering(_:)`, `divisionLabel`), `CountryDisplay` (kód → zászló + magyar név).

- [ ] **Step 1: Bukó app-teszt**

`apps/FootballAppTests/LeagueGroupTests.swift` (XCTest, a teszt-target Swift 5 módú):

```swift
import XCTest
import FootballAppCore
@testable import FootballApp

final class LeagueGroupTests: XCTestCase {
    private func decode(_ json: String) throws -> [DayMatch] {
        let data = json.data(using: .utf8)!
        return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data).matches
    }

    func testGroupsSortByPriorityThenCountry() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6},
          {"id":"00000000-0000-0000-0000-000000000002","kickoff_at":"2026-11-11T16:00:00Z",
           "status":"scheduled","home":"C","away":"D","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a2","competition_name":"Premier League",
           "country_code":"ENG","country_name":"England","tier":1,"priority":1},
          {"id":"00000000-0000-0000-0000-000000000003","kickoff_at":"2026-11-11T19:00:00Z",
           "status":"scheduled","home":"E","away":"F","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a2","competition_name":"Premier League",
           "country_code":"ENG","country_name":"England","tier":1,"priority":1}
        ]}
        """)
        let groups = LeagueGroup.fromDay(matches)
        XCTAssertEqual(groups.count, 2)
        XCTAssertEqual(groups[0].name, "Premier League")
        XCTAssertEqual(groups[0].matches.count, 2)
        XCTAssertEqual(groups[0].country, "Anglia")
        XCTAssertEqual(groups[1].name, "NB I")
        XCTAssertEqual(groups[1].country, "Magyarország")
    }

    func testCardModelCarriesDisplayFields() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"finished","home":"Újpest","away":"Győr","home_score":2,"away_score":1,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6,
           "home_ht":1,"away_ht":0,"home_position":8,"away_position":6,
           "odds":{"home":2.2,"draw":3.3,"away":3.2,"btts_yes":1.85,"btts_no":1.95,"over25":2.15,"under25":1.7},
           "market_count":18,"odds_source":"Demo odds feed"}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertEqual(card.home, "Újpest")
        XCTAssertEqual(card.homeScore, 2)
        XCTAssertEqual(card.homeHt, 1)
        XCTAssertEqual(card.homePosition, 8)
        XCTAssertEqual(card.awayPosition, 6)
        XCTAssertEqual(card.odds?.home, 2.2)
        XCTAssertEqual(card.markets, 18)
        XCTAssertEqual(card.oddsSource, "Demo odds feed")
        XCTAssertEqual(card.winner1x2, .home)
        XCTAssertEqual(card.bttsYesWon, true)
        XCTAssertEqual(card.over25Won, true)
        XCTAssertNil(card.minute)
    }

    func testUnknownCountryFallsBackToName() {
        let display = CountryDisplay.display(code: "XYZ", name: "Ismeretlen ország")
        XCTAssertEqual(display.name, "Ismeretlen ország")
        XCTAssertEqual(display.flag, "🏳️")
    }
}
```

- [ ] **Step 2: Futtatás — piros**

Run: `cd apps && xcodegen generate && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test -project FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO`
Expected: FAIL, fordítási hiba (`cannot find 'LeagueGroup' in scope`).

- [ ] **Step 3: MatchCardModel.swift**

```swift
import Foundation
import FootballAppCore

struct MatchOdds {
    let home: Double
    let draw: Double
    let away: Double
    let bttsYes: Double
    let bttsNo: Double
    let over25: Double
    let under25: Double
}

enum Outcome {
    case home, draw, away
}

/// A kártya display-modellje: valós API-adatból és mintaadatból is építhető.
struct MatchCardModel: Identifiable {
    let id: String
    let home: String
    let away: String
    let time: String
    let status: String
    let homeScore: Int?
    let awayScore: Int?
    let homeHt: Int?
    let awayHt: Int?
    let homeEt: Int?
    let awayEt: Int?
    let homePens: Int?
    let awayPens: Int?
    let minute: Int?
    let odds: MatchOdds?
    let markets: Int
    let oddsSource: String
    let homePosition: Int?
    let awayPosition: Int?

    var winner1x2: Outcome? {
        guard status == "finished", let home = homeScore, let away = awayScore else { return nil }
        if home > away { return .home }
        if home < away { return .away }
        return .draw
    }

    var bttsYesWon: Bool? {
        guard status == "finished", let home = homeScore, let away = awayScore else { return nil }
        return home > 0 && away > 0
    }

    var over25Won: Bool? {
        guard status == "finished", let home = homeScore, let away = awayScore else { return nil }
        return home + away >= 3
    }

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()

    static func from(_ match: DayMatch) -> MatchCardModel {
        MatchCardModel(
            id: match.id.uuidString,
            home: match.home,
            away: match.away,
            time: timeFormatter.string(from: match.kickoffAt),
            status: match.status,
            homeScore: match.homeScore,
            awayScore: match.awayScore,
            homeHt: match.homeHt,
            awayHt: match.awayHt,
            homeEt: match.homeEt,
            awayEt: match.awayEt,
            homePens: match.homePens,
            awayPens: match.awayPens,
            minute: nil,
            odds: match.odds.map {
                MatchOdds(home: $0.home, draw: $0.draw, away: $0.away,
                          bttsYes: $0.bttsYes, bttsNo: $0.bttsNo,
                          over25: $0.over25, under25: $0.under25)
            },
            markets: match.marketCount ?? 0,
            oddsSource: match.oddsSource ?? "–",
            homePosition: match.homePosition,
            awayPosition: match.awayPosition
        )
    }

    static func from(_ match: SampleMatch) -> MatchCardModel {
        MatchCardModel(
            id: match.id, home: match.home, away: match.away, time: match.time,
            status: match.status, homeScore: match.homeScore, awayScore: match.awayScore,
            homeHt: match.homeHt, awayHt: match.awayHt, homeEt: match.homeEt, awayEt: match.awayEt,
            homePens: match.homePens, awayPens: match.awayPens, minute: match.minute,
            odds: match.odds, markets: match.markets, oddsSource: SampleData.oddsSource,
            homePosition: SampleData.position(of: match.home),
            awayPosition: SampleData.position(of: match.away)
        )
    }
}
```

- [ ] **Step 4: LeagueGroup.swift**

```swift
import Foundation
import FootballAppCore

enum CountryDisplay {
    private static let table: [String: (flag: String, name: String)] = [
        "ENG": ("🏴󠁧󠁢󠁥󠁮󠁧󠁿", "Anglia"),
        "ESP": ("🇪🇸", "Spanyolország"),
        "ITA": ("🇮🇹", "Olaszország"),
        "GER": ("🇩🇪", "Németország"),
        "FRA": ("🇫🇷", "Franciaország"),
        "HUN": ("🇭🇺", "Magyarország"),
    ]

    static func display(code: String?, name: String?) -> (flag: String, name: String) {
        if let code, let known = table[code] { return known }
        return ("🏳️", name ?? code ?? "Ismeretlen")
    }
}

struct LeagueGroup: Identifiable {
    let id: String
    let name: String
    let country: String
    let flag: String
    let tier: Int
    let priority: Int
    let matches: [MatchCardModel]

    var divisionLabel: String {
        switch tier {
        case 1: return "1. osztály"
        case 2: return "2. osztály"
        default: return "Kupa"
        }
    }

    func filtering(_ filter: MatchFilter) -> LeagueGroup? {
        let kept = matches.filter { filter.matches(status: $0.status) }
        guard !kept.isEmpty else { return nil }
        return LeagueGroup(id: id, name: name, country: country, flag: flag,
                           tier: tier, priority: priority, matches: kept)
    }

    static func fromDay(_ matches: [DayMatch]) -> [LeagueGroup] {
        let grouped = Dictionary(grouping: matches) { $0.competitionId?.uuidString ?? "unknown" }
        return grouped.map { key, matches in
            let first = matches[0]
            let display = CountryDisplay.display(code: first.countryCode, name: first.countryName)
            return LeagueGroup(
                id: key,
                name: first.competitionName ?? "Verseny",
                country: display.name,
                flag: display.flag,
                tier: first.tier ?? 1,
                priority: first.priority ?? 100,
                matches: matches.map(MatchCardModel.from)
            )
        }
        .sorted {
            $0.priority != $1.priority ? $0.priority < $1.priority : $0.country < $1.country
        }
    }

    static func fromSample(_ leagues: [SampleLeague]) -> [LeagueGroup] {
        leagues.map { league in
            LeagueGroup(
                id: league.id, name: league.name, country: league.country, flag: league.flag,
                tier: league.tier, priority: league.priority,
                matches: league.matches.map(MatchCardModel.from)
            )
        }
    }
}
```

- [ ] **Step 5: MatchListView.swift átírása**

- `MatchListView.leagues: [LeagueGroup]`; `LeagueSectionView.league: LeagueGroup`; `sortedMatches` marad (`$0.time < $1.time`).
- `MatchItemRow.match: MatchCardModel`; a `teamLine` pozíciója: `teamLine(_ name: String, position: Int?)` — hívás: `teamLine(match.home, position: match.homePosition)` / `teamLine(match.away, position: match.awayPosition)`; a `SampleData.position(of:)` hívás törlődik.
- `predictionBlock`, `predItem`, `ProbabilityBar` törlődik; a body-ból a `predictionBlock` sor kikerül.
- `oddsRow`: csak ha `match.odds != nil` (a body: `if match.odds != nil { oddsRow }`); a forrás-szöveg: `Text("forrás: \(match.oddsSource)")`.
- A `minute` megjelenítés marad (valós adatnál nil).

- [ ] **Step 6: SampleData.swift tisztítás**

- `MatchOdds` és `Outcome` definíciók törlődnek (átköltöztek a MatchCardModel.swift-be).
- `Prediction` struktúra és a `SampleMatch.prediction` computed property törlődik (nincs többé használó).
- A `SampleMatch` többi része és a `SampleData` enum marad (preview-khoz).

- [ ] **Step 7: HomeView.swift — valós adat**

A `HomeView` törzse (a privát segédnézetek — TopNavigationBar, HomeHeader, FilterBar, FilterChip, MonthCalendar — megmaradnak, az alábbi módosításokkal):

```swift
struct HomeView: View {
    @State private var model = DayModel()
    @State private var filter: MatchFilter = .all
    @State private var country: String? = nil
    @State private var division: Int? = nil

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var appeared = false

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            TopNavigationBar()

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            HomeHeader(
                date: $model.selectedDate,
                count: loadedCount,
                appeared: appeared,
                onPrevious: { model.goToPreviousDay() },
                onToday: { model.goToToday() },
                onNext: { model.goToNextDay() },
                onRefresh: { Task { await model.load() } }
            )

            FilterBar(filter: $filter, country: $country, division: $division,
                      counts: filterCounts, countries: allCountries)

            content
        }
        .background(AppTheme.background)
        .onAppear {
            withAnimation(reduceMotion ? nil : .easeOut(duration: 0.4)) { appeared = true }
        }
        .task(id: model.selectedDate) { await model.load() }
    }

    @ViewBuilder
    private var content: some View {
        switch model.state {
        case .idle, .loading:
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let matches):
            MatchListView(leagues: filteredLeagues(matches))
        case .empty:
            EmptyStateView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: { Task { await model.load() } })
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var loadedCount: Int? {
        if case .loaded(let matches) = model.state { return matches.count }
        return nil
    }

    private func baseGroups(_ matches: [DayMatch]) -> [LeagueGroup] {
        LeagueGroup.fromDay(matches).filter { league in
            (country == nil || league.country == country) &&
            (division == nil || league.tier == division)
        }
    }

    private var allCountries: [String] {
        guard case .loaded(let matches) = model.state else { return [] }
        return Array(Set(LeagueGroup.fromDay(matches).map(\.country))).sorted()
    }

    private var filterCounts: [MatchFilter: Int] {
        guard case .loaded(let matches) = model.state else { return [:] }
        let all = baseGroups(matches).flatMap { $0.matches }
        var result: [MatchFilter: Int] = [:]
        for candidate in MatchFilter.allCases {
            result[candidate] = all.filter { candidate.matches(status: $0.status) }.count
        }
        return result
    }

    private func filteredLeagues(_ matches: [DayMatch]) -> [LeagueGroup] {
        baseGroups(matches).compactMap { $0.filtering(filter) }
    }
}
```

Megjegyzés: a `shift`/`replayEntrance` privát függvények törlődnek (a DayModel navigál, a refresh betölt).

- [ ] **Step 8: TopNavigationBar és FilterBar módosítások**

- `TopNavigationBar`: a „MINTA ADAT” badge teljes HStack-je törlődik (az óra marad).
- `FilterBar`: új `let countries: [String]` paraméter; a `ForEach(SampleData.countries, id: \.self)` → `ForEach(countries, id: \.self)`.

- [ ] **Step 9: Zöld tesztek**

Run: `cd apps && xcodegen generate && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test -project FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO`
Expected: TEST SUCCEEDED (11 meglévő + 3 új = 14 teszt).
Run: `cd apps/FootballAppCore && swift test` → 15 teszt zöld.

- [ ] **Step 10: Vizuális ellenőrzés a futó appban**

1. Daemon indítás: `cargo build -p footballd`, majd `BIND_ADDR=127.0.0.1:0 cargo run -p footballd` háttérben (.env betöltve, dev DB); a port a runtime.json-ból / naplóból.
2. App build + indítás: `xcodebuild build` (ugyanaz a DEVELOPER_DIR), a `.app` megnyitása.
3. Screenshotok (ablak-specifikus `screencapture`):
   - mai nap (2026-11-11): liga-csoportok prioritás-sorrendben, tabella-helyezések, odds, élő pulzálás perc nélkül, NINCS „MINTA ADAT” és NINCS előrejelzés blokk;
   - lezárt meccs: nyerő-kiemelés az oddsokon, HT eredmény;
   - FA Kupa: `11-es → 120' → 90' → HT` szekvencia, nincs helyezés-zárójel;
   - szűrők: „Befejezett” chip + ország menü (valós országok);
   - naptár popover;
   - üres nap (pl. 2026-11-12): „Nincs meccs ezen a napon.”;
   - hibaállapot: daemon leállítva → Újra gombos hibaüzenet, majd daemon újraindítva → Újra működik.
4. A screenshotok alapján egyeztetés a felhasználóval; ha a design eltér, javítás és ismétlés.

---

### Task 8: Checkpoint, README, záró ellenőrzés

**Files:**
- Create: `docs/checkpoints/2026-11-11/checkpoint.md`
- Modify: `README.md` (állapot bekezdés)

- [ ] **Step 1: Teljes ellenőrző lánc**

Run: `./scripts/check.sh` → reset + fmt + clippy + szekvenciális workspace-tesztek (21 teszt) zöld.
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev OK (üzleti sorok száma rögzítendő), teszt OK 23/6/0.
Run: `cd apps/FootballAppCore && swift test` → 15 zöld.
Run: app `xcodebuild test` → 14 zöld.
Run: `git status --short --branch` → feat/real-data-wiring, commitolatlan változások listája.

- [ ] **Step 2: Checkpoint írása**

`docs/checkpoints/2026-11-11/checkpoint.md`: induló állapot, a napi szeletek (Task 1–7) pipálva csak futtatott ellenőrzés után, futtatott parancsok és eredmények, eltérések, nyitott kérdések, következő döntési pont. Magyar szöveg visszaolvasása.

- [ ] **Step 3: README állapotfrissítés**

Az „Állapot” bekezdés kiegészül: a főoldal valós API-adatból dolgozik (bővített fixture demóadat), 0003 migráció (23 tábla), odds snapshot + tabella. Magyar szöveg visszaolvasása.

- [ ] **Step 4: Zárójelentés**

Összesített jelentés a felhasználónak: mi változott, mely fájlok, mi futott, mi maradt ellenőrizetlen, visszaállítási út. Commit/push csak külön kérésre.