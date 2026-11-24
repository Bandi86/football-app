# Checkpoint — 2026-11-24

## Induló állapot

- A 2026-11-23-i checkpoint zárása szerint: S113 kész, S114 öt nap kész (11-12…11-08), 11-07 backfill zöld de v2-lánc hátra.
- Felhasználói kérés: gyors check, majd adatrendezés (soccerway, duplikátumok, liga logók), nov 7 csak utána.

## S116 — Adatrendezés: soccerway, duplikátumok, liga logók

### Változott
- [x] **Soccerway letiltva**: `UPDATE ops.source_policy SET enabled=false WHERE source_slug='soccerway'` (a mai 07:31-es sikeres fetch zárta be a breakert — a tiltás most biztosítja, hogy ne jöjjenek új magyar/fordított sorok).
- [x] **Verseny-szótár** (`rust/crates/ingest/src/competitions.rs`): új bejegyzés `bajnokok ligaja - fotabla` → `Champions League` + teszt (`hungarian_champions_league_title_maps_to_english_canonical`). 13 competitions-teszt zöld.
- [x] **Verseny-merge** (dokumentált `identity merge-competition --force --apply`): `Bajnokok Ligája - Főtábla` (9 meccs) → `Champions League`; `AFC Bajnokok Ligája - Főtábla` (3) → `AFC Champions League Elite East`.
- [x] **6 duplikátum meccs összeolvasztva** (`footballctl dedupe pair`): 3 CL pár (AEK Athén/Athens, Porto/Feyenoord, PSV/Real) + 3 AFC pár (Công An/Kobe, Johor/Ratchaburi, Al Nassr). Minden áldozat archiválva `ops.dedupe_archive`-ban (142 sor ma).
- [x] **Liga-logók** (`scripts/import-v1-megapack-logos.py`): két javítás — nemzetközi ország-család (`''`/`INT`/`XX`/`UNK`/`ZZ` → `XX`) és pontos névcsoport több entitásának elfogadása. `--apply`: 79 → **82 illesztett verseny**; CL, AFC Elite East/West most `competition_logo`-val rendelkeznek.

### Mért eredmény
- [x] 11-24 versenyek: `Champions League` **9**, `AFC Champions League Elite East` **6**, `West` **2** (előtte 12/9/8 + magyar duplikátumok).
- [x] `integrity --from 2026-11-24 --to 2026-11-24`: **verdict OK**, 0 duplikátum (kickoff/fixture/same-pair), 0 árva provenance, 0 failed job.
- [x] API `/v1/day/2026-11-24`: mindhárom verseny `competition_logo=True`; CL/AFC meccsek száma 33 → **17**.
- [x] Kapuk: `cargo fmt` 0, `cargo clippy --workspace --all-targets` 0, `cargo test --workspace` **584 passed** (.env-vel), `py_compile` 0, `git diff --check` 0.

### Kockázat / visszaállás
- A 6 merge és 2 verseny-merge **adatrevízió**, de minden áldozat a `ops.dedupe_archive`-ban van JSONB payloadtal; a verseny-merge auditja `match_dedupe_merge` + alias/ref átmozgatás. A soccerway tiltás SQL-lel visszafordítható (`enabled=true`).
- A `fold_name` továbbra sem fordít nyelvek között (`AEK Athén` ≠ `AEK Athens`) — a szótár és a kézi merge most rendezte, de új magyar forrásnál újra kellene alias.
- **Biztonsági megjegyzés**: `~/Library/Application Support/FootballAppV2/agent/run.sh` plaintextben tartalmazza az OpenFoot API-kulcsot (`of_live_...`) — nincs a repóban, de Keychainbe kellene kerülni.
- Nov 7 (Rhode Island–Davidson duplikátum) és a nov 9-i `fm:` ref-probléma továbbra is nyitott — a folytatási jegyzet szerint.

### Teljes hónap ellenőrzés (nov 01–24)

- [x] `integrity --from 2026-11-01 --to 2026-11-24`: kiindulás **duplicate_fixture=2**, `same_pair_same_day=3`.
- [x] **11-23 rendezve**: ugyanaz a `livescore:1914527` esemény két rögzítési idővel (21:00 eredmény nélkül / 01:30 eredménnyel) → `dedupe pair`, túlélő az eredményes `01a20caa`, áldozat archiválva.
- [x] **11-07 RENDEZVE** (felhasználói döntés: a Davidson Wildcats hivatalos oldala szerint a 2–2 a valódi): a `espn:401933415` (2–2, 17:00) a túlélő, a `espn:401929719` (0–0, 01:00) áldozat archiválva. A korábbi [- ] döntési pont tartalma: a `Rhode Island – Davidson` pár két **külön ESPN esemény** (`espn:401929719` 0–0 01:00, `espn:401933415` 2–2 17:00), mindkettő `NCAA Men's Soccer`, ugyanaz a nap. A `duplicate_fixture` check kemény hibát jelez, de a két külön eredmény miatt az automatikus merge **adatvesztést okozna** (az egyik eredmény elveszne). Nem olvasztottam össze. Döntés: (a) két valódi külön meccs → a check megkerülése review-jelzéssel, vagy (b) az ESPN duplikálta → kézi kiválasztás, melyik eredmény a valódi.

### Visszaállás / biztonság

- A 7 merge (6 CL/AFC + 1 livescore) minden áldozata a `ops.dedupe_archive`-ban van JSONB payloadtal.
- Soccerway tiltás: `UPDATE ops.source_policy SET enabled=true WHERE source_slug='soccerway'` visszaállítható.
- **Biztonsági teendő**: az agent `run.sh` plaintext OpenFoot API-kulcsot tartalmaz — Keychainbe kell kerülni, de ez külön üzemeltetési döntés, most nem módosítottam.

### Visszafelé haladás folytatása — 11-06 és 11-05

- [x] **11-06**: backfill 215 detail (0 hiba, odds 20 — ebből 19 football-data) → export → job → **214 meccs** → **171 insight sor**, 1 238 játékos / 67 650 stat → integrity **OK**, coverage 214/213 lezárt/21 odds/100 verified.
- [x] **11-05**: backfill 142 detail → export → job → **139 meccs** → **88 insight sor**, 678 játékos / 57 324 stat → integrity **OK**, coverage 139/139 lezárt/42 odds/32 verified.
- [ ] **Megjegyzés (nem elnyomva)**: a 11-05 `deep_verify` 1 hibát jelzett — `fm6112410 FK Crvena Zvezda–Inter Club d'Escaldes: érvénytelen odds DC/HD=1.01`. A tipplap `1X2 home=1.03` mellett a `DC/HD=1.01` matematikailag összhangban van a forrással; a `tools/football.py:1775` `val <= 1.01` határa túl szigorú. A backfill EXIT=0 maradt (deep_verify csak logol). Az adatot betöltöttük; a határ felülvizsgálata külön szelet (research tools, git-en kívül).
- [x] Mindkét napon ismételt `fm:... nincs kanonikus meccs-ref` kimaradás (11-06: `fm:1000019176`, 11-05: `fm:6543254`) — a nov 9-i ref-probléma része, külön szeletre vár.

### 11-04

- [x] backfill 145 detail (0 hiba, odds 29: 4 football-data + 27 tipplap) → export → job → **132 meccs** → **93 insight sor**, 777 játékos / 42 667 stat → integrity **OK**, coverage 132/125 lezárt/28 odds/20 verified. Ismételt `fm:1000022343` ref-kimaradás (nov 9-i szelet).

### 11-03

- [x] backfill 169 detail (0 verify hiba; deep_verify 3× `DC=1.01` határ-hiba — valódi tipplap odds, l. 11-05 megjegyzés) → export → job → **165 meccs** → **159 insight sor**, 889 játékos / 70 792 stat → integrity **OK**, coverage 165/164 lezárt/63 odds/99 verified. Ismételt `fm:6016325` ref-kimaradás.

### 11-02

- [x] backfill 171 detail (0 hiba, odds 75: 15 football-data + 73 tipplap) → export → job → **169 meccs** → **158 insight sor**, 582 játékos / 69 350 stat → integrity **OK**, coverage 169/169 lezárt/75 odds/79 verified. **Nincs** `fm:` ref-kimaradás.

### November teljes feltöltése — ZÁRÁS

- [x] **11-01**: backfill 623 detail (1 `DC/DA=1.01` határ-megjegyzés) → export → job → **614 meccs** → **486 insight sor**, 1 059 játékos / 187 877 stat → integrity **OK**, coverage 614/611 lezárt/239 odds/220 verified. `fm:5161690` ref-kimaradás.
- [x] **Teljes november (01–30)**: integrity `--json` → `duplicate_kickoff=0`, `duplicate_fixture=0`, `orphan_provenance=0`, `active_refs_to_merged=0`, `failed_jobs=0`, `open_breakers=0` — **kemény hibamentes**. `same_pair_same_instant=43` / `same_pair_same_day=45` review-jelzés (11-17 U17/U19 időzóna-párok), `pending_reviews=7`, `withheld_gates=1` — nem kemény hibák.
- [x] **Összesen november**: **7 897 kanonikus meccs / 6 494 eredmény / 2 743 insight sor**. Lemez 35 GiB.
- [x] 12 nap backfill+export+job+import+integrity lánc mind zöld (11-12 → 11-01).

### Döntési pont: október

- A research forrás korábbi hónapokra is ad adatot (mérve: 2026-10-15 → 31 liga / 102 meccs, 2026-09-01 → 59 liga / 118 meccs, 2026-01-15 → 21 liga / 41 meccs).
- Kérdés: folytassuk-e októberrel visszafelé, vagy megálljunk november teljes feltöltésénél és összegezzünk.

## Self-audit 2. kör — hézag feltöltése + kreatív probe tesztek

### A 11-13…11-22 hézag feltöltése

- [x] 10 discovery job betéve (research oldalon minden dokumentum megvolt, **nem volt hálózati letöltés**).
- [x] **7 nap gazdag adattal feltöltve**: 11-16 (60), 11-17 (90), 11-18 (65), 11-19 (57), 11-20 (168), 11-21 (598), 11-22 (380) — összesen **1 418 insight sor**.
- [x] **11-13/14/15: 0 insight** — a research `api/v1/matches` dokumentumok **csak `ls:` (livescore) ref-űek, nincs `fm:` referencia**; az import `fm:` refen köt, ezért ezekre nem megy insight. A meccsek (114/623/438) és eredmények be vannak töltve. Ez az ismert `fm:`-hiány része, nem betöltési hiba.
- [x] Integrity a 10 napra: **0 duplikátum**, 0 árva provenance, 8 pending review.
- [ ] **2 failed job (nem adatvesztés)**: 11-14 `team_entity_id_key` ütközés (a nap már `done` jobbal betöltve, felesleges új job); 11-22 hiányzó research dokumentum `..._bendel-insurance-vs-doma-united_fm6103183.json` (1 meccs kimaradt az insightból).

### Kreatív probe tesztek (nov 01–12)

- [x] **xG vs valós gól (csapat)**: Chelsea W 9.0 xG / 7 gól (**−2.0, alulrúgott**), Twente 14.1/13 (−1.1), Real Madrid 9.1/9 (−0.1).
- [x] **Szöglet**: API `home_corners/away_corners` él (pl. Deportivo Garcilaso–Comerciantes 9–0); `Corners` player-stat 4649 sor.
- [x] **Sárga/piros lap**: API `home_yellow/away_yellow/home_red/away_red` él (pl. 3–6 / 1–1); 6468 `Card` incidens — **de az incidens-sor `detail` mezője üres** (a szín csak az összesített mezőkben van, az `incidents` tömbben nem).
- [x] **Bíró**: `fact.match_official` él (pl. Mohammed Al Hakim 3 meccs: Kalmar–Sirius, Malmö–Hammarby, Panathinaikos–CSKA); API `referee` mezővel.
- [x] **Játékos-teszt (random)**: Esposito 7 gól / 4 meccs / 328 perc — API `/v1/player` 4 appearance + 50 stat sor, DB-vel egyezik.
- [x] **Csapat-teszt (random)**: „Nacional" 3 külön entitás (UY 7 / PY 4 / PT 3 meccs), API-ban is elkülönül — **az entitás-ID a helyes kulcs, nem a `canonical_name`**.
- [x] **xG-lefedettségi hiány**: API `home_xg/away_xg` a `fact.match_stats`-ból jön (novemberben 2312 sor, ebből 903 xG-vel), miközben a research xG a `match_player_stats`-ben van (891 meccs). Ahol a forrás nem adott xG-t, ott `None` — adatelérhetőség, nem serving-hiba.

## S118 — A és B javítások

### A: 11-13/14/15 `fm:`-ref pótlása (KÉSZ)
- [x] Újra gyűjtés: `backfill --from 2026-11-13 --to 2026-11-15 --limit 3` → **3 ok / 0 hiba**, verify/deep_verify 0 hiba. fm detail: 11-13: 88, 11-14: 311, 11-15: 230.
- [x] `api-export` mindhárom napra (fm dokumentumok: 88/311/230) → discovery job → 11-13 `done`, 11-15 `done`, 11-14 két `failed` (`team_entity_id_key` ütközés — de a korábbi `done` job 623 meccset és 85 fm refet már betöltött, így **nem adatvesztés**).
- [x] Insight/player import: **11-13: 87, 11-14: 85, 11-15: 230 insight sor** (előtte 0/0/0). 11-13: 108 játékos, 11-14: 160, 11-15: 411.
- [x] A három nap meccsszáma most 158/659/555 (új live meccsek is bekerültek a friss gyűjtéssel).

### B: lap szín bevezetése az `incidents`-be (KÉSZ)
- [x] Gyökér: `docs/api-export.py:768` a `detail`-t csak `goalDescription`/`eventDescription`/`description`-ből vette — a FotMob `card` (`Yellow`/`Red`) mezőt nem. A `cardDescription` néha dict, ezt is kezelni kellett.
- [x] Javítás (git-en kívüli research könyvtár): `_card_detail(e)` helper → `Yellow card` / `Red card` (+ ok ha van), bekapcsolva a `detail` kifejezésbe. Backup: `docs/api-export.py.bak-20261124-card`.
- [x] Újraexport `2026-11-16` → research dokumentumok: **141/141 Card eseménynek van `detail`-e**.
- [x] Discovery job → worker újraírta az incidenseket: `fact.match_incidents.detail` most **97 Yellow card + 8 Red card** (előtte mind null).
- [x] API ellenőrzés: `/v1/match/{id}` `incidents` tömbben a Card `detail` = `Yellow card` — **a végfelhasználónál is megjelenik**.
- [x] A javítás minden jövőbeli exportra érvényes; a korábbi napokhoz újraexport + discovery job kellene (11-16 bizonyíték).

### Kapu / visszaállás
- A B-hez: `cargo` érintetlen (csak research Python + DB adat). `py_compile` 0, export EXIT=0, job `done`, integrity 0 duplikátum.
- Visszaállás: backup `.bak-20261124-card`; az incidens-sorokat a worker `DELETE + INSERT` újraírja a source_id alapján.

## Következő döntési pont
- Nov 7 és nov 9 kivizsgálása tesztekkel, csak a mai rendezés után.
- A soccerway tiltás tartósságának napi ellenőrzése a `daily-check`-ben.
## S116 zárás — felhasználói döntések és teljes ellenőrzés

- [x] **11-07 döntés (user)**: a Davidson Wildcats hivatalos oldala szerint a **2–2** a valódi eredmény → `dedupe pair`: túlélő `01a20c8f-f34e` (2–2), áldozat `01a20c8f-f326` (0–0) archiválva. A meccs most `home_ft=2, away_ft=2`.
- [x] **11-21 új duplikátum**: `A-League Women` Newcastle Jets W – Adelaide United W, két rögzítés (`espn:401913229` 21:00 eredménnyel / `fm:1000020337` 01:00 eredmény nélkül) → eredményes túlél, áldozat archiválva. (A mai soccerway/catchup újrarögzítésből származott.)
- [x] **Teljes hónap integrity (nov 01–24)**: `duplicate_kickoff=0`, `duplicate_fixture=0`, `orphan_provenance=0`, `active_refs_to_merged=0`, `pending_reviews=0` — **kemény hibamentes**. A `same_pair_same_instant=43` / `same_pair_same_day=46` review-jelzés (11-17 U17/U19 időzóna-duplikátumok), nem kemény hiba.
- [x] **11-07 zárás**: integrity **OK**, 672 meccs / 667 eredmény / 620 insight.
- [x] **11-24 zárás (a mai nap)**: integrity **OK**, 209 meccs / 153 insight / 116 odds / 169 logó, 0 függő review. Preview-adat: a research-ben 175 scheduled meccsből **175-höz van `preview_hu`**, 129-hez `prediction`, mind a 184 dokumentumhoz `odds` — a predikció-hiány ott van, ahol nincs elég szezonstatisztika (adatelérhetőség, nem hiba).
- [x] Összesen 9 duplikátum pár archiválva (6 CL/AFC + livescore 11-23 + ESPN 11-07 + A-League 11-21), mind visszaállítható `ops.dedupe_archive`-ból.

## Következő döntési pont
- A mai nap és a hónap adatai tiszták → **folytatás a visszafelé haladással: 11-06, 11-05…** napi kapuval.
- Nov 9-i `fm:` ref-probléma továbbra is nyitott, külön szelet.
- Soccerway tiltás tartóssága napi ellenőrzés alatt.
## S118 zárás — A+B kész, teljes ellenőrzés

- [x] **Teljes november**: **8 087 meccs / 4 563 insight sor** (előtte 7 897 / 4 161 — a 3 nap +402 insight).
- [x] Integrity nov 01–30: `duplicate_kickoff=0`, `duplicate_fixture=0`, `orphan=0`, `active_refs_to_merged=0` — **kemény hibamentes**. 20 pending review, 3 failed job (11-14×2 ref-ütközés + 11-22 hiányzó fájl), 1 `same_pair_same_instant` + 4 `same_pair_same_day` — review-jelzések, nem kemény hibák.
- [x] **Probe újra (B)**: az új napokon (11-13/14/15) az API `/v1/match` `incidents` tömbben **8 színes lap** (pl. São Bernardo–Athletic, `Yellow card`) — a javítás ott is érvényes.
- [x] **Probe xG új napokon**: még nincs elég xG (a friss 87/85/230 insight nem hordoz xG-t) — adatelérhetőség, nem hiba; a régi napokon (Chelsea W −2.0) már mért.
- [x] Visszaállás: A → `backfill-state.json` done lista + újragenerálható napok; B → `api-export.py.bak-20261124-card` + worker DELETE/INSERT újraírás.

## Következő döntési pont
- Október folytatása (a research mérve ad adatot: 10-15 → 102 meccs), vagy megállni és fenntartani a novemberi állapotot.
- A 3 failed job (2 ref-ütközés + 1 hiányzó fájl) opcionális külön tisztítása — nem adatvesztés, csak zaj a napi checkben.
## S119 — Október felé (augusztus 1-ig)

### Előfeltételek ellenőrizve
- [x] **20 pending review**: mind `fuzzy_candidate` / `multiple_candidates` **ismeretlen csapat-jelzés** (ESPN/FotMob, ma 10:12–10:41, a backfillből). Fail-closed — nem került rossz adat a DB-be, és a szabály (automatikus fuzzy tiltott) miatt **review-ként maradnak**, nem oldjuk meg automatikusan.
- [x] **Rate limit / IP block**: 5 breaker `closed`, `failure_count=0`; a 24h logban 0 `429/403/quota/blocked`; soccerway tiltva maradt. FotMob próba `2026-10-31`: **185 liga / 707 meccs** — forrás elérhető.
- [x] Döntés: napi haladás visszafelé, minden napon teljes lánc + részletes ellenőrzés (integrity, coverage, random probe, webes összehasonlítás), cél **2026-08-01** (26-27 szezon kezdete).

### Folyamatban
- [ ] 10-31 backfill (700+ meccsre számítva).

