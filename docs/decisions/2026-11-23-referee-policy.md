# 2026-11-23 — Játékvezető-identitás és ország policy

## Döntés

A játékvezetők identitása **fail-closed, megjelenítés-központú** marad:

1. **Nincs auto-create.** A research bíró-nevei nem hoznak létre `core.entity` sort.
2. **Nincs automatikus fuzzy egyeztetés.** Csak TELJES név egzakt, foldolt egyezése linkelhet.
3. **A bíró saját országa tárolódik** (`core.entity.country_id`, `fact.match_official.country_code`);
   a hiány `NULL`, nem a versenyország kölcsönzése.
4. **Ország-egyezés csak akkor szűr, ha mindkét oldalon ismert.** Egyik oldalon NULL
   → wildcard (a mai viselkedés megmarad, nincs adatvesztés).
5. A research továbbra sem nyit review-sort (`open_review: false`); a megjelenített név
   a `fact.match_official`-ból jön. Kézi review jóváhagyásánál a fajtátudatos
   katalógus és az ismert ország `country_id`-ként íródik.

## Indoklás (mért adatok, 2026-11-23, dev DB)

- `fact.match_official`: **564 sor / 564 különböző név**, mind research forrásból.
- Egzakt egyezés a meglévő **1195 referee entitással: 1 / 564**.
- Az entitások **994 / 1195** rövid alakúak (`M. Oliver`-szerű, egybetűs token),
  míg a research 560/564 teljes nevet ad. Ebből a szimpla országoszlop önmagában
  nem növeli a feloldási arányt.
- A `core.entity` korábban nem hordozott országot; a linker ország-szűrése
  referee-nél látens volt (a versenyország csak a review-kontextusba jutott el).
- A napi ~150–200 review-sor automatikus megnyitása ezért nem megoldás, hanem
  újabb feldolgozatlan sorozat: a katalógus névformátuma miatt a legtöbb úgyis
  review-ban maradna.

Ezért a szelet nem a „több entitás", hanem a **helyes kontextus** irányába mozdul:
a bíró saját országa utána már rendelkezésre áll a jövőbeli, Tudatosan bővített
katalógushoz, de nem gyárt hamis egyezést és nem duplikál entitást.

## Következmények

- A jelenlegi 1/564 feloldási arány **nem** nőtt automatikusan; ez tudatos, és a
  következő lépés (a rövid alakú entitások normalizálása/alias-seedelés) külön
  szelet, újabb mérési kapuval.
- A versenyország továbbra is review-kontextusként él, ha a forrás nem ad
  bíró-országot — de nem kerül `fact.match_official`-ba és nem lesz `country_id`.
- A `country_id` minden személyfajtára használható; külön `core.referee` tábla
  nincs (S2d-1 spec döntése).

## Visszaállítás

- `git revert` a szelet commitjára; a 0025 migráció visszaállítása:
  `ALTER TABLE fact.match_official DROP COLUMN country_code;`,
  `DROP INDEX idx_entity_country_id;`,
  `ALTER TABLE core.entity DROP COLUMN country_id;`,
  majd a `_sqlx_migrations` 0025-ös sorának törlése külön lépés.
- Az ingest által utána írt `country_code` értékek elvesznek a visszaállításkor;
  a korábbi review- és audit-előzmény érintetlen.
