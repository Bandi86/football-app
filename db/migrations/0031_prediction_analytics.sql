BEGIN;

-- S196: predikciós analytics — mérési snapshotok és de-vigelési piacvonalak.
-- Minden sor hordozza a modellverziót, az as-of időpontot és az input-hash-t;
-- a kanonikus adatot (core/fact) a séma nem módosítja.

CREATE SCHEMA IF NOT EXISTS analytics;

CREATE TABLE IF NOT EXISTS analytics.market_line (
    id uuid PRIMARY KEY,
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    market text NOT NULL,
    outcome text NOT NULL,
    line numeric NOT NULL DEFAULT 0,
    price numeric NOT NULL CHECK (price > 1),
    bookmaker text NOT NULL,
    phase text NOT NULL,
    de_vig_method text NOT NULL,
    implied_probability double precision NOT NULL
        CHECK (implied_probability > 0 AND implied_probability <= 1),
    captured_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (match_id, source_id, market, outcome, line, bookmaker, phase, captured_at)
);
CREATE INDEX IF NOT EXISTS idx_market_line_match
    ON analytics.market_line (match_id, market);

CREATE TABLE IF NOT EXISTS analytics.prediction (
    id uuid PRIMARY KEY,
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    model_version text NOT NULL,
    market text NOT NULL,
    outcome text NOT NULL,
    line numeric NOT NULL DEFAULT 0,
    probability double precision NOT NULL
        CHECK (probability > 0 AND probability <= 1),
    fair_odds numeric CHECK (fair_odds > 1),
    edge numeric,
    quality text NOT NULL CHECK (quality IN ('ok', 'degraded', 'no_data')),
    inputs_hash text NOT NULL,
    as_of timestamptz NOT NULL,
    source_id uuid REFERENCES ops.source(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (match_id, model_version, market, outcome, line)
);
CREATE INDEX IF NOT EXISTS idx_prediction_version
    ON analytics.prediction (model_version, as_of);

CREATE TABLE IF NOT EXISTS analytics.measurement (
    id uuid PRIMARY KEY,
    model_version text NOT NULL,
    date_from date NOT NULL,
    date_to date NOT NULL,
    market text NOT NULL,
    sample_size integer NOT NULL CHECK (sample_size > 0),
    brier double precision NOT NULL,
    brier_baseline double precision NOT NULL,
    brier_market double precision,
    difference double precision NOT NULL,
    ci_low double precision NOT NULL,
    ci_high double precision NOT NULL,
    log_loss double precision NOT NULL,
    wilson_low double precision NOT NULL,
    wilson_high double precision NOT NULL,
    inputs_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (model_version, date_from, date_to, market)
);
CREATE INDEX IF NOT EXISTS idx_measurement_range
    ON analytics.measurement (date_from, date_to, model_version);

COMMIT;

-- Visszaállítás: DROP TABLE analytics.measurement, analytics.prediction,
-- analytics.market_line; DROP SCHEMA analytics; majd a _sqlx_migrations 0031-es
-- sorának törlése. A kanonikus core/fact táblákat a migráció nem érintette.
