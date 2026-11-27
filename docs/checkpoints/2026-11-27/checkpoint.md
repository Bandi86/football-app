# Checkpoint — 2026-11-27

## Induló állapot

- A stabil állapot a `docs/checkpoints/2026-11-26/checkpoint.md`-ben (S185 — az
  adatmegjelenítési terv S0–S8 szeletei kész, S9 külön döntésre vár) és a
  `README.md`-ben olvasandó.
- `git status`: `main...origin/main`, branch `main`. Az RFC-spec
  (`2026-11-23-modern-predikcios-rendszer-terv.md`) továbbra is nem commitolt,
  érintetlen.
- Felhasználói észrevétel: „a filter nem jelzi rendesen, ha hiányzik az eredmény"
  — a tegnapi (11-26) és mai (11-27) adat, illetve a szűrők átvizsgálata.

## S186 — Napi check + szűrővizsgálat (read-only, adatot nem írt)

- [x] `./scripts/daily-check.sh 2026-11-26` → **EXIT=0, ZÖLD**.
  - 11-26: 104 meccs / 101 lezárt / 101 eredmény / 91 odds; hiányok:
    `missing_team_logo 16`, `missing_competition_logo 48`, `missing_odds 13`;
    `open_past 0`, `missing_result 0`, `missing_kickoff 0`. Integritás OK,
    reverify 102 meccs / 0 hiba, verified gate 14.
  - 11-27: 232 meccs / 4 lezárt / 4 eredmény / 93 odds; hiányok:
    `missing_team_logo 66`, `missing_competition_logo 166`, `missing_odds 139`;
    `open_past 0`, `missing_result 0`. A legkorábbi kezdés 08:30Z (10:30 Budapest),
    tehát a méréskor (07:52Z) még egy meccs sem járt le — a 227 „Következik"
    rendben.
- [x] A `gaps` mező és a kliens szűrői **meg egyeznek** mindkét napon. Az API
  `/v1/day` adatából a Swift `MatchFilter` szabályait utánozva számolt darabszámok
  és a futó app AX-címkéi azonosak: 11-26 → Mind 104 / Befejezett 101 / Eredmény
  nélkül 0 / Odds nélkül 13; 11-27 → Mind 232 / Következik 227 / Befejezett 4 /
  Eredmény nélkül 0 / Odds nélkül 139.
- [x] **A gyanús filter-hiba nem igazolódott**: a `missingResult` szűrő definíciója
  (`finished` és nincs végeredmény) következetesen van implementálva a kliensben
  (`MatchListView.swift`, `MatchCardModel.swift`) és a backendben
  (`storage/repositories/day.rs`: `status='finished' AND home_ft IS NULL`); a
  `home_score` API-mező az `r.home_ft`-ből jön. A 11-26-os hat 0:0-s lezárt meccs
  eredménye a research forrásban is 0:0 (FT + félidő adattal), nem placeholder.
- [x] **Amit a felhasználó látott — elavult pillanatkép, nem szűrőhiba**: a futó app
  eleinte 103 meccset / 100 lezártat / 1 „Idő nélkül"-t mutatott, miközben az API
  már 104 / 101 / 0-t. A „Frissítés" gomb AXPress-szel azonnal az API-val egyező
  számokra állt. A napi lista 5 perces csendes háttérkörét a `.task(id:)`
  indítja; a képernyő gyors ellenőrzése nem mutatott újraindulást a régi napon —
  **nem zártuk le**, hogy a timer háttérben/szuszpendálva miért nem frissített.
- [x] Kapuk (mért): `MatchFilterTests` + `MatchCardModelTests` → **27 teszt,
  0 hiba, TEST SUCCEEDED**; `daily-check 11-26` EXIT=0; `psql` read-only
  (1 `abandoned` meccs 2026-12-09-re, nem a két érintett napon; a 2 failed job
  a régi `catchup-2026-11-22` és `slot-03`, nem mai keletkezésű).
- [ ] **Nem ellenőrzött**: az 5 perces háttér-timer viselkedése hosszabb
  háttérben tartózkodás után; a tényleges VoiceOver-hang (a korábbi checkpointok
  óta nyitott); a `missing_result` hiányjel-kód az `ops.issue` detektorban
  jelenleg **nincs** (`OpsIssueModels`/`detect.rs` — ha az eredményhiányt is a
  hiánypanelbe szeretnénk, ez külön szelet).

## Döntések / korlátok

- Az `Eredmény nélkül` szűrő a lezárt, de eredmény nélküli meccseket jelenti —
  **nem** a még el nem kezdett vagy lejárt, nyitott meccseket (azok a
  `Következik`, illetve `Lejárt, nyitott` szűrőben vannak, S163). Ez a
  megkülönböztetés tudatos, mind a backend, mind a kliens így viselkedik.
- A napi check a hiányjeleket írja (`ops.issue`); 11-26-ra 61 jel rögzítve
  (48 logó / 13 odds), 0 lezárás. Ez nem szűrőhiba, hanem a detektor normal
  működése.

## S187 — Aktíváláskori frissítés (a felhasználó által választott szelet)

- [x] **Döntés:** a felhasználó a mai első szeletnek az „elavult kliensfrissítést"
      választotta. A kivizsgálás szerint a kliensnek **nincs** alkalmazás-aktíválódási
      frissítése: csak a `.task(id: autoRefreshKey)` 5/1 perces `Task.sleep` köre
      fut, ami háttérben/szuszpendálva nem garantált. A felhasználó által látott
      elavult számok (103/100/1 ↔ API 104/101/0) pontosan egy ilyen kihagyott
      körnek felelnek meg; kézi „Frissítés" után azonnal korrigált.
- [x] **Core:** `AutoRefreshPolicy` kibővül `AppLifecyclePhase`-szel és a
      `shouldRefreshOnActivation(from:to:)` tiszta döntéssel (background/inactive →
      active = igaz; minden más = hamis). 2 új teszt; Core **155 passed, 0 failed,
      27 suite** (előtte 153/26).
- [x] **Kliens:** `HomeView` figyeli a `@Environment(\.scenePhase)`-t, és
      aktíválódáskor `model.refresh()`-t indít (nem villant betöltőt, nem dobja el
      a meglévő listát); a SwiftUI `ScenePhase` → Core `AppLifecyclePhase`
      leképezés privát extensionben, `@unknown default`-del. Az eredeti 5/1 perces
      kör változatlanul fut.
- [x] **Kapuk (mért):** `db-reset.sh` (TEST_DATABASE_URL-t sourcolva) →
      `check-foundation.sh` EXIT=0 · Core **155/0** · app unit **94/0 TEST
      SUCCEEDED** · `run-ui-tests.sh` **31/31, 0 hiba** · Rust `cargo test
      --workspace -- --test-threads=1` **635 passed, 0 failed** · clippy
      `-D warnings` **0** · `fmt --all --check` **0** · build `--features obscura`
      **OK** · `daily-check 2026-11-26` **EXIT=0 ZÖLD** · `git diff --check` **0**.
- [x] **Valódi macOS:** `install-football-app.sh` → **BUILD SUCCEEDED**; az app
      újraindítva (PID **11199**); Finder→FootballApp háttér/előtér ciklus után
      PID stabil, AX: `Mind 232 / Következik 227 / Befejezett 4 / Eredmény nélkül 0`
      — változatlan, egyezik az API-val.
- [ ] **Nem ellenőrzött**: az aktíválás által kiváltott hálózati kérés **közvetlenül
      nem lett mérve** (a daemon nem naplózza a kéréseket, a fejléc nem mutat
      „utolsó frissítés" időt); a döntést a Core egységtesztje és a bekötött kód
      igazolja. A 11-26-i adat időzítése (103→104 a daemon indulása után) nem
      lett pontosan audit-eseményhez kötve — a javítás a tünetet függetlenül
      orvosolja. A tényleges VoiceOver-hang és az 5 perces kör hosszú háttérben
      tartózkodás utáni viselkedése továbbra is nyitott.

### Visszaállás (S187)

- `git revert <S187 commit>` — a Core döntés és a `HomeView`-bekötés megszűnik;
      az API, az adatbázis és a research slotjai érintetlenek.
- Az app bármely commitból újraépíthető az `install-football-app.sh`-vel.

## Következő döntési pont

- A felhasználó döntése a következő szeletről: (a) `missing_result` hiányjel-kód
  a detektorban; (b) a `feat/estimated-match-clock` worktree becsült órájának
  átvétele; (c) a két régi failed job rendezése; (d) S9 AI-gomb külön döntése.
