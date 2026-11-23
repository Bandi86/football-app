# 2026-11-23 — napi checkpoint

## Induló állapot és kérés

- A felhasználó kérése: „olyan korokban mikor nem fut, én valahogy el tudjam indítani — tedd a dockra az ikont, csinálj neki ikon designt; utána daily check (tegnapi adat hibátlan, mai adat kész, egyéb hibák); utána nézzük a következő fejlesztési lépéseket."
- Kiindulás: `main` ág, 3 committal az `origin/main` előtt (utolsó: `900c480`, S100). A legutóbbi checkpoint 2026-11-22-i.
- [x] A futó környezet **nem volt indítható állapotban**: sem `footballd`, sem `FootballApp` nem futott. A daemon launchd-ja 10 másodpercenként újraindult, `last exit code = 1`, a logban: `Error: migration 23 was previously applied but is missing in the resolved migrations` — a telepített bináris (11-22 15:13) régebbi volt a DB migrációs láncánál (0023 `retention indexes` már alkalmazva volt).
- [x] Az appnak nem volt saját ikonja (nincs `Assets.xcassets`), és nem volt telepítő script sem.

## S101 — app ikon, telepítés, Dock, daemon-indítás

- [x] **Ikon**: az AI képgen `Invalid credential` hibával nem volt használható, ezért determinisztikus vektoros ikon készült Swift/AppKitkel: `scripts/make-app-icon.swift` (S39 „éjféli stadion" paletta: `#08080A` háttér, `#FFB020` borostyán, `#30D158` élő pont). A forrás PNG 1024×1024, a repóban újragenerálható.
- [x] **Asset katalógus**: `apps/FootballApp/Assets.xcassets/AppIcon.appiconset/` 10 mérettel (`idiom: mac`). A `project.yml` megkapta `ASSETCATALOG_COMPILER_APPICON_NAME: AppIcon` és `CFBundleIconName: AppIcon` bejegyzéseket. Az első, modern egyképes (`universal`/`osx`/1024) formátumot az Xcode 27 actool `unassigned child` figyelmeztetéssel elutasította és nem állított elő ikont — a klasszikus 10 méretes macOS formátum megoldotta.
- [x] **Daemon javítása**: `cargo build -p footballd -p footballctl --features obscura` (18 s) → `./scripts/install-footballd-agent.sh`. Új daemon: pid **6942**, port **56394**, `/v1/health` → `{"status":"ok","service":"footballd"}`. A telepítő az OpenFoot kulcsot a Keychainből olvassa (jelenléte ellenőrizve, érték nélkül).
- [x] **App telepítése**: új `scripts/install-football-app.sh` (xcodegen + build + `ditto` a `~/Applications/FootballApp.app`-be, idegen bundle-id felülírása ellenőrzéssel). `xcodebuild … clean build` → **BUILD SUCCEEDED**; a bundle-ben `AppIcon.icns` + `Assets.car`.
- [x] **Dock**: új `scripts/add-football-app-to-dock.sh` (idempotens, `defaults export/import com.apple.dock` + `killall Dock`). A Dock-plistben igazolva: `file:///Users/susleczandras/Applications/FootballApp.app/`, label `FootballApp`.
- [x] **Valódi macOS indítás**: `open ~/Applications/FootballApp.app` → pid **8191**; AX-próba: `home.stat.matches` → **„117 mérkőzés"** (a mai nap betöltött).
- [x] README: az elavult „az asztali alkalmazás még nem indítható" mondat helyett a három új parancs szerepel.

## S102 — napi ellenőrzés

- [x] Új `scripts/daily-check.sh` (read-only, magyar jelentés, kemény hibánál exit 1): daemon health, tegnapi `integrity` + `reverify` + `coverage-day`, mai `coverage-day`, mindkét nap `/v1/day` (státuszok, hiányok, `refreshed_at`), review/job/breaker/gate SQL.
- [x] **Tegnap (2026-11-22)**: `integrity` → **OK**, duplikált kickoff 0, duplikált fixture 0, azonos pillanat 0, azonos nap 0, árva provenance 0, összevont entitásra mutató ref 0, withheld 0, failed job 0 (1 függő review, 1 nyitott breaker). `reverify` → **619 meccs, 0 hiba** (409 provisional / 210 verified). `coverage-day` → **624 meccs / 593 lezárt / 593 eredmény / 289 odds / 360 logó / 0 idő nélkül / 0 withheld**. `/v1/day` → **619 sor** {588 finished, 17 postponed, 12 scheduled, 2 canceled}, `open_past 12`, `missing_team_logo 260`, `missing_competition_logo 485`, `missing_odds 330`.
- [x] **Ma (2026-11-23)**: `coverage-day` → **117 meccs / 23 lezárt / 23 eredmény / 49 odds / 88 logó**. `/v1/day` → **117 sor** {23 finished, 1 live, 93 scheduled}, `refreshed_at=2026-11-23T08:04:26Z`, nincs `open_past`, nincs withheld. A napi adat tehát friss és betöltött (a nap természetesen még nyitva van).
- [x] **Egyéb (figyelmeztetés, nem adathiba)**: 1 pending review (`Karcag`, `no_exact_match`, HU senior male — döntés: új entitás vagy meglévőhöz kötés); 1 failed job (soccerway `day` 2026-11-23, `attempts 2`, `last_error: breaker nyitva` — a régen ismert nyitott soccerway breaker, 7 failure); 5 pending job; 0 withheld gate.

## Futott ellenőrzések (mért)

- [x] `cargo build -p footballd -p footballctl --features obscura` → `Finished dev profile in 18.00s`, 0 hiba.
- [x] `./scripts/install-footballd-agent.sh` → exit 0; `launchctl print …hu.bandi86.football.v2` → `state = running`, `pid = 6942`; log: `footballd elindult port=56394`.
- [x] `cd apps && xcodegen generate` → projekt generálva; `xcodebuild … clean build` → **BUILD SUCCEEDED**.
- [x] `actool … --app-icon AppIcon` → `AppIcon.icns` + `Assets.car`; `iconutil -c iconset …` → 128×128 PNG, a ball + borostyán panel + zöld pont kis méretben is olvasható.
- [x] `./scripts/add-football-app-to-dock.sh` → „hozzáadva"; Dock-plist lekérdezése → találat a FootballApp URL-re.
- [x] `open ~/Applications/FootballApp.app` → pid 8191; `swift scripts/ax-probe.swift FootballApp home.stat.matches` → `value=117 mérkőzés`.
- [x] `./scripts/daily-check.sh` → **EXIT=0**, kétszer futtatva; jelentés `/tmp/fb-daily-check-1123.log`.
- [x] `git diff --check` → 0 hiba; `bash -n` a 3 új shell scriptre → 3/3 OK.

## Nem ellenőrzött

- A Dock ikon vizuális megjelenése képernyőképen nem lett rögzítve (a Dock-plist bejegyzés és az `.icns` tartalom igazolt; a személyes Dock-ot a script módosította).
- A 12 `open_past` meccs forrásánkénti kivizsgálása (a korábbi checkpoint szerint a research/források még nem adnak eredményt — most sem lett egyenként ellenőrizve).
- A „Karcag" review és a soccerway breaker kezelése (döntést igényel).
- VoiceOver beszédhang és a kompakt ablak vizuális ellenőrzése ebben a szeletben nem futott.

## Visszaállás

- Ikon/telepítő: a 4 új script + `project.yml` + asset katalógus `git revert`-tel visszafordítható; az ikon PNG-k a `make-app-icon.swift`-ből újragenerálhatók.
- Dock: a tile eltávolítható a Dockból jobb kattintással, vagy a `persistent-apps` tömből való törléssel; a `~/Applications/FootballApp.app` törölhető.
- Daemon: a régi bináris hibás volt (migrációs lánc-eltérés), visszaállítása nem kívánatos; a javított bináris a repó buildjéből bármikor újratelepíthető az install scripttel.

## S101 — időjárás a backendbe, valódi countdown

A felhasználó a 4 javasolt szeletet sorrendben jóváhagyta („mehetnek sorban"). Ez az 1. szelet; a `main` ágon.

### Változott

- [x] `db/migrations/0024_weather_cache.sql` — `ops.weather_cache` (kulcs: 2 tizedesre kerekített koordinátapár, payload jsonb, `captured_at`, `observed_at`, `next_refresh_at`, source; 7 napos korlát). Tranzakciós, rollback a fájl végén. Táblaszám 50 → **51** (`scripts/db-check.sh`, `storage/tests/connection.rs` frissítve).
- [x] `rust/crates/ingest/src/weather.rs` — kulcs nélküli, read-only Open-Meteo fetcher 5 s időkorláttal, 64 KiB válasz-korláttal, tartalomtípus- és koordináta-ellenőrzéssel; `weather_location_key`, `parse_observed_at`, `WEATHER_REFRESH_SECS = 900`. A reqwest workspace-konfigja nem hordozza a `.query()` helperhez kellő feature-t, ezért az URL épített (a koordináták validált számok).
- [x] `rust/crates/storage/src/repositories/weather.rs` — cache get/upsert; a `test_support` TRUNCATE-listája kiegészült az új táblával.
- [x] `rust/crates/api/src/routes/weather.rs` — `GET /v1/weather?latitude=&longitude=` (opcionális paraméterek, alap Budapest), Read token, cache-hit vagy friss hívás, hiba esetén elavult cache `stale: true`-val, különben 503. Új `AppError::service_unavailable`. OpenAPI: 12 → **13** útvonal, `WeatherResponse` séma, új tag.
- [x] Kliens: `LocalWeatherService` (csak `127.0.0.1`, Bearer token, koordinátákat nem tárolja/naplózza), `WeatherRefreshMetadata` (async protokollmetódus, actor-tároló), `WeatherModel` a backend metaadatokat közvetíti, `HomeHeader` a countdown-t a backend `next_refresh_at`-jéből számolja (csak ha nincs ilyen, marad a régi 6 órás becslés). A forráscímke „helyi API (Open-Meteo)" és az „elavult adat" jelzés is megjelenik.

### Mért eredmény

- [x] Élő daemon: pid **13971**, port **58583**. `GET /v1/weather` → 200, `temperature_c 3.0`, `weather_code 0`, `captured_at 08:21:37Z`, `next_refresh_at 08:36:37Z`, `stale false`. A DB-ben 1 cache-sor (`47.50,19.04`). `GET /openapi.json` → 13 útvonal, `/v1/weather` jelen.
- [x] Élő app: pid **16680**. AX: `home.stat.weather` → „Időjárás: 3,3 °C, Derült, Cserszegtomaj, saját helyzet, **élő adat**" — a kliens nem hívja közvetlenül az Open-Meteo-t.
- [x] Múltbeli napon (11-22, 619 meccs, nincs élő meccs) a countdown: „Frissítés: **14:36**, következő frissítés. A következő adatfrissítés **09:37** körül várható." — ez a DB `next_refresh_at` (09:36:37) alapján, nem a régi 6 órás becslés.

### Futott ellenőrzések (mért)

- [x] `cargo test --workspace -- --test-threads=1` → **568 passed / 0 failed**, 105 teszt-bináris.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` → 0 figyelmeztetés; `cargo fmt --all --check` → 0.
- [x] `./scripts/db-reset.sh` → 0; `./scripts/check-foundation.sh` → 0 (51 tábla).
- [x] `swift test --package-path apps/FootballAppCore` → **121 teszt / 18 suite, 0 hiba** (új: `localServiceUsesLocalApiAndExposesBackendSchedule`).
- [x] `xcodebuild test … -only-testing:FootballAppTests` → **74 teszt, 0 failure** (új: `testBackendScheduleDrivesCountdownMetadata`).
- [x] `./scripts/run-ui-tests.sh -parallel-testing-enabled NO` → **74 app + 8 UI teszt, 0 failure; TEST SUCCEEDED**.
- [x] `git diff --check` → 0 hiba.

### Visszaállás / korlátok

- Visszaállás: `git revert` a szelet commitjára; a 0024 migráció visszaállítása `DROP TABLE ops.weather_cache;` + a `_sqlx_migrations` 0024 sorának törlése (külön, egyeztetett lépés). A kliens visszaáll a napszak-alapú fejlécre, ha a végpont nem elérhető.
- Nem ellenőrzött: a Weather tile VoiceOver-beszédhangja; a `stale: true` ág élőben (csak unit teszt); a képernyőkép nem került fájlba.
- A api crate új belső függőséget kapott (`football-ingest`) — új külső csomag, kulcs vagy hálózati engedély nem.

## S102 — bíró-identitás és ország

A felhasználó a 4 szeletet sorban jóváhagyta; ez a 2. szelet. A tudatos policy: **`docs/decisions/2026-11-23-referee-policy.md`** — nincs auto-create, nincs fuzzy merge, research továbbra is display-only; a bíró saját országa viszont tárolódik és a teljes név egzakt linkjénél érvényesül.

### Változott

- [x] `db/migrations/0025_referee_country.sql` — `core.entity.country_id` (NULL = ismeretlen, minden személyfajtára) és `fact.match_official.country_code` (kizárólag a forrás által kimondott bíró-ország; a versenyország nem kerül ide). Rollback a fájlban.
- [x] `rust/crates/ingest/src/research.rs`, `normalized.rs`, `canonicalize.rs` — a research `referee.country` végigjut az observation → normalized → `fact.match_official` úton; a linkelési kontextus a bíró saját országa, ha van, egyébként a versenyország review-kontextusként.
- [x] `rust/crates/storage/src/repositories/identity.rs` — `ENTITY_COLUMNS`/`ENTITY_JOIN` a `COALESCE(c.code, ec.code)`-vel; `entity_context` most a személyfajták országát is visszaadja, de továbbra is `None`, ha nincs sem team sor, sem entitás-ország.
- [x] `rust/crates/identity/src/review.rs` — a jóváhagyási keresés fajtátudatos (`entities_by_normalized_name` a nem-csapat fajtáknál); ismert országkódnál `country_id` íródik, ismeretlen kódnál NULL (nem találunk ki országot).
- [x] Tesztek: `referee_link.rs` (bíró-ország felülírja a versenyországot; eltérés → nincs link; `fact.match_official.country_code`), `identity_repos.rs` (referee kontextus), 6 ingest-fixture a `referee_country` mezővel.

### Futott ellenőrzések (mért)

- [x] `./scripts/db-reset.sh` → 0; `./scripts/check-foundation.sh` → 0; dev DB migrációk: `24|weather cache`, `25|referee country`.
- [x] `cargo test --workspace -- --test-threads=1` → **571 passed / 0 failed**.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` → 0; `cargo fmt --all --check` → 0.
- [x] Daemon újratelepítve (pid **19681**), health rendben.

### Nem ellenőrzött / korlátok

- A 564 meglévő `fact.match_official` sor nem kapott utólag `country_code`-t (nincs adatjavítás migrációval); az új értékek csak a következő research ingesttel jelennek meg.
- A feloldási arány nem nőtt automatikusan (1/564 marad) — ez tudatos; a rövid alakú katalógus normalizálása külön szelet.
- A live research envelope `referee.country` mezőjének tényleges tartalma nem lett újraingestelve ellenőrizve (a 0022 migráció kommentje igazolja a mezőt).

### Visszaállás

- `git revert` a szelet commitjára; 0025 visszaállítása: `ALTER TABLE fact.match_official DROP COLUMN country_code; DROP INDEX idx_entity_country_id; ALTER TABLE core.entity DROP COLUMN country_id;` + a migrációs sor törlése külön lépés.

## Következő döntési pont

A felhasználó a négy szeletet sorban jóváhagyta; az 1. és 2. kész. Soron következik:

3. **Fázis-2 kilépési feltételek** — a feltárás szerint a **játékosoldal teljes adattartalma blokkolt**: 0 player entitás, 0 játékos-statisztika, 9 156 szöveges játékosnév, `core.player`/`player_team_spell` nem létezik. Ez valós adatfeltöltést és review-t igényel (AGENTS.md §6: külön, explicit engedélyezett fázis) — nem lehet UI-val vagy kitalált adattal lezárni. A megvalósítható rész: globális keresés (Cmd+F) + akadálymentességi maradék; a játékos-adatfázisról a felhasználót kell kérdezni.
4. Adatminőség-maradék: competition-duplikátumok, Liga F forrás-konfliktus, tipplap replay.

Gyors operatív tételek, bármikor elvégezhetők: a „Karcag" review eldöntése és a soccerway breaker újrapróbálása/csendesítése.
