# S4d — Soccerway-adapter megvalósítási terv

**Dátum:** 2026-11-15
**Spec:** [2026-11-15-s4d-soccerway-design.md](../specs/2026-11-15-s4d-soccerway-design.md)
**Branch:** `feat/s4d-soccerway`

## Task 1 — Obscura readiness-marker + TZ-pin

- `FetchRequest.ready_marker: Option<String>` (minden adapter `None`-t ad,
  a Soccerway `Some("data-event-row")`-t); a fetch-tesztek konstruktorai.
- `ObscuraFetcher::render`: settle után poll a markerre (500 ms lépés,
  max 20 s); ha nincs meg → a meglévő HTML megy tovább (a parser dönt).
- `ObscuraServer` spawn: `TZ=UTC` env a gyerekfolyamatra.
- Teszt: marker-poll egység (wiremockos live-serve teszten át a meglévő
  `obscura_fetch` kiterjesztésével); default build érintetlen.

Kapu: `cargo test -p football-ingest --features obscura`, `check.sh`.

## Task 2 — kickoff_precision + decided_by bekötése

- `MatchCorePayload.kickoff_precision: Option<String>` (default `exact`),
  `ResultPayload.decided_by: Option<String>`; `NormalizedMatch`/
  `NormalizedResult` mezők; `From<&FixtureSource>` és a
  `live_materialize` teszt konstruktorai.
- Kanonikalizáló: match-upsert `kickoff_precision` oszloppal; ON CONFLICT
  CASE: pontos → date_only lefokozás tilos (kickoff_at is marad);
  `persist_result` `decided_by` (INSERT + UPDATE); date_only
  natural-key dátum-egyezéssel keres.
- Teszt: `live_materialize` kiterjesztve (date_only nem írja felül a
  pontos kickoffot; decided_by perzisztál; nincs duplikált meccs).

Kapu: `cargo test -p football-ingest`, `check.sh`.

## Task 3 — SoccerwayAdapter

- Új függőség: `scraper = "0.29"`.
- `ingest/src/soccerway.rs`:
  - liga-mapping (7 slug: ország-kód/név, verseny-név, tier, priority);
  - `route_for(today_local, requested)` → `/` vagy `/tegnap/`, más → nincs
    útvonal (üres terv);
  - parse: date-guard (aria-label, `%A, %B %d, %Y`), decoy-őr
    (`404_page` / „can't be displayed"), csoport-bejárás, sor-parse
    (mid, csapatok/kódok, pontszámok, sup-pens, stage-státusz,
    `event__time`), observation-emit (`team_identity`, `match_core`);
  - season-label a dátumból (`YYYY/YY`), round NULL.
- Szintetikus fixture: két csoport (cél + idegen), ütemezett, FT, AET,
  pens, halasztott, törölt sor; decoy- és dátum-guard tesztek.

Kapu: adapter-tesztek, `check.sh`.

## Task 4 — Wiring, ops doc, zárás

- Regisztráció footballd + footballctl adapters-mapben (`soccerway`).
- `docs/operations/soccerway.md`: policy-seed (transport obscura,
  enabled=false), domainek, nap-útvonalak, TZ-pin, decoy-őr, review
  bootstrap, cooldown, rollback.
- Opt-in élő teszt `soccerway_live.rs`: renderelés, date-guard,
  decoy-mentesség, marker-szám, TZ-megfigyelés kiírása.
- README állapot-sor + napi checkpoint; git rendezés (commit/task,
  FF merge a zárásnál).
