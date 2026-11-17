# Backfill leltár (S23a) — 2026-11-17

Read-only mérés a lehetséges történelmi forrásokról. A cél: eldönteni, miből
és milyen sorrendben töltsük fel a korábbi szezonokat (meccsek + eredmények +
odds), a lehető legnagyobb lefedettséggel.

## 1. footballdata archívum (`mmz4281/{szezon}/{liga}.csv`)

- **22 liga, 711 liga-szezon CSV**, 1993/94 – 2026/27 (a szezon-kód
  `YYZZ`, pl. `2526` = 2025/26).
- A séma a mostani adapterrel azonos (Div, Date, Time, HomeTeam, AwayTeam,
  FTHG/FTAG/HTHG/HTAG, játékvezető, stats, B365/PP/BFE/Max/Avg nyitó és
  záró odds, O/U, AH).
- Szezon-tartományok ligánként:
  - Anglia E0–E3: 1993/94-től (34 szezon), EC: 2005/06-tól (22)
  - Skócia SC0–SC1: 1994/95-től (33), SC2–SC3: 1997/98-tól (30)
  - Németország D1–D2, Olaszország I1: 1993/94-től; I2: 1997/98-tól
  - Spanyolország SP1: 1993/94-től; SP2: 1996/97-től
  - Franciaország F1: 1993/94-től; F2: 1996/97-től
  - Hollandia N1: 1993/94-től; Belgium B1: 1995/96-tól
  - Portugália P1: 1994/95-től; Törökország T1: 1994/95-től;
    Görögország G1: 1994/95-től
- Elérhetőség ellenőrzése: a `*m.php` ligaoldalak tartalmazzák a szezon ×
  liga mátrixot (a CSV-k közvetlen linkjei nincsenek kitéve).

## 2. footballdata „new" ligák (`new/{LIGA}.csv`)

- **15 liga, ligánként egy CSV az összes szezonnal** (2012-től):
  ARG, AUT, BRA, CHN, DNK, FIN, IRL, JPN, MEX, NOR, POL, ROU, RUS, SWE, USA
  (SUI: 404 — nincs fájl).
- **Más séma:** `Country,League,Season,Date,Time,Home,Away,HG,AG,Res` +
  záró és pre-closing odds (PSCH/PSCD/PSCA, MaxCH…, AvgCH…, BFECH…,
  B365CH…).
- Méret: pl. ARG 6 475 sor (több szezon egy fájlban).

## 3. v1 adatbázis (`football_app`, csak olvasás)

- **1 467 472 meccs**, 1 437 101 lezárt, 1 436 358 eredménnyel;
  1406 verseny / 7074 szezon; 2020 óta **508 920 meccs**.
- **234 916 meccs oddsszal**, 3,82M odds-sor (nyitó + záró).
- Legnagyobb lefedettség (2015 óta): Premier League 37 495 meccs
  (7 889 oddsszal), Serie D 30 553 (0), Regionalliga 19 530 (0),
  Primera División 18 834 (5 334), Serie C 15 433 (0), Cup 14 958 (0),
  Serie A 9 839 (9 465), Serie B 9 674 (4 346), Championship 7 198
  (6 690), League One/Two, La Liga 2, MLS, Liga Profesional Argentina… —
  a fő ligákban az odds-lefedettség jó, a mély alsóbb ligákban sok
  eredmény van odds nélkül.
- Crosswalk: `team_aliases` 4 537 sor, `external_id_map` forrásonként
  (Reep 1,41M meccs, Football-Data 250k, worldfootball 17k, Tipplap 770).
- Korlát: a v1 hitelességi incidensei miatt (fabrikált eredmények) csak
  auditált sorokat szabad átemelni, forrás-jelöléssel.

## 4. OpenFoot

- Napi feed dátumra (UTC), prototípus-kvóta 5000/hó — múltbeli napokra is
  használható a napi élő ingest mintájával, de a teljes történelmi
  feltöltésre nem méretezett.

## 5. Mérési költség

A leltárhoz használt kérések: `data.php`, `matches.php`, 11 ország-oldal,
18 `new/` HEAD + 1 GET ≈ 32 kérés (footballdata, robots engedett).
