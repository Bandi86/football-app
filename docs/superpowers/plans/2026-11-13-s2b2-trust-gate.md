# S2b-2 — Bizalmi szintes publish gate megvalósítási terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A meglévő verification gate backward kompatibilis bővítése a jóváhagyott bizalmi mátrixszal: TRUST_LEVEL szabály provider-függetlenséggel, kétforrásos odds-szabály mezőszintű trust_state-tel, ops.source_trust tábla.

**Architecture:** A domain tiszta trust/odds típusokat kap; a verification crate TRUST_LEVEL szabálya a SINGLE_SOURCE helyébe lép, az evaluate_odds_trust külön függvény (NEM Finding); a 0006 migráció source_trust táblát és composite odds PK-t ad; a storage LATERAL joinnal választ odds sort meccsenként; a worker/CLI hívók betöltik a trust/odds adatokat és a gate után persistálják az odds trust_state-et.

**Tech Stack:** Rust (edition 2024, rust-version 1.85), sqlx 0.8 (PostgreSQL 18), tokio, chrono, uuid v7, Swift (SwiftUI kliens Core, Codable) — új külső függőség NINCS.

**Spec:** `docs/superpowers/specs/2026-11-13-s2b2-trust-gate-design.md`

## Global Constraints

- AGENTS.md §2: **minden task végén STOP** — összefoglaló + bizonyíték, következő task csak felhasználói engedéllyel.
- AGENTS.md §9 + S2a/S2b-1 ruling: **commit NEM része a task-lépéseknek** — commit csak kifejezett felhasználói kérésre; review-csomag munkafa-diffből.
- Munkaág: `feat/s2b2-trust-gate` (a `main` 304f067-ről indul; létrehozása a végrehajtás első lépése, felhasználói jóváhagyással). Nincs git worktree.
- Dokumentumok, hibaüzenetek, CLI-szövegek **magyarul**; kódazonosítók angolul. Magyar szöveg Write/Edit után visszaolvasandó (ékezetek, idegen glyph).
- Kérésfeldolgozásban **nincs `unwrap()`/`expect()`/elnémítás**; tesztkódban megengedett.
- **Élő hálózati kérés 0**; **nyers payload a repóban 0**.
- Tesztek: `TEST_DATABASE_URL` kötelező (`.env`); integrációs tesztek **egy szálon** (`--test-threads=1`); csomag-suiték KÜLÖN futtatva; hivatalos kapu `cargo test --workspace -- --test-threads=1`.
- Migráció: tesztDB → zöld → devDB (dev csak a záró taskban, külön engedéllyel). Hiányzó adat NULL/státusz, soha nem nulla; UTC.
- **Trust default: medium** (trust-sor nélküli forrás). **Odds tolerancia default 0.0** (egzakt). **Provider NULL = önálló csoport; azonos provider = 1 csoport.**
- A gate állapot-logika (`evaluate_publish_state`) és a parser-őrök változatlanok; a shadow ág érintetlen.
- Swift: `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` (Xcode 27.2), Core teszt `swift test` az apps/FootballAppCore-ban.
- Alapellenőrzés: `./scripts/check.sh` + `./scripts/db-reset.sh && ./scripts/db-check.sh`.

## Fájlstruktúra

| Fájl | Felelősség |
|---|---|
| `rust/crates/domain/src/trust.rs` (új) | TrustClass, SourceTrust, OddsValues, SourceOdds, OddsTrustState, provider-csoportosítás |
| `db/migrations/0006_trust_gate.sql` (új) | source_trust + odds composite PK + trust_state |
| `rust/crates/storage/src/repositories/trust.rs` (új) | source_trust CRUD + lekérdezés |
| `rust/crates/storage/src/repositories/day.rs` (módosul) | LATERAL odds join + odds_trust_state |
| `rust/crates/ingest/src/canonicalize.rs` (módosul) | odds ON CONFLICT composite |
| `rust/crates/verification/src/engine.rs` (módosul) | VerificationContext új mezők |
| `rust/crates/verification/src/rules/mod.rs` (módosul) | TrustLevelRule a SingleSource helyébe |
| `rust/crates/verification/src/odds.rs` (új) | evaluate_odds_trust |
| `rust/crates/worker/src/tick.rs` (módosul) | trust/odds betöltés + odds kiértékelés |
| `rust/bins/footballctl/src/main.rs` (módosul) | ingest-fixture trust/odds |
| `rust/crates/api/src/routes/day.rs` (módosul) | odds_trust_state mező |
| `apps/FootballAppCore/Sources/FootballAppCore/DayModels.swift` (módosul) | oddsTrustState optional |
| `docs/operations/trust.md` (új) | trust karbantartás, seed SQL |

---

### Task 1: Domain trust/odds típusok

**Files:**
- Create: `rust/crates/domain/src/trust.rs`
- Modify: `rust/crates/domain/src/lib.rs` (`pub mod trust;`)
- Test: `rust/crates/domain/tests/trust_types.rs`

**Interfaces:**
- Produces:
  - `TrustClass::{High, Medium, DisplayOnly}` + `as_db_str()` (high/medium/display_only) + `from_db_str()`
  - `SourceTrust { source_code: String, domain: String, trust_class: TrustClass, provider: Option<String> }`
  - `OddsValues { home, draw, away, btts_yes, btts_no, over25, under25: f64 }` + `matches_within(&self, other, tolerance) -> bool` (mind a 7 mezőre `(a-b).abs() <= tolerance`)
  - `SourceOdds { source_code: String, provider: Option<String>, odds: OddsValues }`
  - `OddsTrustState::{Verified, Provisional, NoOdds}` + `as_db_str()` (verified/provisional; NoOdds nem persistálódik)
  - `ODDS_TOLERANCE_DEFAULT: f64 = 0.0`
  - `group_by_provider<'a, T>(items: &'a [T], provider_of: impl Fn(&T) -> Option<&str>) -> Vec<Vec<&'a T>>` — None provider → minden elem önálló csoport; Some(p) → azonos p egy csoport; a csoportok az első előfordulás sorrendjében

- [ ] **Step 1: Bukó teszt**

`rust/crates/domain/tests/trust_types.rs`:

```rust
use football_domain::trust::{
    group_by_provider, OddsValues, SourceOdds, TrustClass, ODDS_TOLERANCE_DEFAULT,
};

fn odds(home: f64) -> OddsValues {
    OddsValues { home, draw: 3.4, away: 4.2, btts_yes: 1.72, btts_no: 2.05, over25: 2.1, under25: 1.75 }
}

#[test]
fn trust_class_db_strings() {
    assert_eq!(TrustClass::High.as_db_str(), "high");
    assert_eq!(TrustClass::Medium.as_db_str(), "medium");
    assert_eq!(TrustClass::DisplayOnly.as_db_str(), "display_only");
    assert_eq!(TrustClass::from_db_str("high").unwrap(), TrustClass::High);
    assert!(TrustClass::from_db_str("unknown").is_err());
}

#[test]
fn odds_matching_tolerance_boundary() {
    let a = odds(1.85);
    assert!(a.matches_within(&odds(1.85), 0.0));
    assert!(!a.matches_within(&odds(1.86), 0.0));
    assert!(a.matches_within(&odds(1.86), 0.01));   // pontosan a tolerance
    assert!(!a.matches_within(&odds(1.87), 0.01));
    assert_eq!(ODDS_TOLERANCE_DEFAULT, 0.0);
}

#[test]
fn provider_grouping_null_is_independent() {
    let items = vec![
        SourceOdds { source_code: "a".into(), provider: None, odds: odds(1.0) },
        SourceOdds { source_code: "b".into(), provider: None, odds: odds(1.0) },
        SourceOdds { source_code: "c".into(), provider: Some("p".into()), odds: odds(1.0) },
        SourceOdds { source_code: "d".into(), provider: Some("p".into()), odds: odds(1.0) },
    ];
    let groups = group_by_provider(&items, |s| s.provider.as_deref());
    assert_eq!(groups.len(), 3); // a, b, {c,d}
    assert_eq!(groups[2].len(), 2);
}
```

- [ ] **Step 2: Futtatás — bukás**

Run: `cd rust && cargo test -p football-domain --test trust_types`
Expected: FAIL — `trust` modul nincs.

- [ ] **Step 3: Implementáció**

`rust/crates/domain/src/trust.rs`:

```rust
pub const ODDS_TOLERANCE_DEFAULT: f64 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustClass { High, Medium, DisplayOnly }

impl TrustClass {
    pub fn as_db_str(self) -> &'static str {
        match self { Self::High => "high", Self::Medium => "medium", Self::DisplayOnly => "display_only" }
    }
    pub fn from_db_str(s: &str) -> Result<Self, String> {
        match s {
            "high" => Ok(Self::High), "medium" => Ok(Self::Medium),
            "display_only" => Ok(Self::DisplayOnly),
            other => Err(format!("ismeretlen trust osztály: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceTrust {
    pub source_code: String,
    pub domain: String,
    pub trust_class: TrustClass,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OddsValues {
    pub home: f64, pub draw: f64, pub away: f64,
    pub btts_yes: f64, pub btts_no: f64, pub over25: f64, pub under25: f64,
}

impl OddsValues {
    pub fn matches_within(&self, other: &OddsValues, tolerance: f64) -> bool {
        let pairs = [
            (self.home, other.home), (self.draw, other.draw), (self.away, other.away),
            (self.btts_yes, other.btts_yes), (self.btts_no, other.btts_no),
            (self.over25, other.over25), (self.under25, other.under25),
        ];
        pairs.iter().all(|(a, b)| (a - b).abs() <= tolerance)
    }
}

#[derive(Debug, Clone)]
pub struct SourceOdds { pub source_code: String, pub provider: Option<String>, pub odds: OddsValues }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OddsTrustState { Verified, Provisional, NoOdds }

impl OddsTrustState {
    pub fn as_db_str(self) -> &'static str {
        match self { Self::Verified => "verified", Self::Provisional | Self::NoOdds => "provisional" }
    }
}

pub fn group_by_provider<'a, T>(
    items: &'a [T],
    provider_of: impl Fn(&T) -> Option<&str>,
) -> Vec<Vec<&'a T>> {
    let mut groups: Vec<Vec<&'a T>> = Vec::new();
    let mut named: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for item in items {
        match provider_of(item) {
            None => groups.push(vec![item]),
            Some(p) => match named.get(p) {
                Some(&idx) => groups[idx].push(item),
                None => { named.insert(p, groups.len()); groups.push(vec![item]); }
            },
        }
    }
    groups
}
```

`lib.rs`: `pub mod trust;`.

- [ ] **Step 4: Futtatás — zöld**

Run: `cd rust && cargo test -p football-domain --test trust_types && cargo fmt --all --check && cargo clippy -p football-domain --all-targets -- -D warnings`
Expected: 3 teszt PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 2: 0006 migráció + kísérő fájlok

**Files:**
- Create: `db/migrations/0006_trust_gate.sql`
- Modify: `rust/crates/storage/src/test_support.rs`, `rust/crates/storage/tests/connection.rs`, `scripts/db-check.sh`
- Test: `rust/crates/storage/tests/trust_schema.rs`

**Interfaces:**
- Consumes: Task 1 db-stringek (CHECK készletek).
- Produces: `ops.source_trust` tábla; `fact.odds_snapshot` composite PK + trust_state; test_pool truncatálja a source_trust-ot; connection 33; db-check per-DB (dev 32 a záró taskig, test 33).

- [ ] **Step 1: Bukó séma-teszt**

```rust
use football_storage::test_support::test_pool;

#[tokio::test]
async fn trust_schema_and_odds_composite_pk() {
    let pool = test_pool().await;
    let t: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname='ops' AND tablename='source_trust'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(t, 1);
    // composite PK: két forrás egy meccsen megfér
    let pk: String = sqlx::query_scalar(
        "SELECT string_agg(a.attname, ',' ORDER BY a.attnum)
         FROM pg_index i JOIN pg_attribute a ON a.attrelid=i.indrelid AND a.attnum=ANY(i.indkey)
         WHERE i.indrelid='fact.odds_snapshot'::regclass AND i.indisprimary")
        .fetch_one(&pool).await.unwrap();
    assert!(pk.contains("match_id") && pk.contains("source_id"));
    // trust_state CHECK
    let bad = sqlx::query(
        "INSERT INTO ops.source_trust (source_id, domain, trust_class)
         VALUES (gen_random_uuid(),'result','unknown')")
        .execute(&pool).await;
    assert!(bad.is_err());
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-storage --test trust_schema -- --test-threads=1` → FAIL.

- [ ] **Step 3: Migráció + kísérők**

`db/migrations/0006_trust_gate.sql`:

```sql
-- 0006 — bizalmi szintes gate (S2b-2): source_trust + odds composite PK + trust_state.
-- Visszaállítási út: scripts/db-reset.sh (tesztDB); dev DB-n: odds_snapshot PK
-- visszaállítása match_id-re (composite sorok deduplikálandók), trust_state drop,
-- source_trust drop. A meglévő odds sorok megmaradnak.

BEGIN;

CREATE TABLE ops.source_trust (
    source_id uuid NOT NULL REFERENCES ops.source(id),
    domain text NOT NULL,
    trust_class text NOT NULL CHECK (trust_class IN ('high','medium','display_only')),
    provider text,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source_id, domain)
);

CREATE INDEX idx_source_trust_source ON ops.source_trust (source_id);

ALTER TABLE fact.odds_snapshot DROP CONSTRAINT odds_snapshot_pkey;
ALTER TABLE fact.odds_snapshot ADD PRIMARY KEY (match_id, source_id);
ALTER TABLE fact.odds_snapshot ADD COLUMN trust_state text NOT NULL DEFAULT 'provisional'
    CHECK (trust_state IN ('verified','provisional'));

COMMIT;
```

Megjegyzés: a PK constraint neve PG-default `odds_snapshot_pkey` — implementáció előtt `\d fact.odds_snapshot`-tal ellenőrizendő; ha eltér, a tényleges név használandó.

`test_support.rs` TRUNCATE-listához (az `ops.source` elé): `ops.source_trust,`
`connection.rs`: `assert_eq!(tables, 33, "a 0001–0006 migráció után 32 terméktábla és a _sqlx_migrations várható");`
`db-check.sh`: per-DB — `DATABASE_URL` → 32 (záró taskig), `TEST_DATABASE_URL` → 33; magyar komment frissítve; záró újsor.

- [ ] **Step 4: Futtatás — zöld + db-check**

Run: `cd rust && cargo test -p football-storage --test trust_schema -- --test-threads=1`
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev OK 32, test OK 33/0.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 3: Storage trust repo + odds/day módosítások

**Files:**
- Create: `rust/crates/storage/src/repositories/trust.rs`
- Modify: `rust/crates/storage/src/repositories/mod.rs`, `day.rs`, `rust/crates/ingest/src/canonicalize.rs`
- Test: `rust/crates/storage/tests/trust_repos.rs`, `rust/crates/storage/tests/day_query.rs` (bővül)

**Interfaces:**
- Consumes: Task 1 típusok, Task 2 táblák.
- Produces:
  - `trust::upsert_trust(pool, source_id: Uuid, domain: &str, trust_class: TrustClass, provider: Option<&str>) -> Result<()>` (ON CONFLICT (source_id, domain) DO UPDATE)
  - `trust::trust_for_sources(pool, source_codes: &[String]) -> Result<Vec<SourceTrust>>` (JOIN ops.source ON code; domainenkénti sorok)
  - `trust::load_match_odds(pool, match_id: Uuid) -> Result<Vec<SourceOdds>>` (odds_snapshot JOIN ops.source + LEFT JOIN source_trust odds domainra a providerért; odds jsonb → OddsValues)
  - `trust::update_odds_trust_state(pool, match_id: Uuid, state: OddsTrustState) -> Result<u64>` (UPDATE fact.odds_snapshot SET trust_state WHERE match_id)
  - `day::DayMatchRow` + `odds_trust_state: Option<String>`; a fetch_day LATERAL joinnal
  - canonicalize odds insert: `ON CONFLICT (match_id, source_id) DO UPDATE SET captured_at, market_count, odds, trust_state='provisional'`

- [ ] **Step 1: Bukó tesztek**

trust_repos.rs: upsert + trust_for_sources (két forrás, domainenként), load_match_odds (két forrás oddsai + provider), update_odds_trust_state (2 sor → verified). day_query.rs bővítés: két odds sor egy meccsen → a fetch_day EGY sort ad vissza, a verified-et választja (trust_state='verified' előny).

```rust
#[tokio::test]
async fn day_picks_verified_odds_row() {
    let pool = test_pool().await;
    // seed: meccs + gate + két odds sor (egy verified, egy provisional)
    // fetch_day → 1 sor, odds_trust_state == "verified"
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-storage --test trust_repos -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció**

`trust.rs` — a fenti interfészek SQLx-szel. A load_match_odds odds jsonb → OddsValues:

```rust
let v: serde_json::Value = row.try_get("odds")?;
let field = |k: &str| -> anyhow::Result<f64> {
    v[k].as_f64().ok_or_else(|| anyhow::anyhow!("hiányzó odds mező: {k}"))
};
let odds = OddsValues {
    home: field("home")?, draw: field("draw")?, away: field("away")?,
    btts_yes: field("btts_yes")?, btts_no: field("btts_no")?,
    over25: field("over25")?, under25: field("under25")?,
};
```

Megjegyzés: a hiányzó odds mező hibát ad (nem nullát) — a hiányzó adat NULL elve szerint a load_match_odds fail-closed; a canonicalize által írt jsonb mindig tartalmazza a 7 mezőt.

`day.rs` LATERAL join (a meglévő LEFT JOIN fact.odds_snapshot helyébe):

```sql
LEFT JOIN LATERAL (
    SELECT o.odds, o.market_count, o.source_id, o.captured_at, o.trust_state
    FROM fact.odds_snapshot o
    WHERE o.match_id = m.id
    ORDER BY (o.trust_state = 'verified') DESC, o.source_id
    LIMIT 1
) o ON true
LEFT JOIN ops.source os ON os.id = o.source_id
```

A SELECT listához: `o.trust_state AS odds_trust_state`. DayMatchRow + `pub odds_trust_state: Option<String>`.

`canonicalize.rs`: az odds INSERT `ON CONFLICT (match_id)` → `ON CONFLICT (match_id, source_id)`, a DO UPDATE SET trust_state='provisional' (új odds sor mindig provisional, a gate után értékelődik).

- [ ] **Step 4: Futtatás — zöld.** Run: `cd rust && cargo test -p football-storage -- --test-threads=1 && cargo test -p football-ingest -- --test-threads=1 && cargo clippy -p football-storage -p football-ingest --all-targets -- -D warnings` → PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 4: Verification — TRUST_LEVEL + evaluate_odds_trust

**Files:**
- Modify: `rust/crates/verification/src/engine.rs`, `rules/mod.rs`, `lib.rs`
- Create: `rust/crates/verification/src/odds.rs`
- Test: `rust/crates/verification/tests/trust_rules.rs` (új), `tests/rules.rs` (SINGLE_SOURCE → TRUST_LEVEL frissítés)

**Interfaces:**
- Consumes: Task 1 típusok.
- Produces:
  - `VerificationContext` új mezők: `source_trust: &'a [SourceTrust]`, `odds_values: &'a [SourceOdds]`, `odds_tolerance: f64`
  - `TrustLevelRule` (key "TRUST_LEVEL") a `SingleSourceWarningRule` helyébe a `default_rules()`-ben
  - `odds::evaluate_odds_trust(odds_values: &[SourceOdds], tolerance: f64) -> OddsTrustState`

- [ ] **Step 1: Bukó tesztek**

trust_rules.rs — a TRUST_LEVEL minden ága (context helperrel):

```rust
// helper: ctx(source_codes, trust sorok) → VerificationContext minimális snapshot-tal
#[test] fn high_alone_no_finding() { /* footballdata high result → 0 TRUST_LEVEL finding */ }
#[test] fn medium_one_group_warning() { /* 1 medium → 1 warning, field "result" */ }
#[test] fn medium_two_independent_no_finding() { /* 2 medium NULL provider → 0 */ }
#[test] fn same_provider_one_group_warning() { /* 2 medium azonos provider → 1 warning */ }
#[test] fn display_only_not_counted() { /* display_only hozzájáruló → nem számít, 1 medium marad → warning */ }
#[test] fn missing_trust_defaults_medium() { /* üres source_trust → medium default */ }
#[test] fn both_domains_evaluated() { /* fixture/kickoff és result külön finding */ }
```

odds tesztek: 2 független egyező → Verified; 2 eltérő → Provisional; 1 → Provisional; üres → NoOdds; tolerance határeset.

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-verification --test trust_rules -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció**

`engine.rs` VerificationContext bővítés (3 új mező). A meglévő tesztek context-konstrukcióit frissíteni kell (üres slice-ok + 0.0 default).

`rules/mod.rs` — TrustLevelRule:

```rust
pub struct TrustLevelRule;
impl VerificationRule for TrustLevelRule {
    fn key(&self) -> &'static str { "TRUST_LEVEL" }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let mut findings = vec![];
        for domain in ["fixture/kickoff", "result"] {
            let contributors: Vec<&str> = c.source_values.iter()
                .map(|s| s.source_code.as_str()).collect();
            if contributors.is_empty() { continue; }
            let mut has_high = false;
            let mut mediums: Vec<(&str, Option<&str>)> = vec![];
            for code in &contributors {
                let class = c.source_trust.iter()
                    .find(|t| t.source_code == *code && t.domain == domain)
                    .map(|t| t.trust_class)
                    .unwrap_or(TrustClass::Medium); // default medium
                match class {
                    TrustClass::High => has_high = true,
                    TrustClass::Medium => {
                        let provider = c.source_trust.iter()
                            .find(|t| t.source_code == *code && t.domain == domain)
                            .and_then(|t| t.provider.as_deref());
                        mediums.push((code, provider));
                    }
                    TrustClass::DisplayOnly => {} // nem számít
                }
            }
            if has_high { continue; }
            if mediums.is_empty() { continue; } // csak display_only hozzájárulók → nem ront (spec §3.1/6)
            // provider-csoportok: None → önálló, Some(p) → azonos p egy csoport
            let mut named: std::collections::HashSet<&str> = Default::default();
            let mut independent_count = 0usize;
            for (_, provider) in &mediums {
                match provider {
                    None => independent_count += 1,
                    Some(p) => { if named.insert(p) { independent_count += 1; } }
                }
            }
            if independent_count < 2 {
                findings.push(Finding::warning("TRUST_LEVEL", domain,
                    "Egy független forrás (közepes bizalom) — provisional."));
            }
        }
        findings
    }
}
```

A `default_rules()`-ben `SingleSourceWarningRule` → `TrustLevelRule`; a SingleSourceWarningRule struct törlődik. A meglévő rules.rs tesztek SINGLE_SOURCE assertjei TRUST_LEVEL-re frissítendők.

`odds.rs`:

```rust
use football_domain::trust::{group_by_provider, OddsTrustState, SourceOdds};

pub fn evaluate_odds_trust(odds_values: &[SourceOdds], tolerance: f64) -> OddsTrustState {
    if odds_values.is_empty() { return OddsTrustState::NoOdds; }
    let groups = group_by_provider(odds_values, |s| s.provider.as_deref());
    if groups.len() < 2 { return OddsTrustState::Provisional; }
    // képviselő: csoport első forrása source_code szerint rendezve
    let mut reps: Vec<&OddsValues> = groups.iter().map(|g| {
        let mut sorted: Vec<&&SourceOdds> = g.iter().collect();
        sorted.sort_by(|a, b| a.source_code.cmp(&b.source_code));
        &sorted[0].odds
    }).collect();
    let first = reps[0];
    if reps.iter().all(|r| first.matches_within(r, tolerance)) {
        OddsTrustState::Verified
    } else {
        OddsTrustState::Provisional
    }
}
```

- [ ] **Step 4: Futtatás — zöld.** Run: `cd rust && cargo test -p football-verification -- --test-threads=1 && cargo fmt --all --check && cargo clippy -p football-verification --all-targets -- -D warnings` → PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 5: Hívók — worker tick + footballctl + E2E

**Files:**
- Modify: `rust/crates/worker/src/tick.rs`, `rust/bins/footballctl/src/main.rs`, `rust/crates/worker/Cargo.toml` (ha kell)
- Test: `rust/crates/worker/tests/trust_e2e.rs` (új)

**Interfaces:**
- Consumes: Task 3 trust repo, Task 4 engine/odds.
- Produces: a verify_and_publish (tick + CLI) trust/odds betöltéssel és gate utáni odds trust_state persistálással; E2E bizonyíték.

- [ ] **Step 1: Bukó E2E tesztek**

trust_e2e.rs (az S2a/S2b-1 tick_e2e helper-mintáján):

```rust
#[tokio::test]
async fn demo_two_medium_sources_verified_with_verified_odds() {
    // két demo forrás (medium seed, NULL provider) → 20/20 verified_publish
    // odds_snapshot trust_state = 'verified' mindkét soron
}

#[tokio::test]
async fn single_medium_source_provisional() {
    // egy forrás (medium) → provisional_publish, TRUST_LEVEL warning a blockersben
}

#[tokio::test]
async fn high_source_alone_verified() {
    // trust seed: high result + fixture/kickoff → egymagában verified_publish
}

#[tokio::test]
async fn differing_odds_provisional_but_match_verified() {
    // két forrás eltérő odds → odds trust_state provisional, meccs verified_publish
}

#[tokio::test]
async fn shadow_unchanged() {
    // shadow → nincs kanonikus, nincs odds trust_state frissítés
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-worker --test trust_e2e -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció**

A `verify_and_publish` (tick.rs ÉS footballctl main.rs IngestFixture ág) bővítése — a materialize után, a context előtt:

```rust
let source_codes: Vec<String> = /* a fixture.source_code + a meccs forrásai */;
let source_trust = football_storage::repositories::trust::trust_for_sources(pool, &source_codes).await?;
// meccsenként:
let odds_values = football_storage::repositories::trust::load_match_odds(pool, materialized.match_id).await?;
let context = VerificationContext {
    // ... meglévő mezők ...
    source_trust: &source_trust,
    odds_values: &odds_values,
    odds_tolerance: football_domain::trust::ODDS_TOLERANCE_DEFAULT,
};
// engine + persist + gate (változatlan)
// gate után:
let odds_state = football_verification::odds::evaluate_odds_trust(&odds_values, context.odds_tolerance);
if odds_state != OddsTrustState::NoOdds {
    football_storage::repositories::trust::update_odds_trust_state(pool, materialized.match_id, odds_state).await?;
}
```

Megjegyzés: a source_codes a fixture.source_code (egy fixture = egy forrás); a kétforrású demo két ingest-fixture hívásból áll (a tick E2E két fixture-t ingestál). A trust_for_sources a meccs source_values forráskódjait is tartalmazza.

- [ ] **Step 4: Futtatás — zöld + regresszió**

Run: `cd rust && cargo test -p football-worker -- --test-threads=1` (külön!), majd `cargo test --workspace -- --test-threads=1`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
Expected: trust_e2e 5/5, a meglévő worker/ingest/storage/verification tesztek zöldek (a SINGLE_SOURCE→TRUST_LEVEL frissítésekkel).

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 6: API + Swift kliens modell

**Files:**
- Modify: `rust/crates/api/src/routes/day.rs`, `apps/FootballAppCore/Sources/FootballAppCore/DayModels.swift`
- Test: `rust/crates/api/tests/routes.rs` (bővül), `apps/FootballAppCore/Tests/FootballAppCoreTests/DayModelsTests.swift` (bővül)

**Interfaces:**
- Consumes: Task 3 day repo odds_trust_state.
- Produces: `/v1/day` odds_trust_state mező; Swift DayMatch.oddsTrustState optional.

- [ ] **Step 1: Bukó tesztek**

api routes.rs: a /v1/day válasz odds objektumában megjelenik az `odds_trust_state` (verified a demo pároknál). Swift DayModelsTests: decode JSON `odds_trust_state: "verified"` → oddsTrustState == "verified"; hiányzó mező → nil (backward compat).

```swift
@Test func decodesOddsTrustState() throws {
    let json = """
    { ... "odds_trust_state": "verified" ... }
    """
    let match = try JSONDecoder().decode(DayMatch.self, from: Data(json.utf8))
    #expect(match.oddsTrustState == "verified")
}
@Test func missingOddsTrustStateIsNil() throws {
    // odds_trust_state nélküli JSON → nil, nem bukik
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-api --test routes -- --test-threads=1` → FAIL; Swift: `cd apps/FootballAppCore && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test` → FAIL.

- [ ] **Step 3: Implementáció**

api day.rs: a DayMatch struct + `pub odds_trust_state: Option<String>` + a row→response mappingben `odds_trust_state: row.odds_trust_state`.

Swift DayModels.swift: DayMatch + `public let oddsTrustState: String?` + CodingKeys `case oddsTrustState = "odds_trust_state"`. Megjegyzés: a Swift Decodable alapból figyelmen kívül hagyja az ismeretlen kulcsokat → a régi kliens nem bukik az új mezőn; az optional hozzáadás additív.

- [ ] **Step 4: Futtatás — zöld.** Run: `cd rust && cargo test -p football-api -- --test-threads=1 && cargo clippy -p football-api --all-targets -- -D warnings`; Swift: `cd apps/FootballAppCore && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test` → PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 7: Zárás — operations doc + dev migráció + ellenőrzések

**Files:**
- Create: `docs/operations/trust.md`
- Modify: `scripts/db-check.sh` (dev 32→33), `docs/checkpoints/2026-11-13/checkpoint.md` (append)

- [ ] **Step 1: Operations doc** — `docs/operations/trust.md` (magyar): a trust tábla karbantartása (upsert SQL), seed SQL a demo forrásokra (medium, NULL provider, fixture/kickoff + result + odds domainek) és a katalógus igazolt forrásaira (footballdata high fixture/kickoff+result, thefa high fixture/kickoff, ESPN/FotMob/OpenFoot/Sky/Tipplap medium, display-only domainek display_only); provider szabály (NULL önálló, azonos provider 1 csoport); odds szabály (2 független egyező ±tolerance → verified, default 0.0); visszaállítási út (0006 rollback: PK visszaállítás deduppal, trust_state drop, source_trust drop).

- [ ] **Step 2: Dev migráció (külön felhasználói engedéllyel)** — `cd rust && cargo run -q -p footballctl -- migrate` → dev 33 tábla; db-check.sh dev 32→33.

- [ ] **Step 3: Záró ellenőrzések**

Run: `./scripts/check.sh` → zöld (pontos szám a checkpointba).
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 33, test 33/0/0.
Run: `cd rust && cargo run -q -p footballctl -- day --date 2026-11-11` → 20 meccs verified_publish + odds_trust_state verified.
Swift Core: `swift test` zöld.

- [ ] **Step 4: Magyar visszaolvasás + glyph-scan** minden új/módosult dokumentumon; záró újsor.

- [ ] **Step 5: Checkpoint záró szekció (append-only)** — taskok + bizonyítékok, deferred minorok, visszaállítási út.

- [ ] **Step 6: STOP — zárójelentés + commit-kérés.** Commit csak a felhasználó kifejezett kérésére.

---

## Önreview-jegyzet (a terv írójának)

- **Spec-lefedettség:** §2 architektúra → Task 1/3/4; §3 adatmodell → Task 2/3; §3.1 trust-feloldás → Task 4; §3.2 odds → Task 4/5; §4 adatfolyam → Task 5; §4.2 API/Swift → Task 6; §5 fail-closed → Task 4/5 tesztjei; §6 tesztek → minden task + Task 7; §7 DoD → Task 7.
- **Típus-konzisztencia:** TrustClass/SourceTrust/OddsValues/SourceOdds/OddsTrustState minden taskban azonos; a repo-függvénynevek a Task 3 interfész-blokkjából jönnek; a VerificationContext új mezői a Task 4-ben definiáltak, a Task 5 hívók ugyanazokat használják.
- **Ismert kockázat (kezelt):** a composite odds PK a day repo JOIN-ját duplikálná → LATERAL join (Task 3); a Swift Decodable nem strict → az új mező additív (Task 6); a demo odds párok egzakt egyezők (mérve: 20/20, 0 eltérés).
