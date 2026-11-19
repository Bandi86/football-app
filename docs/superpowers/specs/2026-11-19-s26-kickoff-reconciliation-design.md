# S26 — Kereszt-forrás kickoff-rekonciliáció (design)

Dátum: 2026-11-19. Státusz: **jóváhagyva és implementálva** (2026-11-19, a felhasználó
„igen dolgozhatsz tovabb” válaszával; eredmény: checkpoint 2026-11-19). Kapcsolódik: S23f (v1 keresztimport), S16/S19/S20 (dedupe/merge).

## Probléma (mért, 2026-11-19)

A v1 import és a footballdata backfill ugyanazt a fixture-t **eltérő kickoff-idővel**
írta le, ezért a meccs-azonosság (competition + season + kickoff + csapatok) nem
fogta össze őket: párhuzamos kanonikus sorok születtek. A jelenlegi dedupe
(eszakt kickoff-kulcs) ezeket nem látja duplikátumnak.

Mérés (`core.match`, competition+season+csapat-pár+UTC-dátum csoportosítás):

- **4 646 csoport / 9 310 érintett meccs / 4 664 felesleges sor**
- Eredmény-egyezés: **4 615 azonos**, **13 eltérő**, 0 hiányos
- Legnagyobb: **Eliteserien 2 845** (v1 pilot vs new-league backfill; a kickoff-
  különbség szisztematikus: +1h 1762, +3h 536, +4h 171, +2h 132, +0.5h 85 …
  → időzóna-konverziós különbség a két forrás között)
- Továbbiak: Ligue 1 369, La Liga 353, Serie A 342, Premier League 210,
  National League 179, League One 178, League Two 146, Bundesliga 23, Ligue 2 1
- Dátumon átnyúló pár (kickoff <24h, de más UTC-nap): **1**
- Példa: Arsenal–West Ham 2015-08-09, 12:00 UTC vs 14:30 UTC, mindkettő finished.
- Csapatnév-változatok is rejtenek párokat (pl. „Man United” vs „Manchester
  United” külön entitás) — ezek a fenti számban NINCSENEK benne (különböző
  team-id); külön audit kell.

A 13 eltérő eredményű pár mind Eliteserien; köztük tükrözött orientációjú
(0-1 vs 1-0) és vélhetően rossz dátumú v1 sorok is — **emberi review**, nem auto-merge.

## Szabály (determinisztikus, nem fuzzy)

1. **Azonos fixture-jelölt**: competition + season + csapat-pár (tükrözés-
   toleráns, LEAST/GREATEST) + **azonos UTC-nap**. Ez a kulcs nem enged
   heurisztikát: ugyanaz a két csapat ugyanazon a napon ugyanabban a
   versenyben legfeljebb egyszer játszik (a kivételeket a 2. pont fogja meg).
2. **Auto-merge csak ha az eredmények orientáció-helyesen egyeznek** (a
   csapat-pár sorrendjére normalizálva), vagy az egyik oldalon nincs eredmény.
   Eltérő eredmény → **nincs merge**, review-listára kerül (fail-closed).
3. **Túlélő**: a footballdata-provenance-ú sor (kanonikus kickoff); ha mindkettő
   v1 vagy mindkettő footballdata, a legkorábbi `created_at`. A kickoff-konfliktus
   a forrás-prioritás szerint dől el, a vesztes kickoff az archívumban megmarad.
4. **Merge-tartalom**: a meglévő `merge_duplicate` gépezet (archívum →
   provenance átmozgatás → additív odds/stats feltöltés → gate → áldozat törlés
   → audit). Tükrözött párnál eredmény/odds oldalcseréje a meglévő szabály szerint.
5. **Dátumon átnyúló párok** (1 db) és a csapatnév-változatok: külön review/audit,
   nem része az auto-merge-nek.

## CLI / implementáció vázlat

- `storage/repositories/dedupe.rs`: `duplicate_fixture_groups(date?)` — az új
  kulcs; `is_same_fixture` őr (orientáció-helyes eredmény-egyezés); a merge a
  meglévő tranzakciós út.
- `footballctl dedupe fixtures [--date YYYY-MM-DD] [--competition N] [--apply]`
  (dry-run alap; a `matches` alparancs változatlan marad).
- Review-lista: `dedupe fixtures --report` a 13+1 kockázatos párral (csapatok,
  dátum, kétféle kickoff/eredmény, provenance-források).
- Tesztek: azonos nap + eltérő kickoff + egyező eredmény → merge; eltérő
  eredmény → elutasítás; tükrözött orientáció eredmény-normalizálással;
  archívum payload tartalmazza mindkét kickoffot; audit event.

## KPI (a szelet végén mérendő)

- fixture-szintű csoportok: **0** (a review-listán kívül)
- a 13+1 kockázatos pár: tételesen döntve (merge/withhold/javítás)
- coverage újramérés: a meccsszámak a forrás-valósághoz közelítenek
  (PL 2015/16: 760 → 380; Eliteserien ~4 000 → ~2 000+)
- reverify után a trust/gate állapotok frissülnek; 0 árva provenance

## Visszaállítási út

Minden merge az `ops.dedupe_archive`-ban teljes előtte-állapottal (match, gate,
result, provenance, odds, stats + mindkét kickoff). Visszaállítás: kézi
visszajátszás az archívumból (S16 minta). A review-listás párokhoz nincs írás.

## Kockázatok

- **Rossz dátumú v1 sorok**: ha két KÜLÖNBÖZŐ fixture azonos napra esik a v1
  hibája miatt, az egyező eredmény véletlen egybeesés lenne — a 13 eltérő
  eredményű pár ezt jelzi; az auto-merge csak egyező eredménynél enged, ezért
  a kockázat a véletlen egyező eredményű rossz-dátumú párokra korlátozódik
  (Eliteserien +1h eltolásnál a dátum ritkán vált; a mintában nem fordult elő).
- **Csapatnév-változatok**: a fenti szám alulbecsülheti a teljes duplikátum-
  mennyiséget; az audit külön lépés (identity aliases + exact-name merge).
- A top-up pass (S23 checklist 0) ezzel NEM helyettesíthető: az a forrásból
  kimaradt meccseket pótolja, ez a meglévő sorokat vonja össze. A coverage
  szerint a korai szezonok eredménye 100% — a top-up szükségessége a
  rekonciliáció után újra mérendő.