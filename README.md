# Football App v2

Helyi, macOS-re tervezett futball-adatplatform. A cél nem egy újabb tippoldal, hanem egy **visszakövethető, ellenőrzött és elemzésre alkalmas adatrendszer**, amelyre később predikciós és papíralapú fogadásszimulációs rétegek épülhetnek.

> **Állapot (2026-11-20):** a kanonikus adatbázis **287 664 meccs**, a 2006–2026-os szezonokra **189 310 meccs / 96,7% eredmény / 86,1% odds**; **withheld 0, duplikált fixture 0, árva provenance 0, feloldatlan konfliktus 0**, `footballctl integrity` → `OK`. A v1 gap-fill most 1993–2026-ra teljes (11 korábbi és 11 újabb liga; kétkörös bootstrap, forrás-ellenőrzött fixture- és eredményrevíziókkal, minden módosítás auditálva). Az identitás ékezet- és GB-alrégió-toleráns. A napi worker-daemon (launchd, `hu.bandi86.football.v2`) a repó buildjével fut (S31d-ben újratelepítve), a SwiftUI kliens **37/37 Core + 26/26 app + 5/5 UI** tesztje zöld, a VoiceOver-címkék közös, tesztelt forrásból jönnek. Kész a kapulánc, az observáció-vezérelt kanonikus út, az Elo/odds/statisztika adapterek, a Versenyek/Csapatok oldal, a meccs-részletező, az Üzemeltetés nézet integritás-kártyája, az automatizált XCUITest UI-harness, a GitHub Actions CI (S32b–S32c, első teljes zöld futás: `44733358476`), a nap-végpont API↔Swift kontrakt, a teljes OpenAPI-dokumentum (`GET /openapi.json`) az app-szintű Tab-sorrend (S34: felső navigáció → fejléc → szűrők → kártyák, saját fókuszgyűrűvel, 7/7 UI-teszttel) a helyi AI-réteg (S35: `football-ai` crate, `footballctl ai search|ask`, Magnitude gemma 4 + helyi SearXNG) és a Tipplap-adapter liga-lefedettségének bővítése (S36a: 16 → 42 ligacím, a daemon újratelepítése után élesben 4 → 29 oddsos meccs a mai napon, 19 verseny, 17 nyitott identitás-review-val), valamint a napon belüli frissítés (S36b: 360 perces refresh-sávok a schedulerből; tipplap napi 5 kérés a 8-as keretből, élesben a slot-00 már lefutott); nyitott: a tényleges VoiceOver-felolvasás (emberi fül kell hozzá), a Tab-kör kiterjesztése a natív menükre/naptár-popoverre/részletezőre, valamint a soccerway JS dayPicker.

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
apps/FootballApp/       SwiftUI macOS kliens (nézetek)
apps/FootballAppCore/   kliens-üzleti logika (Swift Package, tesztelt)
apps/FootballAppUITests/ XCUITest akadálymentességi/viselkedési tesztek
rust/
  crates/               domain, storage, ingest, verification, api, worker
  bins/                 footballd, footballctl
db/migrations/          ellenőrzött PostgreSQL migrációk
tests/fixtures/         parser-, integrációs és UI-fixture-ök (tests/fixtures/ui)
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

## Kliens (SwiftUI) ellenőrzése

Az Xcode-projekt generált (XcodeGen), ezért előbb:

```bash
cd apps && xcodegen generate && cd ..
```

Fordítás és unit-tesztek (a `DEVELOPER_DIR` kötelező, mert az `xcode-select` a CommandLineTools-ra mutat):

```bash
cd apps/FootballAppCore && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test && cd ../..
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test \
    -project apps/FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS'
```

Automatizált UI-tesztek (determinisztikus fixture-szerverrel, futó daemon nélkül):

```bash
./scripts/run-ui-tests.sh
```

A UI-harness a VoiceOver által is használt akadálymentességi címkéket, a billentyűparancsokat
és a betöltött/üres/offline állapotokat ellenőrzi; részletek a
[`docs/operations/ui-accessibility-checklist.md`](docs/operations/ui-accessibility-checklist.md) fájlban.

A futó kliens olvasásához/vezérléséhez (Accessibility-engedéllyel):

```bash
swift scripts/ax-press.swift FootballApp --list        # AX-fa: gombok, kártyacímkék
swift scripts/ax-press.swift FootballApp "Következő nap"  # AXPress, emberi kattintással egyenértékű
swift scripts/ax-key.swift FootballApp tab             # Tab/nyíl/Return/Escape a futó appnak
swift scripts/ax-probe.swift FootballApp match.card     # AX-elemek szűrve, szerep + érték
./scripts/app-window-shot.sh /tmp/ablak.png            # a futó ablak PNG-mentése
```

## Continuous integration

A `.github/workflows/ci.yml` két párhuzamos feladatot futtat pushra és pull requestre:

- **Rust** (`ubuntu-latest`): PostgreSQL 18 szerviz, migrációk a dev és a teszt adatbázisra,
  `./scripts/check-foundation.sh`, `cargo test --workspace -- --test-threads=1`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build -p footballd -p footballctl --features obscura`.
- **Swift** (`macos-latest`): Xcode kiválasztása, `xcodegen`, `swift test --package-path apps/FootballAppCore`,
  majd `xcodebuild test … -only-testing:FootballAppTests`.

Az `FootballAppUITests` harness szándékosan nem fut CI-ben: macOS-en Accessibility/TCC engedély és futó ablak
kell hozzá; helyben a `./scripts/run-ui-tests.sh` futtatja.

## API↔kliens kontrakt

A `tests/fixtures/contract/day-response.json` a `GET /v1/day/{date}` válasz kanonikus alakja.
Két oldal méri ugyanahhoz a fájlhoz:

- `rust/crates/api/tests/day_contract.rs` — a Rust `DayResponse` szerializációja pontosan ezt adja-e;
- `apps/FootballAppCore/Tests/FootballAppCoreTests/DayContractTests.swift` — a Swift DTO-k
  ugyanezt a payloadot hiánytalanul dekódolják-e.

Mezőátnevezés vagy típusváltás így nem tud csendben elcsúszni a két oldal között.

## OpenAPI

A teljes HTTP-szerződés a **`GET /openapi.json`** útvonalon olvasható (OpenAPI 3.1, token nélkül — séma, nem adat).
A dokumentum `utoipa` derive-okból **fordítás időben** épül: a handlerek `#[utoipa::path(...)]`, a válasz- és
sor-típusok `utoipa::ToSchema` annotációt kapnak, ezért a leírás nem tud csendben elszakadni a szerializált
mezőktől, és nincs generált artifact a repóban.

A szerződést tesztek őrzik (`rust/crates/api/tests/openapi.rs`): mind a 11 dokumentált útvonal valóban routol
(ellenőrzött negatív kontrollal), minden `$ref` feloldható, a `DayResponse`/`DayMatch` séma mezői pontosan a
`tests/fixtures/contract/day-response.json` kulcsai, a bearer séma deklarálva van, és a `GET /openapi.json`
token nélkül 200-at ad. A döntés és indoklása: [docs/decisions/2026-11-20-openapi.md](docs/decisions/2026-11-20-openapi.md).

## Helyi AI (S35)

A háttérfeladatokhoz (keresés, forrás-ellenőrzés, későbbi napi összefoglaló) a gép
**helyi** AI-stackje használható; sem külső API, sem kulcs nem kell hozzá:

- **Magnitude** (OpenAI-kompatibilis Chat Completions): `http://127.0.0.1:10100/inference/v1`,
  aktív modell a gemma 4 E4B Artemis (Q4_0);
- **SearXNG** (helyi metakereső, JSON): `http://127.0.0.1:8888/search?q=…&format=json`.

A `football-ai` crate egy tool-call hurokban köti össze őket: a modell `web_search`
hívást kérhet, a hívás a SearXNG-t kérdezi, és a válasz mellett a felhasznált
forrás-URL-eket is visszaadja. CLI-ból, adatbázis nélkül:

```bash
footballctl ai search "Debrecen Paks mérkőzés" --limit 6
footballctl ai ask "Mikor játssza a Debrecen a Paks elleni következő mérkőzését?" --max-steps 2
```

A végpontok felülírhatók a `FOOTBALL_AI_MAGNITUDE_URL`, `FOOTBALL_AI_MODEL`,
`FOOTBALL_AI_SEARXNG_URL`, `FOOTBALL_AI_TIMEOUT_SECS`, `FOOTBALL_AI_MAX_TOOL_STEPS`
változókkal; a default mindegyiknél a fenti lokális cím. A worker-oldali háttér-job
bekötése külön szelet. Részletek: [docs/decisions/2026-11-20-local-ai.md](docs/decisions/2026-11-20-local-ai.md).

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
