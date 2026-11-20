# Név-variáns duplikátumok felismerése és merge-elése (S37c)

**Dátum:** 2026-11-20
**Szelet:** S37c
**Állapot:** elfogadva, élesben lefuttatva a fejlesztői adatbázison

## Probléma

Ugyanaz a valós meccs több forrásból két kanonikus sorként létezik, mert a
források ugyanazt a klubot más néven írják (`SV Darmstadt 98` / `Darmstadt`,
`Llandudno FC` / `Llandudno`, `The New Saints` / `TNS`, `Dinamo Tbilisi` /
`Dinamo Tbiliszi`). A meccs természetes kulcsa a csapat-azonosítókat
tartalmazza, ezért a két sor nem olvad össze; a meglévő
`footballctl dedupe matches` pedig csak az **azonos csapat-azonosítójú**
párokat látja (a 2026-11-20-i 8 valós párból 0-t talált meg).

## Döntés

1. **Új felismerés** (`variant_duplicate_groups`): azonos verseny, legfeljebb
   12 órás kickoff-különbség, eltérő csapat-azonosítók, de a csapatnevek a
   lazább `names_likely_same` szabállyal egyeznek (normál vagy tükrözött
   orientáció). Az azonos csapat-azonosítójú párokat szándékosan kihagyja —
   azok a klasszikus dedupe hatásköre.
2. **Szigorú auto-merge** (`is_strict_variant_same_match`): a backfill csak
   akkor mergel, ha
   - azonos verseny és azonos nap,
   - a kickoff pontosan egyezik **vagy pontosan 2 órával tér el** (mért
     adathiba: a 2026-11-17-i U17/U19 soroknál az egyik forrás nyáron is
     CEST-eltolást használt novemberben),
   - a nevek a `names_strictly_same` szerint egyeznek (korosztály-jelölés
     levágása után fold-egyezés, exact token-részhalmaz, többtokenes névnél
     legfeljebb egy 1 szerkesztésnyi eltérés, egytokenes monogram),
   - tükrözött párnál az áldozat eredménye nem veszne el (különben kézi döntés).
3. **Merge** a meglévő `merge_duplicate`-tel, kiegészítve: a meccs-szintű
   `identity.source_entity_ref` híd átkerül a túlélőhöz, és az archívumba is
   bekerül (`source_ref`).
4. **Új parancs**: `footballctl dedupe variants --date YYYY-MM-DD [--apply]`
   (dry-run alapból; a túlélő: eredmény → odds → provenance → legkorábbi sor).

## Elvetett alternatívák

- **Csak a lazább szabály** (`names_likely_same` prefix/elgépelés): a
  `Dominica` / `Dominican Republic` pár bebizonyította, hogy nemzeti
  válogatottaknál tévesen egyezik — auto-merge-hez nem elég biztonságos.
- **12 órás ablak automatikus mergelése**: két különböző, aznap játszott
  meccs (utánpótlás vs felnőtt) téves összevonását kockáztatja.
- **Kézi lista**: nem skálázódik; a szigorú szabály + dry-run + archívum
  visszajátszható marad.

## Éles eredmény (2026-11-20, fejlesztői DB)

- `2026-11-20`: **9 merge** (8 név-variáns + 1 verseny-szintű azonos pár) →
  a napi meccsszám 131 → **122**, az odds-lefedettség **29/131 → 29/122**
  (nem veszett el egyetlen odds sem), `footballctl integrity` → `verdict: OK`,
  ezt követően a napon 0 variáns- és 0 pontos duplikátum-csoport.
- `2026-11-14` 1, `2026-11-16` 1, `2026-11-17` 66, `2026-11-18` 1,
  `2026-11-19` 2 merge; a 11-17-i 8 párból 7 a 2 órás eltolással mergelt,
  **1 pár kézi döntésre vár** (tükrözött, az áldozatnál eredmény van).
- Összesen **79 S37c-merge** az archívumban (`ops.dedupe_archive`), mindegyik
  visszajátszható; audit-esemény `match_dedupe_merge` minden merge-hez.

## Korlátok / következő lépések

- A név-variáns *keletkezését* nem szünteti meg: a hosszú távú megoldás a
  források közötti csapat-identitás (S37d alias-seed / crosswalk), ezért a
  napi takarításnak rendszeresnek kell maradnia.
- A 12 órás jelölés és a 2 órás DST-szabály adatmegfigyelésen alapul; új
  forrásnál felül kell vizsgálni.
- A visszajátszás kézi: az archívum JSONB-jéből lehet rekonstruálni, erre
  külön eszköz még nincs.
