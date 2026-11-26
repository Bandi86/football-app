# Forrásolt provider-tények (football-docs MCP)

Lekérdezve: 2026-11-26, `football-docs` MCP v0.17.10 (`nutmeg-football-docs`).
Minden tételnél a forrás URL és — ahol a szerver archívumot is megadott — a
Wayback-pillanatfelvétel. Ezek **dokumentált tények**, nem a lokális adat mérései.

## 1. Miért maradhat `scheduled` a kezdés után

Az API-Football v3.9.3 státusztáblája (ellenőrizve 2026-10-15) szerint:

> Competitions without live coverage can retain `NS` until the result is entered
> after the match. Do not treat `NS` alone as proof that the real-world match has
> not started. `TBD`, `PST` and `CANC` fixtures can carry provisional dates. A
> date change is not evidence of a new fixture ID.

Forrás: <https://www.api-football.com/documentation-v3>,
archívum: <https://web.archive.org/web/20261012113830/https://www.api-football.com/documentation-v3>.

**Következtetés a v2-re:** az S179 után fennmaradó `open_past` jelek egy része
nem pipeline-hiba, hanem a forrás viselkedése — a státusz akkor változik, amikor
az eredményt beírják. Ezeknél eredmény kitalálása tiltott; a `daily-check`
figyelmeztetés marad, amíg a forrás nem frissül.

## 2. Provider státuskódok → kanonikus státuszok

A kanonikus halmaz a DB CHECK-korlátban:
`scheduled | confirmed | live | halftime | finished | postponed | canceled | abandoned | awarded`.

TheSportsDB soccer státuszkódjai
(forrás: <https://www.thesportsdb.com/docs_api_data>,
archívum: <https://web.archive.org/web/20260701085447/https://www.thesportsdb.com/docs_api_data>):

| Provider kód | Jelentés | Kanonikus |
|---|---|---|
| `TBD`, `NS` | időpont még nem végleges / nem kezdődött | `scheduled` |
| `1H`, `2H`, `ET`, `BT`, `P` | játékban / szünetben / hosszabbítás / tizenegyes | `live` |
| `HT` | félidő | `halftime` |
| `FT`, `AET`, `PEN` | lezárt | `finished` |
| `PST` | elhalasztva | `postponed` |
| `CANC` | törölve | `canceled` |
| `ABD` | félbeszakadva | `abandoned` |
| `AWD`, `WO` | technikai vereség / walkover | `awarded` |
| `SUSP`, `INT` | felfüggesztve / megszakítva | **nincs kanonikus megfelelő** |

API-Football ugyanezeket a kódokat használja, plusz `LIVE` (általános „játékban").

**Figyelem:** a `rust/crates/ingest/src/research.rs` `match`-je ismeretlen kódnál
`_ => "scheduled"` ágra esik. A 2026-11-i research JSON-ben jelenleg csak
`finished`, `scheduled`, `postponed`, `cancelled`, `live`, `abandoned` fordul elő,
tehát ez most nem okoz hibát — de ha `SUSP`/`INT` megjelenik, a sor hamisan
nyitottként kerülne be. Az `openfoot.rs` ehelyett figyelmeztet és kihagyja a sort;
ezt a mintát érdemes követni.

## 3. xG — nem hasonlítható össze providerenként

A football-docs `xg` kártyája (v1, 2026-10-04) szerint az xG egy provider modelljének
kimenete; az összegek három okból is eltérnek: a tizenegyesek fix értéke
(Opta 0.79, Wyscout 0.76, Hudl StatsBomb 0.78), a birtokonkénti
`1 - ∏(1 - xG)` sapka (FBref, StatsBomb cumulative), illetve a modell maga.
Példa: a 2022-es döntőben Argentína xG-je 2.7583 a lövésértékek összegeként és
2.7027 cumulatív xG-ként.

A FotMob adatforrásairól a football-docs (2026-09-24-én ellenőrizve) azt írja,
hogy a **scores, események és statisztikák forrása nem nyilvánosan dokumentált**
(csak a fizikai metrikákhoz vásárolnak licencelt adatot).
Forrás: <https://www.fotmob.com/en/topnews/22634-statement-physical-metrics-feature-outage>,
archívum: <https://web.archive.org/web/20261125124408/https://www.fotmob.com/en/topnews/22634-statement-physical-metrics-feature-outage>.

**Következtetés a v2-re:** a `fact.match_stats.home_xg/away_xg` mezők research/FotMob
eredetűek. Ezeket **nem szabad** más provider xG-értékeivel összehasonlítani vagy
„ki játszott jobban" állítást alapozni rájuk; a kliensben xG-only rendezésnél
ezt a korlátozást érdemes jelezni.

## 4. Keresztprovider entitás-feloldás (Reep) — jelenleg nem elérhető

A `resolve_entity` eszköz (játékos, csapat, meccs, verseny, **venue** ID-k
keresztprovider térképe) kétféle beállítást igényel:

- `REEP_DUCKDB_PATH` — a teljes Reep register letöltése (több száz MB DuckDB),
  helyi lekérdezés, nincs kulcs; vagy
- `REEP_API_KEY` — kézzel igényelhető a `getintouch+nutmeg@withqwerty.com`-on,
  nincs önkiszolgáló regisztráció.

A v2-ben jelenleg egyik sincs. Ezért az S176 208 `pending review` sorát
(fuzzy/multiple/no_exact_match) ez az eszköz **nem** oldja meg automatikusan.
Ha később mégis hasznosítanánk: a helyi DuckDB a nagyobb, de kulcsmentes út.
