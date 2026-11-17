# 2026-11-16 — Napi checkpoint (Soccerway első teljes kör)

**Állapot:** napi worker-kör lefutott; a szűrt ligáknak ma sincs meccsük
**Előző nap:** 2026-11-15 (S4d + canary + asset + S8 első kör)
**Kapcsolódó dokumentumok:** [tegnapi checkpoint](../2026-11-15/checkpoint.md),
[soccerway ops](../../operations/soccerway.md) §6 (runbook),
[assets ops](../../operations/assets.md)
**Branch:** `main` (5dd49f1, szinkronban originnal)

## Kiinduló állapot

- [x] Kötelező olvasási sorrend: AGENTS.md, README, tegnapi checkpoint,
      git állapot (main tiszta, szinkron).
- [x] Napi kvóták nullázódtak (soccerway 0/14, footballdata 0/25),
      tegnapról 1 függő job (La Liga) maradt.

## Végrehajtás

- [x] **Daemon-indítási hiba (operatív tanulság):** az első indítás a
      feature nélküli `footballd` binárist használta (a build rossz
      könyvtárból futott) → 4 Soccerway-job elbukott
      „obscura feature nincs engedélyezve ebben a buildben" üzenettel.
      A bukott jobok operátori visszaállítása (`pending`, attempts=0)
      után a `--features obscura` builddel indított daemon futott.
      **Szabály:** a napi kört mindig a feature-ös binárissal kell
      indítani (az ops doc tartalmazza).
- [x] Worker-daemon (`WORKER_ENABLED=true`, 60 s tick): a napi discovery
      7 Soccerway + 5 footballdata jobot generált; a min_delay/breaker
      őrök szerint dolgozta fel őket; breaker végig zárt.
- [x] **Soccerway: 7/7 domain `no_data`** (Premier League, La Liga,
      Serie A, Bundesliga, Ligue 1, NB I, NB II) — a válogatott szünet
      miatt a szűrt ligáknak ma sincs meccsük; napi kvóta 8/14.
      Review-sor nem keletkezett (nincs megfigyelt csapat).
- [x] **Footballdata:** a mai ablak (Nov 9–23) üres — a CSV-k a legutóbbi
      lejátszott fordulóig (Nov 8) tartalmaznak sorokat; a Nov 8-i
      meccsek újra-feldolgozása **idempotens** volt (nincs új kanonikus
      sor, a meglévő 44 élő meccs gate-je változatlan).
- [x] **Korrekció a tegnapi jegyzethez:** a dev DB-ben **megvannak** a
      korábbi élő meccsek (44 élő + 20 demó = 64, mind gated); az „csak
      demó" megállapítás téves volt (a gate-query dátum szerint
      csoportosítva a demó és élő sorokat együtt mutatta).

## Megfigyelések (nem blokkoló)

1. [x] **Javítva:** a footballdata `source_record_key` tartalmazta a
   `time_window`-t, ezért ugyanaz a meccs ablakonként más
   observation-kulcsot kapott. Az új kulcs meccshez kötött
   (`footballdata-<dátum>-<home>-<away>`), regressziós teszttel
   (két különböző napi kör azonos kulcsokat ad). A meglévő stage-sorok
   történelmi kulcsai megmaradnak; kanonikus hatás nincs.
2. A Soccerway mai renderjei ~1,2 MB-osak (tegnap 2,2–3,1 MB) — a
   kisebb nap-oldal (kevesebb meccs), a marker-várakozás és a
   dátum-guard így is végigfutott.

## Következő döntési pont

1. Az első **meccses** Soccerway-nap a top-5/NB I visszatérésével
   (várhatóan nov. 21–23.): review-bootstrap a lokalizált nevekre,
   kanonikus meccsek + gate, kliens-ellenőrzés.
2. Opcionális: a worker-daemon launchd-szolgáltatásként (charter-döntés),
   hogy a napi kör emberi indítás nélkül fusson.

## Visszaállítási út

- Kód: nincs változás (a diagnosztika visszaállítva).
- Adat: a mai kör csak observation/ledger/coverage sorokat írt
  (`no_data`), kanonikus adatot nem módosított; a review-queue üres.

## S9 — Napi ingest-modell + verseny-bontás (2026-11-16, jóváhagyott irány)

### Döntések

- [x] A Soccerway (és később a FotMob) a nap **összes versenyét** olvassa,
      besorolással; a kliens alapnézete a **kiemelt kör**, „Minden verseny”
      kapcsolóval.
- [x] Architektúra **A**: job = forrás × nap (`domains=["day"]`); a tick a
      terv minden kérését feldolgozza; a parse után versenyenként bontunk
      (`NormalizedDay::grouped_from_observations`), a `NormalizedDay`
      szerződés marad. Atomicitás: hiba → nincs kanonikus írás a napra.
- [x] Spec + terv: `docs/superpowers/specs/2026-11-16-s9-day-jobs-design.md`,
      `.../plans/2026-11-16-s9-day-jobs.md` (commit `ff692c4`).

### Implementáció (commit-per-green)

- [x] **S9a (`47c7e52`):** worker többkérésű job-loop kérésenkénti
      robots/kvóta/delay/breaker/ledger/artifact kapukkal; intra-job
      min_delay-várakozás (max 15 s); grouped normalizálás; e2e teszt:
      2 kérés → 2 verseny, 2 meccs, 2 gate.
- [x] **S9b (`2159579`):** nullable verseny/csapat ország; competition
      find NULL-safe; a meglévő csapat országát NULL nem írja felül;
      kupatesztek.
- [x] **S9c (`9e20bf9`):** Soccerway minden csoport parse + osztályozás
      (ismert slug → kanonikus meta; ismeretlen → megjelenített név +
      zászló-ország; kupa tier 0/priority 40; női 60; utánpótlás 70;
      kupa/nemzetközi → csapat-ország NULL); parser v2.0; egy napi kérés.
- [x] **S9d (`0972542`):** FotMob minden liga (tiltva marad), ccode→ISO;
      footballdata egy job, ligánként 1 kérés (5 CSV), a liga a
      kérés-kulcsban.
- [x] **S9e (`3ebce4d`):** kliens kiemelt kör alapból + „Minden verseny”
      kapcsoló; üres-kiemelt tipp gombbal; tesztek (18 app teszt).

### Élő bizonyíték (2026-11-16, dev)

- [x] Policy-k `day` domainre állítva (soccerway budget 6, footballdata 25,
      fotmob 10); a meglévő per-liga jobok historikusak.
- [x] Footballdata napi job: **1 job / 5 kérés** (2 s várakozásokkal);
      a mai ablak (nov. 9–23.) üres → `no_data`, coverage `day`.
- [x] Soccerway napi job: **1 render** (a tegnapi 7 helyett) → **87 meccs**
      12+ versenyből (Friendly International, Afrikai Kupa-selejtező,
      Baller League, Kings Cup, Nations League C/B, U20, női U17 stb.),
      **176 csapat-megfigyelés**, 174 új identitás-review sor.
      Kanonikus meccs ma **0** (fail-closed: a csapatok többsége még
      review-ra vár) — ez a várt első kör.
- [x] Fő tanulság: a lap egy renderrel mindent ad; a régi modell 7
      renderrel csak a 7 kiemelt ligát látta.

### Automatizálás

- [x] `scripts/run-footballd.sh` + `scripts/install-footballd-agent.sh`
      (launchd LaunchAgent, KeepAlive, log a
      `~/Library/Logs/FootballAppV2/` alatt) + `docs/operations/automation.md`.
- [ ] Agent telepítése és élő ellenőrzése (a user jóváhagyta az
      automatizálást; telepítés a záró lépésben).

### Nyitott pontok

1. **Review-triage:** 174 pending sor (nemzeti csapatok, női/utánpótlás,
   alsóbb ligák); a batch `approve-create-all` csak ellenőrzés után
   használható — fuzzy tiltott. A review-kontextusba érdemes a versenyt
   is beletenni (későbbi szelet).
2. Az első teljes kanonikus nap a review-bootstrap után (várhatóan a
   hétvégi fordulókkal).

### Automatizálás — telepítve és ellenőrizve

- [x] **TCC-tanulság:** a launchd a Documents mappából nem futtat
      („Operation not permitted"). A telepítő ezért a futtatandó
      elemeket a `~/Library/Application Support/FootballAppV2/agent`
      alá másolja (bináris, Obscura, env 600, adatok; a repó `data/`
      egyszeri átemelése).
- [x] Agent telepítve: `hu.bandi86.football.v2`, `state = running`
      (pid 41010), `obscura fetcher aktiv`, API port 63405, napi tick
      0 pending jobbal; a kliens a runtime.json-t innen olvassa.
      Log: `~/Library/Logs/FootballAppV2/footballd.log`.
- [x] Üzemeltetési doksi: `docs/operations/automation.md` (telepítés,
      frissítés = telepítő újrafuttatása, kézi tick ugyanazzal a
      DATA_DIR-rel, visszavonás `--uninstall`).
- Megjegyzés: a soccerway mai kerete 9/6 (a nap elején még a régi
  per-liga jobok és a teszt-kör fogyasztott) — holnap nullázódik; az új
  napi job 1–2 fetch.

### Záró állapot

- [x] Teljes kapu: `check-foundation.sh` exit 0, workspace 74 csoport
      zöld, clippy/fmt (workspace + obscura feature) tiszta, app 18 teszt
      zöld, teszt-DB resetelve.
- [x] Kliens UI: a „★ Kiemelt kör / Minden verseny" kapcsoló élőben
      megjelenik (a mai nap kanonikusan üres a review-bootstrapig).

### Review-triage és az első kanonikus nap (folytatás)

- [x] `identity review-summary` CLI: pending review-ök csoportosítva
      (nem/ág/verseny/orrkágok/minta), a versenyt a match_core
      observationből kötve (nincs séma-változás).
- [x] Soccerway osztályozás-finomítás: nemzetközi versenyeknél a
      csapatnév ország (angol név-tábla), U##/W tokenek →
      korosztály/nem (Serbia→RS, Belgium U21→BE/youth, Spain U17 W→
      ES/female/women); tesztek.
- [x] Kontrollált bootstrap: `identity approve-create-all` → **58
      nemzeti csapat** létrehozva (országos kontextussal), **116 sor
      pending** maradt (klubok, ország nélkül — fail-closed; a
      country-flag map bővítése későbbi szelet).
- [x] A nap újrafuttatása a feloldott csapatokkal: **24 kanonikus meccs**
      (2026-11-16), mind `verified_publish` (Nations League C/B/A/D,
      Friendly International, Euro U17 Women stb.); az API
      `refreshed_at`-tel adja.
- [x] Kliens-ellenőrzés: a kiemelt nézet „nincs meccs" tippje +
      „Minden verseny mutatása" gomb; a teljes nézet 24 meccset mutat
      verseny szerint; nemzetközi verseny felirata „NEMZETKÖZI"
      (korábbi „ISMERETLEN · 1. OSZTÁLY" javítva).
- [x] Az agent a friss binárissal újratelepítve (pid 50219 → újraindult),
      az adatok megmaradtak.
- [x] Teljes záró kapu: foundation 0, workspace 74 csoport zöld,
      clippy/fmt tiszta, app 18 teszt zöld, teszt-DB resetelve.
- Megjegyzés: a napi keret ma 9+/6 a nap eleji legacy per-liga körök
  miatt; holnap már 1–2 fetch. A `footballctl` binárist mindig
  `--features obscura`-val kell építeni (különben a Soccerway-job
  `transport hiba`).

### Zászló-térkép bővítés + a maradék 116 review feloldása

- [x] A tárolt nyers oldalból (offline) kiolvasott zászló-nevek alapján a
      `country_by_title` **angol országnevekkel** bővült (Argentina, Peru,
      DR Congo, Ukraine, …); a kontinens/világ zászló (Europe, Africa,
      World, North & Central America) mostantól a **nemzetközi meccs**
      jelzője (a csapatnév ország-név).
- [x] Javítás: **hazai kupáknál** a csapat-ország a verseny országa
      (korábban üres volt) — csak kontinens-zászlónál marad ismeretlen.
- [x] A nemzeti-csapat névtábla bővítve (CONCACAF/Afrika/Ázsia: Bermuda,
      Cuba, Benin, Eswatini, Indonesia, Kyrgyzstan, Uzbekistan, Somalia…).
- [x] `approve-create-all --allow-unknown-country`: ország nélküli
      kontextusnál **NULL országgal** hoz létre (őszinte hiány) — ezzel a
      maradék sor is lezárható; alapból továbbra is szigorú.
- [x] Eredmény: **116 sor jóváhagyva** (115 országgal, 1 NULL:
      Auckland City U23), **0 pending** review; összesen 269 approved.
- [x] A nap újrafuttatása: **85 kanonikus meccs** (2026-11-16), mind
      `verified_publish`, 15+ versenyből (Friendly International, Africa
      Cup-selejtező, Kings Cup, Baller League, Persha Liga, Liga 1 Peru,
      Super League 2, Premier League Cup/U22, Liga MX Women, …).
- [x] Kliens: „Minden verseny" nézet 85 meccset mutat (81 következik,
      1 élő, 3 befejezett); nemzetközi felirat „NEMZETKÖZI · KUPA";
      alapnézet visszaállítva kiemelt körre.
- [x] Napi keret visszaállítva 6-ra; az agent a friss binárissal fut.
- [x] Záró kapu: foundation 0, workspace 74 csoport zöld, clippy/fmt
      tiszta, app 18 teszt zöld, teszt-DB resetelve.

## S10 — Meccs-részletező (API + kliens)

- [x] **API:** `GET /v1/match/{id}` (read-token): eredmény-bontás
      (FT/HT/ET/pens + decided_by), kickoff-precízió, játékvezető,
      forrásonkénti statisztika (legfrissebb), odds-lánc + forrás/trust,
      Elo as-of, logók, publikálási kapu (állapot, blokkolók, kiértékelés,
      szabálykészlet). Nem található → 404, érvénytelen id → 400.
      Teszt: teljes mezők + 401/404.
- [x] **Kliens core:** `MatchDetail` modell + `HTTPMatchDetailService`
      (`APIError.notFound` új eset); teszt dekódolás + szolgáltatás.
- [x] **Kliens UI:** a napi lista sorai kattinthatók → részletező sheet:
      fejléc (logók, eredmény, gate-badge, publikálás dátuma), meta
      (verseny, szezon, időpont, játékvezető), eredmény-chipek,
      összehasonlító statisztika, odds-chipek + trust + forrás/idő,
      ellenőrzési szekció (blokkolók). Élő ellenőrzés a 2026-11-08-i
      Brighton–Brentford meccsen (statisztika, odds, gate).
- [x] **Valós hiba javítva:** az `OddsSnapshot` minden piacot kötelezőnek
      várt, a footballdata viszont BTTS nélkül ad → a 2026-11-08-i nap
      dekódolása elhasalt („Váratlan válaszformátum"), a lista üres volt.
      A piacok most `Double?`-k, a UI „–"-t mutat; a nap újra 24 meccses.
- [x] Kapuk: Rust workspace 74 csoport zöld, clippy/fmt tiszta,
      foundation 0; app 20 teszt, core 21 teszt zöld; teszt-DB resetelve.

## S11 — Versenyek oldal + verseny-adatlap (design-reference alapján)

- [x] Design-referenciák: `notes/images/sofascore-design-example/` (liga-
      és kupa-adatlap). Az adatelrendezést ehhez igazítottuk: fejléc
      (logó/ország/liga/szezon) + összesítő chipek, majd Meccsek/Csapatok.
- [x] **Storage:** `competition` repository — napi verseny-lista
      (ellenőrzött meccsszám, élő/lejárt), verseny-összesítők
      (meccs/gól/BTTS%/hazai-döntetlen-vendég), csapat-mérlegek
      (M/GY/D/V/GÓL) a megfigyelt ablakból. `DayFilter` bővítés:
      verseny-szűrő + ablak-napok (a napi API viselkedése változatlan).
      Tesztek: 3/3.
- [x] **API:** `/v1/competitions?date=`, `/v1/competition/{id}?date=&window=`
      (max 30 nap); a meccsek a napi API azonos alakjában (közös
      `map_match`); 401/404. Teszt 8/8.
- [x] **Kliens:** „Versenyek” tab (kereső magyar és angol országnévre is),
      verseny-sheet: fejléc + összesítők, Meccsek (a napi lista komponense,
      kattintható meccs-részletezővel) és Csapatok tábla (logók, W/D/L,
      gólarány). Az ország-tábla ~70 ISO kódra bővült magyar nevekkel;
      ország nélküli verseny → „NEMZETKÖZI”.
- [x] Élő ellenőrzés: 32 verseny a mai napon; Africa Cup-selejtező adatlap
      (6 meccs, NEMZETKÖZI · KUPA); Premier League adatlap ±14 nap:
      9 meccs, 26 gól, 56% BTTS, 18 csapat mérleggel (logókkal).
- [x] Kapuk: workspace 75 tesztcsoport zöld, clippy/fmt tiszta,
      foundation 0; app 22 teszt, core 23 teszt; teszt-DB resetelve.

## S12 — Csapatok oldal (lista + csapat-adatlap)

- [x] **Storage:** `team` repository — kereshető csapat-lista (ország,
      nem/ág, logó, 30 napos ellenőrzött meccsszám), csapat-adatlap
      (Elo as-of, megfigyelt W/D/L + gólarány a ±30 napos ablakból).
      `DayFilter` bővítés `team_id` szűrővel. Teszt 1/1.
- [x] **API:** `/v1/teams` (query, date), `/v1/team/{id}?date=&window=`
      (max 90 nap) — a meccsek a napi alakban, verseny szerint
      csoportosíthatók; 401/404. Teszt 9/9 (a teljes API suite).
- [x] **Kliens:** „Csapatok” tab (kereső csapatra és magyar országnévre),
      csapat-sheet: fejléc (logó, ország, NŐI/UTÁNPÓTLÁS/TARTALÉK badge,
      ELO), forma-chipek (M/GY/D/V/GÓL), meccsek verseny szerint
      (kattintható meccs-részletezővel). Élő ellenőrzés: Liverpool
      (Anglia, 1/1/0/0, 3–1, PL-meccs odds-szal).
- [x] Megjegyzés: a listában a demó- és élő entitások név szerint
      duplikálhatnak (két „Arsenal") — a cross-source entitás-összevonás
      külön téma; a kereső és a meccsszám segít eligazodni.
- [x] Kapuk: workspace 76 tesztcsoport zöld, clippy/fmt tiszta,
      foundation 0; app 24 teszt, core 25 teszt; teszt-DB resetelve.
- [x] **Későbbi követelmény rögzítve (memória):** az összes forrás
      bekötése után **teljes utólagos feltöltés** kell minden ligára és
      országra, a lehető legtöbb meccsre odds-szal (v1 DB vagy más
      forrás) — a mai adat csak részleges kör.

## S13 — OpenFoot forrás bekötése (2026-11-17)

- [x] Research (5 kérés, kvóta 342→~347/5000): robots `Allow: /`;
      `/v1/competitions` = 120 verseny (név/ország/kód); nap-feed
      `data[]` + `meta` (`count/total_count/scope.partial`); lapozás
      `page` paraméterrel (100/oldal); státusz-szűrő nélkül ütemezett +
      lezárt együtt; statuses scheduled/live/finished/postponed/cancelled;
      odds nincs; `sourceRefs` (fotmob) részben.
- [x] **Fetch-modell:** `bearer_token` mező (csak a kérés fejlécébe;
      naplóba/capture-be nem kerül) — az összes adapter/teszt frissítve.
- [x] **Közös ország-segédek:** `ingest::countries` (lokalizált/angol
      név → ISO, nemzetközi/kontinens felismerés) — a Soccerway is ezt
      használja (viselkedés változatlan, tesztek zöldek).
- [x] **`OpenFootAdapter`:** napi job 4 kéréssel (registry + 3 nap-oldal),
      registry-cache, dátum-guard, státusz/eredmény fail-closed mapping,
      ismert versenyek kanonikus nevekkel (top-5 + NB I/II), kupa/női/
      utánpótlás heurisztika, csapat-ország szabály. Teszt 4/4.
- [x] **Wiring:** regisztráció footballd/CLI; agent `run.sh` a Keychainből
      injektálja az `OPENFOOT_API_KEY`-t; policy `canary` napi 8 kéréssel,
      trust `result`/medium; ops doc `docs/operations/openfoot.md`.
- [x] **Élő canary:** 4 kérés sikeres (30 meccs, 20 csapat, 20 review);
      `approve-create-all` (20 új entitás, mind országgal); újrafuttatás:
      **12 kanonikus meccs 2026-11-17-re, mind `verified_publish`**
      (UEFA Nations League 8, Friendly, Liga AUF, NPFL, USL); nincs
      napon belüli duplikált meccs.
- [x] **Render-reziliencia javítás (Soccerway, reggeli automata hiba):**
      a 07:31-es timeout után a 07:33-as kör 594 KB-os **részleges
      rendert** adott (nincs nap-címke) → karantén. A fetcher mostantól,
      ha a ready-marker nem jelenik meg a várakozási ablakban, **tranziens
      Timeout** hibát ad (nem fél oldalt) → a worker backoff-os retry-t
      folytat, nem terminális karantént. Teszt: marker nélküli oldal →
      Timeout. A mai Soccerway-job pending/retry állapotban van.

## S13 folytatás — Obscura önjavítás + Soccerway locale-megfigyelés (2026-11-17)

- [x] **Motor-halál ok:** az agent Obscura-gyermeke elhalt (nincs
      `obscura` process), a hosszú életű CDP-kapcsolat transport hibákat
      adott. A jobok helyesen pending/backoff-ban maradtak (nem
      karantén).
- [x] **Önjavító fetcher:** `ObscuraFetcher::connect_managed(cfg)` — a
      motor health-checkje (`/json/version`) + a CDP-kliens zárt állapota
      alapján újraindítja a szervert és újracsatlakozik; transport hiba
      esetén egy automatikus restart+retry. A daemon és a CLI ezt
      használja; a kliens `is_closed()` jelzőt kapott. Élő bizonyíték:
      a halott motor mellett a következő tick **200-at és 951 KB-ot**
      hozott (31 verseny materializálva).
- [x] A marker nélküli render **tranziens Timeout** (nem fél oldal,
      nem karantén) — teszt hozzáadva.
- [x] **Soccerway locale-fordulat:** a mai oldal **magyar csapatneveket**
      adott (pl. „Albánia U21”, „Csehország U17”) a korábbi angol helyett
      → 166 meccs, de a csapatok nagy része review-ra került (**328
      pending**). Ezeket NEM hoztam létre (az angol nevű entitások
      duplikálását okozná); a kanonikus nap részleges (13 meccs,
      OpenFoot + a feloldott Soccerway-sorok).
- [ ] Következő szelet: **Soccerway locale rögzítése** (int.soccerway.com
      vagy language-paraméter kutatása) + a magyar nevű review-sorok
      tudatos tisztítása (reject indoklással / angol párral való
      összekötés), majd a nap újrafuttatása.
- [x] Záró kapuk: workspace 77 tesztcsoport zöld, clippy/fmt tiszta,
      foundation 0, teszt-DB resetelve.

## S14 — Soccerway locale-normalizálás + review-tisztítás (2026-11-17)

- [x] **Locale-kutatás:** `int.soccerway.com` → geo-redirect a magyar
      oldalra; `?lang=en` hatástalan; élő cookie nincs; a nyelvet a geo
      dönti el (kérésenként akár vált). Locale-pin nem lehetséges →
      **nyelvfüggetlen normalizálás** a megoldás.
- [x] **Normalizálás:** nemzeti csapatok mindig **angol kanonikus névvel**
      (lokalizált + angol ország-táblák, ékezetfüggetlen illesztéssel) és
      ISO országgal; a korosztály/nem token (U21/W) megmarad a névben.
      Nemzetközi verseny-sablonok kanonikus angol neve (Nations League,
      Afrikai Kupa, Euro U17/19/21, AFC Ázsia-kupa, Felkészülési);
      a csoport-zászló a lokalizált és az angol táblából is oldódik.
      Offline mérés a mentett magyar oldalon: **332 csapat, 0 magyar név,
      0 ország nélküli**.
- [x] **Tisztítás:** `identity reject-all-pending --note` batch CLI
      (auditált elutasítás) — két elavult pillanatkép-kör (328 + 314 sor)
      elutasítva; a végleges kör **313 pending, mind országgal**, batch
      approve-create-all (313 új entitás).
- [x] **Eredmény:** a 2026-11-17-i nap **170 kanonikus meccs, mind
      `verified_publish`** (Soccerway 166 + OpenFoot, a feloldott
      csapatokkal).
- [ ] **Talált következő javítás (dedupe-szelet):** 8 kereszt-forrás
      duplikátum (OpenFoot „UEFA Nations League” vs Soccerway
      „- League A/B”). Javaslat: a természetes kulcsból a **forduló**
      kikerül (megjelenítési metaadat marad), és a Nations League
      szülő-versenynévre normalizál mindkét forrás; a meglévő sorok
      migrációja/összevonása a teljes backfill/crosswalk projekttel.
- [x] Keretek visszaállítva: soccerway 6/nap, openfoot 8/nap; az agent a
      végleges binárissal fut. Kapuk: 77 tesztcsoport, clippy/fmt,
      foundation 0, teszt-DB resetelve.

## S15 — Dedupe előrefelé: forduló-kulcs + a név-instabilitás tanulsága (2026-11-17)

- [x] **Forduló kikerült a meccs-azonosságból:** a `find_or_insert_match_id`
      és a `count_duplicate_matches` már nem nézi a `round_label`-t
      (megjelenítési adat marad, frissül) — két forrás eltérő
      forduló-jelöléssel ugyanazt a meccset írja le. Teszt: két round-
      címke → 1 kanonikus sor, frissülő címkével.
- [x] **Tanulság (visszafordítva):** a heurisztikus kétnyelvű
      verseny-sablonok **instabilak** — minden sablon-változtatás új
      verseny-entitást és duplikált meccseket szül (ma 41 pár:
      AFC Asian Cup U17 két alakja, Africa Cup U23 dupla „Qualification",
      Nations League szülő vs League A/B). Ezért a Soccerway-nevezést
      **visszaállítottam a bizonyítottan stabil b51781b állapotra**
      (a locale-normalizálás és a 170-es nap ezt használta); csak a
      forduló-kulcs javítás maradt.
- [ ] **Nyitott takarítás (dedupe-szelet):** a 2026-11-17-i napon a
      kísérleti név-körök után **231 sor / 41 duplikált pár** maradt.
      Szükséges: stabil név-migráció + merge-eszköz (provenance/gate
      átmozgatás, vesztes sorok törlése) VAGY a nap observationjeiből
      történő újramaterializálás stabil nevekkel — a teljes
      backfill/crosswalk projekttel együtt.
- [x] Agent a végleges (stabil nevezésű) binárissal fut; keretek:
      soccerway 6/nap, openfoot 8/nap.

## S16 — Dedupe/merge eszköz + a 2026-11-17-i duplikátumok tisztítása (2026-11-17)

- [x] **Migráció 0014:** `ops.dedupe_archive` (id, merged_at, survivor_id,
      duplicate_id, note, payload jsonb) + időrendi index. A payload a
      teljes előtte-állapotot rögzíti (match, gate, result, provenance,
      odds snapshot/quote, stats) — a visszaállítási út kézi visszajátszás,
      a migráció rollbackje `DROP TABLE ops.dedupe_archive`.
      Érintett: `db-check.sh` (46 tábla), `storage/tests/connection.rs`.
- [x] **Storage réteg (`storage/repositories/dedupe.rs`):**
      `duplicate_groups` (home/away/kickoff szerinti duplikált csoportok
      adott napon), `is_same_match` őr a merge előtt, `merge_duplicate`
      tranzakcióban: archívum beírás → provenance átmozgatás (ütköző
      `is_selected` törlése) → hiányzó eredmény/odds/stats feltöltés →
      publish_gate + maradék provenance + kanonikus sor törlése →
      `ops.audit_event` (`match_dedupe_merge`). Teszt:
      `storage/tests/dedupe_merge.rs` (1 túlélő, mindkét forrás score
      provenance-a megmarad, gate/archívum/audit ellenőrizve).
- [x] **CLI:** `footballctl dedupe matches --date YYYY-MM-DD [--apply]
      [--deny "név1|név2"]` — alapból dry-run; a túlélő a tiltott nevek
      nélküli, legkorábbi `created_at`-ú sor (tie-break: legkorábbi sor);
      minden merge előtt `is_same_match` ellenőrzés.
- [x] **Éles futás (2026-11-17):** 43 csoport, 43 merge. Ellenőrzés:
      napi duplikált csoport **0**, teljes DB-re is **0**; nap vége
      **190 meccs**, mind `verified_publish` (nincs blokkolt sor);
      233 → 190 sor, 43 áldozat (41 korábban jelzett + 2 újabb churn-pár);
      provenance-árva 0;
      `ops.dedupe_archive` 43, `audit_event` 43; day API 190 meccs,
      0 nem publikált.
- [x] **Ismétlődés kizárva:** a churn kizárólag a visszaállított
      kísérleti névadásból jött; az OpenFoot és a Soccerway a Nations
      League A/B meccseken ugyanazt a verseny-entitást és meccssort
      használja (közös provenance), a stabil b51781b névadással nincs
      új duplikáció.
- [x] Kapuk: teszt-DB reset → `check-foundation.sh` OK (46 tábla) →
      `cargo test --workspace -- --test-threads=1` mind zöld →
      clippy `-D warnings` + `fmt --check` tiszta → teszt-DB visszaállítva.

## S17 — Forrás-újramérés: Transfermarkt jelenleg nem beköthető (2026-11-17)

- [x] **Transfermarkt újramérés (blokkoló):** a GB1/ES1 sérüléslista és a
      GB1 góllövőlista üres `<tbody>`-t ad (GB1 sérülés 1 sor, ES1 0,
      góllövő 0); a `?ajax=yw1`, a `/plus/1` és a .de domain is üres; az
      Obscura-render ugyanazt adja (0 tábla-sor). A 2026-11-11-i 85 soros
      minta nem reprodukálható → a szerver-renderelt táblák eltűntek.
      Opt-in spike: `rust/crates/ingest/tests/transfermarkt_live.rs`;
      katalógus-státusz frissítve (új, consent-kompatibilis hozzáférési út
      kell az adapter előtt).
- [x] **Tipplap DOM ellenőrzés (2026-11-17):** `/kalkulator/labdarugas/{nap}`
      200, 497 KB; dátum-guard él (`<select id="dates">` selected option =
      kért nap); desktop tábla `#calcTableDesktop`; sorok `data-event` ID-vel,
      név az `a[data-event-name]` szövegében, odds a `td[data-name=H|D|V]`
      alatti `<strong data-odds>` szövegében; kickoff `MM.DD.` + `HH:MM`
      (HU helyi idő), liga-fejléc `th.tleft` (magyar cím). A HU csapat- és
      versenynevek miatt a kanonikus kapcsoláshoz crosswalk (alias-seed)
      kell — enélkül duplikált entitások keletkeznének; ezért a Tipplap
      adapter a crosswalk-szelet után jön.
- [ ] Következő döntési pont a userrel: crosswalk-enabler (v1 alias-seed)
      vagy ESPN licenc-döntés, illetve footballdata ligabővítés.

## S18 — Crosswalk-seed + Tipplap forrás bekötése (2026-11-17)

- [x] **Crosswalk-seed CLI:** `footballctl identity seed-v1 [--apply]
      [--limit N]` a v1 adatbázist **csak olvasva** használja (teams,
      team_aliases, countries); ékezet-független név-egyezés + ország/nem
      kontextus, csak **egyértelmű** találatnál seedel; a linker kulcsa a
      lower(alias), ezért a nyers és a foldolt alak is bekerül. Dev futás:
      **441 csapat / 1342 reviewed alias**, 23 többértelmű, 0 írás dry-run
      nélkül. Visszavonás: `DELETE FROM identity.alias WHERE
      method='import';`. Teszt: seed → linker `reviewed_alias` feloldás
      (ékezet nélküli forrásnévvel is).
- [x] **Tipplap adapter (canary):** `rust-http`, 1 kérés/nap, dátum-guard
      (`#dates` selected), desktop tábla, speciális tippek kiszűrve,
      HU helyi idő → UTC (Europe/Budapest), szezon-címke a dátum alapján,
      Tippmix 1X2 quote-ok (opening, `provisional`). Verseny-fixszótár a v2
      kanonikus nevekkel (Nations League A–D, Africa Cup - Qualification,
      CONCACAF B, Friendly International, PL, La Liga, Serie A, Bundesliga,
      Ligue 1, NB I); ismeretlen liga → sor kimarad (nincs új entitás).
      Tesztek: parse/dátum-guard/üres tábla/speciális tipp.
- [x] **Canary-eredmény (2026-11-17):** 92 sor → 36 meccs a meglévő
      Soccerway-sorokhoz illesztve, **0 duplikált csoport**, napi össz
      190 változatlan; 36 Tippmix snapshot mind `provisional`;
      `Kyrgyzstan–Vietnam 2.55/3.25/2.33` a nap-API-ban.
      Nap-API javítás: fázis-preferencia (closing, ha nincs → opening,
      `DISTINCT ON` + `LATERAL`) és Tippmix-beemelés a COALESCE-láncba
      (B365 után) — enélkül a Tipplap-odds nem jelent meg.
- [x] **Zászló-tábla egységesítés:** `St. Lucia`/`St. Kitts`/`St. Vincent`
      → `Saint …` kanonikus nevű entitás (a `Saint Lucia` review-t a
      meglévő entitásra jóváhagytuk, ref képződött).
- [x] **Biztonsági javítás:** az agent `run.sh`-ban hardcode-olt OpenFoot
      kulcs helyett Keychain-lekérdezés (a telepítő már így generálja).
      **A korábbi kulcs cseréje javasolt** (a fájlban szerepelt).
- [ ] **Nyitott:** `China`/`Dominica` duplikált kanonikus entitás-párok
      (linker `multiple_candidates`) → identitás-merge szelet;
      `Jordan` Soccerway-review; a Tipplap U19/U20 ligák és a „Nemzetközi
      Torna" nincs a szótárban (szándékos, név-instabilitás).

## S19 — Csapat-entitás merge + tükrözött duplikátumok (2026-11-17)

- [x] **Migráció 0015:** `ops.entity_merge_archive` (entity/team/aliasok/
      refek/meccs-ids/standing/szezon-stat/asset JSONB payload) +
      index; `db-check.sh` és `connection.rs` 47 tábla. Rollback: DROP.
- [x] **Storage (`repositories/identity_merge.rs`):**
      `duplicate_team_groups` (azonos lower(név) + azonos kontextus, ≥2),
      `merge_team_preview` (száraz előnézet), `merge_team` (tranzakciós
      merge: archívum → meccs/standing/szezon-stat/asset/alias/ref
      átmozgatás konfliktus-eldobással → áldozat team-sor törlés →
      entitás `merged_into_id` → audit `team_merge`). Csak azonos név +
      azonos kontextus; fuzzy tilos.
- [x] **CLI:** `identity duplicates`, `identity merge-team --from --to
      [--note] [--apply]` (dry-run alap, előnézet-számokkal). Doksi:
      `docs/operations/identity.md` 7. pont.
- [x] **Éles merge-ek (2026-11-17):** China, Denmark U17, Dominica
      ikerentitások összevonva (3 archívum + 3 audit); a China/Dominica
      pending review-k a túlélőre jóváhagyva (forrás-ref képződött).
- [x] **Tükrözött duplikátumok (a források fordított home/away oldallal is
      listázzák ugyanazt a meccset):** a `duplicate_groups` LEAST/GREATEST
      kulccsal csoportosít (HashMap-alapú, nem szomszédosság-alapú — ez a
      korábbi csoportosítás hibája volt), az `is_same_match` elfogadja a
      tükrözött párt, de **eredmény esetén elutasít** (orientáció-veszély);
      a merge tükrözött párnál nem másol eredményt/oddsot/statisztikát.
      Éles: 24 + 6 tükrözött merge (190 → 166 meccs), **2 pár kézi
      döntésre vár** (Canada–Guatemala és Malaysia–Yemen: a források
      ellentmondó orientációjú, azonosnak tűnő eredményt adnak).
- [x] **Fordított oldalú materializálás:** ha a forrás meccse fordított
      home/away-vel jön, a materializer a meglévő kanonikus sort használja
      (nem hoz létre tükrözött új sort), az 1X2 oddsot oldalcserével írja,
      eredményt/statisztikát/referee-t/provenance-t nem ír. Élő igazolás:
      Tipplap „Qatar–South Korea" (6.75/4.35/1.33) → kanonikus
      „South Korea–Qatar" (1.33/4.35/6.75), napi létszám változatlan.
- [x] **Tipplap lefedettség a merge után:** China/Dominica review-k
      jóváhagyásával és a javított materializerrel a Tippmix-odds 39
      meccsre került fel (nap-API: 166 meccs, 39 odds, provisional).
- [x] Kapuk: teszt-DB reset → `check-foundation` OK (47 tábla) →
      workspace tesztek zöld (`--test-threads=1`) → clippy/fmt tiszta →
      teszt-DB resetelve. Tesztek: `storage/tests/team_merge.rs`,
      `dedupe_merge.rs` (tükrözött merge + eredményes elutasítás).

## S20 — Oldal-orientáció megszilárdítása + ref-javítás (2026-11-17)

- [x] **Gyökérok:** a Soccerway renderenként **tükrözheti az egész
      fixture-t** (kijelzés, slug-szöveg és pontszámok együtt), miközben
      a meccs-URL `<slug>-<Code>` kódja stabil csapat-ID marad. A parser
      a megjelenítési sorrendet használta → fordított kijelzésnél a
      kód↔név párok felcserélődtek (rossz team-refek, home=away crash).
- [x] **Parser-javítás:** orientáció a slug sorrendjéből; a kijelzési
      neveket a slug-szöveghez párosítja (fold-egyezés), fordított
      kijelzésnél a pontszámokat/büntetőket oldalra cseréli. Új teszt:
      `flipped_display_row_keeps_slug_orientation_and_swaps_scores`.
- [x] **Ref-javítás (ops, auditálva):** stabil kód→slug-szöveg térkép két
      angol renderből (332 kód, 0 ellentmondás) → 188 ref korrigálva,
      16 pótolva (`identity_ref_repair`), 74 elavult/keresztbe párosított
      ref superseded (`identity_ref_supersede`). A kételkedő, első körös
      17 ref-változtatást visszavontam (a létrehozási observation
      keresztezett volt).
- [x] **Materializer-őr:** azonos kanonikus csapat a két oldalon →
      figyelmeztetéssel kihagyás (nincs `match_check` összeomlás);
      fordított forrás-sor a meglévő sorra megy, 1X2 odds oldalcserével.
- [x] **Dedupe-politika (lock-first):** tükrözött pár merge-elhető, ha nem
      veszne el egyetlen eredmény sem (elutasítás csak: áldozatnak van
      eredménye, túlélőnek nincs); `is_same_match` ugyanezt a szabályt
      követi; a CLI egy hibás/kockázatos párt kihagy, nem áll meg
      (`kihagyva` számláló). Tesztek frissítve.
- [x] **Napi állapot helyreállítva:** 249 meccs, **0 duplikált csoport**,
      mind `verified_publish`, 39 Tippmix-odds, **0 pending review**;
      a vitatott párok lock-first alapon egy sorra olvadtak
      (Guatemala–Canada 4–1, Malaysia–Yemen 2–1); a téves, keresztezett
      Jordan review elutasítva, a helyes kódú (vNcmJoU2) approve-create,
      a többi új csapat jóváhagyva.
- [x] Kapuk: teszt-DB reset → `check-foundation` OK (47 tábla) →
      workspace tesztek zöld → clippy/fmt tiszta → teszt-DB reset;
      agent a végleges binárisokkal.
- [ ] **Ismert korlát:** semleges pályás válogatott-meccseknél az upstream
      home/away kétértelmű lehet (lock-first + második forrás dönt);
      a Soccerway lokalizált verseny-nevei (`Kings Cup - Thaiföld`) még
      churnt okozhatnak — a crosswalk-projekt feladata.

## S21 — Verseny-név stabilizálás kezdete (2026-11-17)

- [x] **Fix verseny-szótár (`ingest/competitions.rs`):** foldolt kulcsú
      tábla a lokalizált/változat címekre (AFC/Afrika Kupa selejtezők,
      Nations League A/B, Euro U17/19/21 selejtezők, Kings Cup, Copa
      Colombia, Brazil Kupa U20, SAFF, USL, Torneo Federal, Liga MX Women,
      IWL, FA Cup) a dev-ben már meccset tartó angol nevekre; ország-
      kitöltéssel (CO/BR/US/AR/MX/IN/GB). Ismeretlen cím változatlan.
- [x] **Materializer:** a verseny-meta kanonizálása a nap elején; országon
      belül pontos, majd foldolt név-egyezés (csak egyértelmű találatnál).
      Tesztek: szótár-unit tesztek + integráció (két lokalizált cím →
      egy verseny-entitás, két meccs).
- [x] **Canary:** Soccerway-futás a szótárral → **nem jött létre új
      verseny-entitás** (130 maradt); a mai felgyűlt 20 churn-duplikátum
      dedupe-pal zárva (275 → 255 meccs, 0 duplikált csoport); 3 új csapat
      jóváhagyva, 0 pending review.
- [ ] **Nyitott:** 1 sor `withheld` — a forrás csak hosszabbításos
      eredményt ad (USL Play Offs), a 90 perces eredmény hiányzik → helyes
      fail-closed; ET-eredmény publikálási szabálya külön döntés.
- [ ] **Következő:** a 0-meccses, elhagyott verseny-entitások (régi
      lokalizált nevek) opcionális tisztítása; majd S22 (ligabővítés).

## S22 — Forrás-lefedettség bővítés (2026-11-17)

- [x] **footballdata +17 divízió:** E1–E3, EC, SC0–SC3, D2, I2, SP2, F2,
      N1, B1, P1, T1, G1 (22 kérés/nap, budget 25 → 40). Tesztek: 22 kérés
      terve, új liga-metak (Championship → GB, tier 2, priority 12).
      Élő: 50 footballdata kérés ma, mind 200; a bootstrap **100 új csapatot**
      hozott review-ba → approve-create-all, majd újrafutás; 40 játékvezető
      review szintén approve-create. Az új ligák ablakában a válogatott
      szünet miatt nincs sor (a CSV-k a szünet előtti fordulóig frissülnek)
      — a lefedettség a következő fordulónál jelenik meg; nincs duplikátum.
- [x] **Tipplap szótár-bővítés:** `NB II` (HU, tier 2, priority 10) és
      `CONCACAF Nemz.Ligája A/C div` → League A/C (a v1 confirmed címek
      alapján); unit teszt. Az U19/U21 Eb-selejtező kimarad (forduló a
      kanonikus névben — round_label döntés kell).
- [x] **Kockázat javítás:** a korábbi, találgatásos „CONCACAF Nations
      League" (ország nélküli) szótár-bejegyzést eltávolítottam — a
      League A/B meccseket tévesen Play-in alá sorolta volna.
- [x] Kapuk: teszt-DB reset → `check-foundation` OK (47 tábla) → teljes
      workspace teszt zöld → clippy/fmt tiszta → teszt-DB reset; agent a
      végleges binárisokkal. Napi állapot: 255 meccs (2026-11-17),
      0 duplikált csoport, 0 pending review, Tippmix-odds 39 meccsen.
- [ ] **Következő:** OpenFoot registry-nevek a verseny-szótárba
      (adatvezérelt, a S23 backfill-leltárral); az U19/U21 forduló-modell
      (round_label) külön szelet; S23 backfill-terv.

## S23 — Történelmi backfill: leltár + motor (2026-11-17)

- [x] **Leltár (S23a):** `docs/research/backfill-inventory-2026-11-17.md` —
      footballdata mmz4281: 22 liga, **711 liga-szezon CSV** (1993/94–
      2026/27); footballdata `new/`: **15 liga egy-egy fájlban** (2012+,
      más séma, SUI kivételével); v1 DB: **1 467 472 meccs**, 234 916
      oddsszal (3,82M sor), 1406 verseny; OpenFoot nap-feed. Terv:
      `docs/superpowers/plans/2026-11-17-backfill.md` (KPI-k, fázisok
      S23b–S23f, kockázatok).
- [x] **Backfill motor (S23b):** új job-típus (`domain='backfill'`,
      `time_window=<szezonkód>`, `entity_key=<liga>`); a tick ilyenkor
      szezonkódos időablakot is elfogad; a footballdata adapter egy
      liga-szezon CSV-t kér (`mmz4281/{szezon}/{LIGA}.csv`), szezon-módban
      **nincs ±7 napos ablak**, a szezon-címke a kódból jön, a sorok
      dátum-sávja (±2 hónap tűrés) tartalmi őr, üres eredmény → fail-closed.
      CLI: `footballctl backfill plan --from --to --leagues all|lista
      [--apply]`, idempotens, priority 200 (az élő napi jobok előbb futnak).
      Tesztek: egy kéréses terv, ablak nélküli parse, sávon kívüli CSV →
      SchemaDrift.
- [x] **Canary: Championship 2025/26** — 552 meccs, **552 eredmény,
      552 odds**, 0 duplikált csoport; 20 új csapat + 28 játékvezető
      approve-create; a keret ideiglenesen 60, utána vissza 40.
- [ ] **Következő (S23c):** 2015+ mind a 22 ligára (≈242 fájl ≈ 6-7 nap a
      40/nap kerettel) kötegelt approve-create-pel; coverage CLI
      (`footballctl coverage`) a KPI-méréshez; utána S23d (mély múlt +
      new ligák) és S23e (v1 import).

## S23c — Backfill köteg indítva (2026-11-17)

- [x] **2015–2026 köteg sorba állítva:** 263 liga-szezon job (22 liga ×
      12 szezon, a canary E1 2025/26 már kész). A daemon 10 job/kör
      tempóban dolgozza fel; a keret ideiglenesen 450, a köteg után 60.
- [x] **Archívum-sajátosságok javítva:** régi szezonok `Time` oszlop
      nélkül → `date_only`; 2 jegyű év (`13/08/16`) → plausibilitás-alapú
      újraértelmezés; érvénytelen odds-cellák (0) → kihagyás (DB
      `price > 1`). Mind a három hiba élő jobokon derült ki és javítva.
- [x] **Coverage CLI:** `footballctl coverage [--from] [--to]` —
      liga-szezononként meccs, eredmény %, odds %; a canary sora
      (Championship 2025/26: 552 meccs, 100% eredmény, 100% odds).
- [x] **Runbook:** `docs/operations/backfill.md` — sorba állítás, tempó,
      bootstrap-kör (approve-create → kész jobok újrafuttatása), coverage,
      visszaállítás.
- [ ] **Futás közben:** a batch a háttérben folytatódik; jelenleg
      done ≈ 15 / pending ≈ 249, matches ≈ 5 500, pending review 0–1.
      Megfigyelt tempó: **~1 job/tick** (a tick-en belüli delay-kapu
      miatt), így a köteg ~4 óra; a napi keret (450) a köteg végéig
      marad, utána 60-ra állítandó.
      A köteg végén: `identity approve-create-all`, majd a kész jobok
      egyszeri újrafuttatása (a jóváhagyás előtt kimaradt meccsek
      pótlására), végül a budget visszaállítása 60-ra.

## S23d — Mély múlt + „new" ligák (2026-11-17)

- [x] **Batch állapot:** 2015–2026 köteg: done ≈ 57, matches 13 300+;
      a napi kvóta (450) ma elfogyott (692 kérés a diagnosztikákkal),
      a maradék holnap folytatódik. 2 hibás job (i1/f1 2015/16) oka üres
      dátumú sor → a parser most kihagyja a hibás sorokat (decoy-őr marad),
      a jobok újra sorba állítva. 46 új csapat/játékvezető jóváhagyva.
- [x] **Elő-2015 köteg:** 447 job sorba állítva (1993–2014, 37 kihagyás a
      liga első szezonja előtt).
- [x] **„new" ligák adaptere:** `new/{LIGA}.csv` (15 ország, egy fájl =
      sok szezon, más séma): ország+liga+szezon oszlopok, helyi idő →
      UTC ország-időzóna-táblával, szezon-címke normalizálás
      (`2012/2013` → `2012/13`), záró odds (B365/Max/Avg/BFE/PSC),
      hiányzó idő → date_only. Teszt: plan + parse (UTC-konverzió,
      szezon-címke, odds). 15 new-liga job sorba állítva.
- [ ] **Következő:** a teljes backlog (657 pending job) feldolgozása a
      keret-reset után; bootstrap-kör (approve-create + kész jobok
      újrafuttatása); coverage-riport; utána S23e (v1 import).

## S23e — v1-leltár eszköz (2026-11-17)

- [x] **`footballctl backfill v1-inventory --from-year --to-year --top`**
      (read-only v1): verseny+ország+szezon bontásban v1 meccs/odds%,
      v2 visszaigazolás foldolt név+ország+év kulccsal. Eredmény: a
      legnagyobb v1-többlet a mély alsóbb ligák (Serie D, Regionalliga,
      Provincial — odds nélkül); a v1 odds-gazdag ligái nagyrészt már
      fedettek; **marginális érték: Ekstraklasa (PL) és Eliteserien (NO)**
      (~4 000 meccs, ~4 000 oddsszal) → ezek az S23e pilot céljai.
- [ ] **Következő:** pilot import egy verseny-szezonra (v1 → v2
      normalizált sorok, trust medium), coverage/duplikátum/trust
      ellenőrzés; a teljes leltár a checkpoint 2026-11-18 listájában.
- [x] **Pilot dry-run (`backfill v1-pilot`):** Eliteserien (id 27):
      4 010 meccs, 3 933 oddsszal (98,1%); Ekstraklasa (id 28): 4 562
      meccs, 4 177 oddsszal (91,6%); a csapatok 0%-a létezik v2-ben (új
      ligák) → az importnak a **kétfázisú bootstrapot** kell használnia
      (linker-review → approve-create → újrafuttatás), nem közvetlen
      név-illesztést; a v1 odds-piacok: `1X2 H/D/A` (AH/OU is van, a
      pilot csak 1X2-t visz).

## S23e — v1 import író ág + trust-javítás (2026-11-17)

- [x] **`backfill v1-import --tournament --from --to [--apply]`:** a v1
      (football_app) meccsei + 1X2 záró/nyitó oddsai a normál
      observation → materializer → trust/gate láncon mennek át; a tick
      `verify_and_publish` függvénye publikus lett és újrahasznosított.
      Kétfázisú bootstrap (review → approve-create → újrafuttatás).
- [x] **Pilotok:** Eliteserien (id 27) **4 010 meccs, 3 977 eredmény,
      3 933 odds**; Ekstraklasa (id 28) **4 562 meccs, 4 380 eredmény,
      4 177 odds** — mind provisional (egy közepes forrás).
- [x] **Kritikus trust-javítás:** `load_material_values` eddig kanonikus
      meccs-ID-ra szűrt, de a megfigyelések forrás-kulcsot hordoznak → a
      konfliktus/trust-szabály forrásérték nélkül futott, egyetlen forrás
      is „verified" lehetett. Új út: provenance → megfigyelés → aktív ref
      → kanonikus csapat-ID, tükrözött soroknál pontszám-igazítás.
      Tesztek frissítve (megerősítő forrás provenance+ref).
- [x] **`footballctl reverify --from --to`:** fetch nélküli
      újraellenőrzés; 39 219 meccs újraszámolva → verified 4 069,
      provisional 36 776, withheld 2 (a valós forrás-összetétel szerint).
- [x] **`footballctl data-status`:** liga/ország/szezon riport
      (meccs, eredmény %, odds %, verified/provisional/withheld),
      összesítés + duplikált csoport + pending review. Doksi:
      `docs/operations/data-quality.md`.
- [x] **Latin-1 fallback:** régi EC/SC CSV-k nem UTF-8 → Latin-1
      dekódolás; az 5 hibás job újraindítva. 19 új review jóváhagyva,
      pending 0.
- [x] Kapuk: teljes workspace teszt zöld, clippy/fmt tiszta, foundation
      OK (47 tábla), teszt-DB reset; agent a végleges binárisokkal.
