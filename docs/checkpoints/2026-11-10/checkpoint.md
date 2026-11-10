# 2026-11-10 — Napi checkpoint

**Állapot:** kész, zöld (Task 3 + Task 4 + Task 5 + Task 6 + Task 7 + Task 8 + Task 9 + Task 10 + Task 11 + Task 12 + daemon HTTP E2E + Task 13)  
**Előző nap:** 2026-11-09  
**Kapcsolódó terv:** [Ellenőrzött napi lista backend](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md), Task 3

## Napi cél

A `raw`, `stage` és `ops` adatbázis-sémák első, ellenőrzött migrációjának elkészítése és tesztadatbázison való igazolása.

Ez a nap még **nem** tartalmaz meccs-, csapat-, játékos-, forrás- vagy oddsadatot. Az egyetlen engedélyezett adat a rendszer működéséhez szükséges hat validációs szabály metaadata.

## Kiinduló állapot

- [x] A git branch és a munkafa áttekintése megtörtént.
- [x] Az előző napi alapozás változatlanul zöld.
- [x] A `football_v2_dev` és `football_v2_test` adatbázis elérhető.
- [x] Mindkét adatbázis üres; nulla felhasználói tábla.
- [x] Nincs `.env` vagy titok a verziókövetésben.

Ellenőrző parancs:

```bash
./scripts/check-foundation.sh
git status --short --branch
```

Megállási feltétel: ha bármely ellenőrzés piros, a nap nem megy tovább, előbb a hibát kell megérteni és javítani.

## Holnapi munkasorrend

### C0 — Környezet és eszköz

- [x] Az `AGENTS.md` és a legutóbbi checkpoint átolvasása.
- [x] `sqlx-cli` meglétének ellenőrzése.
- [x] Ha hiányzik, telepítés: `cargo install sqlx-cli --no-default-features --features postgres,rustls`.
- [x] `sqlx --version` sikeres.
- [x] `./scripts/db-check.sh` zöld, mindkét adatbázis üres.

### C1 — Migrációs fájl előkészítése

- [x] `db/migrations/0001_raw_stage_ops.sql` létrehozása.
- [x] Csak a következő sémák és táblák kerülnek bele:
  - `raw.artifact`
  - `raw.capture`
  - `stage.observation`
  - `ops.source`
  - `ops.validation_rule`
  - `ops.validation_run`
  - `ops.validation_finding`
  - `ops.audit_event`
- [x] A migráció tranzakcióban fut.
- [x] A migráció újrafuttatható viselkedése és a visszaállítási út dokumentált.
- [x] Nincs meccs-, csapat-, játékos-, forrás- vagy oddsrekord.

### C2 — Tesztadatbázis-first végrehajtás

- [x] A migráció először kizárólag a `football_v2_test` adatbázison fut le.
- [x] A `_sqlx_migrations` rendszer tábla mellett pontosan a nyolc tervezett tábla létezik.
- [x] A hat `validation_rule` sor létezik.
- [x] Az összes többi tábla sorszáma nulla.
- [x] A migráció második futtatása nem hoz létre duplikált szabályt.

Ellenőrző lekérdezések:

```sql
SELECT count(*) FROM pg_tables
WHERE schemaname NOT IN ('pg_catalog', 'information_schema');

SELECT count(*) FROM ops.validation_rule;

SELECT
  (SELECT count(*) FROM raw.capture) AS captures,
  (SELECT count(*) FROM stage.observation) AS observations,
  (SELECT count(*) FROM ops.validation_run) AS runs,
  (SELECT count(*) FROM ops.validation_finding) AS findings,
  (SELECT count(*) FROM ops.audit_event) AS audits;
```

Elvárt:

- táblák száma: 9, ebből 8 terméktábla és 1 SQLx-migrációs tábla;
- `validation_rule`: 6;
- minden üzleti adatszám: 0.

### C3 — Fejlesztői adatbázis

- [x] Tesztadatbázis zöld után a migráció a `football_v2_dev` adatbázison is lefut.
- [x] Ugyanazok a táblák és szabályok jelennek meg.
- [x] Nincs üzleti adat.
- [x] A migrációs napló sikeres.

### C4 — Tesztek és ellenőrzések

- [x] A storage-kapcsolati teszt a létrejött sémához igazítva zöld.
- [x] A foundation-check script zöld.
- [x] `cargo fmt --all --check` zöld.
- [x] `cargo test -p football-domain -p football-storage` zöld.
- [x] `cargo clippy -p football-domain -p football-storage --all-targets -- -D warnings` zöld.
- [x] A magyar nyelvű szövegek visszaolvasása megtörtént.

### C5 — Zárás és holnaputáni előkészítés

- [x] A nap eredménye, a futtatott parancsok és az eltérések beírása ide.
- [x] A nyitott kérdések listázása.
- [x] Csak a Task 4 (core, identitás, provenance) következő szelet jóváhagyása; kódolni még nem kell. (A felhasználó engedélyezte: „Rendben mehet tovább”; a commit/push kérdés nyitva maradt.)
- [x] Commit/push csak külön, explicit jóváhagyással.

## Megállási és visszagörgetési szabályok

- Ha bármely migráció vagy teszt piros, nem lépünk a következő checkpointra.
- Ha a tesztadatbázisban nem várt tábla vagy adat jelenik meg, megállunk és összehasonlítjuk a migrációval.
- A visszaállítási út: a három séma (`raw`, `stage`, `ops`) és a `public._sqlx_migrations` nyilvántartás együttes törlése, majd a migráció újrafuttatása; a tesztadatbázison a `scripts/db-reset.sh` ezt igazoltan elvégzi. Dev adatbázison csak külön jóváhagyással és előzetes ellenőrzéssel szabad futtatni.
- Nem nyúlunk a v1 adatbázisához.
- Nem töltünk be valódi forrásadatot.

## Nem része a holnapi napnak

- meccs-, csapat- és játékos-táblák;
- valódi adatimport;
- forrásadapter;
- verification engine;
- API és SwiftUI;
- commit vagy push automatikusan.

## Döntési pontok holnapra

- [x] A `0001` migráció jóváhagyva.
- [x] A nyolc tábla és a hat validációs szabály elfogadva.
- [x] A következő napra csak a core/identitás/provenance séma marad.

## Nap végi jegyzet

**Eredmény:** A `0001_raw_stage_ops.sql` migráció a terv szerint elkészült, és először a `football_v2_test`, majd zöld eredmény után a `football_v2_dev` adatbázison lefutott. Mindkét adatbázisban pontosan 8 terméktábla + `public._sqlx_migrations` (9 tábla), 6 `validation_rule` sor, minden üzleti adatszám 0. A migráció újrafuttatása nem duplikál; a `scripts/db-reset.sh` a tesztadatbázison igazoltan újraépíti a sémákat. A storage-kapcsolati teszt és a `db-check.sh` a migráció utáni állapotot ellenőrzi. Commit/push nem történt.

**Futtatott ellenőrzések:**
- `./scripts/check-foundation.sh` a nap elején (üres DB-k, 7 teszt, fmt/clippy zöld) és a nap végén (9 tábla / 6 szabály / 0 üzleti sor mindkét DB-n, 7 teszt zöld).
- `sqlx --version` → sqlx-cli 0.9.0 (telepítés: `cargo install sqlx-cli --no-default-features --features postgres,rustls`).
- `DATABASE_URL="$TEST_DATABASE_URL" sqlx migrate run --source db/migrations` → `Applied 1/migrate raw stage ops`; második futtatás: nincs új migráció, a szabályok száma 6 marad.
- `./scripts/db-reset.sh` → sémák + `public._sqlx_migrations` dobása, majd `Applied 1/migrate raw stage ops`; utána 9 tábla, 6 szabály.
- `sqlx migrate run --source db/migrations` a dev adatbázison → `Applied 1/migrate raw stage ops`.
- psql ellenőrző lekérdezések a checkpoint C2/C3 szakasza szerint (táblalista, szabályok, üzleti számlálók, `_sqlx_migrations` napló) mindkét adatbázison.
- `git check-ignore .env` → ignorált; `git ls-files` → titok nincs verziókövetésben.

**Eltérések / hibák:**
- A terv `db-reset.sh` scriptje nem törölte a `public._sqlx_migrations` táblát, így a reset után a migráció nem futott volna újra. A felhasználó jóváhagyásával a script egy sorral kiegészült: `DROP TABLE IF EXISTS public._sqlx_migrations;`.
- A `db-check.sh` és a storage-kapcsolati teszt az üres DB-t várta; a C4 előírása szerint a migráció utáni állapotra lettek igazítva (9 tábla, 6 szabály, 0 üzleti sor). A teszt neve `migrated_test_database_is_reachable` lett.
- A telepített sqlx-cli 0.9.0; a workspace sqlx crate 0.8 (terv szerint). A migrációs formátum kompatibilis, gyakorlatban igazolva.
- A migráció SQL-je a tervvel szó szerint megegyezik; a fejléckomment dokumentálja az újrafuttathatóságot és a visszaállítási utat.

**Nyitott kérdések:**
- A Task 4 (core, identitás, provenance) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A `db-check.sh` és a storage teszt számszerű elvárásait (9/6) a Task 4 migrációjakor majd igazítani kell.
- A commit/push engedélye külön döntés; a munkafa jelenleg nem commitolt változásokat tartalmaz.

**Következő apró szelet:** Task 4 — `0002_core_identity_verification.sql` (core, identitás, provenance és publikálási séma), csak jóváhagyás után.

---

## Második szelet — Task 4: core, identitás, provenance, publikálási séma

**Engedély:** a felhasználó a Task 3 zárása után engedélyezte a folytatást („Rendben mehet tovább”). Commit/push nem része a szeletnek.

**Terv:** [Task 4](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md) — `0002_core_identity_verification.sql`. A végrehajtás az AGENTS.md szerint tesztadatbázis-first: a terv Step 2–3 dev adatbázisra írt parancsait először a `football_v2_test`-en futtatjuk, a dev csak zöld ellenőrzés után kapja meg a migrációt.

### T4.1 — Migrációs fájl

- [x] `db/migrations/0002_core_identity_verification.sql` a terv SQL-jével.
- [x] 12 új tábla: `core.entity`, `core.country`, `core.competition`, `core.season`, `core.team`, `core.match`, `core.match_result`, `identity.source_entity_ref`, `fact.provenance`, `fact.conflict`, `fact.verification`, `ops.publish_gate`.
- [x] Tranzakciós; visszaállítási út dokumentálva.
- [x] Nincs üzleti adat, csak séma.

### T4.2 — Tesztadatbázis-first végrehajtás

- [x] Migráció a `football_v2_test`-en; összesen 21 tábla, ebből 11 a `core`/`identity`/`fact` sémákban.
- [x] A 6 `validation_rule` változatlan; minden üzleti sor 0.
- [x] Constraint-próba: a terv Step 3 szándékosan hibás INSERTje idegen kulcs hibával bukik, a `core.match` üres marad.
- [x] A `scripts/db-reset.sh` mindkét migrációt újraépíti; az állapot zöld.

### T4.3 — Fejlesztői adatbázis

- [x] Zöld tesztadatbázis után a migráció a `football_v2_dev`-en is lefut; ugyanaz az állapot.

### T4.4 — Ellenőrzések

- [x] `db-check.sh` és a storage-kapcsolati teszt igazítva (21 tábla / 6 szabály / 0 üzleti sor, kibővített számlálás).
- [x] `./scripts/check-foundation.sh` zöld (fmt, 7 teszt, clippy).
- [x] Magyar nyelvű szövegek visszaolvasása.

### T4.5 — Zárás

- [x] Eredmény, futtatott parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 5: objektumtár és tesztadatbázis-helper) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 4)

**Eredmény:** A `0002_core_identity_verification.sql` migráció a terv SQL-jével szó szerint megegyezően elkészült, és tesztadatbázis-first sorrendben futott le: először a `football_v2_test`-en (zöld ellenőrzés és constraint-próba után), majd a `football_v2_dev`-en. Mindkét adatbázisban 21 tábla (20 terméktábla + `_sqlx_migrations`), ebből 11 a `core`/`identity`/`fact` sémákban; a 6 `validation_rule` változatlan, minden üzleti sor 0. A `scripts/db-reset.sh` igazoltan újraépíti mindkét migrációt. Commit/push nem történt.

**Futtatott ellenőrzések:**
- `DATABASE_URL="$TEST_DATABASE_URL" sqlx migrate run --source db/migrations` → `Applied 2/migrate core identity verification`.
- psql: 21 tábla összesen, 11 a `core`/`identity`/`fact` sémákban, 12 új tábla listája, 6 szabály, `_sqlx_migrations` 1+2 sikeres.
- psql: mind a 12 új üzleti tábla számlálója 0.
- Constraint-próba a tesztadatbázison: a terv Step 3 hibás INSERTje `ERROR: ... violates foreign key constraint "match_competition_id_fkey"` üzenettel bukott; a `core.match` számlálója utána 0.
- `./scripts/db-reset.sh` → mind a hat séma + `_sqlx_migrations` dobása, majd `Applied 1` és `Applied 2`; utána 21/11/6, mindkét migrációs sor sikeres.
- `sqlx migrate run --source db/migrations` a dev adatbázison → `Applied 2`; 21/11/6, új üzleti sorok összege 0.
- `./scripts/check-foundation.sh` a szelet végén: mindkét DB 21 tábla / 6 szabály / 0 üzleti sor; 6 domain + 1 storage teszt zöld; `cargo fmt --all --check` és `cargo clippy --all-targets -- -D warnings` tiszta.

**Eltérések / hibák:**
- A terv Step 2–3 a dev adatbázist célozta; az AGENTS.md tesztadatbázis-first szabálya miatt a migráció és a constraint-próba először a `football_v2_test`-en futott, a dev csak zöld ellenőrzés után kapta meg. A constraint-próba így nem érintett dev adatot.
- A `db-check.sh` üzleti-sor számlálása a 12 új táblával bővült, elvárása 21 tábla lett; a storage teszt elvárása 21 tábla, üzenete a 0001+0002 migrációkra hivatkozik.
- A terv „Produces” listája nem említi a `fact.conflict` táblát, de a terv SQL-je létrehozza; a SQL az irányadó, a 11-es core/identity/fact táblaszám ezt igazolja.

**Nyitott kérdések:**
- A Task 5 (objektumtár és tesztadatbázis-helper) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz (0001+0002 migrációk, db-reset.sh, db-check.sh, storage teszt, checkpoint).
- A `db-check.sh` és a storage teszt számszerű elvárásait (21/6) a következő migrációnál ismét igazítani kell.

**Következő apró szelet:** Task 5 — objektumtár és tesztadatbázis-helper (`storage/object_store.rs`, `storage/test_support.rs`), csak jóváhagyás után.

---

## Harmadik szelet — Task 5: objektumtár és tesztadatbázis-helper

**Engedély:** a felhasználó a Task 4 zárása után engedélyezte („Mehet a Task 5”). Commit/push nem része a szeletnek.

**Terv:** [Task 5](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md). Adaptáció: a terv a storage crate létrehozását írja le, de a crate a Task 2 óta létezik; ezért bővítés történik, a `connect`/`ping`/`user_table_count` függvények és a connection teszt megmarad. A workspace minden szükséges függőséget tartalmaz, új dependency nem kell.

### T5.1 — Bukó teszt (TDD)

- [x] `rust/crates/storage/tests/object_store.rs` a terv szerint.
- [x] `cargo test -p football-storage` piros (a modul még nem létezik), a hiba rögzítve.

### T5.2 — Implementáció

- [x] `storage/Cargo.toml` a terv függőségeivel (workspace = true).
- [x] `src/object_store.rs`: SHA-256 alapú kulcs, zstd tömörítés, atomi írás, dedup.
- [x] `src/lib.rs`: `object_store`, `repositories`, `test_support` modulok a meglévő függvények mellett.
- [x] `src/repositories/`: mod.rs + raw/core/identity/verification/day placeholderek (a terv szerint üresen, Task 6–10 tölti ki).
- [x] `src/test_support.rs`: `test_pool()` migrációval, truncate + szabály-újratöltés.

### T5.3 — Ellenőrzések

- [x] Objektumtár-teszt zöld: azonos payload egyszer tárolódik, visszaolvasható, egy bejegyzés a raw könyvtárban.
- [x] Connection teszt zöld (21 tábla).
- [x] `cargo fmt --all --check` és `cargo clippy --all-targets -- -D warnings` zöld.
- [x] `./scripts/check-foundation.sh` zöld.
- [x] Magyar szövegek visszaolvasása.

### T5.4 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 6: fixture-forrás, nyers rögzítés, megfigyelés) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 5)

**Eredmény:** A storage crate bővült: `object_store.rs` (SHA-256 tartalomkulcs, zstd tömörítés, atomi tmp+rename írás, azonos payload egyszer tárolódik), `test_support.rs` (`test_pool()` fordítás idejű migrációkkal, truncate + szabály-újratöltés), `repositories/` placeholderek a Task 6–10 számára. A meglévő `connect`/`ping`/`user_table_count` függvények és a connection teszt megmaradt. TDD szerint haladtunk: a teszt előbb piros volt (hiányzó modul), majd az implementációval zöld. Új workspace-függőség nem kellett, a tervben hivatkozott crate-ek már deklarálva voltak. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-storage --test object_store` → `E0432 unresolved import football_storage::object_store` és `E0433 cannot find module or crate tempfile` (az elvárt bukás).
- `cargo fmt --all`, majd `cargo fmt --all --check` → tiszta.
- `cargo test -p football-storage` → `identical_payload_is_stored_once_and_round_trips` és `migrated_test_database_is_reachable` zöld (2 teszt).
- `cargo clippy -p football-domain -p football-storage --all-targets -- -D warnings` → tiszta.
- `./scripts/check-foundation.sh` → zöld (6 domain + 2 storage teszt, fmt, clippy).
- `./scripts/db-check.sh` → mindkét DB: 21 tábla / 6 szabály / 0 üzleti sor.

**Eltérések / hibák:**
- A terv a storage crate létrehozását írta le („a crate még nem létezik”); a valóságban a Task 2 óta létezik, ezért bővítés történt. A `lib.rs` a terv moddeklarációit a meglévő függvényekkel egyesíti; a piros lépés emiatt modulhiányt (nem crate-hiányt) mutatott.
- A `test_support::test_pool()` még nem hívódik meg tesztből (a Task 6–10 integrációs tesztjei fogják); a publikus API miatt nem dead code.
- A `test_pool()` a szabályokat rövid magyar leírásokkal tölti újra („Egy forrás”, „Kickoff” stb.); az első integrációs tesztfutás után a tesztadatbázis `validation_rule` leírásai eltérhetnek a migráció teljes leírásaitól. A `db-check.sh` csak a darabszámot (6) ellenőrzi, így zöld marad; a dev adatbázist a helper nem érinti.

**Nyitott kérdések:**
- A Task 6 (fixture-forrás, nyers rögzítés, megfigyelés) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz.
- A `test_pool()` a közös tesztadatbázist truncatálja; a Task 6-nál figyelni kell, hogy a párhuzamosan futó integrációs tesztek ne zavarják egymást (a terv ezt hogyan kezeli, ott derül ki).

**Következő apró szelet:** Task 6 — fixture-forrás, nyers rögzítés és megfigyelés (`ingest` crate, `tests/fixtures/`, raw repository), csak jóváhagyás után.

---

## Negyedik szelet — Task 6: fixture-forrás, nyers rögzítés és megfigyelés

**Engedély:** a felhasználó a Task 5 zárása után engedélyezte („Mehet a Task 6”). Ez a szelet szintetikus fixture-adatot ír a TESZTadatbázisba (a terv engedélyezett demóforrása); a dev adatbázis nem érintett. Commit/push nem része.

**Terv:** [Task 6](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T6.1 — Fixture és bukó teszt (TDD)

- [x] `tests/fixtures/fixture_a_day.json` a terv szerint.
- [x] `rust/crates/ingest/tests/capture_observation.rs` a terv szerint.
- [x] Piros futtatás: a crate/adapter hiánya miatt bukik, rögzítve.

### T6.2 — Implementáció

- [x] `ingest/Cargo.toml` a terv függőségeivel.
- [x] `src/lib.rs`: `fixture` + `canonicalize` modulok.
- [x] `src/fixture.rs`: FixtureSource modell, `FixtureAdapter::ingest`, upsert_source, insert_observation (idempotens ON CONFLICT mintával).
- [x] `src/canonicalize.rs`: üres placeholder (Task 7 tölti ki).

### T6.3 — Zöld tesztek

- [x] Capture-teszt zöld: 1 artifact, 1 capture, 3 observation (2 team_identity + 1 match_core).
- [x] psql evidence a tesztadatbázison: fixture_a forrás, helyes entity_kind/aspect megoszlás.
- [x] `cargo test --workspace` zöld.

### T6.4 — Reset és teljes ellenőrzés

- [x] Tesztadatbázis visszaállítása `db-reset.sh`-val (21/6/0), dev érintetlen.
- [x] `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` zöld.
- [x] `./scripts/check-foundation.sh` zöld.
- [x] Magyar szövegek visszaolvasása.

### T6.5 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 7: kanonikus entitások, identitás, provenance) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 6)

**Eredmény:** Elkészült az `ingest` crate a `FixtureAdapter`rel: a fixture JSON-t nyers bizonyítékként tárolja (ObjectStore, zstd), `raw.artifact`/`raw.capture`/`ops.source` rekordokat ír, és normalizált `stage.observation` sorokat hoz létre (2 team_identity + 1 match_core). Az írás idempotens (ON CONFLICT minták). A capture-teszt TDD szerint pirosból zöld lett; a teljes workspace 9 tesztje zöld. A fixture-adat kizárólag a tesztadatbázisba került, a szelet végén a `db-reset.sh` visszaállította a nyugalmi állapotot (21/6/0); a dev adatbázis érintetlen. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-ingest --test capture_observation` → `failed to read .../ingest/Cargo.toml` (a crate még nem létezett).
- `cargo test -p football-ingest --test capture_observation` → `fixture_is_stored_raw_and_observed_once` zöld (1 artifact, 1 capture, 3 observation).
- psql a tesztadatbázison: `ops.source` = fixture_a/Fixture A; `stage.observation` = match/match_core/A-1001 + team/team_identity/A-GYOR és A-UJP, schema_version 1.0; `raw.artifact` = zstd/1384 byte/application/json; `raw.capture` = day-fixtures/200/`fixture_a:day-fixtures:2026-11-09 08:00:00 UTC`; 6 szabály.
- `cargo test --workspace` → 6 domain + 1 ingest + 2 storage teszt zöld.
- `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` → tiszta.
- `./scripts/db-reset.sh` → Applied 1 + Applied 2; `./scripts/check-foundation.sh` → mindkét DB 21/6/0, tesztek zöldek.

**Eltérések / hibák:**
- A terv `fixture.rs` kódja két ponton nem fordult: (1) hiányzott a `use sha2::Digest;` import (a `Sha256::digest` trait nélkül nem érhető el); (2) a `content_hash` String az első `.bind()` után move-olódott, a második lekérdezésben már nem volt használható. Javítás: `use sha2::Digest;` + `.bind(&content_hash)` az INSERT-ben. Mindkettő a fordító által javasolt minimális javítás, a terv logikája változatlan.
- A `canonicalize.rs` üres placeholderként készült (a terv lib.rs-je deklarálja, a Task 7 tölti ki).
- A terv Task 6 Files-listája `Modify: storage/repositories/raw.rs`-t ír, de egyetlen step sem ad tartalmat hozzá; a raw repository üres maradt (a Task 7 módosítja). Az SQL ebben a szeletben az ingest adapterben él — a terv kifejezett kódja szerint; a kanonikus entitásokat az adapter nem írja.
- Új munkafolyamat-elem: az integrációs tesztek fixture-sorokat hagynak a tesztadatbázisban, ezért a tesztfuttatás után `db-reset.sh` állítja vissza a nyugalmi állapotot, és csak ezután fut a `check-foundation.sh`. A dev adatbázist tesztek nem érintik.

**Nyitott kérdések:**
- A Task 7 (kanonikus entitások, identitás, provenance) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz.
- A `test_pool()` párhuzamos integrációs tesztek közötti viselkedése: jelenleg minden tesztbináris egy tesztet tartalmaz és a cargo szekvenciálisan futtatja a binárisokat, így nincs ütközés; a Task 7 második tesztjénél ezt újra kell ellenőrizni.
- Az újrafeldolgozás idempotenciáját a constraint-ek szerkezetileg garantálják (ON CONFLICT DO NOTHING/UPDATE); külön teszt még nem gyakorolja, a későbbi kétforrásos fixture-folyam (Task 13) igen.

**Következő apró szelet:** Task 7 — kanonikus entitások, identitás és provenance (`ingest/canonicalize.rs`, storage core/identity/raw repositoryk), csak jóváhagyás után.

---

## Ötödik szelet — Task 7: kanonikus entitások, identitás és provenance

**Engedély:** a felhasználó a Task 6 zárása után engedélyezte („Mehet a Task 7”). Ez a szelet a szintetikus fixture kanonikus entitásait írja a TESZTadatbázisba (a terv engedélyezett demóforrása); a dev adatbázis nem érintett. Commit/push nem része.

**Terv:** [Task 7](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T7.1 — Bukó kanonizálási teszt (TDD)

- [x] `rust/crates/ingest/tests/canonicalize_fixture.rs` a terv szerint.
- [x] Piros futtatás: a `Canonicalizer` hiánya miatt bukik, rögzítve.

### T7.2 — Implementáció

- [x] `ingest/src/canonicalize.rs`: `Canonicalizer::materialize`, `MaterializedDay`/`MaterializedMatch`, ország/entitás/verseny/szezon/csapat upsert, identitáshivatkozások, eredmény és provenance.

### T7.3 — Zöld tesztek és evidence

- [x] Kanonizálási teszt zöld: 1 kanonikus meccs, 2 aktív identitáshivatkozás, legalább 5 provenance sor.
- [x] psql evidence a tesztadatbázison.
- [x] `cargo test --workspace` zöld; a két ingest tesztbináris párhuzamossági aggályát újra ellenőrizve.

### T7.4 — Reset és teljes ellenőrzés

- [x] Tesztadatbázis visszaállítása `db-reset.sh`-val (21/6/0), dev érintetlen.
- [x] `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` zöld.
- [x] `./scripts/check-foundation.sh` zöld.
- [x] Magyar szövegek visszaolvasása.

### T7.5 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 8: ellenőrzési motor és determinisztikus szabályok) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 7)

**Eredmény:** Elkészült a `Canonicalizer::materialize`: egyetlen tranzakcióban kanonikus entitásokat ír a fixture-ból és a `stage.observation` sorokból — ország, entitás, verseny, szezon, csapatok, `identity.source_entity_ref` (fixture_seed, active), `core.match`, eredmény (null esetén nincs sor), valamint 5 mezőnyi `fact.provenance` (mind selected, `fixture-1.0` policy). Visszatérési értéke `MaterializedDay`, domain `MatchSnapshot`tel. A kanonizálási teszt TDD szerint pirosból zöld lett; a teljes workspace 10 tesztje zöld. A fixture-adat kizárólag a tesztadatbázisba került, a szelet végén a `db-reset.sh` visszaállította a nyugalmi állapotot (21/6/0); a dev adatbázis érintetlen. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-ingest --test canonicalize_fixture` → `E0432: no Canonicalizer in canonicalize` (az elvárt bukás).
- `cargo test -p football-ingest --test canonicalize_fixture` → `fixture_materializes_one_canonical_match_with_exact_identity_and_provenance` zöld (1 meccs, pontos csapat-UUID-k, 2 ref, >= 5 provenance).
- psql a tesztadatbázison: entity 3 / country 1 / competition 1 / season 1 / team 2 / match 1 / match_result 0 / source_entity_ref 2 / provenance 5. Meccs: scheduled, kickoff 2026-11-09 18:00 UTC (psql 19:00+01-ként jeleníti meg), Europe/Budapest, Újpest–Győr. Refek: A-UJP → …011, A-GYOR → …012, fixture_seed/active. Provenance: home_team_id, away_team_id, kickoff_at, status, result — mind is_selected=t, fixture-1.0.
- `cargo test --workspace` → 6 domain + 2 ingest + 2 storage teszt zöld; a két ingest tesztbináris szekvenciálisan futott, truncation-ütközés nélkül.
- `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` → tiszta.
- `./scripts/db-reset.sh` (.env betöltéssel) → Applied 1 + Applied 2; `./scripts/check-foundation.sh` → mindkét DB 21/6/0, tesztek zöldek.
- Magyar szövegek visszaolvasva: `canonicalize.rs` hibaüzenetek („ismeretlen csapatforrás”, „ismeretlen meccsstátusz”) és a checkpoint szekció.

**Eltérések / hibák:**
- A terv `use chrono::{DateTime, Utc};` importjában a `DateTime` nem használt a kanonizáló kódjában; a `-D warnings` clippy miatt `use chrono::Utc;`-re csökkentve. A terv logikája változatlan.
- A terv Task 7 Files-listája a `storage/repositories/core.rs`, `identity.rs`, `raw.rs` módosítását írja, de egyetlen step sem ad tartalmat hozzájuk; a kanonizáló a terv kifejezett kódja szerint inline SQL-t használ. A repository fájlok üres placeholderek maradtak (a későbbi feladatok töltik ki őket).
- A `db-reset.sh` a környezetből várja a `TEST_DATABASE_URL`-t; .env nélkül futtatva magyar hibaüzenettel megállt („TEST_DATABASE_URL hiányzik”), .env betöltéssel zöld. Ez workflow-elem, nem kódhiba.

**Nyitott kérdések:**
- A Task 8 (ellenőrzési motor és determinisztikus szabályok, új `verification` crate) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz.
- A `test_pool()` párhuzamossága újra ellenőrizve: binárisonként egy teszt, szekvenciális futás, nincs ütközés. Ha egy binárisba több DB-s teszt kerül, a közös truncate miatt újra kell vizsgálni.
- Az újramaterializálás idempotenciáját az ON CONFLICT minták szerkezetileg garantálják; külön teszt még nem gyakorolja (a Task 13 kétforrásos fixture-folyama fogja).
- A provenance-kiválasztás jelenleg egyforrású: a legfrissebb `match_core` megfigyelés mind az 5 mezőre selected. Többforrású konfliktuskezelés a későbbi szeletekben jön.

**Következő apró szelet:** Task 8 — ellenőrzési motor és determinisztikus szabályok (`verification` crate), csak jóváhagyás után.

---

## Hatodik szelet — Task 8: ellenőrzési motor és determinisztikus szabályok

**Engedély:** a felhasználó a Task 7 zárása után engedélyezte („Mehet a Task 8”). Ez a szelet tiszta domain logikát ad hozzá (új `verification` crate); adatbázist közvetlenül nem ír, valódi fixture-adatot nem tölt be. Commit/push nem része.

**Terv:** [Task 8](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T8.1 — Bukó szabálytesztek (TDD)

- [x] `rust/crates/verification/tests/rules.rs` a terv szerint (3 teszt).
- [x] Piros futtatás: a crate hiánya miatt bukik, rögzítve.

### T8.2 — Implementáció

- [x] `verification/Cargo.toml` a terv függőségeivel.
- [x] `src/lib.rs`, `src/engine.rs`, `src/rules/mod.rs` a terv szerint.
- [x] `src/gate.rs`, `src/persist.rs`: üres placeholder (a Task 9 tölti ki).

### T8.3 — Zöld tesztek és teljes ellenőrzés

- [x] Szabálytesztek zöldek (3 teszt).
- [x] `cargo test --workspace` zöld (13 teszt).
- [x] Tesztadatbázis visszaállítása `db-reset.sh`-val (21/6/0) — az ingest integrációs tesztek fixture-sorai miatt; dev érintetlen.
- [x] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `./scripts/check-foundation.sh` zöld.
- [x] Magyar szövegek visszaolvasása.

### T8.4 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 9: konfliktusforrás, leletek és publikálási kapu) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 8)

**Eredmény:** Elkészült a `verification` crate: a `VerificationEngine::run` determinisztikus szabályokat futtat a `VerificationContext`en, és domain `VerificationReport`ot ad. Hat szabály: EXACT_IDENTITY, DUPLICATE_NATURAL_KEY, KICKOFF_VALID, MATCH_RESULT_CONSISTENT (domain logika), MATERIAL_SOURCE_CONFLICT és SINGLE_SOURCE (warning); a `default_rules()` adja a boxed szabálylistát. A három szabályteszt TDD szerint pirosból zöld lett; a teljes workspace 13 tesztje zöld. A szelet közvetlenül nem ír adatbázist; az ingest integrációs tesztek fixture-sorait a szelet végén `db-reset.sh` állította vissza (21/6/0). A dev adatbázis érintetlen. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-verification --test rules` → `failed to read .../verification/Cargo.toml` (a crate még nem létezett).
- Zöld: `cargo test -p football-verification --test rules` → `valid_match_has_no_blockers`, `unresolved_team_identity_is_blocker`, `duplicate_natural_key_is_blocker` ok.
- `cargo test --workspace` → 6 domain + 2 ingest + 2 storage + 3 verification = 13 teszt zöld.
- `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` → tiszta.
- `./scripts/db-reset.sh` → Applied 1 + Applied 2; `./scripts/check-foundation.sh` → mindkét DB 21/6/0, tesztek zöldek.
- Magyar szabályüzenetek visszaolvasva (7 szöveg, ékezetek hibátlanok).

**Eltérések / hibák:**
- A terv lib.rs-je deklarálja a `gate` és `persist` modulokat, de a Task 8 Files-listája nem sorolja fel őket és tartalmat sem ad; üres placeholder fájlokat hoztam létre, nélkülük a crate nem fordul. A Task 9 tölti ki őket.
- A terv Cargo.toml függőségei (serde, serde_json, sqlx) és dev-függőségei (football-ingest, football-storage, tempfile, tokio) ebben a szeletben még nem használtak; a terv kifejezetten kéri őket (a Task 9–10 használja). Fordítási figyelmeztetést nem okoznak.
- Egyéb eltérés nincs: a terv kódja módosítás nélkül fordult és futott.

**Nyitott kérdések:**
- A Task 9 (konfliktusforrás, leletek és publikálási kapu — fixture_b/fixture_c, `persist.rs`, `gate.rs`) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz.
- A terv megjegyzése szerint a fixture egyetlen forrása miatt a teljes napi tesztek `provisional_publish` állapotot várhatnak (SINGLE_SOURCE warning); ez tudatos, nem hiba. A Task 9–10 fogja gyakorolni.
- A KICKOFF_VALID, MATERIAL_SOURCE_CONFLICT és SINGLE_SOURCE szabályok ágai közvetlen teszttel még nem fedettek; a valid esetet ellenőrző teszt és a Task 9 konfliktusos fixture-jei gyakorolják őket.

**Következő apró szelet:** Task 9 — konfliktusforrás, leletek és publikálási kapu (fixture_b/fixture_c, verification persist/gate), csak jóváhagyás után.

---

## Hetedik szelet — Task 9: konfliktusforrás, leletek és publikálási kapu

**Engedély:** a felhasználó a Task 8 zárása után engedélyezte („Mehet a Task 9”). Ez a szelet a szintetikus fixture-ökhöz tartozó publikálási kaput és lelet-perzisztálást írja a TESZTadatbázisba (a terv engedélyezett demóforrása); a dev adatbázis nem érintett. Commit/push nem része.

**Terv:** [Task 9](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T9.1 — Fixture-ök és bukó gate-teszt (TDD)

- [x] `tests/fixtures/fixture_b_confirm.json` és `fixture_c_conflict.json` a terv szerint.
- [x] `rust/crates/verification/tests/publish_gate.rs` a terv szerint.
- [x] Piros futtatás: a gate-függvény hiánya miatt bukik, rögzítve.

### T9.2 — Implementáció

- [x] `verification/src/gate.rs`: `local_publish_date`, `evaluate_and_persist_gate`.
- [x] `verification/src/persist.rs`: `persist_report` (validation_run, validation_finding, fact.conflict, fact.verification).
- [x] `verification/Cargo.toml`: anyhow + chrono-tz workspace függőségek (a terv kódja importálja őket).

### T9.3 — Zöld gate-teszt és evidence

- [x] Gate-teszt zöld: withheld gate a valós fixture-meccshez.
- [x] psql evidence a tesztadatbázison (ops.publish_gate).
- [x] `cargo test --workspace` zöld (14 teszt).

### T9.4 — Reset és teljes ellenőrzés

- [x] Tesztadatbázis visszaállítása `db-reset.sh`-val (21/6/0), dev érintetlen.
- [x] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `./scripts/check-foundation.sh` zöld.
- [x] Magyar szövegek visszaolvasása.

### T9.5 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 10: napi lista lekérdezése) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 9)

**Eredmény:** Elkészült a verification crate perzisztáló és kapu rétege: a `persist_report` egyetlen tranzakcióban ír `ops.validation_run`, `ops.validation_finding`, `fact.conflict` és `fact.verification` sorokat; az `evaluate_and_persist_gate` a reportból `PublishState`-et képez és upserteli az `ops.publish_gate` sort; a `local_publish_date` UTC kickoffból és időzónából helyi dátumot számol (chrono-tz). Elkészült a `fixture_b` (megerősítő) és `fixture_c` (konfliktusos, eltérő kickoff) fixture. A gate-teszt TDD szerint pirosból zöld lett: a blocker report `withheld` kaput hoz létre a valós fixture-meccshez. A teljes workspace 14 tesztje zöld. A fixture-adat kizárólag a tesztadatbázisba került, a szelet végén `db-reset.sh` visszaállította a nyugalmi állapotot (21/6/0); a dev adatbázis érintetlen. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-verification --test publish_gate` → `E0432: no evaluate_and_persist_gate in gate` (az elvárt bukás).
- Zöld: `blocker_report_creates_withheld_gate` ok.
- psql a tesztadatbázison: `ops.publish_gate` 1 sor — publish_date 2026-11-09, state withheld, rule_set_version verification-1.0, blockers jsonb: MATERIAL_SOURCE_CONFLICT / blocker / „Források eltérnek.”.
- `cargo test --workspace` → 6 domain + 2 ingest + 2 storage + 4 verification = 14 teszt zöld; a tesztbinárisok szekvenciálisan futottak, truncation-ütközés nélkül.
- `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` → tiszta.
- `./scripts/db-reset.sh` → Applied 1 + Applied 2; `./scripts/check-foundation.sh` → mindkét DB 21/6/0, tesztek zöldek.
- Magyar szövegek visszaolvasva: fixture csapatnevek (Újpest, Győr), a teszt üzenete („Források eltérnek.”) és a checkpoint szekció.

**Eltérések / hibák:**
- A terv Task 9 kódja (`gate.rs`, `persist.rs`) `anyhow`-t és `chrono_tz`-t importál, de a Task 8-as Cargo.toml nem tartalmazta őket. Javítás: `anyhow.workspace = true` és `chrono-tz.workspace = true` hozzáadva — mindkettő már szerepelt a workspace manifestben (anyhow 1, chrono-tz 0.10), új külső függőség nem került a rendszerbe. A lockfile a Rust 1.85-tel kompatibilis chrono-tz 0.10.4-et (+ phf, phf_shared, siphasher tranzitív) választotta.
- A `fixture_c` a terv szó szerinti tartalma szerint B-* forrásentitás-ID-kat használ (B-UJP/B-GYOR/B-1001) önálló forrás létére; ez funkcionálisan helyes, mert az identitás-refek forrásonként egyediek (UNIQUE source_id + entity_kind + source_entity_id). Teszt még nem használja (a Task 13 fogja).
- A `persist_report` és a `local_publish_date` publikus API, de teszt ebben a szeletben még nem hívja őket (a gate-teszt csak az `evaluate_and_persist_gate`-et gyakorolja); dead code figyelmeztetés nincs. A Task 11/13 fogja gyakorolni.

**Nyitott kérdések:**
- A Task 10 (napi lista lekérdezése — `storage/repositories/day.rs`, DayFilter/DayMatchRow/fetch_day) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz.
- A `fixture_b`/`fixture_c` fixture-öket még nem gyakorolja teszt; a Task 13 kétforrásos folyamata fogja.
- A `test_pool()` párhuzamossága továbbra is biztonságos: binárisonként legfeljebb egy DB-s teszt, szekvenciális futás.

**Következő apró szelet:** Task 10 — napi lista lekérdezése (`storage/repositories/day.rs`), csak jóváhagyás után.

---

## Nyolcadik szelet — Task 10: napi lista lekérdezése

**Engedély:** a felhasználó a Task 9 zárása után engedélyezte („Mehet a Task 10”). Ez a szelet a napi lista lekérdezését írja meg a storage crate-ben, és szintetikus seed adatokkal teszteli a TESZTadatbázisban; a dev adatbázis nem érintett. Commit/push nem része.

**Terv:** [Task 10](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T10.1 — Bukó napi lekérdezés teszt (TDD)

- [x] `rust/crates/storage/tests/day_query.rs` a terv szerint.
- [x] Piros futtatás: a `fetch_day` hiánya miatt bukik, rögzítve.

### T10.2 — Implementáció

- [x] `storage/src/repositories/day.rs`: `DayFilter`, `DayMatchRow`, `fetch_day`.

### T10.3 — Zöld tesztek és evidence

- [x] Napi teszt zöld: alapértelmezésben 1 sor (verified_publish), `include_all=true` esetén 2 sor.
- [x] psql evidence a tesztadatbázison.
- [x] `cargo test --workspace` zöld (15 teszt).

### T10.4 — Reset és teljes ellenőrzés

- [x] Tesztadatbázis visszaállítása `db-reset.sh`-val (21/6/0), dev érintetlen.
- [x] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `./scripts/check-foundation.sh` zöld.
- [x] Magyar szövegek visszaolvasása.

### T10.5 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 11: helyi API tokenekkel) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 10)

**Eredmény:** Elkészült a napi lista lekérdezése: a `fetch_day` a megadott helyi dátumra (kickoff `AT TIME ZONE`) adja vissza a meccseket csapatnevekkel, eredménnyel, publikálási állapottal és blockers JSON-nal; alapértelmezésben csak `verified_publish` sorokat, `include_all=true` esetén minden kapuzott meccset. A `DayMatchRow` FromRow + Serialize (API-kész). A napi teszt TDD szerint pirosból zöld lett: alapértelmezésben 1 sor, include_all esetén 2 sor. A teljes workspace 15 tesztje zöld. A seed adat kizárólag a tesztadatbázisba került; a nyugalmi állapotot (21/6/0) a szelet végén `db-reset.sh` állította vissza. A dev adatbázis érintetlen. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-storage --test day_query` → `E0432: no DayFilter / fetch_day in repositories::day` (az elvárt bukás).
- Zöld: `default_day_contains_only_verified_publish_matches` ok (1 sor, majd include_all-lal 2 sor).
- psql a tesztadatbázison: 2 seed meccs (Újpest–Győr, 18:00 UTC verified_publish és 19:00 UTC provisional_publish, Europe/Budapest); az alapértelmezett szűrés count = 1.
- `cargo test --workspace` → 6 domain + 2 ingest + 3 storage + 4 verification = 15 teszt zöld; szekvenciális binárisfutás, nincs truncation-ütközés.
- `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` → tiszta.
- `./scripts/db-reset.sh` → Applied 1 + Applied 2; `./scripts/check-foundation.sh` → zöld.
- Új ellenőrzés: check-foundation után `db-check.sh` → HIBA (tesztDB 12 üzleti sor, a day_query seed-je); `db-reset.sh` után → mindkét DB 21/6/0 OK.
- Magyar szövegek visszaolvasva: csapatnevek a tesztben (Újpest, Győr) és a checkpoint szekció.

**Eltérések / hibák:**
- A terv kódja módosítás nélkül fordult és futott; eltérés nincs.
- Új workflow-következmény: a `check-foundation.sh` a storage teszteket is futtatja, és az új `day_query` teszt a `test_pool()` révén seed sorokat ír a tesztadatbázisba (12 üzleti sor). Emiatt a check-foundation után a `db-check.sh` mindaddig HIBA, míg a `db-reset.sh` nem fut. Ebben a szeletben a reset a check-foundation után futott; a scriptek maguk nem módosultak.

**Nyitott kérdések:**
- A Task 11 (helyi API tokenekkel — `api` crate, `footballd` binary) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz.
- Döntési pont: a `check-foundation.sh` automatikusan resetelje-e a tesztadatbázist a tesztek után (script-viselkedés változás), vagy maradjon a „reset a check után” workflow? A felhasználó döntése; ebben a szeletben nem módosult a script.
- A `test_pool()` párhuzamossága továbbra is biztonságos: binárisonként legfeljebb egy DB-s teszt, szekvenciális futás.

**Következő apró szelet:** Task 11 — helyi API tokenekkel (`api` crate + `footballd` binary), csak jóváhagyás után.

---

## Kilencedik szelet — Task 11: helyi API tokenekkel

**Engedély:** a felhasználó a Task 10 zárása után engedélyezte („Mehet a Task 11”). Ez a szelet az `api` crate-et és a `footballd` daemont írja meg; a route-tesztek a TESZTadatbázison futnak (`test_pool`, üzleti seed nélkül). A daemon éles indítása nem része a szeletnek (a Task 12 végponttól végpontig ellenőrzése gyakorolja). Commit/push nem része.

**Terv:** [Task 11](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T11.1 — Bukó route-tesztek (TDD)

- [x] `rust/crates/api/tests/routes.rs` a terv szerint (`health_is_public`, `day_requires_read_token`).
- [x] Piros futtatás: az api crate hiánya miatt bukik, rögzítve.

### T11.2 — API crate implementálása

- [x] `api/Cargo.toml`, `src/lib.rs` (`AppState`, `router`), `src/auth.rs`, `src/error.rs`.
- [x] `src/routes/{mod,health,day,verification}.rs` a terv szerint.

### T11.3 — Daemon és runtime tokenfájl

- [x] `bins/footballd/Cargo.toml`, `src/main.rs`, `src/runtime_file.rs` a terv szerint.
- [x] Workspace members: `bins/*` hozzáadva (a terv nem említi, de a footballd csak így lehet workspace-tag).

### T11.4 — Zöld tesztek és teljes ellenőrzés

- [x] Route-tesztek zöldek: health 200, token nélkül 401, érvényes read tokennel 200.
- [x] Ismételt futtatás: a két DB-s teszt egy binárisban, párhuzamosan is stabil.
- [x] `cargo test --workspace` zöld.
- [x] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` zöld.
- [x] psql evidence a tesztadatbázison; reset + check-foundation + reset (21/6/0), dev érintetlen.
- [x] Magyar szövegek visszaolvasása.

### T11.5 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 12: footballctl CLI és végponttól végpontig ellenőrzés) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 11)

**Eredmény:** Elkészült az `api` crate (`football-api`): `AppState` (pool, read_token, ops_token) és `router` három végponttal — `GET /v1/health` (publikus), `GET /v1/day/{date}` (read vagy ops tokennel, `include=all` opcióval), `GET /v1/ops/verification/gates?date={date}` (ops tokennel). Az `authorize` a Bearer tokent ellenőrzi scope szerint (Read: read vagy ops token; Ops: csak ops token); az `AppError` tipizált JSON hibaválaszt ad magyar üzenetekkel (UNAUTHORIZED, BAD_REQUEST, INTERNAL). Elkészült a `footballd` daemon: `DATABASE_URL` alapján pool (8 kapcsolat), migrációk futtatása, alapértelmezett `127.0.0.1:0` bind (`BIND_ADDR`-ral felülírható), és runtime tokenfájl írása a repón kívülre (`~/Library/Application Support/FootballAppV2/runtime.json`, 0600 jogosultság, atomi rename, 64 karakteres uuid-v4 alapú tokenek). A route-tesztek TDD szerint pirosból zöldek lettek: health 200, token nélkül 401, érvényes read tokennel 200. A teljes workspace 17 tesztje zöld. A tesztek kizárólag a tesztadatbázist érintették, üzleti seed nélkül; a nyugalmi állapotot (21/6/0) a szelet végén `db-reset.sh` állította vissza, mindkét DB-re igazolva. A dev adatbázis érintetlen. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-api --test routes` → `failed to load manifest for workspace member … crates/api` (a crate hiányzik — az elvárt bukás, exit 101).
- Zöld: `health_is_public` és `day_requires_read_token` ok; összesen három futás (első + két ismétlés) → a két DB-s teszt egy binárisban, párhuzamosan is stabil.
- psql a tesztadatbázison a route-tesztek után: `core.match` = 0, `ops.publish_gate` = 0, `ops.validation_rule` = 6 (a `test_pool` truncate-olt, a tesztek nem seedelnek üzleti adatot).
- `cargo test --workspace` → 2 api + 6 domain + 2 ingest + 3 storage + 4 verification = 17 teszt zöld; a `footballd` bináris fordul (0 teszt).
- `cargo fmt --all --check` és `cargo clippy --workspace --all-targets -- -D warnings` → tiszta, a footballd-t is beleértve.
- `./scripts/db-reset.sh` → Applied 1 + Applied 2; `./scripts/check-foundation.sh` → zöld (db-check OK, fmt, 9 domain+storage teszt, clippy); utána ismét `db-reset.sh`, majd `./scripts/db-check.sh` → mindkét DB 21/6/0 OK.
- Magyar szövegek visszaolvasva Write után és fmt után is: hét string az api/footballd fájlokban, az ékezetek épek, idegen glyph nincs.

**Eltérések / hibák:**
- Workspace members: a `bins/*` hozzáadva a `crates/*` mellé — a terv nem említi, de a footballd (`version.workspace = true` stb.) csak így lehet workspace-tag.
- A terv kódja érdemi módosítás nélkül fordult és futott; a `cargo fmt` a szokásos formázást elvégezte (hosszú sorok tördelése).
- Új függőségek: 38 csomag került a lockfile-ba; a resolver 3 mindet Rust 1.85-kompatibilis verzióra választotta (axum 0.8.9, tower 0.5.3, hyper 1.12.0, dirs 6.0.0, rand 0.9.5, tracing-subscriber 0.3.23, http-body-util 0.1.5; a wasip2-ből 1.0.1, mert az 1.0.4 Rust 1.87-et igényel).
- A `rand` a footballd manifestben a terv szerint szerepel, de a kód nem használja (a token uuid v4-ből készül); figyelmeztetés nincs, a terv módosítása nélkül maradt.
- A „binárisonként legfeljebb egy DB-s teszt” invariantum bővül: a `routes.rs` két DB-s tesztje egy binárisban fut. A státusz-szintű assertek (üres nap = 200) és az idempotens `test_pool` (TRUNCATE + ON CONFLICT DO NOTHING) miatt párhuzamosan is biztonságos; három futás igazolja.

**Nyitott kérdések / nem ellenőrzött részek:**
- A daemon éles indítása, a `runtime.json` tényleges írása és a gates (ops) végpont hívása nem történt meg ebben a szeletben — a Task 12 végponttól végpontig ellenőrzése gyakorolja; a footballd fordul és clippy-tiszta.
- A Task 12 (`footballctl` CLI + végponttól végpontig ellenőrzés) szelet jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push engedélye továbbra is külön döntés; a munkafa nem commitolt változásokat tartalmaz.
- A check-foundation automatikus reset kérdése (Task 10) nyitott; a scriptek nem módosultak.
- A `persist_report` / `local_publish_date` (Task 9) és a `fixture_b` / `fixture_c` (Task 13) gyakorlása továbbra is várat magára.

**Következő apró szelet:** Task 12 — `footballctl` CLI és végponttól végpontig ellenőrzés, csak jóváhagyás után.

---

## Tizedik szelet — Task 12: footballctl CLI és végponttól végpontig ellenőrzés

**Engedély:** a felhasználó a Task 11 zárása után engedélyezte („Mehet a Task 12”). A terv Step 3-as ellentmondását (dev fixture-betöltés vs. a db-check.sh 0 üzleti sor elvárása) a felhasználó explicit döntése oldotta fel: a `db-check.sh` a dev adatbázisnál szerkezeti ellenőrzést végez (21 tábla / 6 szabály, üzleti sor megengedett), a tesztadatbázisnál szigorú marad (21/6/0); a pipeline a terv szerint a dev DB-n fut, és a fixture demóadat ott marad. Commit/push nem része.

**Terv:** [Task 12](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T12.1 — footballctl CLI

- [x] Piros evidence: a footballctl csomag hiánya miatt a CLI nem fut.
- [x] `bins/footballctl/Cargo.toml` és `src/main.rs` a terv szerint (migrate, ingest-fixture, day, doctor).
- [x] Fordul és clippy-tiszta.

### T12.2 — Ellenőrző scriptek

- [x] `scripts/check.sh` a terv funkciójával (reset + fmt + clippy + szekvenciális tesztek), jelszó nélküli env-kezeléssel.
- [x] `scripts/db-check.sh` igazítása a felhasználó döntése szerint (dev: szerkezet; teszt: szigorú 21/6/0).

### T12.3 — Teljes pipeline a dev adatbázison

- [x] migrate + fixture_a + fixture_b betöltés a dev DB-re (DATA_DIR a gyökér data/ könyvtárában).
- [x] Elvárt: verified_publish, alapértelmezett lista 1 meccs, --include-all ugyanaz.
- [x] psql evidence a dev DB-n; doctor smoke.

### T12.4 — Helyi fejlesztési útmutató

- [x] `docs/operations/local-development.md` (adaptált: létező scriptek, .env alapú konvenció, valós git-állapot).
- [x] Magyar szöveg visszaolvasása.

### T12.5 — Teljes ellenőrzés

- [x] `./scripts/check.sh` zöld: fmt, clippy, `cargo test --workspace -- --test-threads=1` (17 teszt).
- [x] check-foundation + reset lánc; db-check: dev szerkezet OK (fixture adatokkal), teszt 21/6/0.
- [x] Magyar szövegek visszaolvasása.

### T12.6 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt.
- [ ] Következő szelet (Task 13: kétforrásos konfliktus végponttól végpontig) csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 12)

**Eredmény:** Elkészült a `footballctl` CLI négy paranccsal: `migrate`, `ingest-fixture --path`, `day --date [--include-all]`, `doctor`. Az ingest-fixture a teljes pipeline-t futtatja: FixtureAdapter capture → Canonicalizer materializálás → verification engine (6 szabály) → persist_report → local_publish_date → gate. Elkészült a `scripts/check.sh` (tesztDB reset + fmt + clippy + szekvenciális workspace-tesztek) és a `docs/operations/local-development.md` útmutató. A pipeline a terv szerint a DEV adatbázison futott: fixture_a + fixture_b betöltés után ugyanaz a meccs (Újpest–Győr, 2026-11-09 18:00 UTC, scheduled) `verified_publish` állapotú; az alapértelmezett és az `--include-all` napi lista is egy meccset ad. A `persist_report` és a `local_publish_date` ebben a szeletben futott először élesben — hibátlanul. A dev DB új nyugalmi állapota: 21 tábla / 6 szabály / 43 üzleti sor (fixture demóadat, a felhasználó engedélyével); a tesztDB 21/6/0. Commit/push nem történt.

**Futtatott ellenőrzések:**
- Piros: `cargo run -p footballctl -- doctor` → `package(s) 'footballctl' not found in workspace` (exit 101).
- CLI fordítás + clippy: `cargo clippy -p footballctl --all-targets -- -D warnings` tiszta; 14 új lockfile-csomag (clap 4.6.7 stack, mind Rust 1.85-kompatibilis).
- Dev pipeline: migrate → „migráció kész”; fixture_a → match=…0101, state=verified_publish, observations=3; fixture_b → ugyanaz a match, state=verified_publish, observations=3; day és day --include-all → 1 meccs (verified_publish, blockers []); doctor → „db=1 gates=1”.
- psql dev: 1 meccs, 2 forrás (fixture_a, fixture_b), 2 capture, 6 observation, 4 ref, 10 provenance, 2 validation_run (verified), 5 fact.verification (verified); gate: 2026-11-09 / verified_publish / verification-1.0.
- Adattár: `data/raw` a gyökérben (2 artifact); `git check-ignore` igazolja (.gitignore:45:/data/); `rust/data` nem létezik.
- `./scripts/check.sh` zöld: reset + fmt + clippy (workspace) + 17 teszt szekvenciálisan (`--test-threads=1`).
- db-check ágak igazolva: check.sh után dev OK (43 üzleti sor), teszt HIBA (23 sor, „db-reset.sh szükséges”); reset → check-foundation zöld (9 domain+storage teszt, clippy) → reset → db-check: dev OK 21/6/43, teszt OK 21/6/0.
- Magyar szövegek visszaolvasva: CLI (2 string), scriptek, útmutató, checkpoint szekció.

**Eltérések / hibák:**
- db-check.sh igazítás (a felhasználó explicit döntése): dev = szerkezeti ellenőrzés (üzleti sor megengedett), teszt = szigorú 21/6/0. Mindkét ág futtatással igazolva.
- check.sh: a terv literal jelszavas URL-exportja helyett .env sourcing + kötelező `TEST_DATABASE_URL` (AGENTS.md §7, a db-reset.sh konvenciója); a funkció azonos.
- DATA_DIR: a terv `./data`-ja a `rust/data/` mappát hozná létre (nincs gitignore-olva); helyette `../data` → gyökér `data/` (ignore-olt, igazolva).
- Útmutató-adaptációk: a tervben hivatkozott `check-toolchain.sh` nem létezik → `db-create.sh` + .env; literal jelszó nélkül; a terv „nem git repository” állítása hamis → valós commit/push szabályok; a doctor/include-all parancsok és a db-check szemantika dokumentálva.
- A terv CLI-kódja érdemi módosítás nélkül fordult és futott (cargo fmt formázás). A manifest néhány függősége (football-domain, tracing, uuid) közvetlenül nem használt; a terv szerint maradt, figyelmeztetés nincs.
- Átmeneti viselkedés (terv szerint): a CLI `source_values=[]` értéket ad, a SINGLE_SOURCE szabály csak len==1 esetén jelez, így az egyforrásos állapot is verified_publish; a forrásértékek betöltését a Task 13 egészíti ki.

**Nyitott kérdések / nem ellenőrzött részek:**
- A footballd daemon éles indítása, a runtime.json írása és a HTTP végpontok (health/day/gates) hívása továbbra sem történt meg; a terv hátralévő része (Task 13) CLI-fókuszú, a daemon HTTP end-to-end gyakorlása nyitott kérdés.
- A fixture_c (konfliktus) és a kétforrásos source_values betöltés a Task 13-ra vár.
- A Task 13 (kétforrásos konfliktus végponttól végpontig) jóváhagyása a felhasználóra vár; kódolás nem történt.
- A commit/push továbbra is külön döntés; a Tasks 3–12 változásai nincsenek commitolva.
- Automatikus reset a check.sh/check-foundation után nem épült be; az útmutató a manuális db-reset workflow-t rögzíti.

**Következő apró szelet:** Task 13 — kétforrásos konfliktus végponttól végpontig, csak jóváhagyás után.

---

## Extra szelet — footballd daemon HTTP end-to-end gyakorlás

**Engedély:** a felhasználó a Task 12 zárása után kérte: „Előbb indítsd el a footballd daemont és teszteld a HTTP végpontokat”. Ez a Task 12 zárójegyzetében nyitott kérdésként rögzített daemon HTTP E2E gyakorlása; termék-kód módosítás nélkül, csak futtatás és mérés.

**Eredmény:** A footballd daemon élesben elindult a dev adatbázison (127.0.0.1:56782, véletlen port a `BIND_ADDR=127.0.0.1:0` alapján), a runtime.json kiíródott (0600, 64 karakteres read/ops tokenek), a migráció idempotensan lefutott („_sqlx_migrations already exists, skipping”). Mind a 10 HTTP-teszt az elvárt választ adta; a daemon tisztán leállt, a port felszabadult, mindkét DB a nyugalmi állapotban maradt (dev 21/6/43, teszt 21/6/0). Termék-kód nem változott, commit/push nem történt.

**Futtatott ellenőrzések:**
- Build: `cargo build -p footballd` → exit 0.
- Startup napló (/tmp/footballd-e2e.log, repón kívül): „footballd elindult port=56782”, migráció skip; a 10 kérés alatt nem keletkezett hiba_log.
- runtime.json: port=56782, perms=600, read_token_len=64, ops_token_len=64 (a tokenek értéke nem került naplóra/kimenetre).
- HTTP tesztek (curl, státusz + JSON törzs):
  1. health (publikus, token nélkül) → 200 `{"status":"ok","service":"footballd"}`
  2. day 2026-11-09 read tokennel → 200, 1 meccs (Újpest–Győr, 18:00 UTC, scheduled, verified_publish, blockers [], home/away_score null)
  3. day ops tokennel (a Read scope az ops-t is elfogadja) → 200, ugyanaz
  4. day token nélkül → 401 UNAUTHORIZED
  5. day rossz tokennel → 401 UNAUTHORIZED
  6. day invalid-date (read tokennel) → 400 BAD_REQUEST („A dátum formátuma YYYY-MM-DD legyen.”)
  7. day include=all → 200, 1 meccs
  8. gates 2026-11-09 ops tokennel → 200, verified_publish gate (blockers [], evaluated_at 2026-11-10T08:52:55Z)
  9. gates read tokennel (rossz scope) → 401 UNAUTHORIZED
  10. gates token nélkül → 401 UNAUTHORIZED
- Leállítás: TaskStop → killed; `pgrep footballd` nincs processzus; a 56782 port nem figyel; curl a leállítás után → HTTP 000 / exit 7 (connection refused).
- `db-check.sh` a daemon-futás után: dev OK 21/6/43, teszt OK 21/6/0, exit 0.
- Magyar szövegek visszaolvasva: a HTTP-válaszok üzenetei ékezetesen helyesek (Érvénytelen, hozzáférési, dátum); a checkpoint szekció visszaolvasva.

**Eltérések / hibák:**
- Tesztkörnyezeti hiba (nem termék-kód): a curl-teszt függvényében a `local path="$2"` felülírta a zsh speciális `path` tömbjét (PATH), így a függvényen belül a curl/jq „command not found” lett. A gyökérok azonosítva, a változó átnevezése (`url_path`) után mind a 10 teszt hibátlanul lefutott. A termék kódja nem érintett.
- A daemon csak olvasható lekérdezéseket és idempotens migrációt futtatott a dev DB-n; üzleti sor nem változott (43 maradt).

**Nyitott kérdések / nem ellenőrzött részek:**
- A runtime.json a leállítás után a helyén maradt (~/Library/Application Support/FootballAppV2/runtime.json, 0600, repón kívül); a következő daemon-indítás felülírja. A benne lévő tokenek a leállított daemonhoz tartoznak, már érvénytelenek.
- A footballd HTTP E2E ezzel gyakorolva; a Task 12 zárójegyzetének ezzel kapcsolatos nyitott kérdése lezárult.
- A Task 13 (kétforrásos konfliktus végponttól végpontig) jóváhagyása továbbra is a felhasználóra vár; kódolás nem történt.
- A commit/push továbbra is külön döntés; a Tasks 3–12 (+ ez a gyakorlás) változásai nincsenek commitolva.

**Következő apró szelet:** Task 13 — kétforrásos konfliktus végponttól végpontig, csak jóváhagyás után.

---

## Tizenegyedik szelet — Task 13: kétforrásos konfliktus végponttól végpontig

**Engedély:** a felhasználó a daemon HTTP E2E zárása után kérte: „most csinaljuk meg a commitokat aztan mehet a 13”. A commitok elkészültek (00d38fe DB migrációk + scriptek, f3b818e backend kód, 627488b docs; szerző Bandi86 noreply; push nem történt, a branch ahead 3). Ezután a Task 13 engedélyezett. A terv Task 13 kódja az előfeltételekkel egyezik (SourceMaterialValues/VerificationContext mezők, normalized_payload kulcsok, stage.observation séma, MatchStatus/PublishState variánsok, tempfile dev-dep, fixture_c azonos kanonikus meccs-ID eltérő kickoff-fal). Commit/push a szelet végén csak külön kérésre.

**Terv:** [Task 13](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md).

### T13.1 — Bukó kétforrásos teszt

- [x] `tests/conflict_pipeline.rs` a terv szerint (fixture_a + fixture_c, azonos kanonikus meccs, load_material_values, MATERIAL_SOURCE_CONFLICT blokkoló, Withheld gate).
- [x] Piros evidence: a `football_verification::conflict` modul hiányzik.

### T13.2 — conflict.rs betöltő

- [x] `pub mod conflict;` a lib.rs-ben; `conflict.rs` a terv szerint (DISTINCT ON source_id, match_core observation, kanonikus payload parse).
- [x] Fordul és clippy-tiszta.

### T13.3 — CLI bekötés

- [x] footballctl IngestFixture: `load_material_values` hívás a context előtt, `source_values: &source_values` a `&[]` helyett.
- [x] Fordul és clippy-tiszta.

### T13.4 — Zöld tesztek és teljes ellenőrzés

- [x] conflict_pipeline teszt zöld; `./scripts/check.sh` zöld (fmt + clippy + szekvenciális workspace tesztek).
- [x] CLI E2E smoke a TESZT DB-n: fixture_a + fixture_c → withheld; day --include-all withheld.
- [x] TesztDB reset 21/6/0; db-check: dev változatlan 21/6/43, teszt 21/6/0.
- [x] Magyar szövegek visszaolvasása.

### T13.5 — Zárás

- [x] Eredmény, parancsok, eltérések rögzítve.
- [x] Commit/push nem történt (csak a felhasználó külön kérésére).
- [ ] Következő szelet csak jóváhagyásra előkészítve.

### Záró jegyzet (Task 13)

**Eredmény:** Elkészült a `verification::conflict::load_material_values` betöltő (stage.observation match_core, DISTINCT ON source_id, kanonikus payload → SourceMaterialValues), a CLI `source_values` bekötése és a kétforrásos `conflict_pipeline` teszt. A fixture_a + fixture_c (azonos kanonikus meccs …0101, eltérő kickoff 18:00 vs 19:00) `MATERIAL_SOURCE_CONFLICT` blokkolót és `withheld` gate-et ad. A CLI smoke a teszt DB-n igazolta: egy forrás → `provisional_publish` (SINGLE_SOURCE warning), két konfliktusos forrás → `withheld`; az alapértelmezett napi lista üres, az `--include-all` a withheld meccset a blokkolóval mutatja. A dev DB változatlan (21/6/43), a teszt DB reset után 21/6/0. Ebben a szeletben commit/push nem történt. A terv a Task 13-mal zárul — a backend megvalósítási terv (Tasks 1–13) kész.

**Futtatott ellenőrzések:**
- Piros: `cargo test -p football-verification --test conflict_pipeline` → E0432 unresolved import `football_verification::conflict` (exit 101).
- clippy: `cargo clippy -p football-verification -p footballctl --all-targets -- -D warnings` → exit 0.
- Zöld: conflict_pipeline teszt → 1 passed (exit 0).
- `./scripts/check.sh` → exit 0: reset + fmt + clippy (workspace) + 18 teszt szekvenciálisan (mind passed; +1 conflict_pipeline a Task 12 17-hez képest).
- CLI E2E smoke a TESZT DB-n (DATA_DIR temp, DATABASE_URL=test): reset exit 0; fixture_a → match=…0101 state=provisional_publish, observations=3; fixture_c → state=withheld, observations=3; day default → `[]`; day --include-all → 1 meccs withheld, blockers [MATERIAL_SOURCE_CONFLICT / blocker / „A források anyagi meccsmezőben eltérnek.”], kickoff 19:00 UTC; doctor → db=1 gates=1.
- Reset + db-check: reset exit 0; dev OK 21/6/43, teszt OK 21/6/0, exit 0.
- Magyar szövegek visszaolvasva: conflict.rs („ismeretlen státusz”, „hiányzó mező”), checkpoint szekció + zárójegyzet.

**Eltérések / hibák:**
- A terv Interfaces prózája `load_source_values`-t említ, de minden kód (teszt, Step 3, Step 4) `load_material_values`-t használ; ez utóbbit követtem (konzisztens).
- Viselkedésváltozás a Task 12-höz képest (a bekötés helyes következménye): a CLI most valós source_values-t ad, így az egyforrásos meccs SINGLE_SOURCE warningot kap → `provisional_publish` (Task 12-ben a `&[]` miatt `verified_publish` volt). Két egyező értékű forrás (a+b) verified_publish maradna; a konfliktusos (a+c) withheld.
- A kanonikus meccs kickoff a második materialize után 19:00 (fixture_c); a gate withheld a konfliktus miatt, a blokkoló üzenet helyes.
- Tesztkörnyezeti elírás (nem termék-kód): az első CLI smoke hívás rossz dir_path-tel futott (rust/ a gyökér helyett), semmi nem történt (reset 127); gyökérből újra futtatva hibátlan.
- A terv kódja érdemi módosítás nélkül fordult és futott (cargo fmt formázás).

**Nyitott kérdések / nem ellenőrzött részek:**
- A dev DB-t nem futtattam újra a Task 13 kódjával (üzleti sor 43, változatlan); a dev gate a Task 12-i állapotban maradt. A dev demóadat frissítése az új CLI-vel külön döntés lenne.
- A Task 13 változásai (conflict.rs, lib.rs, main.rs, conflict_pipeline.rs, checkpoint) commitolatlanok; a commit/push külön felhasználói döntés.
- A backend terv kész; a következő lépés (Task 13 commitolása, a development branch lezárása/integrálása, vagy új tervezési szelet) csak a felhasználó jóváhagyásával.

**Következő apró szelet:** a terv a Task 13-mal lezárult; következő lépés csak jóváhagyás után (commit / branch-lezárás / új tervezés).
