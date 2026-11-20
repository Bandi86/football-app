# Döntés: meccs-szintű forrás-híd és név-variáns linkelés (S37b)

Dátum: 2026-11-20. Szelet: S37b. Előzmény: S37a felmérés és terv
(`docs/superpowers/plans/2026-11-20-forras-osszehangolas.md`).

## Probléma

A v2 `identity.source_entity_ref` táblája csak csapat- és játékvezető-szintű
hídat tartott. Ugyanaz a meccs két forrásból csak akkor olvadt össze, ha a
csapatnevek pontosan (foldolva) egyeztek és a `core.match` natural-key
(verseny, szezon, csapatok, kickoff) is egyezett. Az eltérő névváltozatok
(`SV Darmstadt 98` / `Darmstadt`, `TNS` / `The New Saints`) külön kanonikus
meccset hoztak létre — ez volt a 2026-11-20-i 8 közeli duplikátum-pár és a
tipplap-oddsok elmaradásának (29/131) fő oka.

## Döntés

1. A kanonikalizáló a natural-key keresés után **név-variáns linkelést** végez:
   azonos versenyen belül, ±12 órás kickoff-ablakban, a forrás csapatneveit a
   meglévő meccsek csapatneveivel hasonlítva. Csak pontosan egy jelölt esetén
   linkel; több jelöltnél a legkorábbihoz (determinisztikus), és figyelmeztet.
2. Név-variáns linknél a kanonikus `core.match` sor **nem íródik felül**
   (a forrás nem írhatja át a meccs csapatait/időpontját); a forrás-hozzájárulás
   és az odds a meglévő meccshez kerül.
3. Minden materializált forrás-sor kap **meccs-szintű `source_entity_ref`**
   hídat (`method='exact_external_id'`; név-variáns linknél `confidence=0.9`,
   hogy az audit meg tudja különböztetni az ID-egyezéstől).

## Miért nem más megoldás

- Kizárólag ID-alapú híd: a v2 forrásai (soccerway/openfoot/footballdata/
  tipplap) között nincs közös ID-névtér (a reep CC0 hídjában sincs soccerway-sor),
  ezért a név-alapú összevonás elkerülhetetlen a napi lefedettséghez.
- Kizárólag a v1 alias-táblájának átemelése: a v1 27 307 csapatos univerzumából
  indul, a v2-nek 2 812 csapata van; a seedelés csak a már létező entitásokra
  tud alias-t tenni (S37d), a név-variáns linkelés viszont forrásonként is működik.

## Korlátok, kockázat

- A név-hasonlítás heurisztika: csak kontextussal (verseny + nap) együtt
  használjuk, és több jelöltnél nem találgatunk (legkorábbi + warn).
- Ismeretlen csapatú sor továbbra is kimarad (fail-closed); a tipplap-események
  teljes linkelése az S37d (identitás-seed) után várható.
- A meglévő duplikátumok nem tűnnek el automatikusan: az S37c merge szelet
  dolga; ez a döntés csak a jövőbeli újratermelődést akadályozza.

## Bizonyíték

- `cargo test -p football-ingest --test live_materialize -- --test-threads=1`:
  10/10 (új: `variant_team_names_link_to_the_existing_canonical_match`).
- `cargo test -p football-domain --test identity_rules`: 8/8 (új:
  `club_name_variants_are_likely_the_same_team`).
- Teljes kapu: 320 passed / 0 failed; `check-foundation` exit 0; fmt/clippy tiszta.

## Utólagos éles mérés (2026-11-20, daemon újratelepítés után)

- 24 meccs-szintű híd keletkezett egy kényszerített tipplap refreshből, mind
  `confidence=1.0` (natural-key egyezés); a napi meccsszám nem nőtt → nincs új
  duplikátum.
- Az odds-lefedettség nem javult (29/131): a tipplap-sorok nagy része a
  csapat-identitásnál kimarad, tehát a meccs-linkelésig el sem jut. A tényleges
  lefedettség-emeléshez esemény-szintű linkelés kell (v1 `link_events.py`
  szabályai) + identitás-seed (S37d) — ez a következő szelet.
