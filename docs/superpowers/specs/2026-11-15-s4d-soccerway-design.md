# S4d — Soccerway-adapter (Obscura-út)

**Dátum:** 2026-11-15
**Állapot:** research-alapú terv; a döntések mérési bizonyítékkal rögzítve
**Kapcsolódó dokumentumok:** [S1 research](../../superpowers/specs/2026-11-11-real-data-sources-research-design.md),
[S2a ingest keretrendszer](../../superpowers/specs/2026-11-12-s2a-ingest-framework-design.md),
[S2c Obscura](../../superpowers/specs/2026-11-14-s2c-obscura-design.md),
[S2d-4 ET/kickoff](../../superpowers/specs/2026-11-14-s2d2-match-micro-design.md),
[S4b FotMob](../../superpowers/specs/2026-11-14-s3b-footballdata-design.md),
[2026-11-15 research](../../checkpoints/2026-11-14/checkpoint.md)
**Branch:** `feat/s4d-soccerway` (main 5bd5ca5)

## 1. Hatókör és research-bizonyíték (2026-11-15)

A Soccerway a JS-renderelt nap-feed forrás az Obscura-úton. A mai célzott
research (3 render + 1 robots-kérés) alapján:

1. **Dátum-paraméter:** a `?date=YYYY-MM-DD` paramétert a Soccerway
   **figyelmen kívül hagyja** (a „Ma" napot rendereli, 332 sor, 2,2 MB).
2. **Nap-útvonal:** a nap-választó kliens-oldali; az „előző nap" nyíl a
   lokalizált `/tegnap/` útvonalra navigál (hu locale), a nap-címke
   **angol aria-label** (`Saturday, November 14, 2026`) → dátum-guard
   lehetséges. A tegnapi oldal 3,1 MB, 532 sor, 2128 pont-marker.
3. **Readiness:** az 1,5 s settle csak héj-HTML-t ad (152 KB, 0 sor);
   12 s settle után teljes DOM (2,2–3,1 MB). → marker-alapú várakozás kell
   (`div.event__match[data-event-row]`), nem fix settle.
4. **Decoy-200:** ismételt gyors kérések után a Soccerway 200-zal ad
   „The requested page can't be displayed. Please try again later."
   hiboldalt (`404_page` dataLayer) → parser-szintű decoy-őr kötelező.
5. **Meccssor:** `div.event__match[data-event-row=true]`, `id="g_1_<mid>"`,
   `?mid=<mid>` az hrefben; csapatnevek
   `data-testid="wcl-simple-text-01"` (home/away participant); pontok
   `span[data-testid=wcl-tableScore][data-side=1|2]`; büntetők
   `sup.wcl-penaltyScore` (`(4)`); gólnál többes idők nincsenek a listán.
6. **Státuszok (hu lokalizált stage-szöveg):** `Vége` (FT), `Hossz. után`
   (AET), `Büntetők után` (pens, sup-pel), `Elhalasztva`, `Törölve`,
   `Elmarad`; ütemezett sor: `event__match--scheduled` + `event__time`
   (HH:MM); élő sor: `event__match--live`.
7. **Verseny-csoport:** `div[data-testid=wcl-headerLeague]` blokk
   (ország-zászló `title`, liga-név + stabil URL-slug, pl.
   `/anglia/premier-league/`); a sorok a legutóbbi fejléc-csoporthoz
   tartoznak.
8. **Robots:** a `/matches/`, `/tegnap/` engedett (a robots.txt csak más
   sportágakat és pár utat tilt); a worker robots-kapuja változatlanul fut.

### 1.1 Döntések

1. **Egy job = egy liga** (a `NormalizedDay` egy verseny-meta-t hordoz; a
   `from_observations` architektúra nem változik). A domain = liga-slug;
   a parser a nap-oldalról csak az adott liga csoportját emitiálja.
2. **Támogatott napok:** ma → `/`, tegnap → `/tegnap/` (a helyi nap
   szerint). Más dátum → üres fetch-terv (látható job-hiba), amíg a
   naptár-popup útvonala nincs mérve. A `/holnap/` nem kell a napi körhöz.
3. **Időzóna:** az Obscura gyerekfolyamat `TZ=UTC` env-vel indul (a
   renderelés determinisztikus); a `competition_timezone` = UTC. Élő
   ellenőrzés a záró taskban (nemzetközi meccs 19:45 UTC vs 20:45 CET);
   ha a pin nem érvényesül → `date_only` kickoff-precízióra esünk vissza.
4. **Eredmény-szemantika (90 perces FT-konvenció):**
   - `Vége`: `home_ft/away_ft` = fő pontszám, `decided_by='ft'`.
   - `Hossz. után`: `home_et/away_et` = fő pontszám, FT NULL,
     `decided_by='et'`.
   - `Büntetők után`: ET = fő pontszám, `home_pens/away_pens` = sup,
     `decided_by='pens'`.
   - `Elhalasztva`/`Törölve`/`Elmarad`: státusz, eredmény nélkül.
   - Ismeretlen stage + pontszám → `finished`, `decided_by` NULL;
     ismeretlen stage pontszám nélkül → a sor kimarad + warn.
   - Ütemezett/élő sor: kickoff `date_only` (12:00 UTC) — a pontos idő
     csak ütemezett sornál van (HH:MM).
5. **Kickoff-precízió és decided_by bekötése** (a 0011 séma most kap
   írót): `match_core` payload opcionális `kickoff_precision` és
   `result.decided_by`; a kanonikalizáló nem rontja le a pontos kickoffot
   date_only-ra, és a date_only natural-key dátum-egyezéssel keres (nincs
   duplikált kanonikus meccs).
6. **Identity:** forrás-csapat-azonosító a link-szegmens stabil kódja
   (`/merkozes/<slug>-<Code>/...`), a név a lokalizált megjelenített név
   (fail-closed review az első körben, ahogy footballdata-nál).
7. **HTML-parser:** `scraper = "0.29"` (html5ever-alapú, CSS-szelektorok).
   Indoklás: a minified, beágyazott Flashscore-markupot kézzel szkennelni
   törékeny; a crate az egyetlen új függőség.
8. **Default liga-készlet:** top-5 + NB I + NB II (7 domén). Kupák
   később (a csapat-ország feloldásuk külön kérdés). Az induló policy
   `enabled=false` (élő bekapcsolás külön engedély).

### 1.2 Nem célok (YAGNI)

- Történelmi backfill (naptár-popup útvonal), kupák, odds-oszlopok,
  live-frissítés finomítása, több-liga egy jobban.

## 2. Architektúra

- **`SoccerwayAdapter`** (`ingest/src/soccerway.rs`): `plan` (nap-útvonal),
  `parse` (date-guard → decoy-őr → csoportok → sorok → observationök).
- **Readiness:** `FetchRequest.ready_marker: Option<String>`; az
  `ObscuraFetcher` a settle után a marker megjelenéséig polloz
  (max ~20 s, a fetch-limiten belül); a `RustHttpFetcher` figyelmen kívül
  hagyja. A Soccerway markere `data-event-row`.
- **Séma-bővítés:** `kickoff_precision` a `NormalizedMatch`-ben,
  `decided_by` a `NormalizedResult`-ban; a `match_core` payload
  visszafelé kompatibilis (hiányzó mező = `exact` / NULL).
- **Kanonikalizáló:** match-upsert `kickoff_precision`-nel + lefokozás
  elleni CASE; `persist_result` `decided_by`-vel; natural-key date_only
  esetén dátum-egyezés.
- **Wiring:** regisztráció a footballd + footballctl adapters-mapben;
  ops doc `docs/operations/soccerway.md`.

## 3. Tesztelés és DoD

- Unit/integrációs tesztek szintetikus fixture-rel (valós formátumú
  markup, nyers payload nélkül):
  1. `route_for` (ma/tegnap/más);
  2. parse: ütemezett + lezárt + AET + pens + halasztott + idegen liga
     kiszűrése; pontszám/decided_by/ET-pens szétválasztás; csapat-id;
  3. dátum-guard eltérés → `DateGuardMismatch`;
  4. decoy-oldal → `DecoyDetected`;
  5. ismeretlen stage viselkedés.
- Kanonikalizáló: date_only nem írja felül a pontos kickoffot; date_only
  natural-key nem duplikál; decided_by perzisztál.
- `scraper` bekötés után `check.sh` zöld; clippy/fmt tiszta.
- Opt-in élő teszt (`OBSCURA_LIVE_TEST=1`): napi oldal renderelése,
  date-guard, decoy-mentesség, TZ-mérés (kiírt kickoff-óra).

## 4. Kockázatok

- **Lokalizált stage-szövegek** változhatnak → a pontszám-jelenlét a
  tartalmi őr, az ismeretlen szöveg nem blokkolja a napot.
- **Soccerway oldali cooldown/blokk** (mért viselkedés) → a policy
  min_delay + budget; a breaker/retry a workertől jön.
- **TZ-pin hatástalansága** → visszaesés `date_only`-ra, dokumentálva.

## 5. Visszaállítási út

- Branch-revert; séma-változás nincs (0011 oszlopai már megvannak);
  policy-sor törlés vagy `enabled=false`; az observation/stage adatok
  forrás-kötöttek.
