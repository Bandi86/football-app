-- 0022 — meccs-hivatalos személy (elsősorban játékvezető) forrásonként (S97).
--
-- Miért kell új tábla: a research envelope meccsenként adja a játékvezető
-- nevét (`referee: {name, country, source}`), de a név alapján a katalógus
-- entitásra oldása gyakran nem egzakt (a footballdata rövid „M. Marchetti"
-- formát használ, a research teljes keresztnevet). Az entitás-feloldás
-- politikája szándékosan fail-closed: nem egzakt névnél nincs automatikus
-- link, csak review-sor. Emiatt a `core.match.referee_entity_id` a legtöbb
-- meccsen NULL maradna, és a felhasználó nem látna játékvezetőt.
--
-- A megoldás: a forrás által KIMONDOTT nevet forrásonként eltároljuk
-- (source-attributed megfigyelés), entitás-identitás állítása nélkül. A
-- részletező API az entitás kanonikus nevét preferálja, és csak akkor esik
-- vissza erre a névre, ha az entitás-link nem jött létre. Így a megjelenített
-- tényhez tartozik forrás (source_id) és rögzítési idő (captured_at).
--
-- A hiányzó adat NULL, soha nem üres string: a forrás nem minden meccsnél ad
-- játékvezetőt (a 2026-11-21-i research napon 294/1024 meccsen volt).
--
-- Visszaállítás (tranzakciós migráció, egyetlen lánc része):
--   DROP TABLE fact.match_official;
-- A visszaállítás után az ingest játékvezető-írása sémahibával áll meg
-- (fail-closed), nem esik csendben vissza a név nélküli állapotra.
--
-- Adatot nem módosít; a meglévő sorokat nem érinti.

BEGIN;

CREATE TABLE fact.match_official (
    match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE CASCADE,
    source_id uuid NOT NULL REFERENCES ops.source(id),
    role text NOT NULL,
    name text NOT NULL CHECK (btrim(name) <> ''),
    captured_at timestamptz NOT NULL,
    PRIMARY KEY (match_id, source_id, role)
);

COMMENT ON TABLE fact.match_official IS
    'S97: forrás által kimondott meccs-hivatalos személy (role=referee) forrásonként; nem kanonikus entitás-identitás.';
COMMENT ON COLUMN fact.match_official.name IS
    'A forrás által megadott, ember által olvasható név; az entitás-link (core.match.referee_entity_id) ettől független.';
COMMENT ON COLUMN fact.match_official.role IS
    'Szerep (ma referee); a PK része, hogy egy meccsen forrásonként szerepenként egy név éljen.';

COMMIT;