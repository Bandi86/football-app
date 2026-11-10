# Ellenőrzött napi lista backend — Megvalósítási terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Egy működő Rust + PostgreSQL vertikális szelet, amely fixture-forrásból nyers bizonyítékot, normalizált megfigyelést, kanonikus meccset, többrétegű ellenőrzési leleteket és publikálási kaput készít, majd a helyi API-n csak ellenőrzött napi listát szolgál ki.

**Architecture:** Domain-first moduláris monolit. A `domain` tiszta invariánsokat tartalmaz; a `storage` repository-kat; az `ingest` fixture-forrást dolgoz fel; a `verification` független szabályokat és kaput futtat; az `api` localhost HTTP-t ad. Ebben az első szeletben a use case-eket a `footballctl` CLI és a szűk API-handlerek fogják össze; a külön `service` crate a következő tervben emeli ki őket változatlan szerződések mögé. A terv nem tartalmaz élő scrapinget, SwiftUI klienst, predikciót vagy botot.

**Tech Stack:** Rust stable, edition 2024; Tokio; Axum 0.8; SQLx 0.8 PostgreSQL-lel; PostgreSQL 18; Serde; UUID v7; Chrono + Chrono-TZ; SHA-256; Zstandard; Clap; Tracing.

**Spec:** `docs/superpowers/specs/2026-11-09-football-app-v2-design.md`

## Programterv bontása

A spec több önálló alrendszert fed, ezért külön tervekben valósul meg:

1. **Ez a terv:** Rust alap + ellenőrzött napi lista backend.
2. SwiftUI macOS kliens és a napi lista/ellenőrzés alatt nézet.
3. Teljes identitáskezelés, forrásminősítés és forrásbővítés.
4. Meccs-, csapat-, játékos- és versenyoldal.
5. Live eseményfolyam.
6. Predikciós szerződés, kalibráció és value.
7. Papírportfóliók, szelvények és settlement.
8. Ügynökök, autonómia és fejlett üzemeltetés.

Ez a terv az 1. pontot valósítja meg önállóan futtatható és tesztelhető módon.

## Global Constraints

- Helyi, egygépes rendszer; az API kizárólag `127.0.0.1`-en hallgat.
- A PostgreSQL és a tartalom-azonosítós nyers objektumtár együtt az igazság forrása.
- Hiányzó adat nem lehet nulla; a `NULL` és a státuszok külön jelentéssel bírnak.
- Automatikus entitás-összevonás tilos; ebben a szeletben csak pontos külső ID-kapcsolat engedett.
- Bizonytalan csapatnév nem oldható fel fuzzy egyezéssel.
- A napi lista alapértelmezésben csak `verified_publish` elemet ad vissza.
- Minden kanonikus tényhez provenance tartozik.
- Minden időpont UTC-ben tárolt `timestamptz`; a nap szerinti csoportosítás alapértelmezett időzónája `Europe/Budapest`.
- A fixture adapter csak teszt-/demóforrás; valódi forrás nem használhat előre adott kanonikus ID-t.
- Az olvasási és üzemeltetési token külön token; a tokenek a futásidejű állapotfájlban, `0600` jogosultsággal élnek.
- Új production szabály csak mért shadow-hatás után lehet aktív; ebben a szeletben a bootstrap szabályok a fixture-ökön teszteltek.
- Nincs `git init`, staging vagy commit e terv végrehajtásakor, kivéve ha a felhasználó külön kéri. A repó jelenleg nem git repository; a commitlépések ezért szándékosan hiányoznak.
- Minden feladat végén zöld formázás, fordítás és releváns teszt szükséges.
- Ne legyen production `unwrap()` kérésfeldolgozásban; a hibák tipizáltak és naplózottak.

---

## Fájlszerkezet

```text
football-app-v2/
  .env.example
  .gitignore
  rust-toolchain.toml
  rust/
    Cargo.toml
    crates/
      domain/
        Cargo.toml
        src/lib.rs
        src/match_state.rs
        src/verification.rs
      storage/
        Cargo.toml
        src/lib.rs
        src/test_support.rs
        src/object_store.rs
        src/repositories/mod.rs
        src/repositories/raw.rs
        src/repositories/core.rs
        src/repositories/identity.rs
        src/repositories/verification.rs
        src/repositories/day.rs
      ingest/
        Cargo.toml
        src/lib.rs
        src/fixture.rs
        src/canonicalize.rs
      verification/
        Cargo.toml
        src/lib.rs
        src/engine.rs
        src/rules/mod.rs
        src/persist.rs
        src/gate.rs
        src/conflict.rs
      api/
        Cargo.toml
        src/lib.rs
        src/auth.rs
        src/error.rs
        src/routes/mod.rs
        src/routes/health.rs
        src/routes/day.rs
        src/routes/verification.rs
    bins/
      footballctl/
        Cargo.toml
        src/main.rs
      footballd/
        Cargo.toml
        src/main.rs
        src/runtime_file.rs
  db/migrations/
    0001_raw_stage_ops.sql
    0002_core_identity_verification.sql
  tests/fixtures/
    fixture_a_day.json
    fixture_b_confirm.json
    fixture_c_conflict.json
  scripts/
    check-toolchain.sh
    db-create.sh
    db-reset.sh
    check.sh
  docs/operations/
    local-development.md
```

A fájlok felelőssége:

- `domain`: entitás- és állapot-invariánsok, nem IO.
- `storage`: SQL és objektumtár; nem dönt publikálásról.
- `ingest`: fixture beolvasás, nyers rögzítés, normalizálás, kanonikus írás.
- `verification`: szabályok, leletek, publikálási állapot.
- `api`: HTTP, tokenellenőrzés, DTO-k.
- `footballctl`: migráció, fixture-betöltés, napi lista CLI-ből, diagnosztika.
- `footballd`: API daemon és runtime tokenfájl.

---

### Task 1: Eszközlánc és repository-alap

**Files:**
- Create: `.gitignore`
- Create: `.env.example`
- Create: `rust-toolchain.toml`
- Create: `scripts/check-toolchain.sh`
- Create: `scripts/db-create.sh`
- Test: a két script sikeres futása

**Interfaces:**
- Consumes: nincs.
- Produces: `cargo`, `sqlx`, `psql` elérhető; `football_v2_dev` és `football_v2_test` adatbázis; a későbbi feladatok `DATABASE_URL` és `TEST_DATABASE_URL` változót használnak.

- [ ] **Step 1: Ellenőrizd és telepítsd az eszközöket**

A gépen a Rust hiányzik, a PostgreSQL 18.6 és a Swift 6.4.2 megvan, a teljes Xcode viszont ehhez a backend-tervhez nem szükséges.

Futtatandó, amikor a végrehajtásra engedélyt adsz:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
cargo install sqlx-cli --no-default-features --features postgres,rustls
```

Elvárt: `cargo --version` és `sqlx --version` sikeres.

- [ ] **Step 2: Írd meg a `.gitignore` fájlt**

```gitignore
/target/
/rust/target/
**/target/
.env
.env.*
!.env.example
/data/
*.log
.DS_Store
DerivedData/
```

- [ ] **Step 3: Írd meg a `.env.example` fájlt**

```dotenv
DATABASE_URL=postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_dev
TEST_DATABASE_URL=postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test
APP_TIMEZONE=Europe/Budapest
DATA_DIR=./data
BIND_ADDR=127.0.0.1:0
```

- [ ] **Step 4: Írd meg a `rust-toolchain.toml` fájlt**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 5: Írd meg a `scripts/check-toolchain.sh` fájlt**

```bash
#!/usr/bin/env bash
set -euo pipefail

for cmd in cargo rustc sqlx psql pg_config; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    printf 'HIÁNYZÓ ESZKÖZ: %s\n' "$cmd" >&2
    exit 1
  fi
done

printf 'cargo: %s\n' "$(cargo --version)"
printf 'rustc: %s\n' "$(rustc --version)"
printf 'sqlx:  %s\n' "$(sqlx --version)"
printf 'psql:  %s\n' "$(psql --version)"
```

- [ ] **Step 6: Írd meg a `scripts/db-create.sh` fájlt**

```bash
#!/usr/bin/env bash
set -euo pipefail

psql -d postgres -v ON_ERROR_STOP=1 <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'football_v2') THEN
    CREATE ROLE football_v2 LOGIN CREATEDB PASSWORD 'LOCAL_DB_PASSWORD';
  END IF;
END
$$;
SQL

for db in football_v2_dev football_v2_test; do
  if ! psql -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = '${db}'" | grep -q 1; then
    createdb -O football_v2 "${db}"
    printf 'Létrehozva: %s\n' "${db}"
  else
    printf 'Már létezik: %s\n' "${db}"
  fi
done
```

- [ ] **Step 7: Futtasd az ellenőrzéseket**

```bash
chmod +x scripts/check-toolchain.sh scripts/db-create.sh
./scripts/check-toolchain.sh
./scripts/db-create.sh
psql 'postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_dev' -c 'SELECT current_database();'
```

Elvárt: minden eszköz verziója megjelenik; a PostgreSQL válaszban `football_v2_dev` szerepel.

---

### Task 2: Rust workspace és domain-szerződések

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/crates/domain/Cargo.toml`
- Create: `rust/crates/domain/src/lib.rs`
- Create: `rust/crates/domain/src/match_state.rs`
- Create: `rust/crates/domain/src/verification.rs`
- Test: `rust/crates/domain/tests/domain_contracts.rs`

**Interfaces:**
- Consumes: nincs.
- Produces:
  - `football_domain::MatchStatus`
  - `football_domain::MatchResultSnapshot`
  - `football_domain::MatchSnapshot`
  - `football_domain::PublishState`
  - `football_domain::VerificationStatus`
  - `football_domain::Severity`
  - `football_domain::Finding`
  - `football_domain::VerificationReport`
  - `football_domain::result_consistency_findings(&MatchSnapshot) -> Vec<Finding>`

- [ ] **Step 1: Írd meg a workspace `Cargo.toml` fájlját**

```toml
[workspace]
resolver = "3"
members = ["crates/*", "bins/*"]

[workspace.package]
edition = "2024"
rust-version = "1.85"
version = "0.1.0"

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
axum = "0.8"
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.10"
clap = { version = "4", features = ["derive"] }
dirs = "6"
hex = "0.4"
rand = "0.9"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "json", "migrate", "macros"] }
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tower = { version = "0.5", features = ["util"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
uuid = { version = "1", features = ["v4", "v7", "serde"] }
zstd = "0.13"
tempfile = "3"
```

- [ ] **Step 2: Írd meg először a bukó domain-teszteket**

`rust/crates/domain/tests/domain_contracts.rs`:

```rust
use chrono::{TimeZone, Utc};
use football_domain::{
    evaluate_publish_state, result_consistency_findings, Finding, MatchResultSnapshot,
    MatchSnapshot, MatchStatus, PublishState, Severity, VerificationReport, VerificationStatus,
};

fn snapshot(status: MatchStatus, result: Option<MatchResultSnapshot>) -> MatchSnapshot {
    MatchSnapshot {
        id: uuid::Uuid::nil(),
        home_team_id: uuid::Uuid::nil(),
        away_team_id: uuid::Uuid::nil(),
        kickoff_at: Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
        status,
        result,
    }
}

#[test]
fn scheduled_match_without_score_is_valid() {
    assert!(result_consistency_findings(&snapshot(MatchStatus::Scheduled, None)).is_empty());
}

#[test]
fn finished_match_without_score_is_blocked() {
    let findings = result_consistency_findings(&snapshot(MatchStatus::Finished, None));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_key, "MATCH_RESULT_REQUIRED");
    assert_eq!(findings[0].severity, Severity::Blocker);
}

#[test]
fn half_time_score_cannot_exceed_full_time_score() {
    let result = MatchResultSnapshot {
        home_ft: Some(1),
        away_ft: Some(1),
        home_ht: Some(2),
        away_ht: Some(0),
        home_et: None,
        away_et: None,
        home_pens: None,
        away_pens: None,
    };
    let findings = result_consistency_findings(&snapshot(MatchStatus::Finished, Some(result)));
    assert!(findings.iter().any(|f| f.rule_key == "HT_GREATER_THAN_FT"));
}

#[test]
fn blocker_report_withholds_publication() {
    let report = VerificationReport {
        findings: vec![Finding::blocker(
            "TEAM_IDENTITY_UNRESOLVED",
            "home_team_id",
            "A hazai csapat identitása nem megerősített.",
        )],
    };
    assert_eq!(report.status(), VerificationStatus::Blocked);
    assert_eq!(evaluate_publish_state(&report), PublishState::Withheld);
}

#[test]
fn warning_only_report_is_provisional() {
    let report = VerificationReport {
        findings: vec![Finding::warning(
            "SINGLE_SOURCE",
            "status",
            "A meccsállapot csak egy forrásból származik.",
        )],
    };
    assert_eq!(report.status(), VerificationStatus::Provisional);
    assert_eq!(evaluate_publish_state(&report), PublishState::ProvisionalPublish);
    assert!(!PublishState::ProvisionalPublish.default_visible());
}

#[test]
fn empty_report_is_verified_and_visible() {
    let report = VerificationReport { findings: vec![] };
    assert_eq!(report.status(), VerificationStatus::Verified);
    assert_eq!(evaluate_publish_state(&report), PublishState::VerifiedPublish);
    assert!(PublishState::VerifiedPublish.default_visible());
}
```

- [ ] **Step 3: Futtasd a teszteket, és igazold a hiányzó API-t**

```bash
cd rust
cargo test -p football-domain
```

Elvárt: fordítási hiba, mert a `football-domain` crate és a típusok még nem léteznek.

- [ ] **Step 4: Írd meg a domain crate manifestjét**

`rust/crates/domain/Cargo.toml`:

```toml
[package]
name = "football-domain"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
chrono.workspace = true
serde.workspace = true
uuid.workspace = true
```

- [ ] **Step 5: Írd meg a domain típusokat**

`rust/crates/domain/src/lib.rs`:

```rust
pub mod match_state;
pub mod verification;

pub use match_state::{result_consistency_findings, MatchResultSnapshot, MatchSnapshot, MatchStatus};
pub use verification::{
    evaluate_publish_state, Finding, PublishState, Severity, VerificationReport,
    VerificationStatus,
};
```

`rust/crates/domain/src/match_state.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::verification::{Finding, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Scheduled,
    Confirmed,
    Live,
    Halftime,
    Finished,
    Postponed,
    Canceled,
    Abandoned,
    Awarded,
}

impl MatchStatus {
    pub fn requires_result(self) -> bool {
        matches!(self, Self::Finished | Self::Awarded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchResultSnapshot {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnapshot {
    pub id: Uuid,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub status: MatchStatus,
    pub result: Option<MatchResultSnapshot>,
}

pub fn result_consistency_findings(snapshot: &MatchSnapshot) -> Vec<Finding> {
    let Some(result) = snapshot.result.as_ref() else {
        if snapshot.status.requires_result() {
            return vec![Finding::blocker(
                "MATCH_RESULT_REQUIRED",
                "result",
                "Lezárt meccshez hiányzik az érvényes eredmény.",
            )];
        }
        return vec![];
    };

    let mut findings = vec![];

    if snapshot.status.requires_result()
        && (result.home_ft.is_none() || result.away_ft.is_none())
    {
        findings.push(Finding::blocker(
            "MATCH_RESULT_REQUIRED",
            "result",
            "Lezárt meccshez hiányzik a rendes játékidejű eredmény.",
        ));
    }

    if let (Some(ht_home), Some(ft_home)) = (result.home_ht, result.home_ft)
        && ht_home > ft_home
    {
        findings.push(Finding::blocker(
            "HT_GREATER_THAN_FT",
            "home_ht",
            "A hazai félidőeredmény nem lehet nagyobb a rendes játékidejű eredménynél.",
        ));
    }

    if let (Some(ht_away), Some(ft_away)) = (result.away_ht, result.away_ft)
        && ht_away > ft_away
    {
        findings.push(Finding::blocker(
            "HT_GREATER_THAN_FT",
            "away_ht",
            "A vendég félidőeredmény nem lehet nagyobb a rendes játékidejű eredménynél.",
        ));
    }

    if (result.home_pens.is_some() || result.away_pens.is_some())
        && (result.home_et.is_none() || result.away_et.is_none())
    {
        findings.push(Finding::blocker(
            "PENS_WITHOUT_ET",
            "penalties",
            "Tizenegyeseredmény hosszabbítási eredmény nélkül nem értelmezhető.",
        ));
    }

    findings
        .into_iter()
        .map(|mut finding| {
            if finding.severity != Severity::Blocker {
                finding.severity = Severity::Blocker;
            }
            finding
        })
        .collect()
}
```

`rust/crates/domain/src/verification.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Blocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Provisional,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishState {
    VerifiedPublish,
    ProvisionalPublish,
    Withheld,
    Quarantined,
}

impl PublishState {
    pub fn default_visible(self) -> bool {
        self == Self::VerifiedPublish
    }

    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::VerifiedPublish => "verified_publish",
            Self::ProvisionalPublish => "provisional_publish",
            Self::Withheld => "withheld",
            Self::Quarantined => "quarantined",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub rule_key: &'static str,
    pub severity: Severity,
    pub field: &'static str,
    pub message: String,
}

impl Finding {
    pub fn blocker(
        rule_key: &'static str,
        field: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_key,
            severity: Severity::Blocker,
            field,
            message: message.into(),
        }
    }

    pub fn warning(
        rule_key: &'static str,
        field: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_key,
            severity: Severity::Warning,
            field,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub findings: Vec<Finding>,
}

impl VerificationReport {
    pub fn status(&self) -> VerificationStatus {
        if self.findings.iter().any(|f| f.severity == Severity::Blocker) {
            VerificationStatus::Blocked
        } else if self
            .findings
            .iter()
            .any(|f| f.severity == Severity::Warning)
        {
            VerificationStatus::Provisional
        } else {
            VerificationStatus::Verified
        }
    }
}

pub fn evaluate_publish_state(report: &VerificationReport) -> PublishState {
    match report.status() {
        VerificationStatus::Blocked => PublishState::Withheld,
        VerificationStatus::Provisional => PublishState::ProvisionalPublish,
        VerificationStatus::Verified => PublishState::VerifiedPublish,
        VerificationStatus::Unverified => PublishState::Withheld,
    }
}
```

- [ ] **Step 6: Futtasd a domain-teszteket**

```bash
cd rust
cargo fmt --all --check
cargo test -p football-domain
```

Elvárt: minden domain-teszt zöld.

---

### Task 3: Nyers, stage és üzemeltetési séma

**Files:**
- Create: `db/migrations/0001_raw_stage_ops.sql`
- Create: `scripts/db-reset.sh`
- Test: `scripts/db-reset.sh` + `sqlx migrate run`

**Interfaces:**
- Consumes: helyi PostgreSQL adatbázis.
- Produces: `ops.source`, `raw.artifact`, `raw.capture`, `stage.observation`, `ops.validation_rule`, `ops.validation_run`, `ops.validation_finding`, `ops.audit_event` táblák.

- [ ] **Step 1: Írd meg a nyers/stage/ops migrációt**

```sql
BEGIN;

CREATE SCHEMA IF NOT EXISTS raw;
CREATE SCHEMA IF NOT EXISTS stage;
CREATE SCHEMA IF NOT EXISTS ops;

CREATE TABLE ops.source (
    id uuid PRIMARY KEY,
    code text NOT NULL UNIQUE,
    display_name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE raw.artifact (
    id uuid PRIMARY KEY,
    content_sha256 text NOT NULL UNIQUE,
    compression text NOT NULL CHECK (compression IN ('none', 'zstd')),
    byte_size bigint NOT NULL CHECK (byte_size >= 0),
    storage_key text NOT NULL UNIQUE,
    media_type text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE raw.capture (
    id uuid PRIMARY KEY,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    artifact_id uuid NOT NULL REFERENCES raw.artifact(id),
    endpoint_key text NOT NULL,
    request_url text NOT NULL,
    requested_at timestamptz NOT NULL,
    received_at timestamptz NOT NULL,
    http_status smallint NOT NULL CHECK (http_status BETWEEN 100 AND 599),
    adapter_version text NOT NULL,
    parser_version text NOT NULL,
    request_key text NOT NULL,
    error_code text,
    UNIQUE (source_id, request_key)
);

CREATE TABLE stage.observation (
    id uuid PRIMARY KEY,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    capture_id uuid NOT NULL REFERENCES raw.capture(id),
    entity_kind text NOT NULL,
    aspect text NOT NULL,
    natural_key text NOT NULL,
    source_record_key text NOT NULL,
    observed_at timestamptz NOT NULL,
    event_time timestamptz,
    normalized_payload jsonb NOT NULL,
    schema_version text NOT NULL,
    quality_flags text[] NOT NULL DEFAULT '{}',
    content_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_id, entity_kind, aspect, source_record_key, content_hash)
);

CREATE INDEX idx_observation_match_aspect
    ON stage.observation (entity_kind, aspect, natural_key, observed_at DESC);

CREATE TABLE ops.validation_rule (
    id uuid PRIMARY KEY,
    rule_key text NOT NULL UNIQUE,
    version text NOT NULL,
    default_severity text NOT NULL CHECK (default_severity IN ('info', 'warning', 'blocker')),
    active boolean NOT NULL DEFAULT false,
    description text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE ops.validation_run (
    id uuid PRIMARY KEY,
    subject_kind text NOT NULL,
    subject_id uuid NOT NULL,
    rule_set_version text NOT NULL,
    started_at timestamptz NOT NULL,
    finished_at timestamptz,
    outcome text NOT NULL CHECK (outcome IN ('verified', 'provisional', 'blocked')),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE ops.validation_finding (
    id uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES ops.validation_run(id) ON DELETE CASCADE,
    rule_key text NOT NULL,
    severity text NOT NULL CHECK (severity IN ('info', 'warning', 'blocker')),
    field_key text NOT NULL,
    message text NOT NULL,
    observation_id uuid REFERENCES stage.observation(id),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_validation_finding_run ON ops.validation_finding (run_id);

CREATE TABLE ops.audit_event (
    id uuid PRIMARY KEY,
    actor text NOT NULL,
    action text NOT NULL,
    subject_kind text NOT NULL,
    subject_id uuid,
    detail jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO ops.validation_rule (id, rule_key, version, default_severity, active, description)
VALUES
    (gen_random_uuid(), 'SINGLE_SOURCE', '1.0', 'warning', true, 'A meccs csak egy forrásból megerősített.'),
    (gen_random_uuid(), 'KICKOFF_VALID', '1.0', 'blocker', true, 'A kickoff érvényes, értelmezhető időpont.'),
    (gen_random_uuid(), 'EXACT_IDENTITY', '1.0', 'blocker', true, 'A csapat- és versenyidentitás pontos külső ID-val megerősített.'),
    (gen_random_uuid(), 'DUPLICATE_NATURAL_KEY', '1.0', 'blocker', true, 'Nincs duplikált meccs a természetes kulcs alapján.'),
    (gen_random_uuid(), 'MATCH_RESULT_CONSISTENT', '1.0', 'blocker', true, 'Az eredmény és a meccsállapot konzisztens.'),
    (gen_random_uuid(), 'MATERIAL_SOURCE_CONFLICT', '1.0', 'blocker', true, 'Nincs anyagi forráskonfliktus.');

COMMIT;
```

- [ ] **Step 2: Írd meg a `scripts/db-reset.sh` fájlt**

```bash
#!/usr/bin/env bash
set -euo pipefail

: "${TEST_DATABASE_URL:?TEST_DATABASE_URL hiányzik}"

psql "$TEST_DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
DROP SCHEMA IF EXISTS raw CASCADE;
DROP SCHEMA IF EXISTS stage CASCADE;
DROP SCHEMA IF EXISTS core CASCADE;
DROP SCHEMA IF EXISTS identity CASCADE;
DROP SCHEMA IF EXISTS fact CASCADE;
DROP SCHEMA IF EXISTS ops CASCADE;
SQL

DATABASE_URL="$TEST_DATABASE_URL" sqlx migrate run --source db/migrations
```

- [ ] **Step 3: Futtasd a migrációt a fejlesztői adatbázison**

```bash
export DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_dev'
sqlx database create
sqlx migrate run --source db/migrations
psql "$DATABASE_URL" -c '\dt raw.*'
psql "$DATABASE_URL" -c '\dt stage.*'
psql "$DATABASE_URL" -c '\dt ops.*'
```

Elvárt: a felsorolt táblák léteznek, és az `ops.validation_rule` hat sort tartalmaz.

- [ ] **Step 4: Ellenőrizd a tesztadatbázis resetjét**

```bash
chmod +x scripts/db-reset.sh
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
./scripts/db-reset.sh
psql "$TEST_DATABASE_URL" -tAc "SELECT count(*) FROM ops.validation_rule;"
```

Elvárt: `6`.

---

### Task 4: Core, identitás, provenance és publikálási séma

**Files:**
- Create: `db/migrations/0002_core_identity_verification.sql`
- Test: a migráció fut, a constraint-ek elutasítják a hibás sorokat

**Interfaces:**
- Consumes: Task 3 sémái.
- Produces: `core.entity`, `core.country`, `core.competition`, `core.season`, `core.team`, `core.match`, `core.match_result`, `identity.source_entity_ref`, `fact.provenance`, `fact.verification`, `ops.publish_gate`.

- [ ] **Step 1: Írd meg a core/identitás migrációt**

```sql
BEGIN;

CREATE SCHEMA IF NOT EXISTS core;
CREATE SCHEMA IF NOT EXISTS identity;
CREATE SCHEMA IF NOT EXISTS fact;

CREATE TABLE core.entity (
    id uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('team', 'player', 'competition', 'venue', 'manager', 'referee')),
    canonical_name text NOT NULL,
    merged_into_id uuid REFERENCES core.entity(id),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE core.country (
    id uuid PRIMARY KEY,
    code text NOT NULL UNIQUE,
    name text NOT NULL
);

CREATE TABLE core.competition (
    id uuid PRIMARY KEY,
    entity_id uuid NOT NULL UNIQUE REFERENCES core.entity(id),
    country_id uuid REFERENCES core.country(id),
    name text NOT NULL
);

CREATE TABLE core.season (
    id uuid PRIMARY KEY,
    competition_id uuid NOT NULL REFERENCES core.competition(id),
    label text NOT NULL,
    starts_on date,
    ends_on date,
    UNIQUE (competition_id, label)
);

CREATE TABLE core.team (
    id uuid PRIMARY KEY,
    entity_id uuid NOT NULL UNIQUE REFERENCES core.entity(id),
    country_id uuid REFERENCES core.country(id),
    name text NOT NULL,
    gender text NOT NULL CHECK (gender IN ('male', 'female', 'unknown')),
    age_group text NOT NULL DEFAULT 'senior',
    branch text NOT NULL DEFAULT 'senior' CHECK (branch IN ('senior', 'reserve', 'youth', 'women')),
    active boolean NOT NULL DEFAULT true
);

CREATE TABLE core.match (
    id uuid PRIMARY KEY,
    competition_id uuid NOT NULL REFERENCES core.competition(id),
    season_id uuid NOT NULL REFERENCES core.season(id),
    round_label text,
    home_team_id uuid NOT NULL REFERENCES core.team(id),
    away_team_id uuid NOT NULL REFERENCES core.team(id),
    kickoff_at timestamptz NOT NULL,
    competition_timezone text NOT NULL,
    status text NOT NULL CHECK (status IN (
        'scheduled', 'confirmed', 'live', 'halftime', 'finished',
        'postponed', 'canceled', 'abandoned', 'awarded'
    )),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (home_team_id <> away_team_id)
);

CREATE UNIQUE INDEX uq_match_natural_key
    ON core.match (competition_id, season_id, round_label, home_team_id, away_team_id, kickoff_at);

CREATE INDEX idx_match_kickoff ON core.match (kickoff_at);

CREATE TABLE core.match_result (
    match_id uuid PRIMARY KEY REFERENCES core.match(id) ON DELETE CASCADE,
    home_ft smallint CHECK (home_ft >= 0),
    away_ft smallint CHECK (away_ft >= 0),
    home_ht smallint CHECK (home_ht >= 0),
    away_ht smallint CHECK (away_ht >= 0),
    home_et smallint CHECK (home_et >= 0),
    away_et smallint CHECK (away_et >= 0),
    home_pens smallint CHECK (home_pens >= 0),
    away_pens smallint CHECK (away_pens >= 0),
    winner_method text CHECK (winner_method IN ('regular', 'extra_time', 'penalties', 'awarded')),
    validity text NOT NULL DEFAULT 'valid' CHECK (validity IN ('valid', 'void', 'awarded', 'abandoned')),
    decided_at timestamptz,
    reason text
);

CREATE TABLE identity.source_entity_ref (
    id uuid PRIMARY KEY,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    entity_kind text NOT NULL,
    source_entity_id text NOT NULL,
    entity_id uuid NOT NULL,
    method text NOT NULL CHECK (method IN ('exact_external_id', 'manual_review', 'fixture_seed')),
    confidence numeric(4,3) NOT NULL DEFAULT 1.000 CHECK (confidence BETWEEN 0 AND 1),
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'superseded', 'rejected')),
    reviewed_by text,
    evidence_observation_id uuid REFERENCES stage.observation(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_id, entity_kind, source_entity_id)
);

CREATE TABLE fact.provenance (
    subject_kind text NOT NULL,
    subject_id uuid NOT NULL,
    field_key text NOT NULL,
    observation_id uuid NOT NULL REFERENCES stage.observation(id),
    is_selected boolean NOT NULL,
    policy_version text NOT NULL,
    valid_from timestamptz NOT NULL,
    valid_to timestamptz,
    PRIMARY KEY (subject_kind, subject_id, field_key, observation_id)
);

CREATE UNIQUE INDEX uq_provenance_selected_field
    ON fact.provenance (subject_kind, subject_id, field_key)
    WHERE is_selected;

CREATE TABLE fact.conflict (
    id uuid PRIMARY KEY,
    subject_kind text NOT NULL,
    subject_id uuid NOT NULL,
    field_key text NOT NULL,
    severity text NOT NULL CHECK (severity IN ('benign', 'material')),
    status text NOT NULL CHECK (status IN ('unresolved', 'resolved_auto', 'resolved_manual')),
    winner_observation_id uuid REFERENCES stage.observation(id),
    opened_at timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz,
    resolution_reason text,
    UNIQUE (subject_kind, subject_id, field_key)
);

CREATE TABLE fact.verification (
    subject_kind text NOT NULL,
    subject_id uuid NOT NULL,
    field_key text NOT NULL,
    rule_set_version text NOT NULL,
    status text NOT NULL CHECK (status IN ('unverified', 'verified', 'provisional', 'blocked')),
    severity text NOT NULL CHECK (severity IN ('info', 'warning', 'blocker')),
    last_run_id uuid REFERENCES ops.validation_run(id),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (subject_kind, subject_id, field_key)
);

CREATE TABLE ops.publish_gate (
    id uuid PRIMARY KEY,
    match_id uuid NOT NULL REFERENCES core.match(id),
    publish_date date NOT NULL,
    state text NOT NULL CHECK (state IN (
        'verified_publish', 'provisional_publish', 'withheld', 'quarantined'
    )),
    rule_set_version text NOT NULL,
    blockers jsonb NOT NULL DEFAULT '[]'::jsonb,
    evaluated_at timestamptz NOT NULL,
    UNIQUE (match_id, publish_date)
);

CREATE INDEX idx_publish_gate_date_state ON ops.publish_gate (publish_date, state);

COMMIT;
```

- [ ] **Step 2: Futtasd a migrációt**

```bash
export DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_dev'
sqlx migrate run --source db/migrations
psql "$DATABASE_URL" -c '\dt core.*'
psql "$DATABASE_URL" -c '\dt identity.*'
psql "$DATABASE_URL" -c '\dt fact.*'
```

Elvárt: minden tábla létrejön.

- [ ] **Step 3: Ellenőrizd a védelmi constraint-eket**

```bash
psql "$DATABASE_URL" -v ON_ERROR_STOP=0 -c \
"INSERT INTO core.match (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at, competition_timezone, status)
 VALUES (gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), now(), 'Europe/Budapest', 'scheduled');"
```

Elvárt: idegen kulcs hiba, azaz a hibás meccs nem írható be. A parancs szándékosan bukik.

- [ ] **Step 4: Állítsd vissza és ellenőrizd a tesztadatbázist**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
./scripts/db-reset.sh
psql "$TEST_DATABASE_URL" -tAc "SELECT count(*) FROM information_schema.tables WHERE table_schema IN ('core','identity','fact');"
```

Elvárt: `11` tábla (`core`, `identity` és `fact` sémákban).

---

### Task 5: Objektumtár és tesztadatbázis-helper

**Files:**
- Create: `rust/crates/storage/Cargo.toml`
- Create: `rust/crates/storage/src/lib.rs`
- Create: `rust/crates/storage/src/object_store.rs`
- Create: `rust/crates/storage/src/test_support.rs`
- Create: `rust/crates/storage/src/repositories/mod.rs`
- Create: `rust/crates/storage/src/repositories/raw.rs`
- Create: `rust/crates/storage/src/repositories/core.rs`
- Create: `rust/crates/storage/src/repositories/identity.rs`
- Create: `rust/crates/storage/src/repositories/verification.rs`
- Create: `rust/crates/storage/src/repositories/day.rs`
- Test: `rust/crates/storage/tests/object_store.rs`

**Interfaces:**
- Consumes: `DATA_DIR`, `TEST_DATABASE_URL`.
- Produces:
  - `football_storage::ObjectStore::put_json(&[u8]) -> StoredArtifact`
  - `ObjectStore::get(&StoredArtifact) -> Vec<u8>`
  - `StoredArtifact { sha256: String, storage_key: String, byte_size: u64, compression: &'static str }`
  - `football_storage::test_support::test_pool() -> PgPool`
  - `reset_database(&PgPool)`

- [ ] **Step 1: Írd meg a bukó objektumtár-tesztet**

```rust
use football_storage::object_store::ObjectStore;

#[test]
fn identical_payload_is_stored_once_and_round_trips() {
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let payload = br#"{"match":"A-1001"}"#;

    let first = store.put_json(payload).unwrap();
    let second = store.put_json(payload).unwrap();

    assert_eq!(first.sha256, second.sha256);
    assert_eq!(first.storage_key, second.storage_key);
    assert_eq!(store.get(&first).unwrap(), payload);
    assert_eq!(std::fs::read_dir(temp.path().join("raw")).unwrap().count(), 1);
}
```

- [ ] **Step 2: Futtasd, és igazold a hibát**

```bash
cd rust
cargo test -p football-storage
```

Elvárt: a crate még nem létezik.

- [ ] **Step 3: Írd meg a storage manifestet és az objektumtárat**

`rust/crates/storage/Cargo.toml`:

```toml
[package]
name = "football-storage"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
chrono.workspace = true
hex.workspace = true
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true
sqlx.workspace = true
uuid.workspace = true
zstd.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio.workspace = true
```

`rust/crates/storage/src/object_store.rs`:

```rust
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredArtifact {
    pub sha256: String,
    pub storage_key: String,
    pub byte_size: u64,
    pub compression: &'static str,
}

pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().join("raw");
        fs::create_dir_all(&root).context("nyers objektumtár létrehozása")?;
        Ok(Self { root })
    }

    pub fn put_json(&self, payload: &[u8]) -> Result<StoredArtifact> {
        let sha256 = hex::encode(Sha256::digest(payload));
        let relative = format!("{}/{}/{}.json.zst", &sha256[0..2], &sha256[2..4], sha256);
        let path = self.root.join(&relative);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let compressed = zstd::encode_all(payload, 3)?;
            let temp = path.with_extension("tmp");
            let mut file = fs::File::create(&temp)?;
            file.write_all(&compressed)?;
            file.sync_all()?;
            fs::rename(temp, &path)?;
        }

        Ok(StoredArtifact {
            sha256,
            storage_key: relative,
            byte_size: payload.len() as u64,
            compression: "zstd",
        })
    }

    pub fn get(&self, artifact: &StoredArtifact) -> Result<Vec<u8>> {
        let path = self.root.join(&artifact.storage_key);
        let compressed = fs::read(path)?;
        Ok(zstd::decode_all(compressed.as_slice())?)
    }
}
```

`rust/crates/storage/src/lib.rs`:

```rust
pub mod object_store;
pub mod repositories;
pub mod test_support;
```

`rust/crates/storage/src/repositories/mod.rs`:

```rust
pub mod core;
pub mod day;
pub mod identity;
pub mod raw;
pub mod verification;
```

A `repositories` modulok ebben a feladatban még üres fájlok lehetnek, de a Task 6–10 kitölti őket:

```rust
// raw.rs, core.rs, identity.rs, verification.rs, day.rs
```

- [ ] **Step 4: Írd meg a tesztadatbázis-helpert**

`rust/crates/storage/src/test_support.rs`:

```rust
use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn test_pool() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL kötelező az integrációs tesztekhez");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("tesztadatbázis kapcsolat");

    sqlx::migrate!("../../../db/migrations")
        .run(&pool)
        .await
        .expect("teszt migrációk");

    sqlx::query(
        "TRUNCATE ops.publish_gate, fact.verification, fact.conflict, fact.provenance,
         identity.source_entity_ref, core.match_result, core.match, core.team,
         core.season, core.competition, core.country, core.entity,
         stage.observation, raw.capture, raw.artifact, ops.source,
         ops.validation_finding, ops.validation_run, ops.validation_rule,
         ops.audit_event RESTART IDENTITY CASCADE",
    )
    .execute(&pool)
    .await
    .expect("tesztadatok törlése");

    sqlx::query(
        "INSERT INTO ops.validation_rule
         (id, rule_key, version, default_severity, active, description)
         VALUES
         (gen_random_uuid(), 'SINGLE_SOURCE', '1.0', 'warning', true, 'Egy forrás'),
         (gen_random_uuid(), 'KICKOFF_VALID', '1.0', 'blocker', true, 'Kickoff'),
         (gen_random_uuid(), 'EXACT_IDENTITY', '1.0', 'blocker', true, 'Identitás'),
         (gen_random_uuid(), 'DUPLICATE_NATURAL_KEY', '1.0', 'blocker', true, 'Duplikátum'),
         (gen_random_uuid(), 'MATCH_RESULT_CONSISTENT', '1.0', 'blocker', true, 'Eredmény'),
         (gen_random_uuid(), 'MATERIAL_SOURCE_CONFLICT', '1.0', 'blocker', true, 'Konfliktus')
         ON CONFLICT (rule_key) DO NOTHING",
    )
    .execute(&pool)
    .await
    .expect("szabályok");

    pool
}
```

- [ ] **Step 5: Futtasd az objektumtár- és sématesztet**

```bash
cd rust
cargo fmt --all --check
cargo test -p football-storage --test object_store
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
```

Elvárt: az objektumtár teszt zöld, egyetlen fájl keletkezik, és a payload visszaolvasható.

---

### Task 6: Fixture-forrás, nyers rögzítés és megfigyelés

**Files:**
- Create: `tests/fixtures/fixture_a_day.json`
- Create: `rust/crates/ingest/Cargo.toml`
- Create: `rust/crates/ingest/src/lib.rs`
- Create: `rust/crates/ingest/src/fixture.rs`
- Modify: `rust/crates/storage/src/repositories/raw.rs`
- Test: `rust/crates/ingest/tests/capture_observation.rs`

**Interfaces:**
- Consumes: `ObjectStore`, `PgPool`.
- Produces:
  - `FixtureAdapter::ingest(&PgPool, &ObjectStore, &Path) -> IngestReport`
  - `IngestReport { capture_id, observation_ids: Vec<Uuid>, source_id: Uuid }`
  - `FixtureSource` serde modell.

- [ ] **Step 1: Írd meg a fixture JSON-t**

`tests/fixtures/fixture_a_day.json`:

```json
{
  "source_code": "fixture_a",
  "source_display_name": "Fixture A",
  "captured_at": "2026-11-09T08:00:00Z",
  "endpoint_key": "day-fixtures",
  "request_url": "fixture://fixture_a/day/2026-11-09",
  "adapter_version": "fixture-a-1.0.0",
  "parser_version": "fixture-parser-1.0.0",
  "competition": {
    "id": "10000000-0000-7000-8000-000000000001",
    "name": "Hungarian NB I",
    "country_code": "HU",
    "country_name": "Hungary"
  },
  "season": {
    "id": "10000000-0000-7000-8000-000000000002",
    "label": "2026/27"
  },
  "teams": [
    {
      "id": "10000000-0000-7000-8000-000000000011",
      "source_entity_id": "A-UJP",
      "name": "Újpest",
      "country_code": "HU",
      "gender": "male",
      "age_group": "senior",
      "branch": "senior"
    },
    {
      "id": "10000000-0000-7000-8000-000000000012",
      "source_entity_id": "A-GYOR",
      "name": "Győr",
      "country_code": "HU",
      "gender": "male",
      "age_group": "senior",
      "branch": "senior"
    }
  ],
  "matches": [
    {
      "id": "10000000-0000-7000-8000-000000000101",
      "source_match_id": "A-1001",
      "home_source_entity_id": "A-UJP",
      "away_source_entity_id": "A-GYOR",
      "round_label": "12",
      "kickoff_at": "2026-11-09T18:00:00Z",
      "competition_timezone": "Europe/Budapest",
      "status": "scheduled",
      "result": null
    }
  ]
}
```

- [ ] **Step 2: Írd meg a bukó capture-tesztet**

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

    let report = FixtureAdapter::ingest(&pool, &store, &path).await.unwrap();

    let artifacts: i64 = sqlx::query_scalar("SELECT count(*) FROM raw.artifact")
        .fetch_one(&pool).await.unwrap();
    let captures: i64 = sqlx::query_scalar("SELECT count(*) FROM raw.capture")
        .fetch_one(&pool).await.unwrap();
    let observations: i64 = sqlx::query_scalar("SELECT count(*) FROM stage.observation")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(artifacts, 1);
    assert_eq!(captures, 1);
    assert_eq!(observations, report.observation_ids.len() as i64);
    assert_eq!(observations, 3);
}
```

- [ ] **Step 3: Futtasd, és igazold a hiányzó adaptert**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo test -p football-ingest --test capture_observation
```

Elvárt: a crate/adapter hiánya miatt bukik.

- [ ] **Step 4: Írd meg az ingest crate manifestjét**

```toml
[package]
name = "football-ingest"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
chrono.workspace = true
football-domain = { path = "../domain" }
football-storage = { path = "../storage" }
hex.workspace = true
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true
sqlx.workspace = true
uuid.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio.workspace = true
```

- [ ] **Step 5: Írd meg a fixture modelleket és a nyers rögzítést**

`rust/crates/ingest/src/lib.rs`:

```rust
pub mod canonicalize;
pub mod fixture;
```

`rust/crates/ingest/src/fixture.rs`:

```rust
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_storage::object_store::ObjectStore;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureCompetition {
    pub id: Uuid,
    pub name: String,
    pub country_code: String,
    pub country_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureSeason {
    pub id: Uuid,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureTeam {
    pub id: Uuid,
    pub source_entity_id: String,
    pub name: String,
    pub country_code: String,
    pub gender: String,
    pub age_group: String,
    pub branch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureResult {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureMatch {
    pub id: Uuid,
    pub source_match_id: String,
    pub home_source_entity_id: String,
    pub away_source_entity_id: String,
    pub round_label: Option<String>,
    pub kickoff_at: DateTime<Utc>,
    pub competition_timezone: String,
    pub status: String,
    pub result: Option<FixtureResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureSource {
    pub source_code: String,
    pub source_display_name: String,
    pub captured_at: DateTime<Utc>,
    pub endpoint_key: String,
    pub request_url: String,
    pub adapter_version: String,
    pub parser_version: String,
    pub competition: FixtureCompetition,
    pub season: FixtureSeason,
    pub teams: Vec<FixtureTeam>,
    pub matches: Vec<FixtureMatch>,
}

#[derive(Debug, Clone)]
pub struct IngestReport {
    pub source_id: Uuid,
    pub capture_id: Uuid,
    pub observation_ids: Vec<Uuid>,
}

pub struct FixtureAdapter;

impl FixtureAdapter {
    pub async fn ingest(pool: &PgPool, store: &ObjectStore, path: &Path) -> Result<IngestReport> {
        let payload = std::fs::read(path)
            .with_context(|| format!("fixture olvasása: {}", path.display()))?;
        let fixture: FixtureSource = serde_json::from_slice(&payload)?;
        let artifact = store.put_json(&payload)?;

        let source_id = upsert_source(
            pool,
            &fixture.source_code,
            &fixture.source_display_name,
        )
        .await?;

        let artifact_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO raw.artifact
             (id, content_sha256, compression, byte_size, storage_key, media_type)
             VALUES ($1, $2, $3, $4, $5, 'application/json')
             ON CONFLICT (content_sha256) DO UPDATE SET content_sha256 = EXCLUDED.content_sha256
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(&artifact.sha256)
        .bind(artifact.compression)
        .bind(artifact.byte_size as i64)
        .bind(&artifact.storage_key)
        .fetch_one(pool)
        .await?;

        let request_key = format!(
            "{}:{}:{}",
            fixture.source_code, fixture.endpoint_key, fixture.captured_at
        );
        let capture_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO raw.capture
             (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
              received_at, http_status, adapter_version, parser_version, request_key)
             VALUES ($1,$2,$3,$4,$5,$6,$7,200,$8,$9,$10)
             ON CONFLICT (source_id, request_key) DO NOTHING",
        )
        .bind(capture_id)
        .bind(source_id)
        .bind(artifact_id)
        .bind(&fixture.endpoint_key)
        .bind(&fixture.request_url)
        .bind(fixture.captured_at)
        .bind(fixture.captured_at)
        .bind(&fixture.adapter_version)
        .bind(&fixture.parser_version)
        .bind(&request_key)
        .execute(pool)
        .await?;

        let capture_id: Uuid = sqlx::query_scalar(
            "SELECT id FROM raw.capture WHERE source_id = $1 AND request_key = $2",
        )
        .bind(source_id)
        .bind(&request_key)
        .fetch_one(pool)
        .await?;

        let mut observation_ids = Vec::new();

        for team in &fixture.teams {
            let observation_id = insert_observation(
                pool,
                source_id,
                capture_id,
                "team",
                "team_identity",
                &team.id.to_string(),
                &team.source_entity_id,
                fixture.captured_at,
                serde_json::to_value(team)?,
            )
            .await?;
            observation_ids.push(observation_id);
        }

        for fixture_match in &fixture.matches {
            let home = fixture
                .teams
                .iter()
                .find(|team| team.source_entity_id == fixture_match.home_source_entity_id)
                .with_context(|| {
                    format!(
                        "ismeretlen hazai forrásentitás: {}",
                        fixture_match.home_source_entity_id
                    )
                })?;
            let away = fixture
                .teams
                .iter()
                .find(|team| team.source_entity_id == fixture_match.away_source_entity_id)
                .with_context(|| {
                    format!(
                        "ismeretlen vendég forrásentitás: {}",
                        fixture_match.away_source_entity_id
                    )
                })?;

            let payload = serde_json::json!({
                "canonical_match_id": fixture_match.id,
                "home_team_id": home.id,
                "away_team_id": away.id,
                "source_home_id": fixture_match.home_source_entity_id,
                "source_away_id": fixture_match.away_source_entity_id,
                "kickoff_at": fixture_match.kickoff_at,
                "status": fixture_match.status,
                "result": fixture_match.result,
                "round_label": fixture_match.round_label,
                "competition_timezone": fixture_match.competition_timezone,
            });
            let observation_id = insert_observation(
                pool,
                source_id,
                capture_id,
                "match",
                "match_core",
                &fixture_match.id.to_string(),
                &fixture_match.source_match_id,
                fixture.captured_at,
                payload,
            )
            .await?;
            observation_ids.push(observation_id);
        }

        Ok(IngestReport {
            source_id,
            capture_id,
            observation_ids,
        })
    }
}

async fn upsert_source(pool: &PgPool, code: &str, name: &str) -> Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name)
         VALUES ($1, $2, $3)
         ON CONFLICT (code) DO UPDATE SET display_name = EXCLUDED.display_name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(code)
    .bind(name)
    .fetch_one(pool)
    .await
    .context("forrás upsert")
}

#[allow(clippy::too_many_arguments)]
async fn insert_observation(
    pool: &PgPool,
    source_id: Uuid,
    capture_id: Uuid,
    entity_kind: &str,
    aspect: &str,
    natural_key: &str,
    source_record_key: &str,
    observed_at: DateTime<Utc>,
    normalized_payload: serde_json::Value,
) -> Result<Uuid> {
    let content_bytes = serde_json::to_vec(&normalized_payload)?;
    let content_hash = hex::encode(sha2::Sha256::digest(&content_bytes));
    let id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key,
          source_record_key, observed_at, event_time, normalized_payload,
          schema_version, quality_flags, content_hash)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8,$9,'1.0','{}',$10)
         ON CONFLICT (source_id, entity_kind, aspect, source_record_key, content_hash)
         DO NOTHING",
    )
    .bind(id)
    .bind(source_id)
    .bind(capture_id)
    .bind(entity_kind)
    .bind(aspect)
    .bind(natural_key)
    .bind(source_record_key)
    .bind(observed_at)
    .bind(normalized_payload)
    .bind(content_hash)
    .execute(pool)
    .await?;

    sqlx::query_scalar(
        "SELECT id FROM stage.observation
         WHERE source_id=$1 AND entity_kind=$2 AND aspect=$3
           AND source_record_key=$4 AND content_hash=$5",
    )
    .bind(source_id)
    .bind(entity_kind)
    .bind(aspect)
    .bind(source_record_key)
    .bind(content_hash)
    .fetch_one(pool)
    .await
    .context("megfigyelés azonosító")
}
```

- [ ] **Step 6: Futtasd a capture-tesztet**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo fmt --all --check
cargo test -p football-ingest --test capture_observation
```

Elvárt: 1 artifact, 1 capture, 2 team-observation és 1 match-observation jön létre; a `report.observation_ids` három azonosítót tartalmaz.

---

### Task 7: Kanonikus entitások, identitás és provenance

**Files:**
- Modify: `rust/crates/ingest/src/canonicalize.rs`
- Modify: `rust/crates/storage/src/repositories/core.rs`
- Modify: `rust/crates/storage/src/repositories/identity.rs`
- Modify: `rust/crates/storage/src/repositories/raw.rs`
- Test: `rust/crates/ingest/tests/canonicalize_fixture.rs`

**Interfaces:**
- Consumes: `FixtureSource`, `stage.observation`.
- Produces:
  - `Canonicalizer::materialize(&PgPool, &FixtureSource) -> MaterializedDay`
  - `MaterializedDay { matches: Vec<MaterializedMatch> }`
  - `MaterializedMatch { match_id, home_team_id, away_team_id, snapshot: MatchSnapshot, observation_ids: Vec<Uuid> }`
  - `identity.source_entity_ref` active sorok minden fixture-csapatra.

- [ ] **Step 1: Írd meg a bukó kanonizálási tesztet**

```rust
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn fixture_materializes_one_canonical_match_with_exact_identity_and_provenance() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &fixture_path)
        .await
        .unwrap();
    let fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();

    let day = Canonicalizer::materialize(&pool, &fixture).await.unwrap();

    assert_eq!(day.matches.len(), 1);
    assert_eq!(day.matches[0].home_team_id.to_string(), "10000000-0000-7000-8000-000000000011");
    assert_eq!(day.matches[0].away_team_id.to_string(), "10000000-0000-7000-8000-000000000012");

    let refs: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.source_entity_ref")
        .fetch_one(&pool).await.unwrap();
    let provenance: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.provenance")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(refs, 2);
    assert!(provenance >= 5);
}
```

- [ ] **Step 2: Futtasd, és igazold a hiányt**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo test -p football-ingest --test canonicalize_fixture
```

Elvárt: `Canonicalizer` hiánya miatt bukik.

- [ ] **Step 3: Írd meg a kanonizálót**

`rust/crates/ingest/src/canonicalize.rs`:

```rust
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_domain::{MatchResultSnapshot, MatchSnapshot, MatchStatus};
use sqlx::PgPool;
use uuid::Uuid;

use crate::fixture::{FixtureSource, FixtureTeam};

pub struct Canonicalizer;

#[derive(Debug, Clone)]
pub struct MaterializedMatch {
    pub match_id: Uuid,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub snapshot: MatchSnapshot,
    pub observation_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct MaterializedDay {
    pub matches: Vec<MaterializedMatch>,
}

impl Canonicalizer {
    pub async fn materialize(pool: &PgPool, fixture: &FixtureSource) -> Result<MaterializedDay> {
        let mut tx = pool.begin().await?;

        let country_id = upsert_country(
            &mut tx,
            &fixture.competition.country_code,
            &fixture.competition.country_name,
        )
        .await?;

        upsert_entity(
            &mut tx,
            fixture.competition.id,
            "competition",
            &fixture.competition.name,
        )
        .await?;

        sqlx::query(
            "INSERT INTO core.competition (id, entity_id, country_id, name)
             VALUES ($1,$1,$2,$3)
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",
        )
        .bind(fixture.competition.id)
        .bind(country_id)
        .bind(&fixture.competition.name)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO core.season (id, competition_id, label)
             VALUES ($1,$2,$3)
             ON CONFLICT (id) DO UPDATE SET label = EXCLUDED.label",
        )
        .bind(fixture.season.id)
        .bind(fixture.competition.id)
        .bind(&fixture.season.label)
        .execute(&mut *tx)
        .await?;

        for team in &fixture.teams {
            upsert_team(&mut tx, team, country_id, fixture.source_code.as_str()).await?;
        }

        let mut matches = Vec::new();

        for fixture_match in &fixture.matches {
            let home = team_by_source_id(&mut tx, &fixture_match.home_source_entity_id).await?;
            let away = team_by_source_id(&mut tx, &fixture_match.away_source_entity_id).await?;
            let status = parse_status(&fixture_match.status)?;

            sqlx::query(
                "INSERT INTO core.match
                 (id, competition_id, season_id, round_label, home_team_id, away_team_id,
                  kickoff_at, competition_timezone, status)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT (id) DO UPDATE SET
                   round_label = EXCLUDED.round_label,
                   home_team_id = EXCLUDED.home_team_id,
                   away_team_id = EXCLUDED.away_team_id,
                   kickoff_at = EXCLUDED.kickoff_at,
                   status = EXCLUDED.status,
                   updated_at = now()",
            )
            .bind(fixture_match.id)
            .bind(fixture.competition.id)
            .bind(fixture.season.id)
            .bind(&fixture_match.round_label)
            .bind(home)
            .bind(away)
            .bind(fixture_match.kickoff_at)
            .bind(&fixture_match.competition_timezone)
            .bind(status_db(status))
            .execute(&mut *tx)
            .await?;

            let result = fixture_match.result.as_ref().map(|r| MatchResultSnapshot {
                home_ft: r.home_ft,
                away_ft: r.away_ft,
                home_ht: r.home_ht,
                away_ht: r.away_ht,
                home_et: r.home_et,
                away_et: r.away_et,
                home_pens: r.home_pens,
                away_pens: r.away_pens,
            });

            persist_result(&mut tx, fixture_match.id, result.as_ref()).await?;

            let observation_ids = observations_for_match(
                &mut tx,
                &fixture.source_code,
                fixture_match.id,
            )
            .await?;

            for field in [
                "home_team_id",
                "away_team_id",
                "kickoff_at",
                "status",
                "result",
            ] {
                if let Some(observation_id) = observation_ids.first() {
                    sqlx::query(
                        "INSERT INTO fact.provenance
                         (subject_kind, subject_id, field_key, observation_id,
                          is_selected, policy_version, valid_from)
                         VALUES (
                           'match',$1,$2,$3,
                           NOT EXISTS (
                             SELECT 1 FROM fact.provenance p
                             WHERE p.subject_kind='match' AND p.subject_id=$1
                               AND p.field_key=$2 AND p.is_selected
                           ),
                           'fixture-1.0',$4
                         )
                         ON CONFLICT (subject_kind, subject_id, field_key, observation_id)
                         DO NOTHING",
                    )
                    .bind(fixture_match.id)
                    .bind(field)
                    .bind(observation_id)
                    .bind(Utc::now())
                    .execute(&mut *tx)
                    .await?;
                }
            }

            matches.push(MaterializedMatch {
                match_id: fixture_match.id,
                home_team_id: home,
                away_team_id: away,
                snapshot: MatchSnapshot {
                    id: fixture_match.id,
                    home_team_id: home,
                    away_team_id: away,
                    kickoff_at: fixture_match.kickoff_at,
                    status,
                    result,
                },
                observation_ids,
            });
        }

        tx.commit().await?;
        Ok(MaterializedDay { matches })
    }
}

async fn upsert_country(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    code: &str,
    name: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name)
         VALUES ($1,$2,$3)
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(code)
    .bind(name)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn upsert_entity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    kind: &str,
    name: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name)
         VALUES ($1,$2,$3)
         ON CONFLICT (id) DO UPDATE SET canonical_name = EXCLUDED.canonical_name",
    )
    .bind(id)
    .bind(kind)
    .bind(name)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_team(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team: &FixtureTeam,
    country_id: Uuid,
    source_code: &str,
) -> Result<()> {
    upsert_entity(tx, team.id, "team", &team.name).await?;

    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,$4,$5,$6)
         ON CONFLICT (id) DO UPDATE SET
           name = EXCLUDED.name,
           country_id = EXCLUDED.country_id,
           gender = EXCLUDED.gender,
           age_group = EXCLUDED.age_group,
           branch = EXCLUDED.branch",
    )
    .bind(team.id)
    .bind(country_id)
    .bind(&team.name)
    .bind(&team.gender)
    .bind(&team.age_group)
    .bind(&team.branch)
    .execute(&mut **tx)
    .await?;

    let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
        .bind(source_code)
        .fetch_one(&mut **tx)
        .await?;

    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'team',$3,$4,'fixture_seed')
         ON CONFLICT (source_id, entity_kind, source_entity_id)
         DO UPDATE SET entity_id = EXCLUDED.entity_id, status = 'active'",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(&team.source_entity_id)
    .bind(team.id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn team_by_source_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_entity_id: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(
        "SELECT entity_id FROM identity.source_entity_ref
         WHERE entity_kind='team' AND source_entity_id=$1 AND status='active'",
    )
    .bind(source_entity_id)
    .fetch_one(&mut **tx)
    .await
    .with_context(|| format!("ismeretlen csapatforrás: {source_entity_id}"))
}

async fn persist_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    result: Option<&MatchResultSnapshot>,
) -> Result<()> {
    let Some(result) = result else {
        sqlx::query("DELETE FROM core.match_result WHERE match_id=$1")
            .bind(match_id)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    };

    sqlx::query(
        "INSERT INTO core.match_result
         (match_id, home_ft, away_ft, home_ht, away_ht, home_et, away_et, home_pens, away_pens)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         ON CONFLICT (match_id) DO UPDATE SET
           home_ft=EXCLUDED.home_ft, away_ft=EXCLUDED.away_ft,
           home_ht=EXCLUDED.home_ht, away_ht=EXCLUDED.away_ht,
           home_et=EXCLUDED.home_et, away_et=EXCLUDED.away_et,
           home_pens=EXCLUDED.home_pens, away_pens=EXCLUDED.away_pens",
    )
    .bind(match_id)
    .bind(result.home_ft)
    .bind(result.away_ft)
    .bind(result.home_ht)
    .bind(result.away_ht)
    .bind(result.home_et)
    .bind(result.away_et)
    .bind(result.home_pens)
    .bind(result.away_pens)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn observations_for_match(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_code: &str,
    match_id: Uuid,
) -> Result<Vec<Uuid>> {
    let rows = sqlx::query_scalar::<_, Uuid>(
        "SELECT o.id
         FROM stage.observation o
         JOIN ops.source s ON s.id = o.source_id
         WHERE s.code = $1 AND o.entity_kind = 'match'
           AND o.aspect = 'match_core' AND o.natural_key = $2
         ORDER BY o.observed_at DESC",
    )
    .bind(source_code)
    .bind(match_id.to_string())
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows)
}

fn parse_status(value: &str) -> Result<MatchStatus> {
    match value {
        "scheduled" => Ok(MatchStatus::Scheduled),
        "confirmed" => Ok(MatchStatus::Confirmed),
        "live" => Ok(MatchStatus::Live),
        "halftime" => Ok(MatchStatus::Halftime),
        "finished" => Ok(MatchStatus::Finished),
        "postponed" => Ok(MatchStatus::Postponed),
        "canceled" => Ok(MatchStatus::Canceled),
        "abandoned" => Ok(MatchStatus::Abandoned),
        "awarded" => Ok(MatchStatus::Awarded),
        other => anyhow::bail!("ismeretlen meccsstátusz: {other}"),
    }
}

fn status_db(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::Scheduled => "scheduled",
        MatchStatus::Confirmed => "confirmed",
        MatchStatus::Live => "live",
        MatchStatus::Halftime => "halftime",
        MatchStatus::Finished => "finished",
        MatchStatus::Postponed => "postponed",
        MatchStatus::Canceled => "canceled",
        MatchStatus::Abandoned => "abandoned",
        MatchStatus::Awarded => "awarded",
    }
}
```

A `Canonicalizer::materialize` a fixture-azonosító alapján kapcsolja a megfigyeléseket; nincs külön `observation_ids` paramétere.

- [ ] **Step 4: Futtasd a kanonizálási tesztet**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo fmt --all --check
cargo test -p football-ingest --test canonicalize_fixture
```

Elvárt: egy kanonikus meccs, két aktív identitáshivatkozás, legalább öt provenance sor.

---

### Task 8: Ellenőrzési motor és determinisztikus szabályok

**Files:**
- Create: `rust/crates/verification/Cargo.toml`
- Create: `rust/crates/verification/src/lib.rs`
- Create: `rust/crates/verification/src/engine.rs`
- Create: `rust/crates/verification/src/rules/mod.rs`
- Test: `rust/crates/verification/tests/rules.rs`

**Interfaces:**
- Consumes: `football_domain` típusai.
- Produces:
  - `VerificationContext<'a>`
  - `VerificationRule` trait
  - `VerificationEngine::run(&[Box<dyn VerificationRule>], &VerificationContext) -> VerificationReport`
  - `default_rules() -> Vec<Box<dyn VerificationRule>>`

- [ ] **Step 1: Írd meg a bukó szabályteszteket**

```rust
use chrono::{TimeZone, Utc};
use football_domain::{MatchResultSnapshot, MatchSnapshot, MatchStatus, Severity};
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::rules::default_rules;
use uuid::Uuid;

fn valid_snapshot() -> MatchSnapshot {
    MatchSnapshot {
        id: Uuid::now_v7(),
        home_team_id: Uuid::now_v7(),
        away_team_id: Uuid::now_v7(),
        kickoff_at: Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
        status: MatchStatus::Finished,
        result: Some(MatchResultSnapshot {
            home_ft: Some(2),
            away_ft: Some(1),
            home_ht: Some(1),
            away_ht: Some(0),
            home_et: None,
            away_et: None,
            home_pens: None,
            away_pens: None,
        }),
    }
}

#[test]
fn valid_match_has_no_blockers() {
    let snapshot = valid_snapshot();
    let context = VerificationContext {
        snapshot: &snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: &[],
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    assert!(report.findings.is_empty());
}

#[test]
fn unresolved_team_identity_is_blocker() {
    let snapshot = valid_snapshot();
    let context = VerificationContext {
        snapshot: &snapshot,
        home_identity_exact: false,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: &[],
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    assert!(report.findings.iter().any(|f| {
        f.rule_key == "EXACT_IDENTITY" && f.severity == Severity::Blocker
    }));
}

#[test]
fn duplicate_natural_key_is_blocker() {
    let snapshot = valid_snapshot();
    let context = VerificationContext {
        snapshot: &snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 1,
        source_values: &[],
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    assert!(report.findings.iter().any(|f| f.rule_key == "DUPLICATE_NATURAL_KEY"));
}
```

- [ ] **Step 2: Futtasd, és igazold a hiányzó crate-ot**

```bash
cd rust
cargo test -p football-verification --test rules
```

Elvárt: a crate és a szabályok hiányoznak.

- [ ] **Step 3: Írd meg a verification crate-ot**

`rust/crates/verification/Cargo.toml`:

```toml
[package]
name = "football-verification"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
chrono.workspace = true
football-domain = { path = "../domain" }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
uuid.workspace = true

[dev-dependencies]
football-ingest = { path = "../ingest" }
football-storage = { path = "../storage" }
tempfile.workspace = true
tokio.workspace = true
```

`rust/crates/verification/src/lib.rs`:

```rust
pub mod engine;
pub mod gate;
pub mod persist;
pub mod rules;
```

`rust/crates/verification/src/engine.rs`:

```rust
use football_domain::{Finding, MatchSnapshot, VerificationReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMaterialValues {
    pub source_code: String,
    pub home_team_id: uuid::Uuid,
    pub away_team_id: uuid::Uuid,
    pub kickoff_at: chrono::DateTime<chrono::Utc>,
    pub status: football_domain::MatchStatus,
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
}

pub struct VerificationContext<'a> {
    pub snapshot: &'a MatchSnapshot,
    pub home_identity_exact: bool,
    pub away_identity_exact: bool,
    pub duplicate_match_count: i64,
    pub source_values: &'a [SourceMaterialValues],
}

pub trait VerificationRule: Send + Sync {
    fn key(&self) -> &'static str;
    fn evaluate(&self, context: &VerificationContext<'_>) -> Vec<Finding>;
}

pub struct VerificationEngine;

impl VerificationEngine {
    pub fn run(
        rules: &[Box<dyn VerificationRule>],
        context: &VerificationContext<'_>,
    ) -> VerificationReport {
        let findings = rules
            .iter()
            .flat_map(|rule| rule.evaluate(context))
            .collect();
        VerificationReport { findings }
    }
}
```

`rust/crates/verification/src/rules/mod.rs`:

```rust
use chrono::{Datelike, Utc};
use football_domain::{Finding, Severity, result_consistency_findings};

use crate::engine::{VerificationContext, VerificationRule};

pub struct ExactIdentityRule;
impl VerificationRule for ExactIdentityRule {
    fn key(&self) -> &'static str { "EXACT_IDENTITY" }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let mut findings = vec![];
        if !c.home_identity_exact {
            findings.push(Finding::blocker("EXACT_IDENTITY", "home_team_id",
                "A hazai csapat identitása nem erősíthető meg pontos külső ID-val."));
        }
        if !c.away_identity_exact {
            findings.push(Finding::blocker("EXACT_IDENTITY", "away_team_id",
                "A vendég csapat identitása nem erősíthető meg pontos külső ID-val."));
        }
        findings
    }
}

pub struct DuplicateNaturalKeyRule;
impl VerificationRule for DuplicateNaturalKeyRule {
    fn key(&self) -> &'static str { "DUPLICATE_NATURAL_KEY" }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        if c.duplicate_match_count > 0 {
            vec![Finding::blocker(
                "DUPLICATE_NATURAL_KEY",
                "match",
                "A meccs természetes kulcsa duplikált rekordot talál.",
            )]
        } else {
            vec![]
        }
    }
}

pub struct KickoffValidRule;
impl VerificationRule for KickoffValidRule {
    fn key(&self) -> &'static str { "KICKOFF_VALID" }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let year = c.snapshot.kickoff_at.year();
        if !(1900..=2100).contains(&year) {
            vec![Finding::blocker("KICKOFF_VALID", "kickoff_at",
                "A kickoff időpontja értelmetlen tartományba esik.")]
        } else if c.snapshot.kickoff_at > Utc::now() + chrono::Duration::days(730) {
            vec![Finding::warning("KICKOFF_VALID", "kickoff_at",
                "A kickoff több mint két évre a jövőben van.")]
        } else {
            vec![]
        }
    }
}

pub struct ResultConsistencyRule;
impl VerificationRule for ResultConsistencyRule {
    fn key(&self) -> &'static str { "MATCH_RESULT_CONSISTENT" }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        result_consistency_findings(c.snapshot)
    }
}

pub struct MaterialConflictRule;
impl VerificationRule for MaterialConflictRule {
    fn key(&self) -> &'static str { "MATERIAL_SOURCE_CONFLICT" }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        if c.source_values.len() < 2 {
            return vec![];
        }
        let first = &c.source_values[0];
        let conflicting = c.source_values.iter().skip(1).any(|other| {
            other.home_team_id != first.home_team_id
                || other.away_team_id != first.away_team_id
                || other.kickoff_at != first.kickoff_at
                || other.status != first.status
                || other.home_ft != first.home_ft
                || other.away_ft != first.away_ft
        });
        if conflicting {
            vec![Finding::blocker("MATERIAL_SOURCE_CONFLICT", "match",
                "A források anyagi meccsmezőben eltérnek.")]
        } else {
            vec![]
        }
    }
}

pub struct SingleSourceWarningRule;
impl VerificationRule for SingleSourceWarningRule {
    fn key(&self) -> &'static str { "SINGLE_SOURCE" }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        if c.source_values.len() == 1 {
            vec![Finding {
                rule_key: "SINGLE_SOURCE",
                severity: Severity::Warning,
                field: "match",
                message: "A meccs csak egy forrásból megerősített.".into(),
            }]
        } else {
            vec![]
        }
    }
}

pub fn default_rules() -> Vec<Box<dyn VerificationRule>> {
    vec![
        Box::new(ExactIdentityRule),
        Box::new(DuplicateNaturalKeyRule),
        Box::new(KickoffValidRule),
        Box::new(ResultConsistencyRule),
        Box::new(MaterialConflictRule),
        Box::new(SingleSourceWarningRule),
    ]
}
```

- [ ] **Step 4: Futtasd a szabályteszteket**

```bash
cd rust
cargo fmt --all --check
cargo test -p football-verification --test rules
```

Elvárt: minden szabályteszt zöld. A fixture egyetlen forrása miatt a teljes napi tesztek `provisional_publish` állapotot várhatnak; ez tudatos, nem hiba.

---

### Task 9: Konfliktusforrás, leletek és publikálási kapu

**Files:**
- Create: `tests/fixtures/fixture_b_confirm.json`
- Create: `tests/fixtures/fixture_c_conflict.json`
- Create: `rust/crates/verification/src/persist.rs`
- Create: `rust/crates/verification/src/gate.rs`
- Test: `rust/crates/verification/tests/publish_gate.rs`

**Interfaces:**
- Consumes: `VerificationReport`, `PgPool`.
- Produces:
  - `persist_report(pool, match_id, report, rule_set_version) -> run_id`
  - `upsert_publish_gate(pool, match_id, publish_date, state, blockers, rule_set_version) -> Uuid`
  - `local_publish_date(kickoff_at, timezone) -> NaiveDate`

- [ ] **Step 1: Írd meg a konfliktusos fixture-t**

`tests/fixtures/fixture_b_confirm.json` ugyanazt a kanonikus meccs ID-t és ugyanazt a kickoffot adja, de független forrásból:

```json
{
  "source_code": "fixture_b",
  "source_display_name": "Fixture B",
  "captured_at": "2026-11-09T08:30:00Z",
  "endpoint_key": "day-fixtures",
  "request_url": "fixture://fixture_b/day/2026-11-09",
  "adapter_version": "fixture-b-1.0.0",
  "parser_version": "fixture-parser-1.0.0",
  "competition": {
    "id": "10000000-0000-7000-8000-000000000001",
    "name": "Hungarian NB I",
    "country_code": "HU",
    "country_name": "Hungary"
  },
  "season": {
    "id": "10000000-0000-7000-8000-000000000002",
    "label": "2026/27"
  },
  "teams": [
    {
      "id": "10000000-0000-7000-8000-000000000011",
      "source_entity_id": "B-UJP",
      "name": "Újpest",
      "country_code": "HU",
      "gender": "male",
      "age_group": "senior",
      "branch": "senior"
    },
    {
      "id": "10000000-0000-7000-8000-000000000012",
      "source_entity_id": "B-GYOR",
      "name": "Győr",
      "country_code": "HU",
      "gender": "male",
      "age_group": "senior",
      "branch": "senior"
    }
  ],
  "matches": [
    {
      "id": "10000000-0000-7000-8000-000000000101",
      "source_match_id": "B-1001",
      "home_source_entity_id": "B-UJP",
      "away_source_entity_id": "B-GYOR",
      "round_label": "12",
      "kickoff_at": "2026-11-09T18:00:00Z",
      "competition_timezone": "Europe/Budapest",
      "status": "scheduled",
      "result": null
    }
  ]
}
```

`tests/fixtures/fixture_c_conflict.json` ugyanazt a kanonikus meccs ID-t használja, de más kickoffot ad:

```json
{
  "source_code": "fixture_c",
  "source_display_name": "Fixture C",
  "captured_at": "2026-11-09T09:00:00Z",
  "endpoint_key": "day-fixtures",
  "request_url": "fixture://fixture_c/day/2026-11-09",
  "adapter_version": "fixture-c-1.0.0",
  "parser_version": "fixture-parser-1.0.0",
  "competition": {
    "id": "10000000-0000-7000-8000-000000000001",
    "name": "Hungarian NB I",
    "country_code": "HU",
    "country_name": "Hungary"
  },
  "season": {
    "id": "10000000-0000-7000-8000-000000000002",
    "label": "2026/27"
  },
  "teams": [
    {
      "id": "10000000-0000-7000-8000-000000000011",
      "source_entity_id": "B-UJP",
      "name": "Újpest",
      "country_code": "HU",
      "gender": "male",
      "age_group": "senior",
      "branch": "senior"
    },
    {
      "id": "10000000-0000-7000-8000-000000000012",
      "source_entity_id": "B-GYOR",
      "name": "Győr",
      "country_code": "HU",
      "gender": "male",
      "age_group": "senior",
      "branch": "senior"
    }
  ],
  "matches": [
    {
      "id": "10000000-0000-7000-8000-000000000101",
      "source_match_id": "B-1001",
      "home_source_entity_id": "B-UJP",
      "away_source_entity_id": "B-GYOR",
      "round_label": "12",
      "kickoff_at": "2026-11-09T19:00:00Z",
      "competition_timezone": "Europe/Budapest",
      "status": "scheduled",
      "result": null
    }
  ]
}
```

- [ ] **Step 2: Írd meg a bukó gate-tesztet**

```rust
use football_domain::{Finding, VerificationReport};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;
use football_verification::gate::evaluate_and_persist_gate;

#[tokio::test]
async fn blocker_report_creates_withheld_gate() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    FixtureAdapter::ingest(&pool, &store, &fixture_path).await.unwrap();
    let fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    let materialized = Canonicalizer::materialize(&pool, &fixture).await.unwrap();
    let match_id = materialized.matches[0].match_id;

    let report = VerificationReport {
        findings: vec![Finding::blocker(
            "MATERIAL_SOURCE_CONFLICT",
            "match",
            "Források eltérnek.",
        )],
    };

    let state = evaluate_and_persist_gate(
        &pool,
        match_id,
        chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
        &report,
        "verification-1.0",
    )
    .await
    .unwrap();

    assert_eq!(state, football_domain::PublishState::Withheld);
    let stored: String = sqlx::query_scalar(
        "SELECT state FROM ops.publish_gate WHERE match_id=$1",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, "withheld");
}
```

- [ ] **Step 3: Futtasd, és igazold a hiányt**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo test -p football-verification --test publish_gate
```

Elvárt: a perzisztáló és gate-függvény hiányzik.

- [ ] **Step 4: Írd meg a perzisztálást és a kaput**

`rust/crates/verification/src/gate.rs`:

```rust
use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use football_domain::{evaluate_publish_state, PublishState, VerificationReport};
use sqlx::PgPool;
use uuid::Uuid;

pub fn local_publish_date(
    kickoff_at: DateTime<Utc>,
    timezone: &str,
) -> Result<NaiveDate> {
    let tz: chrono_tz::Tz = timezone.parse()?;
    Ok(kickoff_at.with_timezone(&tz).date_naive())
}

pub async fn evaluate_and_persist_gate(
    pool: &PgPool,
    match_id: Uuid,
    publish_date: NaiveDate,
    report: &VerificationReport,
    rule_set_version: &str,
) -> Result<PublishState> {
    let state = evaluate_publish_state(report);
    let blockers = serde_json::to_value(
        report
            .findings
            .iter()
            .filter(|f| f.severity == football_domain::Severity::Blocker)
            .collect::<Vec<_>>(),
    )?;

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)
         ON CONFLICT (match_id, publish_date) DO UPDATE SET
           state = EXCLUDED.state,
           rule_set_version = EXCLUDED.rule_set_version,
           blockers = EXCLUDED.blockers,
           evaluated_at = EXCLUDED.evaluated_at",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(publish_date)
    .bind(state.as_db_str())
    .bind(rule_set_version)
    .bind(blockers)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(state)
}
```

`rust/crates/verification/src/persist.rs`:

```rust
use anyhow::Result;
use chrono::Utc;
use football_domain::{Severity, VerificationReport, VerificationStatus};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn persist_report(
    pool: &PgPool,
    match_id: Uuid,
    report: &VerificationReport,
    rule_set_version: &str,
) -> Result<Uuid> {
    let run_id = Uuid::now_v7();
    let outcome = match report.status() {
        VerificationStatus::Verified => "verified",
        VerificationStatus::Provisional | VerificationStatus::Unverified => "provisional",
        VerificationStatus::Blocked => "blocked",
    };

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO ops.validation_run
         (id, subject_kind, subject_id, rule_set_version, started_at, finished_at, outcome)
         VALUES ($1,'match',$2,$3,$4,$4,$5)",
    )
    .bind(run_id)
    .bind(match_id)
    .bind(rule_set_version)
    .bind(Utc::now())
    .bind(outcome)
    .execute(&mut *tx)
    .await?;

    for finding in &report.findings {
        sqlx::query(
            "INSERT INTO ops.validation_finding
             (id, run_id, rule_key, severity, field_key, message)
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(Uuid::now_v7())
        .bind(run_id)
        .bind(finding.rule_key)
        .bind(match_severity(finding.severity))
        .bind(finding.field)
        .bind(&finding.message)
        .execute(&mut *tx)
        .await?;

        if finding.severity == Severity::Blocker {
            sqlx::query(
                "INSERT INTO fact.conflict
                 (id, subject_kind, subject_id, field_key, severity, status, opened_at)
                 VALUES ($1,'match',$2,$3,'material','unresolved',$4)
                 ON CONFLICT (subject_kind, subject_id, field_key) DO UPDATE SET
                   severity='material',
                   status='unresolved',
                   opened_at=EXCLUDED.opened_at,
                   resolved_at=NULL,
                   resolution_reason=NULL",
            )
            .bind(Uuid::now_v7())
            .bind(match_id)
            .bind(finding.field)
            .bind(Utc::now())
            .execute(&mut *tx)
            .await?;
        }
    }

    for field in ["home_team_id", "away_team_id", "kickoff_at", "status", "result"] {
        let severity = report
            .findings
            .iter()
            .filter(|f| f.field == field)
            .map(|f| f.severity)
            .max()
            .unwrap_or(Severity::Info);
        let status = match severity {
            Severity::Blocker => "blocked",
            Severity::Warning => "provisional",
            Severity::Info => "verified",
        };

        sqlx::query(
            "INSERT INTO fact.verification
             (subject_kind, subject_id, field_key, rule_set_version, status, severity, last_run_id)
             VALUES ('match',$1,$2,$3,$4,$5,$6)
             ON CONFLICT (subject_kind, subject_id, field_key) DO UPDATE SET
               rule_set_version=EXCLUDED.rule_set_version,
               status=EXCLUDED.status,
               severity=EXCLUDED.severity,
               last_run_id=EXCLUDED.last_run_id,
               updated_at=now()",
        )
        .bind(match_id)
        .bind(field)
        .bind(rule_set_version)
        .bind(status)
        .bind(match_severity(severity))
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(run_id)
}

fn match_severity(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Blocker => "blocker",
    }
}
```

- [ ] **Step 5: Futtasd a gate-tesztet**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo fmt --all --check
cargo test -p football-verification --test publish_gate
```

Elvárt: `withheld` gate jön létre a valós fixture-meccshez.

---

### Task 10: Napi lista lekérdezése

**Files:**
- Modify: `rust/crates/storage/src/repositories/day.rs`
- Test: `rust/crates/storage/tests/day_query.rs`

**Interfaces:**
- Consumes: `core.match`, `ops.publish_gate`, `core.team`.
- Produces:
  - `DayFilter { date: NaiveDate, timezone: String, include_all: bool }`
  - `DayMatchRow { match_id, kickoff_at, status, home_name, away_name, home_score, away_score, publish_state, blockers }`
  - `fetch_day(&PgPool, &DayFilter) -> Vec<DayMatchRow>`

- [ ] **Step 1: Írd meg a napi lekérdezés tesztjét**

```rust
use chrono::{NaiveDate, TimeZone, Utc};
use football_storage::repositories::day::{fetch_day, DayFilter};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct MatchBase {
    competition_id: Uuid,
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
}

#[tokio::test]
async fn default_day_contains_only_verified_publish_matches() {
    let pool = test_pool().await;
    let base = seed_match_base(&pool).await;
    let match_id = seed_match(
        &pool,
        &base,
        "verified_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
    )
    .await;
    let provisional_id = seed_match(
        &pool,
        &base,
        "provisional_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 19, 0, 0).unwrap(),
    )
    .await;

    let rows = fetch_day(&pool, &DayFilter {
        date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
        timezone: "Europe/Budapest".into(),
        include_all: false,
    }).await.unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].match_id, match_id);
    assert_ne!(rows[0].match_id, provisional_id);

    let all = fetch_day(&pool, &DayFilter {
        date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
        timezone: "Europe/Budapest".into(),
        include_all: true,
    }).await.unwrap();
    assert_eq!(all.len(), 2);
}

async fn seed_match_base(pool: &PgPool) -> MatchBase {
    let country_id = Uuid::now_v7();
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_entity_id = Uuid::now_v7();
    let home_team_id = Uuid::now_v7();
    let away_entity_id = Uuid::now_v7();
    let away_team_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')")
        .bind(competition_entity_id).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'NB I')",
    ).bind(competition_id).bind(competition_entity_id).bind(country_id)
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id).bind(competition_id).execute(pool).await.unwrap();

    for (entity_id, team_id, name) in [
        (home_entity_id, home_team_id, "Újpest"),
        (away_entity_id, away_team_id, "Győr"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity_id).bind(name).execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        ).bind(team_id).bind(entity_id).bind(country_id).bind(name)
            .execute(pool).await.unwrap();
    }

    MatchBase { competition_id, season_id, home_team_id, away_team_id }
}

async fn seed_match(
    pool: &PgPool,
    base: &MatchBase,
    state: &str,
    kickoff_at: chrono::DateTime<Utc>,
) -> Uuid {
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,'12',$4,$5,$6,'Europe/Budapest','scheduled')",
    )
    .bind(match_id)
    .bind(base.competition_id)
    .bind(base.season_id)
    .bind(base.home_team_id)
    .bind(base.away_team_id)
    .bind(kickoff_at)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,'2026-11-09',$3,'verification-1.0','[]',now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();

    match_id
}
```

- [ ] **Step 2: Futtasd, és igazold a hiányt**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo test -p football-storage --test day_query
```

Elvárt: `fetch_day` hiánya miatt bukik.

- [ ] **Step 3: Írd meg a napi lekérdezést**

`rust/crates/storage/src/repositories/day.rs`:

```rust
use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DayFilter {
    pub date: NaiveDate,
    pub timezone: String,
    pub include_all: bool,
}

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct DayMatchRow {
    pub match_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub status: String,
    pub home_name: String,
    pub away_name: String,
    pub home_score: Option<i16>,
    pub away_score: Option<i16>,
    pub publish_state: String,
    pub blockers: serde_json::Value,
}

pub async fn fetch_day(pool: &PgPool, filter: &DayFilter) -> Result<Vec<DayMatchRow>> {
    let rows = sqlx::query_as::<_, DayMatchRow>(
        "SELECT
            m.id AS match_id,
            m.kickoff_at,
            m.status,
            home.name AS home_name,
            away.name AS away_name,
            r.home_ft AS home_score,
            r.away_ft AS away_score,
            g.state AS publish_state,
            g.blockers
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
         JOIN ops.publish_gate g ON g.match_id = m.id AND g.publish_date = $1
         WHERE (m.kickoff_at AT TIME ZONE $2)::date = $1
           AND ($3::boolean OR g.state = 'verified_publish')
         ORDER BY m.kickoff_at, home.name, away.name",
    )
    .bind(filter.date)
    .bind(&filter.timezone)
    .bind(filter.include_all)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 4: Futtasd a teszteket**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo fmt --all --check
cargo test -p football-storage --test day_query
```

Elvárt: alapértelmezésben egy sor, `include_all=true` esetén két sor.

---

### Task 11: Helyi API tokenekkel

**Files:**
- Create: `rust/crates/api/Cargo.toml`
- Create: `rust/crates/api/src/lib.rs`
- Create: `rust/crates/api/src/auth.rs`
- Create: `rust/crates/api/src/error.rs`
- Create: `rust/crates/api/src/routes/mod.rs`
- Create: `rust/crates/api/src/routes/health.rs`
- Create: `rust/crates/api/src/routes/day.rs`
- Create: `rust/crates/api/src/routes/verification.rs`
- Create: `rust/bins/footballd/Cargo.toml`
- Create: `rust/bins/footballd/src/main.rs`
- Create: `rust/bins/footballd/src/runtime_file.rs`
- Test: `rust/crates/api/tests/routes.rs`

**Interfaces:**
- Consumes: `fetch_day`, `PgPool`.
- Produces:
  - `football_api::router(AppState) -> Router`
  - `AppState { pool, read_token, ops_token }`
  - `GET /v1/health`
  - `GET /v1/day/{date}` read tokennel
  - `GET /v1/ops/verification/gates?date={date}` ops tokennel

- [ ] **Step 1: Írd meg a bukó route-teszteket**

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use football_api::{router, AppState};
use football_storage::test_support::test_pool;
use tower::ServiceExt;

#[tokio::test]
async fn health_is_public() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
    });

    let response = app
        .oneshot(Request::builder().uri("/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn day_requires_read_token() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
    });

    let unauthorized = app
        .clone()
        .oneshot(Request::builder().uri("/v1/day/2026-11-09").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let authorized = app
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Futtasd, és igazold a hiányzó API-t**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo test -p football-api --test routes
```

Elvárt: a crate hiányzik.

- [ ] **Step 3: Írd meg az API crate-ot**

`rust/crates/api/Cargo.toml`:

```toml
[package]
name = "football-api"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
axum.workspace = true
chrono.workspace = true
football-storage = { path = "../storage" }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
tokio.workspace = true
tower.workspace = true
tracing.workspace = true
uuid.workspace = true

[dev-dependencies]
http-body-util = "0.1"
tempfile.workspace = true
```

`rust/crates/api/src/error.rs`:

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            message: "Érvénytelen vagy hiányzó hozzáférési token.".into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL",
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiError {
                code: self.code,
                message: self.message,
                request_id: None,
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        tracing::error!(error = %value, "adatbázis hiba");
        Self::internal("Adatbázis hiba.")
    }
}
```

`rust/crates/api/src/auth.rs`:

```rust
use axum::http::HeaderMap;

use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub enum Scope {
    Read,
    Ops,
}

pub fn authorize(headers: &HeaderMap, scope: Scope, read_token: &str, ops_token: &str) -> Result<(), AppError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(AppError::unauthorized)?;
    let token = value
        .strip_prefix("Bearer ")
        .ok_or_else(AppError::unauthorized)?;

    let allowed = match scope {
        Scope::Read => token == read_token || token == ops_token,
        Scope::Ops => token == ops_token,
    };

    if allowed { Ok(()) } else { Err(AppError::unauthorized()) }
}
```

`rust/crates/api/src/routes/health.rs`:

```rust
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "footballd",
    })
}
```

`rust/crates/api/src/routes/day.rs`:

```rust
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::NaiveDate;
use football_storage::repositories::day::{fetch_day, DayFilter};
use serde::{Deserialize, Serialize};

use crate::auth::{authorize, Scope};
use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct DayQuery {
    pub include: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DayResponse {
    pub date: NaiveDate,
    pub timezone: &'static str,
    pub matches: Vec<DayMatch>,
}

#[derive(Debug, Serialize)]
pub struct DayMatch {
    pub id: uuid::Uuid,
    pub kickoff_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub home: String,
    pub away: String,
    pub home_score: Option<i16>,
    pub away_score: Option<i16>,
    pub publish_state: String,
    pub blockers: serde_json::Value,
}

pub async fn day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(date): Path<String>,
    Query(query): Query<DayQuery>,
) -> Result<Json<DayResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("A dátum formátuma YYYY-MM-DD legyen."))?;

    let rows = fetch_day(&state.pool, &DayFilter {
        date,
        timezone: "Europe/Budapest".into(),
        include_all: query.include.as_deref() == Some("all"),
    })
    .await
    .map_err(|e| {
        tracing::error!(error=%e, "napi lista lekérdezés");
        AppError::internal("A napi lista lekérdezése sikertelen.")
    })?;

    Ok(Json(DayResponse {
        date,
        timezone: "Europe/Budapest",
        matches: rows.into_iter().map(|row| DayMatch {
            id: row.match_id,
            kickoff_at: row.kickoff_at,
            status: row.status,
            home: row.home_name,
            away: row.away_name,
            home_score: row.home_score,
            away_score: row.away_score,
            publish_state: row.publish_state,
            blockers: row.blockers,
        }).collect(),
    }))
}
```

`rust/crates/api/src/routes/verification.rs`:

```rust
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::auth::{authorize, Scope};
use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct GatesQuery {
    pub date: NaiveDate,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GateRow {
    pub match_id: uuid::Uuid,
    pub state: String,
    pub blockers: serde_json::Value,
    pub evaluated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn gates(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GatesQuery>,
) -> Result<Json<Vec<GateRow>>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;

    let rows = sqlx::query_as::<_, GateRow>(
        "SELECT match_id, state, blockers, evaluated_at
         FROM ops.publish_gate
         WHERE publish_date = $1
         ORDER BY evaluated_at DESC",
    )
    .bind(query.date)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}
```

`rust/crates/api/src/lib.rs`:

```rust
pub mod auth;
pub mod error;
pub mod routes;

use axum::{routing::get, Router};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub read_token: String,
    pub ops_token: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(routes::health::health))
        .route("/v1/day/{date}", get(routes::day::day))
        .route("/v1/ops/verification/gates", get(routes::verification::gates))
        .with_state(state)
}
```

A `routes/mod.rs`:

```rust
pub mod day;
pub mod health;
pub mod verification;
```

- [ ] **Step 4: Írd meg a daemont és a runtime tokenfájlt**

`rust/bins/footballd/Cargo.toml`:

```toml
[package]
name = "footballd"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
axum.workspace = true
dirs.workspace = true
football-api = { path = "../../crates/api" }
rand.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
uuid.workspace = true
```

`rust/bins/footballd/src/runtime_file.rs`:

```rust
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeFile {
    pub port: u16,
    pub read_token: String,
    pub ops_token: String,
}

pub fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn write_runtime_file(port: u16) -> Result<RuntimeFile> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("nincs felhasználói adatkönyvtár"))?
        .join("FootballAppV2");
    fs::create_dir_all(&dir)?;
    let path: PathBuf = dir.join("runtime.json");
    let runtime = RuntimeFile {
        port,
        read_token: new_token(),
        ops_token: new_token(),
    };
    let temp = path.with_extension("tmp");
    fs::write(&temp, serde_json::to_vec_pretty(&runtime)?)?;
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    fs::rename(temp, path)?;
    Ok(runtime)
}
```

A `dirs` dependency a workspace `Cargo.toml`-ban és a `footballd` manifestjében szerepel.

`rust/bins/footballd/src/main.rs`:

```rust
mod runtime_file;

use anyhow::Result;
use football_api::{router, AppState};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new().max_connections(8).connect(&database_url).await?;

    sqlx::migrate!("../../../db/migrations").run(&pool).await?;

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:0".into());
    let listener = TcpListener::bind(bind_addr).await?;
    let port = listener.local_addr()?.port();
    let runtime = runtime_file::write_runtime_file(port)?;

    let app = router(AppState {
        pool,
        read_token: runtime.read_token,
        ops_token: runtime.ops_token,
    });

    tracing::info!(port, "footballd elindult");
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 5: Futtasd a route-teszteket**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo fmt --all --check
cargo test -p football-api --test routes
```

Elvárt: `health` 200, token nélküli napi lista 401, érvényes read tokennel 200.

---

### Task 12: `footballctl` CLI és végponttól végpontig ellenőrzés

**Files:**
- Create: `rust/bins/footballctl/Cargo.toml`
- Create: `rust/bins/footballctl/src/main.rs`
- Create: `scripts/check.sh`
- Create: `docs/operations/local-development.md`
- Test: CLI smoke és teljes pipeline

**Interfaces:**
- Consumes: fixture, canonicalizer, verification engine, gate, day repository.
- Produces:
  - `footballctl migrate`
  - `footballctl ingest-fixture --path <path>`
  - `footballctl day --date <date> --include all`
  - `footballctl doctor`

- [ ] **Step 1: Írd meg a CLI-t**

`rust/bins/footballctl/Cargo.toml`:

```toml
[package]
name = "footballctl"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
chrono.workspace = true
clap.workspace = true
football-domain = { path = "../../crates/domain" }
football-ingest = { path = "../../crates/ingest" }
football-storage = { path = "../../crates/storage" }
football-verification = { path = "../../crates/verification" }
serde_json.workspace = true
sqlx.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
uuid.workspace = true
```

`rust/bins/footballctl/src/main.rs`:

```rust
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::repositories::day::{fetch_day, DayFilter};
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::gate::evaluate_and_persist_gate;
use football_verification::persist::persist_report;
use football_verification::rules::default_rules;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "footballctl")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Migrate,
    IngestFixture { #[arg(long)] path: PathBuf },
    Day { #[arg(long)] date: NaiveDate, #[arg(long, default_value_t = false)] include_all: bool },
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let cli = Cli::parse();
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL hiányzik")?;
    let pool = PgPoolOptions::new().max_connections(4).connect(&database_url).await?;
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());
    let store = ObjectStore::new(&data_dir)?;

    match cli.command {
        Command::Migrate => {
            sqlx::migrate!("../../../db/migrations").run(&pool).await?;
            println!("migráció kész");
        }
        Command::IngestFixture { path } => {
            let report = FixtureAdapter::ingest(&pool, &store, &path).await?;
            let fixture: FixtureSource = serde_json::from_slice(&std::fs::read(&path)?)?;
            let day = Canonicalizer::materialize(&pool, &fixture).await?;
            let rules = default_rules();

            for materialized in day.matches {
                let context = VerificationContext {
                    snapshot: &materialized.snapshot,
                    home_identity_exact: true,
                    away_identity_exact: true,
                    duplicate_match_count: duplicate_count(&pool, &materialized).await?,
                    source_values: &[],
                };
                let report = VerificationEngine::run(&rules, &context);
                let run_id = persist_report(&pool, materialized.match_id, &report, "verification-1.0").await?;
                let date = football_verification::gate::local_publish_date(
                    materialized.snapshot.kickoff_at,
                    "Europe/Budapest",
                )?;
                let state = evaluate_and_persist_gate(
                    &pool, materialized.match_id, date, &report, "verification-1.0",
                ).await?;
                println!(
                    "match={} run={} state={}",
                    materialized.match_id, run_id, state.as_db_str()
                );
            }
            println!("capture={} observations={}", report.capture_id, report.observation_ids.len());
        }
        Command::Day { date, include_all } => {
            let rows = fetch_day(&pool, &DayFilter {
                date,
                timezone: "Europe/Budapest".into(),
                include_all,
            }).await?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        Command::Doctor => {
            let ok: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&pool).await?;
            let gates: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.publish_gate").fetch_one(&pool).await?;
            println!("db={ok} gates={gates}");
        }
    }

    Ok(())
}

async fn duplicate_count(
    pool: &sqlx::PgPool,
    materialized: &football_ingest::canonicalize::MaterializedMatch,
) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM core.match
         WHERE competition_id = (SELECT competition_id FROM core.match WHERE id=$1)
           AND season_id = (SELECT season_id FROM core.match WHERE id=$1)
           AND round_label IS NOT DISTINCT FROM (SELECT round_label FROM core.match WHERE id=$1)
           AND home_team_id = $2 AND away_team_id = $3
           AND kickoff_at = $4
           AND id <> $1",
    )
    .bind(materialized.match_id)
    .bind(materialized.home_team_id)
    .bind(materialized.away_team_id)
    .bind(materialized.snapshot.kickoff_at)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}
```

A `source_values` ebben az utolsó lépésben még üres; a Task 9 konfliktus-fixture-jének végponttól végpontig való futtatásához a CLI-nek a két forrás legutóbbi `match_core` megfigyelését kell betöltenie. Ezt a **Task 13** egészíti ki; a jelen task a normál egyforrásos folyamat működését zárja.

- [ ] **Step 2: Írd meg a teljes ellenőrző scriptet**

`scripts/check.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
export DATABASE_URL="$TEST_DATABASE_URL"

./scripts/db-reset.sh
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
```

- [ ] **Step 3: Futtasd a teljes pipeline-t fejlesztői adatbázison**

```bash
export DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_dev'
export DATA_DIR='./data'
cd rust
cargo run -p footballctl -- migrate
cargo run -p footballctl -- ingest-fixture --path ../tests/fixtures/fixture_a_day.json
cargo run -p footballctl -- ingest-fixture --path ../tests/fixtures/fixture_b_confirm.json
cargo run -p footballctl -- day --date 2026-11-09
cargo run -p footballctl -- day --date 2026-11-09 --include-all
```

Elvárt: a két egyező forrás után a meccs `verified_publish` állapotú; az alapértelmezett lista egy meccset ad, az `--include-all` ugyanazt a meccset a state mezővel együtt.

- [ ] **Step 4: Írd meg a helyi fejlesztési útmutatót**

`docs/operations/local-development.md`:

```markdown
# Helyi fejlesztés

## Előkészítés

```bash
./scripts/check-toolchain.sh
./scripts/db-create.sh
export DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_dev'
export DATA_DIR='./data'
```

## Migráció és fixture

```bash
cd rust
cargo run -p footballctl -- migrate
cargo run -p footballctl -- ingest-fixture --path ../tests/fixtures/fixture_a_day.json
cargo run -p footballctl -- ingest-fixture --path ../tests/fixtures/fixture_b_confirm.json
cargo run -p footballctl -- day --date 2026-11-09
```

## Teljes ellenőrzés

```bash
./scripts/check.sh
```

A repo jelenleg nem git repository; commit nem készül automatikusan.
```

- [ ] **Step 5: Futtasd az ellenőrzést**

```bash
chmod +x scripts/check.sh
./scripts/check.sh
```

Elvárt: formázás, Clippy, migráció és minden teszt zöld.

---

### Task 13: Kétforrásos konfliktus végponttól végpontig

**Files:**
- Modify: `rust/bins/footballctl/src/main.rs`
- Modify: `rust/crates/verification/src/lib.rs`
- Create: `rust/crates/verification/src/conflict.rs`
- Create: `rust/crates/verification/tests/conflict_pipeline.rs`
- Test: két fixture betöltése után `withheld` gate

**Interfaces:**
- Consumes: két `FixtureSource`, `stage.observation`, `VerificationContext`.
- Produces: `load_source_values(pool, match_id) -> Vec<SourceMaterialValues>`; a CLI a két forrás betöltése után a valós értékeket adja a kontextusnak.

- [ ] **Step 1: Írd meg a bukó kétforrásos tesztet**

`rust/crates/verification/tests/conflict_pipeline.rs`:

```rust
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;
use football_verification::conflict::load_material_values;
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::rules::default_rules;

#[tokio::test]
async fn conflicting_sources_produce_blocker_and_withheld_gate() {
    let pool = test_pool().await;
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let conflict_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_c_conflict.json");

    FixtureAdapter::ingest(&pool, &store, &fixture_path).await.unwrap();
    FixtureAdapter::ingest(&pool, &store, &conflict_path).await.unwrap();

    let fixture_a: FixtureSource = serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    let fixture_c: FixtureSource = serde_json::from_slice(&std::fs::read(&conflict_path).unwrap()).unwrap();
    let day_a = Canonicalizer::materialize(&pool, &fixture_a).await.unwrap();
    let day_c = Canonicalizer::materialize(&pool, &fixture_c).await.unwrap();
    assert_eq!(day_a.matches[0].match_id, day_c.matches[0].match_id);

    let values = load_material_values(&pool, day_a.matches[0].match_id).await.unwrap();
    let context = VerificationContext {
        snapshot: &day_a.matches[0].snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: &values,
    };
    let report = VerificationEngine::run(&default_rules(), &context);

    assert!(report.findings.iter().any(|f| f.rule_key == "MATERIAL_SOURCE_CONFLICT"));
    let state = football_verification::gate::evaluate_and_persist_gate(
        &pool,
        day_a.matches[0].match_id,
        chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
        &report,
        "verification-1.0",
    ).await.unwrap();
    assert_eq!(state, football_domain::PublishState::Withheld);
}
```

A teszt a `rust/crates/verification/tests/conflict_pipeline.rs` fájlban él, és a két forrás ugyanarra a kanonikus meccs-ID-ra mutat.

- [ ] **Step 2: Futtasd, és igazold, hogy a conflict betöltő hiányzik**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo test -p football-verification --test conflict_pipeline
```

Elvárt: `football_verification::conflict` hiányzik.

- [ ] **Step 3: Írd meg a konfliktusérték-betöltőt**

Először egészítsd ki a `rust/crates/verification/src/lib.rs` fájlt:

```rust
pub mod conflict;
```

Majd írd meg a `rust/crates/verification/src/conflict.rs` fájlt:

```rust
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_domain::MatchStatus;
use sqlx::PgPool;
use uuid::Uuid;

use crate::engine::SourceMaterialValues;

pub async fn load_material_values(
    pool: &PgPool,
    match_id: Uuid,
) -> Result<Vec<SourceMaterialValues>> {
    let rows = sqlx::query_as::<_, RawSourceValue>(
        "SELECT DISTINCT ON (o.source_id)
            s.code AS source_code,
            o.normalized_payload
         FROM stage.observation o
         JOIN ops.source s ON s.id = o.source_id
         WHERE o.entity_kind = 'match'
           AND o.aspect = 'match_core'
           AND o.natural_key = $1
         ORDER BY o.source_id, o.observed_at DESC",
    )
    .bind(match_id.to_string())
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let payload = row.normalized_payload;
            let home_team_id = parse_uuid(&payload, "home_team_id")?;
            let away_team_id = parse_uuid(&payload, "away_team_id")?;
            let kickoff_at = required_str(&payload, "kickoff_at")?
                .parse::<DateTime<Utc>>()?;
            let status = match required_str(&payload, "status")? {
                "scheduled" => MatchStatus::Scheduled,
                "confirmed" => MatchStatus::Confirmed,
                "live" => MatchStatus::Live,
                "halftime" => MatchStatus::Halftime,
                "finished" => MatchStatus::Finished,
                "postponed" => MatchStatus::Postponed,
                "canceled" => MatchStatus::Canceled,
                "abandoned" => MatchStatus::Abandoned,
                "awarded" => MatchStatus::Awarded,
                other => anyhow::bail!("ismeretlen státusz: {other}"),
            };

            Ok(SourceMaterialValues {
                source_code: row.source_code,
                home_team_id,
                away_team_id,
                kickoff_at,
                status,
                home_ft: payload["result"]["home_ft"].as_i64().map(|v| v as i16),
                away_ft: payload["result"]["away_ft"].as_i64().map(|v| v as i16),
            })
        })
        .collect()
}

fn required_str<'a>(payload: &'a serde_json::Value, field: &str) -> Result<&'a str> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("hiányzó mező: {field}"))
}

fn parse_uuid(payload: &serde_json::Value, field: &str) -> Result<Uuid> {
    Ok(Uuid::parse_str(required_str(payload, field)?)?)
}

#[derive(Debug, sqlx::FromRow)]
struct RawSourceValue {
    source_code: String,
    normalized_payload: serde_json::Value,
}
```

A `FixtureAdapter` már a Task 6 szerint a kanonikus UUID-t írja a payload `home_team_id` és `away_team_id` mezőjébe, a forrásazonosítót pedig külön `source_home_id`/`source_away_id` mezőben tartja. A konfliktus-összehasonlítás ezért a forrásazonosítókat nem keveri a kanonikus identitással.

- [ ] **Step 4: Kösd be a CLI-be a valós konfliktusértékeket**

A `Command::IngestFixture` ágban a `VerificationContext` létrehozása előtt hívd meg:

```rust
let source_values = football_verification::conflict::load_material_values(
    &pool,
    materialized.match_id,
).await?;
```

Majd a contextben:

```rust
source_values: &source_values,
```

Ezután a második fixture betöltése a `MATERIAL_SOURCE_CONFLICT` blokkolót és `withheld` publikálási kaput eredményez.

- [ ] **Step 5: Futtasd a kétforrásos tesztet és a teljes ellenőrzést**

```bash
export TEST_DATABASE_URL='postgres://football_v2:LOCAL_DB_PASSWORD@127.0.0.1:5432/football_v2_test'
cd rust
cargo fmt --all --check
cargo test -p football-verification --test conflict_pipeline
cd ..
./scripts/check.sh
```

Elvárt: a konfliktus blokkoló lelet jön létre, a gate `withheld`, és a teljes tesztkészlet zöld.

---

## Önellenőrzés a terv elkészítése után

A terv a specifikáció következő elemeit fedi a backend-szeletben:

- helyi Rust + PostgreSQL architektúra és migration-lánc;
- raw-first rögzítés, stage-megfigyelés, kanonikus meccs;
- exact identity és alias nélküli biztonságos fixture-kapcsolás;
- mezőszintű provenance;
- L0–L2 egyszerűsített capture/parser/domain ellenőrzés;
- L3 exact identity;
- L4 konfliktus;
- L5/L6 eredmény- és duplikátumkonzisztencia;
- L7 publikálási kapu;
- tokenes localhost API;
- csak ellenőrzött napi lista;
- hibából szabály irány, mert a szabályok és a `validation_rule` registry együtt élnek.

A SwiftUI kliens, élő forrás, teljes aliasreview, meccsoldal, live, predikció, bot és ügynök szándékosan a következő tervekben szerepel.
