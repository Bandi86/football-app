# S2d-1 — S3-előkészítő séma-rések: implementációs terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (subagent-típusok nem elérhetők ebben a sessionben → inline execution). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A footballdata-adapter (S3) mezőigényének séma-alapja: normalizált `fact.odds_quote` a `odds_snapshot` fejléc alatt, per-source `fact.match_stats`, és referee/venue entitás-FK a `core.match`-en — teljes pipeline-átállítással, változatlan API/Swift mellett.

**Architecture:** Egy 0008 migráció hozza a két új táblát, backfilleli a meglévő `odds jsonb`-t `default`/`closing` quote-okra, majd dropolja az oszlopot. Mivel az oszlop dropja miatt a vágás atomi, egy taskban történik a writer (canonicalize), a trust-olvasó (load_match_odds) és a display (day LATERAL) átállítása; a 7 mezős `OddsValues` szerződés (trust + API) változatlan.

**Tech Stack:** Rust (edition 2024, rust-version 1.85), sqlx 0.8, PostgreSQL, tokio.

**Spec:** `docs/superpowers/specs/2026-11-14-s2d1-schema-gaps-design.md`

## Global Constraints

- AGENTS.md §2: **minden task végén STOP** — összefoglaló + bizonyíték, következő task csak felhasználói engedéllyel.
- AGENTS.md §9: **commit NEM része a task-lépéseknek** — commit csak kifejezett felhasználói kérésre.
- Munkaág: `feat/s2d1-schema-gaps` a `feat/s2c-obscura` csúcsáról (dffb264); a létrehozás a végrehajtás első lépése, felhasználói jóváhagyással. Nincs worktree.
- Dokumentumok/hibaüzenetek **magyarul**; kódazonosítók angolul. Magyar szöveg Write/Edit után visszaolvasandó.
- Kérésfeldolgozásban nincs unwrap/expect/elnémítás; tesztkódban megengedett.
- Tesztek: `TEST_DATABASE_URL` kötelező; integrációs tesztek egy szálon; hivatalos kapu `cargo test --workspace -- --test-threads=1` (a `./scripts/check.sh` resetel).
- Migráció: teszt DB-n (check.sh) → zöld → **dev DB csak a Task 2-ben, külön engedéllyel**.
- **0 élő kérés**, 0 nyers payload; a demo fixture-ök változatlanok.
- Táblaszám: 0008 után **35** (dev + test); demo 20/20 verified_publish + verified odds marad.

## Fájlstruktúra

| Fájl | Felelősség |
|---|---|
| `db/migrations/0008_s2d1_schema_gaps.sql` (új) | odds_quote + backfill + odds-drop; match_stats; referee/venue oszlopok |
| `rust/crates/ingest/src/canonicalize.rs` (módosul) | odds írás: fejléc-upsert + quote-blokk csere |
| `rust/crates/storage/src/repositories/trust.rs` (módosul) | `load_match_odds`: 7 kulcs a quote-okból |
| `rust/crates/storage/src/repositories/day.rs` (módosul) | display JSON a quote-okból |
| `rust/crates/storage/src/test_support.rs` (módosul) | TRUNCATE-lista + `seed_min_match` helper |
| `scripts/db-check.sh` (módosul) | elvárt táblaszám 33 → 35 |
| `rust/crates/storage/tests/s2d1_schema.rs` (új) | séma-constraint tesztek |
| `rust/crates/ingest/tests/canonicalize_fixture.rs` (módosul) | quote-írás + reingest teszt |
| `rust/crates/storage/tests/trust_repos.rs` (módosul) | load_match_odds seedek quote-okra |
| `rust/crates/storage/tests/day_query.rs` (módosul) | odds seedek quote-okra |
| `rust/crates/storage/tests/trust_schema.rs` (módosul) | seed_odds_row: fejléc (odds oszlop nélkül) |
| `rust/crates/ingest/tests/display_materialize.rs` (módosul) | `odds->>'home'` → quote-lekérdezés |

---

### Task 1: 0008 migráció + atomi odds-vágás + séma-tesztek

**Files:**
- Create: `db/migrations/0008_s2d1_schema_gaps.sql`
- Modify: `rust/crates/ingest/src/canonicalize.rs` (~177–202), `rust/crates/storage/src/repositories/trust.rs` (`load_match_odds`), `rust/crates/storage/src/repositories/day.rs` (~81–93), `rust/crates/storage/src/test_support.rs`, `scripts/db-check.sh`
- Test: `rust/crates/storage/tests/s2d1_schema.rs` (új), `rust/crates/ingest/tests/canonicalize_fixture.rs`, `rust/crates/storage/tests/trust_repos.rs`, `rust/crates/storage/tests/day_query.rs`, `rust/crates/storage/tests/trust_schema.rs`, `rust/crates/ingest/tests/display_materialize.rs`

**Interfaces:**
- Consumes: `FixtureOdds` (7 mező), `OddsValues`/`SourceOdds` (domain), `test_pool`.
- Produces: `fact.odds_quote` séma; írás `default`/`closing` konvencióval; `load_match_odds` ugyanazt a `Vec<SourceOdds>`-ot adja; a day-LATERAL JSON-ja ugyanazt a 7 mezőt adja; `test_support::seed_min_match(pool) -> (Uuid /*match*/, Uuid /*source*/)`; db-check 35.

> **Miért atomi:** a migráció dropolja az `odds_snapshot.odds` oszlopot; ha a writer/olvasók nem ugyanebben a taskban állnak át, a workspace nem fordul. A task lépései ezért bontottak, de a STOP a végén van.

- [x] **Step 1: Bukó séma-teszt (új fájl)**

`rust/crates/storage/tests/s2d1_schema.rs`:

```rust
use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn insert_quote(pool: &PgPool, match_id: Uuid, source_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','home',0,'closing',2.2)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(pool)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn odds_quote_constraints_and_cascade() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    // Fejléc nélkül FK-hiba (23503).
    match insert_quote(&pool, match_id, source_id).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23503")),
        other => panic!("FK-hibát vártunk, ehelyett: {other:?}"),
    }

    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),3)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    insert_quote(&pool, match_id, source_id).await.unwrap();

    // PK-duplikátum (23505).
    match insert_quote(&pool, match_id, source_id).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }

    // price > 1 CHECK (23514).
    let bad_price = sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','draw',0,'closing',1.0)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_price {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (price), ehelyett: {other:?}"),
    }

    // line >= 0 CHECK (23514).
    let bad_line = sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','ou','over',-1,'closing',1.9)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_line {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (line), ehelyett: {other:?}"),
    }

    // phase enum CHECK (23514).
    let bad_phase = sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','away',0,'mid',3.2)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_phase {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (phase), ehelyett: {other:?}"),
    }

    // Fejléc törlése kaszkádol a quote-okra.
    sqlx::query("DELETE FROM fact.odds_snapshot WHERE match_id=$1 AND source_id=$2")
        .bind(match_id)
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
    let quotes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.odds_quote WHERE match_id=$1 AND source_id=$2",
    )
    .bind(match_id)
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(quotes, 0, "a fejléc törlése kaszkádol");
}

#[tokio::test]
async fn match_stats_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    sqlx::query(
        "INSERT INTO fact.match_stats
         (match_id, source_id, captured_at, home_shots, away_shots, home_xg, away_xg)
         VALUES ($1,$2,now(),12,9,1.7,0.9)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    // PK-duplikátum (23505).
    let dup = sqlx::query(
        "INSERT INTO fact.match_stats (match_id, source_id, captured_at) VALUES ($1,$2,now())",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match dup {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }

    // Negatív érték CHECK (23514): másik source-dal.
    let other_source = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'neg','Negatív')")
        .bind(other_source)
        .execute(&pool)
        .await
        .unwrap();
    let bad = sqlx::query(
        "INSERT INTO fact.match_stats (match_id, source_id, captured_at, home_shots)
         VALUES ($1,$2,now(),-1)",
    )
    .bind(match_id)
    .bind(other_source)
    .execute(&pool)
    .await;
    match bad {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (home_shots), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn match_referee_and_venue_entity_fk() {
    let pool = test_pool().await;
    let (match_id, _source_id) = seed_min_match(&pool).await;
    let referee_id = Uuid::now_v7();
    let venue_id = Uuid::now_v7();
    for (id, kind, name) in [
        (referee_id, "referee", "Kovács István"),
        (venue_id, "venue", "Puskás Aréna"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(kind)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Ismeretlen uuid → FK-hiba (23503).
    let bad = sqlx::query("UPDATE core.match SET referee_entity_id=$2 WHERE id=$1")
        .bind(match_id)
        .bind(Uuid::now_v7())
        .execute(&pool)
        .await;
    match bad {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23503")),
        other => panic!("FK-hibát vártunk, ehelyett: {other:?}"),
    }

    sqlx::query(
        "UPDATE core.match SET referee_entity_id=$2, venue_entity_id=$3 WHERE id=$1",
    )
    .bind(match_id)
    .bind(referee_id)
    .bind(venue_id)
    .execute(&pool)
    .await
    .unwrap();
    let (r, v): (Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("SELECT referee_entity_id, venue_entity_id FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((r, v), (Some(referee_id), Some(venue_id)));
}
```

- [x] **Step 2: Futtatás — bukás.** `set -a; source ../.env; set +a; cd rust && cargo test -p football-storage --test s2d1_schema -- --test-threads=1` → FAIL (a `fact.odds_quote` nem létezik; a `seed_min_match` sem — ez a Step 3-ban készül).

- [x] **Step 3: Implementáció — migráció + test_support helper + db-check**

`db/migrations/0008_s2d1_schema_gaps.sql`:

```sql
-- 0008 — S2d-1 séma-rések: odds_quote + match_stats + referee/venue (S2d-1).
-- Visszaállítás: az odds jsonb újraépíthető a default/closing quote-okból
-- (lásd a lenti inverz mintát), a két új tábla és a két entitás-FK dropolható.
-- Inverz minta (kézi, dokumentált):
--   ALTER TABLE fact.odds_snapshot ADD COLUMN odds jsonb;
--   UPDATE fact.odds_snapshot o SET odds = (
--     SELECT jsonb_object_agg(k.field, q.price) FROM fact.odds_quote q
--     JOIN (VALUES ('home','1x2','home'),('draw','1x2','draw'),
--                  ('away','1x2','away'),('btts_yes','btts','yes'),
--                  ('btts_no','btts','no'),('over25','ou','over'),
--                  ('under25','ou','under')) AS k(field, market, outcome)
--       ON q.market=k.market AND q.outcome=k.outcome
--      WHERE q.match_id=o.match_id AND q.source_id=o.source_id
--        AND q.bookmaker='default' AND q.phase='closing'
--        AND (q.market <> 'ou' OR q.line = 2.5));
BEGIN;

CREATE TABLE fact.odds_quote (
    match_id uuid NOT NULL,
    source_id uuid NOT NULL,
    bookmaker text NOT NULL,
    market text NOT NULL,
    outcome text NOT NULL,
    line numeric(6,2) NOT NULL DEFAULT 0,
    phase text NOT NULL CHECK (phase IN ('opening', 'closing')),
    price numeric(8,3) NOT NULL CHECK (price > 1),
    PRIMARY KEY (match_id, source_id, bookmaker, market, outcome, line, phase),
    FOREIGN KEY (match_id, source_id)
        REFERENCES fact.odds_snapshot (match_id, source_id) ON DELETE CASCADE,
    CONSTRAINT odds_quote_line_check CHECK (line = 0 OR line > 0)
);

CREATE INDEX idx_odds_quote_match_market
    ON fact.odds_quote (match_id, market, outcome);

-- Backfill a meglévő 7 mezős JSON-ból (default bookmaker, closing fázis).
INSERT INTO fact.odds_quote
    (match_id, source_id, bookmaker, market, outcome, line, phase, price)
SELECT o.match_id, o.source_id, 'default', k.market, k.outcome, k.line, 'closing',
       (o.odds->>k.field)::numeric
FROM fact.odds_snapshot o
CROSS JOIN (VALUES
    ('1x2', 'home', 'home', 0::numeric),
    ('1x2', 'draw', 'draw', 0),
    ('1x2', 'away', 'away', 0),
    ('btts', 'yes', 'btts_yes', 0),
    ('btts', 'no', 'btts_no', 0),
    ('ou', 'over', 'over25', 2.5),
    ('ou', 'under', 'under25', 2.5)
) AS k(market, outcome, field, line)
WHERE o.odds ? k.field;

ALTER TABLE fact.odds_snapshot DROP COLUMN odds;

CREATE TABLE fact.match_stats (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    home_shots integer CHECK (home_shots >= 0),
    away_shots integer CHECK (away_shots >= 0),
    home_shots_on_target integer CHECK (home_shots_on_target >= 0),
    away_shots_on_target integer CHECK (away_shots_on_target >= 0),
    home_corners integer CHECK (home_corners >= 0),
    away_corners integer CHECK (away_corners >= 0),
    home_yellow integer CHECK (home_yellow >= 0),
    away_yellow integer CHECK (away_yellow >= 0),
    home_red integer CHECK (home_red >= 0),
    away_red integer CHECK (away_red >= 0),
    home_fouls integer CHECK (home_fouls >= 0),
    away_fouls integer CHECK (away_fouls >= 0),
    home_xg numeric CHECK (home_xg >= 0),
    away_xg numeric CHECK (away_xg >= 0),
    PRIMARY KEY (match_id, source_id)
);

ALTER TABLE core.match ADD COLUMN referee_entity_id uuid REFERENCES core.entity(id);
ALTER TABLE core.match ADD COLUMN venue_entity_id uuid REFERENCES core.entity(id);

COMMIT;
```

`rust/crates/storage/src/test_support.rs` — TRUNCATE-lista bővítése és a helper:

```rust
// a TRUNCATE lista elejére (a fact.odds_snapshot elé):
//   fact.odds_quote, fact.match_stats,

/// Minimál érvényes lánc a sématesztekhez: ország → verseny → szezon →
/// 2 csapat → 1 source → 1 meccs. Visszaadja (match_id, source_id).
pub async fn seed_min_match(pool: &PgPool) -> (Uuid, Uuid) {
    let country_id = Uuid::now_v7();
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_entity_id = Uuid::now_v7();
    let home_team_id = Uuid::now_v7();
    let away_entity_id = Uuid::now_v7();
    let away_team_id = Uuid::now_v7();
    let source_id = Uuid::now_v7();
    let match_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id).execute(pool).await.expect("country");
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')")
        .bind(competition_entity_id).execute(pool).await.expect("competition entity");
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'NB I')",
    )
    .bind(competition_id).bind(competition_entity_id).bind(country_id)
        .execute(pool).await.expect("competition");
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id).bind(competition_id).execute(pool).await.expect("season");
    for (entity_id, team_id, name) in [
        (home_entity_id, home_team_id, "Újpest"),
        (away_entity_id, away_team_id, "Győr"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity_id).bind(name).execute(pool).await.expect("team entity");
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        )
        .bind(team_id).bind(entity_id).bind(country_id).bind(name)
            .execute(pool).await.expect("team");
    }
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'seed','Seed')")
        .bind(source_id).execute(pool).await.expect("source");
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
          competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,'2026-11-14T18:00:00Z','Europe/Budapest','scheduled')",
    )
    .bind(match_id).bind(competition_id).bind(season_id)
        .bind(home_team_id).bind(away_team_id)
        .execute(pool).await.expect("match");
    (match_id, source_id)
}
```

`scripts/db-check.sh`: `expected_tables=35` és a fejléc-komment frissítése
(„0001–0008 migrációk után 35 tábla; a 0008 két új táblát ad”).

- [x] **Step 4: Futtatás — zöld.** `cargo test -p football-storage --test s2d1_schema -- --test-threads=1` → PASS (3 teszt).

- [x] **Step 5: Bukó writer-teszt** — `rust/crates/ingest/tests/canonicalize_fixture.rs`, új teszt a fájl végére:

```rust
#[tokio::test]
async fn ingest_writes_odds_quotes_with_default_bookmaker() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let mut fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    fixture.matches[0].odds = Some(FixtureOdds {
        home: 2.0,
        draw: 3.2,
        away: 3.5,
        btts_yes: 1.8,
        btts_no: 1.95,
        over25: 1.9,
        under25: 1.85,
    });
    fixture.matches[0].market_count = Some(7);
    let odds_fixture_path = temp.path().join("fixture_a_odds.json");
    std::fs::write(&odds_fixture_path, serde_json::to_vec(&fixture).unwrap()).unwrap();
    FixtureAdapter::ingest(&pool, &store, &odds_fixture_path)
        .await
        .unwrap();
    Canonicalizer::materialize(&pool, &fixture).await.unwrap();

    let quotes: Vec<(String, String, f64, f64)> = sqlx::query_as(
        "SELECT market, outcome, line::float8, price::float8
         FROM fact.odds_quote
         WHERE match_id=$1 AND bookmaker='default' AND phase='closing'
         ORDER BY market, outcome",
    )
    .bind(fixture.matches[0].id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(quotes.len(), 7, "hét kanonikus quote íródik");
    assert_eq!(quotes[0], ("1x2".into(), "away".into(), 0.0, 3.5));
    assert_eq!(quotes[1], ("1x2".into(), "draw".into(), 0.0, 3.2));
    assert_eq!(quotes[2], ("1x2".into(), "home".into(), 0.0, 2.0));
    assert_eq!(quotes[5], ("ou".into(), "over".into(), 2.5, 1.9));
    assert_eq!(quotes[6], ("ou".into(), "under".into(), 2.5, 1.85));

    // Re-materializálás: a blokk cserélődik, nem duplikálódik.
    Canonicalizer::materialize(&pool, &fixture).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.odds_quote")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 7);
}
```

- [x] **Step 6: Futtatás — bukás.** `cargo test -p football-ingest --test canonicalize_fixture ingest_writes_odds_quotes -- --test-threads=1` → FAIL (a writer még a dropolt `odds` oszlopot írja → SQL-hiba).

- [x] **Step 7: Implementáció — writer** (`canonicalize.rs`)

Az importsorba: `use crate::fixture::FixtureOdds;` (ha hiányzik). A fájl végére:

```rust
/// A fixture flat odds → kanonikus quote-sorok: (market, outcome, line, price).
/// `default` bookmaker + `closing` fázis: a demo egyetlen pillanatképe.
fn odds_quotes(odds: &FixtureOdds) -> [(&'static str, &'static str, f64, f64); 7] {
    [
        ("1x2", "home", 0.0, odds.home),
        ("1x2", "draw", 0.0, odds.draw),
        ("1x2", "away", 0.0, odds.away),
        ("btts", "yes", 0.0, odds.btts_yes),
        ("btts", "no", 0.0, odds.btts_no),
        ("ou", "over", 2.5, odds.over25),
        ("ou", "under", 2.5, odds.under25),
    ]
}
```

A meglévő odds-INSERT blokk helyére (a `let Some(odds) = ...` cikluson belül):

```rust
            sqlx::query(
                "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
                 VALUES ($1,$2,$3,$4)
                 ON CONFLICT (match_id, source_id) DO UPDATE SET
                   captured_at = EXCLUDED.captured_at,
                   market_count = EXCLUDED.market_count,
                   trust_state = 'provisional'",
            )
            .bind(fixture_match.id)
            .bind(source_id)
            .bind(fixture.captured_at)
            .bind(market_count)
            .execute(&mut *tx)
            .await?;
            // A forrás quote-blokkja teljesen cserélődik (eltűnt kulcs nem marad).
            sqlx::query("DELETE FROM fact.odds_quote WHERE match_id = $1 AND source_id = $2")
                .bind(fixture_match.id)
                .bind(source_id)
                .execute(&mut *tx)
                .await?;
            for (market, outcome, line, price) in odds_quotes(odds) {
                sqlx::query(
                    "INSERT INTO fact.odds_quote
                     (match_id, source_id, bookmaker, market, outcome, line, phase, price)
                     VALUES ($1,$2,'default',$3,$4,$5,'closing',$6)",
                )
                .bind(fixture_match.id)
                .bind(source_id)
                .bind(market)
                .bind(outcome)
                .bind(line)
                .bind(price)
                .execute(&mut *tx)
                .await?;
            }
```

- [x] **Step 8: Futtatás — zöld.** `cargo test -p football-ingest --test canonicalize_fixture -- --test-threads=1` → PASS (3 teszt, a reingest-teszttel).

- [x] **Step 9: Bukó trust-teszt** — `rust/crates/storage/tests/trust_repos.rs`: a `seed_odds_row`-szerű seedjeit alakítsd fejlécre + quote-okra (a seed helperben az `odds` JSON-oszlop helyett):

```rust
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),3)",
    )
    .bind(match_id).bind(source_id).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','home',0,'closing',2.10),
                ($1,$2,'default','1x2','draw',0,'closing',3.30),
                ($1,$2,'default','1x2','away',0,'closing',3.40),
                ($1,$2,'default','btts','yes',0,'closing',1.80),
                ($1,$2,'default','btts','no',0,'closing',1.95),
                ($1,$2,'default','ou','over',2.5,'closing',1.90),
                ($1,$2,'default','ou','under',2.5,'closing',1.85)",
    )
    .bind(match_id).bind(source_id).execute(pool).await.unwrap();
```

A meglévő elvárások (7 érték, fail-closed hiányzó mező) maradnak; a
hiányzó-mező esethez a seedből hagyj egy kulcsot ki (pl. `btts_no`), és az
elvárás továbbra is hiba.

- [x] **Step 10: Futtatás — bukás.** `cargo test -p football-storage --test trust_repos -- --test-threads=1` → FAIL (a `load_match_odds` még az `o.odds` oszlopot olvassa, ami nem létezik).

- [x] **Step 11: Implementáció — `load_match_odds`** (`trust.rs`), a függvény teljes cseréje:

```rust
pub async fn load_match_odds(pool: &PgPool, match_id: Uuid) -> Result<Vec<SourceOdds>> {
    let rows = sqlx::query(
        "SELECT s.code AS source_code, t.provider,
                (max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='home'))::float8 AS home,
                (max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='draw'))::float8 AS draw,
                (max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='away'))::float8 AS away,
                (max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='yes'))::float8 AS btts_yes,
                (max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='no'))::float8 AS btts_no,
                (max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='over' AND q.line=2.5))::float8 AS over25,
                (max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='under' AND q.line=2.5))::float8 AS under25
         FROM fact.odds_snapshot o
         JOIN ops.source s ON s.id = o.source_id
         LEFT JOIN ops.source_trust t
           ON t.source_id = o.source_id AND t.domain = 'odds'
         JOIN fact.odds_quote q
           ON q.match_id = o.match_id AND q.source_id = o.source_id
          AND q.bookmaker = 'default' AND q.phase = 'closing'
         WHERE o.match_id = $1
         GROUP BY o.source_id, s.code, t.provider
         ORDER BY s.code",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .context("meccs odds lekérés")?;
    rows.iter()
        .map(|row| {
            let field = |k: &str| -> Result<f64> {
                row.try_get::<Option<f64>, _>(k)
                    .with_context(|| format!("odds mező: {k}"))?
                    .with_context(|| format!("hiányzó odds mező: {k}"))
            };
            Ok(SourceOdds {
                source_code: row.try_get("source_code").context("source kód")?,
                provider: row.try_get("provider").context("provider")?,
                odds: OddsValues {
                    home: field("home")?,
                    draw: field("draw")?,
                    away: field("away")?,
                    btts_yes: field("btts_yes")?,
                    btts_no: field("btts_no")?,
                    over25: field("over25")?,
                    under25: field("under25")?,
                },
            })
        })
        .collect()
}
```

- [x] **Step 12: Futtatás — zöld.** `cargo test -p football-storage --test trust_repos -- --test-threads=1` → PASS.

- [x] **Step 13: Bukó display-teszt** — `rust/crates/storage/tests/day_query.rs`:

`day_row_includes_display_fields` INSERT-je helyére:

```rust
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),18)",
    )
    .bind(match_id).bind(source_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','home',0,'closing',2.2),
                ($1,$2,'default','1x2','draw',0,'closing',3.3),
                ($1,$2,'default','1x2','away',0,'closing',3.2)",
    )
    .bind(match_id).bind(source_id).execute(&pool).await.unwrap();
```

`day_picks_verified_odds_row` INSERT-je helyére:

```rust
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, trust_state)
         VALUES ($1,$2,now(),18,'provisional'), ($1,$3,now(),18,'verified')",
    )
    .bind(match_id).bind(provisional_source).bind(verified_source)
    .execute(&pool).await.unwrap();
    for (source_id, home) in [(provisional_source, 9.9), (verified_source, 1.5)] {
        sqlx::query(
            "INSERT INTO fact.odds_quote
             (match_id, source_id, bookmaker, market, outcome, line, phase, price)
             VALUES ($1,$2,'default','1x2','home',0,'closing',$3),
                    ($1,$2,'default','1x2','draw',0,'closing',3.3),
                    ($1,$2,'default','1x2','away',0,'closing',3.2)",
        )
        .bind(match_id).bind(source_id).bind(home)
        .execute(&pool).await.unwrap();
    }
```

Az elvárások változatlanok (2.2; illetve verified nyer, home 1.5).
`rust/crates/ingest/tests/display_materialize.rs` 38. sora:

```rust
    let (home, market_count): (String, i32) = sqlx::query_as(
        "SELECT (max(price) FILTER (WHERE market='1x2' AND outcome='home'))::float8::text,
                o.market_count
         FROM fact.odds_snapshot o
         JOIN fact.odds_quote q ON q.match_id=o.match_id AND q.source_id=o.source_id
          AND q.bookmaker='default' AND q.phase='closing'
         WHERE o.match_id='30000000-0000-7000-8000-000000000101'
         GROUP BY o.market_count",
    )
```

`rust/crates/storage/tests/trust_schema.rs`: a `seed_odds_row` fejléc-INSERT-je
`(match_id, source_id, captured_at, market_count)` legyen `odds` nélkül
(a PK/CHECK-állítások változatlanok).

- [x] **Step 14: Futtatás — bukás.** `cargo test -p football-storage --test day_query -- --test-threads=1` → FAIL (a day-LATERAL még `o.odds`-ot szelektál).

- [x] **Step 15: Implementáció — day.rs** LATERAL (a `LEFT JOIN LATERAL (...)` teljes cseréje):

```sql
         LEFT JOIN LATERAL (
             SELECT jsonb_build_object(
                        'home', max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='home'),
                        'draw', max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='draw'),
                        'away', max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='away'),
                        'btts_yes', max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='yes'),
                        'btts_no', max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='no'),
                        'over25', max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='over' AND q.line=2.5),
                        'under25', max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='under' AND q.line=2.5)
                    ) AS odds,
                    o.market_count, o.source_id, o.captured_at, o.trust_state
             FROM fact.odds_snapshot o
             JOIN fact.odds_quote q
               ON q.match_id = o.match_id AND q.source_id = o.source_id
              AND q.bookmaker = 'default' AND q.phase = 'closing'
             WHERE o.match_id = m.id
             GROUP BY o.market_count, o.source_id, o.captured_at, o.trust_state
             ORDER BY (o.trust_state = 'verified') DESC, o.source_id
             LIMIT 1
         ) o ON true
```

- [x] **Step 16: Futtatás — zöld + regresszió.** `cargo test -p football-storage -- --test-threads=1 && cargo test -p football-ingest -- --test-threads=1` → PASS.

- [x] **Step 17: Teljes kapu.** `cd .. && ./scripts/check.sh` → exit 0, minden teszt zöld (157+), fmt/clippy tiszta, teszt DB reset a 0008-cal; ellenőrzés: teszt DB 35 tábla + `_sqlx_migrations` max version 8 (psql). A teljes `db-check.sh` (dev 35) a Task 2-ben fut, mert a dev DB még 0007-en van. Megjegyzés: a tervből kimaradt két további seed-hely (storage/tests/connection.rs táblaszám, api/tests/routes.rs odds-insert) — a kapu kimutatta, javítva.

- [x] **Step 18: STOP — jelentés.** Commit NEM készül.

---

### Task 2: Dev migráció + demo validáció + checkpoint (zárás)

**Files:**
- Modify: `docs/checkpoints/2026-11-14/checkpoint.md` (append-only)
- Nincs kódváltozás.

**Interfaces:**
- Consumes: Task 1 migráció + pipeline; a dev DB meglévő 20 odds-sora.
- Produces: dev DB a 0008-cal (35 tábla), backfill-igazolt quote-szám, demo újratöltve, checkpoint-zárás.

- [x] **Step 1: Engedélykérés.** A dev DB migráció és a demo újratöltés **csak felhasználói engedéllyel**; a felhasználó nevezze meg a műveletet. Ha nincs engedély: STOP, a task függőben marad.

- [x] **Step 2: Dev migráció.** `cd rust && set -a; source ../.env; set +a; cargo run -q -p footballctl -- migrate` → „migráció kész”.

- [x] **Step 3: Backfill-ellenőrzés.** A migráció előtti 20 odds-sor → 20 fejléc × 7 quote:

```bash
psql "$DATABASE_URL" -tAc "SELECT count(*) FROM fact.odds_snapshot"
psql "$DATABASE_URL" -tAc "SELECT count(*) FROM fact.odds_quote"
psql "$DATABASE_URL" -tAc "SELECT count(*) FROM (SELECT match_id, source_id, count(*) c
  FROM fact.odds_quote GROUP BY 1,2) s WHERE c = 7"
```

Elvárás: 20 / 140 / 20.

- [x] **Step 4: Demo újratöltés** (a dokumentált, visszajátszható út):

```bash
for f in ../tests/fixtures/demo_day_*.json; do
  cargo run -q -p footballctl -- ingest-fixture --path "$f"
done
cargo run -q -p footballctl -- day --date 2026-11-11
```

Elvárás: a day-kimenet 20 meccs, mind `verified_publish` + `odds_trust_state: verified`; a quote-szám nem duplikálódik (40 header × 7 = 280).

- [x] **Step 5: DB-kapu.** Az utolsó tesztfutás maradékai miatt a test DB resetelendő (a felhasználó nevezze meg a `db-reset.sh`-t; csak `TEST_DATABASE_URL`-t érint): `set -a; source .env; set +a; ./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 35 tábla (üzleti sorok a demo miatt), test 35 tábla, 0 üzleti sor.

- [x] **Step 6: Checkpoint (append-only).** A checkpoint végére: S2d-1 taskok + futtatott parancsok/kimenetek, backfill-bizonyíték, deferred minorok (ha vannak), visszaállítási út (0008 inverz SQL), következő döntési pont (A csoport vagy S3). Kész: 40 fejléc → 280 quote; dev 35/1101/6, test 35/0/0; day 20/20 verified.

- [x] **Step 7: STOP — zárójelentés.** Commit csak kifejezett kérésre; push nem történik.

---

## Önreview-jegyzet (a terv írójának)

- **Spec-lefedettség:** §2.1 odds_quote → Task 1 Step 3; §2.2 backfill + drop → Step 3; §2.3 match_stats → Step 3 + Step 1 (constraint); §2.4 referee/venue → Step 3 + Step 1; §2.5 db-check 35 → Step 3/17; §3 pipeline → Step 7/11/15; §4 tesztek → Step 1/5/9/13/17; §6 rollback → Task 2 Step 6 (inverz SQL a migráció fejlécében).
- **Típus-konzisztencia:** `seed_min_match` → `(Uuid, Uuid)` (Step 1 használja, Step 3 termeli); `odds_quotes` → `[(&'static str, &'static str, f64, f64); 7]`; a quote-konvenció mindenhol `bookmaker='default'`, `phase='closing'`, `ou` line 2.5.
- **Ismert egyszerűsítés:** a Task 1 atomi (a drop miatt) — a lépések bontottak, a STOP a végén; a Task 2 kizárólag dev-validáció + doksi.
