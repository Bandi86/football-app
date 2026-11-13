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