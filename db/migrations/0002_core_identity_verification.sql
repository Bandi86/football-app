-- 0002 — core, identity és fact sémák, valamint az ops.publish_gate.
-- A sqlx a public._sqlx_migrations táblában tartja nyilván; ismételt futtatása nem hajtódik végre újra.
-- Visszaállítási út: scripts/db-reset.sh (kizárólag a tesztadatbázison), amely minden sémát és
-- a migrációs nyilvántartást törli, majd a 0001 és 0002 migrációkat együtt újrafuttatja.

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