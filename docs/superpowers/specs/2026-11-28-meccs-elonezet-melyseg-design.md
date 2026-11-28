# 2026-11-28 — Meccsrészletező mélység: pre-match összefüggések

**Státusz:** DÖNTÉSRE VÁRÓ terv (2026-11-28)
**Előzmény:** `feat/homepage-preview` (`c0fd3f2`) — S220: a kijelölt meccs teljes
részletezése a főoldal jobb paneljében, `MatchDetailPane` néven közös forrás
a sheetnek és a panelnek. Korábbi tervek:
[`2026-11-27-fooldal-reszletezo-panel-design.md`](2026-11-27-fooldal-reszletezo-panel-design.md),
[`2026-11-26-adatmegjelenites-fotmob-design.md`](2026-11-26-adatmegjelenites-fotmob-design.md),
[`2026-11-23-match-insight-design.md`](2026-11-23-match-insight-design.md).

## 1. Cél és változatlan határ

A felhasználó 2026-11-28-i kérése: a **felső fejléc és az alatta lévő szűrőpanel
maradjon úgy, ahogy van**; a jobb oldali részletező panelben viszont jelenjen
meg minden olyan, részletesebb pre-match összefüggés, ami a rendelkezésre
álló adatból kinyerhető:

1. sérült/kérdéses játékosok listája, mindkét csapatra;
2. kiemelt játékosok csapatonként (legjobb játékos / góllövő, kezdhet-e);
3. teljes szezonstatisztika a két csapatról, egymás mellett;
4. szöveges bevezető;
5. kidolgozott H2H — külön az egymás elleni, külön az eddigi meccsekre,
   metrikákkal (gólok, BTTS, over/under, szöglet, lapok);
6. következő meccsek csapatlogókkal; tabella marad;
7. odds-rész átdolgozása, a saját predikció komolyan vétele, részletesebben,
   szépen beágyazva.

A terv **nem** változtat a fejléken, a szűrőkön, a bal oldali listán, a 25/75
arányon és a meglévő AX-azonosítókon. Nem tartalmaz valódi fogadást,
kifizetést, scrapinget vagy új külső forrást. A predikció és a generált
szövegek továbbra is **„NEM ELLENŐRZÖTT / számított"** jelzést kapnak.

## 2. Adatvalóság — mérten, nem feltételezve

A számok a dev adatbázis és a helyi research export read-only lekéréséből
származnak (2026-11-28).

| Kért tartalom | Valójában elérhető? | Bizonyíték / következtetés |
|---|---|---|
| Szöveges bevezető | **Igen, de ritka** | `fact.match_insight.preview_hu`: **88 / 27 035** sor; a 11-28-i research részletfájlok közül 862/957-ben van `preview_hu`. A Swift már dekódolja és a `MatchInsightView` ki is rajzolja a Research részben — fel kell emelni a panel elejére. |
| Sérülések | **Igen, de ritka** | `fact.match_insight.injuries`: **2 881 / 27 035** sor; 11-28-i mintában 123/957 fájlban nem üres. Mezők: `player_id, name, type, expected_return`. Jelenleg csak az első 6 jelenik meg. |
| Szezonstatisztika | **Részben** | `season_stats` 24 048/27 035 sor, de a kért mezők többsége null: `average_rating` **22**, `possession_pct` **22**, `assists` **0**, `clean_sheets` **149** sor; `rank_label` és `goals_per_game` viszont ~1 040/1 914 oldalra teljes. A Swift `InsightSeasonStat` jelenleg **nem dekódolja** ezeket. **Nincs Sofascore-adapter** és Sofascore-oszlop — a `source: fotmob_table` mezőt nem szabad „Sofascore rating"-ként címkézni. |
| Legjobb góllövő (szezon) | **Nincs ilyen adat** | A research `player_statistics` meccs-szintű (`FotMob rating`, `Goals`, xG…), `team_statistics` szintén meccs-szintű (szöglet, lap, passz…). Szezonális játékos-góllista sehol sincs. Maximum „rögzített eseményekből számolt gólszerző" készíthető, kifejezett bizonytalansági jelöléssel. |
| Kezdhet-e | **Csak közvetve** | `lineups[].role` (kezdő/csere) + `injuries[].expected_return` alapján, **előrejelzett**, nem megerősített státusz. Három állapot: kezdő / kispad / sérült, negyedik: „nincs adat". |
| H2H részletes metrika | **Részben számolható** | A kanonikus `head_to_head` minden meccshez ad `match_id` + gólokat; a `fact.match_stats` 21 473/55 041 meccsre tartalmaz szögletet, sárga/piros lapot, xG-t (a Leeds–Coventry H2H mintájában 2/2 meccsre van). BTTS és over/under a gólokból számolható; szöglet/lap a csatlakozó statisztikából — **csak ott, ahol van adat**, `coverage` mezővel. A research `h2h_streaks` csak két számlálót tárol (`no_win`, `no_clean_sheet`), összesített metrikát nem. |
| Következő meccsek logóval | **Logó elérhető** | A `NextMatchEntry` jelenleg csak szöveget + H/V + versenyt hordoz; a `home_logo/away_logo` ugyanazon az `ops.asset` úton elérhető, mint a fő `MatchDetailRow` címereké. DTO-bővítés kell. |
| Tabella | **Kész** | `table_slice` (pozíció, csapat, logó, meccs, győzelem, döntetlen, vereség, GF/GA, pont) + `standing` pillanatkép az insightban. Marad. |
| Odds / saját predikció | **Kész, de alulhasznált** | `/v1/match/{id}/markets` (market, outcome, line, price, bookmaker, phase, captured_at); `/v1/match/{id}/prediction` (model_version, probability, fair_odds, edge, quality, as_of); insight `prediction` (expected_goals, win_probability_pct, most_likely_score, inputs, note, generated/verified). A `PredictionCard` és `MarketOddsList` jelenleg külön, nem beágyazva jelenik meg. |

**Kifejezetten nincs / nem lehet:** Sofascore-értékelés (tiltott adapter, az
`average_rating` 22/27 035 találatból sem azonosítható Sofascore-ként),
megbízható szezonális góllövőlista, biztos kezdőösszetétel, átfogó
szezonális labdabirtoklás/assziszt a legtöbb meccsre. Ezeket **nem** szabad
kitölteni nullával, becsléssel vagy „Sofascore" címkével.

## 3. Információarchitektúra a jobb panelben

A sorrend a felhasználó által felsorolt csoportokból áll össze; az üres
szakaszok nem tűnnek el, hanem egyértelmű „Nincs adat a forrásban" üzenetet
mutatnak (a hiány maga is információ).

| # | Szakasz | Tartalom | Forrás | Jelölés |
|---|---|---|---|---|
| 0 | **Bevezető** | `preview_hu` címe, bekezdései, `key_points`; ha nincs, rövid ténysor (verseny, forduló, kezdés, státusz) | `insight.preview_hu` | „Generált — nem ellenőrzött", `generated_at` |
| 1 | **Összecsapás** |_scoreboard, időjárás, stadion, játékvezető, ELO_ | `/v1/match/{id}` | Megfigyelt / forrás |
| 2 | **Kulcsjátékosok és hiányzók** | két oszlop: sérültek (`type`, várható visszatérés), legjobb értékelésű játékosok az elérhető meccsadatokból, szerep (kezdő/kispad), rögzített gólok | `insight.injuries`, `/lineups`, `/player-stats`, `incidents` | „Forrás szerinti szerep — nem hivatalos kezdő" |
| 3 | **Szezon összehasonlítás** | két csapat egymás mellett: pozíció/rank, lejátszott meccs, G/Y/D, GF/GA, gól/meccs, kapott gól/meccs, GF-hez viszonyított mutatók; **csak akkor** clean sheets / assziszt / birtoklás / átlagértékelés, ha a mező nem null | `insight.season_stats` + `table_slice` | Forrásnév (`source`) + `generated_at` |
| 4a | **Egymás ellen** | H2H meccsenként (dátum, verseny, eredmény, logók), külön számolt összesítő: gólok összesen, BTTS%, over/under 2.5, szögletek, lapok — `coverage` számlálóval | `head_to_head` + `fact.match_stats` | „Számított — csak rögzített meccsekből" |
| 4b | **Eddigi meccsek** | `home_form` / `away_form` külön, eredmény + ellenfél + verseny + logó; nem keveredik a H2H-val | `/v1/match/{id}` | Megfigyelt |
| 5 | **Következő meccsek** | 3–3 meccs, mindkét csapat logójával, H/V, dátum, verseny; kattintás → láncolt részlet | `home_next/away_next` + logó | Megfigyelt |
| 6 | **Tabella** | meglévő `StandingsSlice`, kattintható csapatsorok | `table_slice` | Megfigyelt |
| 7 | **Odds** | kurált 1X2/BTTS/OU mátrix + szűrhető részletes piacok (maximum 10 csoport, „Többi piac" gomb), bookmaker és `captured_at` | `/markets` + `odds{}` | `odds_trust_state` |
| 8 | **Saját predikció** | kiemelt kártya: 1X2 valószínűségek, fair odds, edge, modellverzió, `as_of`, `quality`; research `expected_goals`, `most_likely_score`, `inputs`, magyarázó `note` | `/prediction` + `insight.prediction` | **„NEM ELLENŐRZÖTT modell"** — minden más widgetnél erősebb jelzés |
| 9 | **Források, jegyzetek** | `links`, `match_report_hu` (számított), jegyzetek, idővonal | `/v1/match/{id}`, `/notes`, `/history` | Forrás / számított |

Alapelvek: kétoszlopos, csapatközpontú összehasonlítás; minden szám mellé
„mennyi meccsből" és „mikor rögzítették"; a hiány és a nulla nem azonos;
`reduceMotion`; világos/sötét mód; minden szakasz saját AX-azonosítója
(`match.detail.block.<name>`), magyar címkék.

## 4. Szeletek (részletes terv a külön plan fájlban)

- **S221 — adatszerződés:** Swift DTO-k bővítése (season stat mezők, next-logók,
  H2H opcionális szöglet/lap mezők), Rust oldalon a hiányzó mezők feltöltése
  meglévő táblákból; kontrakt-tesztek, **UI még nem változik**.
- **S222 — bevezető + hiányzók + kulcsjátékosok:** a 0. és 2. szakasz;
  minden mezőhöz generated/forrás jelölés, üres állapotok.
- **S223 — szezon összehasonlítás:** a 3. szakasz, kétoszlopos, csak nem-null
  mezők, source/rank megjelenítés.
- **S224 — H2H és forma szétválasztása:** 4a/4b, számolt metrikák `coverage`-szel.
- **S225 — következő meccsek logóval + tabella finomhangolás.**
- **S226 — odds és predikció átdolgozása:** 7/8, beágyazott kártyák, erős
  nem-ellenőrzött jelzés.
- **S227 — integráció és magyar AX/képi ellenőrzés.**

## 5. Kapu és visszaállás

- Minden szelet külön branch, külön commit, `feat/homepage-preview`-be merge.
- Kapu: app unit + Core teszt, `cargo fmt/clippy` (Rust érintése esetén),
  valódi macOS AX + képernyőkép a futó appon. A teljes 15 perces XCUITest
  **nem** része a rutinnak (felhasználói döntés).
- A terv nem ír kanonikus adatot; minden új mező olvasási út. A DTO-bővítések
  visszafelelően kompatibilisek: régi mezők nem tűnnek el, új mezők opcionálisak.
- Visszaállás: `git revert` a szelet commitjára; a panel üres állapota továbbra
  is működik.
