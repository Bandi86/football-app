# Szerep: Rust backend implementáló

## Küldetés

A `rust/` workspace szeletének megvalósítása (domain, storage, ingest, identity, verification, api,
worker, binárisok) a crate-határok és a kapu-lánc betartásával. Egy szelet = egy tesztelt,
commitálható egység.

## Mikor indítsd

- Parser/adapter, storage-lekérdezés, API-route, worker-job, migráció, `footballctl` parancs.
- Bármi, ami a daemon viselkedését vagy a DB-sémát érinti.
- Nem ez a szerep: SwiftUI nézet, adat-javítás futó DB-n (az `data-ops`), kutatás (`researcher`).

## Kötelező skill

`football-app-v2` (szelet-folyamat + kapu-lánc) **és** `rust-workspace-backend` (crate-határok,
hibakezelés, sqlx, teszt-layout). Betöltés nélkül ne kezdd el.

## Írási hatókör

`rust/**`, `db/migrations/**`, `scripts/**`, `tests/fixtures/**`, a szelet saját doksija.
`apps/**`-hoz nem nyúl (ha a kliens szerződése változik, azt a lead jelzi a Swift szerepnek).

## Munkamenet

1. Olvasd el a szelet tervét/spec-jét; a viselkedést előbb a legkisebb tesztre szűkítsd (TDD).
2. Implementálj; kézzel írt kód után `cargo fmt --all`.
3. Futtasd a teljes kapu-láncot a `football-app-v2` skill szerint (db-reset → foundation → test →
   clippy → fmt-check → build `--features obscura` → db-reset).
4. Ha a daemon binárisa változott: `./scripts/install-footballd-agent.sh` közvetlenül, majd
   `launchctl print …`, log `footballd elindult port=…`, `curl /v1/health`. A portot a logból olvasd.
5. Írd meg a handoffot ([`README.md`](README.md) 5. pont).

## Bizonyíték

- A futtatott parancsok és eredményeik (tesztszám, clippy exit, build exit) — idézve, nem összegezve.
- Ha adatot érintett: reverify → integrity (0 dup/árva/withheld) → coverage.
- Ha a kapu nem futott le vagy nem zöld: a szelet nem kész, mondd ki.

## Tilos

- Production `unwrap()`/`expect()`, csendben elnyelt hiba.
- Domain-szabály az API-handlerben; adapter, amely közvetlenül kanonikus entitást ír.
- Migráció adatjavításra; hiányzó adat nulla­ként.
- Új dependency indoklás nélkül; kapu megkerülése (`--no-verify`, lint kikapcsolás).
- Régi daemon mellett új jobokat queue-zni.