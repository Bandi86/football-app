# S2d-1 — S3-előkészítő séma-rések (odds_quote, match_stats, referee/venue)

**Dátum:** 2026-11-14
**Állapot:** brainstorming-jóváhagyott terv; felhasználói spec-review függőben
**Kapcsolódó dokumentumok:** [source-catalog séma-rés lista](../../research/source-catalog.md)
(18 tétel; 1. ciklus: 1., 3., 4., 5., 6.), [S2a spec](../specs/2026-11-12-s2a-ingest-framework-design.md),
[S2b-2 spec](../specs/2026-11-13-s2b2-trust-gate-design.md),
[2026-11-14 checkpoint](../../checkpoints/2026-11-14/checkpoint.md)
**Branch:** `feat/s2d1-schema-gaps` (a `feat/s2c-obscura` csúcsáról — a S2c
még nincs merge-elve a main-be)

## 1. Hatókör

Az S2d első ciklusa az **S3-előkészítő minimum**: a footballdata-adapter
(S3) mezőigényének séma-alapja. Ebben a szeletben:

- **Odds:** `fact.odds_quote` normalizált tábla (bookmaker, market, outcome,
  line, phase, price) + a meglévő `fact.odds_snapshot` fejléc megtartása
  (captured_at, market_count, trust_state); az `odds_snapshot.odds` JSON
  oszlop kivezetése.
- **match_stats:** meccs-szintű statisztika per-source sorokkal, fix
  nullable oszlopokkal (football-data mezőtérkép).
- **Referee/venue:** `core.match.referee_entity_id` + `venue_entity_id`
  nullable entitás-FK.

Egy migráció (0008), teljes pipeline-átállítás, a display API és a Swift
változatlan.

### 1.1 Jóváhagyott döntések (2026-11-14)

1. **Odds-modell: normalizált `fact.odds_quote`** (a fix 7 mezős flat JSON
   helyett); a jelenlegi 7 érték 7 kulcsra képezhető.
2. **A fejléc megtartása:** a trust/capture továbbra is a
   `(match, source)` fejlécen él; a S2b-2 kétforrásos, mezőszintű szabály
   és a top-level API-állapot változatlan.
3. **match_stats: per-source sorok, fix nullable oszlopok** — minden forrás
   értékei egymás mellett élnek; a display-forrásválasztás S3.
4. **Referee/venue: entitás-FK** (`core.entity` kind `referee`/`venue`)
   külön `referees` tábla helyett — a meglévő crosswalk/review-queue
   architektúrához illeszkedik, nem fragmentálja az identitást.

### 1.2 Nem célok (YAGNI)

- Multi-market display és bookmaker-preferencia (S3 döntés).
- footballdata-adapter és bármely élő forrás (S3).
- A 18 tételes lista többi eleme: match_team_stats, lineups, incidents,
  shots, player-stats, season-stats, availability, asset-nyilvántartás,
  Elo — külön S2d-ciklusok.
- Swift/UI változás; ET/pens és kickoff-konvenciók (E csoport).

## 2. Séma — 0008 migráció

### 2.1 `fact.odds_quote` (új)

| Oszlop | Típus | Megjegyzés |
|---|---|---|
| `match_id` | uuid NOT NULL | FK a fejlécre |
| `source_id` | uuid NOT NULL | FK a fejlécre |
| `bookmaker` | text NOT NULL | demo: `'default'`; S3: forráskód (B365, Max, Avg…) |
| `market` | text NOT NULL | `1x2`, `btts`, `ou`, … |
| `outcome` | text NOT NULL | `home`/`draw`/`away`, `yes`/`no`, `over`/`under`, … |
| `line` | numeric(6,2) NOT NULL DEFAULT 0 | 0 = nincs vonal (`ou` vonala pl. 2.5) |
| `phase` | text NOT NULL | CHECK: `opening`, `closing` |
| `price` | numeric(8,3) NOT NULL | CHECK: `> 1` |

- PK: `(match_id, source_id, bookmaker, market, outcome, line, phase)`.
- FK: `(match_id, source_id) → fact.odds_snapshot (match_id, source_id)`
  ON DELETE CASCADE.
- CHECK: `line = 0 OR line > 0`.
- Index: `(match_id, market, outcome)`.

### 2.2 Backfill és a JSON-oszlop kivezetése

A meglévő `odds_snapshot.odds` JSON (7 mező) soronként átkerül:

- `home/draw/away` → `market='1x2'`, `outcome=…`, `line=0`
- `btts_yes/btts_no` → `market='btts'`, `outcome='yes'|'no'`, `line=0`
- `over25/under25` → `market='ou'`, `outcome='over'|'under'`, `line=2.5`
- `bookmaker='default'`, `phase='closing'`

Utána `ALTER TABLE fact.odds_snapshot DROP COLUMN odds`. A `market_count`
fejléc-metaadat megmarad. A backfill a migráció része (tranzakciós); a dev
DB meglévő 20 odds-sorát a migráció után quote-számlálással ellenőrizzük.

### 2.3 `fact.match_stats` (új)

- PK: `(match_id, source_id)`; `match_id` FK `core.match` ON DELETE
  CASCADE; `source_id` FK `ops.source`; `captured_at timestamptz NOT NULL`.
- Fix nullable oszlopok (CHECK ≥ 0): `home_shots`, `away_shots`,
  `home_shots_on_target`, `away_shots_on_target`, `home_corners`,
  `away_corners`, `home_yellow`, `away_yellow`, `home_red`, `away_red`,
  `home_fouls`, `away_fouls`, `home_xg`, `away_xg` (`numeric`).
- Író ebben a szeletben nincs — az S3 footballdata-adapter tölti; a
  tábla a séma-alapot adja.

### 2.4 `core.match` referee/venue

- `referee_entity_id uuid NULL REFERENCES core.entity(id)`
- `venue_entity_id uuid NULL REFERENCES core.entity(id)`
- A kind-ellenőrzést a linker garantálja (crosswalk); külön CHECK/trigger
  nincs. Adat csak az S3-adapterből kerül bele.

### 2.5 DB-kapu

A 0008 **+2 tábla** → `scripts/db-check.sh` elvárt táblaszám: **35**
(dev és test). A `test_support` TRUNCATE-lista bővül a két új táblával.

## 3. Pipeline-átállítás

- **Írás** (`ingest/canonicalize.rs`): a fixture flat odds (7 érték) →
  `odds_quote` sorok; a fejléc-upsert marad (`captured_at`, `market_count`,
  `trust_state='provisional'`). A forrás quote-blokkja egy tranzakcióban
  cserélődik (delete + insert az adott `(match, source)`-ra), így eltűnő
  kulcs nem marad vissza; a `FixtureOdds` input-formátum változatlan.
- **Trust** (`storage/repositories/trust.rs`): a `load_match_odds` a 7
  kanonikus kulcsot a quote-okból olvassa (`bookmaker='default'`,
  `phase='closing'`); `evaluate_odds_trust` és `update_odds_trust_state`
  változatlan.
- **Display** (`storage/repositories/day.rs`): a LATERAL továbbra is
  forrást választ (verified előre); az `o.odds` helyett a forrás
  `default` quote-aiból állítja elő a 7 mezős JSON-t. **Az API-válasz és
  a Swift-változatlan.**
- A `OddsValues` 7 mezős szerződés marad (trust + display); a quote-tábla
  a tárolási modell. A `default` bookmaker-szűrés e szelet szerződése; az
  S3-adapter bevezetésekor a trust/display szabály bookmaker-tudatossá
  bővül (külön szelet).

## 4. Tesztelés és ellenőrzés

- **Séma/constraint:** `price > 1`, `line` szabály, `phase` enum,
  PK-duplikátum, FK-cascade.
- **Canonicalize:** demo ingest → fejléc + 7 quote; reingest → blokk-csere
  + `trust_state` visszaáll provisional-ra.
- **Trust:** `load_match_odds` quote-okból; a kétforrásos
  verified/provisional szemantika tesztjei változatlanul zöldek.
- **Display:** a day-lekérdezés JSON-ja a quote-okból ugyanazt a 7 mezőt
  adja.
- **match_stats/referee/venue:** csak constraint-tesztek (író nincs).
- **Backfill:** dev DB-n (meglévő sorok) quote-számlálás; teszt DB-n a
  reset utáni séma.
- **Kapuk:** `./scripts/check.sh` (157+ teszt), `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --all --check`, db-check
  (35 tábla), demo 20/20 verified_publish + verified odds változatlan.
- **0 élő kérés**, 0 nyers payload; demo fixture-ök változatlanok.

## 5. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| Dev DB adat-átállítás (20 odds-sor) | tranzakciós migráció + backfill; quote-számlálás ellenőrzés; dokumentált rollback |
| Trust/display viselkedés megcsúszása | a 7 mezős szerződés változatlan; meglévő trust/display tesztek adaptálva, ugyanazokkal az elvárásokkal |
| Két igazságforrás átmenetileg | a JSON-oszlop ugyanabban a migrációban dropolva |
| Jövőbeli multi-bookmaker trust | tudatosan S3-hatókör; a séma készen tartja |

## 6. Visszaállítási út

- Teszt DB: `./scripts/db-reset.sh`.
- Dev DB: a 0008 inverze — `odds jsonb` oszlop visszavétele + feltöltés a
  `default` quote-okból (dokumentált SQL a migráció fejlécében), a két új
  tábla és a két entitás-FK dropja. A dev-en futó inverz **dry-run**
  (ROLLBACK-os másolat) engedéllyel.
- Kód: a branch revertje; a main érintetlen.

## 7. Következő lépés

1. Felhasználói spec-review (ez a dokumentum).
2. writing-plans → `docs/superpowers/plans/2026-11-14-s2d1-schema-gaps.md`.
3. Implementáció inline execution-nel, taskonkénti STOP-pal.
4. Utána a következő S2d-ciklus (A csoport: match_team_stats, lineups,
   incidents, shots, player-stats) vagy S3.
