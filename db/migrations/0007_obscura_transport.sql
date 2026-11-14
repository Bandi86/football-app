-- 0007 — obscura transport engedélyezése a source_policy-ban (S2c).
-- A 0004 CHECK csak rust_http/fixture-t engedett; az Obscura browser-fetcher
-- (S2c) opt-in szállításként az 'obscura' transportot használja.
-- Visszaállítás: a CHECK visszaállítása ('rust_http','fixture') — csak ha
-- nincs obscura sor (különben a sorok sértenék a constraintet).

BEGIN;

ALTER TABLE ops.source_policy DROP CONSTRAINT source_policy_transport_check;
ALTER TABLE ops.source_policy ADD CONSTRAINT source_policy_transport_check
    CHECK (transport IN ('rust_http','fixture','obscura'));

COMMIT;