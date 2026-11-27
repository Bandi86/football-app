# O0 — Raw capture/provenance invariant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Minden hálózati fetch és ismételt import saját `raw.capture` sort kapjon, a változott observation pedig mindig a ténylegesen őt előállító capture-hez kötődjön, adatbázis-szintű árva sor nélkül.

**Architecture:** A `raw.capture` már nem tekinti a stabil `request_key`-t capture-identitásnak. A `persist_raw` forrás- és artifact-upsert után mindig új capture-t szúr be, és a ténylegesen beszúrt UUID-t adja vissza `RETURNING id`-vel. A `stage.observation` tartalomdeduplikációja megmarad: bájtazonos ismétlés új capture-t hoz, de nem duplikálja a meglévő observationt; változott tartalom új observationt hoz az új capture alatt.

**Tech Stack:** Rust 2024, sqlx/PostgreSQL, tokio integrációs tesztek, meglévő `ObjectStore`, chrono, UUIDv7; nem adunk hozzá új függőséget.

**Spec:** `docs/superpowers/specs/2026-11-27-uzemeltetes-triage-console-design.md`, különösen §8.1 és az O0 végrehajtási szelet.

## Global Constraints

- A repó publikus: titok, API-kulcs, cookie, token, adatbázis-jelszó és személyes adat nem kerülhet bele.
- Csak a helyi `127.0.0.1` API hallgathat; a kliens nem kap közvetlen adatbázis-hozzáférést.
- Forráscookie és hitelesítő adat a macOS Keychainben él, nem fájlban és nem naplóban.
- Nyers scrape-elt bemenet méret-, idő- és sémaellenőrzést kap.
- Robots/policy, rate limit és forrásvédelem megkerülése tilos.
- Valódi pénzes fogadás, fogadóiroda-integráció és automatikus kifizetés nem része a rendszernek.
- Automatikus fuzzy entitás-összevonás tilos.
- A `football_v2_dev` és `football_v2_test` fejlesztési adatbázis; a v1 adatbázisához nem nyúlunk.
- A tesztadatbázisra először minden migrációt le kell futtatni, és csak zöld ellenőrzés után kerülhet a dev adatbázisra.
- Valódi forrásadat, nyers payload vagy meccsadat csak külön, explicit engedélyezett fázisban tölthető be.
- A hiányzó adat `NULL`/státusz, soha nem nulla.
- Minden megjelenített tényhez forrás, rögzítési idő és ellenőrzési állapot tartozik.
- Minden időpont UTC; a helyi megjelenítés külön időzónakezelést kap.
- Minden analitika as-of; jövőbeli információ nem szivároghat vissza.
- A storage-tesztek közös tesztadatbázist használnak, ezért `--test-threads=1` kötelező.
- A kód és a dokumentáció magyar üzeneteket használjon; a kódazonosítók angolul maradnak.
- Builder ≠ verifier: a végső kaput és a diffet független ellenőr is átnézi.
- A futó daemon telepítése és a dev adatbázis migrációja külön, koordinált engedélyezési kapu; ezt a terv nem hajtja végre automatikusan.

---

## File Structure

| Fájl | Felelősség a szeletben |
|---|---|
| `db/migrations/0032_raw_capture_history.sql` | Eltávolítja a `(source_id, request_key)` unique constraintot, és indexet ad a legfrissebb capture lekérdezéséhez. |
| `rust/crates/storage/src/repositories/worker.rs` | `persist_raw` minden hívásra új capture-t hoz, és a beszúrt capture UUID-jét adja vissza. |
| `rust/crates/storage/tests/worker_repos.rs` | Regressziós teszt az ismétlődő `request_key`-re, az artifact-deduplikációra és a tranzakciós rollbackra. |
| `rust/crates/ingest/src/fixture.rs` | A fixture raw írása a közös `persist_raw` repository-útra kerül. |
| `rust/crates/ingest/tests/capture_observation.rs` | Kétszeres fixture-ingest: két capture, egy artifact, három deduplikált observation. |
| `rust/crates/worker/tests/tick_e2e.rs` | Két valódi tick ugyanazzal a `request_key`-vel, változó tartalommal; capture-ownership és árvaság-ellenőrzés. |
| `rust/crates/storage/src/repositories/manual.rs` | A kézi szerkesztés továbbra is idempotens marad, de a beszúrt capture azonosítóját `RETURNING id`-vel kapja meg. |
| `rust/crates/api/tests/match_edit.rs` | Regressziós bizonyíték, hogy az explicit idempotencia-key ismétlése nem hoz új manual capture-t. |
| `docs/checkpoints/2026-11-27/checkpoint.md` | A szelet valós futattott kapuit, mért számait, kockázatát és visszaállását rögzíti. |

## Döntések, amelyeket a végrehajtás kötően követ

1. **Fetch és import:** minden `persist_raw` hívás új `raw.capture`; a `request_key` stabil kérésidentitás, nem capture-identitás.
2. **Artifact:** a `content_sha256` unique és a tartalomcímzett objektumtár megmarad; azonos bytes nem hoz második artifact sort vagy fizikai fájlt.
3. **Observation:** a meglévő `(source_id, entity_kind, aspect, source_record_key, content_hash)` unique megmarad. Egy ismétlődő, tartalomazonos observation nem kap új sort; a régi observation továbbra is a régi capture-hez mutat. Egy változott observation új sort kap, és annak `capture_id` mezője az új capture.
4. **Kézi szerkesztés:** nem hálózati fetch. Egy explicit vagy kanonikusan számított idempotencia-key továbbra is egy capture-et, egy observationt és egy auditot jelent. A `core.match` sor `FOR UPDATE` zárolása sorosítja az ismétlést; a capture unique constraintjára az idempotencia többé nem támaszkodik.
5. **Tranzakció:** forrás, artifact és capture ugyanabban a DB-tranzakcióban commitol. Hibás capture-beszúrás után nem marad DB-szintű árva forrás, artifact vagy capture.
6. **Fizikai objektumtár:** a fájl a DB-tranzakció előtt jön létre. DB-rollback esetén fizikai, még nem hivatkozott fájl maradhat; az O0 nem töröl automatikusan fájlokat. Ez a korlátozás a checkpointben nyitott kockázatként szerepel.
7. **Éles átállás:** a dev DB és a futó daemon nem változik ebben a tervben. A régi daemon SQL-je a régi unique constraintra támaszkodik, ezért a schema és a bináris átállítása csak közös, explicit engedélyezett műveletként végezhető.

---

### Task 1: Piros regressziós tesztek a capture-életciklusra

**Files:**
- Modify: `rust/crates/storage/tests/worker_repos.rs:723-793`
- Modify: `rust/crates/ingest/tests/capture_observation.rs:5-32`
- Modify: `rust/crates/worker/tests/tick_e2e.rs` a meglévő helper-ek és importok használatával
- Modify: `rust/crates/api/tests/match_edit.rs:244-299`

**Interfaces:**
- Consumes: a meglévő `repo::persist_raw`, `RawCapture`, `FixtureAdapter::ingest`, `tick::tick`, `FixtureFetcher`, `test_pool` és `DB_LOCK` API-k.
- Produces: három piros O0-regresszió és egy zöld manual-idempotencia regresszió; a későbbi implementációnak ezeket kell zöldre vinnie.

- [ ] **Step 1: Cseréld le a régi capture-idempotencia tesztet**

A `persist_raw_upserts_source_artifact_capture_idempotently` tesztet egészében cseréld a következőre. A teszt most már azt bizonyítja, hogy a forrás és az artifact deduplikálódik, a capture nem.

```rust
#[tokio::test]
async fn persist_raw_creates_capture_per_fetch_and_deduplicates_artifact() {
    let pool = test_pool().await;
    let artifact = stored_artifact('a');
    let capture = raw_capture("demo-synthetic:match:2026-11-12");

    let (source_id, capture_id) = repo::persist_raw(
        &pool,
        "demo-synthetic",
        "Demó forrás",
        &artifact,
        &capture,
        "application/json",
    )
    .await
    .unwrap();

    let (source_id2, capture_id2) = repo::persist_raw(
        &pool,
        "demo-synthetic",
        "Demó forrás v2",
        &artifact,
        &capture,
        "application/json",
    )
    .await
    .unwrap();
    assert_eq!(source_id, source_id2);
    assert_ne!(capture_id, capture_id2);

    let (artifacts, captures, sources): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM raw.artifact),
                (SELECT count(*) FROM raw.capture),
                (SELECT count(*) FROM ops.source)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((artifacts, captures, sources), (1, 2, 1));

    let display_name: String =
        sqlx::query_scalar("SELECT display_name FROM ops.source WHERE id=$1")
            .bind(source_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(display_name, "Demó forrás v2");

    let other = raw_capture("demo-synthetic:match:2026-11-13");
    let (_, capture_id3) = repo::persist_raw(
        &pool,
        "demo-synthetic",
        "Demó forrás v2",
        &artifact,
        &other,
        "application/json",
    )
    .await
    .unwrap();
    assert_ne!(capture_id, capture_id3);
    assert_ne!(capture_id2, capture_id3);

    let (artifacts, captures): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM raw.artifact), (SELECT count(*) FROM raw.capture)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((artifacts, captures), (1, 3));
}
```

- [ ] **Step 2: Illeszd be a capture-rollback tesztet**

Illeszd be a fenti teszt után:

```rust
#[tokio::test]
async fn persist_raw_rolls_back_source_and_artifact_when_capture_fails() {
    let pool = test_pool().await;
    let artifact = stored_artifact('b');
    let mut capture = raw_capture("demo-synthetic:match:rollback");
    capture.http_status = 600;

    let error = repo::persist_raw(
        &pool,
        "demo-synthetic",
        "Demó forrás",
        &artifact,
        &capture,
        "application/json",
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("capture beszúrás"), "{error:#}");

    let (artifacts, captures, sources): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM raw.artifact),
                (SELECT count(*) FROM raw.capture),
                (SELECT count(*) FROM ops.source)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((artifacts, captures, sources), (0, 0, 0));
}
```

- [ ] **Step 3: Bővítsd a fixture-ingest tesztet ismétlődésre**

Cseréld le a `capture_observation.rs` tesztjét:

```rust
use football_ingest::fixture::FixtureAdapter;
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn fixture_is_stored_raw_and_observed_once() {
    let pool = test_pool().await;
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");

    let first = FixtureAdapter::ingest(&pool, &store, &path).await.unwrap();
    let second = FixtureAdapter::ingest(&pool, &store, &path).await.unwrap();

    let artifacts: i64 = sqlx::query_scalar("SELECT count(*) FROM raw.artifact")
        .fetch_one(&pool)
        .await
        .unwrap();
    let captures: i64 = sqlx::query_scalar("SELECT count(*) FROM raw.capture")
        .fetch_one(&pool)
        .await
        .unwrap();
    let observations: i64 = sqlx::query_scalar("SELECT count(*) FROM stage.observation")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(artifacts, 1);
    assert_eq!(captures, 2);
    assert_eq!(observations, 3);
    assert_ne!(first.capture_id, second.capture_id);
    assert_eq!(first.observation_ids, second.observation_ids);
}
```

- [ ] **Step 4: Illeszd be a worker E2E capture-ownership tesztet**

Illeszd be a `tick_e2e.rs` végére, a meglévő importokat és helper-eket használva:

```rust
#[tokio::test]
async fn repeated_fetch_with_same_request_key_keeps_per_capture_provenance() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let path = write_rewritten_fixture("demo_day.json", now, fetch_dir.path());
    let request_key = format!("demo-synthetic:match:{}", now.date_naive());
    let fetchers = as_fetchers(FixtureFetcher::new(HashMap::from([(
        request_key.clone(),
        path.clone(),
    )])));
    seed_and_generate(&pool, SourceMode::Shadow, now).await;

    let first = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(first.processed, 1);
    assert_eq!(first.failed, 0);

    let mut fixture: FixtureSource =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fixture.matches[0].round_label = Some("2. forduló".into());
    std::fs::write(&path, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();

    let later = now + Duration::minutes(1);
    sqlx::query(
        "UPDATE ops.job
            SET status='pending', due_at=$1, leased_until=NULL, worker_id=NULL,
                attempts=0, last_error=NULL
          WHERE source_slug='demo-synthetic' AND status='done'",
    )
    .bind(later)
    .execute(&pool)
    .await
    .unwrap();

    let second = tick::tick(
        &pool,
        &store,
        &fetchers,
        &adapters(),
        Uuid::now_v7(),
        later,
    )
    .await
    .unwrap();
    assert_eq!(second.processed, 1);
    assert_eq!(second.failed, 0);

    let captures: Vec<(Uuid, String, String, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT c.id, a.content_sha256, c.request_key, c.requested_at
           FROM raw.capture c
           JOIN raw.artifact a ON a.id=c.artifact_id
          ORDER BY c.requested_at, c.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(captures.len(), 2);
    assert_eq!(captures[0].2, request_key);
    assert_eq!(captures[1].2, request_key);
    assert_ne!(captures[0].1, captures[1].1);
    assert_ne!(captures[0].3, captures[1].3);

    let distinct_observation_captures: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT capture_id) FROM stage.observation",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(distinct_observation_captures, 2);

    let changed_capture_id: Uuid = sqlx::query_scalar(
        "SELECT capture_id FROM stage.observation
          WHERE aspect='match_core' AND source_record_key='DEMO-1001'
            AND normalized_payload->>'round_label'='2. forduló'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(changed_capture_id, captures[1].0);

    let (orphan_artifacts, orphan_captures, orphan_observations): (i64, i64, i64) =
        sqlx::query_as(
            "SELECT
               (SELECT count(*) FROM raw.artifact a
                 WHERE NOT EXISTS (SELECT 1 FROM raw.capture c WHERE c.artifact_id=a.id)),
               (SELECT count(*) FROM raw.capture c
                 WHERE NOT EXISTS (SELECT 1 FROM raw.artifact a WHERE a.id=c.artifact_id)),
               (SELECT count(*) FROM stage.observation o
                 WHERE NOT EXISTS (SELECT 1 FROM raw.capture c WHERE c.id=o.capture_id))",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((orphan_artifacts, orphan_captures, orphan_observations), (0, 0, 0));
}
```

- [ ] **Step 5: Őrizd meg a manual idempotenciát explicit capture-számossal**

Az `edit_is_idempotent_by_explicit_key_and_canonical_payload` tesztben az explicit key második hívása után, az `assert_eq!(audits, 1, ...)` sor után illeszd be:

```rust
    let manual_captures: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM raw.capture c
           JOIN ops.source s ON s.id=c.source_id
          WHERE s.code='manual-ui' AND c.endpoint_key='manual-match-edit'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(manual_captures, 1, "azonos kulcs: egy manual capture");
```

- [ ] **Step 6: Formázd és futtasd a célzott tesztet**

```bash
set -a && source .env && set +a
./scripts/db-reset.sh
cd rust
cargo fmt --all
cargo test -p football-storage --test worker_repos persist_raw_ -- --test-threads=1
cargo test -p football-ingest --test capture_observation -- --test-threads=1
cargo test -p football-worker --test tick_e2e repeated_fetch_with_same_request_key_keeps_per_capture_provenance -- --test-threads=1
cargo test -p football-api --test match_edit edit_is_idempotent_by_explicit_key_and_canonical_payload -- --test-threads=1
```

**Elvárt piros bizonyíték:** a storage capture-tesztek és a worker E2E elbukik, mert a második `request_key` nem hoz új capture-t; a fixture-teszt is elbukik, mert a két report capture azonosítója megegyezik. A manual-idempotencia teszt zöld marad. A rollback-teszt a meglévő tranzakció miatt zöld lehet. Ezt a Task 2 nem lépheti át piros kimenet nélkül.

---

### Task 2: Migráció, közös raw repository és fixture-átállás

**Files:**
- Create: `db/migrations/0032_raw_capture_history.sql`
- Modify: `rust/crates/storage/src/repositories/worker.rs:764-848`
- Modify: `rust/crates/ingest/src/fixture.rs:122-219`
- Modify: `rust/crates/storage/src/repositories/manual.rs:133-177` és `:316-338`

**Interfaces:**
- Consumes: Task 1 tesztjei és a meglévő `persist_raw(pool, source_code, source_display_name, artifact, capture, media_type) -> Result<(Uuid, Uuid)>` szerződés.
- Produces: minden `persist_raw` hívás új capture UUID-t ad; a fixture ugyanazt a szerződést használja; a manual capture továbbra is idempotens.

- [ ] **Step 1: Hozd létre a `0032` migrációt**

```sql
BEGIN;

-- S218/O0: a request_key stabil kérésidentitás, nem capture-identitás.
-- Régi capture-okat nem törlünk; a változás csak az ismétlődő fetch-ek
-- megőrzését és a provenance pontos megkötését teszi lehetővé.

ALTER TABLE raw.capture
    DROP CONSTRAINT IF EXISTS capture_source_id_request_key_key;

CREATE INDEX IF NOT EXISTS idx_raw_capture_latest
    ON raw.capture (source_id, request_key, requested_at DESC, id DESC);

COMMIT;

-- Visszaállítás: csak akkor adható vissza a
-- capture_source_id_request_key_key UNIQUE (source_id, request_key) constraint,
-- ha a migráció óta keletkezett ismétlődő capture-k előbb archiválhatók vagy
-- egyértelművé válnak. A 0032-es _sqlx_migrations sor törlése önmagában nem
-- visszafordítható, ha már több capture ugyanazzal a request_key-vel létezik.
```

A migráció nem hoz létre táblát, ezért a `scripts/db-check.sh` elvárt táblaszáma nem változik.

- [ ] **Step 2: Írd át a `persist_raw` capture-beszúrását**

A `worker.rs` dokumentációs kommentjét cseréld a következőre:

```rust
/// L0 nyers megőrzés egy tranzakcióban: ops.source upsert + raw.artifact
/// upsert (content_sha256 unique) + minden hívásra új raw.capture. A stabil
/// request_key nem capture-identitás. Visszatérés: (source_id, capture_id), ahol
/// a capture_id a ténylegesen beszúrt sor azonosítója.
///
/// A `source_code` a hiteles forrásazonosító: a hívó a job source_slugját
/// adja át, NEM a payloadból olvasott kódot (megszemélyesítés-védelem).
/// A `source_display_name` csak megjelenítési név; forrásazonosságot nem
/// dönthet el.
```

A capture `INSERT`-et és az utána követő `SELECT`-et egyetlen `query_scalar`-re cseréld:

```rust
    let capture_id: Uuid = sqlx::query_scalar(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(artifact_id)
    .bind(capture.endpoint_key)
    .bind(capture.request_url)
    .bind(capture.requested_at)
    .bind(capture.received_at)
    .bind(capture.http_status)
    .bind(capture.adapter_version)
    .bind(capture.parser_version)
    .bind(capture.request_key)
    .fetch_one(&mut *tx)
    .await
    .context("capture beszúrás")?;
```

A korábbi `ON CONFLICT (source_id, request_key) DO NOTHING` és a külön `SELECT id ... request_key=$2` teljes egészében törlendő.

- [ ] **Step 3: Állítsd a fixture raw írást a közös repository-ra**

A `fixture.rs` importjaihoz add hozzá:

```rust
use football_storage::repositories::worker::{RawCapture, persist_raw};
```

A `FixtureAdapter::ingest` artifact-upsert, source-upsert, capture-insert és capture-select részét a következő blokkal cseréld:

```rust
        let request_key = format!(
            "{}:{}:{}",
            fixture.source_code, fixture.endpoint_key, fixture.captured_at
        );
        let (source_id, capture_id) = persist_raw(
            pool,
            &fixture.source_code,
            &fixture.source_display_name,
            &artifact,
            &RawCapture {
                endpoint_key: &fixture.endpoint_key,
                request_url: &fixture.request_url,
                request_key: &request_key,
                requested_at: fixture.captured_at,
                received_at: fixture.captured_at,
                http_status: 200,
                adapter_version: &fixture.adapter_version,
                parser_version: &fixture.parser_version,
            },
            "application/json",
        )
        .await
        .context("fixture nyers megőrzése")?;
```

A `FixtureAdapter::ingest` végén lévő, most már nem használt `capture_id`-keresést is töröld. A fájl alján lévő `upsert_source` helper-t teljes egészében töröld.

- [ ] **Step 4: Tartsd meg a manual idempotenciát, de pontosítsd a capture visszaadását**

A manual előzetes capture-keresést cseréld:

```rust
        let previous_capture: Option<Uuid> = sqlx::query_scalar(
            "SELECT c.id FROM raw.capture c
              WHERE c.source_id=(SELECT id FROM ops.source WHERE code=$1)
                AND c.request_key=$2
              ORDER BY c.requested_at DESC, c.id DESC
              LIMIT 1",
        )
        .bind(SOURCE_CODE)
        .bind(request_key)
        .fetch_optional(&mut *tx)
        .await?;
```

A manual capture `INSERT`-et `RETURNING id`-vel láttasd el, és töröld a külön `SELECT`-et:

```rust
    let capture_id: Uuid = sqlx::query_scalar(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,$4,'manual://ui',$5,$6,200,$7,$8,$9)
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(artifact_id)
    .bind(ENDPOINT_KEY)
    .bind(captured_at)
    .bind(captured_at)
    .bind(ADAPTER_VERSION)
    .bind(ADAPTER_VERSION)
    .bind(&request_key)
    .fetch_one(&mut *tx)
    .await?;
```

- [ ] **Step 5: Formázd, reseteld a tesztadatbázist, és futtasd le a célzott teszteket**

```bash
cd /Users/susleczandras/Documents/Code/football-app-v2
set -a && source .env && set +a
./scripts/db-reset.sh
cd rust
cargo fmt --all
cargo test -p football-storage --test worker_repos persist_raw_ -- --test-threads=1
cargo test -p football-ingest --test capture_observation -- --test-threads=1
cargo test -p football-worker --test tick_e2e repeated_fetch_with_same_request_key_keeps_per_capture_provenance -- --test-threads=1
cargo test -p football-api --test match_edit edit_is_idempotent_by_explicit_key_and_canonical_payload -- --test-threads=1
```

**Elvárt zöld bizonyíték:** a storage teszt `(artifact, capture, source) = (1, 3, 1)` állapotot ér el; a fixture-teszt `(1, 2, 3)` értékeket ad; a worker E2E két capture-t, két artifactet, két különböző observation-capture-t és `(0, 0, 0)` árva sort mutat; a manual-teszt `reused=true` és egy manual capture mellett zöld.

---

### Task 3: Teljes kapu, független review, checkpoint és commit

**Files:**
- Modify: `docs/checkpoints/2026-11-27/checkpoint.md`
- Review: a Task 1–2 összes módosított fájlja

**Interfaces:**
- Consumes: Task 2 zöld célzott tesztjei.
- Produces: teljes workspace-kapu, független verdikt, magyar checkpoint és egy commitolható O0 szelet.

- [ ] **Step 1: Futtasd le a teljes backend kapuláncot**

```bash
cd /Users/susleczandras/Documents/Code/football-app-v2
set -a && source .env && set +a
./scripts/db-reset.sh
./scripts/check-foundation.sh
cd rust
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p footballd -p footballctl --features obscura
cd ..
./scripts/db-reset.sh
git diff --check
```

Minden parancs exitkódját és a tényleges teszt-, figyelmeztetés- és hibaszámot rögzíteni kell. A `db-reset.sh` a tesztadatbázist reseteli; a dev adatbázist nem érinti.

- [ ] **Step 2: Keresd meg az írt magyar szöveg karakterhibáit**

```bash
python3 - <<'PY'
import pathlib
import re
import subprocess

root = pathlib.Path.cwd()
paths = subprocess.check_output(
    ["git", "diff", "--name-only"], text=True
).splitlines()
bad = []
for name in paths:
    path = root / name
    if not path.is_file():
        continue
    text = path.read_text(encoding="utf-8", errors="replace")
    for number, line in enumerate(text.splitlines(), 1):
        if re.search(r"[\u3400-\u9fff]", line):
            bad.append(f"{name}:{number}: {line}")
if bad:
    print("\n".join(bad))
    raise SystemExit(1)
print(f"OK: {len(paths)} módosult fájl, CJK karakter 0")
PY
```

- [ ] **Step 3: Kérd meg egy független, read-only ellenőrt a diff és a spec ellenőrzésére**

Az ellenőr nem írhat fájlt és nem futtathat DB-írást. A prompt tartalmazza a spec §8.1-et, a tervet, a `git diff --stat` és a `git diff` kimenetét, valamint a Task 2 valós kimenetét. A verdiktnek ki kell mondania:

- minden fetch új capture-t kap-e;
- a változott observation a saját capture-jére mutat-e;
- az artifact-deduplikáció és a manual-idempotencia megmaradt-e;
- létezik-e DB-szintű árva sor vagy nem kezelt író út;
- a migráció és a visszaállítás konzisztens-e;
- a spec bármely O0 követelménye fedetlen maradt-e.

A builder nem váltathatja át a saját munkáját „függetlenül ellenőrzött"-re; a verdiktet a külön ellenőr adja.

- [ ] **Step 4: Írd be a checkpointbe a mért eredményeket**

A `docs/checkpoints/2026-11-27/checkpoint.md` végére kerüljön egy `S218 — O0 raw capture/provenance invariant` szakasz. A szakaszban kötelezően szerepeljenek:

- a változott fájlok és a `0032` migráció;
- a Task 1 valós piros kimenete;
- a Task 2 négy célzott tesztjének valós eredménye és számai;
- a teljes workspace-teszt tényleges passed/failed száma;
- a clippy, fmt, obscura build, `db-reset` és `git diff --check` exitkódja;
- az árva artifact/capture/observation lekérdezés `(0, 0, 0)` eredménye;
- a manual capture számának 1-es eredménye;
- a nem ellenőrzött fizikai objektumtár-fájl kockázata;
- hogy a dev DB és a futó daemon nem változott;
- a visszaállás: kód `git revert`, schema visszaállítás csak az ismétlődő capture-k archiválása után.

A checkpoint `- [ ]` lépései csak a ténylegesen lefutott parancs után pipálhatók ki.

- [ ] **Step 5: Nézd át a staging listát, és commitold a szeletet**

```bash
git status --short
git diff --stat
git diff -- db/migrations/0032_raw_capture_history.sql \
  rust/crates/storage/src/repositories/worker.rs \
  rust/crates/storage/tests/worker_repos.rs \
  rust/crates/ingest/src/fixture.rs \
  rust/crates/ingest/tests/capture_observation.rs \
  rust/crates/worker/tests/tick_e2e.rs \
  rust/crates/storage/src/repositories/manual.rs \
  rust/crates/api/tests/match_edit.rs \
  docs/checkpoints/2026-11-27/checkpoint.md
```

Csak a fenti kilenc fájl kerülhet stagingra. Titok, `.env`, nyers adat, adatbázis-dump és generált fájl nem kerülhet bele.

```bash
git add db/migrations/0032_raw_capture_history.sql \
  rust/crates/storage/src/repositories/worker.rs \
  rust/crates/storage/tests/worker_repos.rs \
  rust/crates/ingest/src/fixture.rs \
  rust/crates/ingest/tests/capture_observation.rs \
  rust/crates/worker/tests/tick_e2e.rs \
  rust/crates/storage/src/repositories/manual.rs \
  rust/crates/api/tests/match_edit.rs \
  docs/checkpoints/2026-11-27/checkpoint.md
git commit -m "$(cat <<'EOF'
fix(S218): Fetch-enkénti raw capture és pontos provenance

Minden fetch saját capture-t kap, miközben az artifact- és
observation-deduplikáció megmarad; a manual szerkesztés idempotens marad.
EOF
)"
git status --short --branch
```

- [ ] **Step 6: Pushold a saját feature branch-et a projekt állandó felhatalmazása szerint**

```bash
git push -u origin feat/ops-triage-console
```

Ne pusholj `main`-re, ne force-pusholj, és ne telepítsd a futó daemont ebben a lépésben.

---

## Spec Coverage — O0

| Spec-követelmény | Megvalósítás és bizonyíték |
|---|---|
| Egy fetch minden esetben új `raw.capture` sort hoz létre. | `0032`, `persist_raw RETURNING id`, storage-regresszió és worker E2E. |
| `(source_id, request_key)` nem lehet egyedi korlátozás. | `0032` eltávolítja a constraintot; a storage-teszt két azonos `request_key`-es capture-t vár. |
| `persist_raw` a ténylegesen beszúrt capture azonosítóját adja vissza. | `INSERT ... RETURNING id`; nincs utólagos `SELECT`. |
| Minden observation a saját capture-jére mutat. | A worker E2E a módosult `DEMO-1001` observationt a második capture-hez köti; a tartalomdeduplikáció megmarad. |
| Új artifact nem maradhat árva egy ütköző capture miatt. | Nincs capture `ON CONFLICT`; a rollback-teszt és az árvaság-lekérdezések `(0, 0, 0)` eredményt várnak. |
| A migráció nem töröl régi capture-t. | A migráció csak constraintot cserél indexre; nincs `DELETE`, `TRUNCATE` vagy adatjavítás. |
| Artifact deduplikáció megmarad. | Storage- és fixture-tesztek: két capture, egy artifact. |
| Manual idempotencia nem romlik. | A meglévő `FOR UPDATE` és request-key előkeresés megmarad; API-teszt egy manual capture-t vár. |
| Nincs új tábla vagy függőség. | `0032` csak `ALTER TABLE` és `CREATE INDEX`; a Cargo manifest nem változik. |
| Éles átállás külön döntés. | A terv nem futtat dev migrációt és nem telepíti a daemont. |

## Ismert korlátok és kockázatok

- A `ObjectStore` fájl írása a DB-tranzakció előtt történik. DB-hiba esetén egy fizikai, adatbázisban nem hivatkozott fájl maradhat. Az O0 nem töröl fájlokat automatikusan; a kockázat checkpointben nyitott marad.
- A régi daemon bináris a régi `ON CONFLICT (source_id, request_key)` SQL-re támaszkodik. A dev schema és a daemon csak egy külön engedélyezett, egyszerre végrehajtott átállításban cserélhető.
- A migráció visszafordítása nem automatikus, ha közben több capture keletkezett ugyanazzal a `request_key`-vel. A régi unique constraint csak az ismétlődő capture-k archiválása után állítható vissza.
- A fizikai objektumtár tisztítása és az éles átállás nem része az O0-nak; ez nem blokkolja a tesztadatbázis és a workspace kapuját, de a változás nem él a futó daemonban.

## Execution Handoff

A tervet a felhasználó állandó autonómiája alapján ugyanebben a munkamenetben, inline hajtom végre a Task 1–3 sorrendben. Külön végrehajtási kérdés nincs szükséges. A dev adatbázis és a futó daemon átállítása nem történik meg a szelet végén; azt külön, explicit engedélyezett lépésként kell kezelni.
