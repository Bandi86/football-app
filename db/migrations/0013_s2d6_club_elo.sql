-- 0013 — S2d-6 (G csoport): club_elo pillanatkép (clubelo tükör).
-- As-of kezelés: soronként a rating dátuma; a provenance címke KÖTELEZŐ, és
-- a 2025-06-01 előtti adat csak 'provisional' lehet (a historikus forrás
-- visszamenőlegesen módosulhat — jövőbeli információ nem szivároghat).
-- Visszaállítás: a tábla dropja.
BEGIN;

CREATE TABLE fact.club_elo (
    country_code text NOT NULL,
    club_name text NOT NULL,
    club_entity_id uuid REFERENCES core.entity(id),
    as_of date NOT NULL,
    elo numeric NOT NULL,
    provenance text NOT NULL CHECK (provenance IN ('observed','provisional')),
    source text NOT NULL DEFAULT 'clubelo',
    captured_at timestamptz NOT NULL,
    PRIMARY KEY (country_code, club_name, as_of),
    CONSTRAINT club_elo_boundary_check
        CHECK (provenance = 'provisional' OR as_of >= DATE '2025-06-01')
);

CREATE INDEX idx_club_elo_entity ON fact.club_elo (club_entity_id, as_of);

COMMIT;
