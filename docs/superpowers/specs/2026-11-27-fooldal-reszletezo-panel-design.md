# 2026-11-27 — Főoldal: 25/75 elrendezés és beágyazott meccs-részletező

**Státusz:** VÁZLAT (döntésre vár a 25/75 értelmezését illetően)
**Előzmény:** `feat/homepage-preview` (`fe1777b`) — 50/50 lista/üres panel,
kompakt kártyák, szűrők finomhangolása,FT + kezdési idő.
**Motiváció:** a meccs-részletező jelenleg külön sheetben nyílik; teljes
képernyőn a jobb oldali üres panel erre a tartalomra használható.

## 1. Cél és határ

- A bal oldali lista szélessége **25%**, a jobb oldali részletező panel **75%**
  (a felhasználó 2026-11-27-i javaslata; ha fordítva értette, a terv arányait
  kell cserélni).
- Kattintás a bal oldali meccsre → a részlet **a jobb panelben** jelenik meg,
  nem új ablakban/sheetben.
- A widget-ek forrása a meglévő, ellenőrzött adat és a megbeszélt sorrend;
  új adatforrás nélkül. Valódi fogadás, kifizetés nincs; a shadow predikció
  továbbra is „NEM ELLENŐRZÖTT" jelzéssel szerepel.
- Keskeny ablaknál (< 1200 pt) a lista teljes szélességű marad, és a régi
  sheet-es részletező él tovább (fallback).

## 2. Adatforrások (mind kész, read-only)

| Widget | Végpont |
|---|---|
| Áttekintés / eredmény / státusz | `GET /v1/match/{id}` |
| Események | ugyanonnan (`incidents`) |
| Felállás / játékosstatisztika / lövések | `/lineups`, `/player-stats`, `/shots` |
| Részletes piacok | `/markets` |
| Shadow predikció | `/prediction` |
| Jegyzetek / idővonal | `/notes`, `/history` |
| Forráslinkek / magyar riport | `/v1/match/{id}` (`links`, `insight`) |

## 3. Widget-sorrend (a felhasználóval egyeztetett adatokból)

1. **Scoreboard** — csapatok, eredmény, kezdés, verseny/ország, publikációs jel.
2. **Meccsmenet** — gólok/lapok/cserék időrendben, hiány esetén üres üzenet.
3. **Felállás** — kezdő 11 + cserék, kattintható játékos.
4. **Játékosstatisztika** — rendezhető(oldal szűrővel), hiány `–`.
5. **Lövéstérkép** — x/y, xG; momentum külön szeletben jöhet.
6. **Odds** — 1X2/BTTS/OU mátrix + részletes piacok (korlátozott darabszám).
7. **Modell** — ShadowPrediction kártya, fair odds, „nem ellenőrzött" jelzéssel.
8. **Research** — magyar riport + forráslinkek.
9. **Jegyzetek / idővonal** — írás csak ops tokennel.

A sorrend és a widgetek láthatósága később felhasználói beállítás lehet; most
fix, de konstans tömbben (`DetailWidgetOrder`).

## 4. Állapot és interakció

- `HomeView` tartja a kijelölt meccs `UUID`-ját (`selectedMatchID`) és a
  `MatchDetailModel`-t; a `.task(id:)` generációval véd a verseny ellen.
- A bal lista kártyája `.onTapGesture` helyett "select" akciót hív; a részletező
  sheet helyett a jobb panel `MatchDetailContent`-et rajzol.
- Billentyűzet: Tab a szűrők → lista → jobb panel; ↑/↓ a listában, Return =
  kijelölés, Escape = panel bezárása. AX-azonosítók megmaradnak
  (`match.detail.section.*`, `match.markets`, `match.prediction`).
- Nagy piacoknál (651 sor) a lista 10 csoporttal indul, „Többi piac" gomb nyitja
  tovább — a főoldal ne ragadjon be.

## 5. Szeletek

- **S210 — arány és kijelölés:** 25/75 `GeometryReader`, bal lista select
  akció, jobb panel `MatchDetailContent`, keskeny ablaknál sheet fallback.
- **S211 — widgetek és sorrend:** a 9 widget a fenti sorrendben, üres/hiba/
  betöltés állapotok, `reduceMotion`, AX-címkék.
- **S212 — billentyűzet és gyors teszt:** csak célzott XCUITest (1–2 eset),
  nem a teljes 15 perces harness.

## 6. Kapu (új szabály)

- A teljes `run-ui-tests.sh` **nem** része a mindennapi kapunak (15 perc, a
  gépet blokkolja, mindig ugyanazt kattintja). Helyette:
  - app unit + Core teszt (gyors),
  - 1–2 célzott UI-tes csak akkor, amikor az adott szelet tényleg érinti,
  - valódi macOS AX/képernyőkép a futó appon, ha vizuális változás van.
- A teljes harness továbbra is elérhető külön, kérésre, de nem futtatjuk rutinban.

## 7. Visszaállás

- Minden szelet külön commit a feature ágon; `git revert` visszaállítja az
  arányt és a panelt. A kanonikus adatot a terv nem írja.
