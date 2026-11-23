BEGIN;

-- S108: játékos-identitás és időben érvényes csapattagság.
-- A játékos alapadatai a core.entity-ben élnek (kind='player'), itt csak a
-- játékos-specifikus, opcionális mezők. A hiány NULL, nem kitalált érték.
CREATE TABLE IF NOT EXISTS core.player (
    entity_id uuid PRIMARY KEY REFERENCES core.entity(id) ON DELETE CASCADE,
    country_id uuid REFERENCES core.country(id),
    position_code text,
    date_of_birth date,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- A jelenlegi csapat nem egyetlen felülírt mező, hanem az időben érvényes
-- kapcsolatból számított nézet (spec §6.3.2). A 2026/11-es research ablakból
-- csak pillanatkép-tagság rekonstruálható — ez tudatos korlát.
CREATE TABLE IF NOT EXISTS core.player_team_spell (
    id uuid PRIMARY KEY,
    player_entity_id uuid NOT NULL REFERENCES core.entity(id) ON DELETE CASCADE,
    team_entity_id uuid NOT NULL REFERENCES core.entity(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    valid_from date NOT NULL,
    valid_to date,
    loan boolean NOT NULL DEFAULT false,
    captured_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT player_team_spell_unique UNIQUE
        (player_entity_id, team_entity_id, source_id, valid_from),
    CONSTRAINT player_team_spell_check CHECK (
        player_entity_id <> team_entity_id AND (valid_to IS NULL OR valid_to >= valid_from)
    )
);

CREATE INDEX IF NOT EXISTS idx_player_team_spell_player
    ON core.player_team_spell (player_entity_id, valid_from);

COMMIT;

-- Visszaállítás (külön, egyeztetett lépés):
--   DROP TABLE IF EXISTS player_team_spell;
--   DROP TABLE IF EXISTS core.player;
-- majd a _sqlx_migrations 0026-os sorának törlése.
