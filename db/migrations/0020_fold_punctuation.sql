-- 0020 — az identitás-fold írásjel-készletének bővítése (S85).
--
-- Az 0018 identity.fold_name a leggyakoribb írásjeleket dobta el, de a ':'
-- és a ';' kimaradt: a "Primera Division: Apertura" foldolt kulcsa
-- "primeradivision:apertura" maradt, míg a "Primera Division - Apertura"
-- kulcsa "primeradivisionapertura" lett, ezért a kettő külön verseny-
-- entitást kapott. Ugyanez a hiba a "Liga Nacional; Apertura" és a
-- "Liga Nacional Apertura" esetén is jelentkezett.
--
-- Ez a migráció az 0018 függvényét definiálja újra UGYANAZZAL a
-- karaktertáblával (normalize + transliteráció + combining dot eldobása),
-- csak a filterlista bővül a ':' és ';' karakterrel. A függvény
-- IMMUTABLE, ezért a rajta épült expression indexek a régi szemantikával
-- készültek el; ezeket DROP + CREATE újra kell építeni UGYANAZOKKAL a
-- nevekkel és definíciókkal, különben a lookupok a régi kulcsokra
-- találnának.
--
-- A migráció tranzakciós: a sqlx::migrate! minden migrációt egyetlen
-- tranzakcióban futtat, ezért itt nincs külön BEGIN/COMMIT (a 0018-cal
-- egyező stílus).
--
-- Visszaállítás (előző függvény-verzió, 0018):
--   -- 1) az 0018 függvénytörzsével CREATE OR REPLACE FUNCTION
--   --    identity.fold_name(text), a filterlista végén ':' és ';' nélkül;
--   DROP INDEX core.idx_entity_team_fold_name;
--   DROP INDEX core.idx_team_fold_name;
--   DROP INDEX identity.idx_alias_fold_name;
--   -- 2) az 0018 indexdefinícióival CREATE INDEX újra (core.entity partial
--   --    kind='team', core.team, identity.alias), majd a 0020 törlése a
--   --    _sqlx_migrations táblából, ha a láncot kézzel állítjuk vissza.
-- A migráció adatot nem módosít: a meglévő sorok foldolt kulcsa a
-- lekérdezésekben a függvényből számolódik újra, a rá épülő indexeket
-- viszont újra kell építeni (ezt teszi ez a fájl).

CREATE OR REPLACE FUNCTION identity.fold_name(input text)
RETURNS text
LANGUAGE sql
IMMUTABLE
STRICT
PARALLEL SAFE
AS $$
    -- Sorrend egyezik a Rust implementációval:
    --   1. normalize_name: split_whitespace + " " kötés + to_lowercase,
    --   2. karakterenkénti ékezet-hajtás (a U+0307 combining dot eltávolítása
    --      a török nagy İ kisbetűsítésének maradéka),
    --   3. szóköz és a leggyakoribb írásjelek elhagyása (S85: ':' és ';' is).
    SELECT translate(
        translate(
            replace(
                lower(btrim(regexp_replace(input, '\s+', ' ', 'g'))),
                chr(775),
                ''
            ),
            'áàâäãåąéèêëęíìîïıóòôöőõøúùûüűçćčñńšśşžźżýÿđłřťğħŧßæœðþ',
            'aaaaaaaeeeeeiiiiiooooooouuuuucccnnssszzzyydlrtghtsaodt'
        ),
        ' .''’-_&/(),:;',
        ''
    );
$$;

COMMENT ON FUNCTION identity.fold_name(text) IS
    'S85: ékezet-független, kisbetűs, whitespace- és írásjel-mentes névkulcs (a ":" és ";" is eldobva) — a Rust football_domain::identity::fold_name SQL tükre; lookuphoz IMMUTABLE.';

-- A lookupok foldolt kulcson mennek, ezért a régi szemantikájú expression
-- indexeket újra kell építeni (a nevek/definíciók változatlanok).
DROP INDEX IF EXISTS core.idx_entity_team_fold_name;
DROP INDEX IF EXISTS core.idx_team_fold_name;
DROP INDEX IF EXISTS identity.idx_alias_fold_name;

CREATE INDEX idx_entity_team_fold_name
    ON core.entity (identity.fold_name(canonical_name))
    WHERE kind = 'team';

CREATE INDEX idx_team_fold_name
    ON core.team (identity.fold_name(name));

CREATE INDEX idx_alias_fold_name
    ON identity.alias (identity.fold_name(alias));