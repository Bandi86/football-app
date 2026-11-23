BEGIN;

-- A fejléc időjárása a háttérből jön: a kliens többé nem hív közvetlenül
-- külső API-t. A cache kulcsa a koordináták 2 tizedesre kerekített párja
-- (kb. 1 km), így a saját helyzet is kiszolgálható anélkül, hogy tetszőleges
-- számú kulcs halmozódna fel.
CREATE TABLE IF NOT EXISTS ops.weather_cache (
    location_key text PRIMARY KEY,
    latitude double precision NOT NULL,
    longitude double precision NOT NULL,
    payload jsonb NOT NULL,
    observed_at timestamptz,
    captured_at timestamptz NOT NULL DEFAULT now(),
    next_refresh_at timestamptz NOT NULL,
    source text NOT NULL DEFAULT 'open-meteo',
    CONSTRAINT weather_cache_payload_object CHECK (jsonb_typeof(payload) = 'object'),
    CONSTRAINT weather_cache_latitude CHECK (latitude BETWEEN -90 AND 90),
    CONSTRAINT weather_cache_longitude CHECK (longitude BETWEEN -180 AND 180)
);

CREATE INDEX IF NOT EXISTS idx_weather_cache_captured_at
    ON ops.weather_cache (captured_at);

COMMIT;

-- Visszaállítás: DROP TABLE ops.weather_cache;
-- majd a _sqlx_migrations 0024-es sorának törlése (csak egyeztetett, külön lépés).
