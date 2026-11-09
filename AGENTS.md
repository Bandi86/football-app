# AGENTS.md — Football App v2 projekt- és ügynökszabályok

Ez a fájl a projekt minden emberi és AI közreműködőjére érvényes. A szabályok a publikus repóra, a helyi gépen futó rendszerre és a v1 archívumra egyaránt vonatkoznak.

Ha a felhasználó ettől eltérő utasítást ad, a felhasználói utasítás az elsőbbséget élvezi. Bizonytalanság esetén nem szabad kitalálni a szándékot: meg kell állni és kérdezni kell.

## 1. Kötelező olvasási sorrend

Minden munka előtt olvasd el:

1. ezt az `AGENTS.md` fájlt;
2. a `README.md` aktuális állapotát;
3. az érvényes specifikációt a `docs/superpowers/specs/` alatt;
4. az aktuális megvalósítási tervet a `docs/superpowers/plans/` alatt;
5. a legutolsó dátumozott checkpointot a `docs/checkpoints/` alatt;
6. a `git status --short --branch` és az aktuális branch állapotát.

A v1 projekt a `../football-app` helyen csak olvasható referencia. Nem módosítjuk, nem migráljuk felül és nem használjuk írásra.

## 2. Lassú és biztos haladás

- Egyszerre csak egy, a felhasználó által megnevezett szeletet szabad megvalósítani.
- A szelet végén meg kell állni, össze kell foglalni az eredményt, és meg kell várni a következő engedélyt.
- Nem szabad a következő tervfeladatra továbblépni csak azért, mert az előző zöld.
- Minden nagyobb döntés előtt egyeztetés, nem utólagos magyarázkodás szükséges.
- A „kész” állítás csak futtatott ellenőrzéssel és bizonyítékkal használható.
- Ha hiba, hiányzó eszköz vagy ellentmondás van, meg kell állni és jelenteni kell.
- Nem nyomjuk el a hibát `--no-verify`, kényszerített git művelet, tiltott kapcsoló vagy destruktív takarítás segítségével.

## 3. Repository-szerkezet

A gyökér csak a legszükségesebb belépési fájlokat tartalmazza:

```text
README.md
AGENTS.md
rust-toolchain.toml
.env.example
.gitignore
apps/                  SwiftUI kliens, amikor elérkezik a fázisa
rust/                  Cargo workspace
  crates/              domain, storage, ingest, verification, api, worker
  bins/                footballd, footballctl
db/migrations/         verziózott PostgreSQL migrációk
scripts/               helyi fejlesztői és ellenőrző scriptek
tests/fixtures/        tesztfixture-ök
docs/
  superpowers/specs/   jóváhagyott specifikációk
  superpowers/plans/   megvalósítási tervek
  checkpoints/         napi, dátumozott checkpointok
  operations/          üzemeltetési útmutatók
  decisions/           visszafordíthatatlan vagy fontos döntések
```

Szabályok:

- Ne hozz létre üres mappákat és `.gitkeep` fájlokat, kivéve ha egy külső eszköz kifejezetten megköveteli.
- Ne legyen generált adat, adatbázis-dump, nyers payload vagy log a repóban.
- A gyökérbe ne kerüljön újabb dokumentumhalom; a dokumentáció a `docs/` megfelelő alkönyvtárába tartozik.
- Egy fájl egyetlen világos felelősséget kap. Túl nagy, mindent tartalmazó modul nem elfogadható.
- A kód és a hozzá tartozó teszt lehetőleg egymás közelében maradjon.
- Új mappa létrehozásakor előbb igazolni kell, hogy a jelenlegi szerkezet nem elegendő.

## 4. Dokumentációkezelés

- **Specifikáció:** `docs/superpowers/specs/YYYY-MM-DD-<téma>-design.md`
- **Megvalósítási terv:** `docs/superpowers/plans/YYYY-MM-DD-<téma>.md`
- **Napi checkpoint:** `docs/checkpoints/YYYY-MM-DD/checkpoint.md`
- **Üzemeltetési útmutató:** `docs/operations/<téma>.md`
- **Fontos döntés:** `docs/decisions/YYYY-MM-DD-<téma>.md`

Minden dokumentum magyar nyelvű. A kód azonosítói angolul vannak; a magyarázó komment csak akkor kell, ha a működés nem magától értetődő.

A checkpoint minden nap két dolgot rögzít:

1. az aznapi induló állapotot és a tervezett apró szeletet;
2. a záró eredményt, a futtatott ellenőrzéseket és a következő döntési pontot.

A checkpoint lépései `- [ ]` jelölést kapnak, és csak ellenőrzés után pipálhatók ki.

## 5. Kód- és architektúraszabályok

- A `domain` crate tiszta: nem függ adatbázistól, HTTP-től, fájlrendszertől vagy gyűjtési rétegtől.
- Az adatbázis-hozzáférés a `storage`, a forrásfeldolgozás az `ingest`, az ellenőrzés a `verification`, a HTTP az `api` felelőssége.
- Az API-handler nem tartalmazhat domain-üzleti szabályt.
- A forrásadapter nem írhat közvetlenül kanonikus entitást.
- A migráció nem lehet adatjavítás helyettesítője.
- Ne legyen production `unwrap()`, `expect()` vagy csendben elnyelt hiba kérésfeldolgozásban.
- Új függőséget csak indoklással és ellenőrzött verzióval szabad hozzáadni.
- A hibák tipizáltak, naplózottak és a felhasználó számára értelmezhető üzenetet kapnak.
- A módosítást előbb a legkisebb tesztelt viselkedésre kell szűkíteni.

Rust formázás és lint:

```bash
cd rust
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

## 6. Adatbázis- és migrációs szabályok

- Minden migráció tranzakciós és egyetlen ellenőrzött lánc része.
- Migráció előtt és után dokumentálni kell a visszaállítási utat.
- Adatjavítást nem migrációval kell elsütni, hanem visszajátszható importtal vagy tudatos revízióval.
- Új táblát és oszlopot csak jóváhagyott szeletben szabad létrehozni.
- A `football_v2_dev` és `football_v2_test` fejlesztési adatbázis; a v1 adatbázisához nem nyúlunk.
- A tesztadatbázisra először minden migrációt le kell futtatni, és csak zöld ellenőrzés után kerülhet a dev adatbázisra.
- Valódi forrásadatot, nyers payloadot vagy meccsadatot csak külön, explicit engedélyezett fázisban szabad betölteni.
- A hiányzó adat `NULL`/státusz, soha nem nulla.
- Minden megjelenített tényhez forrás, rögzítési idő és ellenőrzési állapot tartozik.
- Automatikus fuzzy entitás-összevonás tilos.
- Minden időpont UTC; a helyi megjelenítés külön időzónakezelést kap.
- Minden analitika as-of; jövőbeli információ nem szivároghat vissza.

## 7. Biztonság és publikus repó

- A repó publikus. Titok, API-kulcs, cookie, token, adatbázis-jelszó és személyes adat nem kerülhet bele.
- A helyi `.env`, a `notes/`, a runtime tokenek, a nyers adatok és az adatbázis-mentések ignoráltak.
- Csak a helyi `127.0.0.1` API hallgathat; a kliens nem kap közvetlen adatbázis-hozzáférést.
- Forráscookie és hitelesítő adat a macOS Keychainben él, nem fájlban és nem naplóban.
- Nyers scrape-elt bemenet méret-, idő- és sémaellenőrzést kap.
- Robots/policy, rate limit és forrásvédelem megkerülése tilos.
- Valódi pénzes fogadás, fogadóiroda-integráció és automatikus kifizetés nem része a rendszernek.

## 8. Tesztelés és bizonyítékok

Minden szeletnél kötelező:

- a megfelelő unit- vagy integrációs teszt;
- a futtatott parancs és annak eredménye;
- adatbázis érintése esetén üresség- vagy megszorítás-ellenőrzés;
- UI/bemenet érintése esetén valódi macOS viselkedésellenőrzés;
- a magyar nyelvű szövegek visszaolvasása;
- a nem ellenőrzött részek külön kimondása.

Alapellenőrzés:

```bash
./scripts/check-foundation.sh
```

Új kódnál emellett:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Nem mondható ki, hogy „kész”, ha a teszt nem futott le, vagy ha a futtatás eredménye nem ismert.

## 9. Git, branch és commit

- Nem dolgozunk közvetlenül a `main` ágon, kivéve ha a felhasználó kifejezetten engedélyezi.
- Branch-név: `feat/...`, `fix/...`, `chore/...` vagy `docs/...`.
- Commitot csak kifejezett felhasználói kérésre készítünk.
- Push csak kifejezett felhasználói kérésre történik.
- Nem használunk force-push-t, `reset --hard`-ot, tömeges törlést vagy felülíró git műveletet engedély nélkül.
- Staging előtt ellenőrizni kell a fájllistát; titkot, `.env`-et, nyers adatot vagy generált fájlt nem viszünk be.
- Minden változásnál rögzíteni kell: mi változott, mely fájlok, mi futott, mi maradt ellenőrizetlen, mi a visszaállítási út.

## 10. Kommunikáció

- A felhasználóval magyarul kommunikálunk.
- A frissítések legyenek rövidek, konkrétak és bizonyítékalapúak.
- Egy hosszabb munka végén teljes, összesített zárójelentést adunk.
- Nem állítjuk, hogy valami működik, ha csak a kód írja le a szándékot.
- Ha a felhasználó egy munkaszeletet nevez meg, nem terjeszkedünk ki a következőre.
- Ha egy döntés visszafordíthatatlan, azt előbb fel kell tárni és jóváhagyatni kell.

## 11. Aktuális alapállapot

A projekt a korai alapozási fázisban van. Az aktuális szelet és a következő döntési pont mindig a legutolsó checkpointból olvasandó. A stabil szabályok ettől függetlenül érvényesek.

Amíg a felhasználó mást nem engedélyez:

- nincs valódi forrásadat-betöltés;
- nincs meccs-, csapat- vagy játékos-adat;
- nincs predikció vagy bot;
- nincs UI-adatfolyam;
- nincs automatikus commit vagy push.
