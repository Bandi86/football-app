# Worker üzemeltetési útmutató (S2a ingest keretrendszer)

**Dátum:** 2026-11-12
**Hatókör:** a `footballd` worker loopja, a `footballctl worker` parancsok,
a fixture-alapú gyűjtés és a 0004 worker táblák kézi üzemeltetése.
**Kapcsolódó dokumentumok:**
[S2a spec](../superpowers/specs/2026-11-12-s2a-ingest-framework-design.md),
[S2a terv](../superpowers/plans/2026-11-12-s2a-ingest-framework.md),
`db/migrations/0004_worker_framework.sql`.

Minden parancs a repo gyökeréből fut, a környezet betöltése után:

```bash
set -a; . ./.env; set +a
```

## 1. Be- és kikapcsolás

- `WORKER_ENABLED=true` esetén a `footballd` induláskor külön tokio taskban
  elindítja a worker loopot; hiányzó vagy egyéb érték esetén a worker nem indul
  (az API ettől még kiszolgál). Az `enabled` policy-kapcsoló ettől független:
  a loop csak az `ops.source_policy.enabled=true` forrásokból generál jobot.
- `WORKER_TICK_SECS`: a tick-intervallum másodpercben. Alapérték **60**;
  `0` esetén minimum **1** másodpercre clampelődik; érvénytelen érték esetén
  **60** másodpercre esik vissza — mindkét eset `tracing::warn` naplóval.
- Az első tick azonnal lefut, a továbbiak az intervallum szerint
  (`MissedTickBehavior::Delay`, lassú tick nem okoz burstöt). Tick-hiba nem
  állítja le a daemont: naplózódik, a ciklus folytatódik.
- A `footballd` induláskor automatikusan futtatja a migrációkat
  (`sqlx::migrate!`) — a dev DB jelenleg a 0004-nél tart (29 tábla).
- Leállítás: Ctrl-C (SIGINT) → graceful shutdown: a serve leáll, a worker
  shutdown-jelet kap, a futó kör befejeződik, majd a saját lease-ek `pending`
  állapotba kerülnek (`worker leállt, a lease-ek elengedve` napló, released
  számmal). A bind/serve hibaágán is lefut ugyanez az elengedés.
- Azonnali leállítás: `WORKER_ENABLED=false` + újraindítás (vagy Ctrl-C).
  Beragadt lease-ek esetén a lejárt lease-ek automatikus visszasorolása
  (`requeue_expired_leases`) a következő tickben gyógyít.

## 2. Fixture könyvtár

- Útvonal: `DATA_DIR/worker-fixtures` (a `DATA_DIR` alapértéke `./data`).
- A request-key a **fájlnév törzse** (kiterjesztés nélkül), például
  `demo-synthetic:match:2026-11-12.json` → kulcs
  `demo-synthetic:match:2026-11-12`. A tick a jobból képzett request-key-jével
  keresi a fájlt.
- `FixtureFetcher::from_dir` hiányzó könyvtárnál **üres mapet** ad (nem hibát):
  a daemon `WORKER_ENABLED=true` mellett is elindul fixture-könyvtár nélkül;
  a hiányzó kulcsú fetch `NotFound` hibát ad, ami a ledgerben látható.
- Ismert egyszerűsítések (kontrollált könyvtárnál ártalmatlan): azonos törzsű
  fájloknál az utolsó olvasott nyer (last-wins); a nem UTF-8 fájlnév-törzs
  csendben kimarad.
- Adapter-registry jelenleg egyetlen adaptert tartalmaz: `DemoAdapter`
  (`demo-synthetic` slug). Élő forrásadapter nincs — az S3 fázis része.

## 3. footballctl parancsok

### `footballctl worker status`

Forráspolicy-nként kiírja a slugot, módot, engedélyezettséget, a mai
fetch-kísérletek számát a napi kerethez (UTC nap, csak a számláló outcome-ok:
success/http_error/timeout/oversize/transport_error/parse_error) és a breaker
állapotát (nincs breaker-sor → `closed`). Összesítve: függő (pending) és
leased jobok, karanténos coverage-slotok. Példa kimenet:

```text
forráspolicy-k:
  demo-synthetic mód=shadow engedélyezve=igen ma=1/10 breaker=closed
függő jobok: 0 (leased: 0)
karanténban: 0
```

### `footballctl worker tick`

Egyszeri kézi worker kör: `list_enabled_policies` → `generate_jobs` → `tick`
a `DATA_DIR/worker-fixtures` fixture-eivel és a DemoAdapter registry-vel.
Kimenet: `generált=N feldolgozott=N blokkolt=N sikertelen=N`. Shadow policy
esetén csak `stage.observation` sorok íródnak, kanonikus adat nem.

### `footballctl doctor` (új mezők)

```text
db=1 gates=N jobs_pending=N breakers_open=N quarantined=N
```

- `jobs_pending`: `ops.job` sorok `status='pending'` állapotban;
- `breakers_open`: `ops.breaker_state` sorok `state='open'` állapotban;
- `quarantined`: `ops.coverage_slot` sorok `state='quarantined'` állapotban.

## 4. Karantén-repars menete (új parser-verzió)

Parse-hiba esetén a coverage-slot `quarantined`, a ledger `parse_error`,
a job `failed` állapotba kerül. A karanténból való kihozatal lépései:

1. **Adapter-javítás / új parser-verzió:** a javított adapterrel frissül a
   policy parser-verziója (kézi SQL, nem destruktív):

   ```sql
   UPDATE ops.source_policy SET parser_version='1.1'
   WHERE source_slug='<slug>';
   ```

2. **Érintett jobok újra pending** (kézi SQL UPDATE, nem destruktív — törlés
   nem történik, a ledger append-only megmarad):

   ```sql
   UPDATE ops.job
   SET status='pending', worker_id=NULL, leased_until=NULL, last_error=NULL
   WHERE source_slug='<slug>' AND status='failed';
   ```

   Szűkíthető `domain`/`entity_key`/`time_window` feltételekkel, ha csak
   egyes ablakok érintettek. Az `attempts` számláló szándékosan nem nullázódik
   (előzmény). A coverage-slot állapota a sikeres újrafutáskor upsert-tel
   íródik felül (`done`/`no_data`), külön UPDATE nem kell hozzá.

3. **Újrafuttatás:** `footballctl worker tick` (vagy a futó daemon következő
   tickje felveszi a pending jobot).
4. **Ellenőrzés:** `footballctl worker status` → `karanténban: 0`;
   `ops.fetch_ledger` új sora `success`; `ops.coverage_slot` állapota `done`.

## 5. Breaker policy-review teendő

Auth-hiba (401/403, login-redirect) esetén a breaker azonnal `open` állapotba
kerül (küszöbtől függetlenül), és egy `policy_review` audit-esemény íródik —
a breaker-nyitás és az audit egyetlen tranzakcióban. A teendő:

1. **Review-sorok listája:**

   ```sql
   SELECT created_at, detail FROM ops.audit_event
   WHERE action='policy_review' ORDER BY created_at DESC;
   ```

   A `detail` JSON tartalmazza a `slug`-ot és a `status`-t (`auth_failure`);
   a `subject_id` jelenleg NULL, a forrás a detailben azonosítható.
2. **Forrás állása:** amíg a breaker `open`, a forrás fetch-je blokkolt
   (nem számláló ledger-sor, a job azonnal visszasorolódik pending-re).
3. **Cooldown után probe:** a `breaker_cooldown_secs` leteltével
   (`next_probe_at <= now`) a breaker `half_open` állapotba lép, egyetlen
   probe engedélyezett; siker → `closed` (számláló nullázódik), hiba →
   azonnal újra `open`. Ha a cooldown nem ábrázolható időtartam, a
   `next_probe_at` NULL marad: fail-closed, kézi reset kell.
4. **Kézi beavatkozás (review után):** a hitelesítő adat / policy javítása
   után a breaker kézzel zárható:

   ```sql
   UPDATE ops.breaker_state
   SET state='closed', failure_count=0, opened_at=NULL, next_probe_at=NULL,
       updated_at=now()
   WHERE source_slug='<slug>';
   ```

   Ez nem töröl adatot; az audit-előzmény megmarad.

## 6. Seed SQL a katalógus-forrásokhoz

A forráskatalógus négy elsődleges forrása **enabled=false, mode=shadow**
állapotban kerül a policy-táblába — az élesítés (bármelyik forrás
engedélyezése, canary/live mód) az **S3** fázis döntése, nem része ennek a
szeletnek. A beszúrás idempotens (`ON CONFLICT DO NOTHING`):

```sql
INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
  min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
  adapter_version, parser_version, domains, events, enabled, max_bytes)
VALUES
 ('footballdata','rust_http','shadow',10,2000,500,3,300,'0.0','0.0','["match","odds","standing"]','[]',false,5242880),
 ('tipplap','rust_http','shadow',10,2000,500,3,300,'0.0','0.0','["odds"]','[]',false,5242880),
 ('fotmob','rust_http','shadow',10,2000,500,3,300,'0.0','0.0','["match"]','[]',false,5242880),
 ('openfoot','rust_http','shadow',166,2000,500,3,300,'0.0','0.0','["match"]','[]',false,5242880)
ON CONFLICT (source_slug) DO NOTHING;
```

Megjegyzés: az `events='[]'` sablon nélkül a scheduler nem generál jobot;
az esemény-sablonok kitöltése az élesítési előkészítés része (S3).

## 7. Élesítési lánc emlékeztető

- **shadow → canary → live:** a mód mező és az ágak készen állnak; a canary
  élesítési folyamata és a forrásonkénti minősítés az **S3** fázisban
  történik (a jóváhagyott bizalmi mátrix + adoptációs sorrend szerint).
  Shadow módban csak observation íródik, kanonikus entitás nem.
- **90 perces konvenció (parser-őr):** az AET/pens állás nem írható 90 perces
  eredményként — a DemoAdapter fail-closed karanténba teszi (explicit
  ET/pens mezőkezelés S3). A skip-őr a parserben él, nem az ütemezőben.
- **Robots fail-closed:** a robots-kapu a fetch előtt fut; transport-hiba
  (DNS/timeout/refused) esetén `blocked` verdict (24 órás TTL-lel),
  fixture-módban a hiányzó robots-fájl `no_robots`. A matchernek átadott
  UA-token számjegymentes (`football-app`), a HTTP User-agent fejléc
  verziózott marad. Robots/policy megkerülése tilos (AGENTS §7).

## 8. Visszaállítási út

- **Azonnali leállítás:** `WORKER_ENABLED=false` + újraindítás — a worker
  nem indul, az API változatlanul kiszolgál. Graceful leállításkor a
  lease-ek elengedődnek; beragadt lease-t a lejárat utáni tick sorol vissza.
- **0004 visszagörgetése (csak kifejezett engedéllyel):** a worker táblák
  FK-sorrendben, CASCADE nélkül dobhatók:

  ```sql
  DROP TABLE ops.robots_cache;
  DROP TABLE ops.coverage_slot;
  DROP TABLE ops.job;
  DROP TABLE ops.breaker_state;
  DROP TABLE ops.fetch_ledger;
  DROP TABLE ops.source_policy;
  ```

  Ezzel a worker üzemeltetési előzmények (policy, ledger, jobok) elvesznek;
  a 0001–0003 táblái és a kanonikus adat érintetlen. A `_sqlx_migrations`
  0004-es sorának törlése és a kód visszaállítása külön, egyeztetett lépés.
- **Kód-visszaállítás:** az S2a munka a `feat/s2a-ingest-framework` branch
  munkafáján él (commit a felhasználó kérésére történik); a taskonkénti
  review-csomagok a `.superpowers/sdd/2026-11-12-s2a-ingest-framework/`
  alatt rögzítik a bázisállapotot.

## S3 belépési kapuk

Az éles forrás-adapterek (S3) előtt kötelező zárni a final review
kaputételeit (429/Retry-After, jitter/backoff, half-open breaker
átmenet, robots-lazítások, interval-validáció, media_type, robots-ledger,
CLI repo-k, fail-closed linker, havi számláló, többworker fencing).
A teljes lista: `docs/checkpoints/2026-11-12/checkpoint.md`
„S3 belépési kapuk” szekció.
