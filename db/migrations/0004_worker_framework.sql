-- 0004 — worker keretrendszer ops táblái (S2a).
-- Visszaállítási út: scripts/db-reset.sh (tesztDB); dev DB-n a táblák üresek,
-- eldobásuk fordított sorrendben: robots_cache, coverage_slot, job, breaker_state,
-- fetch_ledger, source_policy (DROP TABLE ... CASCADE nélkül, FK-sorrendben).

BEGIN;

CREATE TABLE ops.source_policy (
    source_slug text PRIMARY KEY,
    transport text NOT NULL CHECK (transport IN ('rust_http','fixture')),
    mode text NOT NULL CHECK (mode IN ('shadow','canary','live')),
    daily_budget integer NOT NULL CHECK (daily_budget >= 0),
    min_delay_ms bigint NOT NULL CHECK (min_delay_ms >= 0),
    jitter_ms bigint NOT NULL CHECK (jitter_ms >= 0),
    breaker_threshold integer NOT NULL CHECK (breaker_threshold >= 1),
    breaker_cooldown_secs bigint NOT NULL CHECK (breaker_cooldown_secs >= 0),
    adapter_version text NOT NULL,
    parser_version text NOT NULL,
    domains jsonb NOT NULL DEFAULT '[]',
    events jsonb NOT NULL DEFAULT '[]',
    enabled boolean NOT NULL DEFAULT false,
    max_bytes bigint NOT NULL DEFAULT 5242880 CHECK (max_bytes > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE ops.fetch_ledger (
    id uuid PRIMARY KEY,
    source_slug text NOT NULL REFERENCES ops.source_policy(source_slug),
    request_key text NOT NULL,
    attempted_at timestamptz NOT NULL,
    outcome text NOT NULL CHECK (outcome IN
        ('success','http_error','timeout','oversize','robots_blocked',
         'quota_blocked','breaker_open','transport_error','parse_error')),
    http_status smallint,
    byte_size bigint,
    latency_ms bigint,
    artifact_hash text,
    job_id uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_fetch_ledger_source_day ON ops.fetch_ledger (source_slug, attempted_at);

CREATE TABLE ops.breaker_state (
    source_slug text PRIMARY KEY REFERENCES ops.source_policy(source_slug),
    state text NOT NULL CHECK (state IN ('closed','open','half_open')),
    failure_count integer NOT NULL DEFAULT 0 CHECK (failure_count >= 0),
    opened_at timestamptz,
    next_probe_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE ops.job (
    id uuid PRIMARY KEY,
    source_slug text NOT NULL REFERENCES ops.source_policy(source_slug),
    domain text NOT NULL,
    entity_key text NOT NULL,
    event_type text NOT NULL,
    time_window text NOT NULL,
    due_at timestamptz NOT NULL,
    priority integer NOT NULL DEFAULT 100,
    status text NOT NULL CHECK (status IN ('pending','leased','done','failed','cancelled')),
    leased_until timestamptz,
    worker_id uuid,
    attempts integer NOT NULL DEFAULT 0,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_slug, domain, entity_key, event_type, time_window)
);

CREATE INDEX idx_job_due ON ops.job (status, due_at, priority);

CREATE TABLE ops.coverage_slot (
    source_slug text NOT NULL REFERENCES ops.source_policy(source_slug),
    entity_type text NOT NULL,
    data_kind text NOT NULL,
    time_window text NOT NULL,
    state text NOT NULL CHECK (state IN
        ('pending','fetching','done','no_data','blocked','failed','stale','quarantined','skipped')),
    last_job_id uuid,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source_slug, entity_type, data_kind, time_window)
);

CREATE TABLE ops.robots_cache (
    host text PRIMARY KEY,
    fetched_at timestamptz NOT NULL,
    verdict text NOT NULL CHECK (verdict IN ('allowed','no_robots','blocked')),
    rules_text text NOT NULL,
    expires_at timestamptz NOT NULL
);

COMMIT;
