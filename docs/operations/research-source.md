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
- A napi láncba még be kell kötni az automatikus dedupe-pass-t (S37e).
