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

## Következő döntési pont

A felhasználóval egyeztetendő, melyik fejlesztési szelet jön:

1. **(Ajánlott)** Időjárás a backendbe + valódi countdown — lezárja az S100 nyitott pontját, a kliens teljes egészében a `127.0.0.1` API-ról kap adatot.
2. Bíró-identitás policy + referee-ország séma — napi ~168 auto-create entitás helyett tudatos döntés (migrációval).
3. Fázis-2 kilépési feltételek (keresés, játékosoldal teljes adattartalma, VoiceOver/billentyűzet) — ez nyitja meg a fázis-3 live utat.
4. Adatminőség-maradék: competition-duplikátumok, Liga F forrás-konfliktus, tipplap replay.

Gyors operatív tételek, bármikor elvégezhetők: a „Karcag" review eldöntése és a soccerway breaker újrapróbálása/csendesítése.
