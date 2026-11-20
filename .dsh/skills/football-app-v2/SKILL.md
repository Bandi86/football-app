---
name: football-app-v2
description: "Use when developing football-app-v2: slices, gates, DB."
category: football-data
version: 0.1.0
author: Bandi (Andras Suslecz), Hermes Agent
license: MIT
platforms: [darwin]
metadata:
  hermes:
    tags: [football, rust, postgres, swiftui, ingest, identity, verification, slice-workflow]
    related_skills: [football-data-research, football-app-monitoring, memory-engineering]
---

# Football App v2 — fejlesztés és üzemeltetés

Repo: `~/Documents/Code/football-app-v2`. Rust workspace (domain/storage/ingest/verification/
identity/api/worker + `footballd` daemon, `footballctl` CLI), PostgreSQL `football_v2_dev`/`_test`,
SwiftUI kliens az `apps/` alatt. A repo `AGENTS.md`-je a kötő szerződés — azt olvasd először; ez a
skill a munkafolyamatot és a csapdákat hordozza, nem mondja újra a szabályokat.

## When to Use

- A v2 repo fejlesztése: új szelet (parser/adapter, storage, API, kliens), hibajavítás, refactor.
- Üzemeltetés: napi kör ellenőrzése, review-triage, bootstrap, dedupe/merge, backfill, coverage.
- „Folytassuk a football app v2-t" / checkpoint-felvétel jellegű kérések.
- v1 (`football-app`) vagy research pipeline munka esetén a `football-data-research` /
  `football-app-monitoring` skill tartozik hozzá, nem ez.

## Felkészültségi audit kérésekor

Ha a felhasználó azt kérdezi, megvannak-e a megfelelő skillek, dokumentáció és tesztek:

1. `skills_list` + `skill_view` a ténylegesen használt skillekre; területenkénti mátrix (terület → skill → fedett/hiány), a hiányzó domainekre `manage_catalog search`. A meglévő umbrella + topikus `references/` fedettségnek számít — ne csinálj új, szűk skillt csak azért, mert nincs dedikált név.
2. **Ne a dokumentumból következtess a készre**: olcsó probe-kkal nézd meg a DoD-kapukat a repóban (`.github/workflows`, `grep accessibilityLabel`, `grep openapi`, Swift/Rust tesztszámok, `git status`). A spec és a kód közötti eltérést hiányként, ne bizonyítékként kezeld.
3. A hiányt P0/P1/P2 sorrendben, futtatott eredménnyel mondd ki, és rögzítsd `docs/operations/development-readiness.md`-be; a „kész" szót csak lefuttatott kapu-kimenetre használd.
4. Csak akkor bővítsd a fő skill törzsét, ha a tudás minden szeletnél kell; egyébként `references/<topic>.md`. A túl nagy törzs minden betöltésnél tokent költ.

## Minden session indulása (sorrendben)

1. AGENTS.md → README → legutolsó `docs/checkpoints/<dátum>/checkpoint.md` → érintett spec/terv →
   `git status --short --branch`.
2. **A checkpoint állításait élő, read-only DB-diagnosztikával ellenőrizd** (job-státuszok, pending
   review-k, budgetek, breaker, mai gate-számok). A checkpoint terv, a DB az igazság: a napi
   automatikus körök a lezárás után is halmoznak review-kat és hibákat — a „0 pending" állítás
   másnapra elavulhat.
3. Egy szelet = egy tesztelt, commitolható egység; a zöld kapu után sorban haladj. A felhasználó
   „mindenre jóváhagyást adok” / „önállóan folytasd” felhatalmazása a commit/push-ra is kiterjed
   (lásd `AGENTS.md` §11); a gateway-blokkolt műveleteket (pl. launchd-telepítés) nem szabad megkerülni.

## Kapu-lánc (minden kód-szelet után, ebben a sorrendben)

```bash
cd ~/Documents/Code/football-app-v2
set -a && source .env && set +a
./scripts/db-reset.sh                 # CSAK a teszt-DB-t reseteli
./scripts/check-foundation.sh
cd rust
cargo test --workspace -- --test-threads=1   # storage tesztek közös teszt-DB-t használnak
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p footballd -p footballctl --features obscura
cd .. && ./scripts/db-reset.sh
```

- A crate-ek neve `football-ingest`, `football-storage`, … — a `-p ingest` nem létező csomag.
- **Kézzel hozzáfűzött kódnál futtass `cargo fmt --all`-t a lánc ELŐTT** — a lánc fmt --check-je
  elbukik a formázatlan teszt-kódon, és a hosszú lánc újrafutása percekbe kerül.
- clippy `-D warnings`: `is_none_or` kell, nem `map_or(true, …)`.
- A binárisokat mindig `--features obscura`-val építsd — anélkül a Soccerway-jobok „obscura feature
  nincs engedélyezve" transport hibával buknak el.
- **Háttérparancsban ne használd a `for spec in "a b c"; do set -- $spec; …` mintát**: a terminal
  background shellje zsh, amely nem bontja szét a változót, így `$2` üres lesz, és a parancs
  usage-hibával bukik — miközben a látszat az, hogy „lefutott". Írj explicit parancsokat vagy
  zsh-tömböt (`args=(a b c)`), és a háttér-naplóban ellenőrizd a tényleges exitet/sorokat.

## Agent-frissítés (parser/daemon változás után kötelező)

- A futó daemon a `~/Library/Application Support/FootballAppV2/agent/bin/` alól fut; a repo buildje
  nem elég. Frissítés = a build után `./scripts/install-footballd-agent.sh` **közvetlen** futtatása
  (bináris másolás + launchd restart); ez a normál út.
- **Ne osascript/Terminal wrapperrel indítsd a telepítőt** — az a gatewayen blokkolt lehet; a script
  közvetlen futtatása működik. Ha a launchd restart mégis jóváhagyást kér, egyszer engedélyezd.
- Telepítés után ellenőrizd: `launchctl print gui/$(id -u)/hu.bandi86.football.v2 | grep -E
  "state|pid"`, a logban az új `footballd elindult port=…` sort, majd `curl
  http://127.0.0.1:<port>/v1/health`-et. **A port minden induláskor változhat**, ezért a futó
  portot mindig a logból olvasd ki, ne a korábbi értéket használd.
- **Amíg a régi daemon-bináris fut, NE queue-zz újra jobokat**: a régi parserrel figyelne meg újra,
  és újratermelné a javított hibát (pl. rossz országú review-kat).

## Review-triage és bootstrap

- **Az elavult pending review blokkolja az újat**: unique index `(source_id, entity_kind,
  source_entity_id, reason) WHERE status='pending'`. Parser-/mapping-javítás után ELŐBB utasítsd el
  a hibás pending sorokat (`footballctl identity reject --note <ok> <id>`), és csak azután futtasd
  újra a napot — különben a javított megfigyelés nem hoz létre új review-t, a régi marad.
- `multiple_candidates` review: nézd meg mindkét jelölt entitást (meccsszám a `core.match`-ból),
  `identity merge-team --from --to` (dry-run alap, `--apply`), majd `identity approve --entity
  <túlélő> <review-id>`.
- **A merge után duplikált meccscsoportok maradnak** (a két entitás ugyanazokat a fixture-öket
  tartotta): keresd meg az érintett napokat (SQL a references-ben), és futtass
  `dedupe matches --date <nap> --apply` minden napra; a végén ellenőrizd, hogy teljes DB-n 0 csoport.
- `approve-create-all` (országos kontextus), majd `--allow-unknown-country` az őszinte NULL-oknak
  (kontinens-zászlós kupák klubjai, nemzetközi versenyek). Fuzzy jóváhagyás tilos.
- Bootstrap = 2 kör: napi jobok újrafuttatása → approve → még egyszer újrafuttatás (a jóváhagyás
  előtt kimaradt meccsek pótlása) → maradék approve → ellenőrzés.

## Napi jobok újrafuttatása

- Kérés/job: **footballdata 22** (ligánként 1 CSV), **openfoot 4** (registry + 3 nap-oldal),
  **soccerway 1**, **tipplap 1**. N nap újrafuttatása N× ennyi kérés A REGGELI KÖRÖN FELÜL —
  ideiglenesen emeld a `daily_budget`-et (pl. footballdata 60→120, openfoot 8→16), a végén állítsd
  vissza. A min_delay/breaker ettől még él.
- A Soccerway `route_for` csak **ma (`/`) és tegnap (`/tegnap/`)** oldalát szolgálja ki — régebbi
  nap nem fetch-elhető újra; az újrafuttatást ebben az ablakban tervezd.
- Requeue: `UPDATE ops.job SET status='pending', attempts=0, last_error=NULL WHERE domain='day'
  AND time_window IN (…)`. A daemon tick (60 s) felveszi; a day priority előbb fut a backfillnél.
- **A Soccerway `/tegnap/` route nem megbízható**: 200-at adhat 0 meccssorral
  (a `div.event__match[data-event-row]` marker többször sem jelenik meg).
  A naptár-popup/dayPicker útvonala **nincs mérve**; a nap-oldal kiszolgálása
  idő/session-függő, ezért a JS-interakciós parser építése előtt mindig mérd
  élőben, hogy a marker és a dayPicker tényleg a DOM-ban van-e. A Tipplap is a
  mai napot adja tegnapi kérésre (dátum-guard helyesen, permanensen elutasítja).
- **Halott oldal retry-ját ne ülje végig a budget**: a függő job backoff-tel újrapróbálkozik;
  a budget visszaállítása (pl. soccerway 6) quota_blocked-ra váltja a további próbákat — udvariasabb,
  mint 20 percenként újabb timeoutot kérni egy renderelhetetlen oldalra. A job holnap üres tervvel
  lezárul (route_for csak ma/tegnap).
- **Bootstrap-körök végén dedupe + reverify**: a csapatok feloldása után a kereszt-forrás meccsek
  egyszerre anyagiasodnak, és a verseny-név változatok (OpenFoot registry vs Soccerway: „LPF Apertura
  Play Off"/"Play Offs", „Slovnaft Cup"/"Slovak Cup", azonos nevű Ligue 1 eltérő országgal)
  duplikált csoportokat szülnek. A körök végén `dedupe matches --date <nap> --apply`, majd
  `reverify --from --to`; a **publish_gate tábla az igazság**, a reverify CLI-összegzője más
  részhalmazt számolhat (eltérő ablak-szűrés).

## Adatjavítási és rekonciliációs szabályok

- Részletes, minden adatműveletnél érvényes szabályok (mapping, kereszt-forrás dedupe, konfliktusfeloldás, withheld-triage, csapat-/fixture-duplikátumok): `references/data-ops.md`. Réselemzés és web-ellenőrzött eredményrevíziók: `references/gapfill-and-result-revisions.md`.

## Napi review-csúcs és coverage-korlát

- Egy reggeli ingest **több száz új review-t** hozhat, ha a forrás új ligákat
  kezd szolgáltatni (soccerway + openfoot). Előbb
  `identity review-summary`, majd a `fuzzy_candidate` sorokat **név szerint**
  vizsgáld: az azonos klub névváltozata (Almere City FC → Almere City) mehet
  `identity approve --entity`-vel, de a tartalékcsapat (Jong AZ, Sporting CP B,
  SK Rapid II, „II”, U21) **külön entitás marad**, még ha a fuzzy jelölt az
  anyaclub. A `no_exact_match` sorokat `approve-create-all`, az ország nélkülieket
  külön `--allow-unknown-country` zárja.
- A `footballctl coverage` alapból csak a **top 300 szezont** listázza (a CLI
  most `--limit` és `--competition` opcióval is fut). Réselemzéshez mindig
  magas `--limit` vagy verseny-szűrés kell — a top-300 riport nem mutatja, ha
  pl. az NB I 2006–2026 hiányzik.
- A soccerway napi fetch újrafuttatása időnként 45 s-es timeoutot ad (az eredeti
  reggeli fetch sikeres). Három próbálkozás után `status='cancelled'` + audit a
  helyes lépés; a további retry csak a napi kvótát égeti.

## Gap-fill és eredményrevíziók

- A szezonréseket a **teljes idővonalon** mérd (v1 vs v2 szezon-számok), ne csak
  a korai éveknél: a kisebb ligák később is hiányozhatnak. Kétkörös
  `v1-import --target-competition`, a körök között `approve-create-all`
  (+ `--allow-unknown-country`), utána `dedupe fixtures` + reverify + integrity.
- A kockázatos fixture-pároknál **ne a túlélő forrásból indulj ki**: páronként
  webes forrás dönt, és a kanonikus eredményt auditált revízióval írd át — így
  a pár azonossá válik és biztonságosan merge-elődik.
- Az `awarded` eredménye 3-0/0-3 lehet, de a jogcímet és az irányt igazolni kell;
  a `finished` lehet elhalasztott is (`postponed`), és előfordul fordított
  hazai/vendég — ilyenkor a csapat-ID-kat is cseréld. Eredmény nélküli meccsnek
  gyakran nincs `core.match_result` sora: `INSERT … ON CONFLICT` kell, nem UPDATE.
- A merge után elavult, provenance nélküli konfliktusok maradhatnak: web-igazolt
  kanonikus értéknél `resolved_manual` + indoklás (winner NULL) elfogadható,
  audittal.
- Részletes eljárás és lekérdezések: `references/gapfill-and-result-revisions.md`.

## Ellenőrzési sorrend minden adatművelet után: `reverify` az érintett időablakra → `integrity` (0 duplikátum, 0 árva, 0 withheld) → `coverage`.

## Bizonyíték és rögzítés

- Parser-hibánál előbb a **tárolt nyers rendert** nézd meg (capture → artifact → zstd, recept a
  references-ben): a HTML lehet helyes, és a mapping téves — a parser átírása ilyenkor rossz irány.
- Minden kód-/scraper-változás kap döntés-ledger sort (`memory-engineering` skill, `--domain
  football-app-v2`): rationale/predicted a mérés előtt, observed az élő ellenőrzés után.
- Checkpoint: `docs/checkpoints/<dátum>/checkpoint.md`, magyarul, `- [ ]` lépések csak futtatott
  ellenőrzés után pipálhatók; minden adat-művelethez visszaállítási út (archívum-tábla/audit).
- Commit/push a szelet végén a felhasználó állandó felhatalmazásával („mindenre jóváhagyást adok”, „önállóan folytasd”) történik; ha ilyen nincs, csak kifejezett kérésre. A „kész" állításhoz futtatott  kapu-kimenet kell, ne a kód szándéka.
- A szelet végén rövid, tényszerű összefoglaló: mi változott, mi van verifikálva
  (futtatott kapu-kimenet), mi maradt nyitva. Ha a futó daemon/bináris vagy egy
  blokkolt művelet miatt egy változás nem él, azt mondd ki expliciten — ne
  állítsd "kész"-nek.

## References

- `references/db-queries.md` — séma-oszlop térkép (az intuíciótól eltérő nevek), bevált diagnosztikai
  lekérdezések, duplikátum-nap keresés, fold-csoportos duplikált csapatok, több utasításos psql
  temp-tábla minta, coverage-értelmezés, auditált revízió minta, nyers artifact inspect recept.
- `references/data-ops.md` — részletes adatjavítási szabályok: ország-/verseny-mapping, fixture- és csapatduplikátumok, konfliktusok, withheld gate-ek.
- `references/gapfill-and-result-revisions.md` — réselemzés, kétkörös v1-import, kockázatos
  fixture-párok párhuzamos ellenőrzése, withheld-triage, eredmény-upsert és konfliktuslezárás.
- `references/roadmap.md` — aktuális célok, kész/nincs kész, fázisok és a következő döntési pontok.
- `references/rust-backend.md` — crate-határok, kapu-lánc, teszt- és migrációs konvenciók.
- `references/swiftui.md` — kliens-theme, nézetek, tesztek, akadálymentességi és UI-ellenőrzési rések.
- `scripts/team_dup_groups.py` — fold-azonos csapatduplikátumok listázása és merge-elése
  (`--apply`; dry-run alapból), a `footballctl identity merge-team` parancsot hívja.
