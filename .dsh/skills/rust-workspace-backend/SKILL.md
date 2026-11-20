---
name: rust-workspace-backend
description: "Use when writing or reviewing Rust code in the football-app-v2 cargo workspace: crate boundaries, dependency policy, typed errors, sqlx queries, tokio/tracing patterns, or test layout."
whenToUse: "Any change under rust/ (crates/*, bins/*), new dependency, new migration-adjacent query, or a Rust test-design decision for football-app-v2."
---

# Rust workspace — football-app-v2 backend

A `rust/` egyetlen cargo workspace: `crates/{domain,storage,ingest,identity,verification,api,worker}`
és `bins/{footballd,footballctl}`. Ez a skill a **kódszintű konvenciókat** hordozza; a szelet-
munkafolyamat, a kapu-lánc pontos parancsai és a daemon-telepítés a `football-app-v2` skillben van
(annak `references/rust-backend.md`-jével együtt) — azt is töltsd be, ne duplikáld.

## 1. Workspace-tények

| Tény | Érték |
|---|---|
| Edition / resolver | 2024 / 3 |
| `rust-version` | 1.85 |
| Toolchain | `rust-toolchain.toml`: stable + rustfmt + clippy (minimal) |
| Crate-nevek | `football-domain`, `football-storage`, `football-ingest`, `football-identity`, `football-verification`, `football-api`, `football-worker` |
| Binárisok | `footballd`, `footballctl` (`--features obscura` kötelező a buildhez) |
| Függőség-verziók | egy helyen: `rust/Cargo.toml` `[workspace.dependencies]` |

## 2. Függőség-politika

- Új dependency: előbb `[workspace.dependencies]`-be (ellenőrzött verzió), utána a crate-ban
  `dep.workspace = true`. Indoklás a szelet tervében/checkpointjában; „csak kényelmi” crate nem kell.
- A crate-ek ne hozzanak létre saját verziót ugyanarra a könyvtárra.
- Jelenlegi fő elemek: `axum` 0.8, `sqlx` 0.8 (runtime-tokio, postgres, uuid, chrono, json, migrate,
  macros), `tokio` 1 (full), `reqwest` 0.13.5 (rustls), `thiserror` 2, `anyhow` 1, `tracing` 0.1,
  `wiremock` 0.6.5, `zstd` 0.13, `chrono`/`chrono-tz`.

## 3. Crate-határok (mért, ne találgass)

```
domain        → (nincs lokális függőség; csak chrono/serde/thiserror/uuid)
storage       → domain
api           → storage
identity      → domain, storage
ingest        → domain, identity, storage
verification  → domain, ingest, storage
worker        → mind (ütemezés, fetch, materializáció)
```

Kötelező szabályok:

- `domain` tiszta: nincs DB, HTTP, fájlrendszer, gyűjtés.
- Forrásadapter **soha** nem ír közvetlenül kanonikus entitást (az ingest → identity/review úton megy).
- API-handler nem tartalmaz domain-üzleti szabályt: a szabály a domain/verification rétegben él.
- Új kereszt-él (pl. `storage → ingest`) tilos; ha mégis kellene, az architektúra-döntés, nem kód-kényelem.

## 4. Hibakezelés

- Tipizált hiba: `thiserror` a domain/identity/ingest rétegben, magyar, felhasználónak értelmezhető
  üzenettel (pl. `crates/domain/src/error.rs`: `FetchError::Timeout`, `ParseError::DateGuardMismatch`).
- `anyhow` csak a széleken (binárisok, worker/API belépési pont) kontextus-hordozásra.
- Production kódban `unwrap()`/`expect()` tilos; a hiba nem tűnhet el csendben — legrosszabb esetben
  `tracing::warn!` + tipizált státusz, de a státusz a felhasználónak is látszik.
- HTTP-válaszban a hibaüzenet magyar és nem szivárogtat belső részletet (SQL, útvonal, token).

## 5. SQL és migrációk

- `sqlx::query_scalar` / `query_as::<_, RowStruct>` explicit row-structtal; `?` propagálás, ne `.ok()`.
- Idő mindig UTC a DB-ben; helyi megjelenítés a kliens dolga (`chrono-tz`).
- Migráció: `db/migrations/NNNN_<slice>_<téma>.sql`, egyetlen `BEGIN;`/`COMMIT;`, a fejlécben a
  visszaállítási út. Adatjavítás **nem** migráció: audítált revízió/import (dry-run alap, `--apply`).
- Hiányzó adat `NULL` vagy státusz (`no_data`, `missing`, `blocked`, `stale`, `conflict`) — soha 0.
- A teszt-DB-re előbb minden migráció lefut, csak zöld után a dev-DB.

## 6. Async, naplózás, hálózat

- `tokio` full; adapter-trait `async-trait`; hosszú fetch időtúllépéssel és rate-limittel.
- Minden külső kérés mögé: robots/policy tisztelet, méretplafon, sémaellenőrzés, breaker/budget.
- `tracing` strukturált mezőkkel (`domain`, `job_id`, `source`), hogy a napló grep-elhető legyen.
- Háttérfeladat ne kapja el a pánikot csendben: `JoinHandle` hibája kerüljön a job-státuszba.

## 7. Tesztek

- Unit teszt a kód mellett (`mod tests`), integrációs teszt `crates/*/tests/`.
- HTTP-adapter: `wiremock` stub; fájlos fixture: `tests/fixtures/` (ne generált adat a repóban).
- A storage-tesztek **közös** teszt-DB-t használnak (setup `TRUNCATE`), ezért
  `cargo test --workspace -- --test-threads=1` kötelező — párhuzamosan versenyhelyzet.
- Új parser-viselkedéshez golden fixture + a rossz eset (decoy, dátum-guard, séma-drift) is tesztelve.
- Egy teszt akkor bizonyíték, ha a futtatott kimenet megvan; a „zöld” szót csak akkor használd.

## 8. Ismert csapdák

- `cargo test -p ingest` **nem létező csomag** — a helyes név `football-ingest`.
- Kézzel írt kód után előbb `cargo fmt --all`, különben a lánc `fmt --all --check`-je elbukik.
- `clippy -D warnings`: `is_none_or(...)`, nem `map_or(true, …)`.
- Bináris build `--features obscura` nélkül a Soccerway-jobokat transport hibára állítja.
- A repo buildje nem frissíti a futó daemont: `./scripts/install-footballd-agent.sh` kell (közvetlenül).
- Amíg a régi bináris fut, ne queue-zz újra jobokat — a régi parserrel figyelne meg újra.