# Valós adat bekötése — design specifikáció

**Dátum:** 2026-11-11
**Állapot:** jóváhagyva (felhasználói „mehet” a szekciónkénti designra)
**Előzmény:** a backend (Tasks 1–13) és a SwiftUI „Ma” kliens + főoldal-redesign elkészült és a `main` ágon van. A redesign óta a főoldal `SampleData`-t mutat; ez a szelet a valós adatutat köti be: DB → API → kliens, bővített fixture demóadattal.

## 1. Cél és hatókör

A főoldal a `SampleData` helyett a `footballd` API valós válaszából épül fel. Ehhez a backend új megjelenítési tényezőket kap: liga tier/prioritás, tabella-helyezések, odds snapshot (1X2, BTTS, Over/Under 2,5), piacszám és odds-forrás. A HT/ET/pens eredmény már létezik a `core.match_result` táblában, csak az API és a kliens bekötése hiányzik.

**Adatforrás:** bővített szintetikus fixture demóadat. Valódi forrásadat-betöltés nem része a szeletnek (AGENTS.md §6/§11). A „valós” a valós adatutat jelenti: kanonikus DB → helyi API → natív kliens.

**Nem része a szeletnek:**
- predikciós réteg (a minta-előrejelzés blokk eltűnik a valós adatútról);
- live perc / élő eseménykövetés (a live státuszú meccs pulzál, de perc nélkül);
- odds-történet, piacokra kattintás, részletes mérkőzés-elemzés oldal;
- valódi forrásadapter, női foci, kereső.

## 2. Séma — `0003` migráció

Fájl: `db/migrations/0003_display_facts.sql`, tranzakciós, a migrációs lánc harmadik eleme.

1. `core.competition` bővül:
   - `tier smallint NOT NULL DEFAULT 1 CHECK (tier >= 0)` — 0 = kupa, 1/2/… = osztály;
   - `priority integer NOT NULL DEFAULT 100` — megjelenítési sorrend (kisebb = előrébb).
2. Új `core.standing`:
   - `season_id uuid NOT NULL REFERENCES core.season(id)`,
   - `team_id uuid NOT NULL REFERENCES core.team(id)`,
   - `position smallint NOT NULL CHECK (position > 0)`,
   - `source_id uuid NOT NULL REFERENCES ops.source(id)`,
   - `captured_at timestamptz NOT NULL`,
   - `PRIMARY KEY (season_id, team_id)`.
3. Új `fact.odds_snapshot`:
   - `match_id uuid PRIMARY KEY REFERENCES core.match(id) ON DELETE CASCADE`,
   - `source_id uuid NOT NULL REFERENCES ops.source(id)`,
   - `captured_at timestamptz NOT NULL`,
   - `market_count integer NOT NULL CHECK (market_count >= 0)`,
   - `odds jsonb NOT NULL` — kulcsok: `home`, `draw`, `away`, `btts_yes`, `btts_no`, `over25`, `under25`; pozitív számértékek (a sémaellenőrzés az ingestben történik).

Táblaszám: 21 → 23. A `scripts/db-check.sh` és a storage-kapcsolati teszt elvárásai igazítandók. Visszaállítási út: `scripts/db-reset.sh` a tesztadatbázison (mindhárom migráció újraépül); a migráció fejléce dokumentálja.

**Döntés:** az odds és a tabella forrás + rögzítési ideje közvetlenül a fact táblában él (snapshot-szintű provenance). Mezőszintű odds-provenance és odds-történet nem cél; ha később kell, a `fact.odds_snapshot` sorokra bontott `fact.odds` táblával váltható fel.

## 3. Ingest — fixture v2

A fixture JSON formátuma bővül (visszafelé kompatibilis: a hiányzó odds/standing/tier mezők opcionálisak):

- `competition.tier` (szám, opcionális, alap 1), `competition.priority` (szám, opcionális, alap 100);
- `matches[].odds` (objektum a fenti hét kulccsal, opcionális), `matches[].market_count` (szám, opcionális);
- `standings` (tömb, opcionális): `{ "team_source_entity_id": "...", "position": N }`.

`FixtureAdapter`: a nyers artifact változatlanul a teljes JSON-t tárolja; új `stage.observation` aspectek:
- `match_odds` (entity_kind `match`, aspect `match_odds`, forrás-meccsazonosítóval) — payload: odds + market_count;
- `standing` (entity_kind `team`, aspect `standing`) — payload: position + season-kontextus.

`Canonicalizer::materialize`: a meglévő kanonikus entitások mellett idempotensen (ON CONFLICT) írja:
- `fact.odds_snapshot` — a legfrissebb `match_odds` megfigyelésből, `source_id` + `captured_at` a capture-ből;
- `core.standing` — a `standing` megfigyelésekből, csapatonként a legfrissebb érték;
- `core.competition.tier/priority` — a fixture competition blokkjából (upsert).

A verification szabályok és a publikálási kapu változatlan (meccsszintű). Az odds/standing nem kapu-tényező.

## 4. API — `/v1/day` bővítés

A `DayMatchRow` és a JSON válasz bővül (a meglévő mezők változatlanok, minden új mező opcionális a kliensben):

- `competition_id`, `competition_name`, `country_code`, `country_name`, `tier`, `priority`;
- `home_ht`, `away_ht`, `home_et`, `away_et`, `home_pens`, `away_pens` (a `core.match_result`-ből);
- `home_position`, `away_position` (LEFT JOIN `core.standing` a meccs szezonjára);
- `odds` (jsonb), `market_count`, `odds_source` (`ops.source.display_name`), `odds_captured_at`.

SQL: LEFT JOIN `fact.odds_snapshot` + `ops.source`, kétszer LEFT JOIN `core.standing` (hazai/vendég). A `fetch_day` szűrése és sorrendje változatlan. A route-ok és az auth változatlan.

## 5. Kliens — valós adatút

**Core (`FootballAppCore`):** a `DayMatch` új opcionális mezőkkel bővül (snake_case CodingKeys), új `OddsSnapshot` struktúra. A régi (bővítés előtti) API-válasz továbbra is dekódolódik. Új decoding tesztek.

**App (`FootballApp`):**
- A `HomeView` a `DayModel` válaszából épít liga-csoportokat: csoportosítás `competition_id` szerint, sorrend `priority`, majd országnév; a státusz-chipek, ország- és osztályszűrők valós adatból számolódnak.
- Zászló + magyar országnév kliensoldali `country_code` → (zászló, név) térképből; ismeretlen kódnál a `country_name` jelenik meg.
- A kártya a valós úton: HT/ET/pens eredmény-blokk, odds nyerő-kiemeléssel lezárt meccsnél, „Piacok (N) · forrás: {odds_source}”, tabella-helyezés zárójelben csak bajnoki meccsnél (tier ≥ 1).
- Eltűnik a valós útról: predikciós blokk, „MINTA ADAT” jelzés, live perc.
- `SampleData` megmarad SwiftUI preview-khoz; a meglévő üres/hiba nézetek, nap-navigáció és naptár változatlan.

## 6. Demóadat

Új fixture-pár: `tests/fixtures/demo_day_a.json` és `demo_day_b.json`. A `demo_day_b` a `demo_day_a` értékeivel egyező második forrás (B-* forrásentitás-ID-k, későbbi capture-idő) — így a meccsek `verified_publish` állapotot kapnak (a SINGLE_SOURCE warning két egyező forrással nem blokkol).

Tartalom: dátum **2026-11-11** (a mai nap, hogy az app induláskor tartalommal nyíljon), ~13 liga / ~20 meccs a minta struktúráját tükrözve: top-5 liga + NB I (1. sáv), alsóbb osztályok (2. sáv), FA Kupa hosszabbítással + 11-esekkel; vegyes státuszok (scheduled/live/finished), odds minden meccsen, tabella a bajnoki csapatokhoz.

Betöltés: `footballctl ingest-fixture` a **dev** adatbázison (a felhasználó által engedélyezett demóadat-állapot bővül); a tesztadatbázis szigorú marad (23/6/0 nyugalmi állapot). A 2026-11-09-i fixture nap megmarad.

## 7. Tesztelés és bizonyítékok

Szeletenként TDD (piros → zöld), az AGENTS.md §8 szerint:
- migráció: tesztadatbázis-first, tábla/számláló evidence, constraint-próba;
- ingest/kanonizáló: integrációs tesztek a tesztadatbázison, utána `db-reset.sh`;
- API: storage + route tesztek;
- kliens: `swift test` (Core) és `xcodebuild test` (app);
- zárás: `./scripts/check.sh`, `db-check.sh`, és **vizuális ellenőrzés a futó appban** (daemon + screenshotok: liga-lista, szűrők, naptár, odds-kiemelés, kupa eredmény-szekvencia, tabella-helyezések);
- magyar szövegek visszaolvasása minden Write/Edit után.

## 8. Munkamenet

- Branch: `feat/real-data-wiring` (nem közvetlenül a `main`).
- Commit/push csak kifejezett felhasználói kérésre.
- Checkpoint: `docs/checkpoints/2026-11-11/checkpoint.md`.
- Megállási feltétel: piros ellenőrzésnél megállunk, megértjük és javítjuk a hibát; nem nyomjuk el.