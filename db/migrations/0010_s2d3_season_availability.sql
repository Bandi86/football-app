-- 0010 — S2d-3 séma-rések: szezon-statisztikák + játékos-elérhetőség.
-- Visszaállítás: a három tábla dropja.
BEGIN;

CREATE TABLE fact.season_team_stats (
    season_id uuid NOT NULL REFERENCES core.season(id) ON DELETE CASCADE,
    team_id uuid NOT NULL REFERENCES core.team(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    stat_key text NOT NULL,
    value numeric,
    PRIMARY KEY (season_id, team_id, source_id, stat_key)
);

CREATE TABLE fact.season_player_stats (
    season_id uuid NOT NULL REFERENCES core.season(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    source_player_id text NOT NULL,
    player_entity_id uuid REFERENCES core.entity(id),
    stat_key text NOT NULL,
    value numeric,
    PRIMARY KEY (season_id, source_id, source_player_id, stat_key)
);

CREATE INDEX idx_season_player_stats_player
    ON fact.season_player_stats (source_player_id);

CREATE TABLE fact.player_availability (
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    source_player_id text NOT NULL,
    player_entity_id uuid REFERENCES core.entity(id),
    kind text NOT NULL,
    expected_return date,
    note text,
    PRIMARY KEY (source_id, source_player_id, kind),
    CONSTRAINT player_availability_kind_check CHECK (kind IN ('injury','suspension','unavailable'))
);

COMMIT;
