# Forrás-összehangolás és napi teljes lefedettség — terv (2026-11-20)

**Cél:** minden nap a lehető legtöbb mérkőzés bekerüljön a nap-listába (minden liga,
osztály, férfi/női/ifjúsági), az oddsok a meccsekhez párosuljanak, az eredmény
ellenőrizhető és hiteles legyen, a nap-lista pedig duplikációmentes.

**Kiindulás:** a v1 (`football_app`) referencia működött és megbízható volt; a v2
jelenleg szűkebb nap-listát és sokkal kevesebb oddsot ad. Ez a dokumentum a v1
mérnoki mintáit bontja le, a v2 mért hiányaira képezi le, és szeletekre vágja a
javítást. Minden szám a 2026-11-20-i állapot mérése.

---

## 1. Mért kiindulási állapot

| Mutató | v1 | v2 |
|---|---|---|
| napi kanonikus meccs (2026-11-20) | 163 (147 tervezett + 16 lezárt) | 131 |
| ebből odds-szal rendelkezik | 109 (67 %) | 29 (22 %) |
| nap-lista sor (kanonikus + csak-tipplap) | 201 | 131 |
| odds-tárolás | `tipplap_odds` 3,65 M sor, `match_odds` 3,82 M (football-data) | `fact.odds_quote` (tipplap 87, footballdata 22, v1-import 11) |
| kanonikus csapat | 27 307 | 2 812 |
| csapat-alias | 4 558 | 2 303 |
| forrás-híd (meccs-szintű) | `external_id_map` 1,69 M meccs-sor | **0** (`identity.source_entity_ref` csak csapat/referee) |
| napi győztes források | 9 forrás (fotmob, sofascore, livescore, openfoot, football-data, …) | 4 valós: soccerway, openfoot, footballdata, tipplap |
| napi duplikátum-próba | `check-day-duplicates.cjs`, napi `dupl_meccs_nap` audit | nincs napi futásban |

v1↔v2 napi összevetés (becsült névpárosítás): közös 34, csak-v1 129, csak-v2 96.
A csak-v1 ligák: Szaúdi 9, J1 3, Regionalliga 11, 1. Division 11, Eerste Divisie 7,
Challenge League 5, Thai/Tunéziai/Algériai Ligue 1, női ligák — ezek a v2-ből ma
teljesen hiányoznak.

## 2. A v1 mintái, amiket át kell venni

1. **Forrás-híd tábla, soha nem felülíró upserttel** — `external_id_map`
   (`UNIQUE(source_id, entity_type, source_entity_id)`, `WHERE core_id = EXCLUDED.core_id`),
   plusz „a cél létezik” trigger. Ez a meccs-szintű provenance alapja.
2. **Review-olt alias-réteg** — `team_aliases` (4 558), `UNIQUE(alias)`, diakritika-fold
   index; a név-feloldás a forrás-ID → alias → foldolt név → szabály → queue sorrendben.
3. **Meccs-szintű linkelés a tipplap eseményekhez** — `link_events.py`: ±1 nap,
   név-szabályok, pontosan 1 jelölt, egyébként `match_link_candidates.needs_review`;
   nem hoz létre új meccset, ha van jelölt.
4. **Nevesített szabályok, nem hasonlósági küszöb** — `match_rule` exact/prefix/
   abbrev/continuation/monogram + `team_tags` (ország/nem/korosztály) kapu.
5. **Alias-tanulás a link-bizonyítékból** — ≥2 linkelt esemény ugyanarra az 1 csapatra,
   ellentmondás és névütközés kizárva (`alias_from_link_v1`).
6. **Döntés-ledger köteg-szintű visszavonással** — `automation_decisions`
   (`batch_id, rule, entity_type, entity_key`, `before/after_state`, `reverted_at`).
7. **Opening/closing odds-fázis és as-of őr** — `snapshot_kind`, „kickoff után nem
   írunk árat” szabály.
8. **Napi duplikátum- és lefedettség-őr** — `dupl_meccs_nap` (nulla tolerancia,
   dokumentált forrás-konfliktus kivétel), `coverage_guard_report`
   (`events/covered/reasons`), és független SQL a riport ellenőrzésére.
9. **Adatkövetelmények forrás-prioritással** — `data_requirements`
   `(entity, field) → source_priority[] + freshness_hours`.

## 3. A v2 mért hiányai (ezekre épülnek a szeletek)

**H1. Nincs meccs-szintű forrás-híd.** `identity.source_entity_ref` csak
`team`/`referee` típusú; meccs-referencia 0 sor minden forrásra. Emiatt nem tudjuk
megmondani, melyik forrás adta a meccset, és a duplikátum-kezelés/linkelés vak.

**H2. Nincs cross-source csapat-crosswalk → valódi duplikátumok a nap-listán.**
Ma 8 közeli duplikátum-pár van ugyanazon a versenyen és kickoffon (név-variáns
miatt külön csapat, külön meccs): SV Darmstadt 98 / Darmstadt; Llandudno FC /
Llandudno; Briton Ferry Llansawel / Briton Ferry; TNS / The New Saints;
FC Gagra / Gagra; FC Meshakhte Tkibuli / Meshakhte Tkibuli; Dinamo Tbilisi /
Dinamo Tbiliszi (más kickoff); AGF / Aarhus. A beépített `dedupe matches` csak
1 pontos-kulcsú csoportot talál (Andorra–Leganés a LaLiga 2 / LaLiga2 versenyek
alatt), tehát a variáns-duplikátum ma láthatatlan a riportokban.

**H3. Országkód-duplikátumok elrontják az identitás-kontextust.** A `core.country`
kétszer tartalmazza ugyanazt az országot: FR/FRA, DE/GER, HU/HUN, IT/ITA, ES/ESP,
GB/GB-SCT. A v1-import ISO3-as kódot hoz, az új források ISO2-t; a
`normalize_country_code` csak a `GB-*` alakot egységesíti, így pl. egy `GER`-es
v1-csapat és egy `DE`-s soccerway-csapat kontextusa nem egyezik → az egzakt név
ellenére „ismeretlen csapat” lesz, review-sor nyílik.

**H4. Soccerway: parse jó, a kanonikalizálás/linkelés veszít.** A mai napra a
parser 252 meccset ad (756 megfigyelés: 252 match + 504 team), a `stage.observation`
tárolja is mind (252 match_core, 494 team_identity), a mai v2 nap-listán viszont
csak 47 soccerway-pár szerepel. A 494 csapatmegfigyelésből 475-re van egzakt
(normalizált) névegyezés a `core.team`-ben — tehát a veszteség a csapat-feloldás
utáni szakaszban van (kontextus-egyeztetés, H3; illetve a meccs-létrehozás
kihagyása). Ez a szelet nyitott diagnózissal indul.

**H5. Tipplap-események nem linkelődnek meccshez.** A v2-ben a tipplap saját
meccset materializál, nem a meglévőhöz linkel; emiatt az odds csak ott jelenik meg,
ahol a tipplap az első forrás (29/131). v1-ben ugyanez a nap 109/163 volt.

**H6. Nincs liga-alias réteg a tipplap címekhez.** v1-ben 434 soros
`tipplap_league_alias` (confirmed 323 / auto 107 / rejected 4); a 427 címből 267
leképezett, az utolsó 14 nap eseményeinek 82,4 %-a. A v2-ben liga-szintű
név-egyeztetés csak a 49 elemű `competitions.rs` szótár és a `fold_name`
egyeztetés.

**H7. Forrás-lefedettség szűk.** A v2-nek 4 valós forrása van; a v1 napi győztesei
közül a fotmob, livescore, sofascore (v2-katalógusban tiltott), worldfootball,
statsbomb hiányzik. A fotmob külön érték: csapat-szintű híd (2 725 kötés) és
napi fixture-lista; a reep CC0 híd (895 163 fotmob-sor) ezt még erősítheti.

**H8. Nincs napi lefedettség-/duplikátum-őr a futásban.** A v2 `footballctl
integrity` a pontos-kulcs duplikátumokat nézi (ma 1 csoport), a variánsokat nem;
nincs `coverage_guard`-szerű riport (hány forrás-eseményből lett meccs, milyen
okokból maradt ki).

**H9. Női/ifjúsági kontextus.** A soccerway U17/nemzeti csapatok ma
„ismeretlen csapat”-ként esnek ki (Wales U17, Zambia, Venezuela, …); a v2
`core.team` 2 812 sora főleg klub. A v1 gender/branch tag-ekkel és nagy csapat-
univerzummal kezelte.

## 4. Szeletek (sorrendben, mindegyik önállóan zöld kapuval)

### S37a — Identitás-kontextus egységesítése (országkód) — kicsi, azonnal mérhető
- `football_domain::identity::normalize_country_code`: ISO3 → ISO2 alias-tábla
  (FRA→FR, GER→DE, HUN→HU, ITA→IT, ESP→ES, NED→NL, POR→PT, …), a `GB-*` szabály marad.
- Új domain-teszt az ekvivalenciára; a linker viselkedése változatlan, csak a
  kontextus-egyeztetés lesz helyes.
- **Elfogadás:** `cargo test -p football-domain`, `cargo test -p football-identity`
  zöld; a mai soccerway-futás „ismeretlen csapat” száma mérhetően csökken (a
  deploy utáni első futás logja alapján).
- **Nyitott:** a duplikált `core.country` sorok összevonása adatművelet (külön
  döntés, dev DB-n visszajátszható SQL-lel).

### S37b — Meccs-szintű forrás-híd + linkelés (H1, H5)
- A materializálás írjon `identity.source_entity_ref` sort `entity_kind='match'`
  típussal (a forrás meccs-azonosítója → kanonikus meccs), ugyanazzal a
  „soha nem írja felül” mintával, mint a csapatoknál.
- A tipplap-esemény bekötése a v1 szabályai szerint: kickoff ±1 nap,
  név-szabály, pontosan 1 jelölt; 0/több jelölt → review-sor; meccset nem hoz létre
  feleslegesen.
- **Elfogadás:** egy napi tipplap-futás után a nap odds-lefedettsége 29 → ≥100
  meccs (v1 szint), a duplikátum-próba nem nő; új integrációs teszt a linkelésre
  (wiremock fixture).

### S37c — Variáns-duplikátum felismerés és merge (H2)
- `footballctl dedupe variants --date …`: ugyanaz a verseny + kickoff, eltérő
  csapat-név, szabály-alapú hasonlóság (fold + club-suffix levágás + token-átfedés,
  rövidítés/prefix); riport + jelölt-lista, merge csak jóváhagyással (audit).
- Napi integritás-riport kapja meg a variáns-számot; cél: 0 nyitott variáns.
- **Elfogadás:** a mai 8 pár mind megjelenik jelöltként; a merge után 0; a
  `dedupe matches` és az `integrity` változatlanul zöld.

### S37d — Csapat-alias seedelés a v1-ből + tipplap liga-alias port (H6)
- `footballctl identity seed-v1` kiterjesztése: v1 `team_aliases` (4 558) és v1
  csapatnevek betöltése a meglévő v2 entitásokra (egzakt fold + ország/nem kapu),
  jelentéssel (hány alias, hány kihagyott, okok).
- A v1 `tipplap_league_alias` 434 sorából a v2 verseny-szótárba átemelhető címek
  listája + jóváhagyási lista; a `GENERAL_LEAGUES` 26 sora mellé.
- **Elfogadás:** mérhető javulás a napi „ismeretlen csapat” és „no_alias” okokban.

### S37e — Napi lefedettség- és duplikátum-őr (H8)
- Új `footballctl coverage guard --date`: forrásonként `events`, `materialized`,
  `covered`, `reasons {no_team, no_alias, unresolved, withheld}`, napi mentés
  (új tábla vagy `ops.coverage_report`), és napi dup-variáns szám.
- Integritás-riport hívja; a CI-nek legyen füsttesztje (szintetikus nap).
- **Elfogadás:** a napi futás után látszik, hol veszik el a meccs; a jelentés
  nem hazudhat (független SQL a riport ellen).

### S37f — Forrás-bővítés: fotmob (H7)
- Új adapter a v1 `scrappers/web/sources/fotmob*` mintájára (nap-lista +
  csapat-híd); a v2 katalógusban a fotmob engedélyezett-e, ellenőrizni kell.
- A reep CC0 hidat (fotmob 895 163 sor) csak a fotmob bevezetése után érdemes
  használni — soccerway-híd ugyanis nincs benne.
- **Elfogadás:** a csak-v1 ligák egy része megjelenik a v2 nap-listán.

### S37g — Női/ifjúsági kontextus (H9)
- Nemzeti és utánpótlás-csapatok létrehozása a forrás-vezérelt úton
  (source-ID + country/gender/age_group context), a fail-closed szabály megtartása.
- **Elfogadás:** a soccerway U17/nemzeti meccsek megjelennek, nem „ismeretlen
  csapat”-ként.

## 5. Kockázatok és nyitott kérdések

- Az országkód-egységesítés (S37a) csak a kód-összevetést javítja; a duplikált
  `core.country` sorok és a hozzájuk tartozó csapatok összevonása adatművelet,
  külön jóváhagyást és visszaállítási tervet igényel.
- Az S37b meccs-hídja visszamenőleg nem pótolja a korábbi napokat; a backfill
  külön szelet.
- A fotmob/reep bevezetés jogi/robot-kérdéseit a forrás-katalógus szerint kell
  ellenőrizni a kód megírása előtt.
- A v1 napi őrei (hourly-audit) maguk is töröttek 2026-11-18 óta (ESM `require`
  hiba); a v1-et nem javítjuk, csak a mechanizmust visszük át.