# CI — GitHub Actions üzemeltetési útmutató

Állapot: 2026-11-22. A workflow a `.github/workflows/ci.yml`, a projekt kapu-láncát futtatja
(`.dsh/skills/football-app-v2/SKILL.md`). A CI a `push` (main), a `pull_request` és a kézi
`workflow_dispatch` eseményre indul; a `concurrency` csoport a korábbi, azonos ágra indult
futásokat megszakítja.

> Történet: a workflow S31b-ben készült, S32b–S32c javítások után a `44733358476` futás volt az
> első teljes zöld futás (2026-11-20, a checkpoint rögzíti). Ez a dokumentum a 2026-11-22-i
> állapotot írja le: a Rust job PostgreSQL 16 service konténert használ, a két job párhuzamos.

## 1. Mit futtat a CI

### Rust job (`ubuntu-latest`, PostgreSQL 16 service, 40 perc timeout)

| Lépés | Parancs / művelet | Mit bizonyít |
|---|---|---|
| Forrás | `actions/checkout@v4` | a commit tartalma |
| Toolchain | `dtolnay/rust-toolchain@stable` + `components: rustfmt, clippy` | a `rust-toolchain.toml` (stable, rustfmt, clippy) teljesül |
| Toolchain-ellenőrzés | `cat rust-toolchain.toml`, `rustup show active-toolchain`, `rustc/cargo/rustfmt/clippy --version` | a tényleges verziók a logban, nem csak szándék |
| Cache | `actions/cache@v4`: `~/.cargo/registry`, `~/.cargo/git`, `rust/target`; kulcs: `os-cargo-hash(rust/Cargo.lock)` | gyorsabb újrafutás |
| sqlx-cli | cache (`~/.cargo/bin/sqlx`, `~/.cargo/bin/cargo-sqlx`) + `cargo install sqlx-cli --version ^0.8 --no-default-features --features rustls,postgres --locked` | a migrációs CLI verziója a workspace sqlx 0.8-hoz illeszkedik |
| DB kliensek | `postgresql-client` telepítése | `psql`, `createdb` elérhető |
| Adatbázisok | `CREATE DATABASE football_v2_dev;` és `..._test;` a service `postgres` alap DB-jéből | a két fejlesztői DB a `.env.example` mintájára létrejön |
| Migrációk (dev) | `sqlx migrate run --source db/migrations` | a `football_v2_dev` a 0001–0021 láncon van |
| Migrációk (test) | `./scripts/db-reset.sh` (repo-script) | a `football_v2_test` üres, migrált állapotban |
| Alapkapu | `./scripts/check-foundation.sh` | `db-check` (49 tábla, 6 szabály), fmt, domain/storage teszt, clippy |
| Teljes teszt | `cd rust && cargo test --workspace -- --test-threads=1` | minden crate tesztje; a közös teszt-DB miatt egy szálon |
| Clippy | `cd rust && cargo clippy --workspace --all-targets -- -D warnings` | nulla figyelmeztetés |
| Formázás | `cd rust && cargo fmt --all --check` | a kód formázott |
| Build | `cd rust && cargo build --workspace` | a teljes workspace fordul |
| Bináris build | `cd rust && cargo build -p footballd -p footballctl --features obscura` | a projekt kapu-láncának daemon/CLI buildje |

A két adatbázist a CI közvetlenül a service `postgres` superuserével hozza létre, nem a
`scripts/db-create.sh`-val: az a script a helyi socket-hitelesítésre és a fejlesztői `.env`-re
épül (role + két DB), ami a konténerben felesleges kör. A migrációt és a teszt-DB ürítését
viszont a repo saját scriptjei végzik (`sqlx migrate run`, `./scripts/db-reset.sh`), hogy a CI
ne duplikálja a helyi lánc logikáját.

Miért `cargo install sqlx-cli`, ha a repóban van beágyazott migráló? Mert van: a
`footballctl migrate` és a `footballd` induláskor `sqlx::migrate!`-tal migrál
(`rust/bins/footballctl/src/main.rs:509`, `rust/bins/footballd/src/main.rs:36`), a tesztek pedig
`rust/crates/storage/src/test_support.rs`-ban. A kapu-lánc viszont a `./scripts/db-reset.sh`-ra
épül, és az `sqlx migrate run`-t hív, ezért a CI a sqlx CLI-t telepíti: így pontosan a helyi
láncot futtatja, nem egy CI-specifikus második migráló utat.

### Swift job (`macos-latest`, 30 perc timeout)

| Lépés | Parancs / művelet | Mit bizonyít |
|---|---|---|
| Forrás | `actions/checkout@v4` | a commit tartalma |
| Xcode | `xcode-select` ellenőrzés/javítás, `xcodebuild -version` | teljes Xcode, nem csak CommandLineTools |
| XcodeGen | `brew install xcodegen` | a projekt generálható |
| Core tesztek | `swift test --package-path apps/FootballAppCore` | a kliens-üzleti logika tesztjei |
| Projekt | `cd apps && xcodegen generate` | az Xcode-projekt a `project.yml`-ból |
| App build | `xcodebuild build -project apps/FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO` | az app aláírás nélkül fordul |
| App unit tesztek | `xcodebuild test ... -only-testing:FootballAppTests CODE_SIGNING_ALLOWED=NO` | az app-szintű unit tesztek |

Az `FootballAppUITests` XCUITest harness **szándékosan nem** fut CI-ben (lásd 5. pont).

## 2. Környezet és hitelesítő adatok

- A PostgreSQL egy eldobható service konténer (`postgres:16-alpine`); a hitelesítő adat a
  PostgreSQL alapértéke (`postgres` / `postgres`), **nem titok**, és a futás után megsemmisül.
- A repóban nincs `.env`; a lépések a job-szintű env változókból dolgoznak:
  `DATABASE_URL`, `TEST_DATABASE_URL`, `APP_TIMEZONE`, `DATA_DIR`, `BIND_ADDR`,
  `PGHOST/PGPORT/PGUSER/PGPASSWORD`, valamint `CARGO_INCREMENTAL=0`.
- Valódi titok (ha valaha kell) csak **GitHub Secret**-ből jöhet; a repóba nem kerülhet.
  Forkolt PR futásakor a secretek nem érhetők el — ezért a workflow secretek nélkül is zöld kell legyen.
- A helyi `.env` és a CI env szándékosan külön él; a CI nem olvassa a fejlesztői `.env`-et.

## 3. Hogyan reprodukáljuk lokálisan

Előfeltétel: a helyi PostgreSQL (a projekt előfeltétele 18+), Rust stable a `rust-toolchain.toml`
szerint, sqlx-cli, és `.env` a `.env.example` alapján.

```bash
cd ~/Documents/Code/football-app-v2
cargo install sqlx-cli --version '^0.8' --no-default-features --features rustls,postgres --locked

cp -n .env.example .env        # ha még nincs; a LOCAL_DB_PASSWORD-öt ki kell tölteni
set -a && source .env && set +a
./scripts/db-create.sh         # a két DB és a football_v2 szerep, ha még nincs
./scripts/db-reset.sh          # CSAK a teszt-DB-t üríti és migrálja

# ugyanaz a kapu-lánc, mint a CI Rust jobja:
./scripts/check-foundation.sh
cd rust
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build --workspace
cargo build -p footballd -p footballctl --features obscura
cd ..
./scripts/db-reset.sh          # a helyi lánc végén a teszt-DB újra üres
```

Swift oldal (a CI Swift jobja, Xcode szükséges):

```bash
cd apps && xcodegen generate && cd ..
swift test --package-path apps/FootballAppCore
xcodebuild build -project apps/FootballApp.xcodeproj -scheme FootballApp \
    -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
xcodebuild test -project apps/FootballApp.xcodeproj -scheme FootballApp \
    -destination 'platform=macOS' -only-testing:FootballAppTests CODE_SIGNING_ALLOWED=NO
```

Különbségek a CI-hez képest, amiket tudni kell:

- a helyi fejlesztés PostgreSQL **18**-at használ (README-előfeltétel), a CI a feladat szerint
  **16**-ot. A migrációk közül egyedül a `db/migrations/0005_identity_layer.sql` használ
  PG15-ös szintaxist (`UNIQUE NULLS NOT DISTINCT`), ezért elvileg kompatibilisek, de a PG16-os
  futás a CI-ben, nem lokálisan bizonyított;
- a CI friss konténerben fut, ezért nincs helyi állapot (a `db-reset.sh` a CI-ban is üres DB-t ad);
- a helyi láncban a `db-reset.sh` a teszt-DB-t üríti, a dev DB tartalmához nem nyúl.

## 4. Mikor zöld a CI

Akkor zöld, ha mindkét job `success`:

- **Rust**: a két DB létrejön, a migrációk lefutnak, a `db-check` 49 táblát és 6 validációs
  szabályt lát, a teszt-DB üres; a `check-foundation`, a teljes teszt, a clippy `-D warnings`,
  a `fmt --check`, a workspace build és az obscura bináris build mind exit 0.
- **Swift**: a Core tesztek zöldek, az XcodeGen generál, az app aláírás nélkül buildel, az app
  unit tesztek zöldek.

A zöld CI **nem** jelenti a teljes Definition of Done-t: a valódi adatminőség, a VoiceOver/Tab
viselkedés, a futó daemon és a valódi macOS UI továbbra is külön ellenőrzés (lásd 5. pont).

## 5. Mi nincs benne (szándékosan)

- **VoiceOver-felolvasás és Tab-fókusz valódi macOS-en**: Accessibility/TCC engedély és futó
  ablak kell hozzá; GitHubon nem reprodukálható. Helyben: `docs/operations/ui-accessibility-checklist.md`,
  illetve `./scripts/run-ui-tests.sh`.
- **`FootballAppUITests` XCUITest harness**: a determinisztikus fixture-szerver ellenére valódi
  macOS ablakot igényel; helyben fut.
- **Valódi források / ingest / hálózat**: a CI nem hív külső forrásokat, nem tölt adatot, nem
  futtatja a daemont, a launchd-t vagy a workert.
- **Adatminőségi és integritás-ellenőrzés**: a CI adatbázisai üresek; a `reverify`, `integrity`,
  `coverage` és a publikációs kapu valódi adaton helyben/üzemeltetésben fut.
- **Backup/restore próba, p95 teljesítmény, terheléses teszt**: nincs mérés.
- **Code signing, notarizáció, release artifact**: a build szándékosan `CODE_SIGNING_ALLOWED=NO`.
- **PostgreSQL 18-as környezet**: a CI PG16-on fut; ha a projekt PG18-specifikus SQL-t kezd
  használni, a service image-et és a helyi előfeltételt együtt kell emelni.
- **Windows/Linux Swift**: a kliens csak macOS-en épül.

## 6. Cache és költség

- Egy `actions/cache@v4` fogja a `~/.cargo/registry`, `~/.cargo/git` és `rust/target` könyvtárat,
  a kulcs a `rust/Cargo.lock` hash-e (fallback: `os-cargo-`).
- A `CARGO_INCREMENTAL=0` kisebb target cache-t ad; CI-ban nincs értelme inkrementális buildnek.
- A `rust/target` cache nagy lehet (több GB). Ha a repó 10 GB-os cache-keretéhez közelít, a
  `rust/target` sort ki kell venni a cache-ből, és csak a registry/git marad — ez lassítja a
  buildet, de nem rontja a zöld feltételt.
- Az sqlx-cli külön cache (`sqlx-cli-0.8-<os>`), hogy ne kelljen minden futásnál telepíteni.
  A cache csak a `sqlx` és `cargo-sqlx` binárist fogja, a teljes `~/.cargo/bin`-t nem: így a
  rustup shimeket (cargo, rustc, rustfmt) nem írhatja felül egy régi cache.

## 7. Hibakeresés

- **`cargo install sqlx-cli` verzióhiba**: a `--version 0.8` alakot a cargo elutasítja; `^0.8`
  kell (ezt a S32b javította). A workspace sqlx 0.8-hoz illeszkedő CLI-t használj. Ha a gépeden
  már újabb (pl. 0.9) sqlx-cli van, a `migrate` azzal is fut; a CI szándékosan 0.8-at telepít.
- **`db-check` piros**: a várt táblaszám 49; ha új migráció jött, a `scripts/db-check.sh`
  `expected_tables` értékét a migrációval együtt kell emelni (nem a CI-ban, hanem a scriptben).
- **Teszt-DB nem üres**: a `db-reset.sh` a teszt-DB-t üríti; ha egy teszt a dev DB-re ír, a
  `db-check`/teszt elhasal — a tesztet kell javítani, nem a scriptet lazítani.
- **Időbélyeg-eltérés Linuxon**: a PostgreSQL `timestamptz` mikroszekundumos; a tesztekben
  `Utc::now().trunc_subsecs(6)` kell (S32c tanulsága), különben macOS-en zöld, Linuxon piros.
- **Xcode/xcodegen hiba**: a `macos-latest` runner Xcode-verziója változhat; a logban az
  `xcodebuild -version` mutatja, melyikkel futott. A projektgenerálás mindig `xcodegen generate`.
- **Cache-hiba**: törölni kell a GitHub Actions cache-t (Settings → Actions → Caches), vagy
  emelni a kulcsot; a cache nem része a zöld feltételnek.
- Újrafuttatás: a GitHub UI „Re-run failed jobs” gombja, vagy `gh run rerun <run-id> --failed`.

## 8. Letiltás és visszaállítás

A CI letiltása nem érint adatot és nem igényel migrációt. Sorrend, ha vissza kell állítani:

1. **Ideiglenes letiltás (fájlmódosítás nélkül)**: GitHub → Actions → CI → „Disable workflow”,
   vagy `gh workflow disable CI`. A már sorban álló futások nem indulnak el.
2. **Végleges eltávolítás**: a `.github/workflows/ci.yml` törlése egy külön commitban
   (`chore(ci): CI workflow eltávolítása`), majd push. A workflow a main-re push után eltűnik.
3. **Visszaállítás git-ből**: `git revert <a törlő commit>` vagy a fájl visszaállítása
   (`git checkout <jó-commit> -- .github/workflows/ci.yml`). A CI minden állapota a verziózott
   fájlban van, nincs rejtett szerveroldali konfiguráció.
4. **Kötelező státusz-check**: ha a branch protection a `CI` checket kötelezővé tette, a
   letiltás/törlés előtt a GitHub repó Settings → Branches alatt kell a checket kivenni, különben
   a PR-ok nem lesznek merge-elhetők.
5. A CI nem ír a dev adatbázisba, nem telepít daemont, nem nyúl a launchd-hez; a letiltásnak
   nincs adat-oldali visszaállítási útja.

## 9. Karbantartás és ismert eltérések

- Az action-ök major tagre vannak kötve (`actions/checkout@v4`, `actions/cache@v4`,
  `dtolnay/rust-toolchain@stable`). Supply-chain keményítésként SHA-ra lehet őket pinnelni.
- A két job megjelenített neve szándékosan a régi maradt (`Rust (domain, storage, API, worker)`,
  `Swift kliens (Core + app unit tesztek)`), mert a required status check a megjelenített névre
  hivatkozik: egy átnevezés némán merge-elhetetlenné teheti a PR-okat. A PostgreSQL 16-ot és az
  új lépéseket a workflow-komment és ez a dokumentum írja le, nem a job neve.
- A PostgreSQL image `postgres:16-alpine`; a projekt helyi előfeltétele 18+, ezért a kettőt
  együtt kell léptetni, ha a migrációk újabb SQL-t használnak.
- **Ismert dokumentációs eltérés**: a skill `references/roadmap.md` 2026-11-20-i sora még azt
  írja, hogy „CI (.github/workflows) HIÁNYZIK”, és a P0-listán is szerepel. Ez elavult: a
  workflow létezik (S31b) és zöld futása volt (`44733358476`, 2026-11-20). A roadmapet nem
  módosítottam (írási hatókörön kívül esik), a leadnek kell frissítenie.
- A README „Continuous integration” szakasza a 2026-11-20-i workflow-t írja le (PG18 service,
  `build --features obscura`); a mostani fájl PG16-ot és `cargo build --workspace`-t is futtat.
  A README-t szintén nem módosítottam; a két dokumentumot egy szeletben kell összehangolni.