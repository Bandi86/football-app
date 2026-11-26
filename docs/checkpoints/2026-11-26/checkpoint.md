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

## S168 — Bővítési terv döntései + S1 (adatmodell és API alap)

- [x] **Döntések**: a `2026-11-25-app-oldalak-bovites-design.md` 10. szakaszának
      mind a 6 döntését a felhasználó elfogadta (ajánlott alapértelmezések):
      írás csak ops token; új egységes `edit` végpont a régi megőrzésével
      (S4); az `ops.issue` perzisztens, a detektor írja (S3); jegyzetek
      read tokennel olvashatók, de nem a napi feedben; WebP/PNG csak az
      első szeletben; AI-gomb alapból ki. Rögzítve:
      `docs/decisions/2026-11-26-app-oldalak-bovites.md`.
- [x] **Terv**: `docs/superpowers/plans/2026-11-26-app-oldalak-s1.md` — S1
      (adatmodell + API alap), és kifejezett korlát: **S1 után megállok,
      jelzem**, S2-re nem lépek tovább.
- [x] **Séma**: `db/migrations/0028_ops_issues_notes.sql` — `ops.issue`
      (natural-key részleges unique, first/last_seen, resolved, source),
      `ops.match_note` (FK `core.match`, típusok, audit), `ops.asset` új
      oszlopai (`origin/uploaded_by/original_filename/mime_type`). Indoklás:
      az `ops.validation_finding` futásonkénti nyers eredmény, az `issue`
      futásokon átívelő életciklus — nem duplikáljuk a séma táblát.
- [x] **Infrastruktúra**: `scripts/db-check.sh` 54 → **56** tábla + a teszt-DB
      üresség-ellenőrzése a két új táblára; `test_support.rs` TRUNCATE-listája;
      `rust/crates/storage/tests/connection.rs` 54 → 56.
- [x] **Domain/storage**: `domain/src/ops.rs` (4 enum + `validate_note_*`,
      4 teszt), `repositories/{issue,note,history}.rs`, 5 storage-integrációs
      teszt (upsert-idempotencia, first/last_seen, resolve/újranyitás, day/
      code/severity szűrés, jegyzet-CRUD, audit+provenance történet).
- [x] **API**: 5 új művelet 4 új útvonalon — `GET /v1/issues` (Read),
      `GET|POST /v1/match/{id}/notes` (Read/Ops), `PATCH /v1/note/{id}` (Ops),
      `GET /v1/match/{id}/history` (Read). Magyar 400/401/404/500, minden
      írás `note_added`/`note_edited` audit-eseménnyel. OpenAPI 15 → **19**
      dokumentált útvonal, új `hiány`/`jegyzet` tag-ek; a bearer-teszt és a
      route-oracle POST/PATCH-ra is kiterjesztve. Új közös kontrakt-fixture
      `tests/fixtures/contract/issues-response.json` + Rust round-trip.
- [x] **Migráció sorrend**: előbb a teszt DB (`sqlx migrate run` → 56 tábla,
      zöld célzott tesztek), csak utána a dev DB (56 tábla, `integrity OK`).
- [x] **Kapuk (mért)**: `db-reset.sh` + `check-foundation.sh` **OK**
      (dev 56 tábla/6 szabály, test 56/6/0 üzleti sor);
      `cargo test --workspace -- --test-threads=1` → **609 passed, 0 failed,
      112 suite**; `cargo clippy --workspace --all-targets -- -D warnings`
      **0 hiba**; `cargo fmt --all --check` **0**;
      `cargo build -p footballd -p footballctl --features obscura` **OK**;
      `./scripts/daily-check.sh 2026-11-25` **EXIT=0, ZÖLD** (élő adat:
      171 meccs, 147 lezárt, 65 verified, `open_past 9`, `missing_odds 44`,
      `[OK] soccerway tiltva`); `footballctl integrity --from 2026-11-25
      --to 2026-11-26` → **verdict OK** (0 duplikátum, 0 árva, 0 aktív ref
      összevontra); `git diff --check` **0**.
- [ ] **Nem ellenőrzött**: a hiányjelek detektora még nem ír — élesben a
      `GET /v1/issues` jelenleg üres listát ad, ez **nem** működő hiányjelzés.
      A telepített daemon nincs újratelepítve (S1-ben nincs UI, S2-ben
      kötelező az `install-footballd-agent.sh`). Swift DTO/kliens nem
      változott. A napi check számai a reggeli checkpoint óta élő adattól
      függetlenül változtak (`open_past 3 → 9`, `missing_odds 44`) — ez nem
      a séma-változás mellékhatása, a napi ciklerek írták át.
- [ ] **Következő lépés**: a felhasználó döntése az S2 (Üzemeltetés UI) indulásáról;
      a detektor bekötése S3, a teljes meccs-szerkesztés S4, logófeltöltés S5.

## S169 — S2: Üzemeltetés — állapot, hiányjelek, meccs-idővonal

- [x] **Döntések**: S2 megy S3 előtt (felhasználói „mehetünk"); a globális
      audit-feed **nem** kerül S2-be (csak a meccs-szintű `/history`), S3-ban
      jön. S2 után itt is megállok. Terv:
      `docs/superpowers/plans/2026-11-26-uzemeltetes-ui.md`.
- [x] **Core**: `OpsIssueModels.swift` (IssueSubjectKind/IssueSeverity/IssueCode/
      HistoryOrigin/JSONValue + DTO-k, ismeretlen enum → `.unknown/.other`),
      `OpsFormatting.swift` (magyar kód/súly/forrás/tárgy címkék, hu_HU idő),
      `AccessibilityLabels` új címkék + azonosítók, `HTTPOpsService`
      `issues(query:)` és `history(matchID:limit:)` query-paraméterekkel.
      Tesztek: `IssuesContractTests` (közös fixture), `OpsIssueModelsTests`,
      `HTTPOpsServiceTests` (saját stub protokollal, hogy ne ütközzön a másik
      suite globális handlerjével), bővített `AccessibilityLabelsTests`.
- [x] **App**: `OpsModel` új `issuesState`/`query`/`selectedIssue`/`historyState`
      generációs versenyvédelemmel; `OperationsView` kattintható kártyákkal,
      szűrőkkel (Picker + Toggle), hiánylistával és issue-ból nyitott
      meccs-idővonal lappal (`before → after` megjelenítéssel). Üres/hiba/
      betöltés állapotok külön szöveggel. `OpsModelTests` kibővítve (9 teszt).
- [x] **UI-harness**: `ui-fixture-server.py` új útvonalai
      (`/v1/ops/status`, `/v1/issues`, `/v1/match/{id}/history` a generic
      detail elõtt), új fixture-ök `tests/fixtures/ui/ops-*.json`;
      `run-ui-tests.sh` mindhárom portra átadja őket; a teszt `runtime.json`
      most ops tokent is tartalmaz. Az UUID azonosítók NAGYBETŰSEK —
      ezért kellett korrigálni a kereső azonosítókat.
- [x] **Kapuk (mért)**: Core `swift test` → **143 passed, 0 failed, 24 suite**
      (S1-ben 140); app unit `xcodebuild … -only-testing:FootballAppTests`
      → **87 tests, 0 failures, TEST SUCCEEDED**; az új UI-tesztek izoláltan
      **3/3 TEST SUCCEEDED**; `daily-check 2026-11-25` **EXIT=0, ZÖLD**;
      `git diff --check` **0**.
- [x] **Valódi macOS ellenőrzés (fixture-szerverrel)**: `ax-press` Üzemeltetés
      fül → a kártyák és a két hiányjel magyar címkével megjelennek
      (`/tmp/fb-s2-ops-history.png`); az első jel AXPress-szel megnyitotta az
      idővonalat: `manual_match_edit` `status: scheduled → finished`, és
      `mező kiválasztás` `kickoff_at / research` látszik. Telepített daemon
      (pid **44797**, port **49372**): `/v1/health` OK, `GET /v1/issues`
      `{"issues":[],"count":0}` — a detektor S3-ban jön. Telepített app:
      BUILD SUCCEEDED, élő AX: kártyák + „Még nincs rögzített hiányjel. A napi
      ellenőrzés a jeleket ide rögzíti." (`/tmp/fb-s2-live-ops.png`).
- [ ] **PIROS kapu — nem regresszió**: a teljes `run-ui-tests.sh` **19 tesztből
      13 zöld / 6 piros**. A hat bukás mind régi teszt (keresés, meccs-részletező,
      játékosadatlap); A/B-vel ellenőrizve az S1-es `1e41241` worktree-ben
      ugyanez a `testMatchDetailShowsRichSections` **ugyanazzal a hibával bukik**
      (`az odds-mátrix nem jelent meg`) — tehát környezeti XCUITest-input probléma,
      nem az S2 által okozott. Az S2 három új UI-tesztje izoláltan zöld.
- [ ] **Nem ellenőrzött**: a szűrők billentyűzetes/egéres kattintása emberileg
      nem végigkattintva (csak a fixture-teszt fedi); a tényleges VoiceOver-hang
      (a felhasználó korábban kihagyta); a globális audit-feed (S3);
      a teljes UI-harness zöldre hozása környezeti beavatkozást igényel.
- [x] **Commit**: a felhasználó 2026-11-26-on kifejezetten engedélyezte a
      commitot a részben PIROS régi UI-kapu ellenére (az A/B bizonyította,
      hogy nem regresszió). A commit a zöld kapukkal commitolt kódot rögzíti;
      a piros régi UI-teszt környezeti hibája továbbra is nyitva marad.

## S170 — S3: detektor, globális audit és magyar összefoglaló

- [x] **Terv**: `docs/superpowers/plans/2026-11-26-uzemeltetes-s3.md`, két
      albontással (backend, kliens). S3 után itt is megállok.
- [x] **Backend**: `repositories/detect.rs` (napi hiányok meccs-szinten a
      `fetch_day_gaps` SQL-jéből + withheld gate + pending review),
      `footballctl issues detect --date … [--apply]` (dry-run alapból, upsert +
      lezárás + `issues_detected` audit), bekötve a `daily-check`-be
      (nem-blokkoló hívás, látható hibajelzéssel).
- [x] **Új végpontok**: `GET /v1/ops/audit` (Ops scope, szűrők, limit 1–200),
      `GET /v1/ops/summary` (Read scope, magyar mondat + kiemelt akciók),
      `POST /v1/issue/{id}/resolve` (Ops scope, idempotens, `issue_resolved`
      audit). OpenAPI 19 → **22** útvonal, új sémák regisztrálva.
- [x] **Kliens**: `OpsSummaryModels.swift`, `OpsService` 3 új művelettel,
      magyar címkék, `OpsModel` summary/audit/resolve állapotok,
      `OperationsView` összefoglaló sáv + globális audit szekció + „Jel
      megoldottnak jelölése" akció; `ui-fixture-server.py` és
      `run-ui-tests.sh` 2 új fixture-öttel, 2 új UI-teszt.
- [x] **Kapuk (mért)**: Rust workspace **613 passed, 0 failed**;
      `clippy -D warnings` **0**, `fmt --check` **0**, build `--features obscura`
      **OK**; Core **146 passed, 0 failed, 25 suite**; app unit **90 tests,
      0 failures**; az S2+S3 öt új UI-tesztje izoláltan **5/5 zöld**;
      `daily-check 2026-11-25` **EXIT=0 ZÖLD**; `integrity 11-25…11-26`
      **verdict OK**; `git diff --check` **0**.
- [x] **Éles adat és valódi macOS**: a detektor dry-run és `--apply` is
      **205 jelet** talált 11-25-re (139 hiányzó logó / 52 odds / 14 lejárt
      nyitott), 0 lezárás, audit-eseménnyel; a `daily-check` újrafuttatása
      idempotens (205 jel, 0 lezárás). Telepített daemon (port 49372):
      `/v1/ops/summary?date=2026-11-25` → 205 nyitott jel, magyar címkék;
      `/v1/ops/audit` → `issues_detected` események. Telepített app:
      `/tmp/fb-s3-live-ops.png` — összefoglaló sáv, 205 hiányjel kártya,
      és AX-fában a globális audit `issues_detected` sorai magyar címkével.
- [ ] **Nem ellenőrzött / nyitva**: a teljes UI-harness továbbra is 6 régi
      teszten piros (S169 A/B-bizonyíték, környezeti XCUITest-input hiba);
      az összefoglaló alapértelmezés szerint a **mai** napra vonatkozik (az
      Üzemeltetés oldalon nincs dátumválasztó), ezért élőben „Nincs nyitott
      hiányjel ezen a napon / Előző nap: 205" látszik — ez nem hiba, de
      félrevezető lehet; dátumválasztó külön döntés.
- [ ] **Következő lépés**: S4 (teljes meccs-szerkesztés, `PUT /v1/match/{id}/edit`)
      vagy a summary dátumválasztója — felhasználói döntés.

## Döntések / korlátok

- [ ] **Maradék hézag = forrás, nem pipeline**: 11-25-ön 21 scheduled és 52 odds nélküli meccs; 11-26-on 92 scheduled (délelőtti mérésnél normális) és 57 odds nélküli meccs. Az odds egy része a 14:15-ös `research-sync` körben érkezik — **nem indítottuk kézzel**, a felhasználói döntés szerint az ütemezett körre marad.
- [ ] **Fennmaradó strukturális rés**: az S139-es frissesség-kapu csak a `catchup-<dátum>` refresh jobokra vonatkozik. A `slot-00/01` jobok továbbra is azonnal futtak daemon-induláskor a régi napi fájlra; a **mai** nap frissítése így 13:00/14:15 előtt nem garantált. Új kód-szelet (slot jobok frissesség-kapuja + teszt) külön döntés — most nem terjeszkedtünk rá.
- [ ] A telepített daemon telepítési ideje (11-25 17:14) és a forrás-build (16:58) eltérése arra utal, hogy egy másik worktree régi binárisa került telepítésre. A pontos eredet nem nyomoztuk ki; a jelenleg telepített bináris a `main`-ről származik, ellenőrizve a kapu-szöveggel.

## Visszaállás

- Bináris: a korábbi telepített példány `agent/bin/footballd.bak-1617` (régi, csak lokális mentés); bármely git-commitból újraépíthető és telepíthető az `install-footballd-agent.sh`-vel.
- Adat: a két discovery job újraenqueue-je idempotens; az új obszervációk a research napi JSON-okból újratermelhetők, nem kézi revízió. Nem történt törlés, merge vagy kényszerített git művelet.

## Következő döntési pont

- **S2 (Üzemeltetés UI) indítása — a felhasználó döntése.** Az S168-as S1 zöld
  kapu után megálltam, S2-be nem léptem tovább. S2-ben kötelező a daemon
  újratelepítése, hogy az 5 új endpoint éljen a kliens számára.
- A hiányjelek detektorának bekötése (S3): `daily-check`/`integrity` írja az
  `ops.issue` sorokat a most elkészült `upsert_seen` úton.
- Változatlanul nyitva: 14:15 utáni odds-lefedettség mérése, a `slot-00/01`
  frissességi kapu, a `docs/operations/remaining-gaps.md` frissítése.
- A `2026-11-25-app-oldalak-bovites-design.md` vázlat 10. szakaszának döntései
  **lezarásra kerültek** (`docs/decisions/2026-11-26-app-oldalak-bovites.md`).
