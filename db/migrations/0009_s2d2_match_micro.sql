-- 0009 — S2d-2 meccs-mikro séma: team_stats, player_stats, lineups,
-- shots, incidents. Visszaállítás: az öt tábla dropja.
BEGIN;

CREATE TABLE fact.match_team_stats (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    side text NOT NULL CHECK (side IN ('home','away')),
    period text NOT NULL CHECK (period IN ('all','first_half','second_half')),
    stat_key text NOT NULL,
    value numeric,
    PRIMARY KEY (match_id, source_id, side, period, stat_key)
);

CREATE INDEX idx_match_team_stats_match_side
    ON fact.match_team_stats (match_id, side);

CREATE TABLE fact.match_player_stats (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    source_player_id text NOT NULL,
    player_entity_id uuid REFERENCES core.entity(id),
    side text NOT NULL CHECK (side IN ('home','away')),
    stat_key text NOT NULL,
    value numeric,
    PRIMARY KEY (match_id, source_id, source_player_id, stat_key)
);

CREATE INDEX idx_match_player_stats_player
    ON fact.match_player_stats (source_player_id);

CREATE TABLE fact.match_lineups (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    side text NOT NULL CHECK (side IN ('home','away')),
    source_player_id text NOT NULL,
    player_entity_id uuid REFERENCES core.entity(id),
    role text NOT NULL CHECK (role IN ('starter','sub')),
    position text,
    shirt_number integer CHECK (shirt_number > 0),
    rating numeric(3,1),
    PRIMARY KEY (match_id, source_id, side, source_player_id)
);

CREATE INDEX idx_match_lineups_match_side ON fact.match_lineups (match_id, side);
CREATE INDEX idx_match_lineups_player ON fact.match_lineups (source_player_id);

CREATE TABLE fact.match_shots (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    shot_no integer NOT NULL CHECK (shot_no > 0),
    side text NOT NULL CHECK (side IN ('home','away')),
    source_player_id text,
    player_entity_id uuid REFERENCES core.entity(id),
    minute integer,
    x numeric,
    y numeric,
    xg numeric,
    xgot numeric,
    shot_type text,
    PRIMARY KEY (match_id, source_id, shot_no)
);

CREATE INDEX idx_match_shots_match ON fact.match_shots (match_id);

CREATE TABLE fact.match_incidents (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    incident_no integer NOT NULL CHECK (incident_no > 0),
    side text NOT NULL CHECK (side IN ('home','away')),
    incident_type text NOT NULL,
    player_source_id text,
    player_entity_id uuid REFERENCES core.entity(id),
    related_player_source_id text,
    related_player_entity_id uuid REFERENCES core.entity(id),
    minute integer,
    detail text,
    PRIMARY KEY (match_id, source_id, incident_no)
);

CREATE INDEX idx_match_incidents_match ON fact.match_incidents (match_id);

COMMIT;