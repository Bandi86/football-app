# S39 — Az app vizuális identitása („éjféli stadion")

Dátum: 2026-11-20
Státusz: jóváhagyott irány (a felhasználó kérése: „app design teljesen")

## Cél

A kliens eddig funkcionális, de semleges SwiftUI-megjelenését cseréljük le egy
egyedi, eredménytábla-ihlette vizuális rendre, amely a futball adatainak
sűrűségét (időpont, állapot, eredmény, odds, ellenőrzési állapot) olvashatóan
rendezi el. A design nem igényel új backend-mezőt: a mai API-ból elérhető
adatokra épül, a hiányzó szakaszok pedig egyszerűen elmaradnak.

## Vizuális nyelv

- **Alap:** OLED-fekete háttér (`#000000`), a felületek hűvös grafitok
  (`#0E0E10`, `#16161A`, `#08080A`), hajszálvonalak 8–14 % fehér.
- **Hangsúly:** borostyán (`#FFB020`) a kiemelésekre, zöld (`#30D158`) az élő
  állapotra, piros (`#FF453A`) a blokkolókra.
- **Tipográfia:** lekerekített, tabuláris számok a „eredménytábla" rétegen
  (`Font.scoreboard`), semleges szöveg (`Font.appText`), ritkított kapitális
  címkék (`Font.appLabel` + `.sectionLabel()`).
- **Rács:** 4 pt-os egység (`AppTheme.Spacing`), kártya/panel/chip sugarak
  18/14/9 pt (`AppTheme.Radius`).
- **Kártya:** grafit kitöltés, finom hajszálvonal, kiemelt állapotban borostyán
  szegély (`.appCard(elevated:accent:)`).

## Komponensek (`apps/FootballApp/Scoreboard.swift`)

| Komponens | Felelősség |
| --- | --- |
| `DayMasthead` | nap fejléce: dátum, meccsszám, élő/oddos számláló, frissítési állapot |
| `ScoreboardHero` | meccs-fejléc: nagy eredmény, csapatmonogram/címer, ELO, gate-jelzés |
| `OddsMatrix` | piaconkénti kimenetelek (1X2, BTTS, 2,5 gól), forrás + időbélyeg |
| `StatsDuel` | kétoldali statisztika arányos borostyán sávval, hiányzó érték „–" |
| `VerificationStrip` | publish-gate állapot, szabálykészlet, blokkoló üzenetek |
| `CrestLogo` | címer, kép nélkül a csapatszín-monogram (nem üres keret) |

A lista kártyái a meglévő `MatchListView`-ben kaptak új tipográfiát és
monogramot; a meccs részletező (`MatchDetailSheet`) teljesen új felépítésű:
hero → meta-rács → periódus-chipek → statisztika-párbaj → odds-mátrix →
ellenőrzési sáv, 720×620-as minimális panelen, 860 pt tartalomszélességgel.

## Adatlefedettség és hiánykezelés

- **Nap-lista:** csapat, időpont, állapot, eredmény (FT/HT/ET/pens), odds
  (ha van), pozíció, ELO, publish-állapot, címer.
- **Meccs-részletező:** odds (forrás, rögzítés), statisztika (lövések, szöglet,
  lapok, szabálytalanság, xG), játékvezető, periódusok, gate-blokkolók.
- Ami hiányzik (pl. statisztika egy nem lejátszott meccsnél), az a szakasz
  elmarad; nulla vagy hamis érték nem jelenik meg.

## Kapcsolódó változások

- `scripts/ui-fixture-server.py`: `--detail-fixture` kapcsolóval a
  `/v1/match/...` végpont is kiszolgálható, így a részletező UI-teszt valódi
  tartalommal fut (`tests/fixtures/ui/match-detail.json`).
- Új UI-teszt: `testMatchDetailShowsRichSections` (odds-, statisztika- és
  ellenőrzési szakasz jelenléte, Escape zárás).

## Korlátok

- A „gazdag" research-szekciók (események, felállások, játékos-statisztika,
  hírek) még nincsenek a v2 API-ban; ezek az R3 szeletben kerülnek be, és a
  design ezeket a meglévő kártya-mintára tudja majd felvenni.
- A logók a forrásokból érkeznek; ahol nincs kép, a monogram helyettesít.

## Bizonyíték

- `./scripts/run-ui-tests.sh` → **TEST SUCCEEDED** (app unit + 8/8 UI teszt,
  köztük `testMatchDetailShowsRichSections` 9,387 s).
- Élő képernyőképek: nap-nézet (`/tmp/design-day3.png`), részletező
  (`/tmp/design-detail2.png`), odds + ellenőrzés
  (`/tmp/design-detail-odds.png`).
