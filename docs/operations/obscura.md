# Obscura browser-fetcher — üzemeltetési útmutató (S2c)

Az ágens `obscura` feature mögötti browser-fetcher a JS-renderelt forrásokhoz
(pl. Soccerway). Alapállapotban **KI**: a default build nem tartalmazza a
CDP-klienst, a worker pedig csak a `source_policy.transport = 'obscura'`
soroknál választja.

## 1. Bináris beszerzés

A bináris **nem** része a repónak (~72 MB, `arm64`), ezért operátori lépés:

- Helyezd a következőre: `tools/obscura/obscura` (gitignore), vagy állítsd
  be az `OBSCURA_BIN` env változót.
- Ezen a gépen a v1 archívum v0.2.6-os példánya újrahasználható:
  `../football-app/tools/obscura/obscura` (a tesztek alapértéke is ez).
- Letöltés: a GitHub release oldal URL-je ezen a hálózaton 504-gyel elakad;
  az asset API út működik:
  `https://api.github.com/repos/h4ckf0r0day/obscura/releases/assets/<ID>`
  (az `<ID>` a release JSON `assets[].id` mezője).
- Ellenőrzés: `tools/obscura/obscura serve --help` kiírja a kapcsolókat.

## 2. Konfiguráció (env)

| Változó | Alapérték | Jelentés |
|---|---|---|
| `OBSCURA_BIN` | `tools/obscura/obscura` | a futtatható bináris |
| `OBSCURA_URL` | — | külső (már futó) obscura base URL; ha be van állítva, a footballd nem indít processzt, csak health-checkel |
| `OBSCURA_PORT` | `9222` | a helyben indított `obscura serve` portja (csak `OBSCURA_URL` nélkül) |
| `OBSCURA_TIMEOUT_MS` | `45000` | a fetcher teljes időtúllépése |

A `footballd` a `source_policy` táblából dönt: ha van **engedélyezett**
`transport = 'obscura'` policy, lazy elindítja az `obscura serve`-t
(`--host 127.0.0.1 --workers 1 --user-agent football-app-v2
--allow-private-network`), `/json/version` health-checkkel (~10 s), és a
leálláskor kill+wait-tel zárja. Ha nincs ilyen policy, a szerver nem indul.

## 3. Opt-in (forrás bekapcsolása)

1. Build a feature-rel: `cargo build -p footballd --features obscura`.
2. A policy sor átállítása (példa):
   ```sql
   UPDATE ops.source_policy
      SET transport = 'obscura', enabled = true
    WHERE source_slug = '<forrás-slug>';
   ```
3. A daemon következő tickje lazy elindítja a szervert; a worker a
   `Fetchers` szelektorral a policy transzportja szerint választ fetchert.

A robots/budget/breaker/delay őrök változatlanul a workerben futnak a fetch
**előtt** — az Obscura `--obey-robots` nincs bekapcsolva és nem helyettesíti
a worker robots-kapuját.

## 4. Biztonsági jegyzetek

- **Stealth KI** (charter 9. pont): a `--stealth` flag nem kerül át, a
  blokk-megkerülés tilos.
- **`--allow-private-network`:** az Obscura alapból blokkolja a
  loopback/RFC1918 fetch-eket (SSRF-védelem). A szerver mégis ezzel a
  flaggel indul, mert a lokális CDP-tesztek loopback wiremockra mennek.
  A worker URL-jeit a robots + policy őr szűri; a fetcher csak a policy
  által engedélyezett URL-t kapja. A flag bekapcsolása önmagában nem ad
  új támadási felületet a worker felé, de a szervert ne tedd publikusra.
- A `serve` stdout/stderr a `footballd`-ben `null`-ra van irányítva; a
  fetcher hibái típusosak (`ObscuraError`) és a worker hibautain landolnak.

## 5. Tesztek

- Default futás: **0 élő kérés**; a CDP-tesztek lokális wiremockra mennek
  (`cargo test -p football-ingest --features obscura`).
- Élő render teszt (opt-in, robots-engedett Soccerway, 2 kérés):
  ```bash
  OBSCURA_LIVE_TEST=1 cargo test -p football-ingest --features obscura \
    --test obscura_live -- --test-threads=1
  ```

## 6. Visszaállítási út

- **Forrás-váltás nincs** (a dokumentált keret: forrás-csere helyett
  policy-váltás). A visszaállítás:
  ```sql
  UPDATE ops.source_policy SET transport = 'rust_http' WHERE source_slug = '<forrás-slug>';
  ```
  majd a daemon újraindítása — obscura policy nélkül a szerver nem indul.
- A feature kikapcsolása: default build (a kód a `cfg(feature = "obscura")`
  mögött marad, a lockból csak a használt crate-ek maradnak).
- A 0007 migráció visszaállítása: a `source_policy_transport_check`
  visszaállítása `('rust_http','fixture')`-re — csak ha nincs `obscura`
  sor, különben a sorok sértenék a CHECK-et.

## 7. Hibaelhárítás

| Tünet | Ok / teendő |
|---|---|
| `obscura bináris nem található` | `OBSCURA_BIN` beállítása vagy `tools/obscura/obscura` elhelyezése |
| `obscura szerver indítási hiba: health-check timeout` | portütközés (`OBSCURA_PORT`), vagy a bináris nem indult el; nézd meg kézzel: `obscura serve --port <port>` |
| `CDP hiba` a fetchnél | a kapcsolat megszakadt; a worker a breaker/delay őrökkel kezeli |
| `nincs HTTP-státusz a CDP válaszból` | a navigáció nem adott main-document választ (hálózati hiba, blokk) — a fetch failed |
| üres renderelt tartalom | a cél nem JS-renderelt vagy a settle nem elég; parser-szintű decoy-őr kötelező |
