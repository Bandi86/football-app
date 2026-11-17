-- 0015 — Csapat-entitás merge archívum (visszaállítási út).
-- A merge a teljes előtte-állapotot (entity, team, aliasok, refek, tabella-
-- sorok, assetek, érintett meccs-azonosítók) ide menti JSONB payloadként.
-- Visszaállítás: DROP TABLE ops.entity_merge_archive.
BEGIN;

CREATE TABLE ops.entity_merge_archive (
    id uuid PRIMARY KEY,
    merged_at timestamptz NOT NULL DEFAULT now(),
    survivor_entity_id uuid NOT NULL,
    duplicate_entity_id uuid NOT NULL,
    note text,
    payload jsonb NOT NULL
);

CREATE INDEX idx_entity_merge_archive_duplicate
    ON ops.entity_merge_archive (duplicate_entity_id);

COMMIT;
