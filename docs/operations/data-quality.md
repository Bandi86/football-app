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

## Pillanatkép (2026-11-17, a teljes újraellenőrzés után)

- **40 847 meccs**, 142 verseny, 937 csapat; **verified 4 069**
  (keresztforrás-egyezés), **provisional 36 776** (egyetlen közepes
  forrás — őszinte jelölés), withheld 2, **0 duplikált csoport**,
  pending review 0.
- Az angol/olasz/spanyol/német/francia ligák teljes szezonjai 100%
  eredmény/odds lefedettséggel; a legtöbb backfill-meccs egy forrásból
  (footballdata, közepes) jön → provisional. A következő lépés a
  keresztellenőrzés bővítése (v1 import ugyanazokra a ligákra → két
  független forrás → verified).
- **v1 import (S23e):** Eliteserien 4 010 meccs (99,2% eredmény,
  98,1% odds), Ekstraklasa 4 562 meccs (96% eredmény; a lezártaknál
  100%, a 182 hiányzó ütemezett jövőbeli meccs) — provisional.
- Backfill köteg: 2015–2026 + 1993–2014 + 15 new-liga; fut (done 153,
  pending ~568); a Latin-1 kódolású régi CSV-k (EC/SC) dekódolása
  javítva, az 5 hibás job újraindítva.
- **Újraellenőrzés lefutott:** 39 219 meccs fetch nélkül újraszámolva;
  a gate-állapotok most a valódi forrás-összetételt tükrözik.

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
