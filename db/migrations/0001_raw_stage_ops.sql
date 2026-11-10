-- 0001 — a raw, stage és ops sémák első migrációja.
-- A sqlx a public._sqlx_migrations táblában tartja nyilván; ismételt futtatása nem hajtódik végre újra.
-- Visszaállítási út: scripts/db-reset.sh (kizárólag a tesztadatbázison), amely a sémákkal együtt
-- a migrációs nyilvántartást is törli, majd újrafuttatja a migrációt.

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