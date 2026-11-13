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

-- Egy pending sor kulcsonként; a NULL source_id is egyenértékű kulcsnak
-- számít (NULLS NOT DISTINCT), különben forrás nélküli sorból több pending
-- kerülhetne be.
CREATE UNIQUE INDEX uq_review_pending
    ON identity.review_queue (source_id, entity_kind, source_entity_id, reason)
    NULLS NOT DISTINCT
    WHERE status = 'pending';

COMMIT;