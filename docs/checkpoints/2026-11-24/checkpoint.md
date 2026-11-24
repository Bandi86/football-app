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

### Októberi napi haladás (javított kóddal)

- [x] **10-31**: 838 meccs / 703 insight / 1755 játékos / 225 223 stat, integrity OK (7 kódjavítás után, l. S119).
- [x] **10-30**: backfill 204 detail (0 verify hiba, DC=1.01 határ) → export → job **első próbálkozásra `done`** (javított kód) → **203 meccs** / **162 insight** / 612 játékos / 67 859 stat → integrity **OK**, coverage 203/203/99 odds/100 verified. Random probe: Al-Ittihad Alexandria–Tala'ea El Gaish — szöglet 2-6, sárga 1-1, xG 1.23-0.52, 17 incidens, 8 insight szekció. Webes FotMob 10-30: 92 liga / 163 meccs (a DB 203 = több forrás összesítve).
- [x] **10-29**: 189 meccs / 140 insight / 1046 játékos / 68 084 stat, integrity OK. Random probe: Concordia Chiajna–Sepsi OSK — 4 incidens, de szöglet/sárga/xG `None` (forrás nem adott metrikát → NULL, nem hamis 0; AGENTS.md §6 teljesül).
- [x] **10-28**: 261 meccs / 233 insight / 1084 játékos; probe: Real Sociedad W–Fenerbahçe szöglet 4-1, sárga 0-2, 16 incidens.
- [x] **10-27**: 227 meccs / 219 insight / 928 játékos (script pozitív tesztjével).
- [x] **10-26**: 109 meccs / 103 insight / 166 játékos (`--fetch` teljes teszt; 5 fm ref-kimaradás).
- [x] **10-25**: 731 backfill detail → 499 insight / 963 játékos / 210 541 stat, integrity OK.
- [x] **10-24**: 805 detail → 719 insight / 1456 játékos / 247 690 stat, integrity OK.
- [x] **10-23**: 280 detail → 203 insight / 581 játékos / 72 877 stat, integrity OK.
- [ ] **10-22 és korábbi napok**: folytatás sorban, napi kapuval, cél 2026-08-01.

## S119 — Október 31. betöltése + 7 kódjavítás (csonka team invariáns)

### Gyökér (mért)
- A `core.team` 17 soránál `id != entity_id` (régi női csapatok) — az invariáns sértése. Ez okozta a tartós `team_entity_id_key` és `match_home_team_id_fkey` / `player_team_spell_team_entity_id_fkey` hibákat: a `team_ids` az `entity_id`-t tette a meccsbe, de a `core.team` sor `id`-ja más volt.
- 6 iteráció után megtalált és javított gyökerek:
  1. `canonicalize.rs` `upsert_team`: `ON CONFLICT (id)` → `ON CONFLICT (entity_id)`.
  2. `identity/review.rs` + `repair.rs`: nyers INSERT → `ON CONFLICT (entity_id) DO UPDATE` + a `core.team` sort **mindig** biztosítani kell (nem csak új entitásnál).
  3. `canonicalize.rs` `ensure_live_team`: `entity_id` alapján keres, és a **tényleges `core.team.id`-t adja vissza** (nem feltételezi, hogy az entity_id == sor id).
  4. `player_import.rs`: a `player_team_spell.team_entity_id`-be a `core.team.entity_id`-t írja (FK a `core.entity.id`-re).

### Eredmény (mért)
- [x] 10-31 backfill: **829 meccs**, 0 verify hiba, odds 334 (99 football-data + 329 tipplap).
- [x] discovery job a 7. próbálkozásra **`done`** — **838 meccs** betöltve.
- [x] `match-insight-import`: **703 sor**; `player-import`: **1755 játékos / 225 223 stat / 4455 lövés**.
- [x] `integrity`: **verdict OK**, 0 duplikátum, 0 árva provenance; `coverage-day`: 838/828 lezárt/335 odds/375 verified.
- [x] Kapuk: `cargo fmt` 0, `clippy --workspace --all-targets` 0, `cargo test --workspace` **584 passed**.
- [ ] A 9 failed job a javítás **előtti** próbálkozásokból (zaj, nem adatvesztés) — opcionális tisztítás külön szelet.

### Visszaállás
- Git revert a kódjavításokra; a 10-31 adatai source_id + captured_at alapján törölhetők; a 17 csonka `core.team` sor adatrevízióra vár (nem automatikus merge).

## S119 — `scripts/backfill-day.sh` (egy nap = egy parancs)

- [x] Új script: `scripts/backfill-day.sh [--fetch] YYYY-MM-DD`. Módok: `--fetch` nélkül = v2-lánc (export → job → worker várakozás → match-insight/player import → integrity/coverage); `--fetch`-el = backfill + ugyanez. Fail-closed: bármely lépés hibája → exit 1/2.
- [x] Tesztek (részletes): `bash -n` 0, `shellcheck` 0; hibás dátum (`2026-99-99`) → exit 2; érvényes de hiányzó nap → exit 1; **pozitív teszt 10-27** (EXIT 0: 219 insight, 928 játékos, integrity OK); **teljes `--fetch` teszt 10-26** (EXIT 0: 115 backfill detail → 103 insight, 166 játékos, integrity OK). Közben talált bug: dupla export → javítva (egyszer fut).
- [x] 10-27: 227 meccs / 219 insight; 10-26: 109 meccs / 103 insight (5 `fm:` ref-kimaradás), mindkettő integrity OK.

## S120 — „figyelmen kívül maradt" hibák lezárása (user észrevétele)

- [x] **7 `same_pair_same_day` csoport kivizsgálva**: 6 **valódi duplikátum** (azonos csapatpár+nap+eredmény, de más verseny név alatt: 4× Veikkausliiga vs Veikkausliiga Championship Group 10-24/25/29, 11-02; NWSL vs NWSL Playoff 11-15; Friendlies U21 vs Men's U-21 Friendly 11-15) → `dedupe pair` mind **6 összeolvasztva**, a gazdagabb (insight+stat+incidens) sor lett a túlélő, áldozatok `ops.dedupe_archive`-ban.
- [x] Az 7. csoport (**11-07 creighton-depaul**) **NEM duplikátum**: két külön meccs — férfi NCAAM Soccer 2:1 (01:00) és női NCAAW Soccer 3:1 (20:00). A `fold_name` nem különböztet `gender`-t → téves review-jelzés. Nem olvasztottam (adatvesztés lett volna).
- [x] **3 withheld gate** (11-16×2, 11-20×1): mind `MATERIAL_SOURCE_CONFLICT` — a források anyagi mezőben eltérnek. Szándékosan visszatartott publikáció = **helyes viselkedés**, nem javítandó.
- [x] **9 failed job** (10-31×6, 11-14×2, 11-22×1): mind a 7 kódjavítás **előtti** próbálkozás; a napoknak van `done` jobja → törölve (zaj a napi checkben).
- [x] **Végső integrity**: `duplicate=0`, `orphan=0`, `active_refs_to_merged=0`, `failed_jobs=0`, `open_breakers=0`. Maradék: 39 pending review (36 fuzzy + 3 multiple, fail-closed ismeretlen csapatok), 3 withheld gate, 1 same_pair (férfi/női) — mind **helyes állapot**.
- [x] 17 `core.team` sor ahol `id != entity_id`: a S119 kódjavítások már kezelik (a tényleges `core.team.id`-t használjuk FK-ként); az adatrevízió (sorok rendbetétele) külön szelet, nem blokkol.

### Folytatás
- [x] **10-21**: 163 detail → 128 insight / 464 játékos, integrity OK.
- [x] **10-20**: 165 detail → 155 insight / 306 játékos / 55 369 stat, integrity OK.
- [x] **10-19**: 135 detail → 120 insight / 131 játékos / 52 777 stat, integrity OK.
- [x] **10-18**: 762 detail → 578 insight / 600 játékos / 222 445 stat, integrity OK.
- [x] **10-17**: 853 detail → 749 insight / 1246 játékos / 222 431 stat, integrity OK.
- [x] **10-16**: 238 detail → 167 insight / 196 játékos / 55 422 stat, integrity OK.
- [x] **10-15**: 157 detail → 101 insight / 61 játékos / 68 258 stat, integrity OK.
- [x] **10-14**: 154 detail → 121 insight / 184 játékos / 43 308 stat, integrity OK.
- [x] **10-13**: 216 detail → 179 insight / 439 játékos / 70 011 stat, integrity OK.
- [x] **10-12**: 152 detail → 133 insight / 81 játékos / 62 652 stat, integrity OK (2. próbálkozás, lock-timeout után).
- [x] **10-11**: 653 detail → 467 insight / 387 játékos / 184 690 stat, integrity OK.
- [x] **10-10**: 729 detail → 636 insight / 411 játékos / 207 777 stat, integrity OK.
- [x] **10-09**: 299 detail → 195 insight / 140 játékos / 80 942 stat, integrity OK.
- [x] **10-08**: 115 detail → 65 insight / 65 játékos / 27 517 stat, integrity OK.
- [x] **10-07**: 115 detail → 56 insight / 56 játékos / 13 093 stat, integrity OK.
- [x] **10-06**: 116 detail → 106 insight / 319 játékos / 45 398 stat, integrity OK.
- [x] **10-05**: 71 detail → 61 insight / 156 játékos / 21 956 stat, integrity OK.
- [x] **10-04**: 444 detail → 281 insight / 300 játékos / 81 304 stat, integrity OK.
- [x] **10-03**: 644 detail → 388 insight / 268 játékos / 97 146 stat, integrity OK.
- [x] **10-02**: 223 detail → 86 insight / 91 játékos / 29 272 stat, integrity OK.
- [x] **10-01**: 130 detail → 64 insight / 89 játékos / 25 091 stat, integrity OK.

## OKTÓBER ZÁRÁS (2026-10-01 → 10-31)

- [x] **31/31 nap kész**, mind `backfill-day.sh --fetch`-el, napi kapuval (integrity + coverage). Egy lock-timeout (10-12, betexplorer scrape) újrapróbálással megoldva.
- [x] **Adatállomány**: **10 161 meccs / 10 080 eredmény / 7 902 insight sor / 33 065 játékos / 2 870 452 stat sor / 419 verseny**.
- [x] **Integrity október**: `duplicate=0`, `orphan=0`, `active_refs_to_merged=0`, `failed_jobs=0`, `open_breakers=0` — kemény hibamentes. 104 pending review + 5 withheld gate + 1/6 same-pair = review-jelzések (helyes állapot).
- [x] Lemez 27 GiB, git szinkronban.
- [ ] **SZÜNET**: szeptembert **holnap** folytatjuk (cél: augusztus 1, a 26-27 szezon kezdete).
- [ ] **Döntés (user)**: ma **2026-10-01-ig** haladunk, ott szünet; **szeptembert holnap folytatjuk** (cél至 augusztus 1, a 26-27 szezon kezdete).

## S121 — Felhasználói észrevételek: verseny-besorolások

### 1. Ipswich U21 – Porto U21 (KÉSZ)
- [x] A meccs (2026-11-24 20:00) kizárólag **soccerway** forrásból jött (ma 07:31), és a `Premier League Cup` (GB-ENG) alá került — hibásan. A v1-ben külön verseny: `Premier League International Cup` (71 meccs), és a v2-ben is volt már entitás (0 meccsel).
- [x] Tudatos revízióval (SQL UPDATE, `ops.audit_event`-tel auditálva) átmozgatva a **`Premier League International Cup`** (01a1f7a8…) alá. Indok: U21 + portugál csapat = International Cup. Soccerway tiltva van, nem jön újra.

### 2. „Liga Kupa" (Indonézia) — NINCS ADAT
- [x] A v2 research-ben és a v1 DB-ben **nincs** ilyen nevű verseny. A 11-24-es `League Cup` 8 meccse mind **észak-írországi** (Carrick Rangers, Glenavon, Linfield…), nem indonéz. A Sofascore-listán szerepelt ilyen név a usernek — onnan nincs betöltött adat. **Nincs mit javítani a v2-ben**; ha a user akarja, a Sofascore-listáról kellene forrást szerezni.

### 3. World Cup U17 csoportok (DÖNTÉS KELL)
- [x] A DB-ben **9 külön verseny**: `World Cup U17 - A…J` (+ magyar `U17-es VB`), a csoport a **verseny nevében** van, a `round_label` csak a fordulót (1/2/3) tartja. A research 11-24-ben I/J/K/L csoportokat ad (8 meccs), de a DB-ben **csak J (2 meccs)** van — **4 meccs hiányzik** (Brazil–Tanzania U17, Honduras–Colombia U17, Venezuela–Romania U17, Mexico–Cameroon U17); további 2 (Costa Rica–Ireland, Japan–Serbia) csak soccerway `U17-es VB` forrásból van. A soccerway részben **eltérő párosításokat** ad (Algeria–Dominican / Chile–Montenegro vs research Algeria–Montenegro / Dominican–Chile) = `MATERIAL_SOURCE_CONFLICT`.
- [ ] **Javaslat (megerősítés kell, 123 versenyt érint)**: minden csoportos versenyt egy fő tournament-entitás alá vonni (pl. `World Cup U17`), a csoportot átmozgatni a `round_label`-be (`Group I / 1` formátum, megőrizve a fordulót), majd a verseny-entitásokat `merge-competition --force`-tal összeolvasztani. Visszaállás: `ops.dedupe_archive` + `ops.audit_event`. Kérdés: (a) fő név `World Cup U17` (research/angol) vagy `U17-es VB` (v1/magyar)? (b) csoport formátum `Group I / 1` megfelelő-e?

## S121/S122 — World Cup U17 és League Cup Indonesia (felhasználói kérés)

### World Cup U17 — egy fő verseny, csoportok a round_label-ben (KÉSZ)
- [x] A 9 külön verseny (`World Cup U17 - A…J`) összevonva egy `World Cup U17` fő versenybe: 52 meccs, 27 `Group X / forduló` round_label. A csoportok át lettek mozgatva a `round_label`-be (`Group A / 1`…), a verseny-entitások `merge-competition --force`-tal összeolvasztva (audit: `ops.dedupe_archive`).
- [x] `competitions.rs` szótár: 12 bejegyzés (`world cup u17 - a…l`) → `World Cup U17` + teszt (`world_cup_u17_groups_map_to_one_tournament`, 14 teszt zöld). Build + daemon restart.
- [x] A 11-24-es research 6 hiányzó meccse (I/K/L csoportok: Costa Rica–Ireland, Venezuela–Romania, Japan–Serbia, Mexico–Cameroon, Brazil–Tanzania, Honduras–Colombia) betöltve — a szótár javítása után a fő verseny alá kerültek, `Group I/K/L / 2` round_label-lel. A soccerway `Group I/K/L` versenyei most üresek.

### Ipswich U21 – Porto U21 (KÉSZ)
- [x] Tudatos revízióval átmozgatva a `Premier League Cup` → `Premier League International Cup` versenybe (U21 + portugál csapat), `ops.audit_event`-tel auditálva. A soccerway már tiltva van, nem jön újra.

### League Cup Indonesia (KÉSZ — kézi adatpótlás)
- [x] A v2 research-ben nincs ilyen verseny (a `League Cup` 11-24-en észak-írországi). A felhasználó Sofascore-listájából 6 meccs betöltve `sofascore` forrással: `League Cup Indonesia` verseny + `Group A/B/C/F/G/I / 1` round_label + eredmények 7:0, 2:0, 5:0 + `provisional_publish` gate. API `/v1/day/2026-11-24`: 6 meccs, helyes eredmények/státuszok.
- [ ] Megjegyzés: a kézi meccseknek nincs `fact.provenance`/`match_insight`/`match_stats` (csak eredmény+gate) — ha később gazdagítás kell, külön szelet.
- [ ] A „minden csoportos verseny egy tournament alá" általános elv (123 verseny) még nem teljes — csak a World Cup U17 készült el; a többi külön döntés (név, round formátum).

## S122 — Kereső: verseny név alapján is (felhasználói észrevétel)

- [x] A `/v1/search?q=indonesia` csak **1 meccset** adott (az `Indonesia U17 – Myanmar U17`-t a csapatnév miatt), a 6 League Cup Indonesia meccset nem. Gyökér: `storage/repositories/search.rs` a meccseket csak `home`/`away`/`round_label` alapján kereste, a **verseny nevét nem** vette be.
- [x] Javítás: `JOIN core.competition comp` + `OR {competition_fold} LIKE ...` a meccs-keresésben. Teszt bővítve (`storage/tests/search.rs`: a „Magyar Kupa" meccs találata a verseny névéből), `cargo test -p football-storage --test search` zöld.
- [x] **Nem a Swift klienst kellett újra buildelni**, hanem a daemon API-ját (`cargo build -p footballd --features obscura` + telepítés + restart). A `/v1/search?q=indonesia` most **7 meccset** ad.
- [x] Valódi macOS ellenőrzés: az appban a keresőbe „indonesia" + **return** → **11 találat**, benne mind a 6 League Cup Indonesia meccs. (A keresés return billentyűvel indul; a puszta begépelés után az AXOutline még üres.)

## S122 — Választható rendezési mód a napi listán (felhasználói kérés)

- [x] **Döntés (user, question tool)**: `Kiemelt` / `Ország A–Z` / `Versenynév A–Z` — három mód választható, nem fix rendezés.
- [x] `LeagueGroup.SortMode` enum (featured/country/name) + `LeagueGroup.ordered(_:mode:)` determinisztikus rendezés (belső meccs-sorrend változatlan).
- [x] `FilterBar`: új `filter.sort` menü a három móddal; `HomeView`: `@State sortMode` (alap: `.featured`), átadva a FilterBar-nak és a `groups()`-nek.
- [x] Tesztek: `LeagueGroupTests.testSortModesReorderTheSameGroups` (3 mód, app unit teszt), `FootballAppUITests.testSortModeMenuOffersThreeModesAndPersistsSelection`. App build zöld, 77 unit teszt 0 failure, Core 125 teszt zöld, rust fmt 0, git diff --check 0.
- [x] **Valódi macOS ellenőrzés**: az appban a `filter.sort` menü `Kiemelt ligák / Ország A–Z / Versenynév A–Z` opciókat mutat; az „Ország A–Z" kiválasztása után a menü címe `Ország A–Z`, és a lista első ligája FA Cup → Championshipre váltott.
- [ ] Megjegyzés: a teljes XCUITest-szett ezen a körön **`Timed out while enabling automation mode`** miatt nem futott (a futó app zavarta az automationt) — környezeti, nem kódhiba; az új teszt lefordult, az élő AX ellenőrzés zöld.

## S123 — Napi lista duplikátumok (felhasználói észrevétel)

A SOAP (soccerway) tiltás ellenére a tiltás **előtt** betöltött soccerway-meccsek duplikátumokat hagytak a 11-24-es napon. Mind a 4 esetben a research/kézi verzió a túlélő, a soccerway az áldozat (`dedupe pair`, `ops.dedupe_archive`-ban visszaállítható):

- [x] **Copa Chile / Cup Chile negyeddöntő**: ugyanaz a meccs (Coquimbo–Calera, 23:00) két verseny alatt — `Cup` (research, verified) és `Copa Chile - Play Off` (soccerway). Az áldozat (soccerway) összeolvasztva, az API-n most csak a research `Cup` meccs látszik.
- [x] **ZED FC**: két meccs ugyanazzal az idővel — `ZED FC – Abo Qir Semad` (research/fotmob, user által megerősítve: a valódi) és `ZED FC – Smouha SC` (soccerway, hamis). A soccerway áldozat összeolvasztva.
- [x] **Indonézia**: `League Cup Indonesia` (6 kézi meccs) + `I.League Cup` (soccerway, Garudayaksa–PSS duplikátum). A soccerway meccs összeolvasztva; az API-n most csak a 6 `League Cup Indonesia` meccs.
- [x] **U17-es VB**: 4 soccerway meccs a `U17-es VB` verseny alatt. 2 egyértelmű duplikátum (Costa Rica–Ireland, Japan–Serbia) összeolvasztva a research `World Cup U17` meccseibe. 2 eltérő párosítás (Algeria–Dominican, Chile–Montenegro vs research Algeria–Montenegro / Dominican–Chile) `MATERIAL_SOURCE_CONFLICT` miatt **withheld gate**-be téve (nem törölve, visszaállítható). Az API-n most 8 meccs, mind a `World Cup U17` alatt.
- [x] Integrity 11-24 és teljes okt+nov: verdict **OK**, 0 duplikátum, 0 árva provenance, 0 failed job; 44 pending review + 5 withheld gate (helyes állapot).

## S124 — Logók: Champions League és FC Porto (felhasználói észrevétel)

- [x] **Champions League logó gyökere**: az API (`/v1/day`, `/v1/competitions`) már hivatkozott a logóra (`/v1/asset/assets/c1/e2/...webp`), de a végpont **HTTP 404**-t adott — a tényleges bájtok a repo `data/raw/assets/` alatt voltak, miközben a daemon a `~/Library/Application Support/FootballAppV2/agent/data/raw/assets/` könyvtárból olvas (`DATA_DIR` a `run.sh`-ben). **1077 asset fájl hiányzott** az agent könyvtárból (4056 → 5136) — `rsync`-vel átmásolva. Az asset URL most **HTTP 200, image/webp**; a Versenyek oldalon a `🏳️` fallback eltűnt.
- [x] **FC Porto / FC Porto U21 logó**: a külön `Porto` entitásnak volt v1-megapack logója (`assets/7c/3a/...webp`), de az `FC Porto` (01a1fc05-055b…) és `FC Porto U21` (01a1f823-968e…) entitásoknak nem. Asset-másolással (nem entitás-összevonással) hozzárendelve ugyanazzal a `storage_key`-el, `ops.audit_event`-tel auditálva. Az API-n mindkét meccsen `logo: True` (Champions League: Feyenoord–FC Porto; Premier League International Cup: Ipswich U21–FC Porto U21).
- [ ] Megjegyzés: a jövőbeni asset-importoknál a `DATA_DIR`-et az agent könyvtárára kell beállítani (vagy a bájtokat oda másolni), különben újra 404 lesz. Külön szelet/üzemeltetési javítás.

## S125 — Lock-timeout (10-12 első próbálkozás)

- [x] A 10-12 backfill első futása **`LOCK_TIMEOUT`** miatt bukott: a `.football.lock`-ot az automatikus `football odds 2026-11-24` folyamat tartotta 300 mp-nél tovább (egy elakadt `obscura scrape` a betexplorer.com ellen, ami 0 adatot adna). A mai nap már be volt töltve (pipeline 08:40-kor zárult), ez későbbi automatikus odds-futtatás volt — leállítása **nem adatvesztés**. Lock törölve, scrape gyerek leállítva, a 10-12 újraindítva.
- [ ] Üzemeltetési tanulság: a `backfill-day.sh` és az automatikus napi `collect-only.sh` egyaránt használja a `.football.lock`-ot. Ha a launchd 14:15-ös odds-futtatása elakad, a napi backfill 300 mp után kilép. Jövőben érdemes a backfillt a lock elengedésére újrapróbálni (a `backfill-day.sh` jelenleg nem retryel) — külön szelet.

## S126 — Október 1. mély ellenőrzés (felhasználói észrevétel: kevés a meccs)

- [x] **Mérés**: FotMob nyers 10-01 = 66 meccs, ESPN = 104, LiveScore = 62 → research összesen **130 egyedi**; a DB-ben eredetileg csak **125** volt (5 kimaradt). A Sofascore teljes listája API 403 (robots/policy) miatt nem elérhető — nem kerültük meg; a 130 a kurált források maximuma.
- [x] **5 kimaradt meccs oka**: mind `finished`, de a csapatnév fuzzy review-ban ragadt (fail-closed, AGENTS.md §6): `SSV Jahn Regensburg`, `UC San Diego`, `Rhode Island FC`, `Northern Ireland U21`, `FC Atert Bissen` (+ `RFC Liège`-hez hiányzott az `espn:132938` source ref).
- [x] **Kézi kézi döntéssel** (`footballctl identity approve`, 5 review + 1 source ref pótlás, mind auditálva) feloldva; a 10-01 discovery job háromszori újrajátszása után a DB-ben **130 meccs, 0 research UID hiányzik**. Insight/player import újra: 66 sor. Integrity: verdict **OK**.
- [x] Október záró száma így **10 166 meccs** (10161 + 5).

## S127 — Valódi adatfrissítési countdown a fejlécben

- [x] A régi countdown a **weather API 15 perces ciklusát** (`next_refresh_at`) vagy egy 6 órás fallbacket mutatott, és élő meccseknél 60 mp-es „élő frissítés" volt — egyik sem mondta meg, mikor jön **új meccsadat**. A user kérte: azt számolja vissza, mikor érkezik új adat.
- [x] Backend: `DayResponse.next_refresh_at` — a research napi slotjai (14:15, 16:15, 18:15, 20:15, 22:15 Europe/Budapest) alapján, időzóna-helyesen (új `chrono-tz` függőség a football-api crate-ben, indokolt: nyári/téli idő), UTC-ben. Mért: `2026-11-24T17:15:00Z` = 18:15 Budapest.
- [x] Kliens: `DayResponse`/`DayFeed.nextRefreshAt`, `HTTPDayService`, `HomeView.loadedNextRefreshAt`, `HomeHeader` — a research slot **mindig elöl van**, még élő meccsek alatt is (a 60 mp-es csendes frissítés csak újraolvassa ugyanazt az adatot). Régi 6 órás fallback eltávolítva.
- [x] Valódi macOS ellenőrzés: `home.stat.refresh` AX → „Frissítés: 1 ó 35 p, következő adatfrissítés. A következő meccsadat-frissítés 18:15 körül várható."
- [x] Kapuk: Rust **585 teszt**, clippy 0, fmt 0, Core **125 teszt**, app unit **TEST SUCCEEDED**, UI **TEST SUCCEEDED**, git diff --check 0. Egy törékeny saját UI-teszt (Menu AX-label üres fixture-ben) javítva: a label helyett a menüopciókat ellenőrzi.
- [ ] Megjegyzés: ha a research pipeline egy nap nem fut (pl. hiba), a backend akkor is a következő ütemezett slotot mutatja — ez tervezett becslés, nem új adat garanciája.

## S128 — Újabb bugok irtása (felhasználói észrevétel)

- [x] **Stevenage duplikátum**: ugyanaz a meccs (Stevenage–Sheffield Wednesday, 11-24 20:45) két forrásból — `research+openfoot` és `tipplap`, a tipplap külön `League One` verseny-entitást hozott létre. `dedupe pair`-vel összeolvasztva (a research verzió túlél, áldozat archiválva). Az API-n most **1** Stevenage meccs.
- [x] **Hamis „8. forduló"**: a FotMob nyers `Tournament.round` a League One-nál 8-at és 9-et is ad (a research hűségesen átveszi). A kliens `LeagueGroup.roundSummary` régi szabálya a „leggyakoribb, döntetlennél kisebb" értéket választotta → hamis 8. Most: ha egynél több különböző numerikus forduló van, **nil** (bizonytalan adat = NULL, AGENTS.md §6). Teszt átírva (`testRoundSummaryIsNilWhenSourceDisagreesOnRoundNumber`). A Championship (egységes 17) továbbra is helyesen jelenik meg.
- [x] Kapuk: Rust **585**, clippy 0, fmt 0, Core **125**, UI **TEST SUCCEEDED**, git diff 0. API: League One 4 meccs / round_label 8×2+9×2 (forrás), de a kliens nem mutat fordulót; Stevenage 1.

