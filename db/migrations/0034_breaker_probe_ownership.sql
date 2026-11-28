BEGIN;

ALTER TABLE ops.breaker_state
    ADD COLUMN IF NOT EXISTS probe_token uuid;

ALTER TABLE ops.breaker_state
    DROP CONSTRAINT IF EXISTS breaker_probe_token_state_check;
ALTER TABLE ops.breaker_state
    ADD CONSTRAINT breaker_probe_token_state_check
    CHECK (probe_token IS NULL OR state = 'half_open');

COMMIT;
