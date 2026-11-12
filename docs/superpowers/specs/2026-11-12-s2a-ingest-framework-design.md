# S2a — Ingest keretrendszer: adapter-szerződés, fetcher, worker (design)

**Dátum:** 2026-11-12
**Státusz:** design jóváhagyva szekciónként a felhasználó által; spec review-kapu előtt
**Bemenetek:**
- [S1 forráskatalógus](../../research/source-catalog.md) — jóváhagyott bizalmi mátrix,
  adoptációs sorrend, 18 tételes séma-rés lista, „Felhasználói döntések (2026-11-12)” fejezet;
- [S1 záró review döntések](../../decisions/2026-11-12-s1-review-dontesek.md) — 16 pont;
- [S1 kutatási spec](2026-11-11-real-data-sources-research-design.md) §1.1–1.2, §3.3;
- [Rendszertervezési spec](2026-11-09-football-app-v2-design.md) §7 (gyűjtés, források,
  lefedettség, L0–L8 lánc);
- AGENTS.md (crate-határok, migrációs fegyelem, tesztelés, publikus repó szabályok).

## 1. Hatókör és dekompozíció

Az S2 (ingest keretrendszer) négy alszeletre bomlik, mindegyik saját
spec → terv → implementáció ciklussal, sorrendben:

1. **S2a (ez a spec):** adapter-szerződés + rust-http fetcher + worker
   (kvóta/breaker/ledger/robots/coverage), teljes ütemezési eseménymodell.
2. **S2b:** bizalmi szintes publish gate (a meglévő gate backward kompatibilis
   bővítése a jóváhagyott mátrixszal) + fail-closed identitás-alapok (crosswalk,
   alias, review-sor).
3. **S2c:** Obscura-integráció a Fetcher trait mögé (csak spike-igazolt
   többletnél; stealth tiltott).
4. **S2d:** séma-rések lezárása (a 18 tételes lista; a 18. tétel a 6. döntés
   szerint saját Elo-feature, nem tükör-pillanatkép).

### 1.1 S2a határvonalai (jóváhagyott döntések)

- **Tiszta keretrendszer:** nincs éles hálózati adapter. A szerződés
  szintetikus golden fixture-ökön és contract-teszteken bizonyítható;
  az első éles adapter (footballdata, rang 1) az S3-ban épül a kész
  keretrendszerre. Élő hálózati kérés az S2a-ban 0.
- **Worker a `footballd`-be ágyazva** (tokio-task, `.env` kapcsoló);
  külön daemon nincs. A DB-s lease/breaker/ledger állapot perzisztens,
  így a későbbi külön processz olcsó refactor.
- **Teljes §7.3 eseménymodell** megépül generikusan, adatvezérelt konfiggal
  (napi felfedezés; T−24h/T−3h/T−60m/T−15m; T+5m/T+30m/T+2h/T+24h/T+72h;
  heti hiánypótlás; napi canary; heti schema-drift riport keret).
  A drift-detektálás tényleges tartalma az adapterekkel (S3) jön.
- **Nem változik:** verification/gate (S2b), core/fact/stage séma (S2d),
  a meglévő `footballctl ingest-fixture` út (backward kompatibilis),
  kliens (S8).

### 1.2 Nem célok (YAGNI)

- Nincs deklaratív scrape-motor (B megközelítés elvetve: az egyedi őrök
  kódolása és contract-tesztelése gyengülne).
- Nincs v1 engine-port (C megközelítés elvetve: anti-pattern-kockázat);
  a v1-ből csak a mért policy-értékek jönnek át adatként, a katalóguson keresztül.
- Nincs több-worker párhuzamosság most (a lease-szemantika megépül, de
  egyetlen worker példány fut).
- Nincs új API-endpoint (az Üzemeltetés nézet S8); a worker állapotát
  a `footballctl` adja.

## 2. Architektúra

```
footballd (daemon)
  └── worker loop (tokio-task, be/kikapcsolható)
        │  due_at jobok, lease, kvóta, breaker, robots, coverage
        ▼
      worker crate (ÚJ)  ──▶ storage (DB + objektumtár)
        │  FetchJob → Fetcher hívás (policy-őrök UTÁN)
        ▼
      ingest crate (bővül)
        ├── Fetcher trait: RustHttpFetcher (reqwest) | FixtureFetcher (teszt)
        │     → kizárólag nyers payload → raw.artifact + raw.capture
        └── SourceAdapter trait: meta + plan + parse(nyers) → observationök
              → stage.observation → canonicalize (meglévő) → verification/gate (változatlan)
```

### 2.1 Crate-határok és függőségek

- **Új `worker` crate** (`rust/crates/worker`): ütemező, kvóta/breaker/ledger,
  robots-kapu, coverage-slot kezelés. Függ: `storage`, `ingest`, `domain`.
  Az `ingest` nem függhet a `worker`-től (egyirányú függőség).
- **`ingest` bővülés:** `Fetcher` trait (szállítás) + `SourceAdapter` trait
  (forrás-szerződés). A policy-típusok a `domain`-ban élnek (tiszta, IO-mentes).
- **`domain` bővülés:** `SourcePolicy`, `FetchJob`, `CoverageState`,
  `BreakerState`, error-enumok — üzleti típusok, IO nélkül.
- **`storage` bővülés:** repository-k az új ops táblákhoz (0004 migráció).
- **`footballd`:** worker loop tokio-taskként, `.env` kapcsoló
  (`WORKER_ENABLED`, tick-intervallum); leállás graceful (lease elengedés).
- **`footballctl`:** `worker status` (kvóta/breaker/ledger/coverage összegzés),
  `worker tick --once` (kézi egyszeri futás ops/teszt célra), doctor-bővítés
  (worker állapot, nyitott breaker-ek, karanténok száma).

### 2.2 Szerződések (trait-vázlat)

```rust
// ingest::fetch
#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn fetch(&self, req: &FetchRequest) -> Result<RawPayload, FetchError>;
}
// RawPayload: bytes + content_type + http_status + fetched_at + sha256.
// A Fetcher NEM dönt robotsról/kvótáról/breakerről — azt a worker előtte eldöntötte.

// ingest::adapter
pub trait SourceAdapter: Send + Sync {
    fn meta(&self) -> AdapterMeta; // slug, adapter_version, parser_version, domains, transport
    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest>; // kéréskulcs-szabályok
    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError>;
}
// A parse MINDIG a tárolt nyersből fut (repars-first); a tartalmi őrök
// (decoy, dátum-guard, 90 perces konvenció, séma-drift) a parserben élnek, fail-closed.
```

- `FixtureFetcher`: request-key → golden fixture hozzárendelés (teszt/shadow replay).
- `RustHttpFetcher`: reqwest (rustls), becsületes UA, méretplafon, timeout,
  státusz-osztályozás; élő használat csak S3-tól, forrásonkénti minősítéssel.
- Az Obscura (S2c) ugyanezen `Fetcher` trait implementációja lesz —
  a keretrendszer nem változik.

### 2.3 Új függőségek (indoklással)

| Függőség | Szerep | Indoklás |
|---|---|---|
| `reqwest` (rustls, async) | RustHttpFetcher | érett, karbantartott HTTP-kliens; TLS rustls-szel (openssl-függőség nélkül) |
| `robotstxt` | robots-matcher | Google-féle parser, wildcard/$ támogatás; az S1 `urllib.robotparser` ruling Rust-megfelelője; fail-closed, nem kézi implementáció |
| `wiremock` (dev) | lokális mock szerver | RustHttpFetcher tesztjeihez; élő hálózati kérés nélkül |
| `async-trait` | Fetcher trait async metódus | dyn-kompatibilis async trait a szállítási réteghez; bevált, kis függőség |

A pontos verziók a megvalósítási tervben rögzítendők (AGENTS: ellenőrzött verzió).

## 3. Adatfolyam

### 3.1 A worker tick lépései

1. **Job-claim:** `ops.job` táblából `due_at <= now` sorok, prioritási
   sorrendben (közelgő kickoff → magas prioritású verseny → nyitott
   identitás/konfliktus → hiányzó kötelező adat → archív backfill),
   `FOR UPDATE SKIP LOCKED` + lease (`leased_until`, `worker_id`).
   Lejárt lease visszasorolása duplikáció nélkül.
2. **Policy-őrök sorrendje** (mind a workerben, a hívás előtt):
   robots-kapu (napi cache, fail-closed parse-hibára) → napi kvóta
   (`ops.fetch_ledger` számláló, UTC nap) → breaker (nyitott: skip +
   újraütemezés; fél-nyitott: egy próba) → min-kérés-köz + jitter az
   utolsó kérés óta.
3. **Reserve → run → log** (S1 ruling): minden kísérlet — oversize/timeout/
   hiba is — előre elszámolt és naplózott a ledgerbe.
4. **Fetch + L0 átvétel:** státusz, content-type, méretplafon, hash,
   duplikátum-szűrés → tömörített, tartalom-azonosítós `raw.artifact` +
   `raw.capture` (azonos tartalom egyszer tárolódik).
5. **Parse a tárolt nyersből** → observationök; a tartalmi őrök fail-closed:
   gyanús payload → karantén, nyers megőrzve, nincs stage-rekord.
   Parse-hibánál először repars a nyersből, nem vak újrakérés.
6. **Meglévő lánc változatlanul:** `stage.observation` → canonicalize
   (idempotens) → verification → publish gate → `/v1/day`.
7. **Tick-zárás:** ledger-eredmény (státusz, bájt, latency), coverage-slot
   állapot, breaker-frissítés, job kész + következő `due_at` az
   eseménymodellből.

### 3.2 Job-generálás és eseménymodell

- Eseménysablonok forrás × domain × időablak szerint, adatvezérelt konfigból:
  az `ops.source_policy.events` jsonb mező tárolja a forrásonkénti
  esemény-sablonokat (típus, offset, ablak); a worker ebből generál jobokat.
- A napi program-felfedezés kanonikus meccsekből generálja a
  T−24h/T−3h/T−60m/T−15m és T+5m/T+30m/T+2h/T+24h/T+72h frissítő jobokat.
- Heti hiánypótlás, napi forrás-canary, heti schema-drift riport: keret
  szinten megépül (job-generálás + riport-váz), a tényleges drift-detektálás
  az adapterekkel (S3) jön.
- A `ops.job` egyedi kulcsa (source, domain, entity, event_type, ablak)
  adja az idempotens job-generálást.

### 3.3 Shadow / canary / live mód

- Az `ops.source_policy.mode` mezője: `shadow` | `canary` | `live`.
- **Shadow:** csak megfigyelés és jelölt termelődik, kanonikus írás nincs
  (rendszer-spec §7.2). Az S2a szintetikus forrásai shadow/replay módban futnak.
- **Canary/live:** élesítés csak dokumentált visszaállítási tervvel,
  az S3 minősítési láncában (shadow → canary szűk körön → teljes élesítés).

## 4. Adatmodell (0004 migráció)

Minden új tábla az `ops` sémában; a `core`/`fact`/`stage`/`raw` sémák
nem változnak (séma-rések: S2d). Egyetlen tranzakciós migráció:
`db/migrations/0004_worker_framework.sql`; sorrend: tesztDB → zöld
ellenőrzés → devDB; visszaállítási út dokumentált.

| Tábla | Kulcs / fontos mezők | Felelősség |
|---|---|---|
| `ops.source_policy` | `source_slug` PK; `transport`, `mode`, `daily_budget`, `min_delay_ms`, `jitter_ms`, breaker-küszöb/cooldown, `adapter_version`, `parser_version`, domainek (jsonb), `events` (jsonb esemény-sablonok), `enabled` | katalógus-policy adatvezérelten; S2a-ban csak szintetikus fixture-források engedélyezettek |
| `ops.fetch_ledger` | id; `source_slug`, `request_key`, `attempted_at`, `outcome`, `http_status`, `bytes`, `latency_ms`, `artifact_hash`, `job_id` | minden kísérlet naplója (reserve→run→log); napi kvóta ebből számolódik (UTC nap) |
| `ops.breaker_state` | `source_slug` PK; `state` (closed/open/half_open), `failure_count`, `opened_at`, `next_probe_at` | perzisztens breaker, újraindítás után is él |
| `ops.job` | id; `source_slug`, `domain`, `entity_key`, `event_type`, `time_window`, `due_at`, `priority`, `status`, `leased_until`, `worker_id`, `attempts`, `last_error`; egyedi: (source, domain, entity, event_type, time_window) | ütemezési sor + lease; idempotens job-generálás |
| `ops.coverage_slot` | kulcs: (source, entity_type, data_kind, time_window); `state`, `updated_at`, `last_job_id` | lefedettség; nulla találat ≠ done |
| `ops.robots_cache` | `host` PK; `fetched_at`, `verdict`, `rules_text`, `expires_at` | napi robots-cache, fail-closed; robots-bizonyíték nyers artifactként is megőrizhető |

- `ops.coverage_slot.state` értékei (rendszer-spec §7.5): `pending`,
  `fetching`, `done`, `no_data`, `blocked`, `failed`, `stale`,
  `quarantined`, `skipped`.
- `ops.fetch_ledger.outcome` értékei: `success`, `http_error`, `timeout`,
  `oversize`, `robots_blocked`, `quota_blocked`, `breaker_open`,
  `transport_error`, `parse_error` (a parse a ledgerben a fetch kísérlet
  eredményéhez kapcsolódik).
- Meglévő táblák változatlanul: `raw.artifact`/`raw.capture`,
  `stage.observation`, `ops.publish_gate`. Új SQL csak az új táblákhoz,
  a meglévő repository-kon keresztül.
- **Seed policy-értékek a katalógusból** (S3-ban élesednek, most seed-adat):
  footballdata 10/nap, delay ≥2 s; tipplap 10/nap; fotmob 10/nap;
  openfoot 5000/hó (havi számláló a ledgerből). Az éles adapter-budgeteket
  az S3 minősítés külön rögzíti — a spike-keret nem automatikus éles keret.

## 5. Hibakezelés és őrök

### 5.1 Típusos hibák

Közös error-enum a worker/ingest határon: `Transport`, `RobotsBlocked`,
`QuotaExhausted`, `BreakerOpen`, `Oversize`, `Timeout`, `DecoyDetected`,
`DateGuardMismatch`, `SchemaDrift`, `Parse`. Kérésfeldolgozásban nincs
`unwrap()`/`expect()`/elnémítás; minden hiba naplózott forrás/job
kontextussal; a CLI/API üzenetek magyarul, cselekvően értelmezhetők.

### 5.2 Őr-mátrix

| Őr | Hely | Viselkedés |
|---|---|---|
| robots | worker, hívás előtt | napi cache; parse-hiba/403 → fail-closed stop; 404 = „nincs robots” jegyzet (S1 ruling); tiltott útvonal → `RobotsBlocked`, nincs retry |
| kvóta + delay + breaker | worker, hívás előtt | ledger-számlálás; 401/403 → azonnali forrás-stop + policy-review jelzés; 429 → Retry-After |
| L0 átvétel | fetcher | 5 MB plafon (forrásonként felülírható), 30 s timeout, content-type, hash-dedup → karantén nyers-megőrzéssel |
| decoy-200 tartalmi őr | parser | üres/strukturálatlan payload → `DecoyDetected` (HTTP 200 önmagában nem bizonyíték) |
| dátum-guard | parser | a kért nap visszhangja a payloadban; eltérés → `DateGuardMismatch`, nincs kanonikus írás (Tipplap v1-mismatch tanulság) |
| 90 perces konvenció | parser | AET/pens állás skip; ET/pens csak explicit forrásjelöléssel |
| séma-drift | parser | ismeretlen mező aránya / hiányzó kötelező mező → `SchemaDrift`, karantén + drift-jelzés a heti riporthoz |

### 5.3 Karantén és újraindítás

- **Karantén-út:** nyers megmarad (artifact + capture), coverage-slot
  `quarantined`, job `failed` + `last_error`, kanonikus írás nincs.
  Karanténból repars indítható új parser-verzióval; vak újrakérés nem.
- **Újraindítás-biztonság:** lease lejárat → visszasorolás; breaker/ledger/
  policy DB-ben él; a tick idempotens (request-key + content-hash dedup).
- **Hibautak:** 401/403 → azonnali leállítás + policy-review jelzés
  (breaker nyit, forrás áll); 429 → Retry-After tisztelete; transport-hiba →
  exponenciális backoff jitterrel, a breaker küszöb felett nyit.

## 6. Tesztelés és ellenőrzés

1. **Contract-tesztek forrásonként** (szintetikus golden fixture-ök a
   `tests/fixtures/` alatt, a katalógus mezőtérképei alapján — nyers payload
   nem kerül a repóba): happy path parse → observation mezők; decoy-200 →
   `DecoyDetected`; dátum-guard eltérés; AET/pens skip; séma-drift;
   hiányzó opcionális → NULL (soha nem nulla).
2. **Fetcher-tesztek:** FixtureFetcher request-key hozzárendelés;
   RustHttpFetcher lokális mock szerveren (wiremock) — méretplafon, timeout,
   státusz-osztályozás, hash-dedup. Élő hálózati kérés az S2a-ban nincs.
3. **Worker integrációs tesztek** a tesztDB-n (egy szál, projekt-konvenció):
   kvóta-kimerülés blokkol; breaker closed→open→half_open átmenetek
   újraindítás után is élnek; lejárt lease visszasorolása duplikáció nélkül;
   robots fail-closed; reserve→run→log a hibákat is számolja; coverage-slot
   állapotok (nulla találat ≠ done); prioritási sorrend; eseménymodell
   due_at-generálás (T−24h…T+72h, heti).
4. **E2E replay:** szintetikus forrás → worker tick → raw → observation →
   canonicalize → gate → `/v1/day` verified elem; a meglévő `ingest-fixture`
   út változatlanul zöld (backward kompatibilitás).
5. **Shadow-teszt:** shadow módban nincs kanonikus írás.

**Kötelező ellenőrzések a szelet végén:** `./scripts/check.sh` (reset + fmt +
clippy `-D warnings` + szekvenciális workspace-tesztek), `db-reset` +
`db-check` (tesztDB 0 üzleti sor), magyar szövegek visszaolvasása,
`footballctl doctor` + `worker status` bizonyíték a checkpointban.

## 7. Sikerkritériumok (Definition of Done)

- Minden fenti teszt futott és zöld; a futtatott parancsok + kimenetek
  a napi checkpointban.
- Élő hálózati kérés 0; nyers payload a repóban 0.
- A meglévő demó pipeline változatlan (a 20 meccs továbbra is verified).
- Üzemeltetési útmutató: `docs/operations/worker.md` (be/kikapcsolás,
  status, karantén-repars).
- Spec + terv + checkpoint magyarul, visszaolvasva (ékezetek, idegen glyph).
- 0004 migráció: tesztDB → zöld → devDB, visszaállítási út dokumentálva.

## 8. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| A keretrendszer éles adat nélkül készül, az S3 adaptereknél derül ki illesztési hiba | a contract-tesztek a katalógusban MÉRT mezőtérképeket és őröket kódolják szintetikus fixture-ökön; az S3 első adaptere (footballdata) a legismertebb forrás |
| A teljes eseménymodell túltervezettnek bizonyul | az események adatvezérelt konfigból jönnek, kikapcsolhatók; a mag (due_at + lease) egyszerű marad |
| Új függőségek (reqwest, robotstxt, wiremock) | indoklás + rögzített verzió a tervben; rustls (openssl nélkül); dev-függőség elkülönítve |
| A worker loop a footballd-ben erőforrást/komplexitást visz | `.env` kapcsoló, graceful leállítás, lease-elengedés; a tick idempotens |
| Kvóta/ledger elszámlálódik (restart, duplikált tick) | reserve→run→log minden kísérletre; request-key + content-hash dedup; integrációs tesztek restart-szimulációval |

## 9. Visszaállítási út

- Kód: feature branch (`feat/...`), commit csak felhasználói kérésre;
  elvetés `git restore` csak kifejezett engedéllyel.
- DB: a 0004 migráció tranzakciós; a visszaállítás dokumentált
  (drop táblák fordított sorrendben, tesztDB-n gyakorolva); dev DB demóadata
  érintetlen (a migráció csak ops táblákat ad hozzá).
- Worker: `.env` kikapcsoló azonnal leállítja; a meglévő fixture pipeline
  a worker nélkül is működik.

## 10. Következő lépés

A spec felhasználói review-ja után a writing-plans skill következik
(megvalósítási terv: `docs/superpowers/plans/2026-11-12-s2a-ingest-framework.md`).
Implementáció csak a terv jóváhagyása után, szeletenkénti stop-pontokkal.
