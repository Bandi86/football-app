# Checkpoint — 2026-11-25

## Induló állapot

- A stabil állapot a `docs/checkpoints/2026-11-24/checkpoint.md`-ben és a `README.md`-ben olvasandó.
- A tegnapi S128–S132 szeletek (bugirtás, header, élő fázis, Következik-szűrő, Sofascore összevetés) pusholva, az utolsó commit `672ac7c`.
- A mai S133 (soccerway integritás, visszatartás, teljes napi check) a dátumváltás miatt még a 11-24-i checkpointbe került, commit `4704a1d`.
- Felhasználói irány: Soccerway integritás → 2. pont (lefedettség) → 3. pont (percátadás) → napi check → backfill folytatása.

## S134 — Research élő perc átadása a kliensig

- [x] **Probléma**: a research API-ben pontos `status.minute` van (pl. 87'), de az ingest `Status` struktúrája csak a `code`-ot őrizte meg, így a perc nem jutott el az API-be. A kliens S129 óta a kezdésből becsült (45+15+45), ami pontatlan, ha a meccs késik.
- [x] **Ingest** (`research.rs`): a `Status` struktúra felveszi a `minute: Option<i32>` mezőt, és a `match_core` payload új kulcsot kap: `status_minute`. Teszt: `status_minute_survives_deserialisation`.
- [x] **Storage** (`day.rs`): új `DayMatchRow.minute` mező, LATERAL join a legfrissebb research `match_core` obszervációból (`fact.provenance` → `stage.observation`, `o2.observed_at DESC`) — **nincs új migráció, nincs új kanonikus oszlop**. Integrációs teszt: `fetch_day_returns_research_live_minute` (valós artifact→capture→observation→provenance lánccal, minute=87).
- [x] **API** (`routes/day.rs`): `DayMatch.minute: Option<i32>`; a kontrakt-fixture két meccse `"minute": null`-lal bővült; új teszt `live_minute_is_serialized_when_present` (87 értékre). OpenAPI séma-teszt zöld.
- [x] **Kliens**: `DayMatch.minute` a Swift DTO-ban (CodingKeys `minute`), `MatchCardModel.from` átadja (`minute: match.minute`) — a korábban `nil` volt. Az S129-es `livePhaseText` így most a valódi percet használja, ha van; ha nincs, a kickoff-becslés marad. Tesztek: `testResearchMinuteReachesCardAndOverridesEstimate` (87' → „2. félidő · 87'"), `testMissingMinuteFallsBackToKickoffEstimate` (20' → „1. félidő · 20'").
- [x] **Telepítés**: az új `footballd` bináris telepítve (`scripts/install-footballd-agent.sh`, felhasználói jóváhagyással a launchd restarthoz), a frissített kliens is telepítve.
- [x] **Kapuk**: Rust **588** (2 új + 1 integrációs teszt), clippy 0, fmt 0, Core **125**, app unit **TEST SUCCEEDED**, UI **TEST SUCCEEDED**, daily-check **EXIT=0**, git diff 0.

## Döntések / korlátok

- [x] **2. pont (lefedettségi hézag) — kivizsgálva, blokkolt**: a Sofascore referencia ~11 meccse / 22 csapata (Buru SC, Vitesse U21, Torreense U23, ASFA Dakar, Koper U19, Ajman, Al Duhail, Halcones stb.) **nincs meg** a research nyers adatában és egyik engedélyezett forrás (research, openfoot, tipplap, footballdata) obszervációiban sem — csak csapatidentitások vannak. A `footballdata` 22+15 rögzített ligát követ, ezek nincsenek köztük. Sofascore adapter kifejezetten tiltott. Új forrás vagy ligakonfig csak külön döntéssel.
- [ ] **S131 nyitott döntés**: a 6 darab, 5 percnél régebben kezdődött de még `scheduled` meccs egyik státusz-szűrőben sincs (a `lejárt, nyitott` 3 órás küszöb alatt). „Mind"-ben láthatók.
- [ ] **Backfill**: a szeptemberi backfill 2026-09-30-tól visszafelé 2026-08-01-ig még hátra van.

## S135 — Szeptemberi backfill (foldálásban)

- [x] **2026-09-30**: research 124 (closed=124, open=0) → DB 119 meccs / 117 eredmény / 34 odds / 35 verified; integritás OK (0 duplikátum, 0 árva provenance). 3 insight/player fájl kihagyva „nincs kanonikus meccs-ref" — az ok: **nyílt identitás-review** (Andorra, South Korea U23, Brunei/Hong Kong), a biztonsági kapu működik, nem hiba.
- [x] **Véletlen minták (6 nap, 18 meccs)**: 17/18 közvetlen egyezés, a 18. egy névváltozat volt (`Crystal Palace W` vs `Crystal Palace (W)`, eredmény 3:0 mindkét forrásban) → **18/18 egyezés**. Külön ellenőrzés: Ethiopia–Seychelles 16:0 (női barátságos) — nyers ESPN + web (ESPN/Wikipedia) szerint valós rekordgyőzelem, nem parser-hiba.
- [x] **2026-09-29 → 09-25**: mind az 5 nap zöld EXIT=0, minden nap `verdict: OK`, 0 duplikált kickoff-csoport. Meccs/eredmény/verified számok: 09-29 135/135/80 · 09-28 85/84/26 · 09-27 475/472/163 · 09-26 520/511/246 · 09-25 168/167/45.
- [ ] **Folyamatban**: 09-24 → 09-19 köteg háttérben fut (a shell timeout miatt háttérbe került); az eredményt a következő checkpoint rögzíti.
- [ ] Hátralévő: 09-18 → 09-01, majd 08-31 → 08-01 (a cél 2026-08-01).

## S136 — Elavult „lejárt, nyitott" tömeges hiba (felhasználói észrevétel)

- [x] **Probléma**: 11-24-en 133–154 meccs volt `lejárt, nyitott` / `scheduled`, miközben a research forrás már **178 lezártat** írt. A DB-ben csak 50 `finished` volt, 147 `scheduled` + 7 `live`.
- [x] **Gyökér**: a research napi fájlja **07:38–07:39-kor** frissül, de a worker S45 catch-up jobja a daemon indulásakor, **07:31-kor** futott — 7 perccel a frissítés előtt. A job `(entity_key, time_window)` páronként naponta egyszeri, így elavult adatot olvasott és nem futott újra. Ugyanez 11-25-ön is (research 6 finished, DB 1).
- [x] **Adatjavítás**: `research-sync.sh <dátum> --enqueue-only` a friss fájlokra 11-24 és 11-25 — mindkét job `done`. Eredmény: 11-24 **finished 50 → 175**, `scheduled 147 → 31`, `live 7 → 0`; az API `open_past` **154 → 10**. 11-25 finished 1 → 6, `open_past` 14 → 8. Napi check mindkét napra ZÖLD.
- [x] **Ütemezési javítás** (`scheduler.rs`): a catch-up `due_at`-je `now + 60 perc` (új `RESEARCH_CATCHUP_DELAY_MINUTES`), így a job mindenképpen a research frissítés után olvas. Teszt: `research_policy_enqueues_daily_catchup_refreshes` kiegészítve a `due_at > now` ellenőrzéssel. Új daemon telepítve (jóváhagyott launchd restart).
- [x] **Kapuk**: Rust **588**, clippy 0, fmt 0, UI **TEST SUCCEEDED**, daily-check (11-24, 11-25) **EXIT=0**, git diff 0.
- [ ] **Maradék 10 nyitott meccs (11-24)**: 6-nál a forrás maga nem ad eredményt (Bhayangkara–Padang, Madura–Persela, Persiraja–Dewa, Niroye Zamini–Havadar, Bandari–Kakamega, Fleetwood–Charlton U21, AFC Portchester–Hayes); 3-nál (Aston Villa–Galatasaray, Lens–RB Leipzig, Real Betis–Slovan Bratislava) a research **finished** eredményt ad, de **fordított hazai/vendég orientációval**, és a friss obszerváció nincs provenance-hoz linkelve — tudatos identitás-/eredményrevízió kell, nem automatikus merge.

## S138 — Fordított orientációjú meccsek javítása (felhasználói döntés)

- [x] **Döntés**: a felhasználó a „research szerint javítani" opciót választotta a 3 felnőtt meccsre (Aston Villa–Galatasaray, Lens–RB Leipzig, Real Betis–Slovan Bratislava).
- [x] **Végrehajtás**: egy tranzakcióban a `core.match.status` → `finished`, `core.match_result` beszúrása a research eredménnyel — a DB orientációhoz igazítva (Aston Villa 0:1 Galatasaray, Lens 1:3 RB Leipzig, Real Betis 2:0 Slovan Bratislava; félidők szintén fordítva). 3 audit esemény (`manual_result_revision`, research UID-val és visszaállítási útmutatóval). Az obszervációk és a soccerway-származású névorientáció érintetlenek.
- [x] **Mérés**: API 11-24 `open_past` **10 → 7**, a 3 meccs `finished` helyes eredménnyel; `missing_result: 0`. daily-check **EXIT=0** (`[OK] soccerway tiltva`).
- [x] Visszaállítás: `UPDATE core.match SET status='scheduled'` + a három `core.match_result` sor és az S138 audit események törlése.

## S135 folytatása — 09-24 → 09-13

- [x] **09-24 → 09-19** (háttérben lefutott): mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 09-24 82/82/22 · 09-23 134/133/37 · 09-22 134/134/83 · 09-21 100/100/43 · 09-20 654/652/273 · 09-19 786/786/369.
- [x] **09-18 → 09-13** (háttérben lefutott): mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 09-18 267/264/97 · 09-17 130/130/34 · 09-16 243/242/79 · 09-15 172/171/89 · 09-14 159/159/56 · 09-13 678/675/262.
- [x] **09-12 → 09-07** (háttérben lefutott): mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 09-12 797/797/351 · 09-11 301/300/100 · 09-10 127/127/53 · 09-09 164/164/63 · 09-08 198/197/108 · 09-07 177/177/58.
- [x] **Véletlen minták (09-07…09-18, 12 nap, 36 meccs)**: **36/36 pontos egyezés** státuszban és eredményben (research UID alapján).
- [x] **Integritás (09-07…09-30)**: `verdict: OK`, 0 duplikált kickoff/fixture, 0 árva provenance. Talált 2 cross-competition duplikátumot (`Fénix–Colón` 09-19, `River Plate–Colón` 09-26) — a research és a v1-import változat ugyanaz a meccs `Segunda División` / `LaLiga 2` verseny-entitás alatt, eredményük egyezett (1:1, 3:0). `dedupe pair`-vel összeolvasztva, research túlél; `azonos csapatpár azonos pillanatban: 2 → 0`.
- [ ] Maradék 10 „azonos csapatpár azonos napon" (eltérő kickoff + verseny): többsége jogos — finn Veikkausliiga fáziscsoportjai (TPS/SJK, HJK/Inter Turku stb.) és NCAA férfi/női meccsek. Nem kerülnek összeolvasztásra.
- [x] **09-06 → 09-01** (háttérben lefutott): mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 09-06 684/680/278 · 09-05 821/820/396 · 09-04 311/305/104 · 09-03 114/113/40 · 09-02 201/200/83 · 09-01 159/159/50.
- [x] **Szeptember lezárva**: 09-30 → 09-01, **30/30 nap zöld**, mind napi integritással/coverage-lal. Teljes hónap-integritás `verdict: OK` (0 duplikált kickoff/fixture, 0 azonos pillanatú pár, 0 árva provenance). Véletlen minták összesen **54/54 egyezés** (09-07…09-18: 36/36, 09-01…09-06: 18/18).
- [x] **08-31 → 08-26** (háttérben lefutott): mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 08-31 271/270/125 · 08-30 649/646/228 · 08-29 695/694/348 · 08-28 353/346/113 · 08-27 123/120/30 · 08-26 193/193/84.
- [x] **Ellenőrzés (08-26…08-31)**: integritás `verdict: OK` (0 duplikátum, 0 azonos pár, 0 árva provenance), véletlen minták **18/18 egyezés**.
- [x] **08-25 → 08-20** (háttérben lefutott): mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 08-25 192/192/107 · 08-24 183/183/49 · 08-23 535/534/182 · 08-22 810/809/428 · 08-21 387/382/115 · 08-20 173/171/57.
- [x] **Ellenőrzés (08-20…08-25)**: integritás `verdict: OK` (0 duplikátum, 0 árva provenance; 1 azonos napi pár jogos eltérő idővel), véletlen minták **18/18 egyezés**.
- [x] **08-19 → 08-18** kész (mind zöld: 08-19 124/121/53, 08-18 131/130/84). A **08-17** első próbálkozása elhasalt: a 14:15-ös időzített research-sync tartotta a `.research-sync.lock`-ot, és a `research-sync.sh --enqueue-only` 0-val, job nélkül lépett ki.
- [x] **S145 (scriptjavítás)**: `scripts/backfill-day.sh` most **közvetlenül SQL-lel** szúrja be a discovery jobot (idempotens: ha van pending/leased job, azt használja) — nem múlik a research-sync lockján. 08-17 újrafuttatva: EXIT=0, 152 meccs / 150 eredmény / 44 verified, `verdict: OK`.
- [x] **08-16 → 08-11** (háttérben lefutott): mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 08-16 510/508/180 · 08-15 628/624/329 · 08-14 246/245/132 · 08-13 175/173/38 · 08-12 104/101/44 · 08-11 147/144/90. A közvetlen SQL-job (S145) minden napon hibamentesen működött.
- [x] **Ellenőrzés (08-11…08-16)**: integritás `verdict: OK` (0 duplikátum, 0 árva; 2 azonos napi pár jogos eltérő idővel), véletlen minták **18/18 egyezés**.
- [x] **08-10 → 08-05**: mind a 6 nap EXIT=0. 08-10 79/79/39 · 08-09 377/377/184 · 08-08 842/835/446 · 08-07 187/186/109 · 08-06 102/100/52 · 08-05 97/97/27.
- [x] **08-04 → 08-01** (a kitűzött cél): mind a 4 nap EXIT=0, `verdict: OK`. 08-04 68/68/31 · 08-03 99/99/39 · 08-02 304/304/137 · 08-01 380/380/177.

## S147 — BACKFILL CÉL ELÉRVE (2026-08-01)

- [x] **Állapot**: a research `backfill-state.json` szerint **108 kész nap**, legkorábbi **2026-08-01**, legkésőbbi 2026-11-19, `failed: []`. Ebből augusztus **31/31**, szeptember **30/30** nap. A cél (2026-08-01, 26/27-es szezonkezdés) teljesült.
- [x] **Teljes backfill-tartomány integritás** (08-01…09-30): `verdict: OK` — 0 duplikált kickoff/fixture, 0 azonos pillanatú pár, 0 árva provenance, 0 failed job, 0 nyitott breaker. (16 azonos napi pár jogos, eltérő idővel/versennyel.)
- [x] **Véletlen minták összesen: 138/138 egyezés** a research forrással (státusz+eredmény, UID alapján): szeptember 54/54, augusztus 84/84 (18+18+18+18+30).
- [x] **Napi check**: 11-25 **EXIT=0**, `verified gate: 64`, `[OK] soccerway tiltva`.
- [ ] **Nyitva maradt döntések** (nem backfill-blokkolók): téves provenance-linkek tisztítása (S140), `is_selected` mezőalapú frissítési szabály, 11-25-i 8 darab 00:00Z placeholder-időjű meccs forrásminősége.

## S139 — A catch-up kockázatának végleges kezelése

- [x] **Kockázat**: az 1 órás fix késleltetés csak becslés volt — ha a research gyűjtése később tolódik, újra versenyhelyzet állhat elő.
- [x] **Megoldás**: a catch-up job most a **forrásfájl tényleges frissességétől** függ. Új `LocalFileFetcher::source_path`, új `repo::job_created_at`, és a `tick::catchup_source_stale(modified, created_at, now)` függvény: ha a napi envelope nem frissebb a job keletkezésénél, a job 15 perccel későbbre kerül (`requeue_with_delay`, `JobResult::Deferred`) — legfeljebb 6 órán át, utána mégis feldolgozás (tartósan nem frissülő forrás). A `TickOutcome` új `deferred` számlálója a worker-logban is megjelenik.
- [x] Teszt: `catchup_waits_until_the_source_file_is_fresh` (07:31 elavult → várakozás; 07:46 friss → feldolgozás; hiányzó fájl → várakozás; 6 óra után → feldolgozás). Rust **589**, clippy 0, fmt 0. Új daemon telepítve.

## S140 — Tegnapi és mai nap részletes, egyeséveli ellenőrzése

- [x] **Teljes státusz+eredmény összevetés** (provenance-UID alapján, minden UID-val): 11-25 **166/166 pontos**; 11-24 **176/178**, a 2 látszólagos eltérés az ellenőrző script műterméke (egy UID két napi fájlban is szerepel elavult értékkel; illetve téves provenance-link) — a megjelenített adat mindenhol egyezik a research végeredményével.
- [x] **Kezdési idők összevetése**: 5 eltérés; 3 téves provenance-link (pl. U19 UID a felnőtt meccsen, Fleetwood két külön meccse ugyanazzal az UID-val), 2 valódi elavult idő. A szisztémás ok: a **legrégebbi** obszerváció `is_selected=true`, a frissebbek `false` — ezért a kickoff nem frissül (a státusz viszont igen). A 2 konkrét idő auditáltan javítva (`kickoff_revision`): Shahin Bandar Ameri–Naft Masjed Soleyman 13:15Z → **14:00Z**, Al Sharkeyah–Ceramica Cleopatra 15:00Z → **18:00Z**; publish_date újraszámolva.
- [x] **Téves provenance-link (dokumentált, nem javított)**: `fm:1000021160` (U19) a felnőtt Real Madrid–PSV-hez is be van linkelve; `livescore:1872665` (Bandari–Mathare) a Bandari–Kakamega meccshoz; `fm:6015575` két külön Fleetwood-meccshez. Ezek az audit-nyomot érintik, a megjelenített adatot nem. Külön döntés a link-tisztításról.
- [x] **Tegnap (11-24)**: daily-check ZÖLD — 207 meccs, 178 lezárt, 178 eredmény, 105 verified, 0 duplikátum, 0 árva provenance, `open_past` 7. **Ma (11-25)**: ZÖLD — 171 meccs, 6 lezárt, `open_past` 8 (7 paraguayi meccs 00:00Z placeholder idővel + 1 mexikói; a research ezeket jövőbeli/Pontos idővel is adja — forrásminőségi kérdés).
- [x] **Kliens AX**: „Mind: 171", „Következik: 129", „Élő: 0", „Befejezett: 6", „Lejárt, nyitott: 8", „Eredmény nélkül: 0" — az API-val egyezik.
- [x] **Kapuk**: Rust **589**, clippy 0, fmt 0, UI **TEST SUCCEEDED**, daily-check mindkét nap **EXIT=0**, git diff 0.

## S148 — Validáció, anomáliakeresés, placeholder-idők (backfill után)

- [x] **Rétegzett véletlen minta + webes valóság-ellenőrzés**: 4 réteg (alsóbb liga / ifjúsági / kupa / női) mintája; webes források megerősítették: Arsenal W 0:2 Lyon (BBC/ESPN/Guardian), Chelsea W 1:1 Aston Villa W (BBC/Sky), Everton U18 0:3 Man Utd U18 (premierleague.com), Koninklijke HFC 0:5 Go Ahead Eagles (ESPN), Delfín 0:1 U. Católica (ESPN/Flashscore), Neretva 3:0 Šibenik (Wikipedia/HRT — visszalépés), Ethiopia 16:0 Seychelles (ESPN/Wikipedia). Mindegyik egyezik az adatbázissal.
- [x] **Anomáliakeresés (08-01…11-24)**: 0 lezárt meccs eredmény nélkül, 0 eredmény nem-lezárt státusszal. 15 extrém eredmény (8+ gól) — mindegyik research-forrásból, többségük NCAA/kupa/ifjúsági (pl. Ethiopia 16:0, NK Slatina 0:18 Segesta) — nem hiba.
- [x] **843 placeholder kezdési idő**: a research éjfél UTC-t ír, ha csak dátumot ismer; az ingest `exact`-ként vette át, így a kliens hamis 01:00-at mutatott. `research.rs` most `kickoff_precision='date_only'`-t ír 00:00 UTC-re (új teszt `midnight_kickoff_is_marked_date_only`); a meglévő **843 sort** auditáltan átállítottuk. Egy 14:15-ös sync a régi binárissal visszaírta 7 ma-i sort — az új daemon telepítése után újra javítva. API: 11-25 `missing_kickoff 0 → 7`, 09-01 `7`; AX: „Idő nélkül: 7".
- [x] **Dokumentum**: `docs/operations/remaining-gaps.md` — a hátralévő hézagok nyilvántartása (téves provenance-linkek, `is_selected` szabály, Sofascore-lefedettség, élő eredmények, S131 szűrő, soccerway, failed job, emberi VoiceOver/Tab).
- [x] **Kapuk**: Rust **590** (1 új teszt), clippy 0, fmt 0, Core 125, UI **TEST SUCCEEDED**, daily-check 11-24 és 11-25 **EXIT=0**, git diff 0.

## S149 — Integritási hézagok: provenance-linkek és `is_selected`

- [x] **Téves provenance-linkek tisztítása**: 70 sor (5 + 35 + 30; mind `is_selected=false`) törölve tranzakcióban, 3 `provenance_link_cleanup` audit eseménnyel. Érintett meccsek: felnőtt Real Madrid–PSV (U19 UID), Bandari–Kakamega (Bandari–Mathare UID), Fleetwood–Charlton U21 (Fleetwood–Leeds UID). Az obszervációk megmaradtak; a megjelenített adat változatlan. Ellenőrzés: 0 maradék sor, integritás `OK`, API 11-24 185 / 11-25 171 meccs.
- [x] **`is_selected` szabály** (`canonicalize.rs`): a legfrissebb obszerváció veszi át a kiválasztást mezőnként. Az `uq_provenance_selected_field` miatt két lépés (UPDATE deselect + UPSERT), az újrafeldolgozás stabil. Új teszt `newer_observation_takes_over_selected_provenance`; Rust **591** (1 új), clippy 0, fmt 0.
- [x] **Valós ellenőrzés**: 11-24 újra-ingestelve az új daemonnal; a Shahin Bandar Ameri `kickoff_at` provenance-a most a 2026-11-25 07:39-as (14:00) obszerváció (`is_selected=t`), a régi 13:15-ös már nem kiválasztott (0 találat).
- [x] **Dokumentum**: `docs/operations/remaining-gaps.md` frissítve — az 1. és 2. pont megoldva, a többi nyitva marad.
- [x] **Kapuk**: daily-check 11-24 és 11-25 **EXIT=0**, Core zöld, UI **TEST SUCCEEDED**, git diff 0. Új daemon telepítve.

## S150 — Forráskutatás és további hibakeresés

- [x] **Forráskutatás**: a research FotMob napi JSON-ja nem tartalmazza a 11 hiányzó meccset. A forráskatalógus alapján a **BetExplorer** engedélyezett (robots üres, HTTP 200, 635 KB valós tartalom): a 11-ből **3 meccset** fed le eredménnyel (Ajman–Al Ettifaq 1:2, Al Duhail–Neom 3:2, Torreense U23–Vizela U23 2:1). Maldives, Liga TDP, Senegal Ligue 2 egyáltalán nincs a feedben. Részleges pótlás; új adapter + forrás-engedély kellene. Megjegyzés: a research a katalógusa által tiltott FotMob `/api/data/matches` végpontot használja (research-projektbeli kérdés).
- [x] **Keresztforrás-eredményütközések**: a talált párok többsége fordított orientáció vagy elavult obszerváció; a kanonikus érték minden ellenőrzött esetben a helyes (Paris FC–Angers 4:0, Girona–Almería 0:1, Loto–Coton 4:0, Chivas–Necaxa 1:0). Nem hiba.
- [x] **Anomáliakeresés átfedő meccsekre**: talált egy valódi **szemantikai duplikátumot** — Everton W–Brighton W (WSL, finished) és Everton W–Brighton & Hove Albion (Women's Super League, scheduled) ugyanabban az időben; mindkét Brighton-entitás női/senior/azonos ország. A `dedupe variants`/`aliases` dry-run nem fogta (1 csoport, 0 merge). További magas hasonlóságú női entitáspárok jelöltek, de sok hasonló név külön csapat — automatikus fuzzy merge tilos (AGENTS.md). Döntésre vár.
- [x] `docs/operations/remaining-gaps.md` frissítve a BetExplorer- és identitás-eredményekkel.

## S151/S152 — Duplikátum, forduló-címke és kiemelt ligák

- [x] **S151 — Everton W–Brighton W duplikátum**: a `Brighton & Hove Albion` (női, ESPN) entitást auditáltan összeolvasztottuk a `Brighton W` (FotMob) entitásba (`identity merge-team --via-alias`, 2 meccs, 2 alias, 1 ref, 2 asset), majd a két fixture-t `dedupe pair`-vel (a `finished` 0:1 maradt). Az API-n 11-22-n most **1** Everton–Brighton meccs. Ellenőriztem, hogy a **férfi** `Brighton & Hove Albion` (EFL Cup) külön entitás maradt — nem olvasztottuk össze.
- [x] **S152a — forduló-címke normálizálás** (`MatchCardModel.foldedRoundLabel`): a `Round 15` / `Forduló 15` címkéket most `15`-refoldoljuk (csak szám előtti előtag), így nem külön címkék és nem rejtjük el a fordulót; a `Round of 16` változatlanul nyolcaddöntő marad. Új teszt `testRoundPrefixIsFoldedOnlyBeforeDigits`.
- [x] **S152b — kiemelt ligák prioritása**: a research adapter minden versenynek `tier=0 / priority=30`-at ír; a versenyszótárban csak a La Liga szerepelt. Hozzáadtuk a top-5 + NB I (tier 1 / priority 6) és a Championship + NB II (tier 2 / priority 12) ország-korlátozott bejegyzéseit, és felvettük őket a homonima-listába (a brazil Serie A / jamaicai Premier League változatlan marad). 2 új teszt. A meglévő **7 versenyt** (822 meccs) auditáltan frissítettük. Eredmény: 11-22-n **19 kiemelt meccs** (Serie A, NB I, La Liga, Premier League, Ligue 1, Bundesliga).
- [x] **Kapuk**: Rust **593** (2 új szótár + 1 kliens teszt), clippy 0, fmt 0, Core zöld, UI **TEST SUCCEEDED**, daily-check 11-22 és 11-24 **EXIT=0**, git diff 0. Új daemon telepítve.

## S153/S154 — Lejárt nyitott, odds, logók, dél-amerikai duplikátumok

- [x] **S153a — `date_only` meccsek lejártnak számítása**: a kliens `isOpenPast` és a backend `fetch_day_gaps` most kizárja a `date_only` meccseket (placeholder 00:00 UTC alapján nem lehet „lejárt" következtetést vonni). Új teszt a kliensben; API 11-25 `open_past 13 → 7`, `missing_kickoff 7`.
- [x] **S153b — az `exact` ragadóssága**: a canonicalize UPSERT korábban soha nem engedte `exact` → `date_only` átmenetet, így a forrás javítása nem jutott át (a 16:25-ös research obszerváció már `date_only`-t írt, de a DB `exact` maradt). Most az éjfél UTC-vel tárolt értéket a friss `date_only` felülírhatja. Új teszt `midnight_exact_can_be_corrected_to_date_only`; újra-ingestelés után a 7 meccs `date_only`.
- [x] **S154 — paraguayi duplikátumok**: 3 meccs kétszer szerepelt (Luqueño–Olimpia, Nacional–Rubio Ñú, Trinidense–Recoleta) — azonos research UID-val, de `Paraguayan Primera` (21:00, valódi idő) és `Division Profesional: Clausura` (00:00 placeholder) verseny alatt. `dedupe pair`-vel a valódi idejű sor maradt; a 00:00Z meccsek száma 7 → 4.
- [x] **Kivizsgált, nem hiba**: a 7 tegnapi lejárt nyitott meccsre a research sem ad eredményt (3 egyáltalán nincs benne) — valódi forrásgap. A mai presztizs meccsek (Sabah–Barcelona, Atlético–Viking, PSG–Roma) odds nélkül: a research `odds: false`-t ad rájuk (csak az U19 változatoknak van odds) — szintén forrásgap.
- [x] **Logók**: 6411 csapatlogó-asset van; a hiányzók többségének nincs assetje. 755 hasonló nevű entitáspár található, de többségük külön csapat ((W), II, B, N) — automatikus összevonás tilos. Dokumentálva.
- [x] **Kapuk**: Rust **594** (+1 teszt), clippy 0, fmt 0, Core zöld, UI **TEST SUCCEEDED**, daily-check 11-24/11-25 **EXIT=0**, integritás `OK`, git diff 0. Új daemon telepítve.

## Következő lépés

- További hibakeresés és forráskutatás: az S131 szűrő-kérdés, az élő eredmények/odds hiánya (új forrás kellhet), és a bizonyítható entitásduplikátumok páronkénti elbírálása.
