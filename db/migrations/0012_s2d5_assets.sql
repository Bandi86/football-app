-- 0012 — S2d-5 (F csoport): asset-nyilvántartás licenc/attribúcióval.
-- Minden megjelenített assethez forrás, rögzítési idő és licencállapot
-- tartozik (AGENTS §6); a licenc bizonytalansága 'unknown', a tiltott 'rejected'.
-- Visszaállítás: a tábla dropja.
BEGIN;

CREATE TABLE ops.asset (
    id uuid PRIMARY KEY,
    entity_kind text NOT NULL,
    entity_id uuid,
    kind text NOT NULL CHECK (kind IN ('logo','flag','other')),
    url text NOT NULL,
    storage_key text,
    source text NOT NULL,
    licence text,
    licence_status text NOT NULL DEFAULT 'unknown'
        CHECK (licence_status IN ('unknown','cleared','rejected')),
    attribution text,
    captured_at timestamptz NOT NULL,
    UNIQUE (entity_kind, entity_id, kind, url)
);

CREATE INDEX idx_asset_entity ON ops.asset (entity_kind, entity_id);

COMMIT;
