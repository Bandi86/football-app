-- 0019 — research detail események és csapatstatisztikák (S82).
--
-- A research detail JSON `events[]` tömbje és `team_statistics` blokkja eddig
-- kihasználatlan volt; a kanonikus `fact.match_incidents` soraihoz viszont
-- hiányzott a játékosnév (a forrás nem ad stabil player-ID-t), a
-- kapcsolódó játékos neve (assist / lecserélt játékos) és a gól utáni
-- állás. A statisztika a meglévő `fact.match_stats` táblába kerül, forrás-
-- preferenciával a lekérdezésben (S82: ez séma-bővítés, nem adatjavítás).
--
-- A `side` oszlop NOT NULL volt, de a research az események egy részénél
-- (félidő, hosszabbítás-jelző, VAR) nem ad csapatot; a hiányzó oldal NULL,
-- nem tippelt érték. A CHECK továbbra is csak a 'home'/'away' értékeket
-- engedi (a NULL-t a CHECK nem vizsgálja).
--
-- Visszaállítás (tranzakciós migráció, egyetlen lánc része):
--   -- csak akkor, ha nincs NULL oldalú sor (különben a NOT NULL visszaállítás
--   -- elbukik, ami szándékos: előbb kézi döntés kell a sorokról)
--   ALTER TABLE fact.match_incidents ALTER COLUMN side SET NOT NULL;
--   ALTER TABLE fact.match_incidents
--       DROP COLUMN player_name,
--       DROP COLUMN related_player_name,
--       DROP COLUMN score_after_home,
--       DROP COLUMN score_after_away;
-- A visszaállítás után a research feldolgozás a plusz mezőket nem írhatja.

BEGIN;

ALTER TABLE fact.match_incidents
    ADD COLUMN player_name text,
    ADD COLUMN related_player_name text,
    ADD COLUMN score_after_home smallint,
    ADD COLUMN score_after_away smallint;

ALTER TABLE fact.match_incidents ALTER COLUMN side DROP NOT NULL;

COMMENT ON COLUMN fact.match_incidents.player_name IS
    'S82: a forrás esemény játékosneve (nincs stabil player-ID a research detailben).';
COMMENT ON COLUMN fact.match_incidents.related_player_name IS
    'S82: assziszt (gól) vagy lecserélt játékos (csere); egyébként NULL.';
COMMENT ON COLUMN fact.match_incidents.score_after_home IS
    'S82: gól utáni hazai állás; csak a vendég oldallal együtt, egyébként NULL.';
COMMENT ON COLUMN fact.match_incidents.score_after_away IS
    'S82: gól utáni vendég állás; csak a hazai oldallal együtt, egyébként NULL.';

COMMIT;