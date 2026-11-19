# Checkpoint — 2026-11-19

## Induló állapot (read-only diagnosztika, 2026-11-19 délelőtt)

- Előző lezárás: S23 (2026-11-17 este, `02d5a23`); backfill **726 job done**,
  0 pending/failed; budgetek visszaállítva (footballdata 60).
- Adatállomány: **242 493 kanonikus meccs**, 237 193 eredmény (97,8%),
  187 881 meccs oddsszal (77,5%).
- Daemon fut (`hu.bandi86.football.v2`, pid 841), mai 4 napi job kész,
  breakerek zárva (kivéve demo-synthetic, ami naponta elbukik — nincs fixture).
- **275 pending identitás-review** (11-18: 184, 11-19: 91; Soccerway 232,
  OpenFoot 43) — a checkpoint óta gyűlt fel; a mai nap csak 9 kanonikus meccs.
- Mai gate: 7 verified_publish + 2 provisional_publish.

## Szelet 1 — Ország-térkép javítás + review-triage

### Gyökérok (bizonyítékkal)

- A „Jordán Kupa” csoport zászló-címe a nyers renderben **„Jordánia”**
  (artifact `2c1b463a…`, 2026-11-19), a megfigyelés mégis `country_code: DK`.
- `countries::by_localized_title` **első találatos részstring-egyezést**
  használt: „jordania” ⊃ „dánia” → JO helyett DK. Ugyanígy potenciálisan:
  „szomália” ⊃ „mali” (SO→ML), „fehéroroszország” ⊃ „oroszország” (BY→RU),
  „dominikai köztársaság” ⊃ „dominika” (DO→DM).
- `is_continent_flag` szintén részstring: „Dél-Afrika” ⊃ „afrika” →
  ország-zászló nemzetközinek minősülne (látens hiba).

### Kódváltozás (tesztekkel)

- [x] `by_localized_title`: **leghosszabb illeszkedő név nyer** (azonos
      hosszban tábla-sorrend, determinisztikus). A dekorált címek
      („Anglia - Premier League”) továbbra is feloldódnak.
- [x] `is_continent_flag`: **egzakt (foldolt) egyezés** a részstring helyett.
- [x] Hiányzó lokalizált nevek pótolva: Jamaika (JM), Francia Guyana (GF),
      Saint Martin (MF), Szent/Saint Vincent és a Grenadine-szigetek (VC);
      angol tábla: French Guiana, Saint Martin. `szint maarten` elírás
      javítva `sint maarten`-re.
- [x] 4 új unit teszt (ütközési párok, dekorált címek, új nevek, kontinens
      zászló): `cargo test -p football-ingest --lib countries` → **4/4 zöld**;
      soccerway/normalized/openfoot/tipplap parser tesztek → **21/21 zöld**.

### Adat-audit és tudatos revíziók (AGENTS.md §6: nem migráció)

- [x] Kár-felmérés: **1 verseny-entitás** rossz országgal (Jordán Kupa, DK,
      0 meccs), **6 pending review** DK-val (jordán klubok), **0 rossz
      csapat-entitás** (a fail-closed linker megfogta). 3 történelmi
      megfigyelés DK-val — bizonyítékként megmarad, felülírás nélkül.
- [x] **Ternana ikerentitás** (multiple_candidates review): v1-import és
      backfill-bootstrap párhuzamosan hozta létre (242 + 602 meccs).
      `identity merge-team --apply`: 242 meccs átmozgatva a 602-esbe,
      1 ref; a review a túlélőre jóváhagyva.
- [x] Merge utáni **108 duplikált meccscsoport** dedupe-olva 109 napon
      (`dedupe matches --date … --apply`): 108 merge, 0 kihagyva;
      **teljes DB-n 0 duplikált csoport** (ellenőrizve).
- [x] **10 review elutasítva** indoklással (`identity reject --note`):
      6 jordán klub (DK-hiba, újramegfigyelés JO-val) + 4 lokalizált
      nemzeti csapatnév (Jamaika/Francia Guyana/Saint Martin/Saint Vincent,
      a névtábla bővítése után kanonikus angol névvel jönnek újra).
- [x] **Jordán Kupa verseny-ország revízió**: DK → JO, `ops.audit_event`
      (`competition_country_revision`, actor `hermes-agent`, ok + 0 meccs
      a revízió pillanatában). Az újrafuttatás a JO-s megfigyeléssel
      megerősíti (upsert COALESCE).

### Élő újrafuttatás és bootstrap (2026-11-19, lefutott)

- [x] Agent újratelepítve a javított binárissal (osascript/Terminal minta,
      felhasználói jóváhagyással; pid 38901, majd az Új-Kaledónia pótlás
      után 39387). A daemon log: obscura fetcher aktív, worker elindult.
- [x] **1. kör**: 8 napi job requeue (11-18 + 11-19, mind a 4 forrás),
      ideiglenes budget footballdata 120 / openfoot 16. 7 job done;
      **tipplap 11-18 failed** (dátum-guard: a site a tegnapi kérésre a mai
      napot adja — helyes fail-closed, a tegnapi Tipplap-adat a tegnapi
      körből már megvan).
- [x] `approve-create-all`: **312 jóváhagyva** — köztük a 4 jordán klub
      **JO országgal** (Al Jazeera Amman, Al Salt, Hay Al Amir, Al Yarmouk)
      → a javítás élő bizonyítéka. Jordán Kupa verseny JO ✓ (a revíziót az
      újrafuttatás megerősítette).
- [x] **Új-Kaledónia U17** (U17 VB): a névtáblából hiányzott → pótolva
      (NC, lokalizált + angol), teszt, rebuild, agent-újratelepítés, a régi
      review elutasítva, mai render újra → **New Caledonia U17, NC** ✓.
- [x] **2. kör** (soccerway 11-19 + openfoot mindkét nap): kanonikus
      meccsszám **11-18: 13 → 22**, **11-19: 11 → 92**.
- [x] Maradék **37 ország nélküli review** `--allow-unknown-country`-val
      jóváhagyva (őszinte NULL): 18 UWCL női klub (kontinens-zászló),
      12 ASEAN Club Championship klub, 5 klubbarátságos („(Fin)/(Aus)/(Swe)”
      suffix), 2 iráni klub (OpenFoot registry-ország hiány). **Pending: 0.**
- [x] **3. kör** (ugyanaz a 3 job): 11-19 → 142 meccs; ezután **38
      kereszt-forrás duplikált csoport** (verseny-név változatok: LPF
      Apertura Play Off/Play Offs, Slovnaft Cup/Slovak Cup, Copa Venezuela
      Play Off/Play Offs, azonos nevű Ligue 1 eltérő országgal stb.) —
      `dedupe matches --date 2026-11-19 --apply`: **39 merge, 0 kihagyva**,
      **teljes DB-n 0 duplikált csoport** ✓. Gyökérok: OpenFoot registry-nevek
      nincsenek a verseny-szótárban (S22 follow-up, külön szelet).
- [x] `reverify --from 2026-11-18 --to 2026-11-19`: gate-állapotok
      **11-18: 11 verified + 11 provisional; 11-19: 11 verified + 92
      provisional**; withheld/quarantined **0**.
- [x] Budgetek visszaállítva: **footballdata 60, openfoot 8, soccerway 6** ✓.

### Ismert korlát / backlog (nem blokkoló)

- **Soccerway `/tegnap/` route halott**: a daemon 4 timeoutja + 3 kézi
  probe szerint az oldal 200-at ad, de **0 meccssort renderel** (a marker
  25 s alatt sem jelenik meg; a dátum-navigáció a mai oldalon JS dayPicker
  widget, nem link). A múltbeli nap fetch-eléséhez dayPicker-interakció
  kell — külön szelet. Következmény: 11-18 soccerway-only meccsei (pl.
  Jerash SC, Al Buqaa jordán klubok) nem anyagiasodtak újra; a pending
  11-18 job quota_blocked-ig retry-zik, holnap üres tervvel lezárul.
- **OpenFoot verseny-név crosswalk + registry-ország mapping**: a 38
  duplikátum gyökere; amíg nincs kész, a napi dedupe az interim (S21 minta).
- Tipplap múltbeli nap: a site a mai napot adja tegnapi kérésre (dátum-guard
  helyesen elutasítja) — a múltbeli fetch itt sem megoldott.
- „(Fin)/(Aus)/(Swe)” suffix ország-tipp klubbarátságos meccseken
  (determinisztikus suffix-parse, opcionális későbbi szelet).
- demo-synthetic napi bukás (nincs fixture) — breaker open; fixture pótlás
  vagy forrás tiltás döntés.

### Záró kapu (2026-11-19, a teljes kódváltozás után)

- [x] Teljes kapu-lánc: teszt-DB reset → foundation → workspace tesztek
      (`--test-threads=1`, 0 failed) → clippy `-D warnings` → fmt →
      binárisok `--features obscura` → teszt-DB reset. (Eredmény: zöld.)
- [x] Commit + push: **`e22727d`** → origin/main (02d5a23..e22727d),
      a felhasználó „minden mehet” jóváhagyásával, a 2026-11-18 checklist
      6. pontja szerint. A repo git-identitása visszaállítva a korábbi
      commitokéhoz (Bandi86).

## Következő döntési pontok

1. **Top-up pass** (S23 checklist 0. pont, elhalasztva): korai szezonok
   kimaradt meccseinek pótlása szekvenciális, udvarias tempóban.
2. **Coverage KPI** (S23 checklist 3.): eredmény ≥99%, odds ≥90%,
   0 duplikált csoport — jelenleg 97,8% / 77,5%.
3. Apró adósság: demo-synthetic napi bukás (nincs fixture) — breaker open;
   vagy fixture pótlás, vagy a forrás tiltása.

## Visszaállítási út

- Kód: `git checkout -- rust/crates/ingest/src/countries.rs` (commit előtt).
- Ternana merge: `ops.entity_merge_archive` (teljes előtte-állapot JSONB).
- Dedupe: `ops.dedupe_archive` (108 sor, teljes payload).
- Review-elutasítások: `identity.review_queue` (státusz + note, visszaállítható).
- Verseny-ország revízió: `ops.audit_event` + UPDATE vissza DK-ra (ha kellene).