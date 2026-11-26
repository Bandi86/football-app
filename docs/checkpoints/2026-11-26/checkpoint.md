# Checkpoint — 2026-11-26

## Induló állapot

- A stabil állapot a `docs/checkpoints/2026-11-25/checkpoint.md`-ben (S157 — utolsó lépés: bővítési terv vázlat) és a `README.md`-ben olvasandó.
- `git status`: `main...origin/main`, egyetlen nem commitolt fájl az RFC tervezet (`docs/superpowers/specs/2026-11-23-modern-predikcios-rendszer-terv.md`) — szándékosan hátrahagyva, nem érintettük.
- Felhasználói észrevétel: a tegnapi és a mai nap az appban „nagyon hiányos", sok helyen nincs eredmény vagy odds; a háttérfolyamatok látszólag nem futottak le. Feladat: bugolás folytatása.
- Felhasználói döntés (question tool): **teljes javítás** — daemon újraépítése/telepítése (launchd restart), egyszeri adatfrissítő jobok, záró ellenőrzés.

## S158 — Vizsgálat: két nap adatainak összehasonlítása

- [x] `./scripts/daily-check.sh 2026-11-25` **kiindulás**: technikailag ZÖLD (kemény integritási hiba nincs), de az adatok elavultak.
  - 11-25: 171 meccs, **58 lezárt / 58 eredmény**, 119 odds, **`open_past` 111**, 64 verified.
  - 11-26: 99 meccs, **1 lezárt / 1 eredmény**, **2 odds**, `open_past` 8, 12 verified.
- [x] Forrás-oldali mérés (`~/Library/Application Support/FootballResearch/api/v1/daily/2026/11/`):
  - 11-25 napi JSON (07:33-kor újraírva): 172 meccs, **145 lezárt**, 3 halasztott, 24 scheduled → a DB **~87 lezárt eredményt nem tartalmazott**.
  - 11-26 napi JSON (08:45-kor írva): 107 meccs, **11 lezárt**, 95 scheduled, 1 halasztott, 0 odds a daily JSON-ban.
- [x] Idővonal (helyi idő, Europe/Budapest): a daemon **07:31-kor** indult, a `catchup-2026-11-19…25` és a `slot-00/01` jobok azonnal `done`-ra futottak 1 kísérlettel — a **07:33/08:45 előtti** napi fájlokból. A research 08:00-s pipeline (12 perces lock-várakozással) csak 08:45-re írta ki a friss exportot; a következő job 13:00 (slot-02), illetve 14:15 (`research-sync`) lett volna.
- [x] **Gyökér (bináris szintjén bizonyítva)**: a telepített `agent/bin/footballd` (11-25 17:14) **nem a jelenlegi `main`-ből épült**. Az S139-es frissesség-kapu egyedi üzenete a repo 16:58-as buildjében **1**, a telepítettben **0** találat; méret 55 931 256 vs 55 905 112 byte. A catchup `due_at`-je is azonnali (07:31) volt, nem `now + 60 perc` (S136). Tehát a két versenyhelyzet-javítás **nem volt élesben**: a job nem várta meg a friss fájlt, és idempotensen lezárta magát.
- [x] **Odds-időzítés**: a 08:00-s research pipeline (`daily-0800.sh`) nem gyűjt odds-ot; az a `research-sync.sh` 14:15-ös körében fut (tegnap 16:25-kor érkezett a 119 odds). A mai 0 odds **forrás-időzítés**, nem serving-hiba.
- [x] A `daily-check` és a `footballctl` a repo friss binárisát használja (`rust/target/debug/`), ezért a mérés maga nem volt hamis — csak az ingest volt elavult.

## S159 — Daemon frissítése (jóváhagyott launchd restart)

- [x] Kapuk a telepítés előtt: `./scripts/check-foundation.sh` zöld (db-check dev/test, `cargo fmt --all --check`, domain+storage **59+** teszt, clippy 0).
- [x] `cd rust && cargo build -p footballd -p footballctl --features obscura` → `BUILD_OK`; a friss binárisban az S139-os kapu **1 találat**.
- [x] `./scripts/install-footballd-agent.sh` → új `footballd` telepítve 09:04:07-kor; `launchctl print … | grep state|pid` → `state = running`, `pid = 8213`; a log új `worker elindult` / `footballd elindult port=49372` sort írt, és a `worker tick` sorban megjelenik a `deferred=0` mező. `/v1/health` → `{"status":"ok","service":"footballd"}`.
- [x] Célzott teszt: `cargo test -p football-worker catchup_waits_until_the_source_file_is_fresh` → **1 passed, 0 failed**.

## S160 — Adatok utánpótlása (egyszeri discovery jobok)

- [x] `./scripts/research-sync.sh 2026-11-25 --enqueue-only` és `… 2026-11-26 --enqueue-only` → két új, idempotens `research` discovery job (`research-sync-2026-11-25-0904`, `research-sync-2026-11-26-0904`). A worker 60 mp-en belül mindkettőt `done`-ra állította, 1 kísérlettel.
- [x] **Mért eredmény (coverage-day, előtte → utána)**:
  - 11-25: 171/58/58/119/64 → **172/147/147/119/65** (meccs/lezárt/eredmény/odds/verified).
  - 11-26: 99/1/1/2/12 → **102/9/9/45/15**.
- [x] `reverify --from 2026-11-25 --to 2026-11-26`: 272 meccs, **0 hiba**, 192 provisional / 80 verified gate.
- [x] `integrity --from 2026-11-25 --to 2026-11-26`: **verdict OK** — 0 duplikátum, 0 árva provenance, 0 aktív ref összevont entitásra; 3 azonos napi pár (jogos), 208 pending review (fail-closed), 6 withheld (MATERIAL_SOURCE_CONFLICT).
- [x] Záró `./scripts/daily-check.sh 2026-11-25` **EXIT=0**:
  - API 11-25: 171 meccs (147 finished / 21 scheduled / 3 postponed), `open_past` **111 → 14**, `missing_odds` 52, `missing_kickoff` 7.
  - API 11-26: 102 meccs (9 finished / 92 scheduled / 1 postponed), `missing_odds` **97 → 57**, `open_past` 0, `missing_kickoff` 2.
  - `[OK] soccerway tiltva`, `verified gate (11-25): 65`.
- [x] `git diff --check` 0; a két régi failed job (`catchup-2026-11-22` hiányzó fájl, `slot-03` 11-24/25 dátum-guard) nem mai adat, nincs elnyomva.

## S161 — A kliens elavult pillanatképe (felhasználói észrevétel)

- [x] **Probléma**: a felhasználó az appban továbbra is a régi számokat látta (Mind 170 / Befejezett 58 / Lejárt nyitott 111), ezért azt gondolta, hogy a build nem történt meg. Az API ellenőrzése viszont friss volt (147 lezárt mindkét napra). A kézi `Frissítés` gomb megnyomása után az app azonnal a friss adatot mutatta (147 lezárt / 21 lejárt nyitott) — tehát **nem build-hiba, hanem kliens-frissítési hiány**.
- [x] **Gyökér** (`HomeView.swift`): a napi lista csak `.task(id: model.selectedDate)` esetén tölt adatot; a csendes háttér-körnek volt egy `guard liveMatchCount > 0` feltétele, és csak a **mai** napra indult. Tegnapra (vagy élő meccs nélküli ma-i napra) tehát **egyszer sem frissített**, amíg a felhasználó nem váltott napot vagy nem nyomott Frissítést.
- [x] **Javítás**: a háttér-kör ezentúl minden kiválasztott napon fut; az intervallumot a Core `AutoRefreshPolicy` számítja — élő meccsek esetén **60 mp**, egyébként **300 mp**. A `.task(id: autoRefreshKey)` változatlanul újraindul napváltáskor és az élő meccsek számának változásakor, tehát az intervallum is követi az állapotot. Új fájl: `apps/FootballAppCore/Sources/FootballAppCore/AutoRefreshPolicy.swift` + 3 teszt.
- [x] **Kapuk**: Core **128/128** (3 új teszt: 0 élő → 300, 1/12 élő → 60, negatív → 300); app unit **82/82 TEST SUCCEEDED**; `xcodegen generate` + `xcodebuild … -only-testing:FootballAppTests` zöld; `git diff --check` 0.
- [x] **Telepítés / valódi macOS ellenőrzés**: `./scripts/install-football-app.sh` → **BUILD SUCCEEDED**, az app binárisa 11-25 17:14-ről **11-26 09:21:53**-ra frissült; az app újraindítva (PID 11126). AX: ma (11-26) **Mind 102 / Befejezett 9 / Lejárt nyitott 0 / Odds nélkül 57**, tegnap (11-25) **Mind 171 / Befejezett 147 / Lejárt nyitott 14 / Idő nélkül 7 / Odds nélkül 52** — mindkettő pontosan egyezik az API-val.
- [ ] A 5 perces időzítő **élőben nem lett végigvárt**; a viselkedést a Core egységtesztje és a telepített kód igazolja, a hosszú wait nem futott le.

## S162 — Részletes debug: elveszett UI + azonos nevű ligák (felhasználói észrevétel)

- [x] **Probléma**: a felhasználó az S161-es telepítés után a **régi, nagy fejlécet** látta, holott tegnap már a kompaktkinézettel jelent meg az app; egyben „többször fordul elő ugyanaz a tournament külön" és hiányosságokat látott a mai adatokban.
- [x] **UI-gyökér**: a kompakt fejléc + követhető meccslánc a `feat/match-chain-compact-header` branch **`14de60d`** commitjában van, és **nincs a `main`-ben** (`git branch --contains 14de60d` csak a brancht mutatja; közös ős `a8ce36e`, a main 27 commitja nincs a branch-en). A tegnap 17:14-kor telepített app a branch **worktree-jéből** épült (ott további 7 nem commitolt UI-fájl van). Az S161-es 09:21-es telepítés a `main`-ről történt, ezért revertálódott a régi kinézet. **Nem build-hiba, hanem forrás-ágtévesztés.**
- [x] **Felhasználói döntés**: a branch commitját összevonjuk a `main`-be és újraépítjük az appot; a worktree-k nem commitolt munkáját **nem** vesszük át. A második kérdés (azonos nevű ligák) tool-hiba miatt nem került eléd — az ajánlott „maradjon így" alapértelmezés érvényesült, adatváltozás nem történt.
- [x] **Összevonás**: `git merge --no-ff feat/match-chain-compact-header` → commit **`ed72a8f`**. A `merge-tree` előre jelezte az egyetlen konfliktust (11-25-i checkpoint); a merge automatikusan feloldotta, **mindkét szakasz megmaradt** (`## Frontend …` az S134 előtt, a többi S-szakasz érintetlen). Az S161-es `AutoRefreshPolicy` és a `HomeView`-javítás a merge után is él (grep: `AutoRefreshPolicy` a HomeView 133. sorában).
- [x] **Adat-debug — 11-25**: research 172 meccs (145 lezárt / 3 halasztott / 24 scheduled) ↔ DB 171 (147 lezárt / 21 scheduled / 3 halasztott). Hiányok: `missing_odds 52`, `missing_kickoff 7`, `open_past 14`, competition logo 138. A fennmaradó scheduled meccsekre a **forrás sem ad eredményt** — valódi forrásgap.
- [x] **Adat-debug — 11-26**: research 107 (11 lezárt / 95 scheduled / 1 halasztott) ↔ DB 102 (9 lezárt / 92 scheduled / 1 halasztott). Hiányok: `missing_odds 57`, `missing_kickoff 2`, `open_past 0`, competition logo 89, team logo 17. A 92 scheduled délelőtt normális; az odds egy része a 14:15-ös research körben jön.
- [x] **„Duplikált tournament" kivizsgálva — NEM hiba**: a mai `Premier League` öt **külön ország** ligája — Líbia (halasztott), Botswana, Kenya (3 meccs), Jamaica, Taiwan. A tegnapi Ukrajna/Botswana/Kenya/Egyiptom/Taiwan. A `Cup` szintén két ország: Szlovénia (2 lezárt) és Ciprus (8 scheduled). A `LeagueGroup.swift:179-191` **ország + versenynév** kulccsal csoportosít, a fejléc második sora kiírja az országot (`KENYA`, `BOTSWANA`, …) — az adat helyes, csak vizuálisan félrevezető. Országok közötti összevonás **tiltott** (külön bajnokságok, adatvesztés).
- [x] **Kapuk a merge után**: Core **131/131** (3 új AccessibilityLabels-teszt a branchből); app unit **TEST SUCCEEDED**; UI-harness **16/16, 0 hiba** (a branch új meccslánc-tesztjeivel együtt); `./scripts/daily-check.sh 2026-11-25` **EXIT=0**, `verified gate 65`, `[OK] soccerway tiltva`; `git diff --check` 0.
- [x] **Telepítés / valódi macOS**: `./scripts/install-football-app.sh` → **BUILD SUCCEEDED**, app bináris **11-26 09:37:35**, újraindítva. Képernyőkép (`/tmp/fb-compact-header.png`): a fejléc **egy sorban** (dátum + dátumvezérlők + meccs/élő/odds/weather/countdown) = kompakt kinézet visszaállt. AX: ma `Mind 102 / Befejezett 9 / Lejárt nyitott 0 / Odds nélkül 57` — egyezik az API-val.
- [ ] A match-chain-interakciókat a UI-harness ellenőrizte (16/16); **emberi kattintásos** részletező-nyitogatás nem futott külön. Az S161-es 5 perces időzítő élő végigvárása továbbra sem történt meg.
- [ ] A három másik worktree (`estimated-match-clock`, `match-detail-navigation`, és a match-chain worktree további 7 nem commitolt fájlja) érintetlen — ezek más ágensek munkái, külön átadás/döntés tárgyai.

## Döntések / korlátok

- [ ] **Maradék hézag = forrás, nem pipeline**: 11-25-ön 21 scheduled és 52 odds nélküli meccs; 11-26-on 92 scheduled (délelőtti mérésnél normális) és 57 odds nélküli meccs. Az odds egy része a 14:15-ös `research-sync` körben érkezik — **nem indítottuk kézzel**, a felhasználói döntés szerint az ütemezett körre marad.
- [ ] **Fennmaradó strukturális rés**: az S139-es frissesség-kapu csak a `catchup-<dátum>` refresh jobokra vonatkozik. A `slot-00/01` jobok továbbra is azonnal futtak daemon-induláskor a régi napi fájlra; a **mai** nap frissítése így 13:00/14:15 előtt nem garantált. Új kód-szelet (slot jobok frissesség-kapuja + teszt) külön döntés — most nem terjeszkedtünk rá.
- [ ] A telepített daemon telepítési ideje (11-25 17:14) és a forrás-build (16:58) eltérése arra utal, hogy egy másik worktree régi binárisa került telepítésre. A pontos eredet nem nyomoztuk ki; a jelenleg telepített bináris a `main`-ről származik, ellenőrizve a kapu-szöveggel.

## Visszaállás

- Bináris: a korábbi telepített példány `agent/bin/footballd.bak-1617` (régi, csak lokális mentés); bármely git-commitból újraépíthető és telepíthető az `install-footballd-agent.sh`-vel.
- Adat: a két discovery job újraenqueue-je idempotens; az új obszervációk a research napi JSON-okból újratermelhetők, nem kézi revízió. Nem történt törlés, merge vagy kényszerített git művelet.

## Következő döntési pont

- 14:15 után újra mérni a 11-26-os odds-lefedettséget; ha a `research-sync` nem fut vagy nem hoz odds-ot, akkor forrás-oldali kivizsgálás.
- Döntés a `slot-00/01` frissesség-kapujáról (l. fennmaradó strukturális rés), majd a `docs/operations/remaining-gaps.md` frissítése.
- A `2026-11-25-app-oldalak-bovites-design.md` vázlat 10. szakaszának döntései változatlanul nyitva.
