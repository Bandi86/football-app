BEGIN;

-- S218/O0: a request_key stabil kérésidentitás, nem capture-identitás.
-- Régi capture-okat nem törlünk; a változás csak az ismétlődő fetch-ek
-- megőrzését és a provenance pontos megkötését teszi lehetővé.

ALTER TABLE raw.capture
    DROP CONSTRAINT IF EXISTS capture_source_id_request_key_key;

CREATE INDEX IF NOT EXISTS idx_raw_capture_latest
    ON raw.capture (source_id, request_key, requested_at DESC, id DESC);

COMMIT;

-- Visszaállítás: csak akkor adható vissza a
-- capture_source_id_request_key_key UNIQUE (source_id, request_key) constraint,
-- ha a migráció óta keletkezett ismétlődő capture-k előbb archiválhatók vagy
-- egyértelművé válnak. A 0032-es _sqlx_migrations sor törlése önmagában nem
-- visszafordítható, ha már több capture ugyanazzal a request_key-vel létezik.
