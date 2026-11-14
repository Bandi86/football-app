# S2d-2 — Meccs-mikro séma: implementációs terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (subagent-típusok nem elérhetők ebben a sessionben → inline execution). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Öt meccs-szintű mikro-adat tábla (match_team_stats, match_player_stats, match_lineups, match_shots, match_incidents) a FotMob/Livescore adapterekhez (S3), writer nélkül, constraint-tesztekkel.

**Architecture:** Egy 0009 migráció; közös per-source minta (match FK cascade, source FK, captured_at); dupla játékos-hivatkozás (`source_player_id` + nullable `player_entity_id`); zárt dimenziók CHECK-kel (side/period/role, sorszámok), nyílt taxonómiák text; a pipeline változatlan.

**Tech Stack:** Rust (edition 2024, rust-version 1.85), sqlx 0.8, PostgreSQL.

**Spec:** `docs/superpowers/specs/2026-11-14-s2d2-match-micro-design.md`

## Global Constraints

- AGENTS.md §2: **minden task végén STOP** — összefoglaló + bizonyíték, következő task csak felhasználói engedéllyel.
- **Commit: zöld task után** (felhasználói utasítás, 2026-11-14) — csak a kapuk zöld állapotában; üzenet a repó stílusában; `git -c user.name='Bandi86' -c user.email='Bandi86@users.noreply.github.com' commit`; push nem történik.
- Munkaág: `feat/s2d2-match-micro` a `feat/s2d1-schema-gaps` csúcsáról (06a04c3); a létrehozás a végrehajtás első lépése. Nincs worktree.
- Dokumentumok/hibaüzenetek **magyarul**; kódazonosítók angolul. Magyar szöveg Write/Edit után visszaolvasandó.
- Tesztek: `TEST_DATABASE_URL` kötelező; integrációs tesztek egy szálon; hivatalos kapu `cargo test --workspace -- --test-threads=1` (a `./scripts/check.sh` resetel).
- Migráció: teszt DB-n (check.sh) → zöld → **dev DB csak a Task 2-ben, külön engedéllyel**.
- **0 élő kérés**, 0 nyers payload; a demo pipeline nem változik.
- Táblaszám: 0009 után **40** (dev + test).

## Fájlstruktúra

| Fájl | Felelősség |
|---|---|
| `db/migrations/0009_s2d2_match_micro.sql` (új) | az öt új tábla |
| `rust/crates/storage/tests/s2d2_match_micro_schema.rs` (új) | constraint/kaszkád tesztek |
| `rust/crates/storage/src/test_support.rs` (módosul) | TRUNCATE-lista +5 tábla |
| `rust/crates/storage/tests/connection.rs` (módosul) | 35 → 40 tábla |
| `scripts/db-check.sh` (módosul) | elvárt táblaszám 35 → 40 |
| `docs/checkpoints/2026-11-14/checkpoint.md` (módosul, Task 2) | záró szekció |

---

### Task 1: 0009 migráció + constraint-tesztek + kapuk + commit

**Files:**
- Create: `db/migrations/0009_s2d2_match_micro.sql`
- Test: `rust/crates/storage/tests/s2d2_match_micro_schema.rs` (új)
- Modify: `rust/crates/storage/src/test_support.rs`, `rust/crates/storage/tests/connection.rs`, `scripts/db-check.sh`

**Interfaces:**
- Consumes: `test_support::seed_min_match(pool) -> (Uuid, Uuid)`.
- Produces: az öt tábla sémája; db-check 40; a constraint-tesztek.

- [x] **Step 1: Bukó séma-teszt (új fájl)**

`rust/crates/storage/tests/s2d2_match_micro_schema.rs`:

```rust
use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_player_entity(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'player','Szoboszlai')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn insert_team_stat(pool: &PgPool, match_id: Uuid, source_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO fact.match_team_stats
         (match_id, source_id, captured_at, side, period, stat_key, value)
         VALUES ($1,$2,now(),'home','all','possession',55)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(pool)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn team_stats_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;
    insert_team_stat(&pool, match_id, source_id).await.unwrap();

    // PK-duplikátum (23505).
    match insert_team_stat(&pool, match_id, source_id).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }

    // side CHECK (23514).
    let bad_side = sqlx::query(
        "INSERT INTO fact.match_team_stats
         (match_id, source_id, captured_at, side, period, stat_key, value)
         VALUES ($1,$2,now(),'neutral','all','possession',55)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_side {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (side), ehelyett: {other:?}"),
    }

    // period CHECK (23514).
    let bad_period = sqlx::query(
        "INSERT INTO fact.match_team_stats
         (match_id, source_id, captured_at, side, period, stat_key, value)
         VALUES ($1,$2,now(),'home','extra_time','possession',55)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_period {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (period), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn player_stats_and_lineup_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;
    let player = seed_player_entity(&pool).await;

    sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, player_entity_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-1',$3,'home','rating',7.4)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(player)
    .execute(&pool)
    .await
    .unwrap();
    // PK-duplikátum.
    let dup = sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-1','home','rating',7.0)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match dup {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }
    // Ismeretlen entitás-FK (23503).
    let bad_entity = sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, player_entity_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-2',$3,'home','rating',7.0)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await;
    match bad_entity {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23503")),
        other => panic!("FK-hibát vártunk, ehelyett: {other:?}"),
    }

    sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, player_entity_id,
          role, position, shirt_number, rating)
         VALUES ($1,$2,now(),'home','p-1',$3,'starter','CM',8,7.4)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(player)
    .execute(&pool)
    .await
    .unwrap();
    // role CHECK (23514).
    let bad_role = sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, role)
         VALUES ($1,$2,now(),'home','p-2','reserve')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_role {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (role), ehelyett: {other:?}"),
    }
    // shirt_number CHECK (23514).
    let bad_shirt = sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, role, shirt_number)
         VALUES ($1,$2,now(),'home','p-3','sub',0)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_shirt {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (shirt_number), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn shots_and_incidents_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    sqlx::query(
        "INSERT INTO fact.match_shots
         (match_id, source_id, captured_at, shot_no, side, source_player_id, minute, x, y, xg, xgot, shot_type)
         VALUES ($1,$2,now(),1,'home','p-1',23,84.5,42.1,0.31,0.18,'header')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    // shot_no > 0 CHECK (23514).
    let bad_shot = sqlx::query(
        "INSERT INTO fact.match_shots (match_id, source_id, captured_at, shot_no, side)
         VALUES ($1,$2,now(),0,'home')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_shot {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (shot_no), ehelyett: {other:?}"),
    }

    sqlx::query(
        "INSERT INTO fact.match_incidents
         (match_id, source_id, captured_at, incident_no, side, incident_type,
          player_source_id, minute, detail)
         VALUES ($1,$2,now(),1,'home','goal','p-1',23,'bal láb, tizenhatoson belül')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    // incident_no > 0 CHECK (23514).
    let bad_incident = sqlx::query(
        "INSERT INTO fact.match_incidents
         (match_id, source_id, captured_at, incident_no, side, incident_type)
         VALUES ($1,$2,now(),0,'home','goal')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_incident {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (incident_no), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn match_delete_cascades_all_micro_tables() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    insert_team_stat(&pool, match_id, source_id).await.unwrap();
    sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-1','home','rating',7.4)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, role)
         VALUES ($1,$2,now(),'home','p-1','starter')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.match_shots (match_id, source_id, captured_at, shot_no, side)
         VALUES ($1,$2,now(),1,'home')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.match_incidents
         (match_id, source_id, captured_at, incident_no, side, incident_type)
         VALUES ($1,$2,now(),1,'home','goal')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("DELETE FROM core.match WHERE id=$1")
        .bind(match_id)
        .execute(&pool)
        .await
        .unwrap();
    for table in [
        "fact.match_team_stats",
        "fact.match_player_stats",
        "fact.match_lineups",
        "fact.match_shots",
        "fact.match_incidents",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} kaszkádol a meccs-törlésre");
    }
}
```

- [x] **Step 2: Futtatás — bukás.** `set -a; source ../.env; set +a; cd rust && cargo test -p football-storage --test s2d2_match_micro_schema -- --test-threads=1` → FAIL (a táblák nem léteznek).

- [x] **Step 3: Implementáció — migráció**

`db/migrations/0009_s2d2_match_micro.sql`:

```sql
-- 0009 — S2d-2 meccs-mikro séma: team_stats, player_stats, lineups,
-- shots, incidents. Visszaállítás: az öt tábla dropja.
BEGIN;

CREATE TABLE fact.match_team_stats (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    side text NOT NULL CHECK (side IN ('home','away')),
    period text NOT NULL CHECK (period IN ('all','first_half','second_half')),
    stat_key text NOT NULL,
    value numeric,
    PRIMARY KEY (match_id, source_id, side, period, stat_key)
);

CREATE INDEX idx_match_team_stats_match_side
    ON fact.match_team_stats (match_id, side);

CREATE TABLE fact.match_player_stats (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    source_player_id text NOT NULL,
    player_entity_id uuid REFERENCES core.entity(id),
    side text NOT NULL CHECK (side IN ('home','away')),
    stat_key text NOT NULL,
    value numeric,
    PRIMARY KEY (match_id, source_id, source_player_id, stat_key)
);

CREATE INDEX idx_match_player_stats_player
    ON fact.match_player_stats (source_player_id);

CREATE TABLE fact.match_lineups (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    side text NOT NULL CHECK (side IN ('home','away')),
    source_player_id text NOT NULL,
    player_entity_id uuid REFERENCES core.entity(id),
    role text NOT NULL CHECK (role IN ('starter','sub')),
    position text,
    shirt_number integer CHECK (shirt_number > 0),
    rating numeric(3,1),
    PRIMARY KEY (match_id, source_id, side, source_player_id)
);

CREATE INDEX idx_match_lineups_match_side ON fact.match_lineups (match_id, side);
CREATE INDEX idx_match_lineups_player ON fact.match_lineups (source_player_id);

CREATE TABLE fact.match_shots (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    shot_no integer NOT NULL CHECK (shot_no > 0),
    side text NOT NULL CHECK (side IN ('home','away')),
    source_player_id text,
    player_entity_id uuid REFERENCES core.entity(id),
    minute integer,
    x numeric,
    y numeric,
    xg numeric,
    xgot numeric,
    shot_type text,
    PRIMARY KEY (match_id, source_id, shot_no)
);

CREATE INDEX idx_match_shots_match ON fact.match_shots (match_id);

CREATE TABLE fact.match_incidents (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    incident_no integer NOT NULL CHECK (incident_no > 0),
    side text NOT NULL CHECK (side IN ('home','away')),
    incident_type text NOT NULL,
    player_source_id text,
    player_entity_id uuid REFERENCES core.entity(id),
    related_player_source_id text,
    related_player_entity_id uuid REFERENCES core.entity(id),
    minute integer,
    detail text,
    PRIMARY KEY (match_id, source_id, incident_no)
);

CREATE INDEX idx_match_incidents_match ON fact.match_incidents (match_id);

COMMIT;
```

- [x] **Step 4: Futtatás — zöld.** `cargo test -p football-storage --test s2d2_match_micro_schema -- --test-threads=1` → PASS (4 teszt).

- [x] **Step 5: Kapuk frissítése**

`rust/crates/storage/src/test_support.rs` TRUNCATE-lista (a `fact.odds_quote` elé):

```
fact.match_team_stats, fact.match_player_stats, fact.match_lineups,
fact.match_shots, fact.match_incidents,
```

`scripts/db-check.sh`: `expected_tables=40` + komment („0001–0009 migrációk
után 40 tábla; a 0009 öt új táblát ad").

`rust/crates/storage/tests/connection.rs`: 35 → 40
(„a 0001–0009 migráció után 39 terméktábla és a _sqlx_migrations várható").

- [x] **Step 6: Futtatás — zöld.** `cargo test -p football-storage --test connection -- --test-threads=1` → PASS (40 tábla).

- [x] **Step 7: Teljes kapu.** `cd .. && ./scripts/check.sh` → exit 0, minden teszt zöld (165+), fmt/clippy tiszta.

- [x] **Step 8: Commit.** (A commit a spec+terv dokumentumokat is tartalmazza.)

```bash
git add db/migrations/0009_s2d2_match_micro.sql \
  rust/crates/storage/tests/s2d2_match_micro_schema.rs \
  rust/crates/storage/src/test_support.rs \
  rust/crates/storage/tests/connection.rs scripts/db-check.sh
git -c user.name='Bandi86' -c user.email='Bandi86@users.noreply.github.com' \
  commit -m "feat: add S2d-2 match micro schema tables" \
  -m "Team/player stats, lineups, shots and incidents with per-source rows,
dual player references (source id + nullable entity FK), CHECKed closed
dimensions and open text taxonomies; db-check expected tables now 40."
```

- [x] **Step 9: STOP — jelentés.**

---

### Task 2: Dev migráció + checkpoint + zárás

**Files:**
- Modify: `docs/checkpoints/2026-11-14/checkpoint.md` (append-only)

**Interfaces:**
- Consumes: Task 1 migráció; a dev DB 35 táblája (S2d-1 után).
- Produces: dev 40 tábla; checkpoint-zárás; commit.

- [x] **Step 1: Engedélykérés.** A dev DB migráció **csak felhasználói engedéllyel**; ha nincs, STOP, a task függőben marad.

- [x] **Step 2: Dev migráció.** `cd rust && set -a; source ../.env; set +a; cargo run -q -p footballctl -- migrate` → „migráció kész"; 40 tábla ellenőrzés.

- [x] **Step 3: DB-kapu.** A test DB resetje (a felhasználó nevezze meg; csak `TEST_DATABASE_URL`), majd `cd .. && ./scripts/db-check.sh` → dev 40, test 40/0/0.

- [x] **Step 4: Checkpoint (append-only).** S2d-2 taskok + parancsok/kimenetek, deferred minorok, visszaállítási út (öt tábla drop), következő döntési pont (B/E/F/G csoportok vagy S3).

- [x] **Step 5: Commit.**

```bash
git add docs/checkpoints/2026-11-14/checkpoint.md
git -c user.name='Bandi86' -c user.email='Bandi86@users.noreply.github.com' \
  commit -m "docs: close S2d-2 checkpoint (match micro schema)"
```

- [x] **Step 6: STOP — zárójelentés.**

---

## Önreview-jegyzet (a terv írójának)

- **Spec-lefedettség:** §2.1–2.5 táblák → Task 1 Step 3; §2.6 db-kapu → Step 5; §4 tesztek → Step 1; §6 rollback → Task 2 Step 4 (drop); §3 határok → a pipeline nem változik (nincs kódváltozás a canonicalize/trust/day-ben).
- **Típus-konzisztencia:** az öt tábla neve és oszlopai a spec szerint; `seed_min_match` újrahasznált; CHECK-kódok (23505/23514/23503) a meglévő tesztek mintájára.
- **Ismert egyszerűsítés:** a táblák író nélkül jönnek létre — tudatos (S3-előkészítés); a demo pipeline érintetlen, ezért nem fut újra.
