-- 0018 — identitás-fold a lookupokhoz (S1).
--
-- A linker eddig `lower()`-ral hasonlított (rust/crates/storage/src/repositories/
-- identity.rs), ami ékezet-érzékeny: "Épinal" és "Epinal" két külön
-- entitásnak látszott. Ráadásul a forrásnevek írásjelekben is eltérnek
-- ("K. Lierse S.K." vs "SK", "Bodø/Glimt" vs "Bodø Glimt"), ezért az exact
-- lookup kulcsa az ékezet-hajtás után a szóközt és a leggyakoribb
-- írásjeleket is elhagyja. A fuzzy kulcs továbbra is a `normalize_name`
-- marad (token-overlap), így a jelöltképzés nem változik.
--
-- Fontos: a függvény PONTOSAN a Rust `football_domain::identity::fold_name`
-- karaktertábláját követi (a kettő egyezését a storage identity_repos teszt
-- bizonyítja: `SELECT identity.fold_name($1)` vs Rust fold_name minta-listára).
-- Ha a Rust tábla bővül, ezt a migrációt új függvény-verzióval kell követni.
-- Szándékosan NINCS `[^a-z0-9]` jellegű szűrés: a nem-latin betűk megmaradnak,
-- csak az explicit karakterkészletet transliteráljuk / dobjuk el.
--
-- Visszaállítás (tranzakciós migráció, egyetlen lánc része):
--   DROP INDEX core.idx_entity_team_fold_name;
--   DROP INDEX core.idx_team_fold_name;
--   DROP INDEX identity.idx_alias_fold_name;
--   DROP FUNCTION identity.fold_name(text);
-- A visszaállítás után a storage lookupjait is vissza kell állítani a
-- lower()-os változatra, különben a lekérdezések hibára futnak.

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
    --   3. szóköz és a leggyakoribb írásjelek elhagyása.
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
        ' .''’-_&/(),',
        ''
    );
$$;

COMMENT ON FUNCTION identity.fold_name(text) IS
    'S1: ékezet-független, kisbetűs, whitespace- és írásjel-mentes névkulcs — a Rust football_domain::identity::fold_name SQL tükre; lookuphoz IMMUTABLE.';

-- A lookupok foldolt kulcson mennek, ezért expression index kell:
--   reviewed_aliases_by_name: identity.fold_name(alias)
--   team_entities_by_normalized_name: identity.fold_name(canonical_name/name)
--   exact kulcs a linkerben: fold_name(proposed_name)
CREATE INDEX IF NOT EXISTS idx_entity_team_fold_name
    ON core.entity (identity.fold_name(canonical_name))
    WHERE kind = 'team';

CREATE INDEX IF NOT EXISTS idx_team_fold_name
    ON core.team (identity.fold_name(name));

CREATE INDEX IF NOT EXISTS idx_alias_fold_name
    ON identity.alias (identity.fold_name(alias));