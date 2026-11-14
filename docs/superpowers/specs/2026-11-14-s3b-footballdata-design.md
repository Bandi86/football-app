# S3b — Footballdata-adapter (első éles forrás)

**Dátum:** 2026-11-14
**Állapot:** brainstorming-jóváhagyott terv; felhasználói spec-review függőben
**Kapcsolódó dokumentumok:** [source-catalog footballdata](../../research/source-catalog.md),
[S1 research spec](../specs/2026-11-11-real-data-sources-research-design.md),
[S2a ingest keretrendszer](../specs/2026-11-12-s2a-ingest-framework-design.md),
[S2d-1 séma](../specs/2026-11-14-s2d1-schema-gaps-design.md),
[S3a kapuk](../specs/2026-11-14-s3a-entry-gates-design.md),
[2026-11-14 checkpoint](../../checkpoints/2026-11-14/checkpoint.md)
**Branch:** `feat/s3b-footballdata` (a main a6ea92d-ről)

## 1. Hatókör

A **footballdata** (football-data.co.uk) statikus CSV-forrás első éles
adaptere: 5 liga (E0, SP1, I1, D1, F1), 2026/27 szezon, teljes
mezőkészlet — meccs-mag, match_stats, referee, 1x2 + O/U 2.5 odds
(B365/PP/BFE/Max/Avg, opening+closing). Előfeltétel: az S3a 13 kapuja
(11 zárt, 2 dokumentáltan halasztva) — teljesült.

### 1.1 Jóváhagyott döntések (2026-11-14)

1. **Observáció-vezérelt kanonikus út:** a tick live ága a parse-observationökből
   építi a `NormalizedDay`-t; a `FixtureSource`-újraolvasás megszűnik, a demo és
   a CLI is az egy úton megy.
2. **Scope:** 5 demo liga, teljes mezőkészlet (meccs-mag, stats, referee,
   odds); BTTS a forrásban nincs → NULL.
3. **Kanonikus odds-szabály:** kulcsonként **B365 closing → Avg → Max**; a
   `default` (demó) kulcs előnyt élvez, ha jelen van.
4. **Induló mód:** `canary`, 10/nap, 2000 ms + 1000 ms jitter; az élő kör a
   záró taskban, külön engedéllyel; canary→live a felhasználó döntése.

### 1.2 Nem célok (YAGNI)

- ~30 liga; AH és egyéb piacok; ET/pens jelölés (a forrás nem adja).
- Multi-market display, bookmaker-választó UI (S8/Swift-ig nincs fogyasztó).
- Automatikus csapat-crosswalk seedelés (a review-queue a fail-closed út);
  az első élő kör review-sorait a felhasználó hagyja jóvá.

## 2. Architektúra — observáció-vezérelt kanonikus út

- **`NormalizedDay`** (domain-típus): verseny/szezon-meta; csapatok
  (forrás-azonosító, név, ország, nem, korosztály, ág); meccsek (kickoff UTC,
  90 perces FT/HT + státusz, stats, referee, venue); odds
  (bookmaker/phase/market/outcome/line/price); tabella-sorok; forrás-kód.
- **Builder:** `NormalizedDay::from_observations(&[Observation])` az ingest
  rétegben — a dokumentált, verziózott observation-aspketekből
  (`team_identity`, `match_core`, `match_odds`, `standing`, új:
  `match_stats`, `match_referee`). Az observation a staging + audit
  igazsága; a kanonikus út ebből indul.
- **Tick live ág:** `adapter.parse` → observationök persistálva →
  `NormalizedDay::from_observations` → `verify_and_publish(&day)` (a
  `FixtureSource`-újraolvasás törlése).
- **Canonicalizer:** `materialize(&NormalizedDay)`; a `FixtureSource`-t a
  fixture-adapter `NormalizedDay`-dé alakítja — egy út a demóra is; a
  footballctl `IngestFixture` ugyanezt hívja.
- Az adapter trait `parse`-ja marad (observationök); a normalizált
  struktúrát mindkét adapter a saját nyers payloadjából állítja elő.

## 3. Footballdata CSV-adapter

### 3.1 Végpont és parse

- URL: `https://www.football-data.co.uk/mmz4281/2627/{league}.csv`
  (E0, SP1, I1, D1, F1); napi 1 kérés/liga (`discovery`, interval 1440).
- Parse: `csv` crate 0.3 (fejléc-alapú olvasás, BOM/CRLF tolerancia);
  **ablak:** a fájlból csak a job dátuma ±7 nap sávjába eső meccsek.
- Séma-drift: hiányzó kötelező oszlop → parse-hiba (karantén, a nyers
  megmarad); ismeretlen extra oszlop → figyelmen kívül.

### 3.2 Mezőtérkép

| Forrás | Cél |
|---|---|
| `HomeTeam`/`AwayTeam` | csapat-név + crosswalk (linker) |
| `Date` + `Time` | kickoff UTC (**Europe/London → UTC**); idő nélkül 12:00 UTC (`date_only`, dokumentált) |
| `FTHG`/`FTAG`/`FTR`, `HTHG`/`HTAG`/`HTR` | 90 perces eredmény (konvenció sérthetetlen) |
| `HS/AS/HST/AST/HC/AC/HY/AY/HR/AR/HF/AF` | `fact.match_stats` |
| `HxG`/`AxG` | `match_stats.home_xg/away_xg` (ha jelen van) |
| `Referee` (E0) | referee-entitás a linkeren át (`referee` kind) |
| `B365H/D/A`, `B365CH/CD/CA` | `1x2` opening/closing |
| `>2.5` / `<2.5` oszlopok (B365/PP/BFE/Max/Avg) | `ou` line 2.5 |
| `Avg*`, `Max*`, `PP*`, `BFE*` + C-változatok | könyvelői quote-ok (opening/closing) |

- BTTS a forrásban nincs → a kanonikus `btts_*` NULL (hiányzó adat NULL elv).
- Az odds tényleges tárolása: `default` helyett a bookmaker-kód
  (`B365`, `PP`, `BFE`, `Max`, `Avg`), a fázis a C-oszlopokból
  (`opening`/`closing`).

### 3.3 Identitás és policy

- Csapat: név + ország-kontextus a fail-closed linkeren (nem egzakt →
  review-sor, withheld); referee ugyanígy.
- `ops.source` + `ops.source_policy`: `footballdata`, `rust_http`,
  `canary`, `daily_budget=10`, `min_delay_ms=2000`, `jitter_ms=1000`,
  `breaker_threshold=3`, `breaker_cooldown_secs=60`, `max_bytes=10 MB`,
  adapter/parser `1.0`; `ops.source_trust`: odds/result `medium`.

## 4. Trust és display

- `load_match_odds` és a day-LATERAL: kulcsonként **B365 closing → Avg →
  Max**; a `default` kulcs előnyt élvez, ha jelen van (demó változatlan).
- A **trust-szemantika változatlan** (két provider-csoport): egyetlen élő
  forrásnál `provisional` (őszinte); a demó két forrása `verified` marad.

## 5. Tesztelés és DoD

- **CSV-fixture:** valós formátumú minta (BOM + CRLF, odds-oszlopok,
  hiányzó mezők, referee) a `tests/fixtures/` alatt; **0 élő kérés**.
- **Parser-unit:** dátum/TZ/DST-konverzió, `date_only`, odds-lánc,
  hiányzó oszlop → hiba, ablak-szűrés.
- **Normalizálás:** `from_observations` — a fixture és a CSV ugyanazt a
  `NormalizedDay`-t adja; a demo observationjei a régi `FixtureSource`-szal
  egyező kanonikus eredményt adnak (regresszió).
- **Pipeline-e2e:** wiremock CSV, canary mód; seedelt csapat-entitások +
  aliasok → `verified_publish`; seed nélkül → review-sor + withheld.
- **Odds-lánc:** storage-teszt a B365→Avg→Max kulcsválasztásra.
- **DoD:** `check.sh` zöld (179+), demo 20/20 változatlan, nincs migráció
  (a séma S2d-1/S2d-2-ben elkészült), 0 élő kérés a default futásban.

## 6. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| CSV-séma drift (oszlopátnevezés) | fejléc-alapú mapping; hiányzó kötelező oszlop → karantén; séma-drift őr |
| Időzóna/DST hiba | Europe/London → UTC unit-tesztek nyári/téli időre |
| Identitás-review hullám az első élő körnél | fail-closed review-sorok; CLI jóváhagyás; dokumentált üzemeltetési lépés |
| Observáció-mennyiség | ±7 napos ablak; idempotens kanonikus upsert |

## 7. Élő engedélyezés (záró task, külön engedéllyel)

- Dev DB: `footballdata` source + policy + trust sorok.
- Egy ellenőrzött élő kör: 5 CSV + robots (canary, 10/nap).
- A nap-lista és a ledger ellenőrzése; az eredmények dokumentálása a
  checkpointban; a canary→live váltás a felhasználó döntése.

## 8. Visszaállítási út

- Kód: branch-revert; **nincs migráció**.
- DB: a `footballdata` source/policy/trust sorok törlése (a kanonikus és
  stage adatok a forráshoz kötöttek); teszt DB reset.

## 9. Következő lépés

1. Felhasználói spec-review (ez a dokumentum).
2. writing-plans → `docs/superpowers/plans/2026-11-14-s3b-footballdata.md`.
3. Implementáció inline execution-nel, zöld task után committal.
4. Élő kör külön engedéllyel; utána a következő forrás (FotMob/Soccerway)
   vagy a maradék S2d-csoportok.
