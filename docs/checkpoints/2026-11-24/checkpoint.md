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
- [ ] **11-07 NYITOTT — döntés kell**: a `Rhode Island – Davidson` pár két **külön ESPN esemény** (`espn:401929719` 0–0 01:00, `espn:401933415` 2–2 17:00), mindkettő `NCAA Men's Soccer`, ugyanaz a nap. A `duplicate_fixture` check kemény hibát jelez, de a két külön eredmény miatt az automatikus merge **adatvesztést okozna** (az egyik eredmény elveszne). Nem olvasztottam össze. Döntés: (a) két valódi külön meccs → a check megkerülése review-jelzéssel, vagy (b) az ESPN duplikálta → kézi kiválasztás, melyik eredmény a valódi.

### Visszaállás / biztonság

- A 7 merge (6 CL/AFC + 1 livescore) minden áldozata a `ops.dedupe_archive`-ban van JSONB payloadtal.
- Soccerway tiltás: `UPDATE ops.source_policy SET enabled=true WHERE source_slug='soccerway'` visszaállítható.
- **Biztonsági teendő**: az agent `run.sh` plaintext OpenFoot API-kulcsot tartalmaz — Keychainbe kell kerülni, de ez külön üzemeltetési döntés, most nem módosítottam.

## Következő döntési pont
- Nov 7 és nov 9 kivizsgálása tesztekkel, csak a mai rendezés után.
- A soccerway tiltás tartósságának napi ellenőrzése a `daily-check`-ben.
