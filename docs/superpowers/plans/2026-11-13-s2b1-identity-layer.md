# S2b-1 — Fail-closed identitás-réteg megvalósítási terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fail-closed identitás-linker (crosswalk, alias, review-sor) megépítése új identity crate-ben, a tick live ágának hardcode-olt identitását lecserélve, CLI review-val — szintetikus fixture-ökön bizonyítva, valódi adatimport nélkül.

**Architecture:** Új `football-identity` crate (linker szolgáltatás) a domain tiszta szabályaira és a storage identity repo-ira épül; a 0005 migráció 3 identity táblát ad (crosswalk, alias, review_queue). A worker tick live ága a linker `resolve`-ját hívja; a CLI `footballctl identity` alparancsokkal kezeli a review-sort. A gate szabályai változatlanok (S2b-2).

**Tech Stack:** Rust (edition 2024, rust-version 1.85), sqlx 0.8 (PostgreSQL 18, NULLS NOT DISTINCT), tokio, thiserror 2, chrono, uuid v7 — mind már workspace-függőség, új külső függőség NINCS.

**Spec:** `docs/superpowers/specs/2026-11-13-s2b1-identity-layer-design.md`

## Global Constraints

- AGENTS.md §2: **minden task végén STOP** — összefoglaló + bizonyíték, következő task csak felhasználói engedéllyel.
- AGENTS.md §9 + S1/S2a ruling: **commit NEM része a task-lépéseknek** — commit csak kifejezett felhasználói kérésre; review-csomag a munkafa-diffből.
- Munkaág: `feat/s2b1-identity-layer` (a `main` 07b1936-ról indul; létrehozása a végrehajtás első lépése, felhasználói jóváhagyással). Nincs git worktree.
- Dokumentumok, hibaüzenetek, CLI-szövegek **magyarul**; kódazonosítók angolul. Magyar szöveg Write/Edit után visszaolvasandó (ékezetek, idegen glyph).
- Kérésfeldolgozásban **nincs `unwrap()`/`expect()`/elnémítás**; tesztkódban megengedett.
- **Élő hálózati kérés 0**; **nyers payload a repóban 0** (szintetikus fixture-ök).
- Tesztek: `TEST_DATABASE_URL` kötelező (`.env`); integrációs tesztek **egy szálon** (`--test-threads=1`); csomag-suiték KÜLÖN futtatva (a kombinált `-p A -p B` TRUNCATE-versenyt okozhat); a hivatalos kapu `cargo test --workspace -- --test-threads=1`.
- Minden migráció: tesztDB → zöld → devDB (dev csak a záró taskban, külön engedéllyel). Hiányzó adat NULL/státusz, soha nem nulla; minden időpont UTC.
- **Fuzzy soha nem ad Exact-et** (AGENTS + spec); kontextus-szűrő minden útvonalon kötelező.
- Alapellenőrzés a szelet végén: `./scripts/check.sh` + `./scripts/db-reset.sh && ./scripts/db-check.sh`.

## Fájlstruktúra

| Fájl | Felelősség |
|---|---|
| `rust/crates/domain/src/identity.rs` (új) | tiszta szabályok: normalizálás, kontextus, token-átfedés, típusok |
| `db/migrations/0005_identity_layer.sql` (új) | crosswalk, alias, review_queue táblák |
| `rust/crates/storage/src/repositories/identity.rs` (kibővül) | identity SQL repository-k |
| `rust/crates/storage/src/test_support.rs` (módosul) | TRUNCATE-lista bővítés |
| `rust/crates/storage/tests/connection.rs` (módosul) | táblaszám 29→32 |
| `scripts/db-check.sh` (módosul) | per-DB elvárás: dev 29 (záró taskig), test 32 |
| `rust/crates/identity/` (új crate) | `linker.rs` (resolve), `review.rs` (approve/reject), `error.rs` |
| `rust/crates/worker/src/tick.rs` (módosul) | live ág: hardcode → linker |
| `rust/bins/footballctl/src/main.rs` (módosul) | `identity` alparancsok |
| `tests/fixtures/identity/` (új) | szintetikus crosswalk/alias fixture-ök |
| `docs/operations/identity.md` (új) | review menete, CLI, visszaállítás |

---

### Task 1: Domain identity típusok + tiszta szabályok

**Files:**
- Create: `rust/crates/domain/src/identity.rs`
- Modify: `rust/crates/domain/src/lib.rs` (`pub mod identity;`)
- Test: `rust/crates/domain/tests/identity_rules.rs`

**Interfaces:**
- Produces (minden későbbi task ezeket használja):
  - `normalize_name(&str) -> String` (kisbetű + whitespace-collapse, ékezetek megmaradnak)
  - `IdentityContext { country_code, gender, age_group, branch: Option<String> }`
  - `context_matches(&IdentityContext, &IdentityContext) -> bool` (minden mezőre: ha mindkettő Some és eltér → false; egyébként true)
  - `token_overlap(&str, &str) -> f64` (normalizált token-halmazok Jaccard: |metszet| / |unió|; üres unió → 0.0)
  - `FUZZY_THRESHOLD: f64 = 0.6`
  - `ResolveMethod::{ExactExternalId, CrosswalkChain, ReviewedAlias, ExactName, ManualReview}` (+ `as_db_str`)
  - `Candidate { entity_id: Uuid, score: f64, reason: &'static str }`
  - `Resolution::{Exact { entity_id, method }, Candidates { candidates, queue_id }, NotFound { queue_id }}`

- [ ] **Step 1: Bukó teszt**

`rust/crates/domain/tests/identity_rules.rs`:

```rust
use football_domain::identity::{
    context_matches, normalize_name, token_overlap, Candidate, IdentityContext,
    Resolution, ResolveMethod, FUZZY_THRESHOLD,
};
use uuid::Uuid;

#[test]
fn normalize_keeps_accents_collapses_whitespace() {
    assert_eq!(normalize_name("  Ferencvárosi   TC "), "ferencvárosi tc");
    assert_eq!(normalize_name("Újpest FC"), "újpest fc");
}

#[test]
fn context_filter_blocks_mismatch_allows_unknown() {
    let hu = IdentityContext { country_code: Some("HU".into()), gender: None, age_group: None, branch: None };
    let en = IdentityContext { country_code: Some("EN".into()), gender: None, age_group: None, branch: None };
    let unknown = IdentityContext { country_code: None, gender: None, age_group: None, branch: None };
    assert!(!context_matches(&hu, &en));
    assert!(context_matches(&hu, &unknown));
    assert!(context_matches(&hu, &hu));
    let women = IdentityContext { country_code: Some("HU".into()), gender: Some("women".into()), age_group: None, branch: None };
    assert!(!context_matches(&hu, &women));
}

#[test]
fn token_overlap_jaccard() {
    assert_eq!(token_overlap("Manchester United", "Manchester United FC"), 2.0 / 3.0);
    assert_eq!(token_overlap("Újpest", "Ferencváros"), 0.0);
    assert_eq!(token_overlap("", ""), 0.0);
}

#[test]
fn fuzzy_threshold_is_candidate_only() {
    // a küszöb értéke rögzített; a linker soha nem ad Exact-et fuzzy-ból (linker tesztek)
    assert_eq!(FUZZY_THRESHOLD, 0.6);
    let c = Candidate { entity_id: Uuid::now_v7(), score: 0.9, reason: "fuzzy_candidate" };
    let r = Resolution::Candidates { candidates: vec![c], queue_id: Uuid::now_v7() };
    assert!(matches!(r, Resolution::Candidates { .. }));
    assert_eq!(ResolveMethod::ManualReview.as_db_str(), "manual_review");
}
```

- [ ] **Step 2: Futtatás — bukás**

Run: `cd rust && cargo test -p football-domain --test identity_rules`
Expected: FAIL — `identity` modul nincs (fordítási hiba).

- [ ] **Step 3: Implementáció**

`rust/crates/domain/src/identity.rs`:

```rust
use uuid::Uuid;

pub const FUZZY_THRESHOLD: f64 = 0.6;

pub fn normalize_name(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityContext {
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
}

fn field_matches(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        _ => true,
    }
}

pub fn context_matches(a: &IdentityContext, b: &IdentityContext) -> bool {
    field_matches(&a.country_code, &b.country_code)
        && field_matches(&a.gender, &b.gender)
        && field_matches(&a.age_group, &b.age_group)
        && field_matches(&a.branch, &b.branch)
}

pub fn token_overlap(a: &str, b: &str) -> f64 {
    let ta: std::collections::HashSet<String> =
        normalize_name(a).split(' ').map(String::from).filter(|t| !t.is_empty()).collect();
    let tb: std::collections::HashSet<String> =
        normalize_name(b).split(' ').map(String::from).filter(|t| !t.is_empty()).collect();
    let union = ta.union(&tb).count();
    if union == 0 { return 0.0; }
    ta.intersection(&tb).count() as f64 / union as f64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveMethod { ExactExternalId, CrosswalkChain, ReviewedAlias, ExactName, ManualReview }

impl ResolveMethod {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::ExactExternalId => "exact_external_id",
            Self::CrosswalkChain => "crosswalk_chain",
            Self::ReviewedAlias => "reviewed_alias",
            Self::ExactName => "exact_name",
            Self::ManualReview => "manual_review",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Candidate { pub entity_id: Uuid, pub score: f64, pub reason: &'static str }

#[derive(Debug, Clone)]
pub enum Resolution {
    Exact { entity_id: Uuid, method: ResolveMethod },
    Candidates { candidates: Vec<Candidate>, queue_id: Uuid },
    NotFound { queue_id: Uuid },
}
```

`lib.rs`: `pub mod identity;` a meglévő modulok mellé.

- [ ] **Step 4: Futtatás — zöld**

Run: `cd rust && cargo test -p football-domain --test identity_rules && cargo fmt --all --check && cargo clippy -p football-domain --all-targets -- -D warnings`
Expected: 4 teszt PASS, fmt/clippy tiszta.

- [ ] **Step 5: STOP — jelentés.** Checkpoint-jegyzet; commit NEM készül.

---

### Task 2: 0005 migráció + teszt-támogatás

**Files:**
- Create: `db/migrations/0005_identity_layer.sql`
- Modify: `rust/crates/storage/src/test_support.rs`, `rust/crates/storage/tests/connection.rs`, `scripts/db-check.sh`
- Test: `rust/crates/storage/tests/identity_schema.rs`

**Interfaces:**
- Consumes: Task 1 db-string értékei (a CHECK-készletek a spec §3 szerint).
- Produces: `identity.crosswalk`, `identity.alias`, `identity.review_queue` táblák; test_pool truncatálja őket; connection.rs 32 táblát vár; db-check: dev 29 (a záró task dev-migrációjáig), test 32.

- [ ] **Step 1: Bukó séma-teszt**

`rust/crates/storage/tests/identity_schema.rs`:

```rust
use football_storage::test_support::test_pool;

#[tokio::test]
async fn identity_tables_and_constraints() {
    let pool = test_pool().await;
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname='identity'
         AND tablename IN ('crosswalk','alias','review_queue')",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(tables, 3);

    // CHECK: ismeretlen rung nem írható
    let bad = sqlx::query(
        "INSERT INTO identity.crosswalk (id, left_source, left_external_id,
         right_source, right_external_id, rung, method, evidence)
         VALUES (gen_random_uuid(),'a','1','b','2','unknown','import','x')",
    ).execute(&pool).await;
    assert!(bad.is_err());

    // részleges pending unique: ugyanarra a kulcsra csak egy pending
    sqlx::query(
        "INSERT INTO identity.review_queue (id, entity_kind, source_entity_id,
         candidate_entity_ids, reason, proposed_name, context)
         VALUES (gen_random_uuid(),'team','x','[]','no_exact_match','X','{}')",
    ).execute(&pool).await.unwrap();
    let dup = sqlx::query(
        "INSERT INTO identity.review_queue (id, entity_kind, source_entity_id,
         candidate_entity_ids, reason, proposed_name, context)
         VALUES (gen_random_uuid(),'team','x','[]','no_exact_match','X','{}')",
    ).execute(&pool).await;
    assert!(dup.is_err());
}
```

- [ ] **Step 2: Futtatás — bukás**

Run: `cd rust && cargo test -p football-storage --test identity_schema -- --test-threads=1`
Expected: FAIL — táblák nincsenek.

- [ ] **Step 3: Migráció + kísérő fájlok**

`db/migrations/0005_identity_layer.sql`:

```sql
-- 0005 — fail-closed identitás-réteg táblái (S2b-1).
-- Visszaállítási út: scripts/db-reset.sh (tesztDB); dev DB-n a táblák üresek,
-- eldobásuk fordított sorrendben: review_queue, alias, crosswalk.

BEGIN;

CREATE TABLE identity.crosswalk (
    id uuid PRIMARY KEY,
    left_source text NOT NULL,
    left_external_id text NOT NULL,
    right_source text NOT NULL,
    right_external_id text NOT NULL,
    rung text NOT NULL CHECK (rung IN ('strong','medium','weak')),
    method text NOT NULL CHECK (method IN ('exact_external_id','manual_review','import')),
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active','superseded','rejected')),
    evidence text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (left_source, left_external_id, right_source, right_external_id)
);

CREATE TABLE identity.alias (
    id uuid PRIMARY KEY,
    entity_id uuid NOT NULL REFERENCES core.entity(id),
    alias text NOT NULL,
    lang text NOT NULL,
    country_code text,
    gender text,
    age_group text,
    branch text,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','reviewed','rejected')),
    method text NOT NULL CHECK (method IN ('manual_review','import','fixture_seed')),
    evidence_observation_id uuid REFERENCES stage.observation(id),
    reviewed_by text,
    reviewed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE NULLS NOT DISTINCT (entity_id, alias, lang, country_code, gender, age_group, branch)
);

CREATE INDEX idx_alias_normalized ON identity.alias (lower(alias));

CREATE TABLE identity.review_queue (
    id uuid PRIMARY KEY,
    source_id uuid REFERENCES ops.source(id),
    entity_kind text NOT NULL,
    source_entity_id text NOT NULL,
    candidate_entity_ids jsonb NOT NULL DEFAULT '[]',
    reason text NOT NULL CHECK (reason IN
        ('no_exact_match','multiple_candidates','alias_conflict','fuzzy_candidate','name_mismatch')),
    proposed_name text NOT NULL,
    context jsonb NOT NULL DEFAULT '{}',
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    created_at timestamptz NOT NULL DEFAULT now(),
    decided_by text,
    decided_at timestamptz,
    decision_note text
);

CREATE UNIQUE INDEX uq_review_pending
    ON identity.review_queue (source_id, entity_kind, source_entity_id, reason)
    WHERE status = 'pending';

COMMIT;
```

`test_support.rs` TRUNCATE-listához (az `identity.source_entity_ref` elé):
`identity.review_queue, identity.alias, identity.crosswalk,`

`connection.rs`: `assert_eq!(tables, 32, "a 0001–0005 migráció után 31 terméktábla és a _sqlx_migrations várható");`

`db-check.sh`: per-DB elvárás — `DATABASE_URL` → 29 (amíg a záró task dev-migrációja le nem fut), `TEST_DATABASE_URL` → 32. A worker_rows/business lekérdezés változatlan; a magyar hibaüzenet „0005 utáni állapot”-ra frissítendő. Záró újsor megmarad.

- [ ] **Step 4: Futtatás — zöld + migrációs fegyelem**

Run: `cd rust && cargo test -p football-storage --test identity_schema -- --test-threads=1`
Expected: PASS.
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh`
Expected: dev OK 29, test OK 32 / 0 üzleti sor. (Dev DB-re a migráció csak a záró taskban kerül, külön engedéllyel.)

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 3: Storage identity repository-k

**Files:**
- Modify: `rust/crates/storage/src/repositories/identity.rs` (a jelenlegi 1 bájtos stub)
- Test: `rust/crates/storage/tests/identity_repos.rs`

**Interfaces:**
- Consumes: Task 1 típusok, Task 2 táblák.
- Produces (mind `anyhow::Result`, `pool: &PgPool`, magyar `.context()`):
  - `source_id_by_code(pool, code) -> Option<Uuid>`
  - `active_ref(pool, source_id, entity_kind, source_entity_id) -> Option<(Uuid, String)>` (entity_id, method; status='active')
  - `CrosswalkRow { right_source, right_external_id, rung }`; `active_crosswalk(pool, source_slug, external_id) -> Vec<CrosswalkRow>` (left→right ÉS right→left irány, status='active')
  - `AliasRow { entity_id, alias, lang, country_code, gender, age_group, branch, status }`; `reviewed_aliases_by_name(pool, normalized_alias) -> Vec<AliasRow>` (status='reviewed', lower(alias) = normalized); `AliasRow::context() -> IdentityContext` helper
  - `EntityRow { entity_id, canonical_name, country_code, gender, age_group, branch }`; `team_entities_by_normalized_name(pool, normalized) -> Vec<EntityRow>` (core.entity kind='team' JOIN core.team JOIN core.country, lower(canonical_name) = normalized); `EntityRow::context() -> IdentityContext` helper
  - `all_team_entities(pool) -> Vec<EntityRow>` (fuzzy jelölt-képzéshez; S2b-1 szintetikus skálán elfogadható, S3-ban szűkítendő — dokumentált korlát)
  - `NewReview { source_id: Option<Uuid>, entity_kind, source_entity_id, candidate_entity_ids: serde_json::Value, reason, proposed_name, context: serde_json::Value }`; `upsert_pending_review(pool, &NewReview) -> Uuid` (ON CONFLICT a részleges unique indexre: `(source_id, entity_kind, source_entity_id, reason) WHERE status='pending'` DO UPDATE SET candidate_entity_ids/proposed_name/context)
  - `ReviewRow { id, source_id, entity_kind, source_entity_id, candidate_entity_ids, reason, proposed_name, context, status, created_at }`; `get_review(pool, id) -> Option<ReviewRow>`, `list_reviews(pool, status: Option<&str>) -> Vec<ReviewRow>` (created_at DESC)
  - `decide_review(pool, id, new_status, decided_by, note: Option<&str>) -> u64` (UPDATE ... WHERE id=$1 AND status='pending'; rows_affected)
  - `insert_manual_ref(pool, source_id, entity_kind, source_entity_id, entity_id) -> ()` (ON CONFLICT DO UPDATE SET entity_id, method='manual_review', status='active')
  - `entity_exists(pool, entity_id) -> bool`
  - `entity_context(pool, entity_id) -> Option<IdentityContext>` (core.team + core.country)
  - `record_identity_decision(pool, actor, action, queue_id, detail: serde_json::Value)` (ops.audit_event insert)
  - `list_aliases(pool, entity_id: Option<Uuid>) -> Vec<AliasRow>`

- [ ] **Step 1: Bukó integrációs teszt** — a lényegi esetek:

```rust
use football_storage::repositories::identity as repo;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn pending_review_upsert_dedup() {
    let pool = test_pool().await;
    let r = repo::NewReview {
        source_id: None, entity_kind: "team".into(), source_entity_id: "x".into(),
        candidate_entity_ids: serde_json::json!([]), reason: "no_exact_match".into(),
        proposed_name: "X".into(), context: serde_json::json!({}),
    };
    let id1 = repo::upsert_pending_review(&pool, &r).await.unwrap();
    let id2 = repo::upsert_pending_review(&pool, &r).await.unwrap();
    assert_eq!(id1, id2); // ugyanaz a pending sor frissül
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.review_queue")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn decide_only_pending_and_manual_ref_upsert() {
    let pool = test_pool().await;
    // seed: ops.source + core.entity (a canonicalize mintáján, minimális FK-lánc)
    let source_id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'demo','Demó')")
        .bind(source_id).execute(&pool).await.unwrap();
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team','Demó FC')")
        .bind(entity_id).execute(&pool).await.unwrap();
    let r = repo::NewReview {
        source_id: Some(source_id), entity_kind: "team".into(), source_entity_id: "x".into(),
        candidate_entity_ids: serde_json::json!([]), reason: "no_exact_match".into(),
        proposed_name: "Demó FC".into(), context: serde_json::json!({}),
    };
    let queue_id = repo::upsert_pending_review(&pool, &r).await.unwrap();
    assert_eq!(repo::decide_review(&pool, queue_id, "approved", "test", None).await.unwrap(), 1);
    assert_eq!(repo::decide_review(&pool, queue_id, "approved", "test", None).await.unwrap(), 0);
    repo::insert_manual_ref(&pool, source_id, "team", "x", entity_id).await.unwrap();
    repo::insert_manual_ref(&pool, source_id, "team", "x", entity_id).await.unwrap();
    let (eid, method): (Uuid, String) = sqlx::query_as(
        "SELECT entity_id, method FROM identity.source_entity_ref
         WHERE source_id=$1 AND entity_kind='team' AND source_entity_id='x' AND status='active'")
        .bind(source_id).fetch_one(&pool).await.unwrap();
    assert_eq!(eid, entity_id);
    assert_eq!(method, "manual_review");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.source_entity_ref")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
}
```

(A teljes fájl a fenti interfész minden függvényét fedje: active_ref, crosswalk mindkét irány, alias-szűrés, entity-join, decide, audit, list_aliases.)

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-storage --test identity_repos -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció** — a fenti interfészek SQLx-szel, a meglző repo-mintán (inline SQL, Uuid::now_v7(), magyar context). Kulcsdarab:

```rust
pub async fn upsert_pending_review(pool: &PgPool, r: &NewReview) -> Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO identity.review_queue
         (id, source_id, entity_kind, source_entity_id, candidate_entity_ids,
          reason, proposed_name, context)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         ON CONFLICT (source_id, entity_kind, source_entity_id, reason)
           WHERE status = 'pending'
         DO UPDATE SET candidate_entity_ids = EXCLUDED.candidate_entity_ids,
           proposed_name = EXCLUDED.proposed_name, context = EXCLUDED.context
         RETURNING id",
    )
    .bind(Uuid::now_v7()).bind(r.source_id).bind(&r.entity_kind)
    .bind(&r.source_entity_id).bind(&r.candidate_entity_ids).bind(&r.reason)
    .bind(&r.proposed_name).bind(&r.context)
    .fetch_one(pool).await.context("review-sor upsert")
}

pub async fn active_crosswalk(pool: &PgPool, source_slug: &str, external_id: &str)
    -> Result<Vec<CrosswalkRow>> {
    sqlx::query_as(
        "SELECT right_source, right_external_id, rung FROM identity.crosswalk
         WHERE status='active' AND left_source=$1 AND left_external_id=$2
         UNION ALL
         SELECT left_source, left_external_id, rung FROM identity.crosswalk
         WHERE status='active' AND right_source=$1 AND right_external_id=$2",
    )
    .bind(source_slug).bind(external_id)
    .fetch_all(pool).await.context("crosswalk lekérdezés")
}
```

- [ ] **Step 4: Futtatás — zöld.** Run: `cd rust && cargo test -p football-storage --test identity_repos -- --test-threads=1 && cargo clippy -p football-storage --all-targets -- -D warnings` → PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 4: Identity crate + linker resolve

**Files:**
- Create: `rust/crates/identity/Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/linker.rs`
- Modify: `rust/Cargo.toml` (workspace members: crates/* már fedi), `rust/crates/worker/Cargo.toml` NEM (majd Task 6)
- Test: `rust/crates/identity/tests/linker.rs`, `tests/fixtures/identity/` (szintetikus seed-adatok SQL-ben a tesztben)

**Interfaces:**
- Consumes: Task 1 domain szabályok, Task 3 repo-k.
- Produces:
  - `IdentityError::{Db(String), NotFound, AmbiguousDecision, ContextConflict, AlreadyDecided, InvalidEntity}` (thiserror, magyar üzenetek)
  - `linker::resolve(pool, source_id: Uuid, source_slug: &str, entity_kind: &str, source_entity_id: &str, proposed_name: &str, ctx: &IdentityContext) -> Result<Resolution, IdentityError>`
  - Megjegyzés: a spec §2.2 szignatúrája `source_slug`-gal bővül (a crosswalk szöveges forrás-kulcsot használ) — backward kompatibilis kiegészítés, a spec review-jegyzetében rögzítendő.

- [ ] **Step 1: Crate-váz + bukó tesztek**

`Cargo.toml`: name `football-identity`; deps: football-domain, football-storage, anyhow, thiserror, chrono, sqlx, tokio, tracing, uuid, serde_json (mind workspace). dev: tempfile.

`tests/linker.rs` — a 6 feloldási út + garanciák (mindegyik saját seed-del):

```rust
// seed helper: ops.source ('demo-synthetic'), core.entity/team ('Demó FC', HU),
// identity.source_entity_ref (fixture_seed), crosswalk/alias sorok igény szerint.

#[tokio::test]
async fn exact_external_id_resolves() { /* ref → Exact(ExactExternalId) */ }

#[tokio::test]
async fn crosswalk_strong_chain_resolves_weak_only_candidates() {
    // strong crosswalk demo→other + other-ref → Exact(CrosswalkChain)
    // weak crosswalk → nem Exact (Candidates vagy NotFound; a queue reason no_exact_match)
}

#[tokio::test]
async fn reviewed_alias_with_context_resolves() {
    // alias 'Demó FC' lang=hu reviewed, ctx HU → Exact(ReviewedAlias)
    // ctx EN → nem Exact (kontextus-szűrő) → Candidates/NotFound
}

#[tokio::test]
async fn exact_name_unique_vs_ambiguous() {
    // 1 entity → Exact(ExactName); 2 azonos normalizált nevű entity → Candidates(multiple_candidates) + queue
}

#[tokio::test]
async fn fuzzy_never_exact_and_queues() {
    // 'Demó FC U19' vs 'Demó FC' token-átfedés ≥ 0.6 → Candidates(fuzzy_candidate), queue pending
    // kontextus-eltérő jelölt nem kerül a listába
}

#[tokio::test]
async fn not_found_queues_and_pending_dedup() {
    // ismeretlen név → NotFound + queue; második resolve → ugyanaz a queue_id
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-identity -- --test-threads=1` → FAIL (crate nincs).

- [ ] **Step 3: Implementáció**

`error.rs`: a fenti enum, `#[error("...")]` magyar üzenetekkel.

`linker.rs` — a resolve lépései (spec §3.1 sorrend):

```rust
pub async fn resolve(pool: &PgPool, source_id: Uuid, source_slug: &str,
    entity_kind: &str, source_entity_id: &str, proposed_name: &str,
    ctx: &IdentityContext) -> Result<Resolution, IdentityError> {
    // 1. egzakt külső ID
    if let Some((entity_id, method)) = repo::active_ref(pool, source_id, entity_kind, source_entity_id)
        .await.map_err(|e| IdentityError::Db(e.to_string()))? {
        let m = match method.as_str() {
            "manual_review" => ResolveMethod::ManualReview,
            _ => ResolveMethod::ExactExternalId,
        };
        return Ok(Resolution::Exact { entity_id, method: m });
    }
    // 2. crosswalk-lánc (strong/medium; minden elem aktív)
    for cw in repo::active_crosswalk(pool, source_slug, source_entity_id).await.map_err(db)? {
        if cw.rung == "weak" { continue; } // weak rung: sem Exact, sem jelölt (spec: csak strong/medium ad Exact-et)
        if let Some(target_source) = repo::source_id_by_code(pool, &cw.right_source).await.map_err(db)? {
            if let Some((entity_id, _)) = repo::active_ref(pool, target_source, entity_kind, &cw.right_external_id).await.map_err(db)? {
                return Ok(Resolution::Exact { entity_id, method: ResolveMethod::CrosswalkChain });
            }
        }
    }
    let normalized = normalize_name(proposed_name);
    // 3. review-olt alias + kontextus
    let aliases = repo::reviewed_aliases_by_name(pool, &normalized).await.map_err(db)?;
    let alias_hits: Vec<_> = aliases.iter().filter(|a| context_matches(ctx, &a.context())).collect();
    let alias_entities: std::collections::HashSet<Uuid> = alias_hits.iter().map(|a| a.entity_id).collect();
    if alias_entities.len() == 1 {
        if let Some(&entity_id) = alias_entities.iter().next() {
            return Ok(Resolution::Exact { entity_id, method: ResolveMethod::ReviewedAlias });
        }
    }
    // 4. egzakt normalizált név + kontextus
    let entities = repo::team_entities_by_normalized_name(pool, &normalized).await.map_err(db)?;
    let name_hits: Vec<_> = entities.iter().filter(|e| context_matches(ctx, &e.context())).collect();
    if name_hits.len() == 1 {
        return Ok(Resolution::Exact { entity_id: name_hits[0].entity_id, method: ResolveMethod::ExactName });
    }
    // 5. fuzzy jelöltek (soha Exact) + weak crosswalk jelöltek
    let mut candidates: Vec<Candidate> = Vec::new();
    if name_hits.len() > 1 {
        for e in &name_hits { candidates.push(Candidate { entity_id: e.entity_id, score: 1.0, reason: "multiple_candidates" }); }
    }
    for e in repo::all_team_entities(pool).await.map_err(db)? {
        if !context_matches(ctx, &e.context()) { continue; }
        let score = token_overlap(proposed_name, &e.canonical_name);
        if score >= FUZZY_THRESHOLD && !candidates.iter().any(|c| c.entity_id == e.entity_id) {
            candidates.push(Candidate { entity_id: e.entity_id, score, reason: "fuzzy_candidate" });
        }
    }
    let reason = if !candidates.is_empty() {
        if name_hits.len() > 1 { "multiple_candidates" } else { "fuzzy_candidate" }
    } else { "no_exact_match" };
    let queue_id = repo::upsert_pending_review(pool, &repo::NewReview {
        source_id: Some(source_id), entity_kind: entity_kind.into(),
        source_entity_id: source_entity_id.into(),
        candidate_entity_ids: serde_json::to_value(&candidates.iter().map(|c| c.entity_id).collect::<Vec<_>>())
            .map_err(|e| IdentityError::Db(e.to_string()))?,
        reason: reason.into(), proposed_name: proposed_name.into(),
        context: serde_json::to_value(ctx).map_err(|e| IdentityError::Db(e.to_string()))?,
    }).await.map_err(db)?;
    if candidates.is_empty() {
        Ok(Resolution::NotFound { queue_id })
    } else {
        Ok(Resolution::Candidates { candidates, queue_id })
    }
}
```

Megjegyzések: (1) a `db` helper a függvény elején definiálandó: `let db = |e: anyhow::Error| IdentityError::Db(e.to_string());`; (2) az `AliasRow`/`EntityRow` `context()` helperje a Task 3-ban definiált (domain IdentityContext-et ad); (3) a weak crosswalk nem ad Exact-et, a jelölt-listába sem kerül külön (a 2. lépés `continue`-ja kihagyja) — a weak rung viselkedését a crosswalk-teszt assertálja: strong/medium → Exact, weak → nem Exact.

- [ ] **Step 4: Futtatás — zöld.** Run: `cd rust && cargo test -p football-identity -- --test-threads=1 && cargo fmt --all --check && cargo clippy -p football-identity --all-targets -- -D warnings` → minden PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 5: Review-életciklus + CLI

**Files:**
- Create: `rust/crates/identity/src/review.rs`
- Modify: `rust/crates/identity/src/lib.rs`, `rust/bins/footballctl/src/main.rs`, `rust/bins/footballctl/Cargo.toml` (football-identity dep)
- Test: `rust/crates/identity/tests/review.rs`

**Interfaces:**
- Consumes: Task 3 repo-k, Task 4 error.
- Produces:
  - `review::approve(pool, queue_id: Uuid, entity_id: Uuid, decided_by: &str, note: Option<&str>) -> Result<(), IdentityError>` — get_review (nincs → NotFound; nem pending → AlreadyDecided); entity_exists (false → InvalidEntity); entity_context vs queue.context (eltér → ContextConflict); egy tranzakcióban: insert_manual_ref + decide_review(approved) + record_identity_decision(action='identity_approve'); ha decide rows_affected=0 → AlreadyDecided (rollback).
  - `review::reject(pool, queue_id, decided_by, note: &str) -> Result<(), IdentityError>` — note kötelező (üres → InvalidEntity üzenettel); pending-ellenőrzés; tx: decide_review(rejected) + audit('identity_reject').
  - CLI: `Identity { #[command(subcommand)] action: IdentityAction }`; `IdentityAction::{ Review { #[arg(long)] status: Option<String> }, Approve { id: Uuid, #[arg(long)] entity: Uuid, #[arg(long)] note: Option<String> }, Reject { id: Uuid, #[arg(long)] note: String }, ListAliases { #[arg(long)] entity: Option<Uuid> } }`. Kimenetek magyarul; a review-lista oszlopai: id, forrás, external ID, név, ok, jelöltek száma, létrehozva.

- [ ] **Step 1: Bukó tesztek**

```rust
#[tokio::test]
async fn approve_writes_ref_and_audit_then_resolves_exact() {
    // seed: source + entity + pending queue (no_exact_match)
    // approve → source_entity_ref manual_review aktív + queue approved + audit 1 sor
    // linker::resolve → Exact(ManualReview)
}

#[tokio::test]
async fn approve_errors() {
    // nem létező queue → NotFound; már decided → AlreadyDecided;
    // nem létező entity → InvalidEntity; kontextus-ütközés → ContextConflict
}

#[tokio::test]
async fn reject_is_final_and_audited() {
    // reject note-tal → rejected + audit; újra reject → AlreadyDecided;
    // resolve ugyanarra a párra → új pending sort NYIT (új evidence nélkül is NotFound → queue; a rejected sor megmarad)
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-identity --test review -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció** — review.rs a fenti interfész szerint (sqlx tx: `pool.begin()`, a repo-függvények `&mut *tx`-szel hívhatók ha Executor-generikusak — a Task 3 repo-függvények pool-t fogadnak; a tx-hez vagy Executor-generikussá alakítandók a decide/insert/audit függvények, vagy a review.rs saját tx-SQL-t futtat a storage repo új `decide_review_tx`/`insert_manual_ref_tx`/`record_identity_decision_tx` függvényeivel. Választott út: a három érintett repo-függvény kap `Executor`-generikus változatot a storage-ban, a review.rs azokat hívja tx-ben — külön storage-teszt nem kell, a review-tesztek fedik). CLI: a footballctl main.rs Command enumjába `Identity`, a match ágba a négy alparancs; a DB-kapcsolat a meglévő mintán.

- [ ] **Step 4: Futtatás — zöld + kézi CLI**

Run: `cd rust && cargo test -p football-identity -- --test-threads=1 && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Kézi (tesztDB, .env): `cargo run -p footballctl -- identity review` → üres lista magyar fejléccel; `identity reject <nem-létező> --note x` → magyar hibaüzenet. A kimenetek a reportba.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 6: Tick-integráció + E2E

**Files:**
- Modify: `rust/crates/worker/src/tick.rs` (verify_and_publish), `rust/crates/worker/Cargo.toml` (football-identity dep)
- Test: `rust/crates/worker/tests/identity_e2e.rs`

**Interfaces:**
- Consumes: Task 4 linker, Task 5 review.
- Produces: a live ág `home/away_identity_exact` flagjei a linker resolve-jából; shadow ág érintetlen; demo 20/20 regresszió.

- [ ] **Step 1: Bukó E2E tesztek**

`rust/crates/worker/tests/identity_e2e.rs` (az S2a tick_e2e helper-mintáján: policy seed, fixture date-rewrite, FixtureFetcher, DemoAdapter):

```rust
#[tokio::test]
async fn exact_fixture_stays_verified() {
    // a Task 6 (S2a) demo fixture → tick → verified_publish (fixture_seed ref Exact)
}

#[tokio::test]
async fn uncertain_identity_withheld_then_review_opens_gate() {
    // tick után DELETE az egyik csapat fixture_seed ref-je (teszt-only SQL)
    // újra tick (generate + tick) → a meccs withheld + review_queue pending
    // identity::review::approve(queue_id, entity_id, "test", None)
    // újra tick → verified_publish (ManualReview ref Exact)
}

#[tokio::test]
async fn shadow_mode_unchanged() {
    // shadow policy → observation van, core.match 0, review_queue 0 (nincs linkelés)
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-worker --test identity_e2e -- --test-threads=1` → FAIL (a hardcode miatt a második teszt bukik: withheld helyett verified).

- [ ] **Step 3: Implementáció** — a `verify_and_publish` cseréje:

```rust
async fn verify_and_publish(pool: &PgPool, fixture: &FixtureSource) -> Result<()> {
    let day = Canonicalizer::materialize(pool, fixture).await?;
    let rules = default_rules();
    let source_id = football_storage::repositories::identity::source_id_by_code(pool, &fixture.source_code)
        .await?.context("forrás-azonosító az identitáshoz")?;
    for materialized in &day.matches {
        let home_exact = resolve_team_identity(pool, source_id, &fixture.source_code, fixture, materialized.home_team_id).await;
        let away_exact = resolve_team_identity(pool, source_id, &fixture.source_code, fixture, materialized.away_team_id).await;
        let source_values = load_material_values(pool, materialized.match_id).await?;
        let duplicate_match_count = repo::count_duplicate_matches(pool, materialized.match_id,
            materialized.home_team_id, materialized.away_team_id, materialized.snapshot.kickoff_at).await?;
        let context = VerificationContext {
            snapshot: &materialized.snapshot,
            home_identity_exact: home_exact,
            away_identity_exact: away_exact,
            duplicate_match_count,
            source_values: &source_values,
        };
        // a többi változatlan (report, persist, gate)
    }
    Ok(())
}

async fn resolve_team_identity(pool: &PgPool, source_id: Uuid, source_slug: &str,
    fixture: &FixtureSource, team_id: Uuid) -> bool {
    let Some(team) = fixture.teams.iter().find(|t| t.id == team_id) else { return false };
    let ctx = football_domain::identity::IdentityContext {
        country_code: Some(team.country_code.clone()),
        gender: Some(team.gender.clone()),
        age_group: Some(team.age_group.clone()),
        branch: Some(team.branch.clone()),
    };
    match football_identity::linker::resolve(pool, source_id, source_slug, "team",
        &team.source_entity_id, &team.name, &ctx).await {
        Ok(Resolution::Exact { .. }) => true,
        Ok(_) => false, // Candidates/NotFound: a queue-t a linker létrehozta
        Err(e) => { tracing::error!("identitás-feloldás hiba (fail-closed): {e}"); false }
    }
}
```

- [ ] **Step 4: Futtatás — zöld + regresszió**

Run: `cd rust && cargo test -p football-worker -- --test-threads=1` (külön!), majd `cargo test --workspace -- --test-threads=1`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
Expected: identity_e2e 3/3, a meglévő worker/ingest/storage tesztek zöldek; a demo fixture E2E (S2a tick_e2e) változatlanul zöld.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 7: Záró ellenőrzés + operations doc + checkpoint + dev migráció

**Files:**
- Create: `docs/operations/identity.md`
- Modify: `scripts/db-check.sh` (dev 29→32), `docs/checkpoints/2026-11-13/checkpoint.md` (append-only)

- [ ] **Step 1: Operations doc** — `docs/operations/identity.md` (magyar): a linker feloldási sorrendje; review menete (`footballctl identity review/approve/reject/list-aliases`, példakimenetek); a döntés append-only auditja; visszavonás csak új döntéssel; visszaállítási út (0005 drop sorrend: review_queue, alias, crosswalk; tick revert az S2a hardcode-ra); fail-closed garanciák listája.

- [ ] **Step 2: Dev migráció (külön felhasználói engedéllyel)** — `footballctl migrate` a dev DB-n → 32 tábla; `db-check.sh` dev elvárás 29→32.

- [ ] **Step 3: Záró ellenőrzések**

Run: `./scripts/check.sh` → reset + fmt + clippy + workspace-tesztek zöld (pontos szám a checkpointba).
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 32/6/… , test 32/6/0/0.
Run: `cd rust && cargo run -q -p footballctl -- day --date 2026-11-11` → 20 meccs, mind verified_publish.
Kézi: `footballctl identity review` üres lista; a reportba minden parancs + kimenet.

- [ ] **Step 4: Magyar visszaolvasás + glyph-scan** minden új/módosult dokumentumon; záró újsor.

- [ ] **Step 5: Checkpoint záró szekció (append-only)** — taskok + bizonyítékok, ami ellenőrizetlen maradt (valódi crosswalk-import S3; fuzzy all_team_entities skálakorlát), deferred minorok, visszaállítási út.

- [ ] **Step 6: STOP — zárójelentés + commit-kérés.** Teljes összesített zárójelentés; commit csak a felhasználó kifejezett kérésére.

---

## Önreview-jegyzet (a terv írójának)

- **Spec-lefedettség:** §2 architektúra → Task 1/3/4; §3 adatmodell → Task 2/3; §3.1 feloldás → Task 4; §3.2 review → Task 5; §3.3 CLI → Task 5; §2.3 tick → Task 6; §4 garanciák → Task 4/5/6 tesztjei; §5 tesztek → minden task + Task 7; §6 DoD → Task 7.
- **Típus-konzisztencia:** a `Resolution`/`Candidate`/`IdentityContext`/`ResolveMethod` neve minden taskban azonos; a repo-függvénynevek a Task 3 interfész-blokkjából jönnek; a linker `source_slug` paramétere a spec §2.2 szignatúrájának backward kompatibilis bővítése (rögzítendő a spec review-jegyzetében).
- **Ismert egyszerűsítések (dokumentált):** a fuzzy `all_team_entities`-en fut (S2b-1 szintetikus skála; S3-ban szűkítendő); a weak crosswalk rung kihagyott (sem Exact, sem jelölt — a teszt assertálja); az approve source_entity_ref-et ír (alias-import külön szelet).
