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

## S103a — globális keresés (⌘F)

A felhasználó a 4 szeletet sorban jóváhagyta; ez a 3. szelet megvalósítható része.

### Változott

- [x] `rust/crates/storage/src/repositories/search.rs` — ékezet-inszenzitív keresés (`translate()` az SQL-ben, nincs `unaccent` kiterjesztés) csapatokra, versenyekre és az utóbbi 7 nap meccseire; fajtánként 8 találat.
- [x] `rust/crates/api/src/routes/search.rs` — `GET /v1/search?q=` (min. 2 karakter, max 80, Read token), `SearchResponse`/`SearchHit` sémák; OpenAPI 13 → **14** útvonal; új `search` tag.
- [x] Kliens Core: `SearchModels.swift`, `SearchService.swift` (`HTTPSearchService`, csak `127.0.0.1`); új kontrakt-fixture `tests/fixtures/contract/search-response.json` és kétoldalas teszt.
- [x] Kliens app: `SearchModel.swift`, `SearchView.swift` (fajta + szöveges címke, AX-címkék), `nav.search` gomb `⌘F` shortcut-tal, a `KeyboardMonitor` is kezeli a ⌘F-et; `SearchField` most AX-címkét kap.
- [x] UI-harness: fixture-szerver `/v1/search` útvonala + `testGlobalSearchCmdFShowsResults` (9. UI-teszt).

### Mért eredmény

- [x] Élő daemon: port **64853**. `GET /v1/search?q=ujpest` → az ékezetes „Újpest" csapat és meccsei is megtalálhatók; `?q=x` → **400**; `/openapi.json` → **14** útvonal, `/v1/search` jelen.
- [x] Élő app: `nav.search` AX-gomb jelen; a `swift scripts/ax-key.swift FootballApp cmd-f` után `search.field` AX-mező jelen („Globális keresés").

### Futott ellenőrzések (mért)

- [x] `cargo test --workspace -- --test-threads=1` → **574 passed / 0 failed**; `cargo clippy … -D warnings` → 0; `cargo fmt --all --check` → 0; `check-foundation.sh` → 0.
- [x] `swift test --package-path apps/FootballAppCore` → **123 teszt / 19 suite**; `xcodebuild test …FootballAppTests` → **76 teszt, 0 failure**.
- [x] `./scripts/run-ui-tests.sh -parallel-testing-enabled NO` → **76 app + 9 UI teszt, 0 failure; TEST SUCCEEDED**.
- [x] `git diff --check` → 0.

### Visszaállás / korlátok

- `git revert` a szelet commitjára; nincs új migráció, nincs adatírás. A meccs-találatok csak 7 napos ablakból jönnek (költségkorlát, dokumentált).
- Az XCUITest `typeKey("f", .command)` nem jutott el a KeyboardMonitorhoz, ezért a UI-teszt a `nav.search` gombot kattintja; a ⌘F működését külön, valódi CGEventtel igazoltuk az élő appon.
- Nem ellenőrzött: a keresési találatok VoiceOver-beszédhangja; a sheet vizuális megjelenése képernyőképen.

## S104a — Liga F forrás-konfliktus és crosswalk (adatrevízió)

A 4. szelet első, legkisebb része. Nincs új kód, nincs migráció, nincs hálózati hívás — csak auditalt, visszafordítható adatrevízió.

### Változott (adat)

- [x] A gyökér nem a 11-21-i forrás volt: a soccerway capture-scoped `team_identity` megfigyelései a helyes `Eibar N–Atletico Madrid N` és `Logrono N–Alaves N` párokat adták, de az aktív `identity.source_entity_ref` sorok **2026-11-19 12:37-ből** (11-18-i capture) származtak, ahol ugyanazok a source-ID-k még `Levante N`/`Cacereno N` neveket hordoztak. Ez a crosswalk-elavulás okozta a „Logroño"/hamis ellenfél-jelenséget.
- [x] `footballctl identity repair-matches --from 2026-11-21 --to 2026-11-22 --apply` → **24 vizsgált, 8 már helyes, 14 átállítva, 0 merge, 0 visszatartva, 2 feloldhatatlan, 0 hiba**. A két Liga F sor ezután: `Eibar N–Atletico Madrid N` és `Logrono N–Alaves N`.
- [x] A két soccerway sor duplikáltja a research/FotMob által megerősített lezárt meccsnek (16:00Z ill. 18:00Z, `1–3`, ill. `3–1`, 12. forduló). Auditált `dedupe pair`:
  - `01a1fb69-cac2-…` → túlélő `01a1fa06-13ed-…` (merge `01a207b9-8c32-…`)
  - `01a1fb69-cac4-…` → túlélő `01a1fa06-13ee-…` (merge `01a207b9-8c51-…`)
- [x] A crosswalk-javítás két további, Serie C-beli duplikátumot is felszínre hozott; `footballctl dedupe matches --date 2026-11-21 --to 2026-11-22 --apply` → **2 csoport, 2 merge**, túlélők `01a1fa06-6b8e-…` és `01a1fa06-6b92-…`.

### Futott ellenőrzések (mért)

- [x] `integrity --from 2026-11-21 --to 2026-11-22` → a javítás közben ideiglenesen **FAIL** (2 duplikált kickoff/fixture, 1 azonos pillanat), a két Serie C merge után **OK** (0/0/0/0, 1 függő review, 1 withheld, 1 failed job, 1 nyitott breaker).
- [x] `reverify --from 2026-11-21 --to 2026-11-21` → **909 meccs, 0 hiba**, withheld **0** (előtte 2), 516 verified / 393 provisional.
- [x] `coverage-day 11-21` → **905 meccs / 848 lezárt / 0 withheld**; a Liga F nap API 4 sora közül a spanyol Liga F 3 meccs **mind finished** (2–1, 1–3, 3–1); a Liga F `open_past` 2 → **0**.
- [x] Audit: `identity_repair_match` **14** esemény; a `dedupe pair` és a `dedupe matches` merge-ek `ops.dedupe_archive`-ban, túlélő/áldozat payloaddal.

### Visszaállás

- Minden lépés auditált: a `repair-matches` átállításai az `ops.audit_event`-ben, a merge-ek az `ops.dedupe_archive.payload`-jában teljes előzménnyel rendelkeznek. Visszaállítás = az archív payloadból való visszaírás (nincs CLI undo), illetve az átállított `core.match` csapat-ID-k visszaállítása a `/tmp/s104-before-distinct.tsv` pillanatképből (25 sor).

## S105 — gyors operatív tételek + GB alrégió-policy

A felhasználó „mehetnek a lépések" felhatalmazására a nyitott pontok sorban jöttek.

- [x] **Karcag review**: a HU/senior/férfi csapatnak nem volt meglévő entitása → `identity approve-create` audittal (`01a2076a-c155-…` → `01a207bd-b604-…`). Pending review **1 → 0**.
- [x] **Soccerway breaker**: **szándékosan nem zártuk**. Mérés: 13 timeout, 382 `quota_blocked`, 3 success (3 nap), a forrás halott; a skill szerint ilyenkor a fail-closed nyitva tartás a helyes, a jobnak megvan a `MAX_TRANSIENT_ATTEMPTS=8` korlátja. A „failed job" a breaker tünete, nem külön hiba.
- [x] **GB alrégió policy** (`docs/decisions/2026-11-23-gb-subregion-competitions.md`): a 4 angol liga GB-SCT ghostját a meglévő `normalize_country_code` mintájára GB-ENG-re hoztuk. Előbb auditalt ország-revízió (`competition_country_revision`, 4 sor), majd 4 `identity merge-competition --apply` (0 meccs, 1-1 ütköző szezon, 1 alias, 1 ref; merge után 0 duplikátum). A valódi skót versenyeket és az NZ „National League"-t nem érintettük.
- [x] **AT maradvány**: a már merge-elt „Bundesliga" entitás elavult `core.competition` sora (0 meccs, 1 szezon) audittal archiválva és törölve (`competition_orphan_cleanup`).
- [x] `integrity --from 2026-11-01 --to 2026-11-30` → **OK**, pending review **0**, withheld **0**, duplikátumok **0**.

## S106 — tipplap replay (új képesség)

- [x] Új CLI: `footballctl tipplap-replay --from … --to … [--apply]` (`rust/bins/footballctl/src/tipplap_replay.rs`). Kizárólag a már tárolt, zstd-tömörített HTML-t olvassa az `ObjectStore`-ból — **nincs hálózati kérés**, ezért robots/kvóta/breaker érintetlen (AGENTS.md §7). A dátum-guardot nem kerüljük meg: a tárolt oldal saját selected optionját ellenőrzi a parser 1.1. Dry-run alapból; `--apply` esetén új `stage.observation` sorok + `verify_and_publish` materializáció.
- [x] Dry-run 11-18..11-20: **3 nap, 248 observation, 0 írás**. Apply: **248 observation / 136 új sor / 27 verseny materializálva / 11 új meccs**.
- [x] A replay 6 duplikátum-csoportot és 7 review-t hozott felszínre: `dedupe matches --apply` → **6 csoport / 6 merge**; a 7 review-ból 6 meglévő entitáshoz került (`approve --entity`, köztük a `fuzzy_candidate` „Gol Gohar Sirjan" kézi review-val, auto-fuzzy nélkül), 1 új entitás („FTC", HU női). Pending review újra **0**.
- [x] Ellenőrzés: `integrity 11-18..11-21` → **OK** (minden duplikátum-mutató 0, review 0, árva 0); `reverify 11-18..11-20` → **466 meccs, 0 hiba**; `dedupe matches` dry-run → **0 duplikátum**; `daily-check.sh` → **EXIT=0**.

### Visszaállás

- Az új observationök és meccsek azonosíthatók a replay idejéből (`/tmp/s106-before.txt` pillanatkép: obs 2714 → 2850, meccs 283808 → 283819); a merge-ek az `ops.dedupe_archive`-ban, a review-döntések az `ops.audit_event`-ben.

## S107 — soccerway hiba oka (read-only diagnózis + 1 engedélyezett render)

- [x] **Tárolt bizonyíték**: soccerway policy `obscura/canary`, `daily_budget=6`, `min_delay=60 s`, `breaker_threshold=3`, `identity_mode=fail_closed`. Utolsó sikeres **nap**-capture: **2026-11-21 01:00:58**, `soccerway:day:2026-11-21`, 3 779 887 B, 16 109 ms — azóta ~57 óra alatt **0 sikeres nap-render**. Az utolsó HTML dekomprimálva: **681 `data-event-row`**, 1 dayPicker, 0 decoy → a parser egészséges.
- [x] A friss hibák mind `FetchError::Timeout` a 20 mp-es marker-ablakban (22,2 s / 45,0 s), `http_error=0`, `robots_blocked=0` (a robots verdict `allowed`, friss cache). A hatalmas `quota_blocked` (605 élethosszig, 357 a 11-22-i UTC napon) a napi 6-os keret kimerülése utáni **szabályos**, ~1 kérés/perces visszasorolás, nem hammering. Breaker `open`, `failure_count=8` (a mai nyitásnak nincs audit-sora — megfigyelhetőségi rés).
- [x] **Döntő élő próba** (egyetlen, robots-kompatibilis render, adatírás nélkül): `OBSCURA_LIVE_TEST=1 cargo test -p football-ingest --test soccerway_live --features obscura` → **„nap-címke: 2026-11-23, sorok: 128, bájtok: 1 080 490"**, első slug `anglia/premier-league`, 254 csapat / 58 verseny; teszt **OK**, 10,12 s.
- [x] **Következtetés**: nem site-wide kiesés és nem parser-hiba — a **`route_for` által használt `/tegnap/` útvonal** renderelési hibája (amit a skill eleve megbízhatatlannak tart: 200-at adhat 0 sorral). A mai `/` útvonal működik, de a daemon a holnapi körben ismét csak a tegnapi napra tudna próbálkozni.
- [x] **Operátori döntés változatlan**: a breakert nem zártuk erővel, a kvótát nem emeltük, a két terminális jobot nem requeue-oltuk (AGENTS.md §7 + skill). A napi lista egyéb forrásokból kiszolgált (11-23: 117 meccs). A feloldás jövőbeli szelet: megbízható naptár-popup/dayPicker útvonal mérése (jelenleg nincs mérve) vagy a `/tegnap/` cseréje — külön döntés, mert a robots és a rate limit miatt nem lehet vakon kísérletezni.
- Nem ellenőrzött: a naptár-popup útvonal DOM-ja; a két terminális soccerway job opcionális auditált `cancelled`-re állítása.

## S108 — játékosadatok a helyi research forrásból

A felhasználó döntése: soccerway később; most a **helyi forrás**; a v1 DB és a kutatási rendszer kutatási kérdések. Ez a szelet a már tárolt research JSON-eket használja, hálózat nélkül.

### Változott

- [x] `db/migrations/0026_player_identity.sql` — `core.player` (entity_id, country_id, position_code, date_of_birth) és `core.player_team_spell` (időben érvényes tagság, loan, source, captured_at, UNIQUE kulccsal). Táblaszám 51 → **53** (`db-check.sh`, `connection.rs`, `test_support` TRUNCATE). Az első migráció a kvalifikálatlan index miatt bukott, javítás után `26|player identity` alkalmazva.
- [x] `rust/crates/ingest/src/research_players.rs` — a research detail JSON `lineups` / `player_statistics` / `shotmap` blokkjainak séma-drift-tűrő olvasója; a `player_id` lehet szám vagy szöveg (a 11-20-i korpuszban 9/617 szöveg). Csak numerikus stat-értékek (formázott szöveg → None, nem 0).
- [x] `footballctl player-import --from … --to … [--apply]` — kizárólag a már tárolt `api/v1/matches/**/_fm*.json` fájlokból; a játékosentitás **csak stabil `fm:<id>` forrás-ID-ból** jön létre (`exact_external_id`, nincs fuzzy merge, nincs név-alapú auto-create); minden fact-sorhoz `source_id` + `captured_at` (research `meta.generated_at`). Dry-run alapból; `--apply` idempotens (ON CONFLICT DO UPDATE), audit-eseményenként.

### Mért eredmény (dev DB, 2026-11-20)

- [x] Dry-run: **168 fájl / 168 meccs / 4 498 lineup / 64 849 stat / 966 lövés**.
- [x] Apply: **4 188 játékosentitás, 3 978 lineup, 64 849 stat, 966 lövés, 3 978 tagság-sor**. Újrafuttatás: **0 új duplikátum**, csak 1 korábban hiányzó lövés-player-link pótlódott (`player` név beolvasása után); `player_entity_id IS NULL` minden táblában **0**.
- [x] Minták: `Marcus Kirchheiner`, mez 10, rating 7,9; `Alexander Ludwig` — „Accurate passes 47", `captured_at=2026-11-22T11:44:31Z` (a research generálási ideje, nem a feldolgozásé).
- [x] `integrity 11-20` → **OK** (0 duplikátum, 0 árva, 0 review, 0 failed job).

### Visszaállás

- 0026 rollback a migrációban; az import sorai a `research` source_id + `captured_at` alapján azonosíthatók és törölhetők; a létrehozott entitások `identity.source_entity_ref`-ben `exact_external_id`-vel követhetők (`/tmp/s108-before.txt`: minden számláló 0 előtte).

## Kutatási eredmények (döntési alap, 0 írás)

- **v1 adatbázis** (read-only): `public` séma, **196 tábla**; `players` **36 678**, `match_player_stats` **820 752**, `match_shots` **133 351**, `match_lineups` **113 802**, `player_availability` **6 487**, `season_player_stats` **4 413**, player `external_id_map` **39 638 / 35 150 core**, `seasons` 7 498 (1888–2030). A statok ~95%-a 2020 utáni. **Kritikus blokk: nincs v1→v2 meccs-híd** (a v2 `v1-import` source-ref 608 team / **0 match**), a `season_player_stats`-ból hiányzik a `source`/`captured_at` (provenance nem található ki), 1 528 leképezetlen + 3 810 többszörösen leképezett játékos. Ez külön, több szeletes munka + licenc-/időszak-döntések.
- **Kutatási rendszer** (read-only): 5 448 detail fájl; a nyers FotMob adatban **6 673 eseményhez van `player.id`**, de az exporter **0-t** tesz ki belőle — a legkisebb hasznos bővítés az `events[].player_id/player_in_id/player_out_id` opcionális mező (`R/docs/api-export.py:726-731`, `R/docs/api-schema.py:91-94`), amit a v2 már meglévő `fact.match_incidents.player_source_id` oszlopa fogadni tud új migráció nélkül. **Az R-be írás külön jóváhagyás** (külső rendszer, export séma változás, újraexport).

## Következő döntési pont

- **Játékos API + UI**: a most betöltött lineup/stat/shots adatokhoz read model, `GET /v1/player/{id}` és játékosoldal kell (kontrakt-fixture, AX-címkék, UI-teszt). Erre külön szelet.
- **Több nap betöltése**: a 16 napos research korpusz (2026-11-13…11-28) dry-run mérése után `player-import` tartományra; döntés, meddig érdemes (a forrás csak friss ablak).
- **v1 játékos-import**: külön engedély kell a forrásokra (fotmob/statsbomb/transfermarkt licence), az időszakra (teljes 1958–2026 vs 2015+) és a meccs-hídra; a `season_player_stats` provenance-a nem található ki.
- **Kutatási export bővítése**: `events[].player_id` az R-ben — írási jog + újraexport szükséges (külön döntés).
- **soccerway**: dayPicker/-popup útvonal mérése később; a breaker jelenleg nyitva marad.
- Akadálymentességi maradék: menük/naptár/sheet vezérlői a Tab-körben, szűrők perzisztenciája, Reduce Motion/Cmd+R; VO-hang tudatosan kihagyva.
