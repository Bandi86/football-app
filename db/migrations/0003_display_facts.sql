-- 0003 — megjelenítési tények: competition tier/priority, tabella, odds snapshot.
-- A sqlx a public._sqlx_migrations táblában tartja nyilván; ismételt futtatása nem hajtódik végre újra.
-- Visszaállítási út: scripts/db-reset.sh (kizárólag a tesztadatbázison), amely minden sémát és
-- a migrációs nyilvántartást törli, majd a 0001–0003 migrációkat együtt újrafuttatja.

BEGIN;

ALTER TABLE core.competition
    ADD COLUMN tier smallint NOT NULL DEFAULT 1 CHECK (tier >= 0),
    ADD COLUMN priority integer NOT NULL DEFAULT 100;

CREATE TABLE core.standing (
    season_id uuid NOT NULL REFERENCES core.season(id),
    team_id uuid NOT NULL REFERENCES core.team(id),
    position smallint NOT NULL CHECK (position > 0),
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    PRIMARY KEY (season_id, team_id)
);

CREATE TABLE fact.odds_snapshot (
    match_id uuid PRIMARY KEY REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    market_count integer NOT NULL CHECK (market_count >= 0),
    odds jsonb NOT NULL
);

COMMIT;