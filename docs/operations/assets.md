# Asset-tár — üzemeltetési útmutató (S4d/S8-előkészítés)

A `ops.asset` tábla entitás-szintű asset-regisztráció (logó, zászló, egyéb)
licenc- és származás-követéssel (0012 migráció). Az import operátori
művelet; a bináris tartalom a helyi objektumtárba kerül
(`DATA_DIR/raw/assets/<aa>/<bb>/<sha256>.<ext>`), az asset-sor a
tartalom-címzett `storage_key`-t és az eredeti `url`-t hordozza.

## 1. Import-formátum

`footballctl asset import --file <json>` — JSON-tömb, soronként:

| mező | kötelező | jelentés |
|---|---|---|
| `entity_kind` | igen | `team` / `competition` / … |
| `entity_id` | nem | kanonikus entitás UUID |
| `kind` | igen | `logo` / `flag` / `other` |
| `url` | igen | eredeti forrás-URL vagy `file://` hivatkozás |
| `file` | nem | helyi fájl; beolvasás → blob a tárba |
| `sha256` | nem | várt hash; eltérésnél az import megáll |
| `storage_key` | nem | kész blob-hivatkozás (kézi import) |
| `source` | igen | forrás-azonosító (pl. `v1-audited-assets`) |
| `licence`, `licence_status`, `attribution` | nem | licenc; default `unknown` |
| `captured_at` | nem | RFC3339; default most |

Az upsert natural-key: `(entity_kind, entity_id, kind, url)`; a meglévő
`storage_key` nem veszik el, ha egy későbbi sor nem hoz blobot.

## 2. v1-es logó-reuse (2026-11-15)

A felhasználói döntés szerint a v1 auditált logók újrahasznosíthatók
(belső, egygépes használat). Az import **exact név + ország** párosítással
készült (nincs fuzzy egyeztetés):

- 86 élő (footballdata) csapatból **30** kapott helyi blobot (v1
  tartalom-címzett `.webp`, sha-ellenőrzéssel), **14** kapott forrás-URL-t
  (v1 `teams.logo_url`, luukhopman/football-logos GitHub),
  **20**-nak nincs elérhető assetje, **22** név nincs a v1-ben
  (eltérő rövidítések, pl. `Ath Madrid`, `Bayern Munich`).
- Ligák: Premier League, Bundesliga, Serie A, Ligue 1 (4). A spanyol
  top liga v1-tornamentje (`Primera División`, id 110) **nem** szerepel
  a liga-logó manifestben, és a külső GitHub-gyűjtemény csak klub-logókat
  tartalmaz → a La Liga liga-logó **operátori forrásra vár** (a kliens
  a liga-fejlécben egyelőre zászlót mutat, nem logót).
- **Licenc:** a v1 logók forrásai (sofascore/megapack/GitHub-gyűjtemény)
  nem adnak dokumentált licencet. A felhasználói döntés (2026-11-15,
  a v1-audit újrahasznosítás jóváhagyása) alapján a v1-audited sorok
  `licence_status = 'cleared'`, licence-szöveg:
  „v1 audit 2026-11-03 újrahasznosítás (belső használat)".
  Új forrás felvételekor a licencet ismét ellenőrizni kell.

## 3. Ellenőrzés

```sql
SELECT entity_kind, count(*), count(storage_key) FROM ops.asset GROUP BY 1;
SELECT count(*) FROM ops.asset WHERE licence_status <> 'cleared';
```

A blob-integráció ellenőrzése: a `storage_key` fájlnév-utótagja a
sha256; `shasum -a 256 data/raw/<storage_key>` egyezik.

## 4. Visszaállítási út

- Sorok törlése: `DELETE FROM ops.asset WHERE source = 'v1-audited-assets';`
- Blobok: a tartalom-címzett fájlok a `data/raw/assets/` alatt maradnak
  (árva blobok ártalmatlanok, később GC-vel takaríthatók).
- Az import nem ír kanonikus adatot; a visszafordítás veszteségmentes.
