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
- [ ] **Folyamatban**: 08-16 → 08-11 köteg. Hátralévő cél: 08-10 → 08-01.

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

## Következő lépés

- A backfill folytatása 09-18 → 08-01-ig napi integritással és véletlen mintákkal; külön döntés a téves provenance-link tisztításáról és a `is_selected` mezőalapú frissítési szabályról.
