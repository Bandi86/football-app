-- 0008 — S2d-1 séma-rések: odds_quote + match_stats + referee/venue (S2d-1).
-- Visszaállítás: az odds jsonb újraépíthető a default/closing quote-okból
-- (lásd a lenti inverz mintát), a két új tábla és a két entitás-FK dropolható.
-- Inverz minta (kézi, dokumentált):
--   ALTER TABLE fact.odds_snapshot ADD COLUMN odds jsonb;
--   UPDATE fact.odds_snapshot o SET odds = (
--     SELECT jsonb_object_agg(k.field, q.price) FROM fact.odds_quote q
--     JOIN (VALUES ('home','1x2','home'),('draw','1x2','draw'),
--                  ('away','1x2','away'),('btts_yes','btts','yes'),
--                  ('btts_no','btts','no'),('over25','ou','over'),
--                  ('under25','ou','under')) AS k(field, market, outcome)
--       ON q.market=k.market AND q.outcome=k.outcome
--      WHERE q.match_id=o.match_id AND q.source_id=o.source_id
--        AND q.bookmaker='default' AND q.phase='closing'
--        AND (q.market <> 'ou' OR q.line = 2.5));
BEGIN;

CREATE TABLE fact.odds_quote (
    match_id uuid NOT NULL,
    source_id uuid NOT NULL,
    bookmaker text NOT NULL,
    market text NOT NULL,
    outcome text NOT NULL,
    line numeric(6,2) NOT NULL DEFAULT 0,
    phase text NOT NULL CHECK (phase IN ('opening', 'closing')),
    price numeric(8,3) NOT NULL CHECK (price > 1),
    PRIMARY KEY (match_id, source_id, bookmaker, market, outcome, line, phase),
    FOREIGN KEY (match_id, source_id)
        REFERENCES fact.odds_snapshot (match_id, source_id) ON DELETE CASCADE,
    CONSTRAINT odds_quote_line_check CHECK (line = 0 OR line > 0)
);

CREATE INDEX idx_odds_quote_match_market
    ON fact.odds_quote (match_id, market, outcome);

-- Backfill a meglévő 7 mezős JSON-ból (default bookmaker, closing fázis).
INSERT INTO fact.odds_quote
    (match_id, source_id, bookmaker, market, outcome, line, phase, price)
SELECT o.match_id, o.source_id, 'default', k.market, k.outcome, k.line, 'closing',
       (o.odds->>k.field)::numeric
FROM fact.odds_snapshot o
CROSS JOIN (VALUES
    ('1x2', 'home', 'home', 0::numeric),
    ('1x2', 'draw', 'draw', 0),
    ('1x2', 'away', 'away', 0),
    ('btts', 'yes', 'btts_yes', 0),
    ('btts', 'no', 'btts_no', 0),
    ('ou', 'over', 'over25', 2.5),
    ('ou', 'under', 'under25', 2.5)
) AS k(market, outcome, field, line)
WHERE o.odds ? k.field;

ALTER TABLE fact.odds_snapshot DROP COLUMN odds;

CREATE TABLE fact.match_stats (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    home_shots integer CHECK (home_shots >= 0),
    away_shots integer CHECK (away_shots >= 0),
    home_shots_on_target integer CHECK (home_shots_on_target >= 0),
    away_shots_on_target integer CHECK (away_shots_on_target >= 0),
    home_corners integer CHECK (home_corners >= 0),
    away_corners integer CHECK (away_corners >= 0),
    home_yellow integer CHECK (home_yellow >= 0),
    away_yellow integer CHECK (away_yellow >= 0),
    home_red integer CHECK (home_red >= 0),
    away_red integer CHECK (away_red >= 0),
    home_fouls integer CHECK (home_fouls >= 0),
    away_fouls integer CHECK (away_fouls >= 0),
    home_xg numeric CHECK (home_xg >= 0),
    away_xg numeric CHECK (away_xg >= 0),
    PRIMARY KEY (match_id, source_id)
);

ALTER TABLE core.match ADD COLUMN referee_entity_id uuid REFERENCES core.entity(id);
ALTER TABLE core.match ADD COLUMN venue_entity_id uuid REFERENCES core.entity(id);

COMMIT;