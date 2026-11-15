# 2026-11-15 — Napi checkpoint (S4d Soccerway-adapter)

**Állapot:** az S4d szelet kész (parser + bekötés + élő render-igazolás)
**Előző nap:** 2026-11-14 (S4c: Soccerway research + asset-import)
**Kapcsolódó dokumentumok:** [S4d spec](../superpowers/specs/2026-11-15-s4d-soccerway-design.md),
[S4d terv](../superpowers/plans/2026-11-15-s4d-soccerway.md),
[S4c checkpoint](../2026-11-14/checkpoint.md),
[source-access findings (memória)],
[soccerway ops](../operations/soccerway.md), [obscura ops](../operations/obscura.md)
**Branch:** `feat/s4d-soccerway` (main 5bd5ca5-ről; záráskor FF merge)

## Kiinduló állapot

- [x] Kötelező olvasási sorrend: AGENTS.md, README, S4c checkpoint, git
      állapot (main, tiszta, szinkronban originnal).
- [x] Az S4c nyitott pontja: Soccerway-parser a rögzített szelektorokra,
      két méréssel (dátum-útvonal, AET-minta).

## Research (mérési bizonyíték, 2026-11-15)

- [x] `?date=YYYY-MM-DD` **figyelmen kívül hagyva** (ma renderel).
- [x] Nap-választó click: „előző nap" → `/<locale>/tegnap/`
      (`hu.soccerway.com/tegnap/`), nap-címke angol aria-label →
      dátum-guard; tegnapi oldal 3,1 MB, 532 sor.
- [x] Readiness: 1,5 s settle = héj (152 KB, 0 sor); 12 s = teljes DOM
      (2,2–3,1 MB) → marker-alapú várakozás.
- [x] AET/pens minta: `Hossz. után` ×2, `Büntetők után` ×26
      (fő pontszám + `sup.wcl-penaltyScore`), + `Elhalasztva`/`Törölve`/
      `Elmarad`; ütemezett sor `event__time` HH:MM.
- [x] Decoy-200: „The requested page can't be displayed" hiboldal
      (`404_page` dataLayer) ismételt gyors kérések után.
- [x] Robots: `/matches/`, `/tegnap/`, `/` engedett (a tiltások más
      sportágakra szólnak); a worker robots-kapuja változatlan.
- [x] A temporális research-módosítások visszaállítva; a repóban nincs
      nyers payload.

## Végrehajtás (taskonként, commit-per-green)

- [x] **Task 1 (4631f72):** `FetchRequest.ready_marker` (CSS-szelektor);
      az `ObscuraFetcher` markerre polloz (max 20 s, utána a meglévő DOM
      megy tovább); az Obscura gyerek `TZ=UTC`-vel indul. Teszt:
      kliens-oldalon 3 s-késleltetéssel beszúrt marker megvárva
      (wiremock content-type javítás: `set_body_raw` — a `text/plain`
      miatt a HTML escape-elve renderelődött).
- [x] **Task 2 (25b5869):** `kickoff_precision` + `decided_by` író:
      observation-payload (opcionális, default exact/NULL) →
      `NormalizedMatch`/`NormalizedResult` → kanonikalizáló match-upsert
      (date_only nem rontja le a pontos kickoffot; nincs duplikált
      kanonikus meccs: date_only natural-key dátum-egyezéssel);
      `persist_result` decided_by. Teszt: date_only + pens eset.
- [x] **Task 3 (182f457):** `SoccerwayAdapter` (`scraper = "0.29"`):
      nap-útvonal (ma/tegnap), date-guard, decoy-őr, csoport-bejárás,
      sor-parse (mid, stabil csapatkódok, pontszám + sup-pens,
      stage-státusz), observation-emit; 7 default liga (top-5 + NB I + NB
      II). Tesztek: route/szezon/kód-unit + teljes parse (ütemezett, FT,
      AET, pens, halasztott, idegen liga kiszűrése), date-guard,
      decoy, ismeretlen stage.
- [x] **Task 4:** regisztráció a footballd és a footballctl
      adapters-mapjében; `docs/operations/soccerway.md` (policy-seed
      `enabled=false`, trust `result`/medium, 15 s + 5 s delay,
      bootstrap, rollback); opt-in `soccerway_live.rs`.

## Élő render-igazolás (2026-11-15, 1 kérés)

- [x] `OBSCURA_LIVE_TEST=1` futás: napi oldal **2,2 MB, 328 sor**,
      nap-címke **2026-11-15**, nincs decoy.
- [x] Kezdési idők: **19:45** (korábbi pin nélküli mérés: 20:45 CET) →
      a `TZ=UTC` pin **érvényesül**; `competition_timezone = UTC`
      helyes.
- [x] Teljes parse-út élőben: első csoport `europa/uefa-nations-league`
      → 8 meccs, 16 csapat; a dátum-guard és a csoport-szűrő zöld.

## Záró kapuk

- [x] `./scripts/check-foundation.sh` exit 0 (teszt-DB reset után; a
      storage-tesztek egy szálon futnak — a közös teszt-DB versenyhelyzete
      miatt a script `--test-threads=1`-gyel hív).
- [x] `cargo test --workspace -- --test-threads=1`: 74 tesztcsoport zöld,
      exit 0.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` +
      obscura feature clippy, `cargo fmt --all --check`, `cargo check -p
      footballd --features obscura` — tiszta.
- [x] FF merge `main`-be + branch törlés + push (a záró lépés).

## Következő döntési pont

1. Soccerway policy **élő bekapcsolása** (canary): kétfázisú
   identity-bootstrap a lokalizált nevekhez (review → approve), utána
   a napi lista bővítése (NB I/NB II + nemzetközi kupák később).
2. Utána: asset-feltöltés, Elo/asset megjelenítés, S8 kliens.

## Visszaállítási út

- Kód: `feat/s4d-soccerway` revert / branch törlés; séma-változás nincs.
- Policy: `enabled=false` (induló állapot), vagy a policy-sor törlése.
- A kanonikus Soccerway-sorok forrás-kötöttek; a review-sorok
  visszafordíthatók.

## Élő Soccerway canary + asset-import (2026-11-15, folytatás)

### Policy és első kör

- [x] Policy-seed a dev DB-n (jóváhagyott élő kapcsolás): `transport=obscura`,
      `mode=canary`, `enabled=true`, 7 domain, napi 14 kérés, 60 s + 15 s
      jitter min_delay, breaker 3/120 s; trust `result`/medium. Megjegyzés:
      a `domains`/`events` oszlop **jsonb** (a korábbi ARRAY-es doc-példa
      javítva).
- [x] CLI-oldali Obscura-támogatás: a `footballctl` új `obscura` feature-e
      ugyanúgy lazy indítja a szervert, mint a daemon (canary tick kézből).
- [x] Első kör tanulságai (valós hibák, javítva):
      1. **Nulla-nap hiba:** üres observation-lista esetén a normalizálás
         elbukott `no_data` helyett → javítás + `empty_canary_day_is_no_data_not_failure`
         e2e teszt (commit `dfeb235`).
      2. **Fetch-timeout:** a 30 s-os statikus plafon levágta a 10–45 s-os
         Obscura-rendereket (3 timeout → breaker 120 s-ra) → 60 s
         (commit `dfeb235`).
      3. Ismételt gyors kérések után a forrás elakad; a 60 s + jitteres
         ütem és a breaker fail-closed viselkedése helyes.

### Élő eredmény (2026-11-15)

- [x] Sikeres feldolgozott domainek (canary, fail-closed): Ligue 1,
      Bundesliga, NB I → `no_data` (válogatott szünet — a szűrt ligáknak
      nincs meccse); a nap-oldal renderelése, a dátum-guard, a decoy-őr és
      a liga-szűrő élőben fut.
- [x] A maradék domainek retry-ja ütemezetten fut (min_delay/breaker
      tisztelettel); a failed jobok operátori visszaállítása dokumentált
      (`status='pending'`, breaker-sor törlése).
- [ ] A 2026-11-16-i napi kör az első teljes (meccses) bizonyíték.

### Asset-import (v1 auditált logók)

- [x] `ObjectStore::put_blob` (tartalom-címzett `assets/<aa>/<bb>/<sha>.<ext>`,
      tömörítés nélkül) + `ops.asset.storage_key` írása/upsertje; a CLI
      `asset import` most `file`+`sha256` mezőt is kezel (sha-ellenőrzés,
      eltérésnél megáll). Tesztek: blob-integráció + storage_key upsert.
      Commit `6dc8354`.
- [x] v1-audit mapping (exact név + ország, nincs fuzzy): 86 élő csapatból
      30 helyi blob, 14 forrás-URL, 20 asset nélkül, 22 név nincs a
      v1-ben; 4 liga-logó (PL, Bundesliga, Serie A, Ligue 1); La Liga
      nyitott (nincs v1 tournament-bejegyzés). Összesen **48 asset-sor,
      34 blob**; egy blob sha256-ja ellenőrizve.
- [x] Licenc: a v1 források nem dokumentálnak licencet →
      `licence_status='unknown'` (megjelenítés előtt tudatos `cleared`
      döntés kell). Ops doc: `docs/operations/assets.md`.
- [ ] S8: asset/Elo megjelenítés a kliensben (következő szelet).

### Nyitott pontok

1. A retry-kör záró státuszai (jobonkénti) — a következő tickekből.
2. A 2026-11-16-i első teljes meccses kör (review-bootstrap a lokalizált
   Soccerway-nevekre).
3. La Liga logó és a 36 asset nélküli/eltérő nevű csapat pótlása
   (operátori mapping vagy újabb forrás).

## S8a/S8b — kliens-integráció első köre (2026-11-15)

- [x] **API (S8a, `870be07`):** a napi válasz `refreshed_at`-tel (a nap
      legutóbbi gate-kiértékelése), meccsenként `home_logo`/`away_logo`
      (blob → `/v1/asset/<storage_key>`, egyébként forrás-URL) és
      `home_elo`/`away_elo` (club_elo as-of; a dev DB-ben még üres, mert a
      clubelo élő kör nem futott). Új `/v1/asset/{*key}` végpont a helyi
      objektumtárból, szigorú kulcs-validációval (path-traversal tiltva),
      read-tokenes auth, cache-header. Tesztek: day payload + asset
      200/401/400/404.
- [x] **Kliens (S8b, `b9f395e`):** `DayFeed` (refreshed_at + meccsek) a
      core csomagban; a fejléc `frissítve <idő>` jelzést mutat; a
      sorokban valódi logó (helyi API blob tokennel, külső URL token
      nélkül, memória-cache; betöltés/hiba alatt semleges helyőrző);
      ELO-chip, ha van adat. **Javított hiba:** a country-kód tábla nem
      ismerte a v2 kódokat (GB/ES/IT/DE/FR/HU) → a zászlók helyőrzők
      voltak; a régi kódok kompatibilitásként megmaradtak.
- [x] Tesztek: core 19 (Swift Testing), app 14 (XCTest), e2e UI-ellenőrzés
      valódi API-val: a 2026-11-11-i napon a fejléc
      „2026 · szerda · 20 meccs · frissítve nov. 14. 10:57", a Chelsea
      logója renderelődik (v1-audit asset), a blob-végpont
      `200 image/webp` 5,5 KB.
- Megjegyzés: a dev DB-ben ezen a napon csak a demó meccsek élnek (a
  korábbi élő footballdata-sorok ebben a DB-állapotban nincsenek, árva
  provenance nélkül — nem a mai futások törölték). Az élő SDK-teszthez
  a 2026-11-16-i teljes kör adja a bizonyítékot.
- Következő: Üzemeltetés nézet (worker/review), asset-licence döntés
  (`unknown` → `cleared`), La Liga logó pótlása, élő adat visszatöltése.

### Soccerway canary nap végeredménye

- [x] 6/7 domain `no_data` (Ligue 1, Bundesliga, Serie A, NB I, NB II,
      Premier League), 1 domain (La Liga) **pending: napi kvóta kimerült**
      → a következő napon fut. Az angliai `failed` coverage az első,
      javítás előtti futás maradéka (a job done).

## Üzemeltetés nézet + licence + első meccses nap előkészítés (2026-11-15, 2. folytatás)

- [x] **Ops API:** `GET /v1/ops/status` (ops-token): worker-számlálók
      (pending/leased/quarantine/review) + forrásonkénti policy (mode,
      enabled, mai kísérletek/büdzsé, breaker). `worker_repo::list_policies`.
      Teszt: számlálók + forrás + read-token elutasítás. (commit lent)
- [x] **Üzemeltetés nézet:** tabváltás a felső sávban (Ma / Üzemeltetés);
      4 stat-kártya; forráslista mód-pillel, enabled-jelzővel,
      kvóta-sávval és breaker-chippel; 30 s automatikus frissítés;
      `OpsModel` + core `OpsService`/`OpsStatus`. Tesztek: core 20, app 17.
      Élő kép: 1 függő job, demo 1/10, footballdata 16/25 (LIVE), fotmob
      0/10 (tiltott), soccerway 14/14 (CANARY, teljes sáv).
- [x] **Asset-licence döntés:** a v1-audited 53 sor
      `licence_status='cleared'` (a 2026-11-15-i újrahasznosítási döntés
      alapján, belső használat; licence-szöveg rögzítve). Rollback:
      vissza `unknown`-ra.
- [ ] **La Liga liga-logó:** nincs auditált forrás (v1 `Primera División`
      tournament nincs a logó-manifestben; a külső gyűjtemény csak
      klub-logókat ad) → operátori forrásra vár; a kliens liga-fejlécben
      zászlót mutat.
- [x] **Első teljes meccses nap runbookja** a Soccerway ops docban
      (daemon worker + review-bootstrap + második kör + ellenőrzés);
      a 2026-11-16-i futás a napi discovery-vel indul.
