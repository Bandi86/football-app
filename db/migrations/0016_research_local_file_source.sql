-- 0016 — helyi fájl-forrás (football-research) + create-on-miss identitás (S38).
--
-- A football-research fájl-alapú JSON API-ját a v2 `local_file` transzporttal
-- olvassa: nincs hálózat, nincs robots és nincs kvóta. A research forrás
-- create-on-miss módban új kanonikus csapatot hozhat létre; minden más forrás
-- marad a szigorú fail-closed review-nál.
--
-- Visszaállítás: a transport CHECK szűkítése és az oszlop eldobása csak akkor
-- biztonságos, ha nincs `local_file` policy és nincs create_on_miss forrás:
--   ALTER TABLE ops.source DROP COLUMN identity_mode;
--   ALTER TABLE ops.source_policy DROP CONSTRAINT source_policy_transport_check;
--   ALTER TABLE ops.source_policy ADD CONSTRAINT source_policy_transport_check
--       CHECK (transport IN ('rust_http','fixture','obscura'));

ALTER TABLE ops.source_policy DROP CONSTRAINT source_policy_transport_check;
ALTER TABLE ops.source_policy ADD CONSTRAINT source_policy_transport_check
    CHECK (transport IN ('rust_http', 'fixture', 'obscura', 'local_file'));

ALTER TABLE ops.source ADD COLUMN identity_mode text NOT NULL DEFAULT 'fail_closed'
    CHECK (identity_mode IN ('fail_closed', 'create_on_miss'));

COMMENT ON COLUMN ops.source.identity_mode IS
    'S38: fail_closed = ismeretlen entitás review-sorba kerül; create_on_miss = a forrás új kanonikus entitást hozhat létre (fuzzy jelöltnél review marad).';