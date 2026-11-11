# 2026-11-11 — Napi checkpoint

**Állapot:** kész, zöld (Task 4 befejezése + Task 5 + Task 6 + Task 7 + Task 8)
**Előző nap:** 2026-11-10
**Kapcsolódó terv:** [Valós adat bekötése](../../superpowers/plans/2026-11-11-real-data-wiring.md), Task 1–8
**Branch:** `feat/real-data-wiring` (commitolatlan)

## Napi cél

A SwiftUI főoldal a SampleData helyett a footballd API valós válaszából épüljön fel; a backend
megjelenítési tényezőket kapjon (tier/prioritás, tabella, odds snapshot, HT/ET/pens), a demóadat
bővített fixture-párként töltődjön a dev adatbázisra.

## Kiinduló állapot (az előző sessionből átvéve)

- [x] Task 1–3 kész: `0003_display_facts.sql` migráció (23 tábla), fixture v2 observationök,
      kanonizáló materializálás; mindkét DB-n lefutott migráció.
- [x] Task 4 implementáció kész (`storage/repositories/day.rs`, `api/routes/day.rs`), de a
      `day_row_includes_display_fields` teszt **hiányzott** — a session itt szakadt meg.
- [x] A tesztDB-ben 12 üzleti sor maradt az előző futtatásból → `db-reset.sh` kellett.
- [x] Task 5–8 nem kezdődött el (nem volt demo fixture, Swift-módosítás).

## Elvégzett szeletek és bizonyítékok

### Task 4 — API /v1/day bővítés befejezése

- [x] TesztDB reset: `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/43, teszt 23/6/0.
- [x] Hiányzó teszt hozzáadva: `rust/crates/storage/tests/day_query.rs`
      (`day_row_includes_display_fields`). Magyar szöveg visszaolvasva („Demó”, „Újpest”, „Győr”).
- [x] `cargo test -p football-storage --test day_query -- --test-threads=1` → 2 passed.
      Megjegyzés: párhuzamos futtatásnál a két teszt a közös tesztDB-n ütközött
      (`country_code_key`); a projekt konvenciója az egy szál (`scripts/check.sh`).
- [x] `cargo fmt --all --check`, `cargo test --workspace -- --test-threads=1` (21 teszt),
      `cargo clippy --workspace --all-targets -- -D warnings` → zöld.
- [x] `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/43, teszt 23/6/0.
- Eltérés: a TDD piros fázis elmaradt, mert az implementáció az előző sessionből már kész volt;
      a teszt utólag került be és azonnal zöld lett.

### Task 5 — Demo fixture-pár + dev pipeline E2E

- [x] Ideiglenes generátor: `/tmp/gen-demo-fixtures.mjs` (nem commitolandó) → 26 fájl
      (`tests/fixtures/demo_day_{a,b}_<liga>.json`), JSON-validálás hibátlan.
- [x] Magyar csapatnevek visszaolvasva a generált fájlokból (Ferencváros, Újpest, Győr,
      Szentlőrinc, Saint-Étienne, Bayern München, Atlético Madrid).
- [x] Betöltés dev DB-re: `footballctl ingest-fixture` mind a 26 fájllal; az első forrás
      `provisional_publish`, a második után mind a 20 meccs `verified_publish`.
- [x] `footballctl day --date 2026-11-11` → 20 meccs, mind verified_publish, új mezőkkel.
- [x] `footballctl doctor` → `db=1 gates=21`.
- [x] psql dev: `core.match` 21, `fact.odds_snapshot` 20, `core.standing` 38,
      `ops.publish_gate` verified_publish 21.
- [x] `footballctl day --date 2026-11-09` → 1 meccs (a régi nap megmaradt, új mezői NULL-ok).
- [x] `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/979, teszt 23/6/0.
- Ismert egyszerűsítés: a terv UUID-sémája szerint a kupás Liverpool külön kanonikus csapat
      (nincs standing sora); a tabella csak a tier ≥ 1 ligákban él.

### Task 6 — Swift Core DayMatch bővítés

- [x] Két bukó teszt (`decodesDisplayFields`, `missingDisplayFieldsDecodeAsNil`) → piros
      (fordítási hiba: nincs `competitionName` tag).
- [x] `OddsSnapshot` + 18 opcionális `DayMatch` mező, snake_case CodingKeys.
- [x] `cd apps/FootballAppCore && swift test` → 15 teszt / 4 suite PASS.

### Task 7 — App HomeView valós adat + vizuális ellenőrzés

- [x] `apps/FootballAppTests/LeagueGroupTests.swift` (3 teszt) → piros (`LeagueGroup` nincs a scope-ban).
- [x] Új fájlok: `MatchCardModel.swift`, `LeagueGroup.swift` (CountryDisplay táblával).
- [x] `MatchListView.swift`: LeagueGroup/MatchCardModel alap, prediction blokk törölve,
      odds sor csak meglévő oddsnál, forrás-szöveg a meccs saját `oddsSource`-ából.
- [x] `SampleData.swift`: MatchOdds/Outcome átköltözött, Prediction törölve; a mintaadat preview-célra marad.
- [x] `HomeView.swift`: DayModel + `.task(id:)` betöltés, állapotgép (loading/loaded/empty/error),
      „MINTA ADAT” badge törölve, FilterBar országlistája a valós adatból.
- [x] `xcodegen generate`, `xcodebuild test` → 14 teszt PASS (11 régi + 3 új).
- [x] Vizuális ellenőrzés futó appban (footballd + xcodebuild build + ablak-screenshotok,
      `/tmp/shots/`, nem repó-tartalom):
  - mai nap: liga-csoportok prioritás-sorrendben, tabella-helyezések, odds nyerő-kiemeléssel,
    élő pulzálás perc nélkül, nincs badge és nincs előrejelzés blokk;
  - lista alja: FA Kupa `11-es 4–3 → 120' 2–2 → 90' 1–1 → HT 1–0`, helyezés-zárójel nélkül;
  - „Befejezett” chip (4 meccs), ország-menü valós magyar országnevekkel, Magyarország szűrő
    (NB I + NB II), üres szűrő üzenet;
  - naptár popover (2026. november, magyar hétköznapok, Ma gomb);
  - üres nap (november 12.): „Nincs meccs ezen a napon.”;
  - hibaállapot daemon nélkül: „Nem érhető el a helyi API. Fut a daemon?” + Újra gomb;
    daemon újraindítás után Újra → üres nap, Ma → 20 meccs.
- Eltérés: a vizuális kör közben két app-példány futott (az `open`-nel indított és a Computer
      Use által indított); az előbbi screenshotjai a másik ablakot mutatták. A felesleges,
      általam indított példányt leállítottam, és a screenshotokat az operált ablakon újra felvettem.

### Task 8 — Záró ellenőrzés

- [x] `./scripts/check.sh` → reset + fmt + clippy + szekvenciális workspace-tesztek (21) zöld.
- [x] `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/979, teszt 23/6/0.
- [x] `swift test` (Core) → 15 zöld; `xcodebuild test` (app) → 14 zöld.
- [x] `git status --short --branch` → `feat/real-data-wiring`, commitolatlan változások.

## Futtatott parancsok összefoglalva

```bash
./scripts/db-reset.sh && ./scripts/db-check.sh
cd rust && cargo test -p football-storage --test day_query -- --test-threads=1
cargo fmt --all --check && cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
node /tmp/gen-demo-fixtures.mjs
cargo run --manifest-path rust/Cargo.toml -p footballctl -- ingest-fixture --path <26 fájl>
cargo run --manifest-path rust/Cargo.toml -p footballctl -- day --date 2026-11-11
cargo run --manifest-path rust/Cargo.toml -p footballctl -- doctor
cd apps/FootballAppCore && swift test
cd apps && xcodegen generate && DEVELOPER_DIR=… xcodebuild test …
./scripts/check.sh
```

## Nyitott kérdések / következő döntési pont

1. Commit engedélyezése a `feat/real-data-wiring` ágon (a szelet teljes, minden ellenőrzés zöld).
2. Következő szelet kijelölése: meccs-detail valós adat, live frissítés, vagy identitás-kezelés.
3. A demo fixture-ök sorsa: maradnak-e tartós demóadatként a repóban, vagy csak a dev DB betöltéséig.

## Visszaállítási út

- Kód: a változások commitolatlanok a `feat/real-data-wiring` ágon; elvetésük `git checkout`/`restore`
  csak kifejezett engedéllyel.
- Dev DB: a demo betöltés visszajátszható (`footballctl ingest-fixture`), a 0003 előtti állapot a
  migrációs láncból nem állítható vissza adatvesztés nélkül — a dev DB demoadata tudatosan vállalt.
- Teszt DB: `./scripts/db-reset.sh` bármikor visszaállítja 23/6/0-ra.

---

## S1 — W1 záró checkpoint (forráskutatás, `feat/source-research`)

**Szelet:** S1 valósforrás-kutatás, W1 gerinc-hullám (Task 2–7) lezárva.
**Spec:** docs/superpowers/specs/2026-11-11-real-data-sources-research-design.md
**Katalógus:** docs/research/source-catalog.md (élő dokumentum, minden spike bejegyzése benne).
**Harness:** /tmp/s1/harness.py (review-kötött, python3.13 controller-előírás);
minták csak /tmp/s1/samples/ — nyers payload a repóba NEM került. DB-írás nem történt.

### W1 forrásstátuszok (a katalógus bizonyítékaiból)

| Forrás | Státusz | Szállítás | Rang | Megjegyzés |
|---|---|---|---|---|
| footballdata | `igazolt` | rust-http | 1 | 5 liga-CSV + notes 200, 500 sor; NB I/NB II rés → Tipplap/FotMob |
| thesportsdb | `regisztráció szükséges` | rust-http (tervezett) | 2 (feltételes) | „3” kulcs 403 → StopSpike; free „123” limit 15 esemény (gerincnek elégtelen) |
| tipplap | `igazolt` | rust-http | 3 | dátum-guard élőben reprodukálta a v1 mismatch-et; 115 piaci odds/32 csoport |
| fotmob | `igazolt` | **hibrid**: nap-oldal obscura, meccs-oldal rust-http | 4 | plain HTTP nap-shell decoy (0 link) → obscura 71 meccs; meccs-oldal v1 parserek zöldek |
| transfermarkt | `igazolt` | rust-http | 5 | GB1 hiányzók 85 sor + góllövők 25 sor; season-id kezdőév-konvenció élőben igazolva |
| worldfootball | `elvetett` (403-blokk, spike-szintű) | rust-http (tervezett) | 6 (függőben) | robots engedett, de a CL all-matches 403 → StopSpike, nincs retry; minta nincs; **FA Kupa/hazai kupák amúgy is RÉSEK** (csak UEFA-kupák) → Task 12 |

A worldfootball-elvetés NEM adatminőségi/ToS-döntés: a v1 2026-09-27-i HTTP-mérése
óta a serving változhatott (kliens-függés/IP-tiltás nem különíthető el retry nélkül).
Felülvizsgálat csak engedélyezett szállítási újra-probe-val (W4 vagy adapter-szelet).

### Halmozott séma-rés lista (W1 végéig, a katalógus bejegyzéseiből)

1. `match_stats` — football-data HS/AS/HST/AST/HC/AC/HY/AY/HR/AR/HF/AF (+HxG/AxG).
2. `referees` — football-data Referee (csak E0 a 2627-es szezonban).
3. `fact.odds_snapshot` bővítés: bookmaker + opening/closing — football-data B365*/PP*/BFE*/Max*/Avg* + C változatok.
4. `core.match.stadium` — TheSportsDB strVenue, FotMob venue (azonos rés).
5. `core.match.referee` — TheSportsDB strReferee, FotMob referee (azonos rés).
6. `fact.odds_snapshot` piac-bővítés (market/outcome dimenziók) — Tipplap 32 piaccsoport/115 odds (S2 bemenet).
7. `match_lineups` — FotMob kezdők/cserék (11+12 oldalanként).
8. `player_availability` — FotMob unavailable + Transfermarkt hiányzók (expected_return nullázható).
9. `match_team_stats` — FotMob 14 metrika (possession, xg, shots…).
10. `match_shots` — FotMob shotmap (xG-domain, 36 lövés a mintán).
11. `match_incidents` — FotMob events (két útvonal, séma-drift őr kötelező).
12. `season_team_stats` — FotMob csapat-oldal (v1-bizonyíték, spike-ban nem mért).
13. `season_player_stats` — Transfermarkt góllövők (stat_key/value; perc NINCS az oldalon).
14. worldfootball: új rés a 403 miatt NEM mérhető; a tervezett kupa-gerinc a meglévő
    `core.match`/`core.match_result` + pens observed státuszra épül (ET/pens csak explicit forrásból).

### Budget-incidensek (harness-ledger, 2026-11-11)

- **fotmob 12/10 = TÚLLÉPÉS** (8 ledger-kérés + 4 ledgeren kívüli v1-probe kérés,
  controller által engedélyezett read-only mérés) — szabálysértésként rögzítve;
  aznap további fotmob kérés nem történt. Új szabály: minden ledgeren kívüli
  mérésnek ELŐRE el kell számolnia a várt kéréseket a harness-budget ellen.
- transfermarkt 5/10 — véletlen duplikált probe-futás (import-mellékhatás,
  2 extra kérés), budget-en belül; a probe azóta `__main__`-guardolt.
- worldfootball 2/10 — robots 200 + tartalmi 403; a breaker azonnal stoppolt,
  retry nem történt.
- Egyéb források a kereten belül: footballdata 7/10, thesportsdb 2/10, tipplap 4/10.

### Nyitott döntési pontok

1. **TheSportsDB-regisztráció** (ingyenes „123” kulcs limitje 15 esemény —
   gerinc-szerep vélhetően prémium; vagy csak stadion/referee/meta kiegészítő).
2. **worldfootball 403-feloldás**: engedélyezett-e későbbi szállítási újra-probe
   (W4/adapter-szelet), vagy a kupa-gerinc szerepét végleg más forrás veszi át.
3. **FA Kupa / hazai kupák rése** → Task 12 probe (thefa.com / Sky Sports /
   Soccerway / ESPN / Tipplap) — a worldfootball kifejezetten NEM fedi le.
4. NB I/NB II lefedettség: Tipplap magyar ligás napi újraellenőrzés + FotMob
   NB II follow-up (adapter-szelet).

### Futtatott parancsok összefoglalva (W1, mind python3.13, /tmp/s1)

```bash
# Task 1: harness TDD
python3.13 -m unittest test_harness -v
# Task 2–7: robots-kapu + probe forrásonként (harness: fetch/robots/guard/save/sha)
python3.13 -c "from harness import robots_allowed; print(robots_allowed('<forrás>', '<base>', '<path>'))"
python3.13 probe_footballdata.py
python3.13 probe_thesportsdb.py        # 403 → StopSpike (auth-fal)
python3.13 probe_tipplap.py
python3.13 probe_fotmob.py; python3.13 probe_fotmob_day.py
python3.13 probe_fotmob_obscura_day.py; python3.13 probe_fotmob_match.py
python3.13 probe_transfermarkt.py
python3.13 probe_worldfootball.py      # robots 200, CL all-matches 403 → StopSpike
# offline decoy-tesztek (minden forrásnál, hálózat nélkül)
python3.13 -c "from harness import guard_rows, DecoyError; …"  # üres lista → DecoyError OK
```

Minden spike részletes parancs-listája és kimenete a task-reportokban:
`.superpowers/sdd/2026-11-11-real-data-sources-research/task-{2..7}-report.md`.
Commit nem készült (felhasználói engedélyhez kötött); a változások a
`feat/source-research` munkafáján (katalógus + checkpoint + SDD-munkafájl).

### Visszaállítási út (S1 szelet)

- Dokumentum: a katalógus/checkpoint append-only bővült; elvetés `git restore`
  csak kifejezett engedéllyel (a munkafa-diff a review-csomag alapja).
- /tmp/s1: eldobható scratch (harness, probe-ok, minták, budget.json) — a repó
  állapotát nem érinti; törlése adatvesztés nélkül lehetséges a review után.
- DB: nem érintett (S1 kutatási fázis, nincs írás).

## S1 — W2 Task 8 checkpoint (logók/zászlók spike, 2026-11-11)

Kiindulás: W1 lezárva (6 forrásbejegyzés a katalógusban); a W2 első szelete
a logók/zászlók (GitHub + Wikimedia) élő, read-only spike-ja; harness kötött,
minden parancs python3.13, minták csak /tmp/s1-ben.

- [x] Robots-kapuk: raw.githubusercontent.com robots.txt **404** → „nincs
  robots” (nem elérhető, NEM tiltás — controller-ruling); api.github.com
  robots.txt **404** → „nincs robots”; commons.wikimedia.org robots.txt 200
  (17 713 B) → **robots tilt** (`User-agent: *` → `Disallow: /w/`, az
  imageinfo nincs az Allow-kivételekben) → StopSpike, nincs tartalmi vételezés.
- [x] GitHub probe: a brief útvonala 404 (master/main) → README + EGY
  git/trees API-keres (2304 fájl, nem truncated): tényleges útvonal
  `logos/England - Premier League/Arsenal FC.png`; PNG 200 (28 744 B),
  magic + IHDR 139×181 zöld; repo meta egyszer: `license: null`, LICENSE-fájl
  nincs a tree-ben → **javasolt** + kötelező licencjegyzet.
- [x] Wikimedia: API-minta NEM mentődött (robots-tiltás); a robots.txt
  bizonyíték külön mentve, offline szabályelemzés → **elvetett (spike-szintű
  robots-tiltás, nem adatminőségi/licenc-elvetés; újra-probe engedéllyel)**.
- [x] Katalógus: `logos-github` + `logos-wikimedia` bejegyzés (rang 7,
  display-only, licenc/attribution kötelező jegyzet) + FotMob CDN (hotlink,
  nem fetch) és FM MegaPack (offline licenc-ellenőrzés) jegyzetek; magyar
  ékezetes visszaolvasás kész, záró újsor megvan.
- [x] Budget (harness-ledger): logos-github **10/10**, logos-wikimedia
  **2/10** (≤10/nap; delay 2 s, robots 4 s); 403/429 nem fordult elő; a
  GitHub-404-ek útvonal-adaptációk voltak, a 2. futás robots-duplikációja a
  napi cache bevezetése előtt történt (10-en belül maradt).
- [x] Jelentés: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-8-report.md`
  (parancsok + kimenetek, sha256, adaptációk, licenc-lelet).

Következő döntési pont: (1) a GitHub-logópack licenc nélküli → adoptáció
előtt felhasználói döntés kell (licenc-audit vagy alternatíva); (2) Wikimedia
robots-tiltás → engedélyezett-e a robots-engedett HTML-leíróoldal külön
spike-ja, vagy alternatív zászlóforrás; (3) zászló-rés + NB I/NB II logó-rés
nyitott. Commit nem készült (felhasználói engedélyhez kötött).

## S1 — W2 Task 9 checkpoint (OpenFoot spike — Keychain-blokk, 2026-11-11)

- [x] Step 1 kísérlet: a v1 `.env`-ben az `OPENFOOT_API_KEY` LÉTEZIK (érték
  nem naplózott). A Keychain-írást (`security add-generic-password
  -a football-app-v2 -s openfoot-api-key -U`) az **engedélyrendszer
  kétszer blokkolta** (automode-block: a jóváhagyás csak delegált
  feladat-szövegből származik, nem közvetlen felhasználói üzenetből).
  A bejegyzés nem létezik (`security find-generic-password` exit 44).
  Kulcs nem került logba/fájlba, API-kérés NEM történt.
- [x] Probe elkészült: `/tmp/s1/probe_openfoot.py` (python3.13
  `py_compile` OK; harness-őrök kézzel: budget_left, log_budget minden
  kísérletre, 2 s delay, 5 MB, 30 s, 403/429 breaker, becsületes UA,
  kulcs-scan a mintákon, havi számláló; Keychainből olvas futásidőben).
- [x] Dekoy-teszt offline OK (`guard_rows([])` → `DecoyError`, python3.13).
- [x] Havi számláló inicializálva: `/tmp/s1/openfoot_month.json`
  (2026-11: count 0; 5000/hó keret).
- [x] Docs-olvasat (nem API-hívás, kulcs nélkül): openfootapi.com homepage
  + nyilvános `openapi.json` — endpoint-lista, mezőtérkép-alap, plan-gate
  szövegek (events/analytics/xG Developer plan; lineups/xg bearer-köteles).
- [x] Katalógus: `openfoot` bejegyzés **`javasolt`** státusszal (rang 8,
  élő bizonyíték nélkül, tervezett mezőtérkép + kötelező őrök + incidens-
  jegyzet); magyar ékezetes visszaolvasás kész, idegen glyph nincs, záró
  újsor megvan.
- [ ] Élő spike: NEM futott (Keychain-blokk) — budget **0/10** (openfoot,
  2026-11-11), havi **0/5000**; minta nincs, sha256 nincs; robots élőben
  nem ellenőrzött; lineups/xG plan-gate élőben nem tesztelt.
- [x] Jelentés: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-9-report.md`.

Következő döntési pont: a felhasználó SAJÁT üzenetben engedélyezi-e a
Keychain-bejegyzés létrehozását (`openfoot-api-key`, account
`football-app-v2`, a v1 `.env` kulcsával). Jóváhagyás után a kész probe
azonnal futtatható (`cd /tmp/s1 && python3.13 probe_openfoot.py`), és a
katalógus-státusz élő bizonyítékkal frissítendő. Commit nem készült.

## S1 — W2 Task 10 checkpoint (Livescore MCP spike — robots-döntési pont, 2026-11-11)

- [x] Step 1 robots-kapu az API útvonalon (python3.13, harness):
  `robots_allowed('livescore-mcp', 'https://livescoremcp.com', '/mcp')` →
  **`(False, 'robots tilt')`**. robots.txt 200 (137 B): `Allow: /`, de
  explicit `Disallow: /mcp`, `/sse`, `/message`, `/health`. Bizonyíték
  mentve: `/tmp/s1/samples/livescore-mcp/robots.txt`
  (sha256 `b91e49166889bd9859b7d8e897e1e27dd71a878127f373c6298d2b037be3cd3d`),
  offline szabályelemzés: `can_fetch /mcp = False`, `can_fetch / = True`.
- [x] Probe elkészült: `/tmp/s1/probe_livescore.py` (python3.13
  `py_compile` OK; fail-closed robots-kapu napi cache-sel; JSON-RPC POST-ra
  minden harness-őr kézzel: budget_left, log_budget minden kísérletre,
  6 s delay, 5 MB, 30 s, 403/429 breaker, becsületes UA; tools/list →
  tools/call sorrend, v1 valódi tool-név `get_day_fixtures` format=text).
- [x] Probe-futás: **StopSpike a robots-kapun** — `tools/list` és
  `tools/call` NEM futott, tartalmi hívás 0, tartalmi minta nincs.
  A v1 `robots_override` anti-pattern NEM került át (nincs megkerülés).
- [x] Dekoy-teszt offline OK (python3.13, hálózat nélkül): üres content,
  hiányzó result, JSON-RPC error envelope, üres tools, üres day-text →
  `DecoyError`; SSE-envelope parse zöld.
- [x] Katalógus: `livescore-mcp` bejegyzés **`javasolt` + FELHASZNÁLÓI
  DÖNTÉSI PONT** státusszal (rang 9; a brief szerint robots-tiltnál NEM
  `elvetett`); tervezett mezőtérkép v1-bizonyítékkal, séma-rések
  (match_lineups/match_incidents/match_team_stats/játékos-stats/
  stadium/referee), kvóta 100/nap 6 s (v1); magyar ékezetes visszaolvasás
  kész, idegen glyph nincs, záró újsor megvan.
- [x] Budget: **2/10** (livescore-mcp, 2026-11-11: robots-kapu +
  robots-bizonyíték); a probe-újrafutás napi cache-ből dolgozott (0 extra).
- [x] Jelentés: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-10-report.md`.

Következő döntési pont: a livescoremcp.com robots.txt explicit tiltja a
/mcp API-útvonalat, miközben a szolgáltató a saját oldalán AI-ügynök
MCP-végpontként hirdeti (v1 engedmény). A felhasználó dönt: (1) kapcsolat-
felvétel a szolgáltatóval a robots/intended-use ellentmondásról, (2) ennek
eredményétől függő engedélyezett újra-probe, vagy (3) elvetés. Megkerülés
nélkül; a kész probe a kapu engedése után azonnal futtatható. Commit nem
készült (felhasználói engedélyhez kötött).

## S1 — Task 9 resume (OpenFoot spike — Keychain-írás ismét blokkolva, 2026-11-11)

- [x] Keychain-migráció újrapróbálva a delegált feladatban idézett
  felhasználói jóváhagyással („Keychain-írás engedélyezése”,
  `security add-generic-password -a football-app-v2 -s openfoot-api-key`):
  a helyi engedélyrendszer **ismét blokkolta** (3. próbálkozás; a kapu
  csak a felhasználó SAJÁT, nem delegált üzenetét fogadja el).
  Megkerülés NEM történt; a kulcs értéke nem naplózott.
- [x] Keychain-bejegyzés továbbra sem létezik (`find` metaadat, exit 44);
  élő probe NEM futott, API-kérés 0.
- [x] Offline újraellenőrzés (python3.13): `py_compile` OK, dekoy-teszt OK
  (`DecoyError` üres listára).
- [x] Budget változatlan: openfoot napi **0/10**, havi **0/5000**
  (`openfoot_month.json` count 0); minta és sha256 nincs.
- [x] Katalógus `openfoot` bejegyzés frissítve (Hozzáférés + Státusz:
  resume-blokk rögzítve), státusz marad **`javasolt`**, rang 8; magyar
  visszaolvasás + glyph-scan + záró újsor kész.
- [x] Jelentés: task-9-report.md resume-szekció hozzáfűzve.

Következő döntési pont: a Keychain-íráshoz a felhasználó saját, közvetlen
üzenete kell (a delegált feladat-idézet a kapun nem megy át). Addig az
OpenFoot élő spike áll; a probe változatlanul azonnal futtatható. Commit
nem készült.

## S1 — Task 9 élő eredmények (OpenFoot spike lefutott, 2026-11-11)

- [x] Keychain-migrációt a **controller elvégezte** a fő sessionben
  (bejegyzés létrehozva 2026-11-11T13:54:04Z; `find` metaadat exit 0).
  A spike a kulcsot csak futásidőben olvassa; Keychain-írás NEM történt,
  v1 `.env`-olvasás NEM történt; a kulcs értéke sehol nem naplózott.
- [x] Élő probe lefutott: `cd /tmp/s1 && python3.13 probe_openfoot.py`
  → exit 0; **5 kérés**: robots 200 engedett (401 B), quota 200,
  mai matches 200, finished matches 200, lineups **403 plan-gate**.
  A breaker után további kérés NEM történt (xG nem élő-tesztelt).
- [x] Kvóta élőben: plan `starter`, period `2026-11`, keret 5000/hó,
  **used 326, remaining 4674** (2026-11-11T13:56:13Z); a v1-gyel közös
  kulcs (v1 fix-log 459 vs mért 326 — eltérés rögzítve, nem ellenőrzött).
- [x] Domain igazolva: mai nap **1 meccs** (nem decoy, guard zöld);
  lezárt nap **100/257 sor, 48 verseny, NB I megvan** (Nyíregyháza
  Spartacus FC–Győri ETO 1–1, finished); dátum-guard 0 eltérés mindkét
  napon; 100/100 finished sor pontszámmal.
- [x] Séma-rés élőben igazolva: lineups 403 `plan_upgrade_required`
  (Developer $14/hó) — terv-előírás szerint séma-rés, nem hiba.
- [x] Élő adaptációk rögzítve (jelentés + katalógus): docs→élő
  verseny-ID-k (`comp_laliga_es`, `comp_serie_a_it`, `comp_ligue_1_fr`),
  `next_cursor` starter kulccsal is megjelenik (használhatóság nem
  tesztelt), `venue` 100/100 null, `sourceRefs` 43/100 hiány, `season`
  `2026` és `2026/27` formátum, `meta.environment=beta`.
- [x] Budget: napi openfoot **5/10** (harness-ledger); havi helyi
  **5/5000** (`openfoot_month.json`). Minta: 4 fájl, sha256 a
  katalógusban; probe kulcs-scan + független kulcs-scan: OK.
- [x] Dekoy-teszt offline OK (`guard_rows([])` → `DecoyError` → `OK`).
- [x] Katalógus `openfoot` bejegyzés frissítve: státusz **`igazolt`**,
  rang 8; jelentés (task-9-report.md) élő szekció hozzáfűzve; magyar
  visszaolvasás + idegen-glyph scan + záró újsor ellenőrzés kész.

Következő döntési pont: a Task 9 szelet lezárult; a következő tervfeladat
(Task 10) csak felhasználói engedéllyel indul. Commit nem készült.

## S1 — W2 Task 11 checkpoint (PL-hivatalos hiányzók + ESPN újraértékelés, 2026-11-11)

- [x] Step 1 PL-hivatalos probe (`/tmp/s1/probe_pl_official.py`,
  python3.13): robots-kapu `(True, 'robots engedett')`
  (`/en/news/4450606`, robots.txt 638 B); sérülésoldal 200 (91 464 B)
  → **decoy-200** (0 h3 / 0 v1-parser sor, kliens-renderelt cikk-törzs,
  v1 DynamicSession-előzmény) → obscura kell; eltiltásoldal 200
  (96 442 B) → **valódi SSR**: 3 eltiltott (Dowman ARS GW11, Igor Thiago
  BRE GW11-13, Mateta CRY GW10-12) + 13 ban-risk sor (offline számolva).
- [x] Step 2 ESPN probe (`/tmp/s1/probe_espn.py`, python3.13):
  robots-kapu `(True, 'robots engedett')` (`/soccer/scoreboard`);
  20261111: 75 esemény / 16 liga, PL 0 (válogatott szünet);
  20261108 (adaptáció): 58 esemény / 16 liga, PL 5, FA Cup jelen;
  dátum-visszhang őr mindkét napon zöld; `__espnfitt__` JSON kinyerve
  (v1 `window['__espnfitt__']` regex + `gmsByLeague[].evts[]` útvonal).
- [x] Linkelhetőség-mérés (offline exact-normalized join a
  `2627_E0.csv`-re, 100 sor / 20 csapat): **PL event-szint 2/5 = 40%,
  név-szint 6/10 = 60%**; 4 alias (Bournemouth, Brighton, Ipswich,
  Man United) → **5/5 = 100% offline plafon**; összliga név-átfedés
  7/110 (6,4%); **PL-eredmények 5/5 egyeznek az E0-val**. A v1 11%-a a
  globális feed szerepre szólt — PL-keresztforrás szerepben nem blokkoló.
- [x] AET/pen rés élőben igazolva: 3 FA Cup esemény 2026-11-08-án
  (1 „After Penalties”, 2 „After Extra Time”) → 90 perces konvenció,
  ET/pens csak explicit forrásból; FA Cup jelenléte Task 12 bemenet.
- [x] Dekoy-tesztek offline OK (8 eset: üres lista, PL kliens-shell,
  ESPN nincs/hibás JSON, üres node, v1 regex szintetikus mintán,
  szigorú norm_name) — hálózati kérés nélkül.
- [x] Katalógus: `pl-official` bejegyzés **`javasolt`** + decoy-jegyzet
  (rang 10; hibrid szállítás: sérülés obscura, eltiltás rust-http;
  30/nap 20+25 s; magas bizalom a hiányzó-tényre, display-only) és
  `espn` bejegyzés **`igazolt`** (rang 11; aggregator, közepes;
  linkelhetőség % mérve; AET/pen rés; ToS-review adoptáció előtt).
  Magyar ékezetes visszaolvasás + glyph-scan kész, záró újsor megvan.
- [x] Budget: **pl-official 3/10** (robots + 2 oldal, 20/25 s),
  **espn 3/10** (robots + 2 nap, 6 s); 403/429 nem történt, retry nem
  volt; minták sha256-tal a katalógusban és a jelentésben.
- [x] Jelentés: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-11-report.md`.

Következő döntési pont: a Task 11 szelet lezárult; a következő tervfeladat
(Task 12, FA Kupa-rés probe) csak felhasználói engedéllyel indul. A PL
sérülés-domain obscura-szállítása és az ESPN ToS-review adoptációs döntés.
Commit nem készült.

## S1 — W2 Task 12 checkpoint (FA Kupa-rés probe, 2026-11-11)

Kiindulás: W2 Task 8–11 lezárva; a Task 12 a FA Kupa lefedettségi rés
probe-ja (thefa / Sky / Soccerway / Tipplap) a worldfootball 403-as
elvetése után. Harness kötött, minden spike-parancs python3.13, minták
csak /tmp/s1/samples/ — nyers payload a repóba NEM került; DB-írás NEM
történt, v1 csak olvasás (Engine CDP-út, `cur=None`).

- [x] Step 1 robots-kapuk (harness, budgetbe számított): thefa
  `/competitions` `(True, 'robots engedett')`, sky `/football/fixtures`
  `(True, 'robots engedett')`, soccerway `/matches/`
  `(True, 'robots engedett')`; robots-bizonyíték forrásonként mentve
  (thefa 1513 B, sky 2175 B, soccerway 2553 B) → kiegészítő útvonalak
  offline kapuja (thefacup{,/fixtures,/results}, `/fa-cup-scores-fixtures`,
  `/england/fa-cup/` engedett).
- [x] Step 2–3 probe-ok (`/tmp/s1/probe_facup.py`, `probe_facup_round2.py`):
  thefa index/hub csak navigáció; **fixtures SSR 20 meccs-sor** (Second
  Round, dec. 5, `td.cOne`–`cSix`, hibás `time datetime` → szöveges parse);
  **results Opta-widget decoy (0 sor)**. Sky `/football/fixtures`
  **JS-shell** (a brief `fixres__item|match-head` regexe 4 JS-modulnév-
  találat = HAMIS ZÖLD, offline reprodukálva); a két nested kupa-útvonal
  **404** (3 kérés, retry nélkül); hub-discovery (`/fa-cup`) → helyes
  **`/fa-cup-scores-fixtures`** 200 (674 952 B): `data-match-id` +
  `data-state` JSON parse → **40 FA Cup meccs, mind `post`**, 7 AET +
  2 pens summary, **ESPN↔Sky 4/4 kereszt-egyezés**. Soccerway plain HTTP
  **decoy** (18 nav-testid, 0 meccs-link) → **Obscura** (v1 Engine
  read-only wrapper, elő-foglalással): `/matches/` 2305 data-testid,
  `/england/fa-cup/` 1403 data-testid, **118 meccs-sor**. Tipplap offline
  (Task 4 minta): 12 liga, 6 kupa, **FA Cup NINCS**; ESPN offline
  (Task 11 minta): 4 FA Cup esemény, 3/4 AET/pen.
- [x] Step 4 katalógus + visszaolvasás: `facup-gap` bejegyzés a
  Források szekció végére (Bejegyzés-séma + spec §3.1); valódi szó-szintű
  magyar visszaolvasás (1 idegen-glyph injektálás javítva), záró újsor
  megvan; **12/12 mintahash egyezik** a katalógus és a lemez között
  (független újraszámítás).
- [x] Budget (harness-ledger, 2026-11-11): **thefa 6/10, sky 8/10,
  soccerway 7/10** — utóbbi tartalmazza a **4-es elő-foglalást**
  (2 obscura URL × 2; tényleges fő dokumentum 2, foglalás konzervatív,
  nem kerül visszavezetésre; Task 5-tanulság: reserve → run → log).
  Túllépés NINCS; 403/429 nem történt; sky 404: 3 (2 útvonal + 1 ismétlés
  a minta-cache csak-200-as viselkedése miatt). Delay: thefa 8 s, sky 8 s,
  soccerway 10 s, robots 4 s.
- [x] Jelölt-státuszok: **Sky `igazolt`** (rust-http, 40 meccs, AET/pens
  explicit); **thefa fixtures `igazolt` / results `halasztott`**
  (Opta-decoy); **Soccerway `halasztott`** (obscura-transport igazolt,
  identity/év/TZ/ToS rések); **Tipplap FA Cup `nem igazolt`** (napfüggő);
  **ESPN `igazolt`** (Task 11, keresztforrás).
- [x] Döntés-előkészítés (S1 kilépési review, Task 15): **Sky elsődleges
  FA Cup-forrás** + ESPN kereszt + thefa hivatalos fixture/round-dates
  kiegészítő; Soccerway obscurás tartalék a v2 Obscura-integráció után,
  ha a hármas rései indokolják; 90 perces konvenció minden
  eredmény-forrásnál kötelező őr (AET/pens skip).
- [x] Jelentés: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-12-report.md`
  (parancsok + kimenetek, minta-sha256 lista, 9 élő adaptáció, aggályok).

### Task 12 fix-kör (review 3 Important lelete, offline, 0 kérés)

- [x] **1. lelet — Soccerway-felosztás + pens**: offline újra-parse a
  `facup_obscura.html`-en: 196 `wcl-tableScore` = **148 plain numerikus
  + 8 penalty `<sup>` cella + 40 „-”** → **78 lezárt + 20 jövőbeli**
  (a korábbi 74/24 a 8 penalty cellát „-”-ként számolta). A finding
  „4 penalty meccs” részletét a bizonyíték felülírta: a 8 cella
  **2 különálló meccs × 2 nézet** (lista + sorsolási ág): Stafford
  3(2)–3(3) Grimsby, Altrincham 2(2)–2(3) Notts County, `Pen`
  stage-címkével, Sky 2 pens-summary + ESPN kereszt-egyezés; a
  „Harborough–Stafford”/„Sutton–Altrincham” párosítás NEM létezik a
  mintán (Leyton Orient 5–1 Harborough, Guiseley 1–0 Sutton). AET:
  `extraTime` marker 0, de **5 explicit `AET` stage-címke** → meccs-szintű
  ET jelölt (a finding „AET nem igazolt” részlete pontosítva). Jelentés
  §2.5/§6/§8.4 + katalógus (4 hely + fix-kör jegyzet) javítva.
- [x] **2. lelet — Sky őr perzisztálva**: `/tmp/s1/parse_sky_guard.py`
  (`data-match-id` + `data-state` JSON + `competition.name.full` szűrő +
  `guard_rows` + decoy önteszt). Önteszt ZÖLD: kupa-oldal 40 sor,
  40/40 ID egyezik a `facup_matches.json`-nal; szintetikus JS-shell,
  mentett shell és üres payload → `DecoyError`. A `probe_facup.py` és
  `probe_facup_round2.py` guardja az importált parserre cserélve
  (hálózati útvonalak változatlanok); offline ellenőrzés blokkolt
  `fetch`-csel: shell fail-closed, kupa-oldal 40 sor zöld, round2
  minta-cache-ből végigfut, budget.json változatlan.
- [x] **3. lelet — checkpoint**: ez a Task 12 szekció hozzáfűzve
  (append-only, controller-ruling: AGENTS §4 kötelező checkpoint).
- [x] Deferred minor leletek (12/12 hash-szám, byte-szintű overclaim,
  First Round Proper következtetés) NEM módosultak; magyar visszaolvasás
  + záró újsor ellenőrzés a fix-körben is kész. Commit NEM készült.

Következő döntési pont: a Task 12 szelet (fix-körrel) lezárult; a
következő tervfeladat (Task 13, W3 mélység-probe-ok) csak felhasználói
engedéllyel indul. A FA Kupa forrás-döntés az S1 kilépési review-n
(Task 15). Commit nem készült (felhasználói engedélyhez kötött).

### Task 13 — W3 mélység-probe-ok (StatsBomb, reep, ClubElo, Wikidata, FM27)

- [x] Robots-kapuk: `raw.githubusercontent.com` 404 = „nincs robots”
  jegyzet (statikus host, nem tiltás); `data.reep.football` robots 200,
  engedett; `www.wikidata.org` SAJÁT robots-ítélet: `User-agent: *` →
  `Disallow: /w/` → `/w/api.php` tiltott (a commons Task 8 verdict NEM
  újrahasznált, de az eredmény azonos).
- [x] Probe-ok python3.13-mal (`/tmp/s1/probe_depth.py`, harness-kötött):
  StatsBomb `competitions.json` 200, 80 verseny-szezon sor / 24 verseny,
  demo-kör: PL 2, La Liga 18, Serie A 2, Bundesliga 2, Ligue 1 3, NB I 0
  szezon (aktuális szezon nincs → történeti xG-szerep); reep
  `latest.json` + v4 `release.json` guard zöld (stamp 2026-10-19.1,
  `matches.csv.gz` 38 793 475 B, kiadói sha256 a manifestban); ClubElo
  README 200 (provenance: observed ≤ 2025-06-01 / provisional
  ≥ 2025-06-15), `EloRatings.csv` 200 de >5 MB → harness-stop,
  CSV-tartalom nem mérve; Wikidata API-hívás NEM történt (robots-stop).
- [x] FM27: offline licenc-ellenőrzés (v1 `scrappers/fm27/README.md`,
  read-only): licencelt játékállomány, nem terjeszthető, 0 hálózati kérés.
- [x] Offline decoy-teszt: `guard_rows([], 'depth-decoy')` → DecoyError, OK.
- [x] Budget (napi 10/forrás): statsbomb 2, reep 3, clubelo 4, wikidata 2;
  delay ≥2 s (robots 4 s); 403/429 nem fordult elő, breaker érintetlen.
- [x] Öt katalógus-bejegyzés (rang 12–16): statsbomb `igazolt`
  (próba-szintű), reep `igazolt` (próba-szintű: manifest), clubelo
  `halasztott` (CSV >5 MB), wikidata `elvetett` (robots, döntési
  jegyzettel: SPARQL/dump alternatívák nem próbáltak), fm27 `halasztott`
  (offline licenc kész, adoptáció felhasználói döntés). Licenc-jegyzetek:
  StatsBomb attribúció + non-commercial; reep CC0 strong rung only;
  ClubElo tükör: nyílt licenc nem deklarált, citálás + provenance kötelező;
  Wikidata CC0; FM27 licencelt.
- [x] Magyar visszaolvasás (szó szintű) + záró újsor ellenőrzés kész;
  két nyelvi javítás a visszaolvasás után („NINCSENEK MÉRVE”,
  „forrás-megjelölést”).
- [x] Jelentés: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-13-report.md`
  (parancsok + kimenetek, budget, adaptációk, sha256, licenc-leletek, aggályok).

Aggályok: a reep `latest.json` stampje (2026-10-19.1) ÖREGEBB a v1 által
2026-11-01-én mért 2026-10-31.1-nél → heti kadencia nem garantált,
stamp-regressziót az adapternek kezelnie kell; a ClubElo tükör CSV-je
és a reep cut a harness 5 MB-os plafonja miatt csak manifest/README-szinten
igazolt; a Wikidata-elvetés alternatív útvonalról (SPARQL/dump) a
felhasználó dönthet a Task 15 review-n.

Következő döntési pont: a Task 13 szelet lezárult; a következő tervfeladat
(Task 14, W4 hozzáférési mátrix) csak felhasználói engedéllyel indul.
Commit nem készült (felhasználói engedélyhez kötött).

### Task 14 — W4 hozzáférési mátrix (API-Football, football-data.org, The Odds API, SSR-probe-ok)

- [x] **Robots-kapuk** (napi cache, harness): api-football robots.txt
  **403** → StopSpike, nincs retry; football-data.org és the-odds-api
  robots **404** = „nincs robots” jegyzet (intended-use API-host,
  openfoot-precedens); bundesliga, guardian, sky robots **engedett**.
- [x] **Auth-fal probe-ok kulcs NÉLKÜL** (`/tmp/s1/probe_access.py`,
  python3.13, harness-kötött): football-data.org
  `GET /v4/competitions/PL/matches` → **403** (152 B hiba-JSON:
  „restricted … check your subscription”); the-odds-api
  `GET /v4/sports/soccer_epl/odds` → **401** (`MISSING_KEY`);
  api-football: a robots 403 miatt a fixtures végpont probe-ja NEM
  futott (nincs endpoint-bizonyíték). **Regisztráció NEM történt, kulcs
  NEM készült**; mindhárom forrás státusza `regisztráció szükséges` +
  stop + felhasználói kérdés (Task 15). Kulcs nélküli 200 nem fordult
  elő (a decoy-test-őr erre az esetre offline tesztelve).
- [x] **SSR-probe-ok** (robots first, delay ≥6 s): bundesliga
  `/en/bundesliga/matchday` 200 (812 572 B) → **9 ld+json SportsEvent**
  (9. forduló, 2026-11-06→08, csak fixture, eredmény nincs); guardian
  `/football/results` 200 (300 940 B) → **41 gu-island props eredmény**
  (2026-11-08→10; demo-kör mind az 5 ligája megvan: PL 5, Bundesliga 3,
  Serie A 5, La Liga 5, Ligue 1 6 + FA Cup 5, UWCL 5, WSL 5, Scottish 2);
  sky `/football/results` 200 (931 348 B) → **14 data-match-id kártya,
  14/14 data-state JSON parse** (alsóbb ligás versenyek, pre/post).
- [x] **Erősített tartalmi őrök (élő adaptáció)**: a brief regexei hamis
  zöldek voltak — bundesliga 27 találat = Angular CSS-osztálynevek,
  guardian 22 találat = `--football-match-*` CSS-változók (Task 12
  anti-pattern). Csere: ld+json SportsEvent parse, illetve gu-island
  props JSON parse; sky: data-state parse. Az erősített őrök a mentett
  mintákon **offline** futottak (`--verify`, 0 új kérés): 9 / 41 / 14 sor
  zöld. Offline decoy-tesztek (5 eset): üres guard_rows, auth-osztályozás
  (401/403/429/200), 200-as decoy-test, üres/JS-shell SSR, **CSS-shell
  hamis-zöld reprodukció** → mind fail-closed DecoyError.
- [x] **Ingyenes csomagkeretek — docs-bizonyíték, NEM spike**:
  API-Football 100 kérés/nap, `x-apisports-key` header (hivatalos
  pricing/docs search-snippet; közvetlen fetch 403); football-data.org
  free 12 verseny + 10 hívás/perc + késleltetett eredmények,
  `X-Auth-Token` (pricing + policies oldal); The Odds API free 500
  kredit/hó, 1 kredit/régió/piac, `apiKey` query-paraméter (pricing +
  v4 guide). Fizetős sávok a katalógusban.
- [x] **Budget (napi 10/forrás)**: api-football 1/10, football-data-org
  2/10, the-odds-api 2/10, bundesliga 2/10, guardian 2/10, sky 10/10
  (8 Task 12/13 + 2 új). 429 nem fordult elő; breaker érintetlen.
  MLSsoccer: **0 kérés** (demo körön kívül, csak katalógus-jegyzet).
- [x] **Katalógus**: 6 bejegyzés (api-football, football-data-org,
  the-odds-api `regisztráció szükséges`; bundesliga, guardian
  `halasztott` próba-szintű SSR; mls-note `javasolt` jegyzet) +
  „Hozzáférési mátrix (W4)” fejezet (forrás × kulcs nélkül / ingyenes
  regisztráció / fizetős, docs-keretekkel, Sky W4-kiegészítéssel,
  döntési pontokkal). Minta-sha256: football-data-org hiba-test
  `2ed7e446…d3c44`, the-odds-api hiba-test `2141792e…aa09`, bundesliga
  `6a0fe5e2…d313`, guardian `89ce3f47…b339`, sky `ab422e44…faa7`
  (teljes hash a katalógusban).
- [x] **Magyar visszaolvasás (szó szintű)**: 6 javítás — 2 idegen
  glyph-beszúrás („历史”, „很快”), 1 idegen szó („possibility”),
  1 hiányzó szóköz („kitöltve,Fixture”), 2 nyelvtani („a ld+json” →
  „az ld+json”, „Guardinen” → „Guardian-oldalon”); nem-ASCII szkenner
  a hozzáfűzött szakaszon: tiszta; záró újsor OK.
- [x] **Jelentés**: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-14-report.md`
  (parancsok + kimenetek, budget, adaptációk, sha256, docs-leletek,
  aggályok).

Aggályok: az API-Football host-szintű 403-ja (robots + web + docs)
regisztráció előtt elérhetőségi próbát igényel — a kulcsos API sem
biztos, hogy elérhető erről a hálózatról; a football-data.org docs
100 kérés/24 h-t ír regisztrálatlan kliensre, de a spike 403-at kapott
(ellentmondás, kulcsos próba nélkül nem feloldható); a Guardian ToS
scrape-politikája NEM ellenőrzött (robots enged; BBC-precedens miatt
adoptáció előtt kötelező); a Sky results-pillanatkép a demo-kör nagy
ligáit nem tartalmazta (a FA Cup-lefedettség a Task 12 mintán igazolt);
a Bundesliga ld+json csak fixture (eredmény-domain rés).

Következő döntési pont: a Task 14 szelet lezárult; a következő
tervfeladat (Task 15, S1 kilépés + review-kapu) csak felhasználói
engedéllyel indul. Commit nem készült (felhasználói engedélyhez kötött).

### W2–W4 záró összegzés (S1 kutatási hullámok, 2026-11-11)

A W2–W4 hullámok minden forrása státusszal és bizonyítékkal (vagy
halasztási indokkal) lezárult; a nyitott kérdések a Task 15
review-kapuja elé kerülnek. Állapot a katalógus szerint:

- **W1 (gerinc, Task 2–7):** footballdata `igazolt`; thesportsdb
  `regisztráció szükséges`; tipplap `igazolt`; fotmob `igazolt`;
  transfermarkt `igazolt`; worldfootball `elvetett` (403-blokk).
- **W2 (meta/display + API + újraértékelés, Task 8–12):** logos-github
  `javasolt` (technikailag zöld, licenc hiányzik); logos-wikimedia
  `elvetett` (robots-tilt); pl-official `javasolt` (sérülésoldal
  decoy-200, eltiltás-domain zöld); espn `igazolt` (újraértékelés);
  openfoot `igazolt`; livescore-mcp `javasolt` (robots-tilt az API
  útvonalon, felhasználói döntési pont); facup-gap döntés-előkészítés
  (Sky `igazolt` 40 FA Cup meccs, ESPN kereszt, thefa, Soccerway
  Obscura).
- **W3 (mélység, Task 13):** statsbomb `igazolt` (próba-szintű); reep
  `igazolt` (próba-szintű: manifest); clubelo `halasztott` (CSV >5 MB);
  wikidata `elvetett` (robots `/w/` tilt, SPARQL/dump alternatíva
  felhasználói döntés); fm27 `halasztott` (offline licenc kész).
- **W4 (hozzáférési mátrix, Task 14):** api-football
  `regisztráció szükséges` (robots 403 stop, endpoint nem probe-olt);
  football-data-org `regisztráció szükséges` (403); the-odds-api
  `regisztráció szükséges` (401); bundesliga `halasztott` (SSR zöld,
  9 SportsEvent); guardian `halasztott` (SSR zöld, 41 eredmény);
  sky `igazolt` (Task 12/13 + W4 SSR-kiegészítés); mls `javasolt`
  (jegyzet, demo körön kívül, 0 kérés).

**Nyitott felhasználói döntések (Task 15):** regisztrációk
(API-Football elérhetőség + kulcs, football-data.org free, The Odds API
free, TheSportsDB), livescore-mcp robots-override kérdése, Guardian
ToS, logos-github licenc, wikidata alternatíva, FA Kupa forrás-döntés,
bizalmi mátrix + adoptációs sorrend jóváhagyása, séma-rés lista
(S2 spec bemenet), S2 spec indítás.

**S1 kilépési kritériumok állása (spec 6.2):** W1+W2 minden forrása
státusszal lezárult, de NEM mind `igazolt`/`elvetett` — a `javasolt` és
`regisztráció szükséges` státuszok felhasználói döntésre várnak; a
bizalmi mátrix és az adoptációs sorrend jóváhagyása, valamint a
regisztrációs kérdések külön döntési pontként a Task 15 napirendje.
Commit nem készült (felhasználói engedélyhez kötött).

## S1 — Task 15 záró checkpoint (S1 kilépés, review-kapu, 2026-11-11)

**Szelet:** S1 kilépés — szintézis a meglévő katalógus-bejegyzésekből,
checkpoint-szekciókból és task-reportokból. **Új hálózati kérés NEM
történt, új spike NEM futott, DB-írás NEM történt, commit NEM készült,
subagent NEM indult.** Meglévő bejegyzések státusza és rangszáma
változatlan (csak összegző fejezetek hozzáadva).

### Elkészült deliverable-ök (append-only)

- [x] `docs/research/source-catalog.md` — 5 új fejezet:
  „Bizalmi mátrix (jóváhagyásra)” (spec §3.2 frissítve, minden cella
  igazolt/tervezett jelöléssel, kétforrásos odds-szabály, display-only
  domainek), „Adoptációs sorrend (jóváhagyásra)” (rang 1–16 a
  rögzítettek szerint + státusz-feltételek + javasolt S3+ szelet-sorrend
  érték × bizonyíték × költség indoklással), „Séma-rés lista (S2 bemenet)”
  (18 konszolidált, deduplikált tétel forrás-hivatkozással és javasolt v2
  céllal, migrációterv nélkül + lefedettségi rés jegyzet), „Döntési
  pontok (S1 záró review)” (16 pont opciókkal és javaslattal), „S1
  kilépési kritériumok” (spec §6.2 pontról pontra, őszinte állással).
- [x] Ez a záró checkpoint-szekció (append-only) + a :477 pointer
  (lentebb).
- [x] Jelentés: `.superpowers/sdd/2026-11-11-real-data-sources-research/task-15-report.md`
  (módszer: mely bemenet mely fejezetet táplálta; visszaolvasási
  bizonyíték; aggályok).

### Hullámok záró állapota (a katalógus szerint, változtatás nélkül)

- **W1 (Task 2–7):** footballdata/tipplap/fotmob/transfermarkt
  `igazolt`; thesportsdb `regisztráció szükséges` („3” kulcs 403, free
  limit 15 esemény); worldfootball `elvetett` (403, spike-szintű).
- **W2 (Task 8–12):** openfoot/espn `igazolt`; sky `igazolt` (FA Cup
  40 meccs); logos-github `javasolt` (licenc null); logos-wikimedia
  `elvetett` (robots); pl-official `javasolt` (sérülés decoy-200,
  eltiltás igazolt); livescore-mcp `javasolt` (robots-döntési pont);
  facup-gap döntés-előkészítés kész (Sky + ESPN + thefa javaslat).
- **W3 (Task 13):** statsbomb/reep `igazolt` (próba-szintű); clubelo
  `halasztott` (CSV >5 MB, licenc-review); wikidata `elvetett` (robots,
  SPARQL/dump alternatíva döntés); fm27 `halasztott` (offline licenc).
- **W4 (Task 14):** api-football/football-data-org/the-odds-api
  `regisztráció szükséges`; bundesliga/guardian `halasztott` (SSR
  próba-szint); sky W4-kiegészítés igazolt; mls `javasolt` jegyzet.

### Incidensek és tanulságok (összegzés, a részletek a fenti szekciókban)

1. **fotmob 12/10 budget-túllépés** (Task 5): 8 ledger-kérés + 4
   ledgeren kívüli v1-probe kérés — szabálysértésként rögzítve, aznap
   több fotmob kérés nem történt. **Szabály (controller-ruling):
   minden ledgeren kívüli mérésnek ELŐRE el kell számolnia a várt
   kéréseket a harness-budget ellen (foglalás → futtatás → naplózás)**;
   a Task 12 soccerway-obscura mérés már ezzel futott (4-es elő-foglalás).
2. **transfermarkt véletlen duplikált probe-futás** (Task 6):
   import-mellékhatás, 2 extra kérés (5/10, budgeten belül); a probe
   azóta `__main__`-guardolt — minden későbbi probe ezt a mintát követi.
3. **Keychain-kapu** (Task 9): a delegált Keychain-írást az
   engedélyrendszer háromszor blokkolta (csak a felhasználó SAJÁT
   üzenetében megnevezett írás engedett); megkerülés nem történt; a
   migrációt a controller végezte a fő sessionben, felhasználói
   jóváhagyással (2026-11-11T13:54:04Z). A spike csak olvasott.
4. Egyéb budget-jegyzetek: logos-github 10/10 (robots-duplikáció a napi
   cache előtt), sky 404-ismétlés (minta-cache csak-200), sky W4 10/10;
   403/429-breaker mindenhol érintetlen, retry sehol nem történt.

### Pointer-korrekciók (append-only, régi sorok NEM szerkesztve)

- **:477 „identity-rés” korrekció (controller-ruling, Task 12 fix-kör
  2):** a fenti Task 12 szekció 477. sorának „identity/év/TZ/ToS rések”
  szóhasználata TÖRTÉNETI — a Soccerway identity-rés a fix-kör 2-ben
  megszűnt: a mentett `facup_obscura.html` mintán **58 `eventRowLink`
  href / 58 különböző mid** mérve (offline, 0 kérés), a crosswalk
  mid/slug exact-match-csel megvalósítható (fuzzy tilos). Fennmaradó
  rések: év nélküli idő, nem ellenőrzött TZ, ToS, mid szezonok közti
  stabilitás. Aktuális állapot: katalógus `facup-gap` bejegyzés +
  task-12-report.md §11.
- **Deferred-minor roll-up:** a taskonkénti halasztott minor leletek
  teljes listája a progress-naplóban él:
  `.superpowers/sdd/2026-11-11-real-data-sources-research/progress.md`
  („minor (deferred)” sorok, Task 1–14). Ezek nem blokkolják az S1
  zárást; az adapter-szeletek és az S2 spec a saját hatókörükben
  veszik át őket.

### Futtatott ellenőrzések (Task 15, mind offline)

```bash
grep -c "igazolt\|elvetett\|regisztráció szükséges\|halasztott" docs/research/source-catalog.md
# → 154 (minden bejegyzés státusszal; W3/W4 halasztási indokkal)
grep -n "^## Bizalmi mátrix\|^## Adoptációs sorrend\|^## Séma-rés\|^## Döntési pontok\|^## S1 kilépési" docs/research/source-catalog.md
# → 2305 / 2341 / 2410 / 2508 / 2621 — ékezetes fejezetek megvannak
```

- [x] Valódi szó-szintű magyar visszaolvasás (Read tool, a teljes
  hozzáfűzött katalógus-szakasz 2305–2654): ékezetek épek; a glyph-scan
  1 CJK-beszúrást talált („粒度” → „granularitás”, javítva), 1 idegen
  szó („presentlegi” → „jelenlegi”, javítva), 2 nyelvtani javítás
  („élő igazolás … kell” → „az élő igazolás … történik”; „szabályai …
  mértek” → „egyeztetés … futott”). A javítások után ismételt scan:
  tiszta (a §/×/→/↔/…/≥ szándékos jelölés). Záró újsor megvan.
- [x] Checkpoint append-only: régi sorok nem módosultak (a :477
  korrekció külön pointer, fent).

### Kilépési kritériumok állása (spec §6.2 — részletes tábla a katalógusban)

1. W1+W2 minden forrása igazolt vagy explicit elvetett: **RÉSZBEN** —
   4+3 igazolt, 2 elvetett (spike-szintű), thesportsdb regisztrációs,
   logos-github/pl-official/livescore-mcp javasolt → explicit döntési
   pontok (1., 2., 3., 9., 14.), csendben lezárt forrás nincs.
2. W3/W4 státusz + bizonyíték vagy halasztási indok: **TELJESÜLT**.
3. FA Kupa döntés-előkészítés: **TELJESÜLT** (facup-gap + 13. pont).
4. Bizalmi mátrix + adoptációs sorrend jóváhagyása: **FÜGGŐBEN** —
   elkészült, felhasználói review-ra vár.
5. Séma-rés lista: **TELJESÜLT** (18 tétel + lefedettségi jegyzet).
6. OpenFoot-kulcs + regisztrációs kérdések külön döntési pont:
   **TELJESÜLT** (5., 1., 9., 14. pont).
7. S2 spec indítási döntés: **FÜGGŐBEN** — review-kapu.

### STOP — felhasználói review-kapu

**A felhasználó review-zolja a katalógust (bizalmi mátrix, adoptációs
sorrend, séma-rés lista, 16 döntési pont).** Jóváhagyás után indul az
S2 spec (ingest keretrendszer, külön brainstorming ciklus). Commit/push
csak külön kérésre. A következő tervfeladatra engedély nélkül NEM
lépünk tovább.

### Visszaállítási út (Task 15)

- A két repó-fájl append-only bővült (katalógus + checkpoint); a Task 15
  előtti állapot a git munkafa-diffből visszaállítható (`git restore`
  csak kifejezett engedéllyel). DB nem érintett; /tmp/s1 érintetlen.

## Napzárás (2026-11-11 este) — S1 commitolva, holnap S2

- A felhasználó döntése: **commit + push most**; az S2 tervezés és a
  döntési pontok megválaszolása **2026-11-12-én** folytatódik.
- S1 slice commitolva a `feat/source-research` ágon (katalógus, checkpoint,
  S1 terv); push originre. A spec (6ec5127) már korábban commitolva.
- A review-kapu NYITOTT marad: bizalmi mátrix + adoptációs sorrend
  jóváhagyása és a 16 döntési pont holnapra halasztva (felhasználói döntés).

### Holnap (2026-11-12) startlista

1. Döntési pontok megválaszolása a katalógus `## Döntési pontok` fejezete
   alapján (regisztrációk: TheSportsDB / api-football / football-data.org /
   The Odds API; Livescore robots-ellentmondás; licencek: logó / ClubElo /
   FM27 / Guardian ToS; Wikidata SPARQL/dump; worldfootball újra-probe;
   Soccerway Obscura-függőség).
2. Bizalmi mátrix + adoptációs sorrend (S3+ szelet-sorrend) jóváhagyása.
3. S2 brainstorming → spec → terv ciklus indítása a jóváhagyott mátrixszal
   és a 18 tételes séma-rés listával (Obscura-integráció, worker
   kvóta/breaker/ledger, bizalmi szintes gate, fail-closed identitás).
4. Spike-harness újrateremtése szükség esetén: a `/tmp/s1` scratch a
   napzáráskor törlésre kerülhet; a harness kódja a task-reportokban és
   a ledger rulingjaiban dokumentált (python3.13, robotparser, budget,
   decoy-őrök).

### Rulings rekord (az SDD workspace törlése előtt gitbe rögzítve)

1. Nincs git worktree: a probe-ok `../football-app` és `/tmp/s1` útvonalai
   miatt a munka a fő workspace-ben, feature branchen folyt.
2. Nincs commit a task-lépésekben (AGENTS); review-csomag munkafa-diffből.
3. Model selection nem elérhető a harnessben → session modell öröklődik.
4. Élő-adaptáció megengedett guardok mellett, reportban ÉS katalógusban rögzítve.
5. Keychain elő-engedélyezés a specből; a host gate csak felhasználó által
   nevezett írást enged → fő sessionbeli írás a felhasználó jóváhagyásával.
6. robots matcher `urllib.robotparser` (wildcard/$), fail-closed parse-hibára.
7. Minden kísérlet budget-logolva (oversize/timeout/hiba is); robots 403/429
   StopSpike-ot propagál.
8. Spike-futtatás python3.13 (a 3.9 robotparser-e nem kezeli a wildcardot;
   harness <3.13 alatt fail-closed deny).
9. robots-kapu belső delay 4,0 s (a spec delay-kötés kétértelműségének
   konzervatív feloldása).
10. Regisztrációs kérdések a Task 15 kapura batchelve; kulcs-próba engedély
    nélkül nem fut.
11. fotmob 12/10 túllépés szabálysértésként rögzítve; aznapi fagyasztás;
    ledgeren kívüli mérések ELŐZETES budget-elszámolása (reserve→run→log).
12. Lefutott+blokkolt spike státusza `elvetett` (legenda); újra-probe W4/adapter.
13. Keychain-írás a fő sessionben, a felhasználó saját üzenetében nevezett
    jóváhagyással (delegált idézet nem megy át a gate-en).
14. Task 12 checkpoint kötelező (AGENTS §4), append-only.
15. Soccerway „0 meccs-href” állítás hamis volt → 58 eventRowLink href/mid,
    crosswalk lehetséges exact-match-csel (fix round 2).
16. checkpoint :477 régi „identity-rés” szóhasználata történeti bejegyzés;
    append-only pointer korrigálja (58 href tény).
17. SDD workspace csak a commit után törölhető (git history nélküli állapotban
    a ledger az egyetlen bizonyíték-lánc) — ez a fejezet őrzi a rulingokat.
18. finishing-a-development-branch a felhasználói integrációs döntésig
    halasztva; a döntés megszületett (commit+push), a branch megtartva.
