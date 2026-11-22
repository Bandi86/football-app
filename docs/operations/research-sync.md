# Research szinkron (napközbeni adatfrissítés)

A meccsrészletező idővonala és statisztikája a research envelope-ból
(`api/v1/daily/<év>/<hó>/<nap>.json` + `api/v1/matches/.../<detail>.json`)
töltődik. A research gyűjtő napi launchd-ágense (`com.footballresearch.daily`)
le van tiltva, ezért a v2-ben a frissítés kézi vagy külső ütemezésű.

Ez a dokumentum a v2-oldali, egyetlen belépőpontot írja le:
`scripts/research-sync.sh`.

## Mit csinál

1. lefuttatja a research projekt `scripts/collect-only.sh` pipeline-ját az
   adott napra (fetch + enrich + odds + gaps + JSON export);
2. berak egy `research` / `discovery` jobot a v2 `ops.job` táblájába, hogy a
   futó worker betöltse az eseményeket (`fact.match_incidents`) és a
   csapatstatisztikát (`fact.match_stats`).

A script idempotens: ha az elmúlt 20 percben már van pending/leased discovery
job ugyanarra a napra, nem rak be újat; lock-fájllal védi a párhuzamos futást.
Napló: `~/Library/Logs/FootballAppV2/research-sync.log`.

## Használat

```bash
# mai nap (Europe/Budapest)
scripts/research-sync.sh

# adott nap
scripts/research-sync.sh 2026-11-21

# csak job (az envelope-ot már máshonnan frissítetted)
scripts/research-sync.sh --enqueue-only

# a research gyűjtő másik helyen van
FOOTBALL_RESEARCH_ROOT=/path/to/football-research scripts/research-sync.sh
```

Előfeltétel: a repó `.env`-jében legyen `DATABASE_URL`, és a research projekt
`scripts/collect-only.sh` létezzen (alapból
`~/Desktop/football-research`).

## Ütemezés (telepítve: `hu.bandi86.football.researchsync`)

A script maga nem telepít agentet, de a gépre **2026-11-22-től** van egy
külön launchd agent, ami napi 5 slotban (14:15, 16:15, 18:15, 20:15, 22:15,
Europe/Budapest) hívja:

- plist: `~/Library/LaunchAgents/hu.bandi86.football.researchsync.plist`;
- futtatott script: `~/Library/Application Support/FootballAppV2/agent/research-sync.sh`
  (a repó `scripts/research-sync.sh` másolata — a script a saját könyvtárának
  szülőjében keresi a `.env`-t, ezért kell egy szinttel lejjebb lennie);
- környezet: `~/Library/Application Support/FootballAppV2/.env` (600),
  `FOOTBALL_RESEARCH_ROOT` a valódi Library-beli könyvtárra mutat
  (a `~/Desktop/football-research` csak szimbolikus link rá);
- log: `~/Library/Logs/FootballAppV2/research-sync.log` és
  `research-sync-launchd.log`.

Ellenőrzés:

```bash
launchctl print gui/$(id -u)/hu.bandi86.football.researchsync | grep -E 'state =|interval'
launchctl kickstart -k gui/$(id -u)/hu.bandi86.football.researchsync   # próbafuttatás
```

Visszafordítás: `launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/hu.bandi86.football.researchsync.plist`
(az adatokat nem törli). Az eredeti `com.footballresearch.daily` agent továbbra
is le van tiltva — az a Markdown renderelő pipeline, erre nincs szükség.

## Ütemezési minta (dokumentáció, nem telepített rész)

A fenti agent nélkül egy launchd agent hívhatta volna (példa):

```xml
<key>StartCalendarInterval</key>
<array>
  <dict><key>Hour</key><integer>14</integer><key>Minute</key><integer>15</integer></dict>
  <dict><key>Hour</key><integer>16</integer><key>Minute</key><integer>15</integer></dict>
  <dict><key>Hour</key><integer>18</integer><key>Minute</key><integer>15</integer></dict>
  <dict><key>Hour</key><integer>20</integer><key>Minute</key><integer>15</integer></dict>
  <dict><key>Hour</key><integer>22</integer><key>Minute</key><integer>15</integer></dict>
</array>
```

A telepítés (launchctl bootstrap) üzemeltetési döntés; a script futtatása
önmagában is elég, ha kézzel indítod.

## Ellenőrzés

```bash
# fut-e a job, és mennyi esemény van a napra
psql "$DATABASE_URL" -c "
  SELECT status, attempts, last_error FROM ops.job
  WHERE event_type='discovery' AND time_window='2026-11-21'
  ORDER BY created_at DESC LIMIT 3;"

psql "$DATABASE_URL" -c "
  SELECT count(*) AS incidents, count(DISTINCT match_id) AS matches
  FROM fact.match_incidents i JOIN core.match m ON m.id=i.match_id
  WHERE (m.kickoff_at AT TIME ZONE 'Europe/Budapest')::date='2026-11-21';"
```

A worker a jobot a következő tickben (max. ~1 perc) felveszi; a részletes
események ezután jelennek meg az API-ban és a kliens idővonalán.

## Visszaállítás

A script nem módosít adatot a research projekten kívül; a berakott job
törölhető:

```bash
psql "$DATABASE_URL" -c "DELETE FROM ops.job WHERE entity_key LIKE 'research-sync-%' AND status='pending';"
```

A research envelope kézi visszaállítása nem szükséges: a következő gyűjtés
felülírja.