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
