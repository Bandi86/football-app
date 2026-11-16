# Automatizálás — napi worker-daemon (launchd)

A `footballd` a napi adatgyűjtést, a worker-t és a helyi API-t egy
folyamatban futtatja. Automatikus napi működéshez felhasználói
LaunchAgent indítja (`hu.bandi86.football.v2`), `KeepAlive`-dal.

## 1. Előfeltételek

- `rust/target/debug/footballd` **obscura feature-rel** építve:
  `cd rust && cargo build -p footballd --features obscura`
  (feature nélkül a Soccerway jobok `transport hiba`-val buknak).
- `.env` a repó gyökerében (DATABASE_URL, TEST_DATABASE_URL) — a burkoló
  (`scripts/run-footballd.sh`) betölti.
- Obscura bináris: `OBSCURA_BIN` env vagy az alapértelmezett
  `../football-app/tools/obscura/obscura` (lásd `obscura.md`).

## 2. Telepítés / eltávolítás

```bash
./scripts/install-footballd-agent.sh            # telepít + elindít
./scripts/install-footballd-agent.sh --uninstall
```

A telepítő a `~/Library/LaunchAgents/hu.bandi86.football.v2.plist` fájlt
írja, majd `launchctl bootstrap gui/$UID` + `kickstart` indítja.

## 3. Működés

- A worker 60 s-enként tickel; a napi `discovery` esemény forrásonként
  **egy napi jobot** generál (`domains = ["day"]`).
- Soccerway: napi 1 betöltés (ma) + 1 (tegnap ablak) render; a parse
  **minden versenyt** besorol; a napi keret 6.
- Footballdata: napi 1 job, ligánként 1 CSV (5 kérés, 2 s várakozással).
- A kvóta fetch-enként számol; a breaker/robots/min_delay őrök
  változatlanok.
- A kliens a `~/Library/Application Support/FootballAppV2/runtime.json`
  fájlból találja az API portját/tokenjeit.

## 4. Ellenőrzés

```bash
launchctl print gui/$UID/hu.bandi86.football.v2 | head -20
tail -f ~/Library/Logs/FootballAppV2/footballd.log
./rust/target/debug/footballctl worker status
```

## 5. Visszaállítási út

- `./scripts/install-footballd-agent.sh --uninstall` (agent + plist).
- A daemon leállítása után a kézi `footballctl worker tick` továbbra is
  működik.
- A logok a `~/Library/Logs/FootballAppV2/` alatt maradnak (kézzel
  törölhetők).