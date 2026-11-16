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
