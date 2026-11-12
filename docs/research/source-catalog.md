# Forráskatalógus (S1) — élő dokumentum

Frissítés: minden spike után; státusz csak bizonyítékkal változik.
Spec: docs/superpowers/specs/2026-11-11-real-data-sources-research-design.md

## Státusz-legenda

- `javasolt` — leltárban szerepel, spike még nem futott.
- `igazolt` — spike zöld: robots, budget, mezőtérkép, lefedettség, tartalmi őr, bizalom, szállítás.
- `elvetett` — indoklással (robots/ToS/blokk/decoy/licenc).
- `regisztráció szükséges` — auth-fal; spike megállt, felhasználói döntés kell.
- `halasztott` — próba-szintű adat megvan, teljes igazolás a saját adapter-szeletben.

## Bejegyzés-séma

slug / név / base URL / típus; hozzáférés + auth (Keychain); szállítás
(rust-http | obscura | chrome-fallback) + bizonyíték; domainek + mezőtérkép
(v2 tábla.oszlop, típus, null, időzóna) + séma-rések; demo-kör lefedettség;
identitás (ID-stabilitás, crosswalk, alias); kvóta/robots (budget, delay, breaker);
frissesség-javaslat; bizalom domainenként; v1-érettség + kötelező őrök;
kockázatok + mitigáció; státusz + bizonyíték (spike-log dátum, sha256, v1-doc);
adoptációs rang.

## Tiltólista (elvetett, v1-bizonyítékkal)

| forrás | indok | bizonyíték |
|---|---|---|
| eredmenyek.com | fabrikált eredmény hamis provenance-szal | v1 docs/fix-log-2026-11-05-fabrikalt-golszamok.md |
| SofaScore | IP-ban, robots.txt is 403 | v1 docs/obscura-source-matrix-2026-11-09.md |
| WhoScored | Cloudflare-tiltás mindkét szállítással | v1 obscura-mátrix |
| UEFA.com | hálózati/IP blokk | v1 obscura-mátrix |
| Understat | decoy-200 (üres oldal 200-cal) | v1 obscura-mátrix 2. pont |
| FlashScore | robots tiltja a hasznos útvonalakat | v1 obscura-mátrix |
| kicker.de | robots tilt | v1 obscura-mátrix |
| BBC Sport | ToS tiltja a scrape-et | v1 obscura-mátrix |
| FBref | ToS + Cloudflare | v1 docs/web-scraping-architecture.md |

## Források

(minden spike-task ide fűzi a bejegyzését)

### footballdata — football-data.co.uk

- **slug / név / base URL / típus:** `footballdata` / Football-Data.co.uk /
  `https://www.football-data.co.uk` / archívum (statikus CSV, 1993/94-től, ~30 liga).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, 7 kérés mind 200,
  bot-kihívás nélkül; v1 előzménymérés: `docs/web-scraping-architecture.md`
  (2026-09-24: statikus CDN, egy fájl = egy kérés).
- **Domainek + mezőtérkép:** fixture/kickoff, eredmény FT/HT, meccs-stats, odds,
  tabella (számított), referee (csak E0).
  - `Date` (nn/hh/éééé) + `Time` → `core.match.kickoff_at` (timestamptz, UTC;
    időzóna-konverzió kell, a forrás-időzónát az adapter-szeletben ellenőrizni);
    `HomeTeam`/`AwayTeam` → `core.match.home/away` (text, not null, alias kell).
  - `FTHG`/`FTAG` → `core.match_result.home_ft/away_ft` (int, null amíg nincs
    eredmény); `HTHG`/`HTAG` → `home_ht/away_ht`; `FTR`/`HTR` FT-konvenció (90 perc).
  - `HS`/`AS`/`HST`/`AST`/`HC`/`AC`/`HY`/`AY`/`HR`/`AR`/`HF`/`AF` (+`HxG`/`AxG`)
    → **séma-rés: `match_stats`**.
  - `Referee` → **séma-rés: `referees`** (a 2627-es szezonban csak E0 tartalmazza).
  - `B365*`/`PP*`/`BFE*`/`Max*`/`Avg*` 1X2 + `C` closing változatok, O/U 2.5, AH
    → **séma-rés: `fact.odds_snapshot` bővítés (bookmaker, opening/closing)**.
  - Tabella: a szezon eredményeiből számítva (v1 minta).
- **Lefedettség (demo kör):** mind az 5 liga megvan, 2627-es szezon, 2026-11-08-ig:
  E0 (PL) 100 sor, SP1 (La Liga) 119, I1 (Serie A) 110, D1 (Bundesliga) 81,
  F1 (Ligue 1) 90. **NB I / NB II hiányzik** → rés-jelölés: Tipplap/FotMob pótolja.
- **Identitás:** nincs stabil külső ID; csapatnevek angol rövid nevek → crosswalk +
  `team_aliases` kell. Az archív `Country`/`League` oszlopok NEM megbízhatóak;
  aliased archív kódok tiltva (v1 `sources/footballdata.py:79`,
  `ARCHIVE_ALIASES_BLOCKED`). A `fixtures.csv` `Div` oszlopa üres (v1 mérés
  2026-09-26) → liga feloldása csapat-szezon membershipből, nem találgatás.
- **Kvóta/robots:** robots engedett (spike 2026-11-11: `(True, 'robots engedett')`,
  `/mmz4281/` útvonal); kvóta 200/nap; delay 4+3 s; breaker: 403/429 → azonnali
  stop, nincs retry (spike-budget 10/nap, ebből 7 fogyott).
- **Frissesség-javaslat:** szezon-CSV batch hetente (a lefutott fordulókig frissül);
  `fixtures.csv` spine naponta (jövőbeli fordulók, nyitó/záró odds, eredmény nélkül);
  odds closing-ablak: meccs utáni batch.
- **Bizalom domainenként:** eredmény/tabella magas (egy forrásból is verified);
  odds kétforrásos (Tipplap + football-data closing), egy forrás = provisional.
- **v1-érettség + kötelező őrök:** v1 adapter éles (`scrappers/web/sources/footballdata.py`,
  mérve 2026-09-24). Kötelező: üres CSV decoy-guard (`guard_rows`, offline teszt OK),
  dátum-guard, aliased archív kódok tiltva, BOM-kezelés (`utf-8-sig`, a `Div` oszlop
  BOM-mal érkezik), 90-perces FT-konvenció.
- **Kockázatok + mitigáció:** bookmaker-készlet változik — a 2627-es szezonban
  `PS*` (Pinnacle) oszlopok HIÁNYOZNAK, helyette `PP*` (Paddy Power) + `PPC*` closing
  van (notes.txt szerint egyes bookmakerek már nem szerepelnek) → szezonfüggő
  odds-mezőtérkép, keresztforrás Tipplap; `Referee` csak E0 → `referees` nullázható;
  NB I/NB II rés → Tipplap/FotMob; szerkezetváltás → spike újraellenőrzés.
- **Státusz + bizonyíték:** `igazolt` — spike 2026-11-11, python3.13,
  `/tmp/s1/probe_footballdata.py`: robots + 5 liga-CSV + notes.txt mind 200,
  nem üres (összesen 500 sor), mezőtérkép assert zöld, decoy-guard offline OK.
  Élő adaptációk (controller-ruling, őrök érintetlenek): need-lista `PSH/PSD/PSA`
  helyett `PPH/PPD/PPA` + closing `B365C*`/`AvgC*`; `Referee` nem hard assert,
  hanem ligánkénti lefedettségi jelzés; `utf-8-sig` dekódolás; minta-cache
  (E0 nem lett újratöltve); notes.txt vétele a bookmaker-kódok igazolására;
  ligánkénti DecoyError/AssertionError-elkapás: a guard az érintett ligán bukik,
  de a spike a többi ligával folytatódik (exit kód 0 marad), üres liga esetén
  az adott liga státusza `javasolt` marad „szezonális üresség” jegyzettel,
  ligánként rögzítve.
  Minta-sha256 (`/tmp/s1/samples/footballdata/`):
  - `2627_E0.csv`: `d598a5f681e1a9a4acf9dad9fb203b68da6f67e2d195678dbe99e95fbba14593`
  - `2627_SP1.csv`: `b4c9a5cc1139f30dbd1026679c360de4626ce97fe64ae62ff1ff8ba3af0b7828`
  - `2627_I1.csv`: `91f517fedb41215a32e80faa8f4ec44e26d970e557c6c120a664686dbc4d22f0`
  - `2627_D1.csv`: `85b5ff5199866f452016473e927ff2431c05c7b2ae045b5edd3b7f5bb5847d2a`
  - `2627_F1.csv`: `351303e69ffd3e18a25ac6f06463929faa3db6b86abbf2564709f3c5a9f92f31`
  - `notes.txt`: `02b5e76154adabd534a302f7a09d4e61d1bae62d24c1bd2bb6ec4c9c2185425b`
- **Adoptációs rang:** 1.

### thesportsdb — TheSportsDB

- **slug / név / base URL / típus:** `thesportsdb` / TheSportsDB /
  `https://www.thesportsdb.com/api/v1/json/` / JSON API (v1; a v2 API prémium).
- **Hozzáférés:** publikus kulcs az URL-ben — a brief szerinti „3” kulcs ma
  **403-at ad** (2026-11-11, élő próba). A hivatalos docs
  (`/docs_api_guide`, 2026-11-11-i olvasat) szerint az ingyenes kulcs már **„123”**,
  endpointonkénti erős limittel; a teljes hozzáférés prémium supporter-kulcs
  (regisztráció + előfizetés, a kulcs a profiloldalon jelenik meg; v2 API
  `X-API-KEY` header). Titok esetén Keychain kell — kulcsot a spike NEM hozott létre.
- **Szállítás:** `rust-http` (tervezett) — élő bizonyíték nincs: az első API-hívás
  403 → harness-breaker azonnali stop, nincs retry. A robots-kapu zöld.
- **Domainek + mezőtérkép (tervezett, spike nem igazolta):** fixture/kickoff,
  eredmény FT, stadion, referee; szezon-gerinc (season spine) szerep a
  `eventsseason.php` végpontból.
  - `idEvent` → külső ID (identity crosswalk); `strEvent` → meccs-név;
    `dateEvent` + `strTime` → `core.match.kickoff_at` (timestamptz, UTC;
    a forrás-időzónát az adapter-szeletben ellenőrizni kell).
  - `strHomeTeam`/`strAwayTeam` → `core.match.home/away` (text, not null, alias kell).
  - `intHomeScore`/`intAwayScore` → `core.match_result.home_ft/away_ft` (int,
    null amíg nincs eredmény); `strStatus` → státusz-mező (pontossága
    spike-jegyzet tárgya, nem igazolt).
  - `strVenue` → **séma-rés: `core.match.stadium`**; `strReferee` →
    **séma-rés: `core.match.referee`** (a v2 sémában jelenleg nincs ilyen oszlop).
  - `intRound` → `round_label`.
- **Lefedettség (demo kör):** NEM ellenőrzött — auth-fal miatt a top-5 liga
  (4328 = PL és társai) és az NB I/NB II league-id keresés is elmaradt;
  a league-id keresés (`search_all_leagues.php?c=Hungary&s=Soccer`) a
  felhasználói döntés utáni adapter-szelet follow-upja.
- **Identitás:** `idEvent`/`idLeague`/`idTeam` stabil külső ID-ként tervezett;
  crosswalk + alias kell (nem igazolt).
- **Kvóta/robots:** robots engedett (spike 2026-11-11: `(True, 'robots engedett')`,
  `/api/v1/json/` útvonal). v1-terv: kvóta 40/nap, delay 2+1 s; a mai docs szerint
  ingyenes 30 kérés/perc (napi limit nincs megadva), túllépés 429. Breaker:
  403/429 → azonnali stop, nincs retry (élesben kioldott: 403). Spike-budget
  10/nap, ebből 2 fogyott (robots + eventsseason).
- **Frissesség-javaslat (tervezett):** szezon-gerinc batch naponta
  (`eventsseason.php`), stadion/referee kiegészítés a gerincről.
- **Bizalom domainenként (tervezett, spec §3.2):** fixture magas; eredmény
  közepes (keresztellenőrzés football-data ellenében; a `strStatus`-mező
  pontossága külön spike-jegyzet tárgya).
- **v1-érettség + kötelező őrök:** v1-leltár szerint ingyenes „3” kulcs,
  szezon-gerinc, 40/nap, 2+1 s. Kötelező: üres `events` decoy-guard
  (`guard_rows`, offline teszt OK: `DecoyError` üres listára), robots-kapu,
  napi budget, 403/429 breaker, méretkorlát.
- **Kockázatok + mitigáció:** a „3” kulcs megszűnt/403 → regisztrációs döntés
  kell; az ingyenes „123” kulcs `eventsseason` limitje **15 esemény** (prémium
  3000) — ez egy 380 meccses szezon-gerinchez elégtelen, tehát a gerinc-szerep
  vélhetően prémium kulcsot igényel; mitigáció: focis gerinc football-data
  (rang 1) mellé TheSportsDB csak stadion/referee/meta kiegészítőként, vagy
  felhasználói döntés a prémiumról.
- **Státusz + bizonyíték:** `elvetett` (**felhasználói döntés, 2026-11-12,
  1. pont** — nincs mért rés: gerinc = footballdata, stadion/referee = FotMob;
  korábbi státusz: `regisztráció szükséges`; újrabírálás mért rés esetén) —
  spike 2026-11-11,
  python3.13, `/tmp/s1/probe_thesportsdb.py`: robots 200 engedett, majd
  `eventsseason.php?id=4328&s=2026-2027` a „3” kulccsal **403** → StopSpike,
  nincs retry, minta nem mentődött (nincs sha256). Dekoy-teszt offline OK.
  Docs-olvasat (nem API-hívás): ingyenes kulcs „123”, 30 kérés/perc,
  `eventsseason` free limit 15 / prémium 3000. Élő adaptáció nem történt
  (auth-fal szabály: 403 → stop, kulcsot nem hozunk létre).
- **Adoptációs rang:** 2 (tervezett; a rang a felhasználói döntésig feltételes).

### tipplap — Tipplap.hu

- **slug / név / base URL / típus:** `tipplap` / Tipplap.hu /
  `https://www.tipplap.hu` / aggregator (HU; Tippmix-szorzók + szavazati
  arányok, napi kalkulátor-lista).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, 4 kérés mind 200
  (robots + mai lap + múlt-napi lap + markets JSON), bot-kihívás nélkül,
  162 KB-os HTML és 155 KB-os JSON sima urllib-bal; v1 előzmény:
  `scrappers/tipplap/client.py` (scrapling Fetcher = egyszerű HTTP, élesben futott).
- **Domainek + mezőtérkép:** odds (1X2 + részletes piacok), fixture/kickoff.
  - `data-event` (int, pl. 5546745) → külső event-ID (identity crosswalk);
    `data-event-expire` (unix ts, not null a desktop sorokon) → lejárati idő.
  - Kickoff: `11.11.` + `20:00` külön cellák → `core.match.kickoff_at`
    (timestamptz, UTC); **év nélkül + HU helyi idő** → év-következtetés a
    lap dátumából és időzóna-konverzió kell (adapter-szeletben ellenőrizni).
  - `data-event-name` szöveg „Hazai - Vendég” → `core.match.home/away`
    (text, not null; HU nevek, alias kell); liga-fejléc `th.tleft` → liga-név.
  - H/D/V `data-odds` cellák → `fact.odds_snapshot` `odds.home/draw/away`
    (snapshot_kind opening/closing); `.votes` → szavazati arány (display-only);
    `min_combi`, `market_number` → kombinációs kényszer / piac-sorszám.
  - `more_markets` („+32” jelölő) + `/kalkulator/markets/{id}` JSON
    (`desktop`/`mobile` HTML-fragment): 32 piaccsoport, 115 odds egy meccsen
    (Kétesély, BTTS, Gólszám 2,5/3,5/4,5, Hendikep, Ázsiai Gólszám/Hendikep,
    Pontos végeredmény, páros/páratlan stb.) → **séma-rés: piacok bővítése**
    a `fact.odds_snapshot`-ban (market/outcome dimenziók, S2 bemenet).
- **Lefedettség (demo kör):** a mai (2026-11-11) listán 12 egyedi liga,
  23 egyedi event: angol Liga-kupa (női), női BL, cseh/szlovák Kupa,
  EFL Trophy, brazil/uruguayi 2., francia női bajnokság stb.
  **NB I / NB II a mai listán NINCS** (válogatott szünet) → a lefedettség
  napfüggő; magyar ligás napon újra kell ellenőrizni (adapter-szelet
  follow-up, a v1-leltár szerint NB I/NB II szerepel). **Kupák vannak**
  (Cseh Kupa, Szlovák Kupa, Angol Liga-kupa, Bolíviai Liga-kupa) — a
  kupa-rés probe (Task 12) bemenete; a top-5 demo liga teljes köre mai
  napon nem igazolt (csak női BL + EFL Trophy + kupameccsek).
- **Identitás:** a Tipplap event-ID forráson belül stabil (v1 gyakorlat),
  de nincs crosswalk football-data/TheSportsDB felé → alias + crosswalk kell.
  **HU↔EN alias-igénylista (élő minta):** csapatnevek pl. „Köge” (Køge),
  „Artis Brno”, „OH Leuven”; liga-nevek „Labdarúgás – Cseh Kupa” alakúak
  (ország + típus magyar szövegben) → `team_aliases` + liga-alias tábla,
  fuzzy összevonás tilos.
- **Kvóta/robots:** robots engedett (spike 2026-11-11:
  `(True, 'robots engedett')`, `/kalkulator/` útvonal). Kvóta: **0,3 s delay
  (v1), napi 3 futás + closing-ablak** (meccs utáni zárószorzó-vételezés);
  breaker: 403/429 → azonnali stop, nincs retry. Spike-budget 10/nap,
  ebből 4 fogyott (robots + mai lap + múlt-napi lap + 1 markets JSON).
- **Frissesség-javaslat:** napi lista napi 3 körben (reggel/délben/este),
  closing-odds a meccsek után; a markets JSON csak a kijelölt meccsekre
  (piaci mélység drága: 1 kérés/meccs).
- **Bizalom domainenként:** odds **kétforrásos** — football-data closing a
  párja (spec §3.2: egy forrás = provisional + kliens-jelzés); a Tipplap
  Tippmix-szorzó, nem piac-átlag. Fixture/kickoff: aggregator scrape →
  közepes, keresztellenőrzés football-data/TheSportsDB ellenében.
- **v1-érettség + kötelező őrök:** v1 adapter éles (`scrappers/tipplap/client.py`,
  TipplapDateMismatch-dokumentációval). Kötelező:
  1. **dátum-guard (fail-closed)** — a `#dates` select `selected` optionje a
     kiszolgált nap; múlt-napi kérésre a Tipplap a MAI listát szolgálja
     (v1: 2026-10-03–16 között napi ~2×130 rossz napra írt duplikált sor).
     Élőben igazolva 2026-11-11-én (lásd státusz).
  2. **üres odds guard** (`guard_rows`, offline teszt OK: `DecoyError`).
  3. desktop-tábla only parse (a mobil tábla duplikál: 138 odds = 2×69 sor).
  4. szimulált-meccs szűrő a zárójeles nevekre (v1 minta, eredmény-oldal).
- **Kockázatok + mitigáció:** múltbeli nap NEM pótolható a kalkulátorból
  (a guard fail-closed megállítja; eredményre a v1 külön
  `/eredmenyek/lista/labdarugas/{nap}` oldalt használ — ez a spike nem
  mérte); napfüggő NB I/NB II lefedettség → magyar ligás napi újraellenőrzés;
  HTML-szerkezet változása → a `#dates`/`data-odds` jelölők elvesztése
  esetén a guard fail-closed (nincs jelölés = nincs írás); odds egyforrásos
  időszaka → provisional jelzés a kliensben.
- **Státusz + bizonyíték:** `igazolt` — spike 2026-11-11, python3.13,
  `/tmp/s1/probe_tipplap.py`: robots 200 engedett; mai lap
  (`/kalkulator/labdarugas/2026-11-11`) 200, dátum-guard zöld
  (`#dates selected = 2026-11-11`, ISO-dátum jelen), 138 odds-érték /
  23 egyedi event, `guard_rows` zöld; **múlt-napi teszt: a 2026-11-08-i
  URL a 2026-11-11-i listát szolgálta (mismatch = True), a két lap a
  CSRF-tokenen kívül byte-azonos** — a v1 TipplapDateMismatch élőben
  reprodukálva, a guard éles és kötelező; markets JSON 200, 115 piaci
  odds / 32 csoport; decoy-teszt offline OK. Élő adaptációk
  (controller-ruling, őrök érintetlenek):
  - URL: a brief `?date=` mintája helyett `/kalkulator/labdarugas/{YYYY-MM-DD}`
    (v1 `fetch_day`); a markets JSON `/kalkulator/markets/{id}`.
  - odds-kitermelés: a brief JSON-regexe (`"h_odds":`) helyett HTML
    `data-odds` cellaszöveg (v1 `parse_day`); 138 találat, attribútum-alak 0.
  - dátum-guard: puszta „mai dátum a szövegben” helyett a `#dates` selected
    option összehasonlítása (a dátumválasztó miatt a mai dátum egy múlt-napi
    oldalon is jelen van — a szubsztring-teszt hamis zöldet adna); tartalék
    a kickoff MM.DD. módusz (v1 `served_day_mmdd`).
  - `showMarkets` jelölő „+32” alakú → regex `\+?(\d+)` (első futás 0
    találat, offline javítás a mentett mintán, új hálózati kérés nélkül).
  - liga/event deduplikálás (desktop + mobil tábla); a múlt-napi minta
    `past_<date>.html` néven mentve, hogy a `day_<date>.html` mintát a
    Task 12 egyértelműen olvashassa.
  Minta-sha256 (`/tmp/s1/samples/tipplap/`):
  - `day_2026-11-11.html`: `da659c56a0cc82e3bd249b73f34a5c9cb54dbc2698dcecf81183a923487fc062`
  - `past_2026-11-08.html`: `e11c8f9676a4e0b41160d0426a9ecc24044795bafcc36f14be4e0437f91faf66`
  - `markets_5546745.json`: `09222c6775c144f6250b01201f15e99c80a00c50c92831be82d99950e4209e81`
- **Adoptációs rang:** 3.

### fotmob — FotMob

- **slug / név / base URL / típus:** `fotmob` / FotMob /
  `https://www.fotmob.com` / aggregator (globális; napi fixture + meccs-detail).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** **hibrid, spike-döntés (2026-11-11):**
  - **Nap-oldal** (`/matches?date=YYYYMMDD` → 308 → `/`): `obscura` — plain
    HTTP (harness UA ÉS Chrome UA curl egyaránt) 299 948 B-os homepage-shellt
    ad: `__NEXT_DATA__` van, de csak translationmapping fallback, 0 DOM
    meccs-link (a két minta byte-azonos, sha256 egyezik → UA-független).
    Obscura-render (v1 Engine CDP-út): 485 573 B, **71 egyedi meccs** a v1
    `_parse_day_page` production parserrel. A nap-feed szerződése ma DOM-anchor
    (`/matches/<slug>/<hash>#<id>`), NEM `pageProps.leagues` JSON (render után
    sincs jelen).
  - **Meccs-oldal** (`/match/{id}`): `rust-http` — curl 200, 1 135 229 B,
    `__NEXT_DATA__` van, 46 lineup-játékos; a v1 probe három szállítása
    (curl/scrapling/obscura) azonos parser-eredményt ad, az obscura 18×
    lassabb (14 342 ms vs 791 ms). A mentett plain HTTP mintán MINDEN v1
    production parser zöld (lásd státusz).
  - v1 előzmény: `docs/obscura-source-matrix-2026-11-09.md`,
    `docs/web-scraping-architecture.md`.
- **Domainek + mezőtérkép:** fixture/kickoff, eredmény, lineups, hiányzók,
  xG, meccs-stats, incidensek, shotmap, játékos-stats, tabella, venue/referee.
  - `general.matchId` → külső ID (identity crosswalk); `header.status.utcTime` /
    `general.matchTimeUTC` → `core.match.kickoff_at` (timestamptz, **UTC — a
    forrás explicit UTC-t jelöl**, konverzió nem kell); `general.leagueId` →
    liga külső ID.
  - `header.teams[].name/id` → `core.match.home/away` (text, not null, alias
    kell); `header.teams[].score` → `core.match_result.home_ft/away_ft` (int,
    null amíg `general.finished` nem igaz); `header.status.halfs` → HT/FT idő.
  - `content.lineup.{homeTeam,awayTeam}.{starters,subs}` (11+12 / oldal;
    játékos `id`, `optaId`, `positionId`, `shirtNumber`, `rating`) →
    **séma-rés: `match_lineups`**.
  - `...unavailable[].unavailability{type,injuryId,expectedReturn}` →
    **séma-rés: `player_availability`**.
  - `content.stats.Periods.{All,FirstHalf,SecondHalf}` — 14 metrika
    (possession, xg, shots, shots_on, big_chances, accurate_passes, yellows,
    corners, woodwork, shots_in_box, passes, offsides, reds, fouls),
    home/away bontásban → **séma-rés: `match_team_stats`**.
  - `content.shotmap.shots` (36 lövés a mintán; `expectedGoals`,
    `expectedGoalsOnTarget`, x/y, min, shotType) → **séma-rés: `match_shots`**
    (v1 minta, xG-domain).
  - Incidensek: `matchFacts.events.events` (elsődleges) / `header.events`
    (visszaesés) → **séma-rés: `match_incidents`**; a v1 séma-drift őr
    (2026-09-29: a régi útvonal csendben 0 sort adott) kötelező.
  - `content.playerStats` (452 sor a mintán: rating + játékos-metrikák) →
    játékos-stats (v1 `match_player_stats` minta, S2).
  - Tabella: a meccs-oldal `content.table` csak referencia
    (`data.fotmob.com/tables.ext.{id}.fot.gz` — **nem vételezett**, másik
    host, robots nem ellenőrzött) + 2 csapat-sor; a nap-oldal renderelt
    DOM-ja teljes PL-tabellát tartalmaz (élő minta: Arsenal 1., 10 meccs,
    27 pont, 15-5 gólkülönbség); v1 út: csapat-oldal fallback →
    `core.standing` keresztforrás (a football-data számított az elsődleges).
  - `season_team_stats`: v1 csapat-oldal (`team_season`, 29 szezon-metrika)
    → **séma-rés: `season_team_stats`** — ebben a spike-ban NEM mért,
    v1-bizonyíték.
  - Venue/referee: a v1 `parse_fotmob_context` a meccs-oldalból olvas
    (minta: Estadio Presidente Juan Domingo Perón, referee: Nazareno Arasa)
    → **séma-rés: `core.match.stadium/referee`** (azonos rés, mint
    TheSportsDB).
- **Lefedettség (demo kör):** a nap-oldalon (2026-11-11, válogatott szünet)
  71 egyedi meccs: Friendlies 1, női BL 2, brazil Série B 1, CAF női BL 2,
  kolumbiai Primera B 2, kolumbiai Kupa 2, cseh Kupa 1 + kiemelt PL-csoport
  60 anchor (**többfordulós körhinta, nem csak mai meccsek — dátum-guard
  kell**). Top-5 liga-ID azonosítva (47 PL, 87 LaLiga, 55 Serie A,
  54 Bundesliga, 53 Ligue 1), NB I = 212, Magyar Kupa = 213 — mai meccsük
  nincs (napfüggő, Tipplap-minta). **NB II a mai oldalon nem található** →
  adapter-szelet follow-up. Kupák vannak (kolumbiai, cseh) → Task 12 bemenet.
- **Identitás:** FotMob match/team/player ID forráson belül stabil (v1
  gyakorlat: `tipplap_events.fotmob_match_id` crosswalk); játékos `optaId`
  jelen (Opta-crosswalk jelölt); EN nevek → HU↔EN alias Tipplap felé,
  fuzzy összevonás tilos.
- **Kvóta/robots:** robots engedett `/matches` (spike 2026-11-11:
  `(True, 'robots engedett')`), **`/api/*` tiltott** (`(False, 'robots tilt')`)
  → csak HTML út. Kvóta 200/nap, delay 6+6 s (v1); breaker: 403/429 →
  azonnali stop, nincs retry. Spike-budget 10/nap; a spike napján
  (2026-11-11) összesen **12/10 fotmob kérés = BUDGET-TÚLLÉPÉS**:
  8 harness-ledger kérés + 4 ledgeren kívüli v1-probe kérés (controller
  által engedélyezett read-only mérés). Ez **szabálysértésként van
  rögzítve, nem budget-konform futásként**; a 12 kérésen felül aznap
  további fotmob kérés nem történt. A jövőben minden ledgeren kívüli
  mérésnek (v1 probe) a várt kéréseket ELŐRE el kell számolnia a
  harness-budget ellen — foglalás, majd futtatás, majd naplózás;
  az utólagos naplózás önmagában nem elég.
- **Frissesség-javaslat:** nap-feed naponta (obscura render); meccs-detail
  meccs utáni batch; lineups a kickoff előtt ~1 órával (v1 gyakorlat,
  spike-ban nem mért).
- **Bizalom domainenként:** minden domain **közepes, keresztforrás**
  (spec §3.2: aggregator fixture; kezdő/hiányzó/xG/stats display-only,
  nem gate-tényező; tabella keresztellenőrzéssel a football-data számított
  ellenében).
- **v1-érettség + kötelező őrök:** v1 adapter éles
  (`scrappers/web/sources/fotmob.py`, 1331 sor). Kötelező:
  1. meccs-oldal `__NEXT_DATA__` guard (nincs JSON → nincs írás);
  2. nap-oldal DOM-anchor decoy-guard (`_parse_day_page` sorok,
     `guard_rows`; offline teszt OK: `DecoyError`);
  3. **séma-verzió jegyzet**: a nap-feed szerződése a v1 mérés (2026-09-24)
     óta változott — `pageProps.leagues` JSON eltűnt, DOM-anchor maradt;
     incidensek két útvonal (matchFacts elsődleges / header.events
     visszaesés, v1 2026-09-29 drift);
  4. **dátum-guard**: a kiemelt liga-körhinta többfordulós anchorokat ad a
     nap-oldalon → az adapternek meccs-dátumra kell szűrnie;
  5. 403/429 breaker + napi budget + 5 MB méretkorlát.
- **Kockázatok + mitigáció:** nap-oldal kliens-renderelt → **v2 Obscura-
  integráció kell** (még nincs megépítve; a v1 bináris mérve, self-host);
  DOM-törékenység → parser-szintű őrök fail-closed; séma-drift → verzió-
  jegyzet + spike újraellenőrzés; a `data.fotmob.com` gz-tabella nem
  vételezett (robots nem ellenőrzött) → tabella a nap-oldal DOM-ból /
  csapat-oldalról; PL-körhinta dátum-szennyezés → dátum-guard.
- **Státusz + bizonyíték:** `igazolt` — spike 2026-11-11, python3.13,
  `/tmp/s1/probe_fotmob.py`, `probe_fotmob_day.py`,
  `probe_fotmob_obscura_day.py`, `probe_fotmob_match.py` + v1
  `scripts/probe_obscura_transport.py --only fotmob_meccs` (read-only,
  v1 venv python 3.13.17): robots-kapu zöld; nap-oldal plain HTTP decoy
  (0 link) → obscura 71 meccs (v1 production parser); meccs-oldal plain
  HTTP 200, a mentett mintán MINDEN v1 production parser zöld: 7 incidens,
  14 stats-metrika (xG 2.69/1.94), 36 lövés, 452 játékos-stats sor,
  venue + referee; decoy-teszt offline OK. Élő adaptációk
  (controller-ruling, őrök érintetlenek):
  1. dátum-formátum: a brief ISO mintája helyett v1 `YYYYMMDD` (`DAY_PAGE`);
     az ISO alak külön próbán 302 → **500** (nincs retry).
  2. `pageProps.leagues` útvonal nem létezik → a nap-feed guard a v1
     `_parse_day_page` DOM-soraira fut (production parser).
  3. nap-oldal curl-mérés Chrome UA-val (a v1 probe script curl-ágának
     mintájára), harness-őrök kézzel alkalmazva (`log_budget`, 6 s delay,
     méretkorlát, breaker).
  4. curl `-L --max-redirs 5`: az első curl-futás 308-at kapott (a v1
     curl-ág nem követ redirectet); redirect-cél `/?date=YYYYMMDD` naplózva.
  5. a v1 probe scriptnek nincs `--url` kapcsolója (a brief parancsa
     elavult) → `--only fotmob_meccs` + külön read-only wrapper a nap-oldal
     obscura mérésére (v1 Engine + parserek, változtatás nélkül, DB nélkül).
  6. meccs-oldal minta + mezőtérkép-dump (a v1 probe célpontja: 1000019612)
     a lineups/hiányzók/xG/stats domainek élő igazolására.
  Minta-sha256 (`/tmp/s1/samples/fotmob/`):
  - `matches_2026-11-11.html` (harness UA shell): `24897e87f129d302af17ad2797e3898b31c70f626359358cc980460c9963b44a`
  - `matches_2026-11-11.json`: `f78e2c7b521b806b833c1cd611fec74ec90b9d3a37ca64729efebab372fe8f98`
  - `matches_curl_2026-11-11.html` (Chrome UA shell, byte-azonos a harness
    UA mintával; a `.json` párja szintén azonos): `24897e87f129d302af17ad2797e3898b31c70f626359358cc980460c9963b44a`
  - `matches_obscura_2026-11-11.html` (renderelt, 71 meccs): `6c0ccb0ef4398b472048e14e4181c472a57344e60d0dfb3511b7c141eba5a611`
  - `match_1000019612_2026-11-11.html`: `15b836f5e6444aa876049bc75a8fdba066101d4ec2afb257f17e3cd246e1d6d5`
  - `match_1000019612_2026-11-11.json`: `574a4631d84bd270c08fdfa3b340254c2df0498950a994f2b45a401c0bf9b3c1`
  Nem ellenőrzött: NB II lefedettség, `season_team_stats` (csapat-oldal),
  teljes tabella (csak PL DOM-table + 2 csapat-sor `content.table`),
  lineups kickoff előtti időzítése.
- **Adoptációs rang:** 4.

### transfermarkt — Transfermarkt

- **slug / név / base URL / típus:** `transfermarkt` / Transfermarkt /
  `https://www.transfermarkt.com` / aggregator (szerver-renderelt HTML;
  sérüléslisták + szezon-góllövőlisták, ingyenes, kulcs nélkül).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, mindkét oldal
  (hiányzók + góllövők) 200 plain urllib-bal, bot-kihívás nélkül, a
  `<table class="items">` adatsorok a HTML-ben vannak (200 304 B, illetve
  153 514 B). v1 előzménymérés: `scrappers/web/sources/transfermarkt.py`
  (2026-09-24: szerver-renderelt, robots engedett; 2026-10-02: a rendszer
  curl-je és a curl_cffi azonos 68 sort adott a GB1 sérüléslapon).
  **Figyelem (v1, 2026-10-02): a TM serving KLIENS-FÜGGŐ** — ugyanarra az
  URL-re a curl TISZTA listát adott, a curl_cffi + chrome-impersonálás
  SZEMETET (ARG/BRA táskán 4–9 purge-olt játékosnév ismétlődött) → a
  rust-http út mellett a név-őr kötelező, chrome-fallback nem indokolt.
- **Domainek + mezőtérkép:** hiányzók (sérüléslista) + góllövők
  (szezon-statisztika). Mindkettő **séma-rés** a v2-ben.
  - Hiányzók (`/{slug}/verletztespieler/wettbewerb/{code}`): a sor
    `a[title]` játékos- (`/profil/spieler/{id}`) és klub- (`/verein/{id}`)
    azonosítót ad; fejléc: Player/Position, Club, Injury, until, Market Value.
    - játékosnév + poszt → **séma-rés: `player_availability`** (name text,
      not null; status `injured`; reason text — élő minta: „Knee problems”,
      „Hamstring injury”).
    - `until` oszlop → `player_availability.expected_return` (date,
      **nullázható** — a GB1 mintán 54/85 sorban DD/MM/YYYY dátum
      (pl. 12/12/2026, 21/12/2026), 31 üres; v1 is None-t ír az üresekre).
    - klub → crosswalk a mag-modell csapatához (v1: következő meccshez
      kötött bejegyzés, `resolved_by=club_name` bizonyítékkal).
    - piacérték (€) display-only kiegészítés, nem gate-tényező.
  - Góllövők (`/{slug}/torschuetzenliste/wettbewerb/{code}/saison_id/{id}`):
    fejléc #, Player, Nat., Age, Club, Appearances, Goals — az utolsó kettő
    **IKON-fejléc** (`title="Appearances"` / `title="Goals"`, szöveges
    fejléc-regex nem találja). Élő minta (Haaland): 10 mérkőzés, 9 gól.
    - goals/apps/age/position → **séma-rés: `season_player_stats`**
      (stat_key/value, v1 minta); **PERC az oldalon NINCS** (v1 mérés
      2026-09-30: a régi kód az életkort gólnak, a gólt percnek olvasta —
      oszlop-csúszás őr kötelező).
  - Időzóna: nincs timestamp-mező; a `until` dátum, a szezon-id kezdőév.
- **Lefedettség (demo kör):** a v1 regiszter (`league_source_registry`,
  235-ös seed) **21 ligát** tartalmaz (a v1 docstring „~25”-öt mond — a
  mért seed 21): top-5 megvan (GB1 premier-league, L1 bundesliga,
  ES1 la-liga, IT1 serie-a, FR1 ligue-1), **+ NB I / NB II HIÁNYZIK** →
  rés-jelölés: hiányzókra FotMob (`player_availability`), góllövőkre
  adapter-szelet follow-up. Ebben a spike-ban élőben csak a GB1 mindkét
  oldala mért; a többi liga a v1 regiszter-bizonyíték (2026-09-24/10-02),
  nem élő igazolás. A kódok NEM ISO-k (Cseh=TS1, Svájc=C1, Horvát=KR1,
  Brazília=BRA1, Argentína=ARG1).
- **Identitás:** játékos-ID (`/profil/spieler/{id}`) és klub-ID
  (`/verein/{id}`) forráson belül stabil (GB1 minta: 86 egyedi játékos-ID,
  18 egyedi klub-ID a sérüléslapon); crosswalk + `team_aliases` kell
  (EN nevek, pl. „Arsenal FC” vs „Arsenal”; v1: teljes jogi nevek
  „CR Flamengo” → kétirányú token-egyezés, fuzzy összevonás tilos).
- **Kvóta/robots:** robots engedett (spike 2026-11-11:
  `(True, 'robots engedett')`, `/premier-league/verletztespieler/` útvonal).
  Kvóta 40/nap, delay 8+8 s (v1); breaker: 403/429 → azonnali stop, nincs
  retry. Spike-budget 10/nap, ebből **5 fogyott**: 1 robots + 2 oldal ×
  2 futás — a második futás VÉLETLEN duplikáció (az offline teszt
  importálta a probe-modult, ami akkor még nem volt `__main__`-guardolt);
  a budget-en belül maradt (5/10), de a 2 extra kérés nem volt tervezve.
  A probe azóta `__main__`-guardot kapott (offline javítás, új kérés nélkül).
- **Frissesség-javaslat:** sérüléslista 48 óránként (v1 regiszter
  `refresh_hours=48`), góllövőlista hetente/havonta (v1 `refresh_hours=720`,
  szezon-task). A hiányzó-bejegyzés a csapat következő meccséhez kötött
  (v1 gyakorlat — a fogadói használat ezt kívánja).
- **Bizalom domainenként:** hiányzók és góllövők **közepes, display-only**
  (spec §3.2: kezdő/hiányzó/xG/stats — nem gate-tényező); keresztellenőrzés
  FotMob hiányzó-listájával, ahol átfedés van.
- **v1-érettség + kötelező őrök:** v1 adapter éles
  (`scrappers/web/sources/transfermarkt.py`, 569 sor; a brief
  `sources/transfermarkt.py` útvonala elavult). Kötelező:
  1. **`_has_table` tartalmi őr (fail-closed)** — `'<table class="items'`
     nyitó tag (a teljes class-lista miatt a `table.items` szöveges
     keresés hamis negatív); ismeretlen slug/szezon a
     `/navigation/wettbewerbe` indexre redirectál (200, tábla nélkül) →
     DecoyError, nem csendben 0 sor. Offline teszt OK: navigation-decoy
     stringre `guard_rows` DecoyError-t dob.
  2. **season-id konvenció (ÉV-KONVENCIO, v1 mérés 2026-10-01):** a TM a
     szezont a KEZDŐÉV alatt tárolja — GB1 id=2025 → '25/26'; naptár-év
     ligáknál (NO1/SE1/BRA1/ARG1) is CÍMKE−1 id kell (NO1 id=2025 →
     '2026'); ARG-lista csak id=2024-től él. Élőben igazolva:
     `saison_id/2026` → selected opció '26/27'. A rossz id tábla nélküli
     200-at ad → az 1. őr fogja meg.
  3. **oszlop-csúszás őr** (v1 `parse_scorer_row`): életkor 15–45,
     mérkőzés 0–100, gól ≤ 3×mérkőzés (gól LEHET több, mint meccs);
     a 2026-09-30-i hiba (életkor→goals, gól→minutes) óta kötelező.
  4. **név-őr** a kliens-függő serving ellen (purge-olt nevek ismétlődése
     → írás leállítása, v1 ARG/BRA gyakorlat).
  5. robots-kapu + napi budget + 403/429 breaker + 5 MB méretkorlát.
- **Kockázatok + mitigáció:**
  - **Szerkezet-drift a v1 mérés óta:** a góllövőlista ma 25 soros,
    lapozott lista (a v1 3×30 hasábos elrendezésével szemben), a második
    `items`-tábla „Most valuable player” widget (nem adatsor) → a parser
    csak a `yw1` főtáblát olvassa, lapszám-őr kell; a fejléc ikon-alapú
    (Appearances/Goals title) → adapter-szeletben újra-mérendő.
  - `until` oszlop: a GB1 mintán 54/85 sorban DD/MM/YYYY dátum, 31 üres →
    expected_return nullázható az üres sorokra, nem nullával
    helyettesítendő; más ligákon élőben újra-mérendő (adapter-szelet).
  - Kliens-függő serving (curl_cffi szemét) → rust-http + név-őr.
  - NB I/NB II rés → FotMob/Tipplap keresztforrás, adapter-szelet follow-up.
  - Rossz slug/szezon-id decoy-200 → `_has_table` fail-closed.
- **Státusz + bizonyíték:** `igazolt` — spike 2026-11-11, python3.13,
  `/tmp/s1/probe_transfermarkt.py`: robots 200 engedett; hiányzók GB1 200
  (200 304 B), `<table class="items">` jelen, 85 adatsor / 85 hauptlink
  cella, 86 egyedi játékos-ID, 18 klub-ID, `guard_rows` zöld; góllövők
  GB1 `saison_id/2026` 200 (153 514 B), 25 góllövő-sor (hauptlink),
  selected szezon-opció '26/27' (kezdőév-konvenció élőben igazolva),
  Haaland 10 meccs / 9 gól, perc-oszlop nincs; decoy-teszt offline OK
  (üres lista + navigation-decoy egyaránt DecoyError). Élő adaptációk
  (controller-ruling, őrök érintetlenek):
  1. v1 referencia-útvonal: a brief `sources/transfermarkt.py` helyett
     `scrappers/web/sources/transfermarkt.py` (a tényleges hely).
  2. a brief hauptlink-regexe elé bekerült a v1 `_has_table` tartalmi őr
     (a regex önmagában nem különbözteti meg a navigation-decoyt).
  3. góllövő-probe `saison_id/2026`-val (v1 kezdőév-konvenció; a brief
     csak a hiányzó-oldalt kérte) — a season-id quirk élő igazolására.
  4. python3.13 (controller-előírás, a brief `python3` helyett).
  5. véletlen duplikált probe-futás (import-mellékhatás, 2 extra kérés,
     5/10 budget-en belül) → `__main__`-guard offline javítás; a góllövő-
     minta a második futásból származik (1 B eltérés: dinamikus tartalom).
  Minta-sha256 (`/tmp/s1/samples/transfermarkt/`, a lemez jelenlegi állapota):
  - `verletztespieler_GB1.html`: `b46999c58264bb580af935caba107be0e92500cd0607da67eb4d68c71bf77d30`
    (mindkét futásban byte-azonos)
  - `torschuetzenliste_GB1_2026.html`: `be11f0aef8c367b987b07e89a24828bd057eecc3a0a1f061527452c4b126e0a7`
    (első futás: `ec8a39e047857e9b4251d838cf14acdbc3af3052012a66797d9b1ec0701a6600`,
    153 513 B — felülírva)
  Nem ellenőrzött: a többi 20 liga élőben (v1 regiszter-bizonyíték),
  NB I/NB II (nincs a regiszterben), góllövő-lapozás (25 sor/oldal).
  Az `until` DD/MM/YYYY formátum a mentett GB1 mintán MÉRT (54/85 dátum,
  31 üres); más ligákon az élő újramérés az adapter-szeletben szükséges.
- **Adoptációs rang:** 5.

### worldfootball — WorldFootball.net

- **slug / név / base URL / típus:** `worldfootball` / WorldFootball.net /
  `https://www.worldfootball.net` / archívum (szerver-renderelt HTML;
  UEFA-kupák gerince: CL/EL/ECL, v1 szerint 1955/56 óta, ~150 meccs/szezon/verseny).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` (tervezett) — **élő bizonyíték NINCS: a tartalmi
  útvonal 403-at adott** (2026-11-11, harness UA, plain urllib), miközben a
  robots.txt ugyanazzal a UA-val 200. v1 előzménymérés:
  `scrappers/web/sources/worldfootball.py` (2026-09-27: HTTP elég,
  szerver-renderelt, Cloudflare-kihívás nélkül mérve) — a mai 403 tehát
  serving-változás vagy kliens-függés (vö. Transfermarkt kliens-függő
  serving), nem igazolt adatminőségi probléma. UA-váltás/chrome-fallback
  ebben a spike-ban NEM engedélyezett (a controller-ruling csak
  versenykód/szezon-slug/szelektor adaptációt enged; 403 → StopSpike,
  nincs retry).
- **Domainek + mezőtérkép (tervezett, v1-szelektorok; spike NEM igazolta):**
  kupa-fixture/kickoff, eredmény FT, pens observed státusz.
  - `data-match_id` → külső meccs-ID (identity crosswalk); a report-link
    `/ma{id}/` második külső ID (v1 `external_id_map` gyakorlat).
  - `data-datetime` (UTC ISO, az autoritatív forrás) → `core.match.kickoff_at`
    (timestamptz, **UTC**, konverzió nem kell); idő nélküli sor → `date_only`
    déli helyőrző (v1 konvenció, nem null).
  - `.team-name-home`/`.team-name-away` → `core.match.home/away` (text,
    not null; EN nevek, alias kell).
  - `.match-result` → `core.match_result.home_ft/away_ft` (int, null amíg
    nincs eredmény); a `match-result-aggregrated` (Total:) és a
    `related:match-result` (1st/2nd leg) NEM eredmény (v1 őr).
  - `pso` (`.match-incident` szöveg) → **pens observed státusz**: a kijelzett
    pontszám a megfigyelt érték, forrással együtt tárolandó, tizenegyesre
    NEM fejtjük szét (spec: ET/pens csak explicit forrásból).
  - `.round-head` → `round_label`; `.match-date`/`.match-time` → kickoff
    visszaesés (date_only ág).
  - Új séma-rés a spike-ban NEM mérhető (403); a tervezett mezőtérkép a
    meglévő `core.match`/`core.match_result` + pens observed státuszra épül.
- **Lefedettség (demo kör):** **UEFA-kupák: CL/EL/ECL** (v1 regiszter:
  co19 `uefa-champions-league`, co132 `uefa-europa-league`, co6395
  `uefa-conference-league`) — élőben NEM igazolt (403). A top-5 bajnoki
  gerinc nem ez a forrás (football-data rang 1). **FA Kupa / hazai kupák:
  NINCS — EXPLICIT RÉS**: a worldfootball csak UEFA-kupákat ad, a hazai
  kupák (FA Kupa, Magyar Kupa) lefedettségét más forrásnak kell pótolnia →
  **Task 12 FA Kupa-rés probe** (thefa.com / Sky Sports / Soccerway / ESPN /
  Tipplap; a Tipplap- és FotMob-mintákon kupameccsek már látszanak).
- **Identitás:** `data-match_id` + report-ID forráson belül stabil (v1
  gyakorlat); az oldal NEM ad országot → csapatnév az egyetlen kulcs,
  0/1/N-szabály (0 → új sor NULL országgal, 1 → egyezés, N → kihagyás,
  nincs csendes összevonás); crosswalk + `team_aliases` kell, fuzzy
  összevonás tilos.
- **Kvóta/robots:** robots engedett (spike 2026-11-11:
  `(True, 'robots engedett')`, `/competition/` útvonal, 4082 B); v1 mérés:
  csak `/xml/` és `/*?` tiltott, AI-tanító crawlerek tiltva (mi tényadatot
  gyűjtünk, nem tanítunk). Kvóta 40/nap, delay 8+8 s (v1), **1 kérés/verseny/
  szezon**; breaker: 403/429 → azonnali stop, nincs retry (**élesben
  kioldott: 403**). Spike-budget 10/nap, ebből 2 fogyott (robots 200 +
  tartalmi oldal 403); a stop után további kérés NEM történt.
- **Frissesség-javaslat (tervezett, v1):** napi 1 kérés/verseny az aktuális
  szezonra; történeti backfill prioritással (aktuális 25, utolsó 5 lezárt
  40, régebbi 80 — a 40/nap kvóta hetek alatt hozza be).
- **Bizalom domainenként (tervezett, spec §3.2):** eredmény FT **magas**
  (hivatalos közeli, egy forrásból is verified — a spec mátrixa explicit
  sorolja); fixture/kickoff a spec magas listáján NEM szerepel → közepes,
  keresztellenőrzés football-data/TheSportsDB ellenében; pens csak
  observed státuszként, nem gate-tényező.
- **v1-érettség + kötelező őrök:** v1 adapter éles
  (`scrappers/web/sources/worldfootball.py`, mérve 2026-09-27; a brief
  `sources/worldfootball.py` útvonala elavult). Kötelező:
  1. üres lista decoy-guard (`guard_rows`, offline teszt OK: üres lista ÉS
     meccs nélküli decoy-HTML egyaránt `DecoyError`);
  2. **pso observed** — a tizenegyespárbajjelzésű eredményt nem fejtjük szét;
  3. `.match-result` only (aggregált/related dobozok nem eredmények);
  4. `data-datetime` UTC autoritatív; idő nélkül `date_only` déli helyőrző;
  5. csapat 0/1/N fail-closed (N jelöltnél kihagyás, alias-réteg dönt);
  6. robots-kapu + napi budget + 403/429 breaker + 5 MB méretkorlát.
- **Kockázatok + mitigáció:**
  - **403-blokk élőben (2026-11-11):** a v1 szeptemberi HTTP-mérése óta a
    serving változhatott (UA-/kliens-függés, IP-alapú tiltás vagy rate-limit
    nem különíthető el egyetlen kérésből — retry tilos). Mitigáció:
    felhasználói döntéssel későbbi, engedélyezett szállítási újra-probe
    (W4 hozzáférési mátrix vagy saját adapter-szelet); addig a kupa-gerinc
    szerepét más forrásnak kell átvennie (FotMob/Tipplap kupameccsei,
    Task 12).
  - ToS-kockázat: © HEIM:SPIEL Medien — kíméletes tempó + attribution
    (v1 gyakorlat).
  - HTML-szerkezet drift → osztály-alapú szelektorok + decoy-guard
    fail-closed.
- **Státusz + bizonyíték:** `elvetett` (**403-blokk a spike szintjén** —
  NEM adatminőségi/ToS-elvetés; a v1-bizonyíték érvényes marad, a státusz
  engedélyezett újra-probe-val felülvizsgálható) — spike 2026-11-11,
  python3.13, `/tmp/s1/probe_worldfootball.py`: robots 200 engedett
  (`/competition/`), majd a CL all-matches oldal
  (`/competition/co19/uefa-champions-league/all-matches/`) **403** →
  StopSpike, nincs retry; minta NEM mentődött (nincs sha256), mezőtérkép/
  lefedettség/pso élőben NEM igazolt. Decoy-teszt offline OK. Élő
  adaptációk (controller-ruling, őrök érintetlenek):
  1. URL-séma: a brief `co19/uefa-champions-league-2026-2027/all-matches/`
     alakja nem a worldfootball sémája (a szezon `se{id}/YYYY-YYYY`
     szegmensben él) → a v1 `uefa_index` gyökér all-matches URL-je
     (`co19/uefa-champions-league/all-matches/`), amely az aktuális szezonra
     irányít és a szezon-navigátort is adja. Az adaptált URL is 403 — a
     blokk tehát nem URL-alak kérdése (a robots-engedett útvonalon belül).
  2. python3.13 (controller-előírás, a brief `python3` helyett).
  3. a probe `parse_page` offline decoy-tesztje (meccs nélküli HTML →
     `DecoyError`) — hálózati kérés nélkül.
  Nem ellenőrzött: MINDEN élő tartalmi állítás (meccs-szám, `data-datetime`
  UTC-alak, `.match-result`, pso, szezon-navigátor, EL/ECL kódok) — a 403
  miatt csak v1-tervezett szinten szerepelnek.
- **Adoptációs rang:** 6 (tervezett; az elvetett státusz feloldásáig
  függőben — a kupa-gerinc szerep átadásáról Task 12 / felhasználói döntés).

### logos-github — football-logos (GitHub, luukhopman)

- **slug / név / base URL / típus:** `logos-github` / football-logos
  (`luukhopman/football-logos`) /
  `https://raw.githubusercontent.com/luukhopman/football-logos` /
  open-data/CDN (statikus PNG-repo, GitHub raw CDN).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, raw PNG 200
  (28 744 B) plain urllib-bal, bot-kihívás nélkül; a GitHub API (repo meta,
  git tree) szintén 200 plain urllib-bal. v1 előzmény nincs (új forrás).
- **Domainek + mezőtérkép:** klublogó (display-only); **zászló: NINCS**
  (a repo csak klublogókat tartalmaz) → rés-jelölés.
  - Repo-útvonal `logos/<Ország - Liga>/<Csapat>.png` → csapat-logó asset;
    **séma-rés: v2 asset/logó-nyilvántartás** (v1 minta: `image_assets`,
    `assets/megapack/team/{id}.webp` slotok).
  - PNG 139×181 px (README + élő IHDR-mérés egyezik); nincs timestamp-mező →
    frissesség = repo `pushed_at` (2026-07-21, repo meta).
  - Csapat-feloldás: fájlnév = EN csapatnév („Arsenal FC”) → crosswalk +
    `team_aliases`, fuzzy összevonás tilos.
- **Lefedettség (demo kör):** top-25 európai liga, 2026/27 szezon
  (`logos/`, 25 liga-könyvtár) + `history/` 2021-22-től 2025-26-ig;
  összesen 2304 fájl (tree, nem truncated). Top-5 liga megvan (England -
  Premier League, Spain - LaLiga, Italy - Serie A, Germany - Bundesliga,
  France - Ligue 1); élő minta: Arsenal FC.png (PL). **NB I / NB II HIÁNYZIK**
  (nincs magyar liga-könyvtár) → rés-jelölés. A PL-könyvtárban pontosan
  20 PNG van (mentett tree, truncated: false): 20 külön csapatnév,
  csapatonként egy fájl, nincs változat/extra csapat.
- **Identitás:** nincs külső ID; a fájlnév az egyetlen kulcs → crosswalk +
  alias kell (a spike-ban csak az Arsenal-minta élt, nem teljes kör).
- **Kvóta/robots:** **nincs robots** mindkét hostról: a
  raw.githubusercontent.com robots.txt **404** és az api.github.com
  robots.txt **404** (spike 2026-11-11:
  `(False, 'robots.txt hiba: HTTP Error 404: Not Found')` — nem elérhető,
  NEM tiltás; controller-ruling: statikus hoston a hiányzó robots nem tiltás).
  GitHub API auth nélküli rate limit 60/óra (policy, spike-ban NEM mért).
  Delay 2 s (robots 4 s); breaker: 403/429 → azonnali stop, nincs retry.
  Spike-budget 10/nap, ebből **10 fogyott** (4 robots + README + 2×404
  brief-útvonal + tree + PNG + repo meta; a 2. futás robots-duplikációja
  a napi cache bevezetése előtt történt — lásd státusz).
- **Frissesség-javaslat:** szezon-batch (a repo szezononként frissül),
  egyszeri letöltés + lokális cache; meccsenkénti vételezés nem kell.
- **Bizalom domainenként:** **display-only** — a logó nem gate-tényező
  (spec §3.2 display-domain).
- **v1-érettség + kötelező őrök:** v1 adapter ehhez a repóhoz nincs; a v1
  logógyakorlat = FM MegaPack (forrás 25, `docs/logo-audit-2026-11-03.md`).
  Kötelező: PNG-magic (`\x89PNG\r\n\x1a\n`) + IHDR-méret őr (élőben zöld),
  `guard_rows` a repo tree-re (offline decoy-teszt OK: `DecoyError`),
  5 MB méretkorlát, robots-kapu + napi budget + 403/429 breaker,
  URL-kódolás (a repo-útvonalak szóközt/`&`-t/ekezetet tartalmaznak).
- **Kockázatok + mitigáció:**
  - **Licenc HIÁNYZIK (kritikus):** repo meta `license: null`, LICENSE-fájl
    nincs a tree-ben → a klublogók védjegyek/szerzői jogosak, a
    továbbterjesztési jog nem tisztázott → **státusz javasolt**, adoptáció
    előtt felhasználói döntés kell; mitigáció: licenc-audit vagy alternatív
    forrás.
  - Szerkezet-drift: a brief útvonala elavult (lásd adaptációk) → tree-alapú
    útvonal-feloldás, nem hardkódolt path.
  - Zászló-rés + NB I/NB II rés → alternatív forrás kell (Wikimedia jelenleg
    robots-tiltott; FotMob CDN csak jegyzet, lásd lent).
- **Státusz + bizonyíték:** `elvetett` (**felhasználói döntés, 2026-11-12,
  3. pont** — kiváltva a v1 logó-megoldás auditált átvételével; korábbi
  státusz: `javasolt`, licenc null) — spike 2026-11-11, python3.13,
  `/tmp/s1/probe_logos.py`: technikailag zöld (robots „nincs robots” jegyzet,
  PNG 200 + magic + 139×181 IHDR, tree 2304 bejegyzés, repo meta 200), de a
  **licenc hiányzik** (`license: null`, nincs LICENSE) → a brief szerint
  javasolt + kötelező jegyzet. Élő adaptációk (controller-ruling, őrök
  érintetlenek):
  1. python3.13 (controller-előírás, a brief `python3` helyett).
  2. a brief `logos/england/arsenal.png` útvonala **404** (master és main
     ágon is) → README (532 B) + EGY `git/trees/master?recursive=1` API-keres:
     a tényleges útvonal `logos/England - Premier League/Arsenal FC.png`;
     URL-kódolás kell (szóköz, `&`, ékezet — az első futás InvalidURL-lel
     bukott, offline javítás, új kérés nélkül).
  3. robots-ítélet napi cache + minta-cache: az újrafutások nem kérnek újra
     (budget-védelem; a kapu mindkét hostra élőben futott le először).
  4. licenc: EGY repo-meta keres (`license: null`) + tree LICENSE-keresés
     (nincs találat) — a deliverable „LICENSE vagy repo meta egyszer” teljesült.
  5. PNG IHDR-méret ellenőrzés (a README 139×181 állítása élőben igazolva).
  Minta-sha256 (`/tmp/s1/samples/logos-github/`):
  - `arsenal.png`: `a3ad4f044686b6292b8f9b8f2eae65109e3f1bf4b7677ce07be66c9927297c5f`
  - `README.md`: `7016d3f4933e9404a7b8c7af3f62109584d2810803a8164afc3b6d657eb1bfb5`
  - `repo_tree.json`: `aed60f255fc0abf88749c43605ce666ba91c7fa005e71b1c58e5180f0ab844b5`
  - `repo_meta.json`: `09d8aca8b00164e5aa7a8fd7eecb4df2cf8b3d9e00d09bc7dd2b2672fa4ccac1`
  Nem ellenőrzött: teljes demo-kör csapatlefedettség (csak Arsenal-minta +
  könyvtárlista), zászlók (nincs a repóban), GitHub API rate limit élőben.
- **Adoptációs rang:** 7.

### logos-wikimedia — Wikimedia Commons

- **slug / név / base URL / típus:** `logos-wikimedia` / Wikimedia Commons /
  `https://commons.wikimedia.org` / open-data/CDN (média-metaadat API,
  CC/PD-licencelt assetek).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` (tervezett) — **élő bizonyíték NINCS: a robots
  tiltja a tartalmi útvonalat** (2026-11-11, harness UA). v1 előzmény nincs.
- **Domainek + mezőtérkép (tervezett, spike NEM igazolta):** logó/zászló +
  **licenc-metaadat** (a forrás elsődleges értéke).
  - `query.pages[].imageinfo[].url` → asset-URL;
    `extmetadata.LicenseShortName`, `LicenseUrl`, `Artist`, `Credit`,
    `UsageTerms`, `AttributionRequired` → **séma-rés: asset
    licenc/attribution-nyilvántartás** (minden megjelenített tényhez forrás +
    rögzítési idő + licencállapot).
  - `File:` cím → stabil külső ID; csapat-feloldás crosswalkkal.
- **Lefedettség (demo kör):** **NEM ellenőrzött** (robots-tiltás); az
  Arsenal-minta lekérése nem futott le.
- **Identitás:** a `File:` cím forráson belül stabil külső ID (tervezett);
  crosswalk + alias kell (nem igazolt).
- **Kvóta/robots:** **robots TILT** (spike 2026-11-11:
  `(False, 'robots tilt')`): robots.txt 200 (17 713 B), a `User-agent: *`
  szekcióban `Disallow: /w/`, csak szűk kivételek
  (`Allow: /w/api.php?action=mobileview&`, `Allow: /w/load.php?`) → az
  `imageinfo` lekérés tiltott; a `Disallow: /wiki/Special:` a
  Special:FilePath-utat is kizárja. A `/wiki/File:...` leíró oldal engedett —
  de az másik szállítás/szerződés, ezt a spike NEM használja (nincs
  megkerülés). Delay 2 s (robots 4 s); breaker: robots-tilt/403/429 →
  azonnali stop, nincs retry. Spike-budget 10/nap, ebből 2 fogyott
  (2× robots.txt: kapu + bizonyíték-mentés).
- **Frissesség-javaslat (tervezett):** assetenkénti batch + lokális cache
  (a licenc-metaadat ritkán változik).
- **Bizalom domainenként:** **display-only** (tervezett; a licenc-metaadat
  az assetekhez autoritatív lenne, de nem igazolt).
- **v1-érettség + kötelező őrök:** v1 adapter nincs. Kötelező: robots-kapu
  (**élesben kioldott**), `guard_rows` a pages/imageinfo-ra (offline
  decoy-teszt OK: `DecoyError`), `LicenseShortName` kötelező mező (hiány →
  javasolt + jegyzet), napi budget + 403/429 breaker + 5 MB méretkorlát.
- **Kockázatok + mitigáció:** robots-tiltás → az API-út generikus UA-val nem
  használható; mitigáció: felhasználói döntés — a robots-engedett HTML
  leíró oldal külön spike-célpont lehet (más szerződés), vagy alternatív
  logóforrás; a CC/PD licenc-metaadat értékes lenne az attributionhöz, de
  jelenleg nem elérhető.
- **Státusz + bizonyíték:** `elvetett` (**robots-tiltás a spike szintjén** —
  NEM adatminőségi/licenc-elvetés; a státusz engedélyezett újra-probe-val
  felülvizsgálható) — spike 2026-11-11, python3.13, `/tmp/s1/probe_logos.py`:
  robots.txt 200, majd `/w/api.php` → `(False, 'robots tilt')` →
  **StopSpike, nincs retry, nincs tartalmi vételezés**; API-minta NEM
  mentődött (nincs API-sha256). Offline szabályelemzés a mentett robots.txt-n:
  a `Disallow: /w/` illeszkedik, az imageinfo nincs az Allow-kivételekben.
  Élő adaptációk (controller-ruling, őrök érintetlenek):
  1. python3.13 (controller-előírás, a brief `python3` helyett).
  2. `redirects=1` a tervezett API-URL-hez (címnormalizálás) — a robots-tiltás
     miatt NEM futott le.
  3. a robots.txt bizonyíték külön mentve (`robots.txt` minta, 2. robots-keres,
     nem tartalmi keres).
  Minta-sha256 (`/tmp/s1/samples/logos-wikimedia/`):
  - `robots.txt`: `89c0b6b55d04a401346110c36c8cd8663a3443f4b69cb2909a26ff79ac8f3884`
  Nem ellenőrzött: MINDEN élő tartalmi állítás (cím, imageinfo, extmetadata
  licencmezők, lefedettség) — a robots-tiltás miatt csak tervezett szinten.
- **Adoptációs rang:** 7 (tervezett; a tiltás feloldásáig függőben).

### openfoot — OpenFoot API

- **slug / név / base URL / típus:** `openfoot` / OpenFoot API /
  `https://openfootapi.com/v1` / API (Bearer-kulcsos JSON API; a
  `https://api.openfoot.com` NEM API — v1 adapter-docstring).
- **Hozzáférés:** Bearer kulcs (Keychain) — a migrációt a **controller
  elvégezte** (bejegyzés: account `football-app-v2`, service
  `openfoot-api-key`, létrehozva 2026-11-11T13:54:04Z; `find` metaadat
  exit 0). A spike a kulcsot CSAK futásidőben olvassa
  (`security find-generic-password … -w`); az érték nem naplózott,
  fájlba nem került (probe kulcs-scan + független kulcs-scan minden
  mintán: OK). Élő auth igazolt: `/v1/account/quota` 200,
  `meta.access.authenticated=true`, plan `starter`. A korábbi három
  blokkolt Keychain-írási kísérlet (2026-11-11, engedélyrendszer)
  lezárult; a spike Keychainet NEM írt, v1 `.env`-et NEM olvasott.
- **Szállítás:** `rust-http` — **élő bizonyíték: 2026-11-11, 5 kérés
  (4×200 + 1×403 plan-gate)**, sima REST/JSON (urllib, Bearer header),
  bot-kihívás nélkül. A válaszok `meta.environment=beta` — béta
  környezet, kockázatként kezelendő. Docs-olvasat (2026-11-11, nem
  API-hívás, kulcs nélkül): homepage + nyilvános `openapi.json`
  (OpenAPI 3.1), bearerAuth séma, `of_live_` kulcsprefix; a v1 adapter
  plain HTTP-vel futott (`engine.get_json`, Bearer header).
- **Domainek + mezőtérkép (élő mintákkal igazolva 2026-11-11; a docs/v1
  térképhez képesti eltérések jelölve):** fixture/kickoff + lezárt FT
  eredmény (a megnevezett domain).
  - `id` (determinisztikus slug-ID) → külső meccs-ID (identity
    crosswalk); élő példák:
    `match_fmb_bra2_2026_20261111_londrina_nautico`,
    `match_fd_usa1_2026_08112026_austin_fc_portland_timbers`,
    `match_fmb_hun1_2026-27_20261108_nyiregyhaza_spartacus_fc_gyori_eto`
    — az ID-prefix upstream-et jelez (`fmb`, `fd`) → heterogén upstream.
  - `competitionId` → liga külső ID; **élő ID-k eltérnek a docs/v1
    példájától:** `comp_laliga_es`, `comp_serie_a_it`, `comp_ligue_1_fr`
    (a docs `_esp/_ita/_fra` végződést írt); `comp_nb_1_hun` változatlan
    (v1 fix-log allowlist). A probe TOP5-konstansa a régi ID-ket
    használta → élő adaptációs jegyzékben rögzítve; újrahívás NEM
    történt (breaker után nincs kérés).
  - `season`: élőben kétféle formátum: `2026` (naptári éves liga) és
    `2026/27` (európai szezon) — az adapter mindkettőt kezelje.
  - `kickoffAt` (date-time, UTC; élőben `2026-11-11T22:00:00.000Z` és
    `2026-11-08T12:15:00Z` — ezredmásodperc nélküli alak is) →
    `core.match.kickoff_at` (timestamptz, UTC); `kickoffTimeKnown`
    minden élő soron `true` (a `false` ág nem tesztelt; date_only
    kezelendő, nem nullázható időponttal).
  - `status` enum (`scheduled/live/finished/postponed/cancelled/unknown`)
    → státusz; `score.home/away` (int vagy null) →
    `core.match_result.home_ft/away_ft` (int, null amíg nincs eredmény);
    élőben 100/100 `finished` soron mindkét pontszám megvan;
    **élő/folytatódó meccs részeredménye NEM írható kanonikus eredményként**
    (v1 adapter-őr: csak `finished` + mindkét pontszám).
  - `homeTeam`/`awayTeam` `{id, name, shortName, code, logoUrl, country,
    competitionId, sourceRefs}` → `core.match.home/away` (text, not null,
    alias kell); élő adatminőség-jegyzet: `Györi ETO` (ékezet nélkül) —
    alias/normalizálás kell. `venue` (string|null) → **séma-rés:
    `core.match.stadium` — élőben 100/100 null**, ebből a feedből nem
    tölthető; `round` → `round_label`; `minute/addedTime/period`
    (1H/HT/2H/ET/PEN/FT) → élő állapot (display).
  - `sourceRefs[{source, id}]` → upstream azonosítók (FotMob is) →
    crosswalk-híd; **élőben 43/100 soron null/hiányzó** (a NB I-meccsen
    megvan: fotmob) → a crosswalk csak részben építhető ebből a feedből
    (v1 `_resolve`: csak már mappingben lévő FotMob-ID).
  - **Séma-rés: lineups/xG plan-gate — ÉLŐBEN IGAZOLT:**
    `/v1/matches/{id}/lineups` → **403 `plan_upgrade_required`**
    („The starter plan includes fixtures, scores, and league standings.
    Upgrade to Developer ($14/mo) to unlock starting lineups and
    formations.”) — account-szintű plan-gate, nem meccs-függő. Breaker:
    nincs retry, további kérés nem történt. **xG NEM élő-tesztelt**
    (breaker); a docs szerint `/v1/matches/{id}/events` és
    `/v1/analytics/xg` explicit Developer-plan → séma-rés, nem hiba
    (terv-előírás).
  - Nem mért további endpointok (docs): standings, scorers, squad, h2h,
    search, odds, live/stream, webhooks — a spike-domainen kívül.
- **Lefedettség (demo kör, élő mérés 2026-11-11):** mai nap
  (`date=2026-11-11&limit=100`): **1 meccs** (`total_count=1`, Serie B
  BRA, scheduled 22:00 UTC) — nem üres, guard zöld; top-5 és NB I az
  aznapi feedben nem szerepel. Lezárt nap
  (`date=2026-11-08&status=finished&limit=100`): **100 sor /
  `total_count=257`**, 48 verseny; **NB I megvan** (1 meccs: Nyíregyháza
  Spartacus FC–Győri ETO 1–1, finished); top-5 élő ID-kkel az első 100
  sorban: La Liga + Serie A jelen (PL/Bundesliga/Ligue 1 az első oldalon
  nincs — csak az első 100 sor mérve, nem a teljes nap). `meta.scope`:
  `default_competitions`, `partial=true`, 120 verseny keresve — a
  „120 verseny / 94 ország” docs-állítás versenylistája élőben
  megjelent, a tényleges meccs-lefedettség napfüggő. NB II nem jelent
  meg a mintán (nem ellenőrzött).
- **Identitás:** determinisztikus forrás-ID-k élőben igazolva (slug-ID-k,
  upstream-prefixszel); `sourceRefs` közvetlen upstream crosswalk
  (FotMob) a NB I-példán megvan, de 43/100 soron hiányzik.
  **v1-incidens tanulsága (fix-log 2026-11-07):**
  ismeretlen verseny SOHA nem kaphat alapértelmezett ligát
  (`DEFAULT_TOURNAMENT_ID` törölve), csapatfeloldás csak pontos, egyedi
  egyezéssel (részstring tilos), forrás-ID nem kerül kanonikus bigint mezőbe.
- **Kvóta/robots:** robots **élőben engedett** (200, 401 B,
  `(True, 'robots engedett')` — a várt 404 helyett valódi robots.txt).
  Kvóta élőben (`/v1/account/quota`, 2026-11-11T13:56:13Z): plan
  **starter**, period **2026-11**, quota **5000/hó (≈166/nap)**,
  **used 326, remaining 4674** — a v1-gyel közös kulcs; a v1 fix-log
  459 kérést rögzített 2026-11-07-én, a mért 326 ettől eltér (a v1
  oldali számláló értelmezése nem ellenőrzött). A kvóta-ellenőrzés
  UTÁNI 3 API-kéréssel a server-side remaining becslés szerint 4671
  (nem mért — egy quota-hívás engedélyezett). Havi helyi számláló
  `/tmp/s1/openfoot_month.json`: **5/5000** (az S2 worker veszi át; a
  v1 fogyasztását nem látja — a server-side remaining az irányadó).
  Delay 2 s; breaker: 403/429 → azonnali stop, nincs retry (élőben ki
  is vált a lineups-403-nál). Spike-budget 10/nap, ebből **5 fogyott**
  (robots + quota + 2 matches + lineups).
- **Frissesség-javaslat (élő mérés alapján):** napi day-feed
  (`/v1/matches?date=...&limit=100`), lezárt eredmények meccs utáni batch.
  A 257-es `total_count` > 100 limit → lapozás vagy versenyenkénti szűrés
  kell. **`next_cursor` élőben MEGJELENIK starter kulccsal is**
  (`b2Zmc2V0OjEwMA`; a docs Developer-only-t írt) — használhatósága NEM
  tesztelt (breaker után nincs kérés); adapter-szelet döntési pontja.
- **Bizalom domainenként (spec §3.2):** fixture/kickoff és eredmény FT
  **közepes, keresztforrás** (aggregator/API osztály; keresztellenőrzés
  football-data ellenében — a spike-ban nem futott); lineups/xG
  display-only, nem gate-tényező (plan-gated).
- **v1-érettség + kötelező őrök:** v1 adapter létezik
  (`scrappers/web/sources/openfoot.py`, fail-closed, 188 sor), de
  **`enabled = false`** a 2026-11-07-i incidens óta (81 hamis kanonikus
  meccs + 2760 registry-duplikátum, rollback archiválva). Kötelező:
  1. read-only fegyelem — az incidens miatt csak olvasó spike/adapter,
     kanonikus írás nincs (v1 fix-log 7. pont szabályai);
  2. üres `data` decoy-guard (`guard_rows`, offline teszt OK: `DecoyError`);
  3. dátum-guard (kickoffAt UTC nap = kért nap);
  4. csak `finished` + teljes pontszám írható eredményként (élő rész nem);
  5. nincs default liga, nincs részstringes feloldás (v1 incidens-őrök);
  6. robots-kapu + napi budget + havi számláló + 403/429 breaker + 5 MB
     méretkorlát + kulcs-scan a mintákon (kulcs soha nem kerül fájlba/logba).
- **Kockázatok + mitigáció:**
  - **Béta környezet + ID-drift:** `meta.environment=beta` minden
    válaszban; a docs→élő verseny-ID eltérés (`_es/_it/_fr`) már
    megmutatkozott → fail-closed adapter, séma- és ID-ellenőrzés,
    allowlist frissítése az adapter-szeletben.
  - Incidens-ismétlődés → read-only + fail-closed őrök (fent), újraengedélyezés
    csak külön döntéssel (v1 fix-log 8. pont).
  - Plan-gate: lineups 403 **élőben igazolt** (`plan_upgrade_required`);
    xG/events docs-alapú séma-rés; a Developer-tier ($14/hó)
    felhasználói döntés.
  - Kvóta-osztozás a v1-gyel → havi számláló + `/v1/account/quota`
    ellenőrzés az adapter-szeletben is (mért remaining 4674 a
    kvóta-híváskor).
  - Cursor-lapozás megjelenik, de használhatósága nem tesztelt →
    100 meccs/kérés plafon; nagy napokon versenyenként szűrt
    (`competition=`) kérés kell (adapter-szelet).
  - Ritka day-feed (élőben 1 meccs) → szezonális üresség kezelendő
    (decoy-guard + üresség-jelölés, nem zöld státusz).
- **Státusz + bizonyíték:** **`igazolt`** — az élő spike lefutott
  2026-11-11-én (a controller által migrált Keychain-kulccsal,
  python3.13): robots engedett, auth 200, fixture + FT eredmény-domain
  élő mintákkal igazolva, guardok zöldek (dátum-guard 0 eltérés mindkét
  napon, decoy-guard nem váltott ki, kulcs-scan OK), a lineups plan-gate
  403 séma-résként rögzítve (terv-előírás szerint nem hiba). Minta-sha256
  (`/tmp/s1/samples/openfoot/`):
  - `quota.json`:
    `698871ceb0d49cf0ce276f2ce62b3669ee1f549ffaf6262836bbc24971f98f14`
  - `matches_2026-11-11.json`:
    `79fd425342e80917d49bea255dca02848419f040e8cfd8444df472debb23e100`
  - `matches_finished_2026-11-08.json`:
    `0476decd7ee922ed92829124357a425069457058fa78f8c74873d9507945bfba`
  - `lineups_match_fmb_bra2_2026_20261111_londrina_nautico_403.json`:
    `82e484521dad62c249373e9f78d4c592dc0f62aa603e9b644ab796537b885302`
  Mért vs tervezett: **5 kérés** (tervezett max 6: robots, quota, 2 nap,
  lineups, xG — xG a breaker miatt elmaradt); napi **5/10**, havi helyi
  **5/5000**, server-side remaining **4674** a kvóta-híváskor.
  Dekoy-teszt offline OK (`DecoyError` üres listára); probe
  (`/tmp/s1/probe_openfoot.py`, python3.13) változtatás nélkül futott.
  v1-doc: `docs/fix-log-2026-11-07-openfoot-hibas-import.md`,
  `scrappers/web/sources/openfoot.py`. Nem ellenőrzött: xG/events
  plan-gate élőben, cursor-lapozás használhatósága, NB II lefedettség,
  `kickoffTimeKnown=false` ág, keresztforrás-egyeztetés, a v1 oldali
  kvóta-számláló eltérése (459 vs 326). Előzmény: a Keychain-írást
  2026-11-11-én háromszor blokkolta az engedélyrendszer (Task 9 +
  resume); a migrációt végül a controller végezte el a fő sessionben.
- **Adoptációs rang:** 8.

### livescore-mcp — Livescore MCP

- **slug / név / base URL / típus:** `livescore-mcp` / Livescore MCP /
  `https://livescoremcp.com` / API (JSON-RPC 2.0 POST `tools/call` az `/mcp`
  MCP-végponton; a szolgáltató AI-ügynököknek szánt, intended-use API-felület).
- **Hozzáférés:** none — nincs kulcs, nincs auth (a v1 adapter kulcs nélkül
  hívta); titok/Keychain nem kell.
- **Szállítás:** `rust-http` (tervezett) — **élő bizonyíték NINCS: a robots
  tiltja az API útvonalát, tartalmi hívás nem történt** (2026-11-11).
  v1 előzmény: plain JSON POST (`engine.post_json`, sima JSON válasz);
  a korábbi GET-próba 0 bájtos event-streamet adott → POST kötelező
  (v1 `scrappers/web/sources/livescore.py` docstring).
- **Domainek + mezőtérkép (tervezett, v1 adapter; spike NEM igazolta):**
  napi fixture/eredmény, lineups, incidents, csapat- és játékos-stats.
  - Tool-nevek: a brief `get_matches_by_date` neve tipp; a v1 adapter valódi
    nevei **`get_day_fixtures`** (date, format=text, timezone, limit, country)
    és **`get_match`** (id, format, timezone, h2h). A `tools/list` NEM futott
    le (robots-tilt) → az élő tool-készlet nem igazolt.
  - Nap-feed text: `match {id}` → külső meccs-ID (identity crosswalk);
    `Matches on DD/MM/YYYY` + `HH:MM` előtag → `core.match.kickoff_at`
    (timestamptz, UTC; a forrás-időzóna Europe/Budapest → konverzió kell);
    „Home X - Y Away” → `core.match_result.home_ft/away_ft` (int, null amíg
    nincs eredmény); státusz-előtagok (FT/AET/PEN/HT/1h/2h/`live 77'`/
    Postp/Canc/Abd) → státusz + perc (v1 élő-perc őr: a nyers `live 77'`
    státuszba nem írható, a perc külön mező).
  - `get_match` JSON: `lineups` + `commentaries.teams/substitutes` →
    **séma-rés: `match_lineups`**; `events` + `commentaries.substitutions` →
    **séma-rés: `match_incidents`**; `commentaries.stats`
    (localteam/visitorteam) → **séma-rés: `match_team_stats`**;
    `commentaries.playerstats` (goals, pen_won, rating, yellowcards,
    redcards, minutes_played) → **séma-rés: játékos-stats** (v1
    `match_player_stats` minta); venue/referee → **séma-rés:
    `core.match.stadium/referee`** (azonos rés, mint TheSportsDB/FotMob).
  - v1 quirkök: a `format=json` kimenet **60 000 karakternél hard-kapolt**
    (érvénytelen JSON → toleráns részfa-kinyerítés kell, nem tippelés);
    a `localteamshapefixtures`/`visitorteamshapefixtures` belsejében lévő
    `injuries` HISTORIKUS forma-adat → SOHA nem írható hiányzásként;
    a nap-feed alapból az első 500 meccset adja (teljes napon 1470+) →
    ország-szeletelt kérések kellenek (v1 gyakorlat).
- **Lefedettség (demo kör):** NEM ellenőrzött (robots-tilt, 0 tartalmi hívás).
  v1 szerint globális nap-feed, angol nevek, ország-aliasok (pl. Czechia →
  Czech Republic); top-5 + NB I/NB II élő lefedettség nem mért.
- **Identitás:** a livescore meccs-ID forráson belül stabil (v1
  `external_id_map` gyakorlat); crosswalk + alias kell; csapatfeloldás csak
  pontos név + azonos nap + egyetlen jelölt, egyébként review-sor (v1
  fail-closed minta), fuzzy összevonás tilos.
- **Kvóta/robots:** **robots TILT** (spike 2026-11-11:
  `(False, 'robots tilt')`, `/mcp` útvonal): robots.txt 200 (137 B),
  `User-agent: *` + `Allow: /`, de explicit `Disallow: /mcp`, `/sse`,
  `/message`, `/health` → az API útvonal tiltott, a honlap engedett.
  v1 kvóta: **100/nap, delay 6 s + jitter 3 s** (v1 Policy: 10 eszköz/perc,
  burst 30). Breaker: robots-tilt/403/429 → azonnali stop, nincs retry
  (**élesben kioldott: robots-tilt**). Spike-budget 10/nap, ebből **2
  fogyott** (2× robots.txt: kapu + bizonyíték-mentés); tartalmi hívás 0.
  **A v1 `robots_override=True` anti-pattern NEM kerül át** (spec
  tiltólista): az intended-use API robots-tiltása **felhasználói döntési
  pont**, nem csendes megkerülés — a v1 egyszeri, dokumentált engedménye
  (`engine.POLICIES["livescore"]`) a v2-ben nem érvényes alap.
- **Frissesség-javaslat (tervezett):** nap-feed napi több körben; meccs-detail
  (lineups/incidents/stats) meccs utáni batch; lineups kickoff előtt ~1 óra.
- **Bizalom domainenként (tervezett, spec §3.2):** minden domain **közepes,
  keresztforrás** (aggregator/API osztály; a spec mátrixa explicit sorolja);
  eredmény keresztellenőrzés football-data ellenében; lineups/stats
  display-only, nem gate-tényező.
- **v1-érettség + kötelező őrök:** v1 adapter éles
  (`scrappers/web/sources/livescore.py`, 582 sor, fail-closed linkeléssel).
  Kötelező:
  1. POST-only JSON-RPC (GET → 0 bájtos event-stream decoy, v1 mérés);
  2. üres `content` decoy-guard (`guard_rows`, offline teszt OK: üres
     content, hiányzó result, üres tools, üres day-text egyaránt
     `DecoyError`); JSON-RPC error envelope → `DecoyError`, nem csendes 0 sor;
  3. 60k JSON-kap toleráns részfa-kinyerítése (hibás részfa → None, nem tipp);
  4. élő-perc státusz-őr (`live 77'` → státusz `live` + perc külön mező);
  5. forma-fixture `injuries` SOHA nem hiányzás (historikus adat);
  6. dátum-guard + 500-meccs plafon kezelése (ország-szeletelés);
  7. robots-kapu + napi budget + 403/429 breaker + 5 MB méretkorlát +
     becsületes UA.
- **Kockázatok + mitigáció:** robots-tilt az API útvonalon → **döntési pont**
  (a szolgáltató intended-use hirdetése és a robots.txt ellentmond; a
  felhasználó dönthet: kapcsolatfelvétel a szolgáltatóval / engedélyezett
  újra-probe / elvetés — megkerülés nélkül); 60k JSON-kap → text formátum +
  toleráns részfa; 500-meccs feed-plafon → ország-szeletelt kérések a kvótán
  belül; serving-drift → spike újraellenőrzés az adapter-szelet előtt.
- **Státusz + bizonyíték:** `elvetett` (**felhasználói döntés, 2026-11-12,
  2. pont** — robots-tilt az API útvonalon, megkerülés nélkül; a fedettség
  FotMob/ESPN/OpenFoot; korábbi státusz: `javasolt` + döntési pont;
  újrabírálás szolgáltatói engedéllyel) — spike
  2026-11-11, python3.13: robots-kapu `(False, 'robots tilt')` (`/mcp`);
  a `/tmp/s1/probe_livescore.py` fail-closed **StopSpike-pal megállt a
  kapun** — `tools/list` és `tools/call` NEM futott, tartalmi minta NEM
  mentődött (nincs tartalmi sha256). Offline dekoy-teszt OK (5 eset +
  SSE-envelope parse, hálózati kérés nélkül). Élő adaptációk
  (controller-ruling, őrök érintetlenek):
  1. python3.13 (controller-előírás, a brief `python3` helyett).
  2. tool-név: a brief `get_matches_by_date` tippje helyett a v1 valódi
     `get_day_fixtures` (format=text) — a probe a `tools/list` nevéből
     választana, ha a kapu engedne.
  3. robots-ítélet napi cache (`/tmp/s1/robots_cache.json`): az élő kapu
     egyszer futott, a probe-újrafutás nem kér újra (budget-védelem).
  4. a JSON-RPC POST-ra minden harness-őr kézzel alkalmazva (budget_left
     pre-check, log_budget minden kísérletre hibára is, 6 s delay, 5 MB,
     30 s, 403/429 breaker, becsületes UA) — a harness `fetch` GET-only.
  Minta-sha256 (`/tmp/s1/samples/livescore-mcp/`):
  - `robots.txt`: `b91e49166889bd9859b7d8e897e1e27dd71a878127f373c6298d2b037be3cd3d`
  Nem ellenőrzött: MINDEN élő tartalmi állítás (tool-készlet, mezőtérkép,
  lefedettség, 60k-kap, 500-as plafon, státusz-alakok) — a robots-tilt
  miatt csak v1-tervezett szinten szerepelnek.
- **Adoptációs rang:** 9 (a döntési pont feloldásáig függőben).

### pl-official — Premier League hivatalos hiányzólisták

- **slug / név / base URL / típus:** `pl-official` / Premier League hivatalos
  (premierleague.com) / `https://www.premierleague.com` / hivatalos
  (ligaoldal; cikk-alapú sérülés- és eltiltáslisták, **PL only**).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** **hibrid, spike-döntés (2026-11-11):**
  - **Sérüléslista** (`/en/news/4450606`): plain HTTP 200 (91 464 B), de a
    cikk-törzs KLIENS-RENDERELT — 0 `<h3>`, 0 v1-parser sor, nincs
    `__NEXT_DATA__`, a „Player” találatok csak navigációs linkek; a tábla
    fejlécei (Player/Injury/Latest/Details) nem szerepelnek szöveg-tokenként
    → **decoy-200 a hiányzó-domainre, `obscura` kell** (v1 előzmény:
    DynamicSession, valódi böngésző — `scrappers/web/sources/premierleague.py`
    docstring). A v2 Obscura-integráció még nincs megépítve (FotMob-jegyzet).
  - **Eltiltáslista** (`/en/news/4425344`): `rust-http` — 200 (96 442 B),
    két valódi SSR `<table>`: 3 eltiltott játékos + 13 „close to a one-match
    ban” sor, a v1 token-parser a mentett mintán zöld.
- **Domainek + mezőtérkép:** hiányzók (sérülés + eltiltás) — **séma-rés:
  `player_availability`** (azonos rés, mint FotMob/Transfermarkt).
  - Sérüléslista (v1 minta, élőben NEM igazolt — decoy): „Klub | Player |
    Injury | Latest | Játékos | Sérülés | Details” token-struktúra →
    játékosnév (text, not null), klub, ok (text), státusz `injured`.
  - Eltiltáslista (élőben igazolt): Player / Club (3 karakteres kód) /
    Pos. / Suspended for (GW-tartomány) → státusz `suspended`, ok
    „eltiltás (GW11-13)” alak; `expected_return` **nullázható** (a GW-tartomány
    nem dátum); élő minta: Max Dowman (ARS, GW11), Igor Thiago (BRE,
    GW11-13), Jean-Philippe Mateta (CRY, GW10-12).
  - „Players close to a one-match ban” tábla (élőben 13 sor, mind 4 sárga
    lap) → display-only kockázati jelzés; a v1 parser ezt NEM olvassa
    (a „Last updated” tokennél megáll) — adapter-szelet döntési pontja.
  - Nincs timestamp-mező; a cikk „Last updated” szövege a frissesség
    (élőben: sérülés 2026-11-09 17:50 GMT, eltiltás 2026-11-08) → manuális
    cikk-frissítés, nem élő feed.
- **Lefedettség (demo kör):** **PL only** — a többi top-5 liga és
  NB I/NB II HIÁNYZIK → rés-jelölés: FotMob (`player_availability`) +
  Transfermarkt (GB1 és társai). Élőben az eltiltás-domain igazolt
  (3+13 sor); a sérülés-domain plain HTTP-vel NEM igazolt (decoy).
- **Identitás:** nincs külső ID; klub-feloldás a v1 3 karakteres kódtáblájával
  (CLUB_CODES, 26 bejegyzés — a 2627-es PL-készlet is szerepel: HUL, IPS,
  SUN, COV, LEE), játékosnév-egyezés → crosswalk + `team_aliases`,
  fuzzy összevonás tilos. **Név-normalizálás őr:** a mintán NBSP szerepel
  („Ethan\xa0Ampadu”) → whitespace-normalizálás kötelező. A v1 gyakorlat
  szerint a bejegyzés a csapat KÖVETKEZŐ meccséhez kötött
  (`resolved_by=club_name`, confidence 1.0/0.8); nincs meccs a listán →
  a bejegyzés kimarad, nem találgatunk helyettesítőt.
- **Kvóta/robots:** robots engedett (spike 2026-11-11:
  `(True, 'robots engedett')`, `/en/news/4450606` útvonal, robots.txt 638 B).
  Kvóta **30/nap, delay 20+25 s** (spec §4.2 / v1 gyakorlat); breaker:
  403/429 → azonnali stop, nincs retry. Spike-budget 10/nap, ebből
  **3 fogyott** (robots + sérülésoldal + eltiltásoldal).
- **Frissesség-javaslat:** cikk-alapú lista → 48 óránkénti batch vagy
  meccsnap-ellenőrzés (a „Last updated” szöveg őre); a hiányzó-bejegyzés
  a csapat következő meccséhez kötött (v1 gyakorlat).
- **Bizalom domainenként:** **hivatalos forrás → magas bizalom a
  hiányzó-tényre** (a liga saját listája); a spec §3.2 mátrixa szerint a
  kezdő/hiányzó domain ugyanakkor **display-only, nem gate-tényező** —
  keresztellenőrzés FotMob/Transfermarkt hiányzólistájával, ahol átfedés van.
- **v1-érettség + kötelező őrök:** v1 adapter éles
  (`scrappers/web/sources/premierleague.py`, DynamicSession szállítással).
  Kötelező:
  1. decoy-guard fail-closed (`guard_rows`, offline teszt OK: üres lista ÉS
     kliens-shell HTML egyaránt `DecoyError`);
  2. token-struktúra őr (a v1 parser a „Player/Injury/Latest” fejléc-triplát
     és a „Details” oszlopot várja — szerkezetváltásnál 0 sor, nem szemét);
  3. név-szűrő (All Clubs/Shop/About/Related kiszűrése, v1 minta) + NBSP/
     whitespace-normalizálás;
  4. klubkód-allowlist (ismeretlen kód → nem írunk, v1 CLUB_CODES);
  5. robots-kapu + napi budget + 403/429 breaker + 5 MB méretkorlát.
- **Kockázatok + mitigáció:**
  - A sérüléslista kliens-renderelt → **v2 Obscura-integráció kell**
    (még nincs megépítve); addig csak az eltiltás-domain használható
    rust-http-vel. Mitigáció: FotMob/Transfermarkt keresztforrás.
  - A cikk-ID (4450606/4425344) a kulcs → cikk-csere/URL-drift esetén a
    tartalmi őr fail-closed megállít (nincs csendes 0 sor).
  - Manuális cikk-frissítés → késleltetett adat; a „Last updated” szöveg
    rögzítendő provenance-ként.
  - PL only → a demo-kör többi ligájára nem megoldás (rés-jelölés fent).
- **Státusz + bizonyíték:** `javasolt` — **a sérülésoldal decoy-200-ja miatt
  a brief szerint javasolt + jegyzet** (nem elvetett: a robots enged, az
  eltiltás-domain élőben igazolt, a sérülés-domain obscura-szállítással
  járható — v1 DynamicSession-előzmény). Spike 2026-11-11, python3.13,
  `/tmp/s1/probe_pl_official.py`: robots 200 engedett; sérülésoldal 200
  (91 464 B) → 0 h3 / 0 v1-sor → DecoyError-jelzés; eltiltásoldal 200
  (96 442 B) → 3 eltiltott + 13 ban-risk sor, guard zöld; decoy-teszt
  offline OK (8 eset, mindkét probe közös futtatásban). Élő adaptációk
  (controller-ruling, őrök érintetlenek):
  1. URL: a brief `/news/4450606` alakja helyett a v1 `/en/news/4450606`
     (+ a v1 második oldala, `/en/news/4425344` eltiltások — a 20+25 s
     késleltetési minta ehhez a két oldalhoz tartozik).
  2. parser: a brief h3-regexe helyett/mellett a v1 token-parser
     (`_text_from_html` + `_parse_injuries`/`_parse_suspensions` mintája)
     — a h3-blokk mindkét oldalon 0, az eltiltás-tábla SSR, a sérülés nem.
  3. fejléc-drift: a v1 „Posn” fejléce élőben „Pos.” — a pozicionális
     token-parser ezt tolerálja (élőben igazolva).
  4. robots-ítélet napi cache (`/tmp/s1/robots_cache.json`,
     probe_livescore mintája); `__main__`-guard (Transfermarkt-tanulság).
  5. python3.13 (controller-előírás, a brief `python3` helyett).
  Minta-sha256 (`/tmp/s1/samples/pl-official/`):
  - `injuries_4450606.html`: `a4e2e5dcbe7f98f0a88d0262d59329fabbc6eea9fa4ca5a89ccbe1ea2e44de40`
  - `suspensions_4425344.html`: `eaafa8444c44642a103049524c5844307e7155ed0844e56704dc6ead1f147851`
  - `probe_summary.json`: `22b5006d2e326297fa6fa7544e87e99868fa6d2b51ecd3a4fc2605aeb8831ed9`
  Nem ellenőrzött: a sérülés-domain élő tartalma (obscura kell), a cikk-ID-k
  stabilitása, a ban-risk tábla adapter-szintű feldolgozása, a „Last updated”
  frissesség-jelzésének pontossága.
- **Adoptációs rang:** 10.

### espn — ESPN soccer scoreboard (újraértékelés)

- **slug / név / base URL / típus:** `espn` / ESPN soccer scoreboard /
  `https://www.espn.com` / aggregator (SSR HTML + beágyazott
  `window['__espnfitt__']` JSON; napi scoreboard, globális).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, 3 kérés mind
  200 (robots 3 223 B + két scoreboard ~1,15 MB), bot-kihívás nélkül, sima
  urllib; v1 előzménymérés: `docs/espn-source-2026-11-09.md` (SSR, egy
  kérés = egy nap, 60 meccs/nap mérve 2026-11-08-ra).
- **Domainek + mezőtérkép:** napi fixture/kickoff (pontos UTC), eredmény
  FT, státusz (AET/pen jelzéssel), venue.
  - `gmsByLeague[].evts[].id` → külső meccs-ID (ESPN gameId, identity
    crosswalk); `gmsByLeague[].league.name/slug` → liga (élőben:
    „English Premier League” / `eng.1`).
  - `evts[].date` (ISO, **UTC — a forrás explicit Z-jelöléssel ad**, élőben
    `2026-11-08T14:00Z`) → `core.match.kickoff_at` (timestamptz, UTC,
    konverzió nem kell); az ESPN a napot keleti parti idő szerint
    csoportosítja → v1 gyakorlat: 2 kérés/nap (kért nap + következő),
    Budapest-napra szűrés.
  - `competitors[].displayName` (hivatalos nevek) → `core.match.home/away`
    (text, not null, **alias kell** — lásd linkelhetőség); `isHome` jelöli
    az oldalakat; `score` (int|null) → `core.match_result.home_ft/away_ft`
    (int, null amíg nincs eredmény).
  - `status.description/state` → státusz; **AET/pen RÉSSZEL**: a
    „Final Score - After Extra Time” / „After Penalties” állás 120 perces
    illetve 11-esekkel eldöntött → 90 perces eredményként NEM írható
    (v1 `skipped_extra_time`).
  - `vnue.fullName` → **séma-rés: `core.match.stadium`** (azonos rés, mint
    TheSportsDB/FotMob; a spike-ban a mező jelenléte a JSON-ban nem mért
    külön, v1 mezőtérkép).
  - **Séma-rés: ET/pens explicit forrás** — az ESPN AET/pen státuszt ad, de
    a 90 perces állást nem bontja ki; ET/pens csak explicit forrásból
    (spec: worldfootball pso observed, football-data 90’).
- **Lefedettség (demo kör, élő mérés):** 2026-11-11 (válogatott szünet):
  **75 esemény / 16 liga, PL 0** (női BL, EFL Trophy, barátságosok, NCAA,
  SAFF stb.); 2026-11-08 (fordulonap): **58 esemény / 16 liga, PL 5**,
  top-5 jelen (PL, Bundesliga, Serie A, LaLiga, Ligue 1), **English FA Cup
  is szerepel** → Task 12 FA Kupa-rés probe bemenet. NB I/NB II a mintákon
  nem szerepel (nem ellenőrzött).
- **LINKELHETŐSÉG (újraértékelés-deliverable, offline exact-normalized join
  a `footballdata` 2627_E0.csv mintára, 100 sor / 20 csapat):**
  - **PL event-szintű egyezés: 2/5 = 40%** (mindkét csapatnév pontosan
    egyezik normalizálás után: casefold + ékezet-telenítés +
    punctuáció-eltávolítás, alias NINCS).
  - **PL név-szintű egyezés: 6/10 = 60%**; a 4 nem illeszkedő név hivatalos
    alak: „AFC Bournemouth” (E0: Bournemouth), „Brighton & Hove Albion”
    (E0: Brighton), „Ipswich Town” (E0: Ipswich), „Manchester United”
    (E0: Man United) → **4 bejegyzéses alias-réteggel 5/5 = 100%** (offline
    plafon, alias ADAT nem kód — fuzzy összevonás tilos).
  - Összliga név-átfedés: 7/110 egyedi ESPN-név (6,4%) szerepel az E0
    PL-készletben — konzisztens a v1 globális mérésével (11% linkelhetőség,
    85% ismeretlen név; a v1 nevezője a teljes kanonikus modell + minden
    ESPN-esemény volt, nem a PL-only szerep).
  - **Eredmény-keresztellenőrzés (bonus, offline): az 5 PL-meccs E0-beli
    eredménye 5/5 egyezik** (Brighton 2-0 Brentford, Crystal Palace 1-3
    Liverpool, Ipswich 1-2 Bournemouth, Sunderland 1-0 Chelsea,
    Man United 4-4 Aston Villa) → az ESPN PL-eredménye pontos keresztforrás.
  - **Következtetés:** a v1 11%-os linkelhetőség a GLOBÁLIS nap-feed
    szerepre szólt (ott a forrás jogosan kikapcsolt); **PL-only
    keresztforrás-szerepben a linkelhetőség nem blokkoló** (40% exact,
    100% 4 alias-sal), és az eredmény-pontosság élőben igazolt.
- **AET/pen rés (élőben igazolva):** a 2026-11-08-i mintán **3 FA Cup
  esemény** hosszabbítás/11-esekkel zárult (Stafford Rangers 3-3 Grimsby
  Town „After Penalties”; Cray Wanderers 2-3 Stockport County és Oldham
  Athletic 2-1 Sheffield Wednesday „After Extra Time”) → a 90 perces
  konvenció résként jelölve: ilyenkor eredményt NEM írunk, ET/pens csak
  explicit forrásból (v1 `skipped_extra_time` őr kötelező).
- **Identitás:** ESPN gameId forráson belül stabil (v1 gyakorlat:
  13 `external_id_map` mapping élesítve); hivatalos csapatnevek →
  `team_aliases` + crosswalk kell (a 4 PL-alias fent mérve); a v1 szigorú
  feloldás (normalizált egyedi csapatpár + azonos nap, egyetlen jelölt)
  minta; fuzzy összevonás tilos.
- **Kvóta/robots:** robots engedett (spike 2026-11-11:
  `(True, 'robots engedett')`, `/soccer/scoreboard` útvonal). A `?date=`
  query forma engedett; a `/_/date/` forma **robots-TILTOTT** (v1 mérés
  2026-11-09, élőben nem újra-mért — a query formát használjuk, nincs
  megkerülés). v1 kvóta: 60/nap, 4+3 s; spike-delay 6 s; breaker:
  403/429 → azonnali stop, nincs retry. Spike-budget 10/nap, ebből
  **3 fogyott** (robots + 20261111 + 20261108).
- **Frissesség-javaslat:** napi scoreboard naponta (ET-napcsoportosítás →
  2 kérés/nap Budapest-szűréssel, v1 minta); eredmény a meccs utáni
  batch-ben; élő állás NEM írható véglegesnek (v1 őr).
- **Bizalom domainenként:** aggregator → **közepes, keresztforrás**
  (spec §3.2); eredmény FT keresztellenőrzés football-data ellenében
  (a spike-ban 5/5 egyezés élő mintán); fixture/kickoff közepes;
  AET/pen állás nem gate-tényező (rés).
- **v1-érettség + kötelező őrök:** v1 adapter elkészült és élesben
  validált, de **`enabled=false`** (`docs/espn-source-2026-11-09.md`:
  7 nap alatt 0 eredmény-hézag, 0 kezdőidő-pótlás, 11% linkelhetőség).
  Kötelező:
  1. `__espnfitt__`/`scoreboard` tartalmi őr (nincs JSON → no_data,
     nem „0 meccs”; offline teszt OK: decoy-HTML és hibás JSON → None);
  2. **dátum-visszhang őr** (`gmsByLeagueDate` == kért nap; élőben zöld
     mindkét napon);
  3. **AET/pen skip** (`skipped_extra_time`, 90 perces konvenció);
  4. élő állás nem végleges; `postponed` nem ír felül meglévő eredményt;
  5. fail-closed linkelés (csak mapping vagy szigorú auto-feloldás,
     ismeretlen csapatnév → nem review-jelölt, csak számlálás — v1
     higiénia);
  6. üres eseménylista decoy-guard (`guard_rows`, offline teszt OK);
  7. robots-kapu + napi budget + 403/429 breaker + 5 MB méretkorlát.
- **Kockázatok + mitigáció:**
  - **ToS nincs ellenőrizve** (v1 doc §8: a robots enged, de az ESPN ToS-e
    külön ellenőrzést igényel éles használat előtt) → adoptáció előtt
    licenc/ToS-review, felhasználói döntés.
  - Alias-függőség: a hivatalos nevek miatt exact join önmagában 40% →
    alias-réteg (adat, review-val), fuzzy tilos.
  - ET-napeltolódás → 2 kérés/nap + Budapest-szűrés + dátum-visszhang őr.
  - Globális feedben alacsony átfedés a modellünkkel (7/110 név) → a
    forrás szerepe PL/top-5 keresztforrás + FA Kupa-jelölt, nem globális
    hézagkitöltő (v1 mérés tanulsága).
- **Státusz + bizonyíték:** `igazolt` — spike 2026-11-11, python3.13,
  `/tmp/s1/probe_espn.py`: robots 200 engedett; mindkét scoreboard 200,
  `__espnfitt__` JSON kinyerve, dátum-visszhang zöld, 75+58 esemény,
  guardok zöldek; linkelhetőség offline mérve (40% event / 60% név,
  4 alias → 100% PL-plafon); AET/pen rés élőben igazolva (3 FA Cup
  esemény); decoy-teszt offline OK (8 eset). Élő adaptációk
  (controller-ruling, őrök érintetlenek):
  1. regex: a brief `__espnfitt__` mintája helyett a v1
     `window\['__espnfitt__'\]` alak (a brief alak fallbackként maradt).
  2. JSON-útvonal: a brief `page.content.scoreboard.events` helyett a v1
     `page.content.scoreboard.gmsByLeague[].evts[]` (liga-csoportok).
  3. dátum-visszhang őr (`gmsByLeagueDate`) a v1 `_fetch_days` mintájára.
  4. a brief napján (20261111) 0 PL-esemény (válogatott szünet) → a
     20261108-i fordulonap is vételezve a linkelhetőség-méréshez
     (budgeten belül, naplózva).
  5. a teljes `__espnfitt__` payload mentve (brief szerint), nem csak a
     scoreboard csomópont (v1 raw-konvenció) — a minta nagyobb, de a
     szerződés-változás offline vizsgálható.
  6. robots-ítélet napi cache; `__main__`-guard; python3.13
     (controller-előírás).
  7. offline determinizmus-javítás (hálózat nélkül): az `--offline`
     újrafutás nap-sorrendje eltért az élőtől → a `linkability.json`
     sha256-ja változott; a probe rendezett nap-sorrendet kapott, a minta
     offline újragenerálva (stabil sha, a mért számok változatlanok).
  Minta-sha256 (`/tmp/s1/samples/espn/`):
  - `scoreboard_20261111.html`: `6e88db6e1d3e9580031813de8420f60dd54b009d3df9a2532a5629b317d59d68`
  - `scoreboard_20261111.json`: `2b53deacc341ba860ecf0ce9501842d91a65df8a26f331c393958349baab66a4`
  - `scoreboard_20261108.html`: `183f9347ecdcff4d0ed3bdeeb652b8703121c106106bf78916311542ab46febc`
  - `scoreboard_20261108.json`: `69cc71f9780d6e99f80ea391cb7bad2338a62f66d02cae2f22ac135f5fd10988`
  - `linkability.json`: `57e7ff3d1bb1ce306e670388c71526f320b45a1cd0ce90a9b7a6f6290a94bf2c`
  Nem ellenőrzött: ToS/licenc, NB I/NB II lefedettség, `vnue` mező élő
  tartalma, a 2 kérés/nap-es ET-sáv élőben, `/_/date/` robots-tiltás
  újra-mérése (v1-bizonyíték).
- **Adoptációs rang:** 11.

### facup-gap — FA Kupa / hazai kupák lefedettsége (W2 döntés-előkészítés)

- **slug / név / base URL / típus:** `facup-gap` / FA Kupa-rés probe (Task 12) /
  jelöltek: `https://www.thefa.com`, `https://www.skysports.com`,
  `https://int.soccerway.com` (+ offline bemenet: Tipplap Task 4 minta,
  ESPN Task 11 minta) / rés-bejegyzés (nem egyetlen forrás; a worldfootball
  explicit FA Kupa-résének döntés-előkészítése, spec §4.1).
- **Hozzáférés:** mindhárom élő jelölt none — nincs kulcs, nincs auth,
  titok/Keychain nem kell.
- **Szállítás + bizonyíték (jelöltenként, spike 2026-11-11, python3.13):**
  - **thefa.com (hivatalos):** `rust-http` a **fixtures** domainre —
    `/competitions/thefacup/fixtures` 200 (180 603 B), SSR tábla
    (`td.cOne`–`cSix`), **20 meccs-sor** a Second Roundra, dátum-fejléc
    „Saturday 5 December 2026” (`td.headl`), kickoff 15:00, tie-sorszám.
    A **results** domain plain HTTP-vel **decoy**: `/competitions/thefacup/results`
    200 (190 603 B), de 0 tábla/0 eredmény — Opta cloud widget
    (`secure.widget.cloud.opta.net`, JS-renderelt; a widget-host robots-a
    NEM ellenőrzött) → results `halasztott` (obscura ebben a spike-ban
    thefa-ra NEM volt engedélyezett, nem mért). A hub
    (`/competitions/thefacup`, 177 249 B) hír/navigáció, nem meccs-adat.
  - **Sky Sports (hivatalos közvetítő):** `rust-http` — a
    `/fa-cup-scores-fixtures` oldal 200 (674 952 B), **40 FA Cup meccs**
    SSR `data-state` JSON-ban (`ui-tournament-matches__match-item`,
    `data-match-id`, `data-match-state`), First Round Proper,
    2026. november 6–9, mind `post` állapot, FT/AET státusz
    (`statusDescription`), `statusQualifier=extraTime` (7 AET meccs),
    11-es párbaj summary-szöveg (2: „Notts County win 3-2 on penalties”,
    „Grimsby Town win 3-2 on penalties”). **ESPN↔Sky keresztellenőrzés:
    4/4 egyezés** (Stafford 3-3 Grimsby, Cray 2-3 Stockport, Wimborne 2-0
    Crawley, Oldham 2-1 Sheffield Wednesday). A brief
    `fixres__item|match-head` regexe **élőben elavult**: a `/football/fixtures`
    oldal JS-shell (0 valódi fixres-sor, a 4 találat JS-modulnév), a kupa-
    oldal új szerződése a `data-state` JSON. Útvonal-drift: a
    `/football/fa-cup/scores-fixtures` és `/fa-cup/scores-fixtures` **404**;
    a helyes alak a hubról discoverált **`/fa-cup-scores-fixtures`** (lapos).
  - **Soccerway (aggregator, Perform/DAZN):** plain HTTP **decoy** —
    `/matches/` 200 (411 520 B), de csak **18 `data-testid`** (mind
    navigációs chrome: `wcl-navigation` stb.), 0 meccs-link, FA Cup csak
    nav-link → a v1 curl-méréssel (18) byte-szintű egyezés. **Obscura
    szállítással igazolt** (v1 Engine CDP-út, read-only wrapper,
    elő-elszámolással): `/matches/` 200, 1 026 276 B, **2305 data-testid**
    (13,0 s), `/england/fa-cup/` 200, 1 191 509 B, **1403 data-testid**
    (12,7 s), **236 `wcl-matchRow-participant` = 118 meccs-sor**,
    196 `wcl-tableScore` = **148 plain numerikus + 8 penalty `<sup>` cella
    + 40 „-”** → **78 lezárt + 20 jövőbeli (05.12-i Second Round)**
    (fix-kör offline újra-parse: az eredeti 74/24 felosztás a 8 penalty
    cellát tévesen a „-” vödörbe számolta), 60 `wcl-drawCell` (sorsolási
    ág: 40 pontszámos cella + 20 üres hely).
    A v1 mátrix 1901-es renderelt testid-száma ma 2305 (szerkezet bővült).
- **Domainek + mezőtérkép (jelöltenként, a spike-ban mért szinten):**
  - thefa fixtures: `td.cThree`/`cFive` csapatnév → `core.match.home/away`
    (text, not null, alias kell); `td.headl` dátum + `time` kickoff →
    `core.match.kickoff_at` (timestamptz, UTC; **UK helyi idő, év a
    fejlécben, konverzió kell**; a `time datetime` attribútum hibás alakú:
    `datetime=">15:00"` → szöveges parse kell); tie-sorszám (`cTwo`) →
    round-kiegészítés. Eredmény a results oldalon NEM mérhető (decoy).
  - Sky `data-state`: `id` (pl. 5172786135332560232) → külső meccs-ID
    (identity crosswalk); `start.date` („Friday 6th November”) +
    `start.time` (19:30) → `core.match.kickoff_at` (**év nélkül, UK helyi
    idő → év-következtetés + konverzió kell**); `teams.home/away.name.full`
    → `core.match.home/away`; `teams.*.score.current` →
    `core.match_result.home_ft/away_ft` (int, null amíg nincs eredmény);
    `status` FT/AET + `statusQualifier` + `summary.full` (pens szöveg) →
    **ET/pens explicit forrás** (a 90 perces állás NEM bontott → AET/pens
    meccsnél 90 perces eredményt NEM írunk, v1 `skipped_extra_time` őr);
    `competition.name.full` = „The FA Cup” → liga-szűrő; `venue`,
    `attendance` mező létezik, de a mintán **null** (nem töltött).
  - Soccerway: `wcl-matchRow-participant` csapatnév → `core.match.home/away`;
    `wcl-tableScore` → eredmény (int, „-” = null, soha nem 0; a penalty
    `<sup class="wcl-penaltyScore_…">` külön cella, NEM „-”);
    `wcl-stageTime` („07.11. 16:00”) → kickoff (**DD.MM. HH:MM, év nélkül,
    forrás-időzóna NEM ellenőrzött** → év-következtetés + TZ-ellenőrzés
    adapter-szeletben); **meccs-szintű ET/pens a mintán JELÖLT**
    (fix-kör offline újra-parse): 2 `Pen` + 5 `AET` `wcl-stageContent`
    címke, 8 penalty cella = 2 különálló meccs (Stafford 3(2)–3(3) Grimsby,
    Altrincham 2(2)–2(3) Notts County; lista + ág nézetben is), Sky/ESPN
    kereszt-egyezéssel; a 90 perces állás NEM bontott (összesített állás)
    → AET/pens skip-őr kötelező;
    **meccs-szintű link VAN a renderelt listán (fix-kör 2, offline
    újra-parse): 58 `eventRowLink` href**, mind az 58 lista-soron
    (38 lezárt + 20 jövőbeli), abszolút match-URL csapat-slugokkal +
    8 karakteres `mid` paraméterrel (pl.
    `https://www.soccerway.com/match/afc-fylde-r9mZOIF0/luton-SlybbadF/?mid=xlm4EAnB`),
    58 különböző URL + 58 különböző mid → külső meccs-ID kinyerhető →
    **identity crosswalk megvalósítható mid/slug exact-match-csel
    (fuzzy tilos)**; a 60 sorsolásiág-sor (40 pontszámos + 20 üres)
    link nélküli (a 40 pontszámos közül 38 a lista-sorokkal azonos
    meccs, 2 ág-only: Bristol Rovers–Cambridge Utd, Oxford City–
    Leicester → ott név+idő exact-match kell); a mid szezonok közti
    stabilitása NEM ellenőrzött. A korábbi „0 link” állítás a
    class-before-href regex-sorrend hibája volt.
- **Lefedettség (demo kör, FA Kupa fókusz):**
  - **Sky: 40/40 FA Cup meccs** (First Round Proper, nov. 6–9, non-league
    csapatokkal is) — a legszélesebb élő FA Cup-lefedettség a spike-ban.
  - **ESPN: 4 FA Cup esemény** a 20261108-i mintán (`English FA Cup`,
    slug `eng.fa`, id 3918; 3/4 AET/pen) — Task 11 óta `igazolt`,
    keresztforrás-szerepben már bizonyított.
  - **thefa: 20 jövőbeli fixture** (Second Round, dec. 5) — hivatalos
    gerinc-jelölt; az 1. round eredményei a results-widgetben (decoy).
  - **Soccerway: 118 sor = 58 lista-sor + 60 sorsolásiág-sor** (a
    pontszám-cellák felosztása a két nézeten át: 78 lezárt + 20 jövőbeli;
    pens: 2 különálló meccs / 8 cella, AET: 5 címke) — széles, de
    obscura-függő.
  - **Tipplap (offline, Task 4 minta):** a 2026-11-11-i listán **FA Cup
    NINCS** (12 liga, 6 kupa: Cseh Kupa, Szlovák Kupa, Angol Liga-kupa
    női Dél/Észak, Bolíviai Liga-kupa, Európa Kupa női) → a kupa-lista
    napfüggő, FA Cup-odds ezen a napon nem igazolt; a Tipplap mint
    FA Cup-elsődleges forrás a minta alapján NEM alátámasztott.
- **Identitás:** Sky `data-match-id` forráson belül stabil (nagy egész,
    crosswalk kell); thefa tie-sorszám + csapatnév (nincs külső ID a
    fixtures táblán); Soccerway meccs-ID a renderelt listán ELÉRHETŐ
    (fix-kör 2): 58 `eventRowLink` href → 58 különböző `mid` (8 karakter)
    + csapat-slug → crosswalk mid/slug exact-match-csel (fuzzy tilos);
    a link nélküli ág-sorokon csapatnév + időpont egyeztetés, 0/1/N-szabály;
    a mid szezonok közti stabilitása nem ellenőrzött; ESPN gameId
    (Task 11) a kereszt-kulcs.
    HU↔EN alias minden jelöltre kell (Sky/ESPN/thefa EN nevek).
- **Kvóta/robots:** thefa robots engedett `/competitions` és offline
    elemzésben `/competitions/thefacup{,/fixtures,/results}` (robots.txt
    1513 B); Sky robots engedett `/football/fixtures`, offline
    `/fa-cup-scores-fixtures` (robots.txt 2175 B); Soccerway robots
    engedett `/matches/` és offline `/england/fa-cup/` (robots.txt 2553 B),
    a v1 szerint `/standings/`, `/draw/` tiltott (nem újra-mért).
    Spike-delay konzervatív: thefa 8 s, sky 8 s, soccerway 10 s (robots 4 s);
    breaker: 403/429 → StopSpike, nincs retry (404-re sincs retry).
    **Spike-budget (2026-11-11): thefa 6/10, sky 8/10, soccerway 7/10** —
    utóbbi tartalmazza a **4-es elő-foglalást** (2 obscura URL × 2:
    fő dokumentum + tartalék); a tényleges fő-dokumentum kérés 2 volt,
    a foglalás konzervatív (nem kerül visszavezetésre). Sky 404: 3 kérés
    (2 útvonal + 1 ismétlés a minta-cache csak-200-as viselkedése miatt).
- **Frissesség-javaslat:** Sky kupa-oldal meccsnapokon napi 1-2 kör
    (fordulónként ~40 meccs, 1 kérés/oldal); thefa fixtures forduló-
    előtti batch (a round-dates oldalról); Soccerway csak akkor, ha a
    Sky/ESPN/thefa hármas nem elég (obscura 13 s/oldal); ESPN napi
    scoreboard (meglévő Task 11 javaslat).
- **Bizalom domainenként (spec §3.2):** thefa **hivatalos → magas**
    (fixtures; results nem mért); Sky közvetítői aggregator → **közepes,
    keresztforrás** (de az ESPN 4/4 egyezés és az explicit AET/pens
    státusz erősíti); Soccerway aggregator → **közepes, keresztforrás**;
    ET/pens mindenhol csak explicit forrásból, a 90 perces konvenció
    sérthetetlen (AET/pens állás nem írható 90 perces eredményként).
- **v1-érettség + kötelező őrök:** thefa/Sky v1 adapter NINCS (új
    jelöltek); Soccerway v1 adapter nincs, csak mátrix-mérés
    (`docs/obscura-source-matrix-2026-11-09.md`: 18 vs 1901 testid,
    16,3 s). Kötelező:
    1. **Sky szerződés-őr:** `data-match-id` + `data-state` JSON parse +
       `competition.name` szűrő — a brief `fixres__item|match-head`
       regexe JS-modulneveken is zöld (HAMIS ZÖLD, offline tesztben
       reprodukálva: 4 találat a shellen). **Perzisztálva (fix-kör):
       `/tmp/s1/parse_sky_guard.py`** (`parse_sky_matches` + `guard_sky`,
       offline önteszt: kupa-oldal 40 sor zöld, JS-shell → DecoyError);
       a `probe_facup.py` és `probe_facup_round2.py` ezt importálja,
       újrafutás nem zöldülhet hamisan;
    2. **thefa sor-őr:** `td.cThree`/`cFive` páros + `td.headl` dátum
       (offline teszt: fixtures 20 sor zöld, results 0 sor → DecoyError);
    3. **Soccerway decoy-őr:** `wcl-matchRow-participant` (plain HTTP 0 →
       DecoyError, obscura 236 → zöld, offline teszt OK); „-” pontszám
       null, nem 0; év-következtetés a stageTime-hoz;
    4. **AET/pens skip** (Sky 7 AET + 2 pens, ESPN 3/4, Soccerway
       5 AET + 2 pens stage-címke — fix-kör) — 90 perces
       eredményt nem írunk; pens csak megfigyelt státusz/szöveg;
    5. dátum-guard (Sky év nélküli dátum, thefa fejléc-dátum);
    6. robots-kapu + napi budget + 403/429 breaker + 5 MB méretkorlát.
- **Kockázatok + mitigáció:**
  - **Sky szerkezet-drift már megtörtént** (fixres → ui-tournament-matches,
    lapos URL-alak) → szerződés-őr fail-closed, spike újraellenőrzés az
    adapter-szelet előtt; a 404-es útvonalak nem retry-zhatók.
  - **thefa results Opta-widget** → a results domainhez obscura vagy a
    widget-feed külön engedélyezett probe-ja kell (más host, robots
    nem ellenőrzött); addig a fixtures a használható domain.
  - **Soccerway fennmaradó rések** (fix-kör 2: az identity-rés megszűnt,
    58 meccs-link, mid/slug kinyerhető): év nélküli idő + nem ellenőrzött
    TZ + ToS (Perform/DAZN) nem ellenőrzött + a mid szezonok közti
    stabilitása nem ellenőrzött → csak tartalék-szerep (az obscura-
    költség változatlan), adapter-szeletben mélyítendő.
  - **Obscura-költség:** 13 s/oldal + a v2 Obscura-integráció még nincs
    megépítve (FotMob-jegyzet) → Soccerway nem elsődleges.
  - **Napfüggőség:** FA Cup-meccs csak fordulónapokon van (a Tipplap
    mintán nincs, az ESPN 20261111-i napján 0 FA Cup) → üres nap ≠ decoy,
    de a guardnak külön kell választania (dátum-guard + forduló-naptár).
- **Státusz + bizonyíték:** `facup-gap` döntés-előkészítés — spike
    2026-11-11, python3.13, `/tmp/s1/probe_facup.py`,
    `probe_facup_round2.py`, `probe_soccerway_obscura.py` (v1 venv
    python 3.13.17, v1 Engine, DB nélkül), `parse_sky_guard.py`
    (fix-kör: perzisztált Sky szerződés-őr, offline önteszt). Jelölt-státuszok:
    **Sky `igazolt`** (FA Cup fixture+eredmény, rust-http, 40 meccs,
    AET/pens explicit, ESPN 4/4 kereszt); **thefa fixtures `igazolt` /
    results `halasztott`** (hivatalos, rust-http SSR; results Opta-decoy);
    **Soccerway `halasztott`** (obscura-transport igazolt, FA Cup 118 sor,
    identity: 58 meccs-link mid/slug-gal — fix-kör 2; év/TZ/ToS rések
    maradnak — teljes igazolás adapter-szeletben);
    **Tipplap FA Cup `nem igazolt`** (a mintán nincs; napfüggő);
    **ESPN `igazolt`** (Task 11, keresztforrás). Offline decoy-tesztek
    zöldek (9 eset: 2 alap [üres lista, decoy-HTML] + 7 szerződés-teszt
    [Sky shell hamis-zöld reprodukció, Sky shell data-match-id, Sky kupa
    zöld, thefa fixtures zöld, thefa results DecoyError, Soccerway plain
    DecoyError, Soccerway obscura zöld]). Élő adaptációk
    (controller-ruling, őrök érintetlenek):
    1. python3.13 minden harness-parancsban (a brief `python3` helyett).
    2. robots-bizonyíték újratöltés forrásonként (+1 kérés: thefa, sky,
       soccerway) a kiegészítő útvonalak offline kapujához
       (logos-wikimedia minta); az 1. futás kapuja nem mentett robots.txt-t.
    3. thefa: a brief `/competitions` kapuja után hub + fixtures + results
       aloldalak vételezése a mentett linkekből (https-re normalizálva).
    4. Sky: a brief `/football/fixtures` célpontja FA Cup-ra JS-shell →
       hub-discovery (`/fa-cup` 200) → a helyes `/fa-cup-scores-fixtures`
       (a két nested útvonal 404); a brief regexe helyett `data-state`
       szerződés-parse (40 meccs, offline mintán).
    5. Sky 404-ismétlés (1 extra kérés a round2 újrafutásban, a minta-cache
       csak 200-as mintát tárol) — budgeten belül (8/10), jegyzet.
    6. Soccerway: a brief `probe_obscura_transport.py --url` parancsa
       elavult (nincs `--url`, nincs soccerway-célpont) → read-only
       wrapper a v1 Engine `_obscura_get` CDP-útjával; a v1 Policy frozen
       → `dataclasses.replace(wait=8000)` hidratálási őr (v1 2026-11-09-i
       mért hiba: load után azonnal olvasva hidratálatlan DOM).
    7. **Elő-elszámolás (controller-ruling):** az obscura-mérés előtt
       4 kérés foglalva a `/tmp/s1/budget.json` soccerway kulcsára
       (3 → 7/10); a wrapper NEM hívott `log_budget`-et (a foglalás az
       elszámolás), a tényleges 2 fő-dokumentum kérést a futásnapló
       rögzíti (`soccerway_obscura_log.json`); al-erőforrások nem külön
       ledger-tételek (Task 5 precedens).
    8. Konzervatív delay: thefa 8 s (≥6), sky 8 s (≥6), soccerway 10 s (≥8).
    Minta-sha256 (`/tmp/s1/samples/`):
    - `thefa/robots.txt`: `8a0a32cca22fef4cd9f560cc2bd3832fd605c8809f261d9f6769f13ebae8fe80`
    - `thefa/competitions.html`: `160740f85954316bd4a3fc20bd24e6d660c9b953a615736b172553db8be7949c`
    - `thefa/thefacup_hub.html`: `d3f5e7962066724026cb2e0cc494e4255548629547586c95dea25a48ad64baf1`
    - `thefa/thefacup_fixtures.html`: `2e96e3e9d8228d4565a2f3050acaf883a33f68205980b186e444283f79e9b3cf`
    - `thefa/thefacup_results.html`: `22aa1c8a3bbd2083c359600fe0e3f008a619b8d0de10234b6ca399794378b1d5`
    - `sky/robots.txt`: `2e2f0dd736038831ae44b430a13daedbbf0660a53fc71b598622b6f2ae27caca`
    - `sky/facup_probe.html` (fixtures shell): `67f3b23fdb13442e288c93d1ca1b4be7063a919723cdd1f5d09761fcc493dfd8`
    - `sky/facup_hub.html`: `cc6b8898037bd35e40de3308d335471d58df3fb80ebf7ed423137bccbfeb616d`
    - `sky/facup_cup_fixtures.html`: `39b840f43d7b4db2df79efd87a44d48bcb1469c7ffddaf36200ff210bf47061e`
    - `sky/facup_matches.json` (offline parse): `e6dddf6170add04879b91f35283337dc003aa4f8126e10cf5482848a78fafeb6`
    - `soccerway/robots.txt`: `f04a115a104fc9730e02faf28395043cb1bc9b142c771dc9a4f6379291aa801b`
    - `soccerway/facup_probe.html` (plain decoy): `ee70f7707e17c38daaaf0edaea9f0407a41ad6537bf0f712386b379a6036f2f1`
    - `soccerway/matches_obscura.html`: `d06c44fb04277a4cb1151574a0881ef1e576bde26321895f034395c5be2e47b4`
    - `soccerway/facup_obscura.html`: `f4ba24e265bcf6c85d465635345934fff6a5dd44ea7452b96635118700dda156`
    - `/tmp/s1/soccerway_obscura_log.json`: `6c8e82240f56044d88e8e3d09bada0e5ad340a7c6b9d5bf9c2dfe3b169349d58`
    Offline bemenetek: `tipplap/day_2026-11-11.html`
    `da659c56a0cc82e3bd249b73f34a5c9cb54dbc2698dcecf81183a923487fc062`,
    `espn/scoreboard_20261108.json`
    `69cc71f9780d6e99f80ea391cb7bad2338a62f66d02cae2f22ac135f5fd10988`.
    Nem ellenőrzött: thefa results (Opta-widget), Soccerway mid-stabilitás/TZ/
    ToS, Sky venue/attendance (null a mintán), FA Cup-odds (Tipplap),
    Magyar Kupa (a spike FA Cup-fókuszú; a Magyar Kupa-rés a FotMob
    213-as liga-ID-jával és a Tipplap kup-listájával követendő).
- **DÖNTÉS-ELŐKÉSZÍTÉS (S1 kilépési review-ra):**

  | jelölt | FA Cup-fedettség (mért) | szállítás | költség | státusz | javasolt szerep |
  |---|---|---|---|---|---|
  | **Sky** | 40 meccs, fixture+eredmény, AET/pens explicit | rust-http | 1 kérés/oldal, 8 s | igazolt | **elsődleges FA Cup-forrás** |
  | ESPN | 4 esemény (20261108), 3/4 AET/pen | rust-http | 1-2 kérés/nap | igazolt (Task 11) | keresztforrás + AET/pen jelzés |
  | thefa | 20 jövőbeli fixture (hivatalos); results decoy | rust-http (fixtures) | 1 kérés/oldal, 8 s | fixtures igazolt / results halasztott | hivatalos fixture-kereszt + round-dates |
  | Soccerway | 118 sor = 58 lista-sor + 60 sorsolásiág-sor; pontszám-cellák a két nézeten át: 78 lezárt + 20 jövőbeli, pens 2 meccs/8 cella, AET 5 címke; 58 meccs-link (mid/slug) | **obscura** | ~13 s/oldal, v2 integráció kell | halasztott | tartalék (identity crosswalk lehetséges; obscura-költség marad) |
  | Tipplap | a mintán 0 FA Cup (napfüggő) | rust-http | 1 kérés/nap | nem igazolt FA Cup-ra | odds csak fordulónapon, újraellenőrzendő |

  **Javaslat:** a FA Kupát **Sky** kapja (rust-http, széles lefedettség,
  explicit AET/pens), **ESPN** keresztforrás marad, **thefa** a hivatalos
  fixture/round-dates kiegészítő; Soccerway obscurás tartalék csak akkor,
  ha a hármas rései (pl. korai selejtezőkörök) ezt indokolják — a v2
  Obscura-integráció megépítése után. A 90 perces konvenció mindhárom
  eredmény-forrásnál kötelező őr (AET/pens skip). A felhasználói döntés
  az S1 kilépési review-n (Task 15); a worldfootball `elvetett` státusza
  (403) ettől független, a kupa-gerinc szerepét ez a hármas veszi át.
- **Adoptációs rang (rés-szerepben):** Sky 1., ESPN 2. (kereszt), thefa 3.
  (fixtures), Soccerway 4. (tartalék), Tipplap 5. (odds, napfüggő).
- **Fix-kör (2026-11-11, offline újra-parse, 0 kérés):** a review 3 Important
  lelete javítva. (1) Soccerway-felosztás: 196 `wcl-tableScore` = 148 plain
  + 8 penalty `<sup>` + 40 „-” → **78 lezárt + 20 jövőbeli** (a korábbi
  74/24 a 8 penalty cellát „-”-ként számolta). A finding „4 penalty meccs”
  részletét a bizonyíték felülírta: a 8 cella **2 különálló meccs × 2 nézet**
  (lista + ág): Stafford–Grimsby, Altrincham–Notts County; a
  „Harborough–Stafford” / „Sutton–Altrincham” párosítás NEM létezik a mintán
  (Leyton Orient 5–1 Harborough, Guiseley 1–0 Sutton, sima állások).
  AET: `extraTime` marker 0, de **5 explicit `AET` stage-címke** van →
  meccs-szintű ET jelölt (a finding „AET nem igazolt” részlete ezzel
  pontosítva). (2) Sky szerződés-őr perzisztálva: `/tmp/s1/parse_sky_guard.py`
  (a probe-ok importálják, hamis zöld kizárva). (3) Task 12 checkpoint
  hozzáfűzve a napi checkpointhoz. Részletek + parancs-kimenetek:
  task-12-report.md fix-kör szekció.
- **Fix-kör 2 (2026-11-11, offline újra-parse, 0 kérés):** a controller
  1 Important lelete javítva — a „0 meccs-href / identity-rés” állítás
  HAMIS volt (a korábbi parse class-before-href regex-sorrendje miatt
  nem talált href-et). Mért tények a `facup_obscura.html`-en (sha256
  `f4ba24e2…da156`, változatlan): **58 `eventRowLink` href** (mind
  href-before-class sorrendű), mind az 58 lista-soron (38 lezárt +
  20 jövőbeli); 58 különböző abszolút match-URL (csapat-slug + 8
  karakteres token) + **58 különböző 8 karakteres `mid`** → identity
  crosswalk megvalósítható mid/slug exact-match-csel (fuzzy tilos).
  A 60 ág-sor link nélküli (a 40 pontszámos közül 38 a listával azonos
  meccs, 2 ág-only); a dokumentum további 20 `/match/` hrefje más
  versenyeké (sidebar). A napi feed mintáján (`matches_obscura.html`)
  122 `eventRowLink` → site-wide minta. A mid szezonok közti stabilitása
  NEM ellenőrzött; az obscura-költség és a `halasztott` státusz
  változatlan. Részletek: task-12-report.md fix-kör 2 szekció.

### W2 logó/zászló — kiegészítő jegyzetek (nem vételezett)

- **FotMob CDN (csak jegyzet, NEM fetch):** a csapatlogó-URL-ek a FotMob
  meccsadataiból elérhetők (v1 gyakorlat), de a közvetlen CDN-hotlink
  külső függőség + ToS/robots-kockázat; ez a spike NEM vételezett.
  Használat esetén: lokális cache + licenc-review + felhasználói döntés.
- **FM MegaPack (offline licenc-ellenőrzés jegyzet, NEM fetch):** v1 forrás 25
  „MegaPack TCM26” FM-logópack (`assets/megapack/team/{id}.webp`,
  v1 `docs/logo-audit-2026-11-03.md`, `docs/review-2026-11-03-madrid-identity.md`;
  sortitoutsi/FM Scout-ökoszisztéma). Fan-made pack: a klublogók
  védjegyek/szerzői jogosak, a pack licencfeltételei NEM ellenőrzöttek →
  **adoptáció előtt explicit licenc-ellenőrzés kell** (a továbbterjesztés
  joga nem tisztázott); hálózati kérés nem történt, a v2-ben való lokális
  jelenlét nem ellenőrzött.

### statsbomb — StatsBomb Open Data

- **slug / név / base URL / típus:** `statsbomb` / StatsBomb Open Data /
  `https://raw.githubusercontent.com/statsbomb/open-data/master/data` /
  open-data (kurált event-archívum, GitHub statikus raw).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11,
  `competitions.json` 200 (34 887 B), bot-kihívás nélkül; v1 előzmény:
  `scrappers/web/sources/statsbomb.py` éles adapter (mérve 2026-09-26),
  a raw host statikus fájljait olvassa, nincs scraping-védelem.
- **Domainek + mezőtérkép (próba-szintű):** verseny/szezon-meta, meccs-meta,
  event-szintű lövések ELŐRE SZÁMÍTOTT xG-vel, lineups.
  - `competitions.json`: `competition_id`/`season_id` (int) →
    `external_id_map` ('statsbomb' competition/season); `competition_name`,
    `country_name`, `competition_gender` (text) → verseny-feloldás alias-szal.
  - `matches/{comp}/{season}.json` → `core.match` (kickoff, csapatok) +
    **séma-rés: `match_shots` (lövésenkénti xG)** + **séma-rés:
    `match_team_stats` (származtatott xG-összeg, lövések, birtoklás)** —
    a v1 táblák v2-ben még nincsenek.
  - Kickoff helyi idő + zóna → UTC-konverzió kell (v1 adapter ZoneInfo-t használ).
- **Lefedettség (demo kör, mérve 2026-11-11):** 80 verseny-szezon sor,
  24 verseny (male + female). Premier League 2 szezon (2003/04, 2015/16),
  La Liga 18 (1973/74–2020/21), Serie A 2 (1986/87, 2015/16),
  1. Bundesliga 2 (2015/16, 2023/24), Ligue 1 3 (2015/16, 2021/22, 2022/23),
  **NB I 0**. Aktuális (2026/27) szezon NINCS → **nem demo-kör
  eredményforrás**, hanem történeti xG-gazdagítás. Női ág: WSL, Liga F,
  NWSL, Frauen Bundesliga, Serie A Women, női VB, női EB.
- **Identitás:** stabil StatsBomb match/team/competition ID →
  `external_id_map` 'statsbomb'; versenynevek kanonikus térképe kell
  (v1 `COMP_NAME_MAP`: „La Liga” → „Primera División”, különben
  duplikátum-verseny keletkezne); csapatnév alias-réteg.
- **Kvóta/robots:** `raw.githubusercontent.com/robots.txt` 404 =
  „nincs robots” jegyzet (statikus host, NEM tiltás); spike-budget
  2/10 fogyott; delay 2 s; breaker: 403/429 → azonnali stop, nincs retry.
- **Frissesség-javaslat:** kurált archívum, rendszertelen bővülés
  (a mintán a legfrissebb `match_updated` 2026-05-26T13:35:19.781918,
  La Liga 2007/2008) → batch új release-kor, napi kérés nem kell.
- **Bizalom domainenként:** xG/stats közepes (display-only, nem
  gate-tényező); eredmény innen NEM kanonikus (kurált archívum,
  a football-data/ESPN marad az eredmény-gerinc).
- **v1-érettség + kötelező őrök:** v1 adapter éles. Kötelező:
  decoy-200 tartalmi őr (`guard_rows`, offline teszt OK), régi meccseknél
  events 404 → `no_data` (nem hiba), gender-scope, kanonikus
  versenynév-térkép, attribúció megjelenítése.
- **Kockázatok + mitigáció:** PL és aktuális szezonok hiánya → szerep
  történeti xG-re szűkítve; non-commercial licenc → kereskedelmi
  használat tilos; repo-átrendezés → újra-probe az adapter-szeletben.
- **Licenc:** StatsBomb Open Data — **attribúció kötelező, non-commercial**
  (v1 `LICENSE_NOTE` + a repo nyílt licenc-nyilatkozata); a megjelenített
  xG-adatoknál forrás-megjelölés kell.
- **Státusz + bizonyíték:** `igazolt` (próba-szintű) — spike 2026-11-11,
  python3.13, `/tmp/s1/probe_depth.py`: robots-jegyzet + `competitions.json`
  200, 80 sor guard zöld, demo-kör lefedettség mérve. Teljes igazolás
  (matches/events séma, xG-mező térkép) a saját adapter-szeletben.
  Minta-sha256 (`/tmp/s1/samples/statsbomb/competitions.json`):
  `e6cd42f5d8956d6aa30fb917ce8d4c3b3df1879a93f02f8feba820930a6971fa`.
  Élő adaptáció: a brief `python3` hívása python3.13-ra cserélve
  (controller-ruling); forrásonkénti StopSpike-elkapás (egy forrás
  megállása nem öli a többit, retry nincs); robots-ítélet napi cache
  (`/tmp/s1/robots_cache.json`) — a második futás robots-kapuja
  0 kérést tett.
- **Adoptációs rang:** 12.

### reep — reep Register (identity crosswalk)

- **slug / név / base URL / típus:** `reep` / reep Register /
  `https://data.reep.football` / open-data regiszter (identitás-gráf +
  meccs-archívum, heti release).
- **Hozzáférés:** none — nincs kulcs, nincs auth, titok/Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, robots 200
  engedett, `latest.json` 200 (637 B), `release.json` 200 (7 151 B);
  v1 előzménymérés (2026-10-02, `launchd/hu.footballapp.reepcut.plist`):
  a `data.reep.football` Range-képes, a letöltés checksummal ellenőrizhető.
- **Domainek + mezőtérkép (próba-szintű):** ELSŐDLEGES szerep:
  identitás-crosswalk. A v4 release fájljai (mérve): `entities`,
  `bridges`, `edges`, `provider_aliases`, `overlay_aliases`,
  `overlay_links`, `redirects`, `entity_search`, `reep-register.duckdb`
  (csv/csv.gz/parquet változatokban) + `matches.csv.gz`.
  - `release.json`: `files.<név>.key` (objektum-útvonal) + `.sha256` +
    `.bytes` → letöltési URL + hash-őr; v2: `external_id_map` 'reep',
    `team_aliases` (strong rung).
  - Meccs-eredmény csak erős keresztellenőrzéssel — a regiszter túlnyomó
    része eredmény nélküli (lásd lefedettség).
- **Lefedettség:** v1 mérés (2026-10-02): 1 402 603 sor, ebből 1 309 305
  `match_state` üres → **nem eredményforrás**. Aktuális release
  (2026-10-19.1): `matches.csv.gz` 38 793 475 B, kiadói sha256
  `fb27f9d7dcbb4e39d9061c0e4ca2f8042a00bf7fefab9788af1e2895e65a0759`.
  Demo-kör csapat-lefedettség a crosswalk-szeletben mérendő.
- **Identitás:** reep ID stabil (v1: 1 364 275 `external_id_map` sor,
  a meccsek pontosan reep-azonosítón felelnek meg, névfelismerés NEM
  kell); a bizalmi mátrix szerint „strong rung” (name-dob) csak egzakt
  egyezésre, minden más → review-sor; fuzzy összevonás tilos.
- **Kvóta/robots:** `data.reep.football/robots.txt` 200, engedett
  (spike-ítélet, a v1 `reep.football` mérés mellett SAJÁT host-ítélet);
  spike-budget 3/10 fogyott; delay 2 s; breaker: 403/429 → stop.
- **Frissesség-javaslat:** heti cut (v1: hétfő 04:10 launchd).
  **Aggály (mérve):** a `latest.json` jelenlegi stampje **2026-10-19.1**
  (`generated_at` 2026-10-19, `activation_requested_at` 2026-11-05),
  ÖREGEBB a v1 által 2026-11-01-én mért 2026-10-31.1-nél → a heti
  kadencia nem garantált, a selection visszafelé is válthat; az
  adapternek a stamp-regressziót kezelnie kell (idempotens, stamp-alapú).
- **Bizalom domainenként:** identitás-híd magas (strong rung, egzakt);
  eredmény alacsony (túlnyomórészt üres) → eredményre nem használjuk.
- **v1-érettség + kötelező őrök:** v1 `scrappers/tipplap/fetch_reep_cut.py`
  a v4 layoutot kezeli (mérve 2026-11-01: `objects/<uuid>` útvonal,
  `checksums.txt` megszűnt, a hash a `release.json`-ban van); a
  `reep_results` modul tesztelt, de v1-ben SOHA NEM FUTOTT (a heti cut
  kézi lépés volt). Kötelező: stamp-guard, kiadói sha256-ellenőrzés,
  decoy-guard, 90 perces konvenció ha eredményt olvasunk, fuzzy tilos.
- **Kockázatok + mitigáció:** a 38,8 MB-os gz meghaladja a harness 5 MB-os
  plafonját → a spike csak a manifestot próbálta; teljes letöltés +
  CSV-fejléc az adapter-szeletben (Range/streaming); layoutváltás →
  újra-probe; kadencia-bizonytalanság → stamp-figyelés.
- **Licenc:** Register **CC0** (v1 mérés 2026-10-02, `reep.football`);
  ez a spike a főoldal licencét NEM kérte újra (0 kérés oda) → az
  adapter-szeletben újraellenőrzendő. CC0 strong rung only.
- **Státusz + bizonyíték:** `igazolt` (próba-szintű: manifest) — spike
  2026-11-11, python3.13, `/tmp/s1/probe_depth.py`: robots engedett,
  `latest.json` stamp-guard zöld, v4 `release.json`
  `files.matches.csv.gz.key` + sha256 guard zöld (`schema:
  reep-public-release/v4`). A CSV fejléce NEM ellenőrzött (méret) —
  teljes igazolás a saját adapter-szeletben.
  Minta-sha256 (`/tmp/s1/samples/reep/`):
  - `latest.json`: `29c46a091aa6ec49a7deaae34b72a0ddfbe92ab2fdb2eead1689ad9af5c29fed`
  - `release.json`: `9aca0c2610571c3a4d788dee4f03e47ff69064fc2fd5a878480a5c6f60668e54`
  Élő adaptáció: a brief CSV-fejléc probe-ja manifest-probe-ra cserélve
  (controller-ruling: release index/manifest, 1+1 kérés; a 69 MB-os
  teljes cut a harness plafonja miatt nem tölthető).
- **Adoptációs rang:** 13.

### clubelo — ClubElo tükör (xgabora bootstrap)

- **slug / név / base URL / típus:** `clubelo` / ClubElo ratings
  (club-football-match-data tükör) /
  `https://raw.githubusercontent.com/xgabora/club-football-match-data/main` /
  harmadik féltől származó tükör (Elo-pillanatképek + football-data
  származék meccs-tábla).
- **Hozzáférés:** none — nincs kulcs, nincs auth.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, README 200
  (19 917 B); `data/EloRatings.csv` 200, de **>5 MB** → harness-stop,
  a tartalom nem mérhető a spike-ban. Közvetlen ClubElo-belépés NEM
  járható: v1 mérés (2026-11-06) szerint `api.clubelo.com` timeout,
  `clubelo.com` Cloudflare-challenge; anti-bot-kikerülés tiltott.
- **Domainek + mezőtérkép (README-dokumentált, CSV nem mérve):**
  Elo-pillanatképek `date,club,country,elo` (a v1 `REQUIRED_COLUMNS`
  egyezik). `date` → as-of pillanatkép-dátum (dátum, UTC-nap);
  `elo` float (két tizedes); `club` angol név (remapolt); `country`
  3 betűs kód. **Séma-rés: v2 club_elo pillanatkép-tábla** (vagy
  származtatott feature) még nincs.
- **Provenance-felosztás (README, mérve 2026-11-11):** a 2025-06-01-ig
  terjedő rekordok a ClubElo SAJÁT pillanatképei (`observed`);
  2025-06-15-től a kiadó SAJÁT számítású folytatása (`provisional`);
  a pillanatképek a hónap 1. és 15. napján vannak; egyes klubnevek
  remapoltak (pl. „Bayern” → „Bayern Munich”). A v1
  `OBSERVED_THROUGH = 2025-06-01` egyezik → a dátum-alapú
  observed/provisional címke KÖTELEZŐ, a modell soha ne olvassa
  a provisionalt mértként.
- **Lefedettség (README-szintű, nem mérve):** 273 972 Elo-rekord
  (2026-09 állapot), 38 liga/divízió kód; a meccs-tábla 238 858 sor
  2000/01–2026-09-03 (PL, Bundesliga, La Liga explicit említett).
  **NB I nem ellenőrzött.** FIGYELEM: a tükör meccs-táblája
  Football-Data.co.uk származék → NEM független keresztforrás.
- **Identitás:** klubnév nem identitás — csak egyedi `teams.name` vagy
  vizsgált `team_aliases.alias` írhat pillanatképet (v1 szabály);
  országkód csak döntetlenfeloldásra; `ROM` → `ROU` alias kell
  (v1 mérés); minden feloldatlan → unmapped-queue, csapatot soha
  nem teremtünk.
- **Kvóta/robots:** `raw.githubusercontent.com/robots.txt` 404 =
  „nincs robots” jegyzet (statikus host, NEM tiltás); spike-budget
  4/10 fogyott; delay 2 s; breaker: 403/429 → stop.
- **Frissesség-javaslat:** a kiadó periodikusan frissít (README);
  adapter: új kiadás bootstrap + saját számítású folytatás (v1 minta),
  as-of ellenőrzéssel.
- **Bizalom domainenként:** observed közepes-magas (ClubElo eredetű),
  provisional közepes (kiadói számítás, keresztellenőrzés kell);
  display-only/feature, nem gate-tényező.
- **v1-érettség + kötelező őrök:** v1 `scrappers/web/sources/clubelo.py`
  éles (bootstrap + saját frissítés). Kötelező: fejléc-őr
  (`RatingsFormatError`, hiányzó oszlopnál stop), identitás-queue,
  provenance-címke, decoy-guard.
- **Kockázatok + mitigáció:** a tükör README-je NYÍLT LICENCET NEM
  deklarál (csak idézési kérés) → adoptáció előtt licenc-review;
  a CSV >5 MB → streaming/Range az adapterben; a kiadó folytonossága
  nem garantált → saját számítású folytatás + archivált bootstrap.
- **Licenc:** a README **citálást kér** (Gábor, A. 2026,
  Club Football Match Data); a ClubElo eredeti adatok jogtisztasága
  a kiadóra hárul; explicit nyílt licenc nincs a README-ben →
  **licenc-review kötelező adoptáció előtt**; a provenance-felosztás
  (observed/provisional) minden esetben kötelező.
- **Státusz + bizonyíték:** `elvetett` (**felhasználói döntés, 2026-11-12,
  6. pont** — saját Elo-számítás a footballdata-eredményekből, a tükör nem
  adoptálható; korábbi státusz: `halasztott`) — a README-probe zöld
  (szerkezet + provenance dokumentált, guard zöld), de a CSV
  fejléce/sorai NINCSENEK MÉRVE (>5 MB harness-plafon; a 200-as válasz
  az útvonal létezését igazolja). Teljes igazolás a saját
  adapter-szeletben. Minta-sha256 (`/tmp/s1/samples/clubelo/README.md`):
  `d8befb1c51ca751959695df99864ca740551a2fa8dea3620b582a74f00209c57`.
  Élő adaptációk: a brief `master` ága `main`-re cserélve (v1
  `REPO_RAW_URL`); a README-guard az „EloRatings” kulcsszó helyett
  a ClubElo-forrás-megjelölést + a négy dokumentált oszlopnevet ellenőrzi
  (a repo README-je összevont dataset-leírás lett); CSV-próba a v1
  útvonalon (200, >5 MB → stop, retry nincs); eredmény-merge
  (`/tmp/s1/probe_depth_results.json`): a részleges újrafutás nem
  írta felül az első futás többi forrásának eredményét.
- **Adoptációs rang:** 14.

### wikidata — Wikidata (crosswalk-jelölt)

- **slug / név / base URL / típus:** `wikidata` / Wikidata /
  `https://www.wikidata.org` / open-data tudásbázis (CC0), MediaWiki API.
- **Hozzáférés:** none (elvileg kulcs nélküli API) — a spike a
  robots-kapun megállt, API-hívás NEM történt.
- **Szállítás:** nem eldöntött — a robots-tiltás miatt nincs
  szállítási bizonyíték.
- **Domainek + mezőtérkép (tervezett, nem mérve):** identitás-crosswalk
  (Q-ID, csapat/versenyazonosítók, aliasok). A spec 3.2 szerint
  Wikidata-egyezés csak **egzakt** egyezésnél strong rung, minden
  más → review-sor.
- **Lefedettség:** nem mérve (robots-stop).
- **Identitás:** Q-ID stabil külső azonosító; crosswalk csak egzakt
  egyezésre (fuzzy tilos).
- **Kvóta/robots:** **SAJÁT host-ítélet (a commons Task 8 verdict NEM
  újrahasznált):** `www.wikidata.org/robots.txt` 200 (15 896 B),
  `User-agent: *` szekció `Disallow: /w/` (153. sor) → a
  `/w/api.php` útvonal TILTOTT. Stop, nincs retry; a tartalmi
  útvonalat a spike NEM kérte. Spike-budget 2/10 fogyott (robots-kapu
  + bizonyíték-megőrző robots-letöltés).
- **Frissesség-javaslat:** nincs (elvetett útvonal).
- **Bizalom domainenként:** nincs mérve; a tervezett szerep
  identitás-híd (egzakt) lett volna.
- **v1-érettség + kötelező őrök:** v1-ben nincs éles Wikidata-adapter
  a spike tudása szerint; kötelező őrök (egzakt egyezés, review-sor)
  csak alternatív útvonal esetén értelmezhetők.
- **Kockázatok + mitigáció:** a robots-tiltás megkerülése TILOS →
  alternatívák külön probe-ot és felhasználói döntést igényelnek.
- **Licenc:** a Wikidata strukturált adata **CC0** — a licenc NEM
  akadály, a robots az.
- **Státusz + bizonyíték:** `elvetett` (robots-tiltás a `/w/` útvonalon)
  — spike 2026-11-11, python3.13, `/tmp/s1/probe_depth.py`;
  bizonyíték: `/tmp/s1/samples/wikidata/robots.txt`, sha256
  `31077e60ea72809fd71fdb414951397be2f141e1e23ea2408e69d77aefa62593`.
  **DÖNTÉSI JEGYZET (Task 15):** az elvetett státusz a
  `www.wikidata.org/w/api.php` útvonalra szól. Nem próbált
  alternatívák: `query.wikidata.org` SPARQL (más host, SAJÁT
  robots-ítélet kell), hivatalos Wikidata-dumpok (statikus, CC0).
  Ha a crosswalkhoz Wikidata kell, a felhasználó dönt az alternatív
  útvonal probe-járól; a reep strong rung + v1 alias-táblák a
  crosswalk egy részét így is fedik.
- **Adoptációs rang:** 15 (feltételes; az elvetett státusz feloldásáig
  vagy alternatív útvonal döntéséig függőben).

### fm27 — Football Manager 27 helyi mentés

- **slug / név / base URL / típus:** `fm27` / FM27 helyi mentés /
  helyi fájl (nincs URL) / licencelt játékállomány (NEM scraper:
  nincs hálózat, nincs ütemezés, nincs robots).
- **Hozzáférés:** helyi, licencelt játék + mentés; auth/Keychain nem
  kell; a v2-ben való lokális jelenlét NEM ellenőrzött.
- **Szállítás:** helyi fájlolvasás — v1: `fmsave` parser (Python 3.12+),
  pontos build-egyezés (`27.0.0+2486695`), frame-plafon wrapper
  (512 MiB); hálózati szállítás nincs.
- **Domainek + mezőtérkép (v1 mérés, 2026-10-31, Dortmund-mentés):**
  identitás (211 066 játékos, 28 353 klub, 52 988 staff, 63 309 csapat),
  szerződés (189 135), sérülés (2 436), CA/PA + 52 attribútum
  **`derived_opinion` címkével** (scout-vélemény, nem tény);
  szimulált fixture-naptár (82 323) SOHA nem kanonikus eredmény.
  Nincs odds, nincs meccskeret, nincs historikus eredményarchívum,
  nincs verseny-/nemzet-/stadionnév (csak `database_id`), nincs
  klubcímer/klubszín (külső graphics pack kellene).
- **Lefedettség:** v1 mentés: 30 tábla, 650 997 rekord; 22 ok, 5 üres,
  3 failed (az `fmsave` minőségi kapuja); a szezon-történet friss
  karrierben üres (mérve: minden sor 0 perces, most futó szezon).
  Demo-kör: a mentés tartalma karrierfüggő, nem liga-garancia.
- **Identitás:** FM UID / `database_id` stabil a mentésen belül;
  kanonikus egyeztetés nélkül kép/asset NEM kerül az `image_assets`-be
  (külön felülvizsgált lépés); csapat-feloldás slot-0 szabállyal
  (v1 mérve: egyértelmű).
- **Kvóta/robots:** nincs hálózat → nincs robots, nincs budget;
  ez a spike 0 kérést tett az FM27-hez (offline licenc-ellenőrzés).
- **Frissesség-javaslat:** kézi, mentésenkénti export (új időbélyeges
  run, idempotens import `run_key` alapján).
- **Bizalom domainenként:** identitás/szerződés közepes (játék-adat,
  nem valós tény); CA/PA/attribútum `derived_opinion`; eredményre,
  oddsra, meccskeretre SOHA nem kanonikus forrás.
- **v1-érettség + kötelező őrök:** v1 `scrappers/fm27` éles
  (export/staging/kép réteg, 45 teszt). Kötelező: pontos build-egyezés
  (más buildet a parser elutasít), versenynevek csak CSV-ből
  (nem találgatunk), hiányzó adat NULL (nem 0), gitignore minden
  származtatott tartalomra.
- **Kockázatok + mitigáció:** licencelt tartalom → továbbterjesztés
  tilos, a publikus repóba mentés/export/facepack NEM kerülhet;
  a v2 gépén mentés jelenléte nem ellenőrzött → adoptáció előtt
  helyi ellenőrzés + felhasználói döntés.
- **Licenc (offline ellenőrzés, v1 `scrappers/fm27/README.md`,
  read-only):** a mentés **licencelt játékállomány**, a belőle
  származó tartalom NEM terjesztésre való (v1: mindhárom mappa
  gitignore-olt); a facepack/logópack licenc külön kérdés
  (Task 8 jegyzet: MegaPack-licenc nem tisztázott). Hálózati kérés
  NEM történt.
- **Státusz + bizonyíték:** `halasztott` — az offline licenc-ellenőrzés
  kész (v1 README, read-only), hálózati probe tervezetten NINCS;
  a v2-ben való lokális jelenlét és a tényleges adoptáció
  felhasználói döntés (Task 15). Spike: 2026-11-11, 0 kérés.
- **Adoptációs rang:** 16 (opcionális, licenc-ellenőrzéssel és
  felhasználói döntéssel).

### api-football — API-Football (API-Sports)

- **slug / név / base URL / típus:** `api-football` / API-Football
  (API-Sports) / `https://v3.football.api-sports.io` / API (regisztrációs,
  json REST).
- **Hozzáférés:** ingyenes regisztráció (dashboard-kulcs); auth
  `x-apisports-key` HTTP-header (**docs-bizonyíték**, nem spike: a hivatalos
  docs közvetlen letöltése 403, a header-név a hivatalos docs
  search-snippetjéből származik). Kulcs NEM készült, regisztráció NEM
  történt (S1 szabály); adoptáció esetén a kulcs Keychainbe kerül.
- **Szállítás:** NEM mért — a spike a robots-kapun megállt, végpont-probe
  nem futott. A `rust-http` csak hipotézis.
- **Domainek + mezőtérkép:** NEM készült (auth-fal/robots-stop). A
  docs-katalógus szerint fixture, eredmény, lineups, odds, tabella,
  játékos-stats domainek léteznek; mezőszintű térkép és séma-rés lista
  csak regisztráció + kulcsos spike után készíthető.
- **Lefedettség (demo kör):** nem ellenőrzött.
- **Identitás:** saját API-Football ID-k (stabilitás nem mért); crosswalk
  nem készült.
- **Kvóta/robots:** robots.txt **403** → harness-StopSpike, nincs retry
  (2026-11-11, 1 kérés, 0 B). Ingyenes keret: **100 kérés/nap**
  (docs-bizonyíték: hivatalos pricing oldal, search-snippet; közvetlen
  fetch 403). Fizetős: $16/hó 7 500/nap, $26/hó 75 000/nap, $36/hó …
  (a snippet csonka; a teljes lista nem ellenőrzött).
- **Frissesség-javaslat:** nem adott (nem mért forrás).
- **Bizalom domainenként:** nem javasolható mérés nélkül.
- **v1-érettség + kötelező őrök:** v1 adapter NINCS. Kötelező lenne:
  kulcs-őr (Keychain), decoy-200 tartalmi őr (a 200-as hiba-válasz
  lehetősége miatt), napi 100-as keret számláló.
- **Kockázatok + mitigáció:** a host-szintű 403 (robots.txt, weboldal és
  docs egyaránt 403 erről a kliensről) azt jelzi, hogy a közvetlen
  API-Sports végpont erről a hálózatról/IP-ről elérhetetlen lehet —
  **regisztráció előtt tisztázandó**, hogy a kulcsos API egyáltalán
  válaszol-e (különben a regisztráció felesleges). Alternatíva: RapidAPI
  közvetítés (nem probe-olt).
- **Státusz + bizonyíték:** `elvetett` (**felhasználói döntés, 2026-11-12,
  9. pont** — host-403, elérhetőségi próba és regisztráció nélkül; korábbi
  státusz: `regisztráció szükséges`) — spike 2026-11-11,
  python3.13, `/tmp/s1/probe_access.py`: a robots.txt 403-nál
  **StopSpike, a fixtures végpont kulcs nélküli probe-ja NEM futott le**
  (nincs endpoint-spike-bizonyíték); a kulcsigény és az ingyenes keret
  docs-bizonyíték. Minta: nincs (0 B, 403). **Stop + felhasználói kérdés
  (Task 15):** regisztráljunk-e, és ha igen, előbb elérhetőségi próba a
  403 miatt.
- **Adoptációs rang:** nem kijelölt — W4, felhasználói döntés függvénye.

### football-data-org — football-data.org

- **slug / név / base URL / típus:** `football-data-org` /
  football-data.org / `https://api.football-data.org` / API
  (regisztrációs, json REST, v4).
- **Hozzáférés:** ingyenes regisztráció (kliensportál-token); auth
  `X-Auth-Token` HTTP-header (**docs-bizonyíték**:
  docs.football-data.org policies oldal). Kulcs NEM készült, regisztráció
  NEM történt; adoptáció esetén Keychain.
- **Szállítás:** `rust-http` valószínű (a 403-as válasz tiszta JSON,
  bot-kihívás nélkül) — de kulcsos 200-as válasz NEM mért, a döntés
  regisztráció után véglegesíthető.
- **Domainek + mezőtérkép:** NEM készült (auth-fal). Docs szerint a free
  csomag: fixtures, késleltetett eredmények, tabellák; lineups, góllövők,
  lapok, keretek és history csak fizetős csomagban. Mezőszintű térkép
  kulcsos spike után.
- **Lefedettség (demo kör):** free csomag **12 verseny** (docs); a
  versenylista ebben a spike-ban NEM ellenőrzött (a probe a PL-meccs
  végpontot célozta, kulcs nélkül 403).
- **Identitás:** saját football-data.org ID-k (nem mért); a
  football-data.co.uk CSV-vel való név-egyeztetés alias-igényű.
- **Kvóta/robots:** robots.txt **404** = „nincs robots” jegyzet
  (intended-use API-host, openfoot-precedens); végpont kulcs nélkül
  **403** (152 B hiba-JSON). Ingyenes keret: **10 hívás/perc**, 12 verseny,
  késleltetett eredmények (docs-bizonyíték: pricing + policies).
  Regisztrálatlan kliensnek a docs 100 kérés/24 h-t ír — a spike ezt NEM
  tapasztalta (403); az ellentmondás feloldása kulcsos próba nélkül nem
  lehetséges. Fizetős: €12/hó live scores 20/perc; €29/hó ML Pack Light
  vagy Deep Data (lineups, scorers, cards, squads) 30/perc; €49/hó
  Standard 30 verseny 60/perc; €99/hó Advanced 50 verseny 100/perc;
  €199/hó Pro 100 verseny 120/perc; add-onok €15/hó (Odds: 40 verseny
  pre-match 1X2; Statistic: corners, fouls, possession, shots stb.).
- **Frissesség-javaslat:** nem adott (nem mért); a free 10/perc keret
  napi batch-re elegendő lehet, live-ra nem.
- **Bizalom domainenként:** nem javasolható mérés nélkül; a
  football-data.co.uk (CSV) már igazolt gerinc, ez API-kiegészítés lenne.
- **v1-érettség + kötelező őrök:** v1 adapter NINCS. Kötelező lenne:
  token-őr (Keychain), 429-breaker, decoy-200 őr, késleltetett eredmény
  jelzése (a free csomag delayed score-t ad — frissesség-guard).
- **Kockázatok + mitigáció:** a free csomag késleltetett eredménye és a
  12 versenyes korlát a demo-kör lefedettségét szűkítheti → kulcsos
  spike előtt a versenylista ellenőrzendő; odds csak fizetős add-on →
  odds-keresztforrásként a Tipplap + football-data.co.uk closing marad
  az elsődleges.
- **Státusz + bizonyíték:** `regisztráció szükséges` — spike 2026-11-11,
  python3.13, `/tmp/s1/probe_access.py`: `GET /v4/competitions/PL/matches`
  kulcs nélkül **403**, hiba-test: „The resource you are looking for is
  restricted … Please check your subscription.”, `errorCode: 403`.
  Minta-sha256 (`/tmp/s1/samples/football-data-org/authwall_error_body.txt`):
  `2ed7e446e2d24b88fc0a2944c28555c9df188c3193a9ca7f0189709b821d3c44`.
  **Stop + felhasználói kérdés (Task 15):** regisztráljunk-e a free
  csomagra (12 verseny, 10/perc, delayed)?
- **Adoptációs rang:** nem kijelölt — W4, felhasználói döntés függvénye.

### the-odds-api — The Odds API

- **slug / név / base URL / típus:** `the-odds-api` / The Odds API /
  `https://api.the-odds-api.com` / API (regisztrációs, json REST, v4,
  odds-aggregator).
- **Hozzáférés:** ingyenes regisztráció (API-kulcs); auth `apiKey`
  **query-paraméter** (docs-bizonyíték: hivatalos v4 guide). Kulcs NEM
  készült, regisztráció NEM történt; adoptáció esetén Keychain (a
  query-paraméteres kulcs naplóban/URL-ben nem jelenhet meg — header-es
  alternatíva nem ellenőrzött).
- **Szállítás:** `rust-http` valószínű (a 401-es válasz tiszta JSON) —
  kulcsos 200-as válasz NEM mért.
- **Domainek + mezőtérkép:** NEM készült (auth-fal). Docs szerint: odds
  (1X2, spreads, totals, bookmaker-régiók), scores, historical odds.
  Mezőszintű térkép kulcsos spike után; séma-rés: `fact.odds_snapshot`
  bővítés (bookmaker, opening/closing) — a football-data.co.uk odds
  domainjével közös rés.
- **Lefedettség (demo kör):** nem ellenőrzött; a probe a `soccer_epl`
  sportkódot célozta (PL), kulcs nélkül 401.
- **Identitás:** saját event-ID-k (nem mért); csapatnév-alapú crosswalk
  kellene (alias-igény).
- **Kvóta/robots:** robots.txt **404** = „nincs robots” jegyzet; végpont
  kulcs nélkül **401** (152 B hiba-JSON). Ingyenes keret: **500
  kredit/hó** (docs-bizonyíték: hivatalos pricing, Starter csomag, „All
  sports, Most bookmakers, Historical Odds”). Kredit-számítás (docs):
  odds = 1/régió/piac; scores = 1 (2 `daysFrom`-mal); historical =
  10/régió/piac; `/sports`, `/events`, `/participants` ingyenes; üres
  válasz nem számít; 10 bookmaker = 1 régiónak felel meg. Kvóta-headerek:
  `x-requests-remaining`, `x-requests-used`, `x-requests-last`.
  Rate-limit: 429-es szakasz a docsban létezik, a számérték NEM
  ellenőrzött. Fizetős: $30/hó 20 000 kredit; $59/hó 100 000; $119/hó
  5 M; $249/hó 15 M.
- **Frissesség-javaslat:** nem adott (nem mért); az 500 kredit/hó napi
  1-2 PL-odds-körre elég lehet (1 régió × 1 piac = 1 kredit), closing-
  ablakokra szűkös.
- **Bizalom domainenként:** odds mindig kétforrásos (spec 3.2) → ez a
  forrás keresztforrás-jelölt a Tipplap + football-data.co.uk closing
  mellé, egy forrásból soha nem gate-tényező.
- **v1-érettség + kötelező őrök:** v1 adapter NINCS. Kötelező lenne:
  kulcs-őr (Keychain, URL-naplózás tiltva), kredit-számláló
  (x-requests-remaining), 429-breaker, decoy-200 őr (üres odds-lista).
- **Kockázatok + mitigáció:** az 500 kredit/hó gyorsan elfogyhat több piac ×
  régió esetén → piaci szűkítés (1X2 + O/U 2.5, 1 régió) és a
  kredit-számláló guard; a query-paraméteres kulcs szivárgás-veszély →
  napló-maszkolás kötelező.
- **Státusz + bizonyíték:** `regisztráció szükséges` — spike 2026-11-11,
  python3.13, `/tmp/s1/probe_access.py`: `GET /v4/sports/soccer_epl/odds`
  kulcs nélkül **401**, hiba-test: „API key is missing”,
  `error_code: MISSING_KEY`. Minta-sha256
  (`/tmp/s1/samples/the-odds-api/authwall_error_body.txt`):
  `2141792e2ef818edfee9c2d33ddc76a7c7cec150d853c5d1942e090acad0aa09`.
  **Stop + felhasználói kérdés (Task 15):** regisztráljunk-e az 500
  kredit/hós free csomagra odds-keresztforrásnak?
- **Adoptációs rang:** nem kijelölt — W4, felhasználói döntés függvénye.

### bundesliga — Bundesliga hivatalos (SSR)

- **slug / név / base URL / típus:** `bundesliga` / Bundesliga hivatalos /
  `https://www.bundesliga.com` / hivatalos ligaoldal (Angular SSR,
  ld+json SportsEvent).
- **Hozzáférés:** none — nincs kulcs, nincs auth, Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, robots 200
  engedett, oldal 200 (812 572 B), bot-kihívás nélkül; a meccs-adat az
  SSR HTML-ben van (ld+json + Angular markup, `ngh` hidratáció), Obscura
  nem szükséges a fixture-domainhez.
- **Domainek + mezőtérkép (próba-szintű, ld+json SportsEvent):**
  - `startDate` (`2026-11-06T19:30:00+0000`, ISO UTC) →
    `core.match.kickoff_at` (timestamptz, UTC, not null).
  - `homeTeam.name` / `awayTeam.name` → `core.match.home/away` (text,
    not null; HU↔EN alias kell).
  - `homeTeam.logo` / `awayTeam.logo` → klublogó URL
    (`assets.bundesliga.com/clublogos/…/DFL-CLU-XXXXXX.svg`) — a DFL-CLU
    azonosító stabil crosswalk-jelölt; meta-domain.
  - `location.name` (pl. Volksparkstadion) → stadion (meta-domain,
    séma-rés jelölés a stadion-táblára).
  - `eventStatus` = `https://schema.org/EventScheduled` mind a 9
    eseményen → **csak fixture/kickoff; eredmény NINCS az ld+json-ban**
    (score mező 0 előfordulás) → eredmény-domain rés: másik oldal vagy
    forrás kell (football-data.co.uk D1 már igazolt).
  - `name` („Matchday 9: …”), `url`, `offers`, `image` → display/meta.
- **Lefedettség (demo kör):** csak Bundesliga (D1). A mintaoldal a
  9. fordulót mutatta (2026-11-06 → 11-08, 9 SportsEvent — a Bundesliga
  18 csapatos, 9 meccs/forduló). NB I / NB II / többi liga: nincs.
- **Identitás:** DFL-CLU logó-ID stabil jelölt; csapatnév-alias kell;
  meccs-ID az ld+json-ban nem külön mező (az `url` slug tartalmazza).
- **Kvóta/robots:** robots engedett (`/en/bundesliga/matchday`, spike
  2026-11-11); delay ≥6 s; breaker: 403/429 → azonnali stop, nincs retry;
  spike-budget 2/10 fogyott.
- **Frissesség-javaslat:** meccsnapokon napi 1 kör (fixture + későbbi
  eredmény ellenőrzés más forrásból); nem live-forrás.
- **Bizalom domainenként:** fixture/kickoff **magas** (hivatalos, spec
  3.2); eredmény: nem szolgáltatott (n/a).
- **v1-érettség + kötelező őrök:** v1 adapter NINCS (a v1 Obscura-mátrix
  SSR-jelöltként mérte). Kötelező: **ld+json SportsEvent parse-őr**
  (homeTeam/awayTeam/startDate kötelező, üres lista → DecoyError);
  dátum-guard (a fordulóoldal a legutóbbi fordulót mutatja, nem a mai
  napot); 90-perces konvenció az eredmény-forrásban.
- **Kockázatok + mitigáció:** a brief `match-day|kickoff` regexe a mentett
  mintán 27 találatot adott, de ezek túlnyomórészt **Angular CSS-osztály-
  nevek** (hamis zöld, Task 12 anti-pattern) → az erősített ld+json őr
  kötelező (9 valódi esemény, offline verify zöld); Angular-szerkezet
  drift → parse-őr bukása fail-closed; eredmény-domain hiány →
  keresztforrás (football-data.co.uk D1).
- **Státusz + bizonyíték:** `halasztott` — próba-szintű SSR-hozzáférés
  zöld (robots + 200 + 9 parse-olt SportsEvent), teljes igazolás
  (végleges mezőtérkép, eredmény-domain, frissesség-mérés) a saját
  adapter-szeletben. Spike: 2026-11-11, python3.13,
  `/tmp/s1/probe_access.py` (élő regex-guard) + `--verify` (offline
  erősített őr, 0 új kérés). Minta-sha256
  (`/tmp/s1/samples/bundesliga/ssr_page.html`):
  `6a0fe5e232fc46511b5b564f152b9d20e9912473b169a7eeaf5698229a14d313`.
- **Adoptációs rang:** nem kijelölt — W4, próba-szintű; a D1-eredményt a
  football-data.co.uk már fedi, ez a forrás fixture/meta-kiegészítés.

### guardian — The Guardian football (SSR)

- **slug / név / base URL / típus:** `guardian` / The Guardian football /
  `https://www.theguardian.com` / média-aggregator (SSR, `gu-island`
  props JSON).
- **Hozzáférés:** none — nincs kulcs, nincs auth, Keychain nem kell.
- **Szállítás:** `rust-http` — spike-bizonyíték: 2026-11-11, robots 200
  engedett (`/football/results`), oldal 200 (300 940 B), bot-kihívás
  nélkül; a meccs-adat a `FootballMatchesPageWrapper` gu-island props
  JSON-jában SSR-ozva van, Obscura nem szükséges.
- **Domainek + mezőtérkép (próba-szintű, island props):**
  - `dateTimeISOString` (`2026-11-08T14:00:00.000Z`, UTC) →
    `core.match.kickoff_at` (timestamptz, UTC).
  - `homeTeam.name` / `awayTeam.name` → `core.match.home/away` (text,
    alias kell); `homeTeam.score` / `awayTeam.score` →
    `core.match_result.home_ft/away_ft` (int; `kind: Result` esetén
    kitöltve, Fixture esetén null — a mintán mind a 41 `Result`).
  - `homeTeam.id` / `awayTeam.id` → Guardian belső csapat-ID (forráson
    belül stabil jelölt); `paId` → **PA (Press Association) crosswalk-
    jelölt** (identitás-híd, nem ellenőrzött).
  - `competitions[].name/nation/tag/id` → liga-feloldás (alias kell).
  - **Séma-rés:** HT/ET/pens mező nincs a mintában; 90-perces FT-konvenció
    NEM ellenőrzött → guard kell; `comment` mező display-only.
- **Lefedettség (demo kör, 2026-11-08 → 11-10 minta):** mind az 5 liga
  megvan: Premier League 5, Bundesliga 3, Serie A 5, La Liga 5, Ligue 1 6
  eredmény; ezen kívül FA Cup 5, Women's Champions League 5, Women's
  Super League 5, Scottish Premiership 2 (összesen 41). **NB I / NB II
  hiányzik** → rés-jelölés: Tipplap/FotMob pótolja.
- **Identitás:** Guardian csapat-ID + paId jelölt; HU↔EN alias minden
  csapatra kell.
- **Kvóta/robots:** robots engedett (`/football/results`, spike
  2026-11-11); delay ≥6 s; breaker: 403/429 → stop, nincs retry;
  spike-budget 2/10 fogyott.
- **Frissesség-javaslat:** napi 1-2 kör (eredmény-batch); a `kind` mező
  alapján fixture/results szétválasztható; live-ra nem javasolt.
- **Bizalom domainenként:** eredmény **közepes** (média-aggregator, spec
  3.2: keresztellenőrzés vagy provisional); fixture közepes; nem
  gate-tényező egy forrásból.
- **v1-érettség + kötelező őrök:** v1 adapter NINCS (a v1 Obscura-mátrix
  SSR-jelöltként mérte). Kötelező: **gu-island props JSON parse-őr**
  (homeTeam/awayTeam/dateTimeISOString kötelező, üres → DecoyError);
  dátum-guard; 90-perces konvenció ellenőrzése (ET/pens nincs a mintában).
- **Kockázatok + mitigáció:** a brief `football-match` regexe a mentett
  mintán 22 találatot adott, de mind **CSS-változó**
  (`--football-match-*`), meccs-markup 0 (hamis zöld) → az erősített
  island-JSON őr kötelező (41 valódi meccs, offline verify zöld);
  **ToS ellenőrzés adoptáció előtt kötelező** (a BBC Sport ToS miatt
  elvetett; a Guardian robots enged, de a ToS scrape-politikája ebben a
  spike-ban NEM ellenőrzött); island-szerkezet drift → fail-closed őr.
- **Státusz + bizonyíték:** `halasztott` — próba-szintű SSR-hozzáférés
  zöld (robots + 200 + 41 parse-olt eredmény), teljes igazolás a saját
  adapter-szeletben (ToS-ellenőrzéssel). Spike: 2026-11-11, python3.13,
  `/tmp/s1/probe_access.py` (élő regex-guard) + `--verify` (offline
  erősített őr, 0 új kérés). Minta-sha256
  (`/tmp/s1/samples/guardian/ssr_page.html`):
  `89ce3f4735c4d790d6272741395244a16d925b8a3b595bc9465afed78113b339`.
- **Adoptációs rang:** nem kijelölt — W4, próba-szintű; eredmény-
  keresztforrás jelölt (a football-data.co.uk + worldfootball gerinc
  mellé), ToS-döntés függvénye.

### mls-note — MLSsoccer (katalógus-jegyzet, demo körön kívül)

- **slug / név / base URL / típus:** `mls` / MLSsoccer /
  `https://www.mlssoccer.com` / hivatalos ligaoldal (SSR-jelölt a v1
  Obscura-mátrixban).
- **Jegyzet:** az MLS a **demo körön kívül** esik (S1 demo kör: PL,
  La Liga, Serie A, Bundesliga, Ligue 1 + NB I/NB II rés), ezért a
  spec 4.2 szerint csak katalógus-jegyzet: **spike NEM futott, 0 hálózati
  kérés, robots NEM ellenőrzött, budget 0**. Hozzáférés/szállítás/
  mezőtérkép nem mért. Ha a demo kör később bővül, a forrást újra kell
  értékelni (robots-kapu + SSR-probe a saját szeletben).
- **Státusz + bizonyíték:** `javasolt` — leltári jegyzet, spike nem
  futott (2026-11-11, Task 14, demo körön kívül).
- **Adoptációs rang:** nincs (demo körön kívül).

## Hozzáférési mátrix (W4)

Készült: 2026-11-11, Task 14, python3.13, `/tmp/s1/probe_access.py`
(harness-kötött). **Kulcs NEM készült, regisztráció NEM történt** — az
auth-fal probe-ok kulcs nélkül futottak; a falaknál a státusz
`regisztráció szükséges` + stop + felhasználói kérdés (Task 15). Az
ingyenes csomagkeretek **docs-bizonyítékok** (hivatalos pricing/docs
oldalak, illetve API-Football esetén search-snippet, mert a közvetlen
fetch 403) — NEM spike-mérés. A spike csak a hozzáférési falat és az
SSR-tartalmat mérte; mezőszintű véglegesítés mindenhol az adapter-szelet
feladata.

| forrás | kulcs nélkül | ingyenes regisztráció | fizetős | ingyenes keret (docs-evidence) | státusz (2026-11-11) |
|---|---|---|---|---|---|
| api-football | nem (robots 403 → stop; endpoint nem probe-olt; docs: kulcs kötelező) | igen | igen ($16/hó-tól) | 100 kérés/nap | `regisztráció szükséges` |
| football-data-org | nem (spike: 403) | igen | igen (€12–199/hó + €15 add-onok) | 12 verseny, 10 hívás/perc, késleltetett eredmények | `regisztráció szükséges` |
| the-odds-api | nem (spike: 401 MISSING_KEY) | igen | igen ($30–249/hó) | 500 kredit/hó (1 kredit/régió/piac) | `regisztráció szükséges` |
| bundesliga | **igen** (SSR, robots engedett, 9 SportsEvent) | — | — | nincs kulcs | `halasztott` (próba-szintű SSR zöld) |
| sky | **igen** (SSR, Task 12: 40 FA Cup meccs; W4: 14 parse-olt kártya) | — | — | nincs kulcs | `igazolt` (Task 12/13, FA Cup-rés) |
| guardian | **igen** (SSR, robots engedett, 41 eredmény) | — | — | nincs kulcs | `halasztott` (próba-szintű SSR zöld) |
| mls (MLSsoccer) | nem probe-olt (demo körön kívül) | — | — | — | `javasolt` (jegyzet, 0 kérés) |

**Sky Sports W4-kiegészítés (új probe, a meglévő `facup-gap` bejegyzés
mellé):** a `/football/results` oldal robots-engedett, 200 (931 348 B),
14 `data-match-id` kártya, mind a 14 `data-state` JSON parse-olható
(matchState: pre/post). A versenyek a mintán: EFL Trophy, Northern
Premier League, Players Cup, Scottish Women's Premier League, Southern
Premier League South, UEFA Women's Champions League, Women's French
Division 1 — a demo-kör nagy ligái ebben a pillanatképben NEM
szerepelnek (az oldal a legutóbbi alsóbb ligás eredményeket mutatta);
a FA Cup-lefedettség a Task 12 mintán igazolt (40 meccs). A sky napi
budgetje a W4-probe-bal **10/10** (8 Task 12/13 + 2 új: robots + oldal).
Minta-sha256 (`/tmp/s1/samples/sky/ssr_page.html`):
`ab422e44968ff09f3b54ac2b6e4fbf845101ccf95608ba30a611665664b9faa7`.

**W4 spike-budget (2026-11-11, napi 10/forrás):** api-football 1/10
(robots 403 → StopSpike), football-data-org 2/10 (robots 404 + endpoint
403), the-odds-api 2/10 (robots 404 + endpoint 401), bundesliga 2/10,
guardian 2/10, sky 10/10. 429 nem fordult elő; 403/429-breaker érintetlen
(api-football: robots 403 → azonnali stop, nincs retry).

**Élő adaptációk (controller-ruling, őrök érintetlenek — részletesen a
task-14-reportban):** (1) python3.13 a brief `python3` helyett;
(2) auth-fal probe robots-kapuval + harness-stílusú kéréssel (UA,
budget, minta-mentés, sha256), 401/403 → forrásonkénti stop, a többi
forrás folytatódik; (3) API-hoston robots 404 = „nincs robots” jegyzet
(openfoot-precedens), robots TILT → StopSpike; (4) 200-as válasz esetén
decoy-test-ellenőrzés (errors/üres response) — kulcs nélküli 200 nem
fordult elő; (5) SSR: forrásonkénti StopSpike/DecoyError-elkapás
(probe_depth-precedens); (6) Sky: a brief Step 2 kódjából hiányzó Sky
SSR-probe a `/football/results` oldalon, a napi budget 2 fennmaradó
kérésével; (7) **erősített tartalmi őrök**: a brief regexei a
Bundesliga-oldalon (27 CSS-találat) és a Guardian-oldalon (22
CSS-változó találat) egyaránt hamis zöldek voltak → ld+json SportsEvent
parse (9 esemény), illetve gu-island props JSON parse (41 meccs);
Sky: data-state JSON parse (14/14). Az erősített őrök a mentett mintákon
offline futottak (`--verify`, 0 új kérés), a CSS-shell decoy-teszt
reprodukálja a hamis zöldet és fail-closed bukik.

**Felhasználói döntési pontok (Task 15 review-kapu):**
1. API-Football: regisztráció előtt elérhetőségi próba kell (host-szintű
   403), csak utána regisztrációs döntés.
2. football-data.org: free regisztráció (12 verseny, 10/perc, delayed)
   — a CSV-gerinc API-kiegészítéseként?
3. The Odds API: free regisztráció (500 kredit/hó) odds-keresztforrásnak?
4. Guardian: ToS-ellenőrzés az adoptáció előtt (robots enged, ToS nem
   ellenőrzött).
5. Bundesliga/Guardian SSR-források: halasztott státusz feloldása a
   saját adapter-szeletben (W4-ben csak hozzáférés-mérés).

## Bizalmi mátrix (jóváhagyásra)

Készült: 2026-11-11, Task 15 — szintézis a W1–W4 spike-eredményeiből,
új hálózati kérés NEM történt. Alap: a spec §3.2 táblája, frissítve a
katalógus-bejegyzések bizonyítékaival. Cellajelölés: **(igazolt)** =
spike-bizonyíték a bejegyzésben (2026-11-11); **(tervezett)** = docs-/
v1-bizonyíték vagy nem mért ág — az élő igazolás az adapter-szeletben történik.
A mátrix a verification szabálymotor bemenete lesz (S2, a meglévő gate
backward kompatibilis bővítése); jóváhagyás előtt nem érvényes gate.

| Domain | Magas bizalom (egy forrásból is verified) | Közepes (keresztellenőrzés vagy provisional) |
|---|---|---|
| Fixture/kickoff | football-data (igazolt), thefa fixtures (igazolt — FA Cup), hivatalos bundesliga (tervezett — próba-szintű SSR, halasztott), TheSportsDB (tervezett — auth-fal) | FotMob (igazolt — nap-feed obscura), Tipplap (igazolt), ESPN (igazolt), OpenFoot (igazolt), Sky (igazolt), Guardian (tervezett — próba-szintű SSR), Livescore (tervezett — robots-tilt), Soccerway (tervezett — obscura) |
| Eredmény FT/HT | football-data FT+HT (igazolt), worldfootball (tervezett — 403, elvetett), hivatalos results (tervezett — thefa results Opta-decoy, halasztott) | ESPN (igazolt — PL 5/5 offline kereszt az E0-val), FotMob (igazolt), OpenFoot (igazolt — csak `finished` + teljes pontszám), Sky (igazolt — FA Cup), Guardian (tervezett), Livescore (tervezett), Soccerway (tervezett). **ET/pens csak explicit forrásból**: Sky AET/pens jelzés (igazolt), ESPN AET/pen státusz (igazolt — a 90 perces állás nem bontott, skip-őr), Soccerway AET/Pen stage-címke (tervezett), worldfootball pso observed (tervezett) |
| Odds | — nincs egyforrásos magas. **Kétforrásos szabály**: Tipplap (igazolt) + football-data closing (igazolt) — a pár mindkét oldala spike-igazolt, maga az egyeztetési szabály S2-ben épül meg (tervezett) | egy forrás = provisional + kliens-jelzés; The Odds API keresztforrás-jelölt (tervezett — regisztráció szükséges). Jegyzet: a Tipplap Tippmix-szorzó, nem piac-átlag |
| Tabella | football-data számított (igazolt adat — a számítás v1 minta, a pipeline tervezett) | FotMob (igazolt részben — PL DOM-tabella + 2 csapat-sor; teljes tabella nem mért), OpenFoot/Livescore standings (tervezett — docs, nem probe-olt) |
| Kezdő/hiányzó/xG/stats — **display-only, nem gate-tényező** | — | FotMob lineups/hiányzók/xG/stats/shotmap/játékos-stats (igazolt), Transfermarkt hiányzók + góllövők (igazolt), PL-hivatalos eltiltás (igazolt) / sérülés (tervezett — obscura kell), football-data match_stats CSV (igazolt), StatsBomb történeti xG (tervezett — próba-szintű), Livescore (tervezett), OpenFoot lineups/xG (tervezett — 403 plan-gate, starter kulcsból nem tölthető) |
| Identitás-híd | reep „strong rung” (name-dob) egzakt (igazolt próba-szintű — manifest; a CSV nem mért), Wikidata egzakt (tervezett — elvetett robots, alternatíva felhasználói döntés) | minden más → review-sor; `team_aliases` adat-réteg (ESPN 4 PL-alias offline mérve — igazolt offline); fuzzy összevonás tilos |
| Meta/display (logó/zászló/stadion/referee) — **display-only** | — | logos-github (igazolt technika — licenc nyitott, javasolt), logos-wikimedia (tervezett — elvetett robots), FotMob venue/referee (igazolt), TheSportsDB stadion/referee (tervezett), ESPN vnue (tervezett), OpenFoot venue (igazolt — 100/100 null, ebből a feedből nem tölthető), bundesliga location (tervezett) |
| Elo/rating (feature, nem gate-tényező) | — | ClubElo observed (tervezett — CSV >5 MB, halasztott; közepes-magas), kiadói provisional folytatás (tervezett — közepes, keresztellenőrzés kell; a tükör meccs-táblája football-data származék, NEM független keresztforrás) |

Kiegészítő szabályok (a spike-ok tanulságaiból):

1. **90 perces konvenció sérthetetlen**: AET/pens állás soha nem írható
   90 perces eredményként (ESPN/Sky/Soccerway skip-őr); HT explicit
   forrásból: football-data HTHG/HTAG (igazolt), FotMob halfs (igazolt);
   a Guardian mintán HT/ET/pens mező nincs (tervezett őr kell).
2. **Élő részeredmény nem kanonikus** (OpenFoot/FotMob/ESPN/Livescore
   őrök); hiányzó adat NULL/státusz, soha nem nulla.
3. A worldfootball- és logos-wikimedia- és wikidata-elvetés spike-szintű
   (403/robots), NEM adatminőségi/licenc-ítélet; engedélyezett
   újra-probe-val felülvizsgálhatók (Döntési pontok 4., 11.).
4. A display-only domainek (kezdő/hiányzó/xG/stats, logó/meta, Elo)
   egy forrásból is megjeleníthetők provenance-szal, de publish-gate-et
   nem befolyásolnak; a kliens bizalmi jelzése S8 feladata.

## Adoptációs sorrend (jóváhagyásra)

A rangok a bejegyzésekben rögzítettek (változatlanok); a státusz-feltételek
és az S3+ adapter-szelet javaslat külön oszlopban. Elvetett/függő tétel
adapter-szeletet NEM kaphat, amíg a státusz vagy a felhasználói döntés
nem oldja fel (spec §2: csak igazolt forrás kaphat adapter-szeletet).

| Rang | Forrás | Státusz (2026-11-11) | Feltétel / megjegyzés |
|---|---|---|---|
| 1 | footballdata | igazolt | nincs feltétel; NB I/NB II rés → Tipplap/FotMob |
| 2 | thesportsdb | regisztráció szükséges | feltételes: free „123” kulcs 15 eseményes limitje gerincnek elégtelen; prémium vagy meta-szerep — döntési pont 1 |
| 3 | tipplap | igazolt | nincs feltétel; dátum-guard kötelező; NB I/NB II magyar ligás napon újraellenőrzendő |
| 4 | fotmob | igazolt | meccs-detail rust-http azonnal; nap-feed csak a v2 Obscura-integráció megépítése után |
| 5 | transfermarkt | igazolt | nincs feltétel; NB I/NB II nincs a regiszterben (rés) |
| 6 | worldfootball | elvetett (403, spike-szintű) | függőben: engedélyezett újra-probe vagy végleges szerep-átadás — döntési pont 11 |
| 7 | logos-github | javasolt | licenc hiányzik → licenc-döntés előtt nem adoptálható — döntési pont 3 |
| 7 | logos-wikimedia | elvetett (robots, spike-szintű) | függőben: HTML-leíróoldal külön spike vagy alternatíva — döntési pont 3 |
| 8 | openfoot | igazolt | read-only fegyelem (v1-incidens); közös kvóta + plan-gate — döntési pont 5 |
| 9 | livescore-mcp | javasolt (robots-döntési pont) | függőben: a /mcp robots-tilt feloldása nélkül nincs hívás — döntési pont 2 |
| 10 | pl-official | javasolt | eltiltás-domain igazolt (rust-http); sérülés-domain obscura-függő |
| 11 | espn | igazolt | ToS-review adoptáció előtt kötelező — döntési pont 15 |
| 12 | statsbomb | igazolt (próba-szintű) | attribúció + non-commercial; történeti xG-szerep |
| 13 | reep | igazolt (próba-szintű: manifest) | CSV >5 MB → streaming az adapterben; stamp-regresszió kezelés — döntési pont 10 |
| 14 | clubelo | halasztott | licenc-review + CSV-streaming — döntési pont 6 |
| 15 | wikidata | elvetett (robots, spike-szintű) | feltételes: SPARQL/dump alternatíva döntése — döntési pont 4 |
| 16 | fm27 | halasztott | opcionális: helyi jelenlét + licenc + felhasználói döntés — döntési pont 7 |
| — | sky | igazolt | FA Cup-rés szerepben rang 1 (facup-gap); döntési pont 13 |
| — | thefa | fixtures igazolt / results halasztott | FA Cup-rés szerepben rang 3 (hivatalos fixture-kiegészítő) |
| — | soccerway | halasztott | FA Cup-tartalék (rang 4 a rés-szerepben); obscura-függő — döntési pont 12 |
| — | api-football | regisztráció szükséges | rang nem kijelölt; elérhetőségi próba a regisztráció előtt — döntési pont 9 |
| — | football-data-org | regisztráció szükséges | rang nem kijelölt — döntési pont 14 |
| — | the-odds-api | regisztráció szükséges | rang nem kijelölt — döntési pont 14 |
| — | bundesliga | halasztott (próba-szintű SSR) | rang nem kijelölt; D1 fixture/meta-kiegészítő, eredményt a footballdata fedi |
| — | guardian | halasztott (próba-szintű SSR) | rang nem kijelölt; ToS-döntés függvénye — döntési pont 8 |
| — | mls | javasolt (jegyzet) | demo körön kívül, nincs rang |

**Javasolt S3+ adapter-szelet sorrend** (érték × bizonyíték × költség;
a rögzített rangoktól eltérő sorrend indoklással — jóváhagyásra):

1. **footballdata** (S3): eredmény/HT/stats/odds-closing/tabella-gerinc
   egy adapterben; teljesen igazolt, rust-http, legnagyobb érték/költség.
2. **reep** (S3 második fele vagy S4): az identitás-crosswalk minden
   többforrásos lépés előfeltétele (fail-closed linker, spec §1.1) — a
   rögzített 13. rang ellenére előre javasolt; manifest igazolt, a
   streaming + strong-rung import költsége közepes.
3. **tipplap**: odds-elsődleges + HU-nevek/NB I; igazolt, rust-http,
   alacsony költség; a kétforrásos odds-szabály másik fele a footballdata.
4. **espn**: eredmény-keresztforrás (PL 5/5 mérve) + FA Cup-kereszt;
   igazolt, alacsony költség; kapu: ToS-review (döntési pont 15).
5. **sky**: FA Cup elsődleges (40 meccs igazolt, AET/pens explicit);
   alacsony költség; kapu: FA Cup-döntés jóváhagyása (döntési pont 13).
6. **fotmob meccs-detail** (rust-http): lineups/hiányzók/xG/stats —
   a display-domainek legnagyobb fedettsége; a nap-feed (obscura) külön
   későbbi szelet, a v2 Obscura-integráció után.
7. **transfermarkt**: hiányzók + góllövők; igazolt, alacsony költség,
   display-only érték.
8. **openfoot**: fixture/FT-kereszt + NB I; igazolt, de közös havi kvóta
   és béta környezet → a worker-ledger (S2) élesítése után.
9. **pl-official eltiltás** (rust-http részszelet); a sérülés-domain az
   Obscura-integrációhoz kötött.
10. **thefa fixtures**: hivatalos FA Cup fixture/round-dates kiegészítő.
11. **statsbomb**: történeti xG-gazdagítás (nem demo-kör eredményforrás).
12. **clubelo / fm27 / logos-github**: licenc-/helyi döntések után,
    opcionális meta/feature szeletek.
13. **thesportsdb / livescore-mcp / worldfootball / wikidata / soccerway /
    api-football / football-data-org / the-odds-api / bundesliga /
    guardian**: csak a megfelelő döntési pont feloldása és (ahol kell)
    engedélyezett újra-probe után kerülhetnek szeletbe.

## Séma-rés lista (S2 bemenet)

Konszolidált, deduplikált lista az ÖSSZES bejegyzésben jelölt résről
(Task 15 szintézis). Minden tétel: forrás-hivatkozás + javasolt v2 cél.
**Migrációterv NEM része** (S2 hatókör); a cél csak irány, a jóváhagyott
S2 spec dönti el a tábla/oszlop alakját. Hiányzó adat mindenhol
NULL/státusz, soha nem nulla.

1. **`match_stats` (új tábla)** — meccs-szintű lövés/szöglet/lap:
   football-data HS/AS/HST/AST/HC/AC/HY/AY/HR/AR/HF/AF (+HxG/AxG)
   (igazolt CSV-oszlopok).
2. **`match_team_stats` (új tábla)** — csapat-metrikák periódusonként:
   FotMob 14 metrika × All/FirstHalf/SecondHalf (igazolt), Livescore
   `commentaries.stats` (tervezett), StatsBomb származtatott xG-összeg
   (próba-szintű). Az 1. tételtől eltérő granularitás: kulcsos metrika
   (stat_key/value) vs. fix oszlopok — S2 döntés.
3. **`referees` + `core.match.referee`** — football-data Referee (csak
   E0 a 2627-es szezonban, igazolt), FotMob referee (igazolt),
   TheSportsDB strReferee (tervezett), Livescore venue/referee
   (tervezett). Nullázható; a játékvezető-entitás crosswalk-igényes.
4. **`core.match.stadium`** — TheSportsDB strVenue (tervezett), FotMob
   venue (igazolt), ESPN vnue (tervezett — a mező élő tartalma nem
   mért), Livescore (tervezett), Sky venue (igazolt jelenlét — a mintán
   null), bundesliga location.name (próba-szintű). OpenFoot venue
   100/100 null (igazolt) → ebből a feedből NEM tölthető.
5. **`fact.odds_snapshot` bővítés: bookmaker + opening/closing** —
   football-data B365*/PP*/BFE*/Max*/Avg* + `C` closing változatok,
   O/U 2.5, AH (igazolt; szezonfüggő bookmaker-készlet: PS* helyett PP*
   a 2627-es szezonban); The Odds API (tervezett — közös rés).
6. **`fact.odds_snapshot` piac-bővítés: market/outcome dimenziók** —
   Tipplap 32 piaccsoport / 115 odds egy meccsen (Kétesély, BTTS,
   Gólszám 2,5/3,5/4,5, Hendikep, Ázsiai gólszám/hendikep, Pontos
   végeredmény, páros/páratlan…) (igazolt markets JSON).
7. **`match_lineups` (új tábla)** — FotMob kezdők/cserék 11+12
   oldalanként, játékos-ID/optaId/poszt/mezszám (igazolt); Livescore
   lineups + commentaries (tervezett); StatsBomb lineups (próba-szintű);
   OpenFoot lineups 403 plan-gate (igazolt fal — starterből nem
   tölthető, Developer $14/hó döntés).
8. **`player_availability` (új tábla)** — FotMob unavailable
   (type/injuryId/expectedReturn, igazolt); Transfermarkt hiányzók
   (igazolt — `until` DD/MM/YYYY 54/85, 31 üres → expected_return
   nullázható); PL-hivatalos eltiltás (igazolt — GW-tartomány, nem
   dátum) / sérülés (tervezett, obscura); Livescore forma-fixture
   `injuries` SOHA nem hiányzás (historikus, v1-őr).
9. **`match_incidents` (új tábla)** — FotMob `matchFacts.events.events`
   elsődleges / `header.events` visszaesés (igazolt; séma-drift őr
   kötelező); Livescore events + substitutions (tervezett).
10. **`match_shots` (új tábla)** — FotMob shotmap (36 lövés a mintán,
    xG/xGOT/x/y/perc/shotType, igazolt); StatsBomb lövésenkénti előre
    számított xG (próba-szintű).
11. **`match_player_stats` (új tábla)** — FotMob `content.playerStats`
    (452 sor a mintán, igazolt); Livescore `commentaries.playerstats`
    (tervezett — goals, pen_won, rating, lapok, minutes_played).
12. **`season_team_stats` (új tábla)** — FotMob csapat-oldal, 29
    szezon-metrika (v1-bizonyíték, spike-ban NEM mért → tervezett).
13. **`season_player_stats` (új tábla)** — Transfermarkt góllövők
    (stat_key/value: goals/apps/age/position, igazolt; PERC az oldalon
    NINCS — oszlop-csúszás őr).
14. **ET/pens explicit forrásjelölés** — `core.match_result` státusz/
    observed-jelzés: worldfootball pso observed (tervezett), Sky
    statusQualifier + pens summary (igazolt), ESPN AET/pen státusz
    (igazolt — 90 perces állás nem bontott), Soccerway AET/Pen
    stage-címke + penalty `<sup>` cella (próba-szintű). A 90 perces
    konvenció sérthetetlen; pens megfigyelt érték, nem fejtjük szét.
15. **Kickoff UTC-konverzió + date_only konvenció** — konverzió kell:
    football-data Date+Time (forrás-TZ ellenőrzendő), Tipplap év nélküli
    HU helyi idő, thefa UK helyi idő + hibás `time datetime` attribútum,
    Sky év nélküli UK helyi idő, Soccerway DD.MM. HH:MM év nélkül + nem
    ellenőrzött TZ, Livescore Europe/Budapest (tervezett), StatsBomb
    helyi idő + zóna, TheSportsDB (tervezett). Explicit UTC (konverzió
    nem kell): FotMob, ESPN, OpenFoot, worldfootball `data-datetime`,
    bundesliga, Guardian. `date_only` déli helyőrző (v1 konvenció) +
    OpenFoot `kickoffTimeKnown=false` ág (nem tesztelt) kezelendő.
16. **Identitás-crosswalk táblák (`external_id_map` + `team_aliases` +
    liga-alias)** — minden forrás külső ID-je és név-aliasa; reep strong
    rung (manifest igazolt), ESPN 4 PL-alias (offline mérve), Tipplap
    HU↔EN, PL CLUB_CODES (26 kód), Soccerway mid/slug (58 link mérve),
    Guardian paId (nem ellenőrzött), OpenFoot sourceRefs (43/100 hiány),
    StatsBomb COMP_NAME_MAP, FM database_id, worldfootball 0/1/N-szabály.
    Fail-closed linker, review-sor, fuzzy összevonás tilos (S2 alap).
17. **Logó/asset-nyilvántartás + licenc/attribution** — v2 asset-tábla
    (v1 `image_assets` minta): logos-github (licenc null — döntés),
    Wikimedia licenc-metaadat (elvetett útvonal), FotMob CDN (jegyzet),
    FM MegaPack (licenc nem tisztázott); zászló-rés + NB I/NB II
    logó-rés nyitott. Minden megjelenített assethez forrás + rögzítési
    idő + licencállapot.
18. **`club_elo` pillanatkép-tábla (vagy származtatott feature)** —
    clubelo tükör `date,club,country,elo` (README-dokumentált, CSV nem
    mért); observed/provisional provenance-címke KÖTELEZŐ (2025-06-01
    határ), as-of kezelés.

Nem séma-rés, hanem lefedettségi rés (az S2 spec bemenetéhez külön
jelölve): NB I/NB II (football-data, Transfermarkt-regiszter, logók),
FA Cup-odds (Tipplap napfüggő), Magyar Kupa (FotMob liga-ID 213 +
Tipplap kupa-lista követendő), PL-hivatalos PL-only, StatsBomb aktuális
szezon nincs. A FA Cup fixture/eredmény-rés a facup-gap döntéssel
(Sky + ESPN + thefa) fedett — jóváhagyásra.

## Döntési pontok (S1 záró review)

Konszolidált lista a felhasználói review-kapuhoz (Task 15). Minden
pontnál: helyzet, opciók, javaslat. A döntések `docs/decisions/`-be
kerülnek jóváhagyás után (spec §3.1); ez a lista csak előkészítés.

1. **TheSportsDB — regisztráció / free-tier elégtelenség.** A „3” kulcs
   403 (élő); a free „123” kulcs `eventsseason` limitje 15 esemény
   (380 meccses gerinchez elégtelen, docs-bizonyíték). Opciók: (a) prémium
   supporter-kulcs (előfizetés), (b) free kulcs csak stadion/referee/meta
   szerepben, (c) elvetés. **Javaslat: (b) vagy (c)** — a gerincet a
   footballdata fedi, stadion/referee a FotMob-ból igazoltan jön; prémium
   költsége a jelenlegi demo-körhöz nem indokolt.
2. **Livescore MCP — robots vs. intended-use ellentmondás.** A
   szolgáltató AI-ügynök MCP-végpontként hirdeti az oldalt, a robots.txt
   explicit tiltja a `/mcp` útvonalat. Opciók: (a) kapcsolatfelvétel a
   szolgáltatóval, (b) ennek eredményétől függő engedélyezett újra-probe,
   (c) elvetés. **Javaslat: (a) vagy (c)** — megkerülés nélkül; a
   gerinc-szerepet FotMob/ESPN/OpenFoot már fedi, a forrás nem kritikus.
   A v1 `robots_override` anti-pattern NEM kerül át.
3. **Logó-licenc.** GitHub football-logos: `license: null`, LICENSE
   nincs (technikailag igazolt); Wikimedia Commons: robots tiltja az
   imageinfo API-t; alternatívák: FotMob CDN-hotlink (ToS-kockázat, nem
   fetch-elt), FM MegaPack (fan-made, licenc nem tisztázott), bundesliga
   DFL-logók (csak D1). Opciók: (a) licenc-audit a GitHub-packre,
   (b) Wikimedia robots-engedett HTML-leíróoldal külön spike-ja,
   (c) logók nélküli megjelenítés, (d) alternatív forrás keresése.
   **Javaslat: (c) most + (a)/(b) külön engedéllyel** — adoptáció előtt
   licenc-döntés kötelező; zászló-rés és NB I/NB II logó-rés nyitott marad.
4. **Wikidata — SPARQL/dump alternatívák.** A `www.wikidata.org/w/`
   robots-tiltott (elvetett, spike-szintű). Nem próbált alternatívák:
   `query.wikidata.org` SPARQL (más host, SAJÁT robots-ítélet kell),
   hivatalos Wikidata-dumpok (statikus, CC0). Opciók: (a) alternatív
   útvonal probe-ja, (b) nem kell. **Javaslat: (b)** — a reep strong
   rung + v1 alias-táblák a crosswalkot részben fedik; csak mért
   identitás-rés esetén érdemes (a).
5. **OpenFoot — közös kvóta + plan-gate.** A kulcs a v1-gyel közös
   (5000/hó; server-side used 326 vs. v1 fix-log 459 — eltérés nem
   ellenőrzött); lineups 403 `plan_upgrade_required` (Developer $14/hó),
   xG/events docs szerint szintén Developer. Opciók: (a) kulcs-
   újrahasználat folytatása read-only, havi számlálóval, (b) Developer-
   upgrade, (c) kulcs-visszavonás. **Javaslat: (a)** — a server-side
   remaining az irányadó, a worker-ledger (S2) veszi át a számlálást;
   lineups/xG a FotMob-ból jön, upgrade nem indokolt. A v1-incidens
   miatt kanonikus írás csak külön döntéssel.
6. **ClubElo — licenc-review.** A tükör README-je nyílt licencet nem
   deklarál (csak citálást kér); a CSV >5 MB (nem mért); a meccs-tábla
   football-data származék (nem független keresztforrás). Opciók:
   (a) licenc-review + adoptáció observed/provisional címkével,
   (b) saját számítású Elo a footballdata-eredményekből (bootstrap
   nélkül), (c) halasztás. **Javaslat: (a) licenc-reviewval, vagy (b)** —
   az Elo predikció-feature, nem gate-tényező; a provenance-címke
   minden esetben kötelező.
7. **FM27 — helyi jelenlét.** A v2 gépén mentés jelenléte NEM
   ellenőrzött; licencelt játékállomány, továbbterjesztés tilos
   (offline licenc-ellenőrzés kész). Opciók: (a) helyi ellenőrzés +
   opcionális identitás/attribútum-import `derived_opinion` címkével,
   (b) kihagyás. **Javaslat: (b) most** — opcionális mélységi forrás,
   a mag-pipeline nem függ tőle.
8. **Guardian — ToS.** Robots engedett, SSR igazolt próba-szinten
   (41 eredmény), de a ToS scrape-politikája NEM ellenőrzött
   (BBC-precedens: ToS miatt elvetett). Opciók: (a) ToS-review, majd
   halasztott státusz feloldása, (b) elvetés. **Javaslat: (a)** — csak
   eredmény-keresztforrás szerepben, a ToS ismeretében.
9. **API-Football — host-403 elérhetőség.** Robots + web + docs mind
   403 erről a kliensről; a kulcsos API elérhetősége kérdéses.
   Opciók: (a) engedélyezett elérhetőségi próba (regisztráció előtt),
   (b) RapidAPI-közvetítés probe-ja, (c) elvetés. **Javaslat: (a), és
   csak zöld próba után regisztrációs döntés** — különben a regisztráció
   felesleges lehet.
10. **reep — stamp-regresszió.** A `latest.json` stampje 2026-10-19.1,
    ÖREGEBB a v1 által 2026-11-01-én mért 2026-10-31.1-nél → a heti
    kadencia nem garantált. Ez nem elvetési ok: az adapternek
    stamp-alapú, idempotens kezelést kell kapnia (S2 követelmény).
    **Javaslat: tudomásulvétel + stamp-guard követelmény rögzítése** —
    külön felhasználói döntést nem igényel, de a review jegyzőkönyvébe
    kerül.
11. **worldfootball — 403 újra-probe.** Spike-szintű elvetés (robots
    engedett, tartalom 403; serving-változás/kliens-függés nem
    különíthető el retry nélkül). Opciók: (a) engedélyezett szállítási
    újra-probe (adapter-szeletben), (b) a kupa-gerinc szerep végleges
    átadása a Sky/ESPN/thefa hármasnak. **Javaslat: (b) most, (a) csak
    akkor, ha a UEFA-kupa archívum-mélység (1955/56 óta) indokolja** —
    az aktuális kupa-fedettség a facup-gap szerint megoldott.
12. **Soccerway — obscura-függőség.** Plain HTTP decoy; obscurával 118
    FA Cup-sor + 58 meccs-link (mid/slug crosswalk megvalósítható), de
    ~13 s/oldal, a v2 Obscura-integráció nincs megépítve; év/TZ/ToS/
    mid-stabilitás nem ellenőrzött. Opciók: (a) tartalék-szerep az
    Obscura-integráció után, (b) elvetés. **Javaslat: (a)** — csak ha a
    Sky/ESPN/thefa hármas rései (korai selejtezőkörök) indokolják.
13. **FA Kupa forrás-választás (döntés-előkészítés a facup-gap
    bejegyzésből).** Mért fedettség: Sky 40 meccs (AET/pens explicit,
    ESPN 4/4 kereszt), thefa 20 jövőbeli fixture (results decoy),
    Soccerway 118 sor = 58 lista-sor + 60 sorsolásiág-sor (obscura;
    pontszám-cellák a két nézeten át: 78 lezárt + 20 jövőbeli, pens
    2 meccs/8 cella, AET 5 címke), ESPN 4 esemény, Tipplap 0 (napfüggő).
    **Javaslat: Sky elsődleges + ESPN kereszt + thefa hivatalos
    fixture/round-dates kiegészítő**; 90 perces skip-őr mindenhol;
    Magyar Kupa követendő (FotMob 213 + Tipplap).
14. **Regisztrációk: football-data.org free (12 verseny, 10/perc,
    késleltetett) és The Odds API free (500 kredit/hó).** Opciók:
    (a) regisztráció kulcsos spike-pal (Keychain), (b) nem kell.
    **Javaslat: (b) most** — a CSV-gerinc + Tipplap closing a kétforrásos
    odds-szabályt fedezi, az eredmény-gerinc igazolt; regisztráció csak
    mért keresztforrás-rés esetén indokolt. Kulcsot S1 nem hozott létre.
15. **ESPN — ToS-review.** A spike igazolt (linkelhetőség mérve, PL
    5/5 eredmény-egyezés), de a ToS nincs ellenőrizve (v1 doc §8).
    **Javaslat: ToS-review az adoptáció előtt** — a 4. és 5. javasolt
    adapter-szelet kapuja.
16. **S2 spec indítás.** A fenti döntések + a bizalmi mátrix +
    adoptációs sorrend jóváhagyása után indul az S2 spec (ingest
    keretrendszer) külön brainstorming ciklusban, a séma-rés listával
    mint bemenettel. **STOP: ez a review-kapu** — jóváhagyás nélkül
    következő fázis nem indul.

## S1 kilépési kritériumok

Spec §6.2 pontról pontra, őszinte állással (Task 15, 2026-11-11).
A §6.1 forrásonkénti kritériumok teljesültsége a bejegyzések
„Státusz + bizonyíték” és „Nem ellenőrzött” rovataiban van rögzítve.

| # | Kritérium (spec §6.2) | Állás | Bizonyíték / hivatkozás |
|---|---|---|---|
| 1 | W1+W2 minden forrása igazolt vagy explicit elvetett | **RÉSZBEN TELJESÜLT** | W1: footballdata/tipplap/fotmob/transfermarkt igazolt, worldfootball elvetett (403), thesportsdb `regisztráció szükséges`. W2: openfoot/espn/sky igazolt, logos-wikimedia elvetett (robots), logos-github/pl-official/livescore-mcp `javasolt` (licenc-/decoy-/robots-döntési pont). A nem igazolt/elvetett státuszok explicit döntési pontokra várnak (1., 2., 3., 9., 14.) — csendben lezárt forrás nincs |
| 2 | W3/W4 minden bejegyzése státusszal és bizonyítékkal vagy halasztási indokkal | **TELJESÜLT** | W3: statsbomb/reep igazolt (próba-szintű), clubelo halasztott (CSV >5 MB), wikidata elvetett (robots + döntési jegyzet), fm27 halasztott (offline licenc kész). W4: 3 API `regisztráció szükséges` (docs-keretek + auth-fal bizonyíték), bundesliga/guardian halasztott (próba-szintű SSR), sky igazolt, mls javasolt jegyzet (0 kérés, demo körön kívül) — Hozzáférési mátrix (W4) fejezet |
| 3 | FA Kupa-résre döntés-előkészítés | **TELJESÜLT** | `facup-gap` bejegyzés döntés-előkészítés táblája + 13. döntési pont (Sky/ESPN/thefa javaslat, Soccerway tartalék) |
| 4 | Bizalmi mátrix + adoptációs sorrend felhasználói jóváhagyással | **FÜGGŐBEN — review-kapu** | „Bizalmi mátrix (jóváhagyásra)” + „Adoptációs sorrend (jóváhagyásra)” fejezetek elkészültek; a jóváhagyás a felhasználó döntése (16. döntési pont) |
| 5 | Séma-rés lista összeállítva (S2 spec bemenet) | **TELJESÜLT** | „Séma-rés lista (S2 bemenet)” — 18 konszolidált tétel + lefedettségi rés jegyzet |
| 6 | OpenFoot-kulcs újrahasználat és regisztrációs kérdések külön döntési pontként | **TELJESÜLT** | 5. döntési pont (OpenFoot kvóta/plan-gate; Keychain-migráció a controller által, felhasználói jóváhagyással lezárult), 1., 9., 14. döntési pontok (regisztrációk) |
| 7 | Döntés az S2 spec indításáról | **FÜGGŐBEN — review-kapu** | 16. döntési pont; jóváhagyás után külön brainstorming ciklus |

Ismert, NEM teljesített részletek (őszinte zárás):

- A §6.1 „minden domain ≥1 minta” kritérium a függő/halasztott
  forrásoknál értelemszerűen nem teljesült (auth-fal/robots/403/
  méretplafon) — ezek státusza ezt tükrözi, nem zöld.
- NB I/NB II lefedettség napfüggő (válogatott szünet a spike-napokon):
  Tipplap magyar ligás napi újraellenőrzés + FotMob NB II follow-up
  adapter-szelet feladat; a Transfermarkt-regiszterből és a logókből
  NB I/NB II hiányzik.
- A kétforrásos odds-egyeztetés és a keresztforrás-egyeztetés spike-ban
  csak offline mintán futott (ESPN 5/5, Sky 4/4); az éles
  verification-motor S2 feladata.
- A v2 Obscura-integráció nincs megépítve — a FotMob nap-feed, a
  PL-hivatalos sérüléslista és a Soccerway tartalék ettől függ.
- Deferred minor leletek taskonként a progress-naplóban
  (`.superpowers/sdd/2026-11-11-real-data-sources-research/progress.md`
  „minor (deferred)” sorai); a checkpoint :477 „identity-rés”
  szóhasználatát a Task 15 checkpoint-pointer korrigálja (append-only).

## Felhasználói döntések (2026-11-12, S1 záró review)

A felhasználó mind a 16 döntési pontról egyesével döntött; a hivatalos
rögzítés: `docs/decisions/2026-11-12-s1-review-dontesek.md`. Ez a fejezet
a katalógus státuszaira gyakorolt hatást vezeti át (a fenti bejegyzések
közül az érintettek státusz-sora frissült; a többi bejegyzés változatlan,
a döntés itt hivatkozott). Új hálózati kérés NEM történt.

| Pont | Forrás / téma | Döntés | Státusz-hatás |
|---|---|---|---|
| 1 | thesportsdb | elvetés (nincs mért rés; free tier elégtelen) | `regisztráció szükséges` → `elvetett` |
| 2 | livescore-mcp | elvetés (robots-tilt, megkerülés nélkül) | `javasolt` → `elvetett` |
| 3 | logók/zászlók | a v1 réteges megoldás auditált átvétele (MIT zászlók; v1 asset-bázis lokális, nem publikus tárolóba + audit; FotMob liga-logók; új forrás csak mért résekre, lefedettség-ellenőrzéssel) | logos-github `javasolt` → `elvetett` (kiváltva); logos-wikimedia elvetett marad (kiváltva); zászló-rés zárul |
| 4 | wikidata | nem kell most (reep + v1 alias-táblák fedik) | `elvetett` marad; új spike csak mért identitás-rés esetén |
| 5 | openfoot | read-only folytatás közös kulccsal; havi számláló → S2 worker-ledger; server-side remaining az irányadó; kanonikus írás csak külön döntéssel | `igazolt` marad, read-only fegyelemmel |
| 6 | clubelo | saját Elo-számítás a footballdata-eredményekből (predikciós fázis, as-of, provenance kötelező) | `halasztott` → `elvetett` (tükör nem adoptálható); séma-rés 18. tétel módosul |
| 7 | fm27 játékmentés-adat | kihagyás most (MegaPack logók helyi használata a 3. pont szerint külön él) | `halasztott` marad, döntés rögzítve |
| 8 | guardian | ToS-review az adoptáció előtt (tiltás esetén elvetés) | `halasztott` marad, ToS-kapuval |
| 9 | api-football | elvetés (host-403; próba és regisztráció nélkül) | `regisztráció szükséges` → `elvetett` |
| 10 | reep | tudomásulvétel; stamp-guard + idempotens adapter + kiadói sha256 → S2 követelmény | `igazolt` marad, stamp-guard feltétellel |
| 11 | worldfootball | kupa-gerinc végleg a Sky/ESPN/thefa hármasé; újra-probe csak indokolt archívum-mélységnél | `elvetett` marad, szerep-átadással |
| 12 | soccerway | tartalék az Obscura-integráció után (mért hármas-rés + év/TZ/ToS/mid-stabilitás tisztázása esetén) | `halasztott` marad, aktiválási feltételekkel |
| 13 | FA Kupa | Sky elsődleges + ESPN kereszt + thefa fixture/round-dates; 90 perces skip-őr mindenhol; Magyar Kupa: FotMob 213 + Tipplap | facup-gap döntés-előkészítés → jóváhagyott felállás |
| 14 | football-data-org / the-odds-api | regisztráció nem kell most (kétforrásos odds-szabály fedett) | `regisztráció szükséges` marad, halasztva mért résig |
| 15 | espn | ToS-review az adoptáció előtt (tiltás esetén keresztforrás-szerep újratárgyalása) | `igazolt` marad, ToS-kapuval |
| 16 | bizalmi mátrix + adoptációs sorrend | jóváhagyva a fenti átvezetésekkel; S2 spec ciklus elindul | a mátrix és a sorrend érvényes gate-bemenet, az átvezetésekkel |

**Érvényesség:** a „Bizalmi mátrix (jóváhagyásra)” és „Adoptációs sorrend
(jóváhagyásra)” fejezetek ezzel jóváhagyott státuszba kerülnek, a fenti
átvezetésekkel együtt; a „Döntési pontok (S1 záró review)” fejezet
történeti előkészítés marad (nem szerkesztett). Az S2 spec bemenete:
jóváhagyott mátrix + adoptációs sorrend + 18 tételes séma-rés lista
(a 18. tétel a 6. pont szerint módosul: saját Elo-feature, nem
tükör-pillanatkép).
