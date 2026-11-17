# Következő szakaszok — ütemterv (2026-11-17)

Kiinduló állapot (2026-11-17 este, main `a6d1ad9`):

- Élő források: **footballdata** (5 liga, eredmény + closing odds),
  **Soccerway** (napi 1 render, minden verseny), **OpenFoot** (nap-feed),
  **Tipplap** (napi lista, Tippmix 1X2 nyitó odds, provisional).
- Identitás: fail-closed linker + v1 crosswalk-aliasok (1342 alias),
  entitás-merge eszköz, meccs-dedupe (tükrözött párokkal), lock-first
  orientáció, ref-javítás auditálva.
- Napi példa: 249 meccs, 0 duplikált csoport, mind `verified_publish`,
  39 meccsen Tippmix odds, 0 pending review.
- Üzem: launchd agent fut (App Support), napi jobok, budgetek:
  soccerway 6, openfoot 8, footballdata 25, tipplap 6.

A szakaszok sorrendje a függőségek szerint: **S21 → S22 → S23**, közben
S24/S25 folyamatos. Minden szakasz a megszokott szelet-fegyelemmel megy:
egy szelet = egy commitolható, tesztelt, ellenőrzött egység.

## S21 — Verseny-nevezés stabilizálása (crosswalk-alap)

**Probléma:** a Soccerway renderenként lokalizál (HU/EN), és az ismeretlen
hazai kupák címe a megfigyelt szöveg → ugyanaz a verseny két entitássá
válik (ma: `Kings Cup - Thaiföld` vs `Kings Cup (Thailand)`,
`Copa Colombia - Második szakasz` vs `- Second stage`, `AFC U17 Asian Cup
u17` variánsok). Minden ilyen váltás duplikált meccseket szül, amit naponta
dedupe-olni kell.

**Scope:**
1. Fix verseny-szótár a Soccerway adapterben a gyakori kupákra/ligákra
   (HU↔EN cím → kanonikus EN név), a naming-stability szabály szerint
   (csak fix tábla, nincs heurisztika; a b51781b nevezés a referencia).
2. Verseny-alias réteg: `competition` kanonikus feloldás a materializerben
   (megfigyelt cím → meglévő verseny-entitás), hogy egy forrás-neváltozás
   ne hozzon létre új entitást.
3. A meglévő churn-duplikátumok összevonása (dedupe a meglévő eszközzel,
   illetve a verseny-entitások összevonása, ha kell).

**Elfogadás:** három egymást követő napi futás után nincs új verseny-entitás
ismert versenyre, a dedupe 0 csoportot talál; a Kings Cup / Copa Colombia
példák egy entitás alatt élnek.

**Kockázat:** a szótár bővítése önmagában is churnt okoz, ha nem fix —
ezért minden új bejegyzés külön, tesztelt adatváltozás, és a régi nevek
alias-ként maradnak.

## S22 — Forrás-lefedettség bővítése

**Cél:** több liga, teljes fordulók, több odds — a backfill előfeltétele.

**Scope:**
1. **footballdata ligabővítés:** E1–E3, EC (Anglia), SC0–SC3 (Skócia),
   D2, I2, SP2, F2, N1, B1, P1, T1, G1 — ugyanazzal a parserrel; budget
   25 → 40; az új csapatok approve-create bootstrapja; az angol
   alsóbb ligák teljes fordulója megjelenik.
2. **Tipplap szótár bővítése:** U19/U21 Eb-selejtező, CONCACAF A, NB II,
   további kupák — a v1 `tipplap_league_alias` confirmed soraiból, csak
   stabil kanonikus nevekkel; ismeretlen továbbra is kimarad.
3. **OpenFoot név-crosswalk:** a registry-versenyek kanonikus névre
   képezése (jelenleg 7 ismert), hogy a verseny-entitások összeérjenek.

**Elfogadás:** a napi meccsszám érdemben nő, 0 duplikált csoport, a
review-terhelés naponta kezelhető (< ~50), és a top-ligák fordulói
teljesek (nem csak a napi minta).

**Kockázat:** a források névformátumai eltérnek — a crosswalk/alias réteg
(S21) előbb kell, különben a duplikátumok újratermelődnek.

## S23 — Történelmi backfill (a korábban rögzített követelmény)

**Cél:** minden liga/ország, a lehető legtöbb meccs + odds, részletesen.

**Források:** footballdata archívum (1993/94-től), a v1 adatbázis
(read-only: meccsek, eredmények, odds, crosswalk), OpenFoot múltbeli napok
(API-kvóta), StatsBomb open data (eseményszint, nem napi fixture).

**Scope (szeletek):**
1. **Leltár + gap-analízis:** forrás × szezon × liga lefedettségi riport,
   hiányzó szezonok/odds-mappák listája.
2. **Backfill job-típus:** batch ingest (resumable, budget-tervezett,
   napi/heti keret), a day-jobtól elkülönítve.
3. **Skálázott identitás:** crosswalk-aliasok tömeges seedje (v1), új
   csapatok approve-create folyamata, meccs-dedupe nagy mennyiségre.
4. **Minőség-riport:** coverage KPI-k (meccsszám, odds %, trust-szintek,
   review-átfutás), anomália-scan.
5. **As-of analitika:** historikus ELO/számított mutatók a backfillből.

**Elfogadás:** definiált KPI-ok teljesülése (pl. top-20 liga 10 évre,
odds-lefedettség célzott %-a), idempotens, újrafuttatható backfill; a napi
élő ingest érintetlen.

## S24 — Kliens szeletek (párhuzamosan)

1. **Odds megjelenítés vizuális ellenőrzése:** trust badge (provisional /
   verified), odds-chipek a nap- és meccs-nézetben (screenshot-ellenőrzés).
2. **Meccs-adatlap bővítés:** odds-piacok, ELO, később sérülések
   (Transfermarkt jelenleg blokkolt — csak ha új hozzáférés lesz).
3. **Kereső, női nézet, verseny-oldal** — a korábbi backlog tételei.

## S25 — Üzemeltetés és minőség (folyamatos)

1. **Napi integritás-riport:** duplikátum-scan, ref-egészség (aktív ref ↔
   kanonikus sor), orientation-audit, pending review szám — Ops nézetben.
2. **Review SLA:** napi szintű review-összegzés és batch-eszközök karbantartása.
3. **Monitoring/alert:** agent-hibák, breaker-ek, kvóta-kifutás jelzése.
4. **Nyitott user-döntések:** OpenFoot kulcs rotációja (a korábbi
   run.sh-ban szerepelt), ESPN használat (ToS/licenc), football-data.org /
   The Odds API regisztráció; saját ELO-számítás (clubelo élő elvetve).

## Ajánlott következő lépés

**S21 (verseny-nevezés stabilizálása)** — ez állítja meg a mai napon is
megfigyelt churn-duplikátumokat, és ez az S22/S23 előfeltétele. Utána
S22 ligabővítés, majd S23 backfill-terv.
