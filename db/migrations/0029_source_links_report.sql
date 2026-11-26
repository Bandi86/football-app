BEGIN;

-- S181 (S2): a research magyar riportja és a kattintható forráslinkek.
-- A match_insight 9 szekciója változatlan; a riport új, opcionális oszlop,
-- hiány esetén NULL (soha üres objektum vagy kitalált szöveg).
ALTER TABLE fact.match_insight
    ADD COLUMN IF NOT EXISTS match_report_hu jsonb;

-- Egy meccshez több, különböző célú URL tartozhat. Az azonos URL egy sor;
-- a forrás és a rögzítési idő minden sorban megvan (AGENTS.md §6).
CREATE TABLE IF NOT EXISTS fact.match_link (
    id uuid PRIMARY KEY,
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('source', 'reference', 'standing', 'weather', 'other')),
    label text NOT NULL CHECK (length(btrim(label)) > 0),
    url text NOT NULL CHECK (url ~ '^https?://'),
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (match_id, kind, url, source_id)
);

CREATE INDEX IF NOT EXISTS idx_match_link_match
    ON fact.match_link (match_id, kind);

COMMIT;

-- Visszaállítás (külön, egyeztetett lépés):
--   DROP TABLE IF EXISTS fact.match_link;
--   ALTER TABLE fact.match_insight DROP COLUMN IF EXISTS match_report_hu;
--   majd a _sqlx_migrations 0029-es sorának törlése.
