# S2c — Obscura browser-fetcher integráció (design)

**Dátum:** 2026-11-14
**Státusz:** design jóváhagyva szekciónként a felhasználó által; spec review-kapu előtt
**Bemenetek:**
- [S1 kutatási spec](2026-11-11-real-data-sources-research-design.md) §3.3 (Obscura szerepe);
- [S2a spec](2026-11-12-s2a-ingest-framework-design.md) (Fetcher trait, ops.source_policy.transport);
- v1 [obscura-transport-probe](../../../../football-app/docs/obscura-transport-probe.md) (2026-11-09, ez a gép);
- S2c spike (2026-11-14, /tmp/s2c-spike, throwaway) — chromiumoxide 0.9.1 → Obscura CDP;
- AGENTS.md (crate-határok, charter 9. pont: stealth/blokk-megkerülés tilos).

## 1. Hatókör

Az S2c az Obscura browser-fetcher réteget integrálja a Fetcher trait mögé,
**opt-in** szállításként a JS-renderelt forrásokhoz.

### 1.1 Spike-előzmény (2026-11-14, ezen a gépen)

A v1 probe (2026-11-09) már igazolta az Obscura engine feasibility-t
(v0.2.6 macOS arm64, CDP serve). Az S2c spike a **Rust CDP kliens**
ismeretlent oldotta fel:

- **chromiumoxide 0.9.1** kapcsolódik az Obscura CDP-hez
  (`ws://127.0.0.1:9222/devtools/browser`, Chrome/145, Protocol 1.3)
- `page.goto` renderel (~3.6s JS-oldalon); `page.content()` a goto utáni
  **1.5s settle + retry** után stabil (Obscura context-id időzítés)
- **Tartalmi többlet igazolt:** Soccerway /matches/ — obscura **2060**
  `wcl-tableScore` marker vs reqwest **0** (3.7MB renderelt vs 411KB nyers)
- **Nyitott detail:** a HTTP-státusz capture `event_listener`-rel üres
  maradt → a designban explicit `Network.enable` + response stream kell
- **Feature-tanulság:** chromiumoxide default-features=false,
  features=["rustls","zip8"] (a fetcher crate zip feature-t igényel;
  tokio-runtime feature nincs, beépített)

### 1.2 Határvonalak (jóváhagyott döntések)

- **Opt-in infrastruktúra:** egyetlen forrás policy-ja sem vált át
  `transport=obscura`-ra automatikusan; a választás logika + egy
  teszt-forrás bizonyítja. Az éles forrás-váltás külön későbbi szelet (S3).
- **Default rust-http:** az Obscura csak ahol a spike többletet igazolt
  (JS-renderelt: Soccerway, PL sérülés, FotMob nap-feed). SSR oldalak
  (ESPN, Transfermarkt) maradnak HTTP-n — az obscura ott rosszabb/felesleges.
- **Stealth KI** (charter 9. pont); a `--stealth` flag nem kerül át.
- **Nem változik:** a worker őrök (robots/budget/breaker/delay), a parser
  decoy-200 őr, a gate, a domain, a Fetcher trait.

### 1.3 Nem célok (YAGNI)

- Nincs forrás-váltás ebben a szeletben (opt-in infra).
- Nincs `page_action` / `raw_body` támogatás az obscura-ágon (a v1 korlátja;
  nyers JSON/gzip források maradnak rust-http-n).
- Nincs Chromium/Playwright fallback (az Obscura önálló bináris, Chrome nélkül).
- Nincs általános SSR-csere (az obscura csak JS-renderelt többletnél).

## 2. Architektúra

```
footballd (daemon)
  ├── ObscuraServer (lazy start/stop, CDP 9222)  ← csak ha van obscura forrás
  └── worker loop
        └── fetcher-választás policy.transport szerint
              ├── rust_http → RustHttpFetcher (meglévő)
              ├── obscura   → ObscuraFetcher (ÚJ, chromiumoxide CDP)
              └── fixture   → FixtureFetcher (meglévő)
```

### 2.1 Crate-határok

- **`ingest` crate bővül** egy `obscura` **cargo feature** mögött
  (default KI — a chromiumoxide nehéz függőség):
  - `src/obscura/mod.rs`: `ObscuraFetcher` (a `Fetcher` trait impl-ja)
  - `src/obscura/server.rs`: `ObscuraServer` (start/stop, CDP URL, health-check)
  - A `Fetcher` trait (ingest::fetch) változatlan.
- **`worker`/`footballctl`:** fetcher-választás `policy.transport` szerint
  (helper: transport → fetcher).
- **`footballd`:** lazy `ObscuraServer.start` (csak engedélyezett obscura
  forrásnál), shutdown-kor stop.
- **Új függőség:** `chromiumoxide 0.9.1` (optional, obscura feature;
  default-features=false, features=["rustls","zip8"]).
- **Nem változik:** worker őrök, parser decoy-őr, gate, domain.

## 3. CDP fetch-folyam

Az `ObscuraFetcher::fetch(req, limits)` lépései:

1. **Kapcsolat:** chromiumoxide `Browser` a CDP URL-hez; a kapcsolat a
   fetcher példányban él (újrahasznált); handler tokio-taskban.
2. **Új page + Network.enable:** státusz-capture-hoz Network domain +
   `EventResponseReceived` stream.
3. **Navigáció:** `page.goto(req.url)` (vár a load-ra).
4. **Státusz-capture:** a response stream-ből a fő dokumentum
   `response.status` (URL-egyezés, redirect-követés). Ha nincs státusz a
   timeout-on belül → `FetchError::Transport` (fail-closed).
5. **Content delay+retry:** goto után 1.5s settle (Obscura context-id),
   majd `page.content()` 3 retry (1s köz). Context-hiba retry után is →
   `FetchError::Transport`.
6. **Méretplafon:** renderelt HTML byte-jai `limits.max_bytes` ellen.
7. **RawPayload:** `{ bytes, content_type: "text/html", http_status,
   fetched_at: Utc::now(), sha256 }`.
8. **Page close** fetch után; a Browser kapcsolat marad.

**Timeout:** `limits.timeout` a teljes fetch-re; obscura források policy-ja
nagyobb timeout-ot adhat (default `OBSCURA_TIMEOUT_MS=45000`, a v1 mintája).

**Decoy-200 őr:** az adapter parser-ében marad (S2a) — a fetcher csak a
nyers renderelt HTML-t adja, NEM parse-ol.

## 4. Szerver-lifecycle és konfiguráció

**`ObscuraServer`:**
- **start:** `Command::new(obscura_bin).args(["serve", "--port", port,
  "--host", "127.0.0.1", "--workers", "1", "--user-agent", UA])`; child
  process, stdout/stderr naplózva; **stealth NEM**.
- **health-check:** start után poll `http://127.0.0.1:{port}/json/version`
  (max ~10s, 500ms köz). Ha nem indul → `ObscuraError::ServerStart`.
- **stop:** child kill + wait (footballd shutdown).
- **reuse:** `OBSCURA_URL` megadva → nem indít processzt, csak health-check;
  stop nem kill-ol (külső szerver).

**Konfiguráció (env):**

| Env | Default | Jelentés |
|---|---|---|
| `OBSCURA_BIN` | `tools/obscura/obscura` | bináris út |
| `OBSCURA_URL` | (üres) | külső CDP szerver override |
| `OBSCURA_PORT` | `9222` | CDP port |
| `OBSCURA_TIMEOUT_MS` | `45000` | obscura fetch timeout |

**Lazy indítás:** csak ha van engedélyezett `transport=obscura` forrás az
`ops.source_policy`-ban. Ha nincs → nem indul processz.

**Bináris beszerzés:** NEM része a repónak (gitignore, ~72MB). Beszerzés az
operations docban (asset API út: a release URL 504-özött ezen a hálózaton,
az `api.github.com/repos/h4ckf0r0day/obscura/releases/assets/<ID>` működik —
a v1 probe tanulsága). A spike a v1 binárisát reuse-olta.

**Hiányzó bináris:** obscura forrás + nem létező OBSCURA_BIN → naplózott
hiba, a fetch `FetchError::Transport("obscura bináris nem található")`,
budget nem fogyasztódik (v1 őr).

## 5. Hibakezelés és őrök

**Típusos hibák** (`ObscuraError`, thiserror): `ServerStart(String)`,
`BinaryNotFound(String)`, `Cdp(String)`, `NoStatus`, `Timeout`.

**FetchError mapping:** `Cdp/ServerStart/BinaryNotFound → Transport`,
`NoStatus → Transport` (breaker státusz nélkül nem enged át),
`Timeout → Timeout`. A worker a meglévő hibautakon kezeli.

**Őrök (változatlan helyen):**
- robots/budget/breaker/delay: a workerben, fetch ELŐTT (S2a). Az Obscura
  `--obey-robots` NEM helyettesíti a worker robots-kapuját.
- decoy-200: az adapter parser-ében (S2a).
- méretplafon: a fetcherben (limits.max_bytes).
- stealth: KI (charter 9. pont).

**Fail-closed irányok:**
1. Hiányzó bináris / nem induló szerver → forrás failed, budget nem fogyasztódik.
2. Nincs HTTP-státusz → Transport → breaker nem enged át.
3. Context-hiba retry után is → Transport → backoff/breaker.
4. Szerver-processz halála → következő fetch Cdp hiba → footballd újraindíthatja.

**Karantén:** a parse-karantén (S2a) érintetlen.

## 6. Tesztelés és ellenőrzés

Az `obscura` feature mögött (default KI):

1. **Unit:** ObscuraError → FetchError mapping; fetcher-választás transport
   szerint; env config parse.
2. **Server lifecycle (mock/lokális):** hiányzó bináris → BinaryNotFound
   (budget nem fogyasztódik); health-check timeout → ServerStart;
   OBSCURA_URL override → nem indít processzt. Nincs élő hálózat.
3. **CDP fetch (lokális, env-gated):** obscura serve + lokális HTML szerver
   (wiremock statikus HTML, JS-renderelt marker) → fetch → státusz + content
   assert. Bináris meglétéhez kötött (ha nincs, skip). Nincs élő külső kérés.
4. **Élő render (opt-in, OBSCURA_LIVE_TEST=1):** Soccerway /matches/ →
   státusz 200 + marker > 0 (többlet HTTP-hez képest). Kézi/engedélyezett,
   robots-budget.
5. **Regresszió:** default build (feature KI) változatlan — 152+ teszt zöld,
   demo 20/20 verified; contract-tesztek FixtureFetcher-rel érintetlenek.

**Kötelező ellenőrzések:** `./scripts/check.sh`; `cargo test -p
football-ingest --features obscura`; `cargo clippy --workspace --all-targets
-- -D warnings` (default + obscura); magyar visszaolvasás + glyph-scan.

## 7. Sikerkritériumok (Definition of Done)

- Az obscura feature fordul; a default build nem húzza be a chromiumoxide-ot.
- ObscuraFetcher impl Fetcher; server lifecycle (start/health/stop) működik.
- Fetcher-választás transport szerint; opt-in (forrás-váltás nélkül).
- Lokális CDP teszt bizonyítja a státusz + content flow-t; élő render teszt
  (env-gated) a többletet.
- Őrök/gate/demo változatlan (20/20 verified).
- Operations doc: bináris beszerzés (asset API), env config, opt-in, stealth KI.
- 0 élő kérés a default CI-ban; 0 nyers payload a repóban.

## 8. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| Az Obscura saját renderelője (nem Chromium) long-tail Web API eltér | parser-bizonyíték (nem byte-hasonlítás); a spike 2060 markert igazolt Soccerway-en; minden obscura forrásnál külön spike kell |
| A státusz-capture (Network.enable) nem működik minden oldalon | fail-closed (NoStatus → Transport); a lokális CDP teszt bizonyítja a flow-t; éles forrásnál spike |
| A bináris beszerzés hálózati függő (release URL 504) | asset API út dokumentálva; a bináris gitignore, operátori lépés |
| A szerver-processz erőforrás (memória/thread) | --workers 1, --max-connections default 128; a footballd lazy indítása + stop shutdown-kor |
| A chromiumoxide nehéz függőség | obscura cargo feature (default KI); a default build nem húzza be |

## 9. Visszaállítási út

- Kód: feature branch (`feat/s2c-obscura`), commit csak felhasználói kérésre.
- Az obscura feature KI kapcsolása = a default build (nincs obscura ág).
- Szerver: `OBSCURA_URL` üres + nincs obscura forrás → nem indul processz.
- Forrás-váltás nem történt → a production utak változatlanok (v1 rollback minta).

## 10. Következő lépés

A spec felhasználói review-ja után a writing-plans skill következik
(megvalósítási terv: `docs/superpowers/plans/2026-11-14-s2c-obscura.md`).
Implementáció csak a terv jóváhagyása után. **Technikai korlát:** a
subagent-típusok nem elérhetők ebben a sessionben → inline execution
(executing-plans) a fallback, taskonkénti STOP-pontokkal.

## Terv-jegyzetek (2026-11-14, writing-plans)

1. **0007 migráció + Transport::Obscura:** az `ops.source_policy.transport`
   CHECK (0004) csak `rust_http/fixture`-t enged. Az obscura transzporthoz
   a 0007 migráció bővíti a CHECK-et (`+obscura`), és a domain `Transport`
   enum `Obscura` variánst kap. A spec §2 ezt nem említette; a terv Task 1
   pótolja. A 0007 nem ad új táblát (táblaszám 33 marad).
2. **`--allow-private-network`:** az Obscura serve alapból blokkolja a
   loopback/RFC1918 fetch-eket (SSRF-védelem). A lokális CDP teszthez
   (wiremock loopback) a serve `--allow-private-network` flag-gel indul.
   Élesben a worker URL-jeit a robots + policy őr véd; a flag csak a
   lokális teszt miatt kell, külső célokhoz nem szükséges. Biztonsági
   döntés: a fetcher csak a policy által engedélyezett URL-t kapja.
3. **Fetchers szelektor:** a tick jelenleg egyetlen `&dyn Fetcher`-t kap;
   a transzport-szerinti választáshoz a terv `Fetchers` szelektort vezet be
   (rust_http/fixture/obscura), a tick `run_loop`/`process_job` szignatúrája
   változik, a robots a rust_http fetchert használja. A meglévő worker
   tesztek run_loop hívásai frissülnek.

## Implementációs döntés (2026-11-14, Task 3 — felhasználó által jóváhagyva)

**chromiumoxide → saját mini CDP-kliens.** A terv által előirányzott
chromiumoxide 0.9.1 + `Network.enable` alapú státusz-capture bizonyítottan
nem működik az Obscura 0.2.6 payloadjával:

- Az Obscura `Network.responseReceived` eseménye hiányos a chromiumoxide
  tipizált `Response` kötelező mezőihez képest (charset, connectionReused,
  connectionId, encodedDataLength, securityState) → az egész
  `CdpEventMessage` deszerializáció elbukik, az esemény eldobódik. A
  `CdpEvent::Other` fallback csak ismeretlen metódusnévre él, custom event
  nem segít.
- Nyers CDP-probe igazolta: az esemény megérkezik (`"status":200`, helyes
  sessionId); a `Fetch` domain válasz-stádiuma `responseStatusCode: null`;
  a `performance` navigation timing nem implementált; a `scrape` CLI nem
  ad státuszt; második CDP-kapcsolat ugyanahhoz a targethez
  `Target not found` (a targetek nem osztottak kapcsolatok között).

**Döntés:** az `obscura` feature-ből a chromiumoxide kikerül; helyette
`async-tungstenite` 0.32-alapú, saját, lenient (serde_json::Value) minimál
CDP-kliens (`rust/crates/ingest/src/obscura/cdp.rs`) végzi a
createTarget → attachToTarget (flatten) → Network.enable → Page.navigate →
státusz (responseReceived) → Runtime.evaluate (outerHTML) → closeTarget
láncot. A nyers probe minden lépést igazolt; a Fetcher trait, a
szerver-lifecycle és az őrök változatlanok. A döntés egyben eltávolítja a
chromiumoxide és tranzitív függőségeit a lockból.
