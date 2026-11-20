# Football Research forrás (S38)

A v2 napi adatának új elsődleges forrása a helyi **football-research** rendszer
fájl-alapú JSON API-ja. A v2 nem HTTP-t hív: a `local_file` transzport a
megadott könyvtárból olvassa ugyanazokat a fájlokat, amelyeket a research a
`api/v1/` alá exportál.

## Hely és konfiguráció

- Adatkönyvtár: `~/Library/Application Support/FootballResearch` (a Desktopon
  `~/Desktop/football-research` szimlink mutat rá; a TCC ezért nem blokkolja a
  launchd-ből indított shellt).
- API gyökér: `…/FootballResearch/api/v1`; felülírható a
  `FOOTBALL_RESEARCH_API_DIR` környezeti változóval (a daemon `agent/env`-jében
  vagy a `footballctl` futtatásakor).
- Forrás-regisztráció: `scripts/seed-research-source.sh [DB_URL]` — idempotens,
  `research` forrást `create_on_miss` identitásmóddal, `local_file`
  transzporttal, napi 1 discovery + 4 órás refresh-sávokkal veszi fel. A
  `local_file` forrásnál a worker nem futtat robots- és kvóta-kaput (nincs
  hálózati forgalom).

## Mit olvas

- `daily/YYYY/MM/<dátum>.json` — napi envelope: meccsek (uid, kickoff, status,
  verseny, csapatok, eredmény, játékvezető) + `coverage.odds` jelző.
- `matches/YYYY/MM/DD/<slug>.json` — a `coverage.odds`-szal jelölt meccsek
  detail-fájlja; az odds a `odds.markets` alatt (`1X2`, `BTTS`, `DC`, `OU2.5`,
  `OU3.5`, `OU4.5`), `phase` = `pre_match`/`closing`, `observed_at` a forrás
  megfigyelési ideje.

## Kanonikus leképezés

- `team_identity`: a research azonosító (`<prefix>:<id>`, pl. `fm:6580833`,
  `livescore:1796153`), név, ország, nem/korosztály (a versenynév alapján).
- `match_core`: verseny + szezon (július–június), csapat-hivatkozások,
  kickoff UTC-ben, status és FT/HT eredmény; az ismeretlen országkódok
  ISO3→ISO2 ekvivalencián mennek át, a brit nemzetek a v2 GB-je alá.
- `match_odds`: quote-onként (bookmaker, market, outcome, line, phase, price).
- `match_referee`: ha a napi envelope ad játékvezetőt.
- Identitás: a `research` forrás **create-on-miss** módban új kanonikus
  csapatot hozhat létre (név + ország + forrás-azonosító); fuzzy jelöltnél
  marad a review. A többi forrás továbbra is szigorúan fail-closed.

## Mért eredmény (2026-11-20, fejlesztői DB)

- Research napi fájl: **285 meccs**, ebből **177** oddsos.
- A v2 napi meccsszáma 122 → **311**, az oddsos meccsek 29 → **163**;
  a publikált (verified) napi feed 46 → **95** meccs, ebből 64 oddsos.
- 1 696 research odds-quote, 154 meccsen; 348 új kanonikus csapat.
- 22 pontos (azonos kickoff + azonos csapatpár) duplikátum keletkezett a
  research verseny-név-variánsai miatt (pl. „Ligue 1 McDonald's”, „LaLiga EA
  Sports”, walesi „Premier League”); ezeket az S37c dedupe motor merge-elte,
  így a nap végül duplikátum-mentes (`dedupe matches`/`dedupe variants`: 0),
  `integrity` → OK.

## Ismert nyitott pontok

- A research oldali 35 fuzzy duplikátum-pár kezelése a forrásnál történik; a
  v2-ben a napi záró dedupe-pass hidalja át.
- Verseny-név-feloldás: a „Ligue 1 McDonald's” / „LaLiga EA Sports” típusú
  szponzor-szuffixek és a walesi „Premier League” jelenleg új versenyt
  nyitnak, amit a dedupe merge zár le; a következő szelet ezt megelőző
  verseny-alias réteget ad (S38b).
- A napi láncba bekötött automatikus dedupe-pass (S37e) a research job után
  lefut: a pontos duplikátumokat (azonos kickoff + csapatpár) a legrégebbi sor
  felé merge-eli; a 2026-11-20-i éles futásban 22 duplikátumot zárt le.

## Versenynevek kanonizálása (S38b)

A research `competition` blokkban a rövid `name` a szponzor-szuffix nélküli
név (`Ligue 1`), a `display_name` a szponzoros változat (`Ligue 1 McDonald's`).
Az adapter a `name`-et használja elsődlegesen, így a szponzoros nevek nem
hoznak létre külön versenyt.

A brit hazai nemzetek ligái a v2-ben GB alatt, más néven élnek, ezért a
`research.rs` `COMPETITION_ALIASES` táblája kezeli őket:

| research ország | research név | v2 kanonikus név |
| --- | --- | --- |
| ENG | League 1 / League 2 / League One / League Two | League One / League Two |
| WAL | Premier League / Premier Division | Cymru Premier |
| SCO | Premiership / Championship / League 1 / League 2 | Scottish Premiership / Scottish Championship / Scottish League One / Scottish League Two |
| SCO | SWPL 1 | SWPL 1 - női |
| NIR | Premiership / Championship | NIFL Premiership / NIFL Championship |
| ESP | LaLiga / LaLiga2 / Copa del Rey - 1st Round | La Liga / LaLiga 2 / Copa del Rey |

Az ismeretlen versenynevek változatlanul mennek tovább (nincs találgatás), az
ország nélküli bejegyzések nemzetközi (`INT`) besorolást kapnak. A női
versenyek felismerése kiegészült a jelöletlen ligákkal (SWPL, WE League, Liga F,
NWSL, WSL, Toppserien, NCAAW, Damallsvenskan stb.).

Betöltés előtti ellenőrzés: `python3 scripts/precheck-research-days.py
2026-11-18 2026-11-19 2026-11-21` (meccsszám, eredmények, oddsok, napon belüli
duplikátumok, kanonizált versenyek, a v2-ben hiányzó versenyek).

## Backfill és automatikus tisztítás

A korábbi napok betöltése kézi `ops.job` sorral történik (a daemon áll):

```sql
INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type, time_window, due_at, priority, status)
VALUES (gen_random_uuid(),'research','day','backfill-2026-11-18','discovery','2026-11-18', now(), 0, 'pending');
```

majd `footballctl worker tick` (a `OBSCURA_BIN` env szükséges lehet). A
materializálás után a napi automatikus dedupe pass három szinten tisztít:
pontos duplikátum (azonos kickoff+csapatpár), név-variáns (azonos nap,
legfeljebb 2 óra eltérés, szigorú névegyezés) és fixture-szintű pár (azonos
verseny+szezon+csapatpár+UTC-nap, eltérő kickoff). A research saját fuzzy
duplikátumai (egy forrás órás időzóna-eltéréssel) így a betöltés után
automatikusan lezáródnak.
