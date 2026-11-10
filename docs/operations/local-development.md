# Helyi fejlesztés

## Előkészítés

A környezet a `.env` fájlban állítható be (a `.env.example` alapján; a `.env` nem kerül verziókövetésbe). Az adatbázisok létrehozása és a környezet betöltése:

```bash
./scripts/db-create.sh
set -a; . ./.env; set +a
export DATA_DIR="$PWD/data"
```

## Migráció és fixture

A fixture-ök szintetikus demóadatok (nem valódi forrásadat). A pipeline a dev adatbázisra tölti őket; a nyers artifactok a gyökér `data/` könyvtárába kerülnek (gitignore-olt).

```bash
cd rust
cargo run -p footballctl -- migrate
cargo run -p footballctl -- ingest-fixture --path ../tests/fixtures/fixture_a_day.json
cargo run -p footballctl -- ingest-fixture --path ../tests/fixtures/fixture_b_confirm.json
cargo run -p footballctl -- day --date 2026-11-09
cargo run -p footballctl -- day --date 2026-11-09 --include-all
cargo run -p footballctl -- doctor
```

A `DATA_DIR` a gyökér `data/` könyvtárára mutasson (abszolút útvonal, vagy a `rust/` mappából `../data`). A `rust/data` nincs gitignore-olva, oda nyers payload nem kerülhet.

## Teljes ellenőrzés

```bash
./scripts/check.sh             # tesztadatbázis: reset + fmt + clippy + szekvenciális tesztek
./scripts/check-foundation.sh  # szerkezeti ellenőrzés mindkét DB-n + alaptesztek
./scripts/db-reset.sh          # a tesztek után a tesztadatbázis visszaállítása
```

A `db-check.sh` a dev adatbázisnál a szerkezetet ellenőrzi (21 tábla, 6 szabály; üzleti sor megengedett), a tesztadatbázisnál szigorú (0 üzleti sor). A `check.sh` és a `check-foundation.sh` tesztjei üzleti sorokat hagyhatnak a tesztadatbázisban; utánuk `db-reset.sh` szükséges.

## Git

Commit csak kifejezett felhasználói kérésre készül; push csak kifejezett kérésre. A repo publikus: titok, jelszó, token és nyers adat nem kerülhet bele.