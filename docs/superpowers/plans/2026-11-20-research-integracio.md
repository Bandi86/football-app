# Football Research → football-app-v2 integrációs terv

**Dátum:** 2026-11-20
**Állapot:** terv, jóváhagyás alatt
**Érintett rendszerek:** `~/Desktop/football-research` (gyűjtő, fájl-alapú JSON API) és
`football-app-v2` (kanonikus adatbázis + kliens)

## 1. Kiindulás: mit tud a research rendszer (audit, 2026-11-20)

A `football-research` **fájl-alapú JSON API** (nincs futó HTTP szerver; a manifest minden
endpointja `GET (local file read)`). Kanonikus kimenet: `api/v1/` — 1287 fájl, 27,3 MiB.

| Erőforrás | Tartalom | Méret |
|---|---|---|
| `daily/YYYY/MM/<date>.json` | napi envelope: 285 meccs (11-20), facets, metrika, revisions | 4 nap, 4,1 MB |
| `matches/YYYY/MM/DD/*.json` | 1239 meccs-dokumentum, 37 szekció (events, lineups, stats, shotmap, odds, h2h, weather, prediction, preview_hu, research…) | 19,2 MB |
| `coverage/`, `quality/`, `gaps/` | lefedettség, mező-kitöltöttség, hiánylista | 2-4 nap |
| `index/`, `schemas/`, `manifest.json` | lookup + JSON Schema + API-leírás | – |

Mai (2026-11-20) számok: **285 meccs**, ebből **177 oddsos (62%)**, **190 enriched**
(ESPN/FotMob részletek), 95 `fixture_only` (LiveScore napi teljes lefedettség),
12 research-notes; a v2 ugyanezen a napon **122 meccs / 29 odds** a DB-ben (46 közzétéve).

Források: FotMob API, ESPN public API, LiveScore napi fixtures, football-data.co.uk,
tipplap (SQLite), BetExplorer (SearXNG + obscura), Open-Meteo, gemma-4 helyi modell.

## 2. A research pipeline állapota (hibák, amiket a v2-nek tudnia kell)

1. **launchd TCC-tiltás:** a `com.omp.football` agent `/bin/bash: ... Operation not permitted`
   hibával állt le, mert a Desktop-mappát launchd-ból nem tudja olvasni. A napi futás ma
   Hermes cronból/kézi indításból megy; a research-heartbeat „launchd" host-stringje hardcode.
2. **Backfill leállt:** `backfill-state.json` → `done: ["2026-11-19"]`, azóta nincs előrelépés;
   2026-11-17 kimaradt („nincs még napi mappa"). A research lefedettség csak 4 nap.
3. **Acceptance 11-20 piros:** 2 hiányos research blokk, research-verify exit=1,
   „a napi lefedettség nem változott (1340 → 1340)".
4. **Hiányzó/elavult hivatkozások:** `api-export.sh`, `gemma-research.sh` nem létezik;
   a root `gaps/*.md` és a dátumozott változat eltér.
5. **Duplikációk:** 35 fuzzy cross-source fixture-pár nem összevonva (bayern-munchen/1-fc-koln,
   stade-nyonnais/wil, női BL `(W)` névhiba).
6. **Validáció gyenge:** kézi checker csak type/const/enum/required/$ref; a tömböket
   `obj[:200]`-nál levágja, `additionalProperties` nincs; a `match_detail` séma 37 kulcsból
   ~20-at nem ír le (`statistics` vs `team_statistics`/`player_statistics`).

Ezek a research oldal javításai (R5 előkészítés), nem a v2 hibái — de a v2 integrációnak
számolnia kell velük (pl. napzár, revision-alapú idempotencia, fuzzy duplikátum-szűrés).

## 3. Célarchitektúra

```
football-research (gyűjtő)                 football-app-v2 (kanonikus + app)
  fetch (FotMob/ESPN/LiveScore)              ingest: research adapter (file)
  enrich (részletek, statisztika)     ──►    canonicalize: identity + core.match
  odds (tipplap/ESPN/footballdata)           fact.*: odds, stats, incidents, lineups…
  research (gemma + források)                publish-gate → API → SwiftUI kliens
  api/v1 export (JSON API)
```

- A research marad az egyetlen napi gyűjtő; a v2 nem duplikálja a forrásokat.
- A v2 kanonikus identitást ad: stabil `core.match`/`core.team` + `identity.source_entity_ref`
  a research ID-ire (`espn:`, `fm:`, `livescore:`).
- A v2 a research **napi JSON API-t** fogyasztja, konfigurálható gyökérrel.

## 4. Döntési pontok

**D1 — Hozzáférés.** A research a Desktopon lakik, a v2 daemon launchd-ból fut → TCC.
Lehetőségek: (a) az `api/v1` (vagy az egész mappa) áthelyezése
`~/Library/Application Support/FootballResearch/` alá, a Desktopon symlinkkel; (b) Full Disk
Access a `footballd` binárisnak; (c) a research szolgáljon ki localhost HTTP-t (akkor is kell
TCC a szerver folyamatnak). Javaslat: **(a)**, mert nem igényel széles jogosultságot.

**D2 — Ismeretlen csapatok a research forrásból.** Jelenleg a v2 fail-closed: ismeretlen
csapat → review-sor, a meccs kimarad. A research napi 285 meccsének többsége ilyen lenne.
Lehetőségek: (a) create-on-miss: a research (mint megbízható forrás) hozhat új kanonikus
csapatot a név+ország+forrás-ID alapján, review csak név-ütközésnél; (b) marad a fail-closed
review. Javaslat: **(a)**, mert enélkül nincs teljes napi lefedettség.

**D3 — A v2 saját napi adaptereinek sorsa.** A research lefedi a soccerway/openfoot/footballdata/
tipplap/fotmob forrásokat. Javaslat: a **napi** futásból a soccerway/openfoot/tipplap kikapcsol,
a research lesz az elsődleges; a `footballdata` (historikus odds) és a `reep` (crosswalk) marad
a backfillhez; a v2 adapterek kódja megmarad (kikapcsolt policy), mert a research kiesésekor
visszakapcsolhatók.

## 5. Szeletek

- **R1 — Szerződés + hozzáférés (döntés után).** A research `api/v1` útvonal konfigurálhatóvá
  tétele a v2-ben (`FOOTBALL_RESEARCH_API_DIR`), a napi fájlok (daily + matches + index)
  formátumának rögzítése a v2 doksijában; TCC-megoldás végrehajtása.
- **R2 — research adapter (első kör).** Új `rust/crates/ingest/src/research.rs`:
  `Transport::File` (új transport) vagy fixture-szerű lokális olvasó; a daily envelope-ből
  `team_identity` + `match_core` (+ `match_referee`, `standing`) observationök; a match detail
  fájlokból `match_odds` (markets → quote-ok); source slug `research`; policy + worker wiring;
  unit- és file-fixture teszt; élő futás a 2026-11-20-i napra.
  **Elfogadás:** a v2 DB-ben ≥280 meccs a research 285-ből, ≥170 meccsen odds, `integrity` OK,
  a meglévő forrásokkal nincs új duplikátum (`dedupe variants` 0), a meglévő 323 teszt zöld.
- **R3 — részletek.** `match_stats`, `match_referee`, majd incidents/lineups/player-stats
  (a fact-táblák már léteznek; a canonicalize bővítése szükséges) — ezek adják az app
  meccs-részletező nézetét.
- **R4 — napi ütemezés + monitoring.** A research napi futása után (heartbeat/revision) a v2
  research job fusson; coverage/quality riport a v2-ben; a research acceptance hibáinak
  javítása (launchd TCC, hiányzó scriptek, gaps-latest).
- **R5 — Backfill.** Research-oldali backfill visszamenőleg (2026-07…11: LiveScore/ESPN múltbeli
  napok) + v2-oldali visszatöltés ugyanazon az adapteren; a v1-import + footballdata marad a
  mély历史hez; a fuzzy duplikátumok szűrése a backfill során (a v2 S37c dedupe már tudja).
- **R6 — App design.** A research gazdag JSON-jaira épülő nézetek: meccs-részletező (events,
  lineups, stats, shotmap, h2h), előnézet (preview_hu, prediction), odds-piacok, tabella,
  játékos-statisztika; a v2 API és a SwiftUI kliens újratervezése.

## 6. Kockázatok

- TCC/macOS jogosultság (R1) — a legnagyobb blokkoló.
- Identity: create-on-miss nélkül a lefedettség nem javul; create-on-miss mellett viszont
  a duplikátum-kockázat nő → a v2 meglévő S37b/S37c védelmei (variáns-linkelés, dedupe) és
  a review-queue kötelező.
- A research pipeline jelenleg részben törött (launchd, backfill, acceptance) → a v2 nem
  építhet a „minden nap automatikusan jön" feltételezésre, amíg R4 nem zöld.
- A research JSON sémája 20+ szekcióval gazdagabb, mint a v2 jelenlegi kanonikus modellje;
  az R2 csak a magot viszi át, a részletek R3-ban jönnek.
