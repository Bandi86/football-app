# Adatminőség és adatbázis-állapot (S23, 2026-11-17)

A cél: **folyamatos, részletes kép** arról, milyen liga/ország/szezon
mekkora adatbázissal és milyen minőséggel él — hogy ne ismétlődhessen a
v1 hibája (sok forrás/összevonás után nyom nélkül sérülő adat).

## Ellátási lánc és bizonyíték

Minden kanonikus sorhoz forrás (provenance), rögzítési idő és ellenőrzési
állapot tartozik. A publikálási állapot (`ops.publish_gate.state`):

- **verified_publish** — a trust-mátrix szerint igazolt (magas bizalmú
  forrás, vagy két független közepes forrás egyező anyagi értékkel).
- **provisional_publish** — egyetlen közepes forrás (a kliens badge-dzsel
  jelzi; nem szűri ki).
- **withheld** — blokkoló lelet (pl. lezárt meccs érvényes 90 perces
  eredmény nélkül); nem jelenik meg publikáltként.

**Trust-javítás (2026-11-17):** a `load_material_values` eddig a kanonikus
meccs-ID-ra szűrt, de a megfigyelések a forrás rekord-kulcsát hordozzák —
így a konfliktus- és trust-szabály forrásérték nélkül futott, és egyetlen
forrás is „verified" lehetett. Új út: provenance → megfigyelés → aktív
forrás-ref → kanonikus csapat-ID, tükrözött soroknál pontszám-orientáció
igazítással. A korábbi gate-állapotok a `footballctl reverify` paranccsal
fetch nélkül újraszámolhatók.

## Riportok

- **`footballctl data-status [--from] [--to] [--top]`** — liga/ország
  bontásban: szezonok, meccs, eredmény %, odds %, verified/provisional/
  withheld darabszám; végén összesítés, duplikált csoportok, pending
  review-k.
- **`footballctl coverage [--from] [--to]`** — szezononkénti
  eredmény/odds lefedettség (KPI-mérés).
- **`footballctl reverify --from --to`** — gate-állapotok újraszámolása a
  tárolt megfigyelésekből (fetch nélkül).

## Pillanatkép (2026-11-17, lezárás után)

- **242 479 meccs**, 160 verseny, 1 925 csapat, 985 szezon, 701 Elo-rating.
- Gate: **verified 40 937** (két független forrás egyezése),
  **provisional 201 527** (egyetlen közepes forrás), withheld 15.
- **0 duplikált csoport, 0 pending review.**
- Fő ligák 34 szezonja: eredmény ~97–98%, odds ~70–80% (a régi szezonok
  CSV-iben nincs odds oszlop — ott az odds-lefedettség természetes hiány).
- A provisional tömeg oka: a backfill java egy forrásból (footballdata)
  jön; a keresztellenőrzés (v1) a 2015+ szezonokra ad második forrást
  (→ verified). A pre-2015 és a new-ligák egyelőre provisional-ok.
- Elhalasztott top-up pass: a korai szezonok néhány kimaradt meccse
  (site throttle a burst után) — későbbi szekvenciális szelet.

## Keresztellenőrzés, ELO, szezon-nézet (2026-11-17)

- **Keresztellenőrzés:** a v1 import ugyanazokra a ligákra/szezonokra
  (17 liga, 2015–2026) a footballdata mellé második forrást ad; a dedupe
  merge additívan mozgatja az oddsokat, így a trust-szabály két független
  közepes forrást lát → **verified**. Reverify után ~40 066 verified.
- **Saját Elo:** `footballctl elo [--as-of]` a lezárt meccsekből számol
  (155 305 meccs → 728 csapat), `fact.club_elo` `source='own-elo'`,
  `provenance='observed'`; a nap-API `home_elo/away_elo` adja. A rating
  as-of pillanatkép (mai dátum); történelmi as-of sorozat későbbi szelet.
- **Szezon-nézet:** `/v1/competition/{id}/seasons` + a kliens verseny-
  adatlap „Szezonok" füle (szezon / meccs / eredmény / odds).

## Figyelőlista (ismert kockázatok)

- **Verseny-név churn** ismeretlen kupáknál (pl. „Kings Cup - Italy"
  lokalizált renderből): a fix szótár bővítendő, amikor előfordul; a
  `data-status` sorai mutatják az új, gyanús verseny-neveket.
- **Ismételt import** (ugyanaz a forrás több futásból): a természetes kulcs
  miatt nem duplikál; a megfigyelések viszont futásonként új sorba
  kerülnek (content hash) — a provenance a legfrissebbet használja.
- **v1 örökség:** csak auditált, konzisztens sorok importálhatók; az import
  külön forrás (`v1-import`), így a forrás-összetétel bármikor
  visszakövethető.
- **Részleges szezon:** ha egy liga-szezon CSV később frissül, a backfill
  job újrafuttatása pótolja a hiányzó sorokat (idempotens).
- **Csapatnév-változatok:** a v1 és a footballdata eltérő rövid nevei
  (pl. „Manchester City" vs „Man City") külön entitást hozhatnak létre;
  az alias-seed (`identity seed-v1`) és az entitás-merge kezeli — a
  `data-status`/coverage sorokban duplikált csapatnév pár figyelendő.
- **Backfill-tempó:** a tick-en belüli delay-kapu ~1 job/tick-et enged;
  párhuzamos tick-loopok a breakert nyitják (rate-limit védelem) — csak
  szekvenciális, udvarias delay-jel futtatandó.
