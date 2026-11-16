# Automatizálás — napi worker-daemon (launchd)

A `footballd` a napi adatgyűjtést, a worker-t és a helyi API-t egy
folyamatban futtatja. Automatikus napi működéshez felhasználói
LaunchAgent indítja (`hu.bandi86.football.v2`, KeepAlive).

## 1. Miért az Application Support alatt fut?

A macOS TCC a launchd-ből indított folyamatoknak **nem engedi** a
Documents (a repó) elérését („Operation not permitted"). Ezért a telepítő
minden futásidejű dolgot a
`~/Library/Application Support/FootballAppV2/agent` alá másol:

| Elem | Hely |
|---|---|
| `footballd` bináris | `agent/bin/footballd` |
| Obscura bináris | `agent/bin/obscura` |
| env (600) | `agent/env` (a repó `.env` másolata) |
| futásidejű adatok | `agent/data` (a repó `data/` egyszeri átemelése) |
| indító | `agent/run.sh` |

A repó buildje a forrás; **frissítéshez futtasd újra a telepítőt** (a
binárist és az env-et felülírja; a `data/` megmarad).

## 2. Telepítés / eltávolítás

```bash
cd rust && cargo build -p footballd --features obscura
cd .. && ./scripts/install-footballd-agent.sh
```

Ez másol, plistet ír, `launchctl bootstrap gui/$UID` + `kickstart`.
Eltávolítás (az adatok megmaradnak):

```bash
./scripts/install-footballd-agent.sh --uninstall
```

## 3. Működés

- Worker 60 s-onként tickel; a napi `discovery` forrásonként **egy jobot**
  generál (`domains = ["day"]`).
- Soccerway: napi 1 betöltés (ma) + 1 (tegnap ablak) render; a parse
  minden versenyt besorol; napi keret 6.
- Footballdata: napi 1 job, 5 CSV-kérés (2 s várakozásokkal).
- A kliens a `~/Library/Application Support/FootballAppV2/runtime.json`
  fájlból találja az API portját/tokenjeit.
- A napló: `~/Library/Logs/FootballAppV2/footballd.log`.

## 4. Ellenőrzés

```bash
launchctl print gui/$UID/hu.bandi86.football.v2 | head -20
tail -f ~/Library/Logs/FootballAppV2/footballd.log
./rust/target/debug/footballctl worker status
```

Kézi karbantartó ticknél ugyanazt az adatkönyvtárat használd, mint az
agent (`DATA_DIR="$HOME/Library/Application Support/FootballAppV2/agent/data"`),
különben a nyers objektumtár szétválik.

## 5. Visszaállítási út

- `--uninstall` → plist törlése, a futó folyamat leáll.
- Az `agent/` könyvtár kézzel törölhető; a repó `data/` érintetlen
  (a másolás után is megmaradt).
- A kézi `footballctl worker tick` továbbra is használható.