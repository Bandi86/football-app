BEGIN;

-- S188: a lezárt, de még eredmény nélküli meccsek eddig a félrevezető
-- 'stale_status' kódot kapták. A napi hiány-számláló (`missing_result`) és a
-- kliens szűrője (`Eredmény nélkül`) fogalmához igazodva külön kód kerül be.
-- A régi 'stale_status' a visszaféle kompatibilitás miatt továbbra is engedélyezett,
-- de a detektor nem írja többé.

ALTER TABLE ops.issue DROP CONSTRAINT IF EXISTS issue_code_check;
ALTER TABLE ops.issue ADD CONSTRAINT issue_code_check CHECK (code IN (
    'missing_logo', 'missing_odds', 'open_past', 'duplicate_suspect',
    'identity_review', 'stale_status', 'missing_result', 'source_conflict'
));

COMMIT;

-- Visszaállítás: a fenti CHECK konstans visszaállítása a hét elemű listára
-- ('missing_result' nélkül), majd a _sqlx_migrations 0030-as sorának törlése.
-- A már rögzített 'missing_result' jeleket előbb meg kell oldani (resolved_at),
-- hogy a szűkebb konstans ne utasítsa vissza a következő upsertet.
