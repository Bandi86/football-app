-- 0021 — meccs-környezet: stadion és időjárás (S90).
--
-- A research napi envelope meccsenként tartalmaz `venue` (1024/1024 meccsen a
-- 2026-11-21-i napon) és `weather` blokkot (398/1024), a v2 ingest viszont
-- eddig csak a játékvezetőt dolgozta fel: a stadion- és időjárás-adat a
-- nyers payloadban maradt, a DB-ben nem volt helye, és a meccs-részletező API
-- sem adta ki. Ez a migráció létrehozza a meccs-szintű tény-táblát.
--
-- Miért meccs-szintű tény, és miért nem kanonikus entitás: a venue-nak nincs
-- stabil identitása, nincs venue-review és nincs venue-összefonás a
-- rendszerben; a stadion csak a forrás által adott, ember által olvasható
-- név/település/ország/adatok. Ezért a `venue` a meccs pillanatképének
-- része, nem `core.entity`. A `core.match.venue_entity_id` oszlop egyelőre
-- kitöltetlen marad (amíg nincs venue-identitás és review-folyamat).
--
-- A hiányzó adat NULL, soha nem 0: a forrás nem minden meccsnél ad
-- időjárást, és a 0 fok / 0 csapadék / 0 szél a valós adattal keveredne.
-- A PRIMARY KEY (match_id, source_id) azt jelenti, hogy forrásonként egy
-- pillanatkép él (a legutóbbi feldolgozás írja felül), a forrás-preferenciát
-- a lekérdezés dönti el (research előbb, aztán captured_at DESC).
--
-- Visszaállítás (tranzakciós migráció, egyetlen lánc része):
--   DROP TABLE fact.match_environment;
-- A visszaállítás után az ingest `match_environment` observationje nem
-- írhat sémátlanul: a kanonizálásnak hibáznia kell, nem csendben eldobnia.
--
-- Adatot nem módosít; a meglévő sorokat nem érinti.

BEGIN;

CREATE TABLE fact.match_environment (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    captured_at timestamptz NOT NULL,
    venue_name text,
    venue_city text,
    venue_country text,
    venue_capacity integer CHECK (venue_capacity >= 0),
    venue_surface text,
    venue_lat double precision,
    venue_lon double precision,
    temperature_c numeric,
    weather_description text,
    wind_speed numeric,
    wind_direction text,
    humidity_pct integer CHECK (humidity_pct BETWEEN 0 AND 100),
    precipitation numeric,
    cloud_cover_pct integer CHECK (cloud_cover_pct BETWEEN 0 AND 100),
    weather_observed_at timestamptz,
    PRIMARY KEY (match_id, source_id)
);

COMMENT ON TABLE fact.match_environment IS
    'S90: meccs-környezet (stadion + időjárás) forrásonként, a legutóbbi feldolgozás pillanatképe; hiányzó adat NULL.';
COMMENT ON COLUMN fact.match_environment.venue_name IS
    'A forrás stadion-neve; nem kanonikus venue-entitás (nincs venue-review), ezért meccs-szintű tény.';
COMMENT ON COLUMN fact.match_environment.venue_capacity IS
    'A forrás nézőkapacitása; ismeretlen esetén NULL (nem 0).';
COMMENT ON COLUMN fact.match_environment.temperature_c IS
    'A forrás által jelentett hőmérséklet Celsius-fokban; a forrás egység-mezőjét (celsius) a parser normalizálja.';
COMMENT ON COLUMN fact.match_environment.weather_observed_at IS
    'Az időjárás-forrás megfigyelési ideje; hiányzó értéknél NULL.';

COMMIT;