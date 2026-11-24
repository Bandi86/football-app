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
