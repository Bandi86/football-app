# 2026-11-27 — Adatminőségi hibák: gyökérokok és kapu

Ez a dokumentum a felhasználó által 2026-11-27-en jelzett három esetet
vizsgálja ki. Minden szám a dev adatbázis read-only lekéréséből származik.

## 1. Paraguayan Primera — duplikátum és hiányzó eredmény (2026-11-25)

**Tünet:** a három meccs kétszer-háromszor szerepelt a listában, eredmény nélkül.

**Gyökérok:** a research napi JSON ugyanazt a három fixture-t két külön
bejegyzésként hozta:
- `fm:1000017473` / `fm:...472` / `fm:...476` — **date_only** kezdés (00:00 UTC
  placeholder), verseny: `Division Profesional: Clausura`, ország PY;
- `espn:401888557` / `...558` / `...643` — **pontos** 20:00 UTC kezdés,
  verseny: `Paraguayan Primera`, ország XX.

A kanonikus csapat-ID-k **megegyeznek** (pl. `Luqueño` és `Olimpia` ugyanazok
az entitások), de a `canonicalize.rs::find_cross_source_fixture` csak akkor
köt hidat, ha a verseny országa nem null, a kezdések 15 percen belül vannak,
és a verseny azonos. Itt az ország XX volt és 20 óra a különbség, tehát három
külön `core.match` sor keletkezett.

**Eredmény:** a research mind a hat obszervációja máig `status=scheduled`,
`result=null` — a forrás nem adott eredményt. Nem lehetett kitalálni.

**Javítás (S215):**
- az integritási riport új mutatója, `unresolved_duplicate_fixture_groups`,
  **kemény hibaként** fail-el (ID-alapú, tehát nem fuzzy név-egyezés; az
  azonos nevű férfi/női NCAA-csapatok nem esnek bele);
- a napi API (`fetch_day`) determinisztikusan egy sorra szűri a csoportot:
  a pontos kezdésű sort tartja meg, a `date_only` placeholder-eket elrejti;
  a hiány-számlálók (`fetch_day_gaps`) ugyanazt a szűrőt használják.

**Mérés:** 11-25-re a riport `FELOLDATLAN cross-source duplikátum: 3`,
`verdict: FAIL`. Az API 6 helyett **3** paraguayi meccset ad vissza, mind
pontos 20:00 UTC kezdéssel; `missing_kickoff` 7-ről 4-re csökkent.

## 2. Ciprus-kupa — nincs eredmény (2026-11-25)

**Tünet:** 8 meccs, mind `scheduled`, eredmény nélkül.

**Gyökérok:** a research napi JSON és az összes rögzített obszerváció ezeket
is `scheduled`/`result=null` értékkel tartalmazza még 11-27-i frissítésben is.
Más forrás (`fotmob`, `soccerway`, `footballdata`, `sofascore`) egyáltalán nem
rögzített hozzájuk obszervációt. **Nincs mit visszajátszani** — az adat a
forrásban hiányzik.

**Kezelés:** a kártyán a korábbi S214 óta `VÁRHATÓ` jelzés jelenik meg a lejárt,
de le nem zárt meccseknél; az integritási riport mostantól külön számolja:
`lejárt, scheduled, eredmény nélkül: 36` a 11-25…11-27 ablakban (globálisan
135). Ez adathiány-jelzés, nem kemény integritási hiba: eredményt nem
találunk ki (AGENTS.md §6).

## 3. Algéria — két kézi meccs elhalasztva (2026-11-25)

**Tünet:** a felhasználó által beírt `CR Belouizdad – CS Constantine` és
`USM Alger – MC Alger` meccsek `postponed` állapotban vannak.

**Gyökérok:** **nem rendszerhiba.** Az `ops.audit_event` tanúsága szerint a
felhasználó 2026-11-27 16:58-kor maga szerkesztette át mindkettőt
`scheduled` → `postponed` állapotra, `el lett halasztva` indoklással. A
manual-edit út observationt, provenance-t és audit eventet írt — ez a
kívánt, visszakövethető viselkedés. Ha a szándék most az eredmény rögzítése,
az újabb kézi szerkesztéssel vagy forrásadattal lehet megtenni.

## Miért nem derült ki korábban

A `footballctl integrity` már mérte az `azonos csapatpár azonos napon`
mutatót (11-25-re: 5), de a `has_hard_failures()` nem vette figyelembe, így a
verdict `OK` maradt. A `daily-check.sh 2026-11-26` ezért volt zöld. Az S215
óta a determinisztikus, ID-alapú duplikátumkemény hibának számít, a lejárt
eredményhiány pedig külön, látható mutató.

## Újra futtatható ellenőrzés

```bash
./rust/target/debug/footballctl integrity --from 2026-11-25 --to 2026-11-27
curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:$PORT/v1/day/2026-11-25"
```

## Visszaállás

- A kód `git revert`-tel visszaállítható; az API-szűrés nem ír adatot, így
  visszaállítás után a korábbi 6 sor újra látható.
- A kézi algériai szerkesztések audit eventben visszakereshetők; nem lettek
  felülírva.
