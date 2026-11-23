BEGIN;

-- S113: a már tárolt research meccsdokumentumok gazdag szekciói.
-- A hiány NULL, nem üres objektum és nem kitalált 0. A számított blokkok
-- (preview_hu, prediction) önmagukban hordozzák a `generated`/`verified`
-- jelzőket — a kliens ezeket kötelezően megjeleníti.
CREATE TABLE IF NOT EXISTS fact.match_insight (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    preview_hu jsonb,
    prediction jsonb,
    h2h_streaks jsonb,
    standings jsonb,
    season_stats jsonb,
    injuries jsonb,
    fan_poll jsonb,
    odds_markets jsonb,
    venue_details jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (match_id, source_id)
);

COMMIT;

-- Visszaállítás (külön, egyeztetett lépés):
--   DROP TABLE fact.match_insight;
-- majd a _sqlx_migrations 0027-es sorának törlése.
