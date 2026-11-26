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

## S163 — „Lejárt, nyitott" szűrési rés (S131 lezárása)

- [x] **Probléma**: ma 3 meccs (AC Taipei–Hang Yuan 08:00Z, Borneo–Persita és South Melbourne–Oakleigh 08:30Z) már 12–102 perce kezdődött, de a forrás még `scheduled`. A „Következik" 5 perces tűrése miatt kiesett onnan, a „Lejárt, nyitott" 3 órás küszöbe miatt pedig nem jelent meg sehol — csak a „Mind"-ben volt látható.
- [x] **Javítás** (`MatchCardModel.swift`): `openPastThreshold` **3 óra → 10 perc**; az `isOpenPast` élő/félidős státusznál a `liveWindow` (150 perc) küszöböt használja, így az élő meccs addig az „Élő" szűrőben van, nem szerepel kétszer. A `date_only` kizárás (S153) változatlan.
- [x] **Tesztek frissítve**: `MatchCardModelTests` (12 perc → igaz, 2 perc → hamis, élő 2 óra → hamis, élő 3 óra → igaz, konstans = 600 mp) és `MatchFilterTests` (11:55 kezdés −5 perc, új −12 perces és élő-ablakos esetek). App unit **82/82 TEST SUCCEEDED**.
- [x] **Valódi macOS**: app újratelepítve, AX „Lejárt, nyitott" **0 → 3**, és a három meccs név szerint is megjelenik a szűrőben (South Melbourne–Oakleigh, Borneo–Persita, AC Taipei–Hang Yuan). A backend `open_past` jelzőszámlálója továbbra is 3 órás — ez egy diagnosztikai mező, a UI a meccsekből számol; az összehangolás daemon-restartot igényel, külön döntés.
- [ ] A `slot-00/01` frissességi kapuja és a backend `open_past` küszöbének összehangolása változatlanul nyitott.

## S164 — Valódi verseny-duplikátumok összevonása

- [x] **Kivizsgálás**: a két napra vetítve 4 átfedő aliasú pár volt; ebből 2 valódi duplikátum, 2 jogos (Serie C Girone B vs C = külön csoport; Liga MX vs Liga MX negyeddöntő = külön szakasz).
- [x] **Duplikátumok**: `UEFA Conference League` (research-only, 2 meccs) ↔ `Conference League` (72 meccs, openfoot+research, azonos aliasok); `UEFA Europa League` (1 meccs) ↔ `Europa League` (89 meccs, ugyanúgy). Mindkettőnél a régi, gazdagabb entitás a túlélő.
- [x] **Végrehajtás**: dry-run után `footballctl identity merge-competition --force --apply` kétszer, auditáltan (meccsek 2+1, ref 1+1, alias duplikátumok eldobva, minden áldozat archívumba). Az `integrity` 11-25/11-26-ra **verdict OK**, 0 duplikátum/árva.
- [x] **Mérés**: az API-n 11-26-on a `Conference League` **17+2 → 19 meccs egy csoportban**, az `Europa League` **17+1 → 18 meccs egy csoportban**; a „UEFA …" változatok 0 meccsesek. `daily-check 2026-11-25` **EXIT=0**.
- [ ] **Rendszerszintű hézag (nem javítva)**: a DB-ben több tucat hasonló, azonos országú és átfedő aliasú entitáspár él (pl. Belgian Pro League / Jupiler League, Serie B / Brazil Serie B, Regionalliga North / Nord, 1. CFL / Prva Crnogorska Liga, magyar/angol nevek). Tömeges automatikus összevonás az AGENTS.md §6 miatt **tiltott**; forrásalapú, páronkénti döntés vagy egy külön ingest-szabály (alias-egyezés országon belül) szükséges hozzá. Ebben a szeletben csak a két, a tegnapi/mai listát ténylegesen érintő párt oldottuk meg.

## S165 — Verseny-név probléma: forrásalapú azonosítás és összevonás

- [x] **Feladat**: a felhasználó kérésére a verseny-entitásokat nem automatikus fuzzy egyeztetéssel, hanem forrásalapúan kell eldönteni (helyi referencia + internet), majd az azonos ligákat összevonni.
- [x] **Módszer**: a DB-ből az azonos országú, átfedő aliasú párok listája (novemberben aktívak), majd páronként: webes/helyi ellenőrzés → dry-run → auditált `identity merge-competition --force --apply` (a gazdagabb, több meccsel/forrással rendelkező entitás a túlélő). Két bizonytalan webes kérdés tisztázva: **WSL Players Cup = Women's League Cup** (átnevezés), **dán A-Liga = női élvonal** (az „A-Liga Women" ugyanaz, azonos klubokkal).
- [x] **1. köteg (33 pár)**: 1. divízió→Ligue 1 (DZ), Elite One→Elite 1 (CM), Baller League UK, Mozzart Bet Prva Liga→Prva Liga (RS), Jupiler League→Belgian Pro League, Liga Portugal→Primeira Liga, Superligaen/Danish Superliga→Superliga (DK), J1 League→J. League, Pro League→Persian Gulf Pro League (IR), Stars League→Iraq Stars League, két UY-pár, A-League Men, V-League 1, Liga 2→Liga II (RO), Ukrainian PL, Crystalbet Erovnuli Liga, Prva Crnogorska Liga→1. CFL, két IN-pár, Parva Liga, 2nd Division (CY), két Regionalliga-pár (DE), Brazil Serie B, két Friendlies-pár, WSL Players Cup, Women's Liga MX: Apertura, MLS: Play-off 2026, A-Liga Women.
- [x] **2. köteg (12 pár)**: Liga Profesional 8th Finals→Clausura Playoff (AR), Second Professional League→Vtora Liga (BG), két Segunda RFEF/Federacion Group (ES), két Serie C Group/Girone (IT), J. League 2→J2 League, Serbian SuperLiga→Super Liga, Salvadoran Primera, USL Super League Women, UY Segunda playoff, Club Friendlies→Club Friendly.
- [x] **Gyökér-javítás**: a mai catch-up három duplikált meccset hozott vissza (11-19 U17 női Belgium–Svájc 6:2 és Portugália–Spanyolország 0:4; 11-21 A-League Women Newcastle–Adelaide). Mindháromnál a régi, gazdagabb sor lett a túlélő (`dedupe pair`, áldozatok `ops.dedupe_archive`-ban). A „Qualification Round 1: Group A4" verseny-entitást is az eredeti `UEFA U17 (női) - Qualification - League A` alá vontuk — különben a következő catch-up újra duplikáltat szült volna.
- [x] **Nem összevont, jogos elkülönítések**: férfi/női (Copa del Rey vs Copa de la Reina, NCAAM vs NCAAW Soccer), csoport-/szakasz-entitások (Serie C Girone A/B/C, EFL Trophy csoportok, Liga MX negyeddöntő, Premier League Group A/B LY, World Cup U17 Group B, Australian Championship csoportok). Ezek összevonása elvesztené a `round_label`/csoport-információt, illetve adatvesztés lenne.
- [x] **Kapuk**: mind a 46 competition-merge dry-run + apply hibamentes; `integrity` 11-25/11-26 **verdict OK**; **teljes november integrity újra OK** (a három dedupe pair után 0 duplikátum, 0 azonos pillanatú pár, 0 árva); `daily-check 2026-11-25` **EXIT=0**; API 11-26: 102 meccs / 34 verseny, Conference League 19 és Europa League 18 egy-egy csoportban.
- [ ] **Fennmaradó rendszerszintű hézag**: ~104 átfedő aliasú pár van még a DB-ben (ebből ~26 érinti a két napot, de jogos csoport/szakasz/női különbség). **208 verseny-entitásnak nincs egyetlen aliasa sem** (50-nek van meccse) — ezért a forrás új elnevezése új entitást hoz létre (l. U17 eset). Megoldás külön szelet: (a) `ingest/competitions.rs` szótár bővítése a megfigyelt változatokkal, (b) alias-backfill az örökölt entitásokra, (c) a catch-up újraingestelés utáni automatikus dedupe. Nem automatizáltuk — az AGENTS.md §6 tiltja a fuzzy merge-et.
- [ ] Az összes 104 pár nem lett egyenként webes forrással ellenőrizve; csak a 46 összevont pár és a két napot érintő 26 klasszifikációja készült el.

## S166 — Felhasználói észrevételek + korábbi nyitott pontok lezárása

- [x] **Konferencia Liga 19 → 18**: a felhasználó észrevette, hogy az Europa League 18, a Conference League 19 meccses. A 19. sor a **Gent–FK Crvena Zvezda** és a **KAA Gent–Red Star Belgrade** duplikátum volt (webes forrás: ESPN/UEFA/Yahoo — ugyanaz a meccs, 2026-11-26 21:00). A gazdagabb sort tartottuk meg (20 provenance, 16 odds quote, openfoot+research), a másik `dedupe pair` + audit + archívum. API: Conference League **18**, Europa League 18; integritás zöld.
- [x] **Odds nélküli meccsek kivizsgálása**: a research részletfájljaiban a Brann–AGF, Union St.Gilloise–Lech Poznań, Braga–KuPS és Ferencváros–NK Celje meccseknek **van** bet365 1X2 odds-a (08:06:13Z), de a 07:31/07:33-as ingest még a frissítés előtt olvasott. Egy `research-sync.sh 2026-11-26 --enqueue-only` job (`done`) után az API odds-száma **45 → 58**, hiány **57 → 44**, a négy meccs oddsot kapott. A **Kauno Žalgiris–Riga FC**-nek a forrásban (`markets: null`, `sources: []`) sincs odds — valódi forrásgap, nem hiba. A `fair_odds` (modellból számolt ár) tudatosan nem kerül be valódi bookmaker-oddsakként.
- [x] **Alias-backfill (S165 nyitott pontja)**: 207 örökölt verseny-entitásnak nem volt aliasa — ezeket a saját kanonikus nevükkel írtuk be (`status=reviewed`, `method=import`, semleges ország → NULL), 1 audit eseménnyel. Most **0** alias nélküli verseny van; a jövőbeli ingest alias-alapúan meg tudja találni őket.
- [x] **Backend `open_past` összehangolása (S163)**: `fetch_day_gaps` 3 óra helyett 10 perc küszöböt használ, élő/félidős sornál 150 percet — megegyezik a kliens `MatchCardModel` szabályával. Mérés: API 11-26 `open_past` **3**, 11-25 **14**, pontosan az app szűrőjével.
- [x] **Slot-frissességi kapu (S163)**: a research `slot-NN` refresh jobok cél-dátumát a `time_window`-ból olvassa (`research_refresh_target`), és ugyanaz a frissességi kapu vonatkozik rájuk, mint a catchup-ra (a forrás mtime-ja <= job created_at → 15 perces halasztás, legfeljebb 6 óra). Új regressziós teszt: `research_refresh_target_reads_slot_from_time_window_catchup_from_key`.
- [x] **Kapuk**: `cargo fmt --all --check` 0 · `cargo test -p football-worker -p football-storage -- --test-threads=1` mind zöld (a slot-teszt is) · `cargo clippy --workspace --all-targets -- -D warnings` 0 · `cargo build -p footballd -p footballctl --features obscura` OK · daemon újratelepítve/újraindítva, `/v1/health` OK · `daily-check 2026-11-25` **EXIT=0** · `git diff --check` 0.
- [ ] A slot-kapu **élőben** csak a következő daemon-induláskor (holnap 07:31) fog működni; a jobok naponta egyszer keletkeznek, ezért most nem volt frissíthető slot-job. A viselkedést az egységteszt igazolja.
- [ ] A `fair_odds` megjelenítése külön termékdöntés (jelenleg nem mutatjuk, mert nem valódi bookmaker-ár).

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
