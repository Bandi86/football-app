BEGIN;

ALTER TABLE ops.job
    ADD COLUMN IF NOT EXISTS lease_token uuid;

CREATE UNIQUE INDEX IF NOT EXISTS idx_job_lease_token
    ON ops.job (lease_token)
    WHERE lease_token IS NOT NULL;

ALTER TABLE ops.job
    DROP CONSTRAINT IF EXISTS job_lease_token_status_check;
ALTER TABLE ops.job
    ADD CONSTRAINT job_lease_token_status_check
    CHECK (lease_token IS NULL OR status = 'leased');

ALTER TABLE ops.fetch_ledger
    DROP CONSTRAINT IF EXISTS fetch_ledger_outcome_check;
ALTER TABLE ops.fetch_ledger
    ADD CONSTRAINT fetch_ledger_outcome_check
    CHECK (outcome IN (
        'success', 'http_error', 'timeout', 'oversize', 'robots_blocked',
        'quota_blocked', 'breaker_open', 'transport_error', 'parse_error',
        'reserved'
    ));

COMMIT;
