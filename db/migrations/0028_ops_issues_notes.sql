BEGIN;

-- S168: üzemeltetési hiányjelek és meccsjegyzetek tartós tárolása.
-- Az ops.validation_finding futásonkénti nyers szabályeredmény; az ops.issue
-- ezzel szemben futásokon átívelő életciklus (mióta látszik, mikor látták
-- utoljára, megoldva-e). A hiány maga NULL/státusz, nem kitalált érték.

CREATE TABLE IF NOT EXISTS ops.issue (
    id uuid PRIMARY KEY,
    subject_kind text NOT NULL CHECK (subject_kind IN ('match', 'team', 'competition')),
    subject_id uuid NOT NULL,
    code text NOT NULL CHECK (code IN (
        'missing_logo', 'missing_odds', 'open_past', 'duplicate_suspect',
        'identity_review', 'stale_status', 'source_conflict'
    )),
    severity text NOT NULL CHECK (severity IN ('info', 'warning', 'hard')),
    detail jsonb NOT NULL DEFAULT '{}'::jsonb,
    first_seen_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz,
    source text NOT NULL CHECK (source IN ('daily-check', 'integrity', 'manual', 'agent'))
);

-- Ugyanaz a jel egy nyitott sor marad, a detector csak a last_seen_at-t frissíti.
CREATE UNIQUE INDEX IF NOT EXISTS uq_issue_open
    ON ops.issue (subject_kind, subject_id, code, source)
    WHERE resolved_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_issue_subject
    ON ops.issue (subject_kind, subject_id);
CREATE INDEX IF NOT EXISTS idx_issue_open_last_seen
    ON ops.issue (last_seen_at DESC)
    WHERE resolved_at IS NULL;

CREATE TABLE IF NOT EXISTS ops.match_note (
    id uuid PRIMARY KEY,
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    author text NOT NULL CHECK (length(btrim(author)) > 0),
    body text NOT NULL CHECK (length(btrim(body)) > 0),
    kind text NOT NULL CHECK (kind IN ('info', 'problem', 'fix', 'question', 'resolved')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz
);

CREATE INDEX IF NOT EXISTS idx_match_note_match
    ON ops.match_note (match_id, created_at DESC);

-- A feltöltött logók megkülönböztetése az importáltaktól; a feltöltési
-- végpont és a formátum-/méretellenőrzés az S5 szeletben jön.
ALTER TABLE ops.asset
    ADD COLUMN IF NOT EXISTS origin text NOT NULL DEFAULT 'import'
        CHECK (origin IN ('import', 'user_upload')),
    ADD COLUMN IF NOT EXISTS uploaded_by text,
    ADD COLUMN IF NOT EXISTS original_filename text,
    ADD COLUMN IF NOT EXISTS mime_type text;

COMMIT;

-- Visszaállítás (külön, egyeztetett lépés):
--   DROP TABLE IF EXISTS ops.match_note;
--   DROP TABLE IF EXISTS ops.issue;
--   ALTER TABLE ops.asset DROP COLUMN IF EXISTS mime_type,
--       DROP COLUMN IF EXISTS original_filename,
--       DROP COLUMN IF EXISTS uploaded_by,
--       DROP COLUMN IF EXISTS origin;
-- majd a _sqlx_migrations 0028-as sorának törlése.
