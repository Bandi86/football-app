BEGIN;

-- S102: a játékvezető saját országa (a forrás által kimondott adat).
-- A versenyország NEM bíróország, ezért külön oszlop; a hiány NULL, nem kitalált érték.
-- A country_id minden személyfajtára (referee, player, manager) használható,
-- külön core.referee tábla nélkül (S2d-1 döntése).
ALTER TABLE core.entity
    ADD COLUMN IF NOT EXISTS country_id uuid REFERENCES core.country(id);

CREATE INDEX IF NOT EXISTS idx_entity_country_id
    ON core.entity (country_id) WHERE country_id IS NOT NULL;

ALTER TABLE fact.match_official
    ADD COLUMN IF NOT EXISTS country_code text;

COMMIT;

-- Visszaállítás:
--   ALTER TABLE fact.match_official DROP COLUMN IF EXISTS country_code;
--   DROP INDEX IF EXISTS idx_entity_country_id;
--   ALTER TABLE core.entity DROP COLUMN IF EXISTS country_id;
-- majd a _sqlx_migrations 0025-ös sorának törlése (külön, egyeztetett lépés).
