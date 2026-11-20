# Football App v2

Helyi, macOS-re tervezett futball-adatplatform. A cél nem egy újabb tippoldal, hanem egy **visszakövethető, ellenőrzött és elemzésre alkalmas adatrendszer**, amelyre később predikciós és papíralapú fogadásszimulációs rétegek épülhetnek.

> **Állapot (2026-11-20):** a kanonikus adatbázis **287 664 meccs**, a 2006–2026-os szezonokra **189 310 meccs / 96,7% eredmény / 86,1% odds**; **withheld 0, duplikált fixture 0, árva provenance 0, feloldatlan konfliktus 0**, `footballctl integrity` → `OK`. A v1 gap-fill most 1993–2026-ra teljes (11 korábbi és 11 újabb liga; kétkörös bootstrap, forrás-ellenőrzött fixture- és eredményrevíziókkal, minden módosítás auditálva). Az identitás ékezet- és GB-alrégió-toleráns. A napi worker-daemon (launchd, `hu.bandi86.football.v2`) fut, a SwiftUI kliens **32/32 Core + 24/24 app** tesztje zöld, a VoiceOver-címkék közös, tesztelt forrásból jönnek. Kész az S3a kapulánc, az observáció-vezérelt kanonikus út, az Elo/odds/statisztika adapterek, a Versenyek/Csapatok oldal, a meccs-részletező és az Üzemeltetés nézet integritás-kártyája (API + kliens); nyitott: soccerway JS dayPicker, asset-licence döntés, a futó daemon kézi frissítése az új API-mezőhöz.

## Mi ez a projekt?

A v2 egy helyi, egygépes rendszer:

- **Rust daemon** végzi az adatgyűjtést, identitásfeloldást, ellenőrzést, ütemezést és a helyi API-t;
- **PostgreSQL** tárolja a kanonikus adatokat és az üzleti állapotot;
- **nyers, tömörített objektumtár** őrzi a forrásbizonyítékokat;
- **natív SwiftUI kliens** jeleníti meg a napi listát, meccseket, csapatokat, játékosokat és az üzemeltetési állapotot.

Minden megjelenített tény visszavezethető forráshoz és rögzítési időhöz. A hiányzó adat nem jelenik meg nullaként, a bizonytalan identitás nem kerül automatikus összevonásra, és a napi lista csak a publikálási kapun átjutott elemeket mutatja ellenőrzöttként.

## Alapelvek

- Nyers forrásadat előbb, kanonikus érték csak utána.
- Forrásidentitás és belső kanonikus identitás külön kezelve.
- Aliasok kontextussal, bizonytalan egyezés emberi review-val.
- Mezőszintű provenance és konfliktuskezelés.
- Többrétegű, L0–L8 ellenőrzési lánc a publikálás előtt.
- Csak `verified_publish` elem kerül alapértelmezésben a napi listára.
- Minden számítás as-of; nincs jövőbeli információból származó look-ahead.
- Valódi pénzes fogadás és fogadóiroda-integráció nem része a rendszernek.
- Az ügynökök csak korlátozott, naplózott és visszafordítható műveleteket hajthatnak végre önállóan.

## Architektúra

```text
SwiftUI macOS kliens
        |
        v
localhost API (tokenes, csak 127.0.0.1)
        |
        v
Rust daemon
  |-- ingest: forrásadapterek, rate limit, retry
  |-- identity: aliasok, jelöltek, visszafordítható döntések
  |-- verification: szabálymotor és publikálási kapu
  |-- worker: ütemezett feladatok, lefedettség
  `-- storage: repository-k, SQL, objektumtár
        |
        +--> PostgreSQL
        `--> tömörített, tartalom-azonosítós nyers objektumtár
```

A részletes döntéseket a [rendszertervezési specifikáció](docs/superpowers/specs/2026-11-09-football-app-v2-design.md) tartalmazza.

## Tervezési dokumentumok

- [Rendszertervezési specifikáció](docs/superpowers/specs/2026-11-09-football-app-v2-design.md)
- [Ellenőrzött napi lista backend — megvalósítási terv](docs/superpowers/plans/2026-11-09-v2-verified-daily-list-backend.md)

A teljes program több önálló tervre bomlik: backend alap, SwiftUI kliens, teljes identitás- és forráskezelés, meccsközpont, live, predikció, papírszimuláció és autonóm üzemeltetés.

## Projekt- és ügynökszabályok

Minden emberi és AI közreműködőre az [AGENTS.md](AGENTS.md) szabályai érvényesek. Ott találhatók a kötelező olvasási sorrend, a repository- és dokumentációstruktúra, a lassú haladási és megállási pontok, a migrációs, adatvédelmi, tesztelési és git-szabályok.

A projekt tudása a repóban él, nem egy külső profilban:

- [`.dsh/skills/`](.dsh/skills/) — a DSH (és bármely más ágens-motor) automatikusan katalógusba veszi:
  `football-app-v2` (szelet-folyamat, kapu-lánc, adat-szabályok), `swiftui-macos-client`,
  `rust-workspace-backend`.
- [`agents/`](agents/README.md) — a projekt-specifikus szerepkártyák (`lead`, `rust-backend`,
  `swiftui-client`, `data-ops`, `researcher`, `verifier`) a dispatch- és handoff-protokollal.
  Alapelv: builder ≠ verifier, diszjunkt írási hatókör, „kész” csak futtatott bizonyítékkal.

## Tervezett repository-szerkezet

```text
apps/FootballApp/       SwiftUI macOS kliens (későbbi fázis)
rust/
  crates/               domain, storage, ingest, verification, api, worker
  bins/                 footballd, footballctl
db/migrations/          ellenőrzött PostgreSQL migrációk
tests/fixtures/         parser- és integrációs fixture-ök
.dsh/skills/            projekt-specifikus skillek (automatikus katalógus)
agents/                 szerepkártyák és handoff-protokoll
docs/                   specifikációk és üzemeltetési dokumentáció
scripts/                helyi fejlesztői és ellenőrző scriptek
```

## Fejlesztői előfeltételek

- macOS
- Rust stable, `rustfmt` és `clippy`
- PostgreSQL 18 vagy újabb
- teljes Xcode a SwiftUI kliens fázisához
- SQLx CLI a migrációkhoz és a későbbi fordítás idejű SQL-ellenőrzéshez

A Rust verziót a [`rust-toolchain.toml`](rust-toolchain.toml) rögzíti.

## Helyi környezet

A backend már futtatható (a `footballd` daemon és a `footballctl` CLI; lásd a [helyi fejlesztési útmutatót](docs/operations/local-development.md)). A teljes asztali alkalmazás a SwiftUI kliens elkészültéig még nem indítható. A környezeti változók mintája:

```bash
cp .env.example .env
```

A `.env` fájl helyi titkokat tartalmazhat, ezért verziókövetésbe nem kerül.

Rust ellenőrzése:

```bash
rustc --version
cargo --version
rustfmt --version
cargo clippy --version
```

## Adatvédelem és közzététel

Ez egy nyilvános repó. Ne kerüljön bele:

- valódi jelszó, API-kulcs, cookie vagy hozzáférési token;
- nyers forrás-payload vagy licencelt adatbázis-export;
- személyes adat vagy helyi géphez kötött titok.

A nyers adatok, a helyi `.env`, a runtime tokenek, a játékos-adatbázisok és a privát munkajegyzetek a [`.gitignore`](.gitignore) fájlban ki vannak zárva.

## Roadmap

1. Alapozás: repository, eszközlánc, migrációs fegyelem.
2. Identitás, forrásrögzítés, ellenőrzési szabályok és publikálási kapu.
3. Megbízható napi lista, meccs-, csapat- és játékosoldal.
4. Live eseménykövetés.
5. Predikciós szerződés, kalibráció és mérés.
6. Papírportfóliók és szelvénykészítés.
7. Korlátozott autonómiájú ügynökök és fejlett üzemeltetés.
