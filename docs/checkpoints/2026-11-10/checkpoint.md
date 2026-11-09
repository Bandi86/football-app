# 2026-11-10 — Napi checkpoint

**Állapot:** tervezett  
**Előző nap:** 2026-11-09  
**Kapcsolódó terv:** [Ellenőrzött napi lista backend](../../superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md), Task 3

## Napi cél

A `raw`, `stage` és `ops` adatbázis-sémák első, ellenőrzött migrációjának elkészítése és tesztadatbázison való igazolása.

Ez a nap még **nem** tartalmaz meccs-, csapat-, játékos-, forrás- vagy oddsadatot. Az egyetlen engedélyezett adat a rendszer működéséhez szükséges hat validációs szabály metaadata.

## Kiinduló állapot

- [ ] A git branch és a munkafa áttekintése megtörtént.
- [ ] Az előző napi alapozás változatlanul zöld.
- [ ] A `football_v2_dev` és `football_v2_test` adatbázis elérhető.
- [ ] Mindkét adatbázis üres; nulla felhasználói tábla.
- [ ] Nincs `.env` vagy titok a verziókövetésben.

Ellenőrző parancs:

```bash
./scripts/check-foundation.sh
git status --short --branch
```

Megállási feltétel: ha bármely ellenőrzés piros, a nap nem megy tovább, előbb a hibát kell megérteni és javítani.

## Holnapi munkasorrend

### C0 — Környezet és eszköz

- [ ] Az `AGENTS.md` és a legutóbbi checkpoint átolvasása.
- [ ] `sqlx-cli` meglétének ellenőrzése.
- [ ] Ha hiányzik, telepítés: `cargo install sqlx-cli --no-default-features --features postgres,rustls`.
- [ ] `sqlx --version` sikeres.
- [ ] `./scripts/db-check.sh` zöld, mindkét adatbázis üres.

### C1 — Migrációs fájl előkészítése

- [ ] `db/migrations/0001_raw_stage_ops.sql` létrehozása.
- [ ] Csak a következő sémák és táblák kerülnek bele:
  - `raw.artifact`
  - `raw.capture`
  - `stage.observation`
  - `ops.source`
  - `ops.validation_rule`
  - `ops.validation_run`
  - `ops.validation_finding`
  - `ops.audit_event`
- [ ] A migráció tranzakcióban fut.
- [ ] A migráció újrafuttatható viselkedése és a visszaállítási út dokumentált.
- [ ] Nincs meccs-, csapat-, játékos-, forrás- vagy oddsrekord.

### C2 — Tesztadatbázis-first végrehajtás

- [ ] A migráció először kizárólag a `football_v2_test` adatbázison fut le.
- [ ] A `_sqlx_migrations` rendszer tábla mellett pontosan a nyolc tervezett tábla létezik.
- [ ] A hat `validation_rule` sor létezik.
- [ ] Az összes többi tábla sorszáma nulla.
- [ ] A migráció második futtatása nem hoz létre duplikált szabályt.

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

- [ ] Tesztadatbázis zöld után a migráció a `football_v2_dev` adatbázison is lefut.
- [ ] Ugyanazok a táblák és szabályok jelennek meg.
- [ ] Nincs üzleti adat.
- [ ] A migrációs napló sikeres.

### C4 — Tesztek és ellenőrzések

- [ ] A storage-kapcsolati teszt a létrejött sémához igazítva zöld.
- [ ] A foundation-check script zöld.
- [ ] `cargo fmt --all --check` zöld.
- [ ] `cargo test -p football-domain -p football-storage` zöld.
- [ ] `cargo clippy -p football-domain -p football-storage --all-targets -- -D warnings` zöld.
- [ ] A magyar nyelvű szövegek visszaolvasása megtörtént.

### C5 — Zárás és holnaputáni előkészítés

- [ ] A nap eredménye, a futtatott parancsok és az eltérések beírása ide.
- [ ] A nyitott kérdések listázása.
- [ ] Csak a Task 4 (core, identitás, provenance) következő szelet jóváhagyása; kódolni még nem kell.
- [ ] Commit/push csak külön, explicit jóváhagyással.

## Megállási és visszagörgetési szabályok

- Ha bármely migráció vagy teszt piros, nem lépünk a következő checkpointra.
- Ha a tesztadatbázisban nem várt tábla vagy adat jelenik meg, megállunk és összehasonlítjuk a migrációval.
- Mivel a dev adatbázis még üres, a visszaállítási terv a három új séma (`raw`, `stage`, `ops`) eldobása lehet; ezt csak külön jóváhagyással és előzetes ellenőrzéssel szabad futtatni.
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

- [ ] A `0001` migráció jóváhagyva.
- [ ] A nyolc tábla és a hat validációs szabály elfogadva.
- [ ] A következő napra csak a core/identitás/provenance séma marad.

## Nap végi jegyzet

**Eredmény:**  
**Futtatott ellenőrzések:**  
**Eltérések / hibák:**  
**Nyitott kérdések:**  
**Következő apró szelet:**  
