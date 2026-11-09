# Football App v2 — Rendszertervezési specifikáció

**Dátum:** 2026-11-09  
**Állapot:** jóváhagyott tervezési alap, implementáció előtt  
**Bemenet:** `notes/11.09/start.md`, a v1 (`Documents/Code/football-app`) kód- és dokumentációvizsgálata, valamint a 2026-11-09-i tervezési egyeztetés  
**Cél:** a v1 jó megoldásainak megőrzése mellett olyan, alapjaiban új v2 rendszer tervezése, amelyben az adatmegbízhatóság, az identitás, a mérés és a karbantarthatóság első osztályú követelmény.

---

## 0. Vezetői összefoglaló

A v2 egy helyi, egygépes macOS alkalmazás. A háttérben egy Rust daemon fut, amely a forrásgyűjtést, az identitásfeloldást, a kanonikus adatkezelést, az ütemezett feladatokat és a helyi API-t működteti. A felület natív SwiftUI kliens. A PostgreSQL és a hozzá tartozó, tartalom-azonosítós objektumtár együtt az igazság forrása, de a kliens és a scrape-réteg nem kap közvetlen adatbázis-hozzáférést.

A v2 nem a v1 nézeteinek másolata. A termék első értéke az, hogy bármely meccs, csapat, játékos, verseny vagy dátum esetében megválaszolható legyen:

1. milyen adat van;
2. melyik forrásból származik;
3. mikor rögzítettük;
4. mennyire friss és teljes;
5. melyik a kanonikus érték, ha a források eltérnek;
6. mi hiányzik, elavult, karanténban van vagy konfliktusos;
7. mi használható biztonsággal elemzéshez;
8. milyen biztonságos művelet szükséges a hiány javításához.

A predikció és a fogadásszimuláció csak a megbízható adatalap után, külön fázisban és mérési szerződéssel indul. Valódi pénzes fogadás nem része a rendszernek.

---

## 1. Termékcélok, felhasználók és alapdöntések

### 1.1 Elsődleges felhasználók

- **Napi operátor:** látja, hogy a mai program, felállások, eredmények és oddsok megérkeztek-e; látja a hiányt, a forrásállapotot, a konfliktusokat, és biztonságosan tud újrapróbálni vagy javítani.
- **Elemző:** kanonikus entitásokat és tényeket kérdez le; minden értéket vissza tud vezetni forráshoz; as-of módon tud számolni; meg tudja különböztetni a hiányzó adatot a valós nullától.
- **Meccsnéző:** meccs-, csapat-, játékos- és versenyoldalon gyorsan, olvashatóan látja az összes rendelkezésre álló információt és annak minőségét.

### 1.2 Rögzített termékdöntések

- **Platform:** Rust mag + natív SwiftUI macOS kliens.
- **Üzemeltetés:** helyi, egygépes rendszer; a PostgreSQL, a daemon és a kliens ugyanazon a Macen fut.
- **Architektúra:** domain-first moduláris monolit.
- **Első mérföldkő:** megbízható adatalap és meccsközpont; pre-match és eredmények előbb, live később.
- **Adatmigráció:** tiszta v2 adatbázis, a v1-ből szelektív, bizonyítékalapú visszatöltés.
- **Predikció:** fokozatos, szerződéses piacbővítés; 1X2, OU 2.5 és BTTS az első körben; szöglet és lap később.
- **Botok:** papírszimuláció, elkülönített portfóliók, jóváhagyott stratégiák.
- **Önjavítás:** biztonságos L1 automatika és javaslatvezérelt emberi jóváhagyás; destruktív művelet automatikusan tilos.

### 1.3 Nem célok

- valódi pénzes fogadás, fogadóiroda-integráció vagy automatikus kifizetés;
- többfelhasználós SaaS, jogosultsági szerepkörökkel működő felhőplatform;
- mobilalkalmazás vagy webes kliens az első verziónál;
- globális, minden versenyt lefedő archívum az első mérföldkőben;
- LLM által generált tipp a predikciós modell helyett;
- a v1 összes nézetének mechanikus újrateremtése;
- a v1 kanonikus ID-inak vagy bizonytalan összevonásainak változatlan átvétele.

---

## 2. Termékelvek

1. **A bizonyíték előbb van, mint a kanonikus érték.** Nyers válasz nélkül nincs normalizált tény; normalizált megfigyelés nélkül nincs kanonikus érték.
2. **A hiányzó adat nem nulla.** A `NULL`, `no_data`, `missing`, `blocked`, `stale` és `conflict` külön jelentéssel bír a sémában és a felületen.
3. **A forrásidentitás nem kanonikus identitás.** A külső ID csak hivatkozás; a v2 saját, stabil belső azonosítót használ.
4. **A név bizonyíték, nem kulcs.** Azonos nevű entitás lehet különböző; az alias csak kontextussal oldható fel.
5. **Automatikus összevonás csak igazolt azonosság esetén.** Bizonytalan eset mindig review-ba kerül.
6. **Minden adat visszavezethető.** A megjelenített tény forrása, rögzítési ideje és parserverziója lekérdezhető.
7. **Minden számítás as-of.** Az előrejelzés, forma, H2H és szezonstatisztika nem használhat a kickoff után keletkezett információt.
8. **A forrás hozzáadása mérés és szerződés kérdése.** Nincs forrás minősítés nélkül, nincs policy-kerülés.
9. **A hibák nem tűnnek el csendben.** A nulla találat, a parse-eltérés, a konfliktus és a forráskiesés riasztást vagy karantént eredményez.
10. **A modell csak mérési kapuval kerülhet a felületre.** A „value” nem lehet bizonyítás nélküli becslés.
11. **A felület nem írhat közvetlenül adatbázist.** Minden művelet a helyi API-n, jogosultsággal, indoklással és naplóval megy át.
12. **A karbantarthatóság architekturális követelmény.** Egy migration-lánc, tiszta modulhatárok, kicsi fájlok, ismételhető ellenőrzések.
13. **Az ellenőrzés többrétegű és publikálás előtti.** Egy forrás vagy parser soha nem minősít egy tényt önmagában ellenőrzöttnek; a napi lista csak a publikálási kapun átjutott elemekből épül.

---

## 3. Termékhatár és információarchitektúra

### 3.1 Négy munkamód

1. **Ma**
   - dátumválasztó, prioritásos versenycsoportok, meccssorok;
   - állapot-, frissesség- és minőségjelvények;
   - a legfontosabb hiány és a következő biztonságos teendő;
   - billentyűzetes navigáció és gyors szűrés.
2. **Felfedezés**
   - versenyek, csapatok, játékosok, edzők, bírók, helyszínek és meccsek;
   - globális keresés és parancspaletta;
   - entitások közötti navigáció.
3. **Labor**
   - csak a 4. fázistól látható;
   - predikciók, value-tábla, modellek, kalibráció, stratégiák, papírszelvények és portfóliók.
4. **Üzemeltetés**
   - vizuálisan elkülönített operátori mód;
   - források, lefedettség, identitásreview, konfliktusok, karantén, futások, ügynökök, audit, mentések.

### 3.2 Stabil útvonalak

- `today/{yyyy-mm-dd}`
- `competition/{competitionId}/{seasonId}`
- `match/{matchId}`
- `team/{teamId}`
- `player/{playerId}`
- `manager/{managerId}`
- `referee/{refereeId}`
- `venue/{venueId}`
- `search?q={query}`
- `ops/overview`
- `ops/sources`
- `ops/coverage`
- `ops/identity`
- `ops/conflicts`
- `ops/runs`
- `ops/agents`
- `ops/audit`
- `ops/backups`
- `settings`

A navigációs állapot visszaállítható; a vissza/előre művelet megőrzi a dátumot és a szűrőket.

### 3.3 Első mérföldkő képernyői

- napi lista;
- versenyoldal;
- meccsoldal;
- csapatoldal;
- játékosoldal;
- globális keresés;
- üzemeltetési áttekintés, lefedettség, források, identitásreview, konfliktusok, futások, audit.

Az edző-, bíró- és helyszínoldal az entitásnavigáció része; első körben csak a rendelkezésre álló adatot mutatja, nem generál mesterséges tartalmat.

### 3.4 Oldalak tartalma

**Napi lista:** dátum, versenycsoport, hazai/vendég csapat, kickoff, állapot, eredmény, félidő, adatminőség, forrásfrissesség. A sor nem tartalmaz rejtett nullaértéket. A lista csak a publikálási kapun átjutott meccseket mutatja ellenőrzöttként; a függő vagy karanténba tett elemek az Üzemeltetésben, illetve külön „Ellenőrzés alatt” nézetben jelennek meg. A lista ellenőrzöttsége a lista-szintű kötelező tényekre vonatkozik: a hiányzó felállás vagy elérhetőség a meccsoldalon továbbra is külön hiányként látszik.

**Versenyoldal:** szezonválasztó, versenyformátum, fordulók/stádiumok, tabella vagy csoportok, következő és legutóbbi meccsek, szezonstatisztikák, adatlefedettség.

**Meccsoldal:**
- fejléc: csapatok, állapot, kickoff, eredmény, félidő, hosszabbítás/tizenegyes, helyszín, bíró, adatminőség;
- **Összefoglaló:** gólok, kártyák, cserék, fontos események, elhalasztás, érvénytelenítés, megismétlés;
- **Statisztika:** csapat- és játékosszintű, hiányjelöléssel;
- **Felállások:** formáció, kezdők, cserék, hiányzó játékosok és okuk;
- **Forma és H2H:** as-of számított nézetek;
- **Elérhetőség:** sérülés, eltiltás, betegség, várható visszatérés;
- **Odds:** nyitó, záró és további pillanatképek, piacok és irodák szerint;
- **Adat:** források, mezőszintű provenance, konfliktusok, lefedettség, hiányzó részek.

**Csapatoldal:** profil, ország, nem/korosztály/csapatág, hivatalos és social linkek, következő és legutóbbi meccsek, forma, keret, szezonstatisztika, elérhetőség, aliasok, források.

**Játékosoldal:** profil, állampolgárság, születési adat, pozíció, csapattagsági időszakok, szezon- és meccsstatisztika, elérhetőség, aliasok, források.

### 3.5 Adatőszinteségi felületi szerződés

Minden megjelenített állapot az alábbiak egyike:

- `Megfigyelt`
- `Számított`
- `Hiányzó`
- `Elavult`
- `Konfliktusos`
- `Karanténban`

A szín nem egyedüli hordozó; szöveg, ikon és tooltip is szükséges. A provenance-panel minden fontos értékhez egy kattintással elérhető. A hiányzó érték helyén a hiány típusa és a javítás lehetősége látszik, nem `0`, `0–0` vagy üres, félreérthető cella.

### 3.6 UX és akadálymentesség

- natív macOS világos/sötét mód, csökkentett mozgás és rendszerkontraszt tisztelete;
- WCAG 2.1 AA kontrasztcél;
- teljes billentyűzetes navigáció, fókuszcsapda dialógusokban, roving tabindex füleknél;
- VoiceOver-címkék minden interaktív elemen;
- színvakság-biztos állapotjelölések;
- magyar felület, `hu-HU` dátum-, szám- és odds-formázás;
- szűrők elmenthetők, a keresés aliasokra és elírásokra is működik.

### 3.7 UX-teljesítmény

- a napi listáról bármely entitás legfeljebb három lépésben elérhető;
- a meccs első képernyőjén látszik az eredmény, az állapot és az adatminőség;
- helyi API p95 < 150 ms a gyakori lekérdezéseknél;
- alkalmazás hidegindítás < 2 s;
- listák lapozottak és virtualizáltak;
- nincs végtelen spinner: betöltés, üres, hiba, daemon-offline és részleges adat külön állapot.

---

## 4. Rendszerarchitektúra

### 4.1 Repository-szerkezet

```text
football-app-v2/
  apps/FootballApp/            SwiftUI macOS kliens
  rust/
    Cargo.toml                 workspace
    crates/
      domain/                  tiszta entitások, értéktípusok, invariánsok
      storage/                 repository-k, SQL, migrációfuttatás
      ingest/                  forrásadapterek, rate limit, parserek
      identity/                aliasok, jelöltek, összevonási döntések
      verification/            szabálymotor, publikálási kapu, ellenőrzési leletek
      analytics/               as-of lekérdezések, mutatók
      prediction/              későbbi modell- és kalibrációs réteg
      simulation/              későbbi portfólió- és szelvénymotor
      service/                 use case-ek, tranzakcióhatárok
      api/                     helyi HTTP API, OpenAPI-szerződés
      worker/                  ütemezett feladatok, retry, lefedettség
      telemetry/               strukturált log, trace, audit-események
    bins/
      footballd                daemon: API + worker + egészségállapot
      footballctl              migráció, doctor, backfill, admin CLI
  db/migrations/               egyetlen, ellenőrzött migration-lánc
  tests/                       contract, integration, golden fixture
  docs/                        élő architektúra- és üzemeltetési dokumentumok
```

A repó gyökere nem tartalmazhat generált nyers adatot, több száz dátumozott riportot vagy a v1-hez hasonló dokumentumtömeget. A riportok, döntések és fixlogok kategorizált, dátumozott alkönyvtárba kerülnek.

### 4.2 Modulhatárok

- `domain` nem függ adatbázistól, HTTP-től, forrásadaptertől vagy UI-tól.
- `storage` csak repository-interfészeket valósít meg; a domain invariánsait nem kerüli meg.
- `ingest` nyers forrásadatot állít elő és normalizál, de kanonikus entitást nem írhat közvetlenül.
- `identity` csak jelöltet, döntést és visszafordítható merge-eseményt kezel.
- `verification` független szabályokat futtat a stage-, core-, identity- és idősoros adaton; nem a forrásadapter saját sikerjelentésére támaszkodik.
- `verification` három szigorúsági szintet ad: `blocker`, `warning`, `info`; a publikálási kapu csak a dokumentált kötelező szabályokat kényszeríti ki.
- `service` fogja össze a use case-eket és a tranzakcióhatárokat.
- `api` csak DTO-t és HTTP-szemantikát ad; üzleti szabályt nem duplikál.
- `worker` a service use case-eket hívja, nem saját SQL-t futtat.
- `prediction` és `simulation` a core adatot csak olvassa, saját ledgerbe ír.

### 4.3 Folyamatmodell

A `footballd` egy folyamat, benne API és worker-poolok. A nehéz böngészős gyűjtés külön, korlátozott élettartamú alfolyamatban fut. A Postgres kapcsolatok korlátozott poolban élnek. A daemon induláskor:

1. ellenőrzi a konfigurációt és a runtime-könyvtárat;
2. futtatja a szükséges migrációkat;
3. ellenőrzi a sémát és a kritikus indexeket;
4. elindítja az API-t localhoston, tokennel;
5. elindítja a worker-poolokat;
6. egészségállapotot publikál a kliensnek.

A kliens nem indítja el automatikusan a v1-et, nem olvas annak adatbázisából, és nem módosítja a v1 munkafájait.

### 4.4 Élettartam és hibaszigetelés

- minden worker-feladat időkorlátos;
- a worker-panic nem viheti magával a daemon egészét;
- a böngészős alfolyamat külön leállítható;
- a félbeszakadt lease lejárattal visszakerül;
- a forrásonkénti circuit breaker izolálja a hibát;
- a daemon `safe mode`-ban csak olvasási API-t és diagnosztikát nyújt.

---

## 5. Helyi API-szerződés

### 5.1 Alapok

- REST + JSON, `/v1` prefix;
- kizárólag `127.0.0.1`;
- rövid életű, helyi hozzáférési token;
- OpenAPI-szerződés és generált Swift kliens;
- kurzoros lapozás;
- egységes hibaformátum gépi kóddal és magyar magyarázattal;
- írási műveletnél idempotenciakulcs, aktor és indoklás;
- az üzemeltetési végpontok külön scope-ot igényelnek.

### 5.2 Olvasási végpontok

- `GET /v1/health`
- `GET /v1/day/{date}`
- `GET /v1/competitions`
- `GET /v1/competitions/{id}`
- `GET /v1/competitions/{id}/seasons`
- `GET /v1/competitions/{id}/seasons/{seasonId}`
- `GET /v1/competitions/{id}/seasons/{seasonId}/fixtures`
- `GET /v1/competitions/{id}/seasons/{seasonId}/standings`
- `GET /v1/matches/{id}`
- `GET /v1/matches/{id}/events`
- `GET /v1/matches/{id}/lineups`
- `GET /v1/matches/{id}/statistics`
- `GET /v1/matches/{id}/odds`
- `GET /v1/matches/{id}/provenance`
- `GET /v1/teams/{id}`
- `GET /v1/teams/{id}/fixtures`
- `GET /v1/teams/{id}/squad`
- `GET /v1/teams/{id}/form`
- `GET /v1/players/{id}`
- `GET /v1/players/{id}/appearances`
- `GET /v1/players/{id}/statistics`
- `GET /v1/managers/{id}`
- `GET /v1/referees/{id}`
- `GET /v1/venues/{id}`
- `GET /v1/search?q={query}&types={types}`
- `GET /v1/ops/overview`
- `GET /v1/ops/sources`
- `GET /v1/ops/coverage`
- `GET /v1/ops/conflicts`
- `GET /v1/ops/identity-candidates`
- `GET /v1/ops/runs`
- `GET /v1/ops/agents`
- `GET /v1/ops/audit`
- `GET /v1/ops/backups`
- `GET /v1/ops/verification/summary`
- `GET /v1/ops/verification/findings`
- `GET /v1/ops/verification/gates`

### 5.3 Írási végpontok

- `POST /v1/ops/conflicts/{id}/resolve`
- `POST /v1/ops/identity-candidates/{id}/decide`
- `POST /v1/ops/manual-observations`
- `POST /v1/ops/jobs/{id}/retry`
- `POST /v1/ops/jobs/{id}/cancel`
- `POST /v1/ops/raw/{captureId}/reparse`
- `POST /v1/ops/observations/{id}/quarantine`
- `POST /v1/ops/agents/{id}/run`
- `POST /v1/ops/agents/{id}/stop`
- `POST /v1/ops/backups/run`
- `POST /v1/ops/verification/gates/{id}/approve-provisional`
- `POST /v1/ops/verification/gates/{id}/reopen`
- `POST /v1/ops/verification/rules/{id}/activate`

Az `/v1/day/{date}` válasz elemenként hordozza a publikálási állapotot, a nyitott blokkolók rövid listáját és a forrásfrissességet. Minden írási válasz tartalmazza az érintett rekordokat, az auditazonosítót és a visszaállítási lehetőséget.

### 5.4 Hibamodell

```json
{
  "code": "CONFLICT_MATERIAL",
  "message": "A meccs kezdési idejében anyagi konfliktus van.",
  "requestId": "req_20261109_0001",
  "details": {
    "matchId": "3f9c2a6e-6e7b-4a4f-9b58-0b6d5f0c9a11",
    "field": "kickoff_at",
    "conflictId": "f7a2d8c0-0a88-45c1-84eb-6f74c0a47cda"
  }
}
```

A hiba soha nem tartalmaz nyers tokent, cookie-t, teljes nyers payloadot vagy személyes adatot.

---

## 6. Adatmodell

Az adatbázis logikai sémái: `raw`, `stage`, `core`, `identity`, `fact`, `ops`, valamint később `analytics`, `prediction` és `simulation`. Minden belső entitás stabil UUID-t kap; az UUID v7 használata előnyös az időrendezés miatt.

### 6.1 `raw` séma

- `raw.artifact`
  - `id`, `source_id`, `content_sha256`, `compression`, `byte_size`, `storage_key`, `media_type`, `created_at`
- `raw.capture`
  - `id`, `source_id`, `artifact_id`, `endpoint_key`, `request_url`, `requested_at`, `received_at`, `http_status`, `adapter_version`, `parser_version`, `request_key`, `redacted_headers`, `error_code`
- `raw.retention_policy`
  - `source_id`, `aspect`, `hot_days`, `cold_days`, `delete_after`, `policy_version`

A nyers tartalom a repón kívüli, tömörített, tartalom-azonosítós objektumtárban él. Az adatbázis csak metaadatot és kereshető normalizált megfigyelést tartalmaz.

### 6.2 `stage` séma

- `stage.observation`
  - `id`, `source_id`, `capture_id`, `entity_kind`, `aspect`, `natural_key`, `source_record_key`, `observed_at`, `event_time`, `normalized_payload`, `schema_version`, `quality_flags`, `content_hash`, `created_at`

A megfigyelés append-only. Javítás nem update, hanem új megfigyelés vagy revízió.

### 6.3 `core` séma

- közös entitásregiszter: `entity`
- referencia: `country`, `venue`, `bookmaker`
- verseny: `competition`, `season`, `competition_stage`, `competition_membership`
- csapat: `team`, `team_branch`, `team_parent_link`
- személy: `player`, `manager`, `referee`, `player_team_spell`, `staff_tenure`
- meccs: `match`, `match_result`, `match_period`, `match_event`
- összeállítás: `lineup`, `lineup_player`, `player_appearance`
- elérhetőség: `availability`
- statisztika: `stat_definition`, `team_match_stat`, `player_match_stat`, `season_stat`
- odds: `odds_market`, `odds_selection`, `odds_snapshot`
- kapcsolódó források: `external_resource`

#### 6.3.1 Meccs

`core.match` fő mezői:

- `id`, `competition_id`, `season_id`, `stage_id`, `round`;
- `home_team_id`, `away_team_id`;
- `kickoff_at` UTC-ben, `competition_timezone`, `kickoff_precision`;
- `venue_id`, `neutral_venue`, `referee_id`;
- `status`;
- `original_kickoff_at`;
- `rescheduled_from_match_id`;
- `replay_of_match_id`;
- `source_status`, `last_observed_at`.

Meccsstátuszok:

- `scheduled`
- `confirmed`
- `live`
- `halftime`
- `finished`
- `postponed`
- `canceled`
- `abandoned`
- `awarded`

`core.match_result` külön kezeli:

- `home_ft`, `away_ft`;
- `home_ht`, `away_ht`;
- `home_et`, `away_et`;
- `home_pens`, `away_pens`;
- `winner_method`: `regular`, `extra_time`, `penalties`, `awarded`;
- `validity`: `valid`, `void`, `awarded`, `abandoned`;
- `decided_at`, `reason`.

A hiányzó félidő, hosszabbítás vagy tizenegyes eredmény `NULL`, nem nulla. Az elhalasztott meccs eredeti és új időpontja is megmarad. A megismételt meccs új rekord, amely az eredetire hivatkozik.

`core.match_period` a félidők és hosszabbítások kezdését, végét, eredményét és ráadáspercét tárolja. `core.match_event` eseménytípusa lehet többek között `goal`, `own_goal`, `penalty_goal`, `yellow_card`, `red_card`, `substitution`, `var`, `shootout_goal`, `shootout_miss`; minden eseményhez periódus, perc, ráadásperc, csapat, játékos és forrásmegfigyelés tartozik.

#### 6.3.2 Csapat és játékos identitás

`core.team` és `core.player` közös `core.entity` rekordra hivatkozik. A csapatspecifikus mezők:

- ország;
- nem;
- korosztály;
- csapatág: `senior`, `reserve`, `youth`, `women`;
- szülőklub;
- aktív státusz.

A női, tartalék-, ifjúsági és felnőtt csapat külön entitás. A játékos csapattagsága `player_team_spell` rekord, kezdő és záró dátummal, kölcsönjelzővel. A jelenlegi csapat nem egyetlen felülírt mező, hanem az időben érvényes kapcsolatból számított nézet.

### 6.4 `identity` séma

- `identity.source_entity_ref`
  - `source_id`, `entity_kind`, `source_entity_id`, `entity_id`, `method`, `confidence`, `status`, `reviewed_by`, `evidence_observation_id`
  - egyedi: `(source_id, entity_kind, source_entity_id)`
- `identity.name`
  - `entity_id`, `name`, `normalized_name`, `language`, `script`, `country_scope`, `is_primary`, `valid_from`, `valid_to`, `source_ref`
- `identity.candidate`
  - `entity_kind`, bal oldali forrásrekord, jobb oldali kanonikus entitás, pontszám-összetevők, állapot, prioritás, létrehozás ideje
- `identity.merge_event`
  - `from_entity_id`, `to_entity_id`, `method`, `actor`, `reason`, `started_at`, `reverted_at`, `audit_id`

#### 6.4.1 Identitásszabályok

- Az automatikus kapcsolás alapértelmezett esete a pontos, ellenőrzött külső ID → kanonikus ID egyezés.
- Új forrás neve önmagában soha nem hoz létre automatikus egyesítést.
- Azonos normalizált név esetén is kötelező az ország, nem, korosztály, csapatág és versenykontextus ellenőrzése.
- Több lehetséges jelölt esetén `candidate` rekord készül, a forrásrekord nem kap kanonikus ID-t.
- Női/férfi, felnőtt/utánpótlás és klub/tartalék csapat közötti automatikus merge tilos.
- A merge visszafordítható; a megszűnt entitás nem törlődik, hanem `merged_into_id` hivatkozást kap.
- Az alias nem globálisan egyedi; nyelvhez, országhoz, kontextushoz és forráshoz kötött.
- A v1-ből átvett alias- és külső ID-kapcsolat csak jelölt, nem automatikusan elfogadott igazság.

### 6.5 `fact` séma

- `fact.provenance`
  - `subject_kind`, `subject_id`, `field_key`, `observation_id`, `is_selected`, `valid_from`, `valid_to`, `policy_version`
- `fact.conflict`
  - `subject_kind`, `subject_id`, `field_key`, `severity`, `status`, `winner_observation_id`, `opened_at`, `resolved_at`, `resolution_reason`
- `fact.verification`
  - `subject_kind`, `subject_id`, `field_key`, `rule_set_version`, `status`, `severity`, `last_run_id`, `valid_from`, `valid_to`

A `fact.verification.status` értékei: `unverified`, `verified`, `provisional`, `blocked`. Az ellenőrzési állapot mezőszintű, mert egy meccs lehet ellenőrzött azonosításban és kezdésidőben, miközben a felállása még hiányos vagy függő.

Minden megjelenített kanonikus tényhez tartozik provenance. A származtatott érték a bemeneteire hivatkozik. Materiális konfliktusnál a korábbi jóváhagyott érték marad, a konfliktus látható, és a következő azonos prioritású megfigyelés nem írja felül csendben.

### 6.6 `ops` séma

- `ops.source`
- `ops.source_capability`
- `ops.source_policy`
- `ops.adapter_version`
- `ops.job`
- `ops.job_attempt`
- `ops.coverage_slot`
- `ops.circuit_breaker`
- `ops.alert`
- `ops.incident`
- `ops.decision_log`
- `ops.audit_event`
- `ops.agent_role`
- `ops.agent_run`
- `ops.agent_action`
- `ops.approval`
- `ops.backup_record`
- `ops.validation_rule`
- `ops.validation_run`
- `ops.validation_finding`
- `ops.publish_gate`
- `ops.correction_guard`

Az audit és döntésnapló append-only. A napló nem tartalmazhat titkot, cookie-t vagy teljes nyers payloadot. Az `ops.correction_guard` a visszatérő hibaosztályokat, a hozzájuk tartozó ellenőrzési szabályt és az esetleges kapuszigorítást követi.

### 6.7 Későbbi sémák

- `analytics`: as-of nézetek, forma, H2H, szezonmutatók, anyagiasított nézetek.
- `prediction`: piackatalógus, előrejelzés, kalibráció, kísérlet, backtest.
- `simulation`: portfólió, stratégia, szelvény, láb, settlement, CLV.

A `prediction` és `simulation` saját írási ledgerrel rendelkezik; a core adatot nem módosítja.

### 6.8 Adattípus- és időszabályok

- minden időpont `timestamptz`, UTC-ben;
- a helyi megjelenítéshez külön időzóna és formázás tartozik;
- a dátumintervallumos entitások `valid_from` / `valid_to` mezőt használnak;
- pénz egész egységben, odds legalább három tizedesjeggyel;
- a statisztika hosszú formátumú, definícióvezérelt;
- a hiány és a nulla külön érték;
- a meccs természetes kulcsa verseny + szezon + forduló/stádium + hazai és vendég entitás + UTC kickoff, de ez csak egyezésvizsgálatra szolgál, nem helyettesíti a belső ID-t.

---

## 7. Gyűjtés, források és lefedettség

### 7.1 Forrásszerződés

Minden adapter rendelkezik:

- forrásazonosítóval és adapterverzióval;
- támogatott entitás- és adatfajtákkal;
- endpoint- és kéréskulcs-szabályokkal;
- policy- és jogosultsági hivatkozással;
- rate-limit és párhuzamossági profillal;
- parser- és sémaverzióval;
- golden fixture-ökkel és contract-tesztekkel;
- ismert hibamódokkal és helyettesítési tervvel.

Forrás hozzáadása előtt kötelező minősítőlap készül: lefedettség, frissesség, stabilitás, identitásleképezési arány, költség, visszatöltési érték, licenc/policy kockázat, helyettesíthetőség.

### 7.2 Forrásminősítés és élesítés

1. jogi/policy ellenőrzés;
2. mintavételes adatfeltárás;
3. adapter- és parser-contract tesztek;
4. shadow-üzem;
5. canary-élesítés egy szűk körön;
6. teljes élesítés és folyamatos egészségfigyelés.

Shadow módban a forrás csak megfigyelést és jelöltet termel. Kanonikus adatot nem ír. Élesítés csak dokumentált visszaállítási tervvel történik. Nyugdíjazásnál a bizonyíték és a forrás története megmarad.

### 7.3 Scheduler

Ütemezett események:

- napi program-felfedezés;
- T−24h, T−3h, T−60m, T−15m frissítés;
- lefújás után T+5m, T+30m, T+2h, T+24h, szükség esetén T+72h;
- heti hiánypótlás;
- napi forrás-canary;
- heti schema-drift és minőségi riport.

A worker `due_at` szerint dolgozik, lease-pel, `FOR UPDATE SKIP LOCKED` szemantikával. A prioritás: közelgő kickoff → magas prioritású verseny → nyitott identitás- vagy konfliktusprobléma → hiányzó kötelező adat → archív backfill.

### 7.4 Rate limit és retry

- forrásonként és endpoint-típusonként külön token bucket;
- perzisztens napi kvóta és minimális kérésköz;
- jitter és exponenciális backoff;
- `Retry-After` tisztelete;
- 401/403 esetén azonnali leállítás és policy-ellenőrzés;
- parse-hiba esetén elsőként nyersből repars, nem vak újrakérés;
- circuit breaker nyitott/fél-nyitott/zárt állapottal;
- lejárt lease visszasorolása duplikáció nélkül.

### 7.5 Lefedettség

`ops.coverage_slot` kulcsa legalább: forrás, entitás, adatfajta, időablak. Állapotok:

- `pending`
- `fetching`
- `done`
- `no_data`
- `blocked`
- `failed`
- `stale`
- `quarantined`
- `skipped`

Kötelező adatfajták például: identitás, ütemezés, eredmény, félidő, felállás, esemény, csapatstatisztika, játékosstatisztika, elérhetőség, odds. A követelmény versenyosztálytól és meccsfázistól függ. A nulla találat nem `done` automatikusan.

### 7.6 Konfliktus és forrásprioritás

A prioritás kulcsa: entitás + mező + hatókör. Példák:

- hivatalos versenystátusz: hivatalos ligaoldal elsőbbsége;
- záró odds: fogadóiroda-pillanatkép;
- történeti eredmény: ellenőrzött archív forrás;
- felállás: a legfrissebb, közvetlen meccsforrás, eltérés esetén konfliktus.

Konfliktusállapotok:

- `benign`: formázási eltérés;
- `material`: kickoff, résztvevő, eredmény, identitás;
- `unresolved`;
- `resolved_auto`;
- `resolved_manual`.

Materiális konfliktus nem írja felül automatikusan a jóváhagyott értéket. A feloldás során minden forrásérték, nyers részlet, időpont és javaslat látszik. A döntés append-only és visszakereshető.

### 7.7 Kézi adat

A kézi javítás nem közvetlen `UPDATE`. `manual` forrású megfigyelést hoz létre:

- aktor;
- indok;
- bizonyíték vagy hivatkozás;
- érintett entitás és mező;
- előtte/utána diff;
- jóváhagyási állapot.

A megfigyelés ugyanazon validációs és provenance-láncon megy át, mint a gépi forrás.

### 7.8 Nyers tárolás és retention

- a nyers payload nem a repóban van;
- tömörített, tartalom-azonosítós objektumtár;
- azonos tartalom csak egyszer tárolódik;
- forrásonként és adatfajtánként hot/cold/delete policy;
- a kanonikus és auditadatok hosszú távúak;
- a konkrét méretkeretet az első v1 import és baseline mérés után kell rögzíteni;
- a cél, hogy a v1 20 GB-os, 30 ezer fájlos állapotához képest mérhetően kisebb, deduplikált és politikával kordában tartott lábnyom legyen.

### 7.9 Forrásjelöltek az első körben

Minden forrás csak a minősítés után használható:

- football-data.co.uk: történeti eredmények, statisztikák, szezonfájlok;
- SofaScore: meccsrészletek, felállások, statisztikák, tabellák, a policy betartásával;
- FotMob: kiegészítő meccskontextus és elérhetőség, policy szerint;
- Livescore / OpenFoot: napi program és eredmények;
- ESPN: kiegészítő forrás;
- Tipplap: odds-pillanatképek, piacok, eredmények;
- Transfermarkt: elérhetőségi és keretadatok, policy szerint;
- hivatalos liga- és kluboldalak: hivatalos státusz és elérhetőség;
- StatsBomb open data: opcionális történeti xG-visszatöltés, nem napi kritikus forrás.

A forráslista nem ígéret a teljes lefedettségre; a minősítés és a shadow-eredmény dönt.

### 7.10 Többrétegű ellenőrzési és publikálási lánc

A cél nem az, hogy külső forrásokból garantáltan hibátlan valóságot állítsunk elő, hanem hogy a napi lista csak olyan elemet mutasson ellenőrzöttként, amely minden kötelező kapun átment. Minden bizonytalanság explicit; semmi nem kerül csendben a listára.

Az ellenőrzés nem a forrásadapter „sikeres futás” jelentése. A nyers megfigyelés külön ellenőrzési láncon megy át, és a lánc minden lépése saját, verziózott szabályokkal, leletekkel és állapottal rendelkezik.

| Réteg | Mit ellenőriz | Hiba esetén |
|---|---|---|
| L0 – Átvétel | HTTP-státusz, tartalomtípus, karakterkódolás, méret, teljes payload, hash, duplikált kérés | karantén, nyers megőrzés, nincs stage-rekord |
| L1 – Parser és séma | kötelező mezők, típusok, értéktartományok, sémaverzió, ismeretlen mezők aránya | karantén, nincs kanonikus írás |
| L2 – Forráshelyi szemantika | meccsállapot, periódusok, eredménytartomány, felállás- és odds-szabályok, időzóna, csapatág-kontextus | lelet, anyagi hiba esetén blokk |
| L3 – Identitás | pontos külső ID, alias-kontextus, ország/nem/korosztály/csapatág, több jelölt | jelöltlista, publikálás felfüggesztése |
| L4 – Források közötti egyeztetés | független megerősítés, mezőnkénti prioritás, konfliktus, elavult forrás | konfliktus, anyagi mezőnél kapu zárva |
| L5 – Idő és logika | megfigyelési sorrend, kickoff-változás, állapotátmenetek, félidő/rendes játékidő/hosszabbítás/tizenegyesek konzisztenciája | blokk vagy emberi review |
| L6 – Összesített üzleti szabályok | duplikált meccs, egy csapat két egyidejű meccse, szezon- és versenytagság, tabella, keretlétszám, gólösszeg, odds-tartomány | anyagi hiba blokkol, opcionális hiány figyelmeztet |
| L7 – Publikálási kapu | kötelező tények megléte, nyitott blokkoló konfliktus hiánya, forrásfrissesség, identitás megerősítése | állapot: ellenőrzött, ideiglenes, visszatartott vagy karantén |
| L8 – Publikálás utáni ellenőrzés | későbbi anyagi korrekció, eredmény utóellenőrzés, napi konzisztencia-audit | kapu újranyitása, verziózott javítás |

#### 7.10.1 Források közötti ellenőrzés szabályai

- A szavazás nem elegendő: két gyenge vagy egymástól nem független forrás nem írja felül a hatókörében mérvadó forrást.
- Három, ugyanazon adatszolgáltatótól származó felület egy független forrásnak számít.
- Egy mező akkor lehet `verified`, ha van hatókörében mérvadó forrás vagy elegendő független megerősítés.
- Ha nincs ilyen, de nincs ellentmondás sem, az állapot `provisional`, nem `verified`.
- Anyagi mező: kickoff, hazai/vendég entitás, meccsállapot, eredmény, félidő, hosszabbítás/tizenegyes, verseny és szezon.
- Nem anyagi példa: opcionális leírás, logó, külső link; ezek hiánya vagy eltérése nem blokkolja a meccslistát.
- A bizonytalan csapatnév soha nem oldódik fel automatikusan fuzzy egyezéssel; a fuzzy csak jelöltet adhat.
- A záró odds és a settlement szempontjából szükséges ár külön ellenőrzést kap: időbélyeg, iroda, piac, kimenet és értelmes tartomány.
- Az „egy forrásból származó, de hatókörében mérvadó” eset nem hiba, hanem dokumentált kivétel a forrásprioritási szabályban.

#### 7.10.2 Publikálási kapu

A kapu meccs- és napi szinten is fut. Meccsszintű kötelező tények a napi listához:

- megerősített identitású hazai és vendég csapat;
- verseny és szezon;
- érvényes kickoff vagy dokumentált bizonytalanság;
- állapot;
- lezárt meccsnél érvényes eredmény;
- nincs nyitott anyagi konfliktus;
- a kritikus forrás nem elavult.

A felállás, elérhetőség, részletes statisztika és odds hiánya nem tartja vissza a meccset a napi listától, de a meccsoldalon és a lefedettségben külön hiányként látszik. Így a napi lista nem lesz üres egyetlen hiányzó részadat miatt, miközben a lista szintjén ellenőrzött tényeket mutat.

Publikálási állapotok:

- `verified_publish`: minden kötelező kapu nyitva;
- `provisional_publish`: nincs blokkoló hiba, de valamely kötelező tény csak ideiglenes megerősítésű;
- `withheld`: anyagi hiba vagy identitásbizonytalanság miatt nem publikálható;
- `quarantined`: a bemenet vagy a levezetés érvénytelen.

A napi lista alapértelmezésben csak `verified_publish` elemeket tartalmaz. A `provisional_publish` elemek külön „Ellenőrzés alatt” nézetben vagy szűrőben jelennek meg, hangsúlyos jelöléssel; `withheld` és `quarantined` elem csak az Üzemeltetésben látható. A kapu döntése `ops.publish_gate` rekordba kerül a szabályverzióval, blokkolókkal, időponttal és állapottal.

#### 7.10.3 Hibából szabály, szabályból teszt

Minden valós adathiba, kézi javítás vagy incidens után kötelező megválaszolni:

1. melyik ellenőrzési réteg nem fogta meg;
2. új szabály, meglévő szabály szigorítása vagy tudatos kivétel szükséges;
3. készül-e hozzá regressziós fixture és teszt;
4. okoz-e várhatóan túl sok hamis blokkolást.

Az új szabály először shadow-ban fut a korábbi adatokon; csak mért hatás után lesz aktív. Ha ugyanaz a hibaosztály megismétlődik, a szabály szigorúsága emelkedik, és a publikálási kapu automatikusan záródik az érintett kategóriára.

Ez a „hibából szabály” hurok az utólagos javítások elleni fő védelem: nem elég a hibát egyszer kijavítani, a hibaosztálynak legközelebb már a kapun kell megakadnia.

#### 7.10.4 Ellenőrzési teher és hamis riasztások

- A leletek szigorúsága `blocker`, `warning`, `info`.
- Csak a dokumentált kötelező szabály lehet blokkoló; minden más figyelmeztetés.
- A review-sor a kickoff közelsége, a verseny prioritása és az anyagi hatás szerint rendezett.
- Az ismétlődő, azonos gyökerű leleteket összevonjuk.
- Egy új szabály élesítése előtt mért hamis blokkolási arány szükséges; a szabály nem teheti használhatatlanná a napi listát.
- Az ellenőrzési lefedettség és a visszatérő hibaosztályok száma külön operátori mérőszám.

---

## 8. Üzemeltetés, ügynökök, biztonság és helyreállítás

### 8.1 Egészség három tengelyen

- **Adat:** forrásfrissesség, lefedettség, karantén, konfliktus, identitásreview, nullás futás, schema-drift.
- **Rendszer:** daemon, Postgres, worker-lease, queue-kor, lemezhely, mentés, migráció, klienshiba.
- **Modell és szimuláció:** kalibráció, piaci kapuk, champion/challenger, szelvényvalidáció, portfólió.

A riasztás cselekvhető, deduplikált, és tartalmazza a forrást/entitást, időt, állapotot, kísérleteket, utolsó sikert, következő próbát, érintett hatókört és a javasolt biztonságos lépést.

### 8.2 Ügynökautonómia

- **L0 — olvasás:** diagnózis, riport, javaslat.
- **L1 — biztonságos automatika:** retry, lease-frissítés, repars, karantén, újraütemezés, worker-restart.
- **L2 — javaslat:** kód-, parser-, forrás- vagy adatjavítási terv.
- **L3 — jóváhagyott végrehajtás:** emberi jóváhagyás után.
- **L4 — tiltott:** adattörlés, destruktív migráció, tömeges merge, forrásvédelem megkerülése, jogosultságmódosítás, valódi fogadás.

Minden ügynökfutás naplózza:

- célt és szerepkört;
- prompt-/sablonverziót és modellt;
- használt eszközöket;
- olvasott/írt rekordokat;
- javaslatot és jóváhagyást;
- eredményt, költséget;
- visszaállítási pontot.

Az ügynök nem módosíthatja saját jogosultságát és nem törölheti a naplót.

### 8.3 Helyi és felhős ügynökök

A helyi ügynök az alapértelmezett. Felhős ügynök csak külön engedélyezett feladatra, minimalizált adattal működhet. Titok, cookie, licencelt nyers adat és teljes forráspayload nem megy ki automatikusan. Minden ügynöknek van költség- és futásidejű korlátja, valamint globális leállítókapcsolója.

### 8.4 Biztonság

- külön, legkisebb jogosultságú Postgres-felhasználó;
- localhost-only API, token és scope;
- a kliens nem kap adatbázis-jogosultságot;
- forrás-cookie/token a macOS Keychainben;
- kötött paraméteres SQL;
- SQLx fordítás idejű ellenőrzés;
- scrape-elt bemenet méret-, idő- és mélységkorlátja;
- URL-allowlist SSRF ellen;
- karakterkódolás és payload-validáció;
- külső link a rendszer böngészőjében;
- verziózott, auditált függőségek;
- nincs dinamikus shell-végrehajtás;
- nincs automatikus önfrissítés.

### 8.5 Mentés és helyreállítás

- napi automatikus adatbázis- és objektumtár-mentés, konzisztens párban;
- heti és havi megőrzés;
- titkosított, külön kötet;
- havi visszaállítási próba;
- konfiguráció, forrásmanifest és ügynökszabály mentése;
- nyers adatok retention policy szerint;
- safe mode: leállított kanonikus írás, olvasható felület;
- javítás előtt snapshot, utána diff és ellenőrzés;
- tömeges javítás csak importtal vagy visszajátszással.

### 8.6 Üzemeltetési felület

Oldalak: áttekintés, források és policy-k, lefedettség, identitásreview, konfliktusok, futások és lease-ek, karantén, ügynöknapló, döntésnapló, mentések, beállítások.

Minden művelet mutatja az érintett rekordszámot és a visszaállítási lehetőséget. Destruktív vagy nagy hatású művelet külön megerősítést és indoklást kér.

---

## 9. Predikció, botok és papírszelvények

Ez a rész a 4. fázistól aktiválódik. A szerződést most rögzítjük, hogy az adatmodell és az API felkészült legyen.

### 9.1 Piackatalógus

Minden piac adatbázis-katalógusból jön:

- `market_key`;
- kimenetek;
- származtatás: `goal_matrix`, `count_model`, `half_time_share`;
- minimális tiszta minta;
- kalibrációs réteg;
- kapuállapot;
- árforrás;
- settlement-szabály;
- piaci korlátok;
- bevezetés/promóció/nyugdíjazás dátuma.

Nincs olyan piac, amely csak kódban létezik.

Első kör:

- 1X2;
- Over/Under 2.5;
- BTTS.

Későbbi kör:

- szöglet;
- lap;
- további gólvonalak;
- félidőpiacok;
- egyéb, külön settlement-szabállyal rendelkező piacok.

### 9.2 Valószínűségi szerződés

Minden előrejelzés hordozza:

- `model_version`;
- `market`;
- `outcome`;
- `p_model`;
- `p_market_devig`;
- `p_blend`;
- `blend_weight`;
- `calibration_id`;
- `quality`;
- `as_of`;
- `inputs_hash`;
- `max_source_ts`;
- `settlement_rule_version`.

A kiadási szabály: nyitott piackapu + dokumentált árforrás + settlement-szabály + elégséges tiszta minta. Ha bármelyik hiányzik, a piac legfeljebb shadow sávban látszik, és nem generál value jelzést.

### 9.3 Modellfejlődés

A kezdeti champion egy egyszerű, kalibrált góleloszlás-modell. Minden új modell shadow-ban indul, saját verzióval. Promóció csak:

- klaszterezett bootstrap CI alapján bizonyított javulás;
- többszörös hipotézis-korrekció után;
- feltartott időszakon nem romló teljesítmény;
- emberi jóváhagyás;
- utólagos megfigyelési időszak.

Az automatikus újratanítás csak a kalibrátorra vonatkozik. A modell- és stratégiaváltás javaslatvezérelt.

### 9.4 Értékelés

- nap szerint klaszterezett bootstrap;
- 95%-os CI;
- purge és embargo;
- rögzített seed;
- rögzített adatkivágás;
- konfiguráció-hash és commit;
- `prediction_trial` napló;
- legfeljebb három nyitott piac-hipotézis egy körben;
- FDR-korrekció;
- verdikt: `insufficient_n`, `no_change`, `better`, `worse`.

Backtest-szám csak regisztrált futásból idézhető. A backtest és az éles út ugyanazt a tiszta predikciós függvényt hívja.

### 9.5 Kutatási sorrend

1. bázisráta és piaci ár benchmark;
2. tiszta góleloszlás-alapmodell;
3. hazai előny, támadó/védő erő és recency óvatos bevonása;
4. kalibráció és logit-térbeli piaci blend;
5. hiba- és szegmensanalízis liga, csapat és időszak szerint;
6. új piac vagy összetettebb modell csak bizonyított javulás után.

### 9.6 Botportfóliók

Minden bot saját papírportfólióval rendelkezik:

- saját bankroll;
- verziózott stratégia;
- tétkezelés;
- piacválasztás;
- időkeret;
- teljes tranzakciós napló.

Portfóliók nem keverednek. Nincs valódi pénz és nincs külső fogadási hívás.

A bot döntési lánca:

1. piacjelöltek;
2. érvényes ár és időablak;
3. edge- és odds-kapu;
4. Kelly-tét;
5. szelvény-összeállítás;
6. kötelező validáció;
7. mentés vagy indokolt kihagyás.

A Kelly törtfrakciós és bankroll-korlátos. Nulla vagy negatív élből nem lesz tét. Nem képződhet 0 Ft-os szelvény. A kihagyás `reason_code`-dal naplózott.

### 9.7 Szelvényvalidáció

Kötelező ellenőrzések:

- legalább egy érvényes láb;
- pozitív tét;
- érvényes, nem lejárt piac;
- nincs ugyanarra a meccsre egymást kizáró vagy korrelált láb;
- nincs duplikált láb;
- a szelvény típusa engedi a kombinációt;
- odds- és tétkorlát teljesül;
- kezdési idők sorrendje helyes;
- a piac settlement-szabálya ismert.

Hibás szelvény nem kerül a portfólióba; elutasítási rekord készül.

### 9.8 Settlement és portfólióriport

Settlement piaconként explicit:

- nyertes;
- vesztes;
- void;
- nyitott.

Kombinációnál a void láb semlegesítése és a többi láb szorzása dokumentált szabály szerint történik.

Portfólióriport:

- ROI;
- yield;
- drawdown;
- CLV;
- találati arány;
- valószínűségi kalibráció.

Csak lezárt és érvényes lábak számítanak; a nyitott és void tételek nem torzíthatják a nevezőt.

### 9.9 Predikciós felület

A Labor nézet részei:

- napi predikciós tábla;
- meccsenkénti modell- és piaci valószínűség;
- value/watch/no-signal állapot, indoklással;
- minta, bizonytalansági sáv, kalibrációs állapot;
- modell-egészség;
- kísérleti napló;
- botportfóliók és szelvénynapló.

Nem jelenik meg „bizonyított” címke lejárt kalibráció vagy elégtelen minta mellett.

---

## 10. Szelektív v1-átvétel

### 10.1 Alapelv

A v1 érintetlen marad. A v2 csak olvasási célú v1 pillanatképből importál. Az import nem másolja a v1 kanonikus ID- és merge-láncot.

### 10.2 Gyűjtésgazda-váltás

Az átmenet alatt egy forrásból egyszerre csak egy rendszer gyűjt. A v2 először korlátozott shadow-mintát gyűjt. Amint a prioritásos körben átmegy a kapukon, a v2 lesz a gyűjtés gazdája; a v1 olvasható archívummá válik. A v1 adatbázisát és fájljait nem töröljük.

### 10.3 Átveendő adat

- ellenőrzött nyers forrásválaszok, forrással, rögzítési idővel és hash-sel;
- történeti meccsek, eredmények, tabellák, statisztikák;
- ellenőrzött odds-pillanatképek;
- bizonyítékkal rendelkező alias- és külső ID-kapcsolatok.

### 10.4 Nem átveendő adat

- v1 kanonikus ID-k vakon;
- bizonytalan vagy hibás merge-ek;
- női/férfi, felnőtt/utánpótlás vagy klub/tartalék csapatok közötti téves kapcsolatok;
- javító migrációk melléktermékei;
- fabrikált vagy ellenőrizetlen eredmények;
- v1 predikciós/hint/betsim adatok a tiszta v2 modellmintában.

A v1 predikciós és szimulációs története külön archívumba kerül, csak összehasonlítási célra.

### 10.5 Importfolyamat

1. v1 leltár forrásonként, szezononként, adatfajtánként;
2. v1 pillanatkép készítése;
3. ellenőrzött exportcsomag;
4. entitás-újraépítés;
5. történeti tények betöltése az új pipeline-on;
6. dry-run összevetés;
7. kapunyitás;
8. batch-elfogadás;
9. v1 olvasási archívumba állítása.

Minden batch rendelkezik:

- batch-azonosítóval;
- forrásmanifesttel;
- checksummal;
- rekordszámmal;
- hibakimutatással;
- visszaállítási ponttal.

### 10.6 Importkapuk

A teljes import minden rekordja végigmegy az L0–L7 ellenőrzési láncon és a publikálási kapun. Egy batch csak akkor válik kanonikussá, ha:

- nincs automatikusan összevont bizonytalan entitás;
- nincs duplikált meccs a prioritásos körben;
- minden átvett tény visszavezethető;
- nincs jövőbeli információt használó származtatott érték;
- a forrás nélküli tények karanténban vannak;
- a dry-run eltéréslista jóváhagyott.

---

## 11. Tesztelés és minőségi kapuk

### 11.1 Tesztpiramis

- domain unit tesztek;
- storage-integrációs tesztek;
- parser golden fixture-ök;
- API contract-tesztek;
- OpenAPI↔Swift kliens egyeztetés;
- teljes import–reconciliáció–lekérdezés integráció;
- migráció forward/rollback ellenőrzés;
- SwiftUI logika- és állapotteszt;
- manuális macOS UI-, billentyűzet- és VoiceOver-ellenőrzés;
- helyi teljesítménymérés;
- backup-restore próba.

### 11.2 Kötelező kapuk

- nincs look-ahead;
- nincs bizonytalan auto-merge;
- nincs publikálási kapu nélkül megjelenített napi lista elem;
- nincs nyitott anyagi konfliktusú `verified_publish` elem;
- nincs provenance nélküli megjelenített tény;
- nincs duplikált természetes kulcs;
- nincs nulla, amely hiányzó adatot jelent;
- nincs érvénytelen vagy nulla tétes szelvény;
- nincs jóváhagyás nélküli destruktív művelet;
- nincs forrás nélküli kanonikus érték;
- nincs zöld állapot nulla találat miatt;
- nincs migration nélküli séma- vagy adatszerződés-változás.

### 11.3 Változás lezárása

Minden változásnál rögzíteni kell:

- mi változott;
- mely fájlok, migrációk és adatszerződések érintettek;
- mit futtattunk;
- pontos teszt- és builderedményt;
- mi maradt ellenőrizetlen;
- üzemeltetési kockázat és visszaállítási út.

---

## 12. Fázisterv

| Fázis | Cél | Kilépési feltétel |
|---|---|---|
| 0 | Alapozás | repo/crate-szerkezet, CI, migrációs fegyelem, ellenőrzési szabálymotor váza, SwiftUI váz, v1 read-only leltár |
| 1 | Identitás és gyűjtés | forrásmanifestek, raw/stage/core, coverage, konfliktus- és review-folyamat, L0–L6 ellenőrzési szabályok, publikálási kapu, prioritásos ligák importja |
| 2 | Meccsközpont | Ma, verseny-, meccs-, csapat-, játékosoldal, keresés, alap üzemeltetési nézetek; a napi lista csak kapun átjutott elemekből épül |
| 3 | Live | élő eseményfolyam, SSE/WebSocket, incidens- és frissítési versenyhelyzetek |
| 4 | Predikció | piackatalógus, benchmark, champion/challenger, shadow, kalibráció, 1X2/OU2.5/BTTS |
| 5 | Botok és szelvények | portfóliók, stratégiák, validáció, settlement, CLV, riportok |
| 6 | Autonómia | ügynökök, javaslatvezérelt önfejlesztés, riasztások, opcionális felhős kutatás |

A fázisok nem indíthatók a megelőző fázis kilépési feltétele nélkül. A live nem lehet a meccsközpont helyettesítője, és a predikció nem lehet az adatminőségi munka helyettesítője.

---

## 13. Sikermérők

### 13.1 Adat

- nulla auto-összevont bizonytalan entitás a prioritásos körben;
- nulla duplikált meccs a prioritásos körben;
- a napi lista minden eleme rendelkezik publikálási kapu rekorddal;
- `verified_publish` elemnél nincs nyitott anyagi konfliktus;
- minden megjelenített tény visszavezethető;
- minden lefedettségi slot rendelkezik állapottal;
- a visszatérő, azonos gyökerű hibaosztályok száma csökken, és mindegyikhez tartozik ellenőrzési szabály vagy regressziós teszt;
- a konkrét lefedettségi százalékok az első dry-run baseline után rögzítettek;
- nincs fabrikált vagy forrás nélküli eredmény.

### 13.2 Rendszer

- helyi API p95 < 150 ms a gyakori lekérdezéseknél;
- hidegindítás < 2 s;
- a napi kör hibái nem némák;
- a havi restore-próba sikeres;
- nincs lejárt lease a napi operátori nézetben látható indok nélkül.

### 13.3 Felület

- bármely entitás legfeljebb három lépésben elérhető a napi listáról;
- a meccs állapota, eredménye és adatminősége az első képernyőn látszik;
- a hiányzó és konfliktusos adat nem téveszthető össze valós nullával;
- billentyűzetes és VoiceOver-alapfolyamat végigjárható.

### 13.4 Predikció és botok

- minden látható piac rendelkezik kalibrációval, árforrással, settlement-szabállyal és mérési naplóval;
- nincs bizonyítás nélküli value jelzés;
- nulla nulla tétes szelvény;
- nulla validálatlan kombináció;
- minden kihagyás okkódos;
- minden portfóliómozgás visszavezethető.

### 13.5 Karbantarthatóság

- build, typecheck, migrációs ellenőrzés, contract-tesztek és releváns UI-tesztek zöldek;
- új adatjavítás nem igényel sémamódosítást;
- a dokumentáció és a tényleges viselkedés nem tér el;
- a repó gyökere nem tartalmaz generált adatot vagy dokumentumtömeget.

---

## 14. Kockázatok és kezelésük

| Kockázat | Kezelés |
|---|---|
| Túl nagy első építés | vertikális szeletek, fáziskapuk, live/predikció/bot később |
| Forráskiesés vagy schema-drift | adapter-contract, canary, circuit breaker, karantén, helyettesítési terv |
| Jogi/licenc kockázat | forrásonkénti policy, retention, shadow, adatminimalizálás |
| v1 hibás adatainak átcsúszása | snapshot, dry-run, karantén, identitás-review, batch-visszaállítás |
| Túl szigorú ellenőrzés miatt leapadó lista | rétegenkénti szigorúság, csak kötelező szabály blokkol, új szabály shadow-mérés után aktív |
| Források közötti téves „többségi igazság” | mezőnkénti prioritás, függetlenség számítása, nem egyszerű szavazás |
| Predikciós önámítás | as-of, klaszterezett CI, trials, FDR, shadow, feltartott időszak |
| Ügynökök hibás autonómiája | L0–L4, napló, javaslatvezérelt változtatás, globális stop |
| Egygépes kiesés | automatikus mentés, restore-próba, safe mode |
| Tárolási csapda | repo-n kívüli tömörített objektumtár, deduplikáció, retention |
| Rust/Swift/API széttartás | OpenAPI, generált kliens, contract-tesztek, közös domain-nyelv |
| Túl sok felület | négy munkamód, kontextusfüggő részletek, fáziskapu a Laborra |

---

## 15. Döntési kapuk és alapértékek

Az alábbi döntések tudatosan a megfelelő fázis elejére kerülnek. Mindegyikhez tartozik alapérték, hogy a terv ne függjön nyitott helykitöltőtől.

| Döntés | Mikor | Alapérték, ha nincs felülírás |
|---|---|---|
| Első ligalista | Fázis 1 eleje | magyar élvonal + a felhasználó által kiválasztott európai prioritásos ligák, legfeljebb 12 verseny |
| Támogatott macOS-verziók | Fázis 0 | aktuális és az azt megelőző két főverzió |
| Nyers retention keret | Fázis 1 után | forrásonként mért baseline alapján, hot/cold bontással |
| Mentési cél | Fázis 0 | helyi titkosított kötet + külön külső kötet |
| Felhős ügynök engedélyezése | Fázis 6 | alapból kikapcsolt, feladatonkénti jóváhagyással |
| Első élesítendő források | Fázis 1 | a minősítésen legjobban teljesítő 3–4 forrás, shadow és canary után |
| Első ligák történeti mélysége | Fázis 1 | 2–3 szezon, a forrásoktól és licenctől függően |

A döntési kapu nem kitöltetlen kérdés: a fázis addig nem indul, amíg az alapérték vagy a felhasználói felülírás rögzítve nincs.

---

## 16. A v1-ből tudatosan átvett jó megoldások

- raw-first és provenance-alapú szemlélet;
- kanonikus ID és külső ID szétválasztása;
- alias- és match-link jelöltkezelés;
- lefedettségi ledger;
- as-of számítási fegyelem;
- forrásprioritás és konfliktuskezelés;
- napi operátori nézet;
- predikciós szerződés, kalibráció és backtest-registry iránya;
- papírszimuláció, CLV és settlement-napló ötlete;
- biztonságos autonóm működés charterje.

A v1-ből nem vesszük át a nézetrobbanást, a javító migrációk láncát, a globálisan egyedi alias-feltételezést, a bizonytalan automatikus merge-eket és a néma nullaeredményt.

---

## 17. Definition of Done

Egy v2 változás akkor kész, ha:

1. a viselkedés megfelel ennek a specifikációnak;
2. a domain-invariánsok teszteltek;
3. a szükséges migráció és visszaállítási út dokumentált;
4. az API- és kliensszerződés egyezik;
5. a hiány-, hiba- és konfliktusállapot kezelt;
6. az adat visszavezethető forráshoz;
7. a teljesítmény a megadott kereten belül van;
8. a billentyűzetes és akadálymentes használat ellenőrzött;
9. a pontos build- és teszt eredmény rögzített;
10. az ismert korlátok és üzemeltetési kockázatok kimondottak;
11. minden új valós adathiba vagy javítás esetén eldőlt, hogy milyen ellenőrzési szabály, regressziós fixture vagy tudatos kivétel védi a jövőben.
