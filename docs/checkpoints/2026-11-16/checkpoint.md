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
