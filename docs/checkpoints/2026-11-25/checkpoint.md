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

## S135 folytatása — 09-24 → 09-19 (háttérben lefutott)

- [x] Mind a 6 nap EXIT=0, `verdict: OK`. Meccs/eredmény/verified: 09-24 82/82/22 · 09-23 134/133/37 · 09-22 134/134/83 · 09-21 100/100/43 · 09-20 654/652/273 · 09-19 786/786/369.

## Következő lépés

- Döntés a maradék 3 fordított orientációjú meccsről, majd a backfill folytatása 09-18 → 08-01-ig napi integritással és véletlen mintákkal.
