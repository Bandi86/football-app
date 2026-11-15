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
