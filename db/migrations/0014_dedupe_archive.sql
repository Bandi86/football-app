-- 0014 — S16: duplikált kanonikus meccsek merge-archívuma.
-- A dedupe/merge eszköz a törölt (áldozat) sor teljes JSONB képét ide menti,
-- így a kanonikus tisztítás visszafordítható (a sor újraépíthető a payloadból).
-- Visszaállítás: a tábla dropja (az archívum csak audit/visszaállítási cél).
BEGIN;

CREATE TABLE ops.dedupe_archive (
    id uuid PRIMARY KEY,
    merged_at timestamptz NOT NULL DEFAULT now(),
    survivor_id uuid NOT NULL,
    duplicate_id uuid NOT NULL,
    note text NOT NULL,
    payload jsonb NOT NULL
);

CREATE INDEX idx_dedupe_archive_duplicate ON ops.dedupe_archive (duplicate_id);

COMMIT;
