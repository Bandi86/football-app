# Rust backend — projekt-specifikus konvenciók

## Modulhatárok (AGENTS.md §5 + spec §4.2)
- `domain`: tiszta, DB/HTTP/fs nélkül. `storage`: DB. `ingest`: forrásadapterek. `verification`: szabálymotor.
  `api`: HTTP. `worker`: ütemezés, fetch, materializáció. `identity`: alias/review/merge.
- Az API-handler nem tartalmaz domain-szabályt; a forrásadapter nem ír közvetlenül kanonikus entitást.
- Production `unwrap()`/`expect()` és csendben elnyelt hiba tilos.

## Kapu-lánc (minden kódváltozás után, ebben a sorrendben)
```bash
cd ~/Documents/Code/football-app-v2
set -a && source .env && set +a        # TEST_DATABASE_URL nélkül a db-reset elbukik
./scripts/db-reset.sh                 # CSAK a teszt-DB-t
./scripts/check-foundation.sh
cd rust
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p footballd -p footballctl --features obscura
cd .. && ./scripts/db-reset.sh
```
- Kézzel írt kódnál előbb `cargo fmt --all`, különben a `fmt --check` elbukik.
- A DB-s integrációs tesztek közös teszt-DB-t használnak → `--test-threads=1` kötelező.
- A binárisokat mindig `--features obscura`-val buildeld; enélkül a Soccerway-jobok transport hibát adnak.
- A futó daemon a `~/Library/Application Support/FootballAppV2/agent/bin/` alól fut: a repo buildje önmagában nem elég,
  a telepítő `./scripts/install-footballd-agent.sh` (közvetlenül; osascript/Terminal wrapper nélkül).

## Adat- és migrációs fegyelem
- Migráció tranzakciós, forward+rollback dokumentálva; adatjavítás nem migráció, hanem audítált revízió/import.
- Minden revízióhoz `ops.audit_event`; dry-run az alap, `--apply` a végrehajtás.
- A hiányzó adat `NULL`/státusz (`no_data`, `missing`, `blocked`, `stale`, `conflict`), soha 0.
- A `publish_gate` tábla az igazság; a CLI-összegző más ablakra számolhat.
- Provenance nélküli tény nem jelenhet meg; bizonytalan auto-merge tilos.

## Tesztkészlet és ismert rések
- Jelenleg: 79 Rust forrás tartalmaz `#[test]`/`#[tokio::test]`; a teljes workspace 85 csoport / 0 failure.
- Megvan: domain unit, storage integráció, API route tesztek, parser golden fixture-ök.
- **Hiányzik/ellenőrizetlen:** CI workflow, OpenAPI↔Swift kliens-kontrakt teszt, backup-restore próba,
  helyi p95 teljesítménymérés, havi restore-ellenőrzés. Ezek a spec §11/§13 követelményei.
- Hibatípusok és hibás minták részletesen: `errors`/`pitfalls` a fő SKILL.md-ben és `references/data-ops.md`-ben.
