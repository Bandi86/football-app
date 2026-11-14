# S3a — Belépési kapuk zárása (éles forrás előtt)

**Dátum:** 2026-11-14
**Állapot:** brainstorming-jóváhagyott terv; felhasználói spec-review függőben
**Kapcsolódó dokumentumok:** [2026-11-12 checkpoint S3-kapuk](../../checkpoints/2026-11-12/checkpoint.md#s3-belépési-kapuk-éles-forrás-előtt-kötelező-zárni),
[2026-11-13 checkpoint](../../checkpoints/2026-11-13/checkpoint.md) (11+2 tétel),
[S2a spec](../specs/2026-11-12-s2a-ingest-framework-design.md), [S2b-1 spec](../specs/2026-11-13-s2b1-identity-layer-design.md)
**Branch:** `feat/s3a-entry-gates` (a main 6741319-ről)

## 1. Hatókör

Az S3 első alszelete: **a 13 belépési kapuból 11 zárása** kód-szinten, élő
forrás bevezetése előtt. Az S3b (footballdata-adapter) csak ezek után indul.
**Nincs új migráció** — a döntések meglévő sémán/ledgeren nyugszanak.

### 1.1 Kapuk és döntések (jóváhagyva 2026-11-14)

| # | Kapu | Döntés |
|---|---|---|
| 1 | 429 → Retry-After | `FetchError::RateLimited(Option<Duration>)`; fejléc (sec/HTTP-date) parse; job pending `due_at = now + retry_after`; ledger `http_error`+429 |
| 2 | jitter + backoff | átmeneti hiba → pending `due_at = now + min(60s·2^n, 1h) + jitter`; n = job fetch-kísérletei a ledgerből; új `requeue_with_delay`; végleges hibák nem requeue |
| 3 | Half-open beragadás | `breaker::on_success` a **sikeres fetch után azonnal** (fetch-szintű egészség); a kései hívás törlődik |
| 4 | Robots-lazítások | Sitemap-only → engedett; BOM-strip; cache-kulcs **origin** (`scheme://host:port`, a meglévő `host` oszlopban); TTL: átmeneti hiba 1h, 403/no_robots 24h |
| 5 | Scheduler-validáció | ismeretlen `event_type` → skip + warn; `interval_minutes` csak 1440/10080, más → skip + warn |
| 6 | media_type | `persist_raw` `media_type` paraméter a `RawPayload.content_type`-ból; artifact-insert bindeli |
| 7 | Robots-ledger | minden robots-fetch kísérlet nem-számláló ledger-sor (`{slug}:robots:{origin}`); a kvóta-számláló kizárja a `:robots:` kulcsokat |
| 8 | CLI repo-függvények | `worker status` + `doctor` nyers SQL → storage repo-fn-ök |
| 9 | Live identitás | `footballctl IngestFixture` `identity_exact=true` hardcode törlése; tick és CLI **közös fail-closed linker-feloldás** |
| 12 | Cross-kind approve | az approve ellenőrzi a választott entitás kind-ját a queue-sor `entity_kind`-jével szemben |
| 13 | Crosswalk-uniqueness | az import validálja a kötések egyértelműségét (nincs ütköző/ellentmondó) |

**Halasztva (dokumentáltan):** #10 OpenFoot havi számláló (OpenFoot-szelet);
#11 többworker fencing + half-open atomitás (többworker bevezetésekor).

### 1.2 Nem célok (YAGNI)

- S3b (footballdata-adapter, élő forrás) — külön szelet.
- Bookmaker-szintű trust és multi-market display.
- Új tábla/migráció; a robots-cache régi sorai TTL-lel kifutnak.

## 2. Fetch-reziliencia

### 2.1 RateLimited (#1)

- `football_domain::error::FetchError` új variáns:
  `RateLimited(Option<Duration>)` (Display: „a forrás kérte: várakozás …”).
- `RustHttpFetcher`: 429 → `RateLimited(retry_after)`, ahol a `Retry-After`
  fejlécet másodpercként vagy HTTP-date-ként parse-oljuk; hiányzó/érvénytelen
  → `None`. A többi nem-2xx marad `HttpStatus(u16)`.
- `tick::fetch_failure`: `RateLimited` → ledger `http_error` + 429-es sor
  (a kísérlet számít), `breaker::on_failure`, a job **pending** + delayed
  requeue (`retry_after` vagy backoff, ha None).

### 2.2 Backoff + jitter (#2)

- Átmeneti hiba (Transport/Timeout/HttpError 5xx) → pending requeue:
  `delay = min(60s · 2^n, 3600s) + random(0..jitter_ms)`, ahol `n` a job
  fetch-kísérleteinek száma a `fetch_ledger`-ben (nincs új oszlop).
- Végleges hibák (`Oversize`, `NotFound`, `HttpStatus(4xx)` nem-429,
  `ParseError`-t okozó fetch nincs) → nincs requeue (failed).
- Új storage fn: `requeue_with_delay(pool, job_id, due_at)` (status pending
  + due_at; a leased flag/worker felszabadítása a meglévő mintát követi).

### 2.3 Half-open (#3)

- A tickben a `breaker::on_success` a fetch sikeressége után, a pipeline
  előtt fut; a jelenlegi, a teljes feldolgozás végén lévő hívás törlődik.
- Ezzel a half-open probe bármely kimenet után lezárul (success → closed;
  a parse-karantén/infra-hiba nem hagyja örökre half-openben).

## 3. Robots + ledger (#4, #7)

- **Direktíva-kapu:** `has_user_agent_directive` helyett: ha a fájlban nincs
  `User-agent` szakasz, de van `Sitemap` (vagy más legitim) direktíva →
  `NoRobots` (engedett); üres fájl → `NoRobots`. **BOM-strip** a parse előtt.
- **Cache-kulcs:** a robots cache kulcsa az **origin** (`scheme://host:port`),
  a meglévő `host` oszlopban (dokumentált szemantika-váltás; a régi
  host-kulcsú sorok a TTL-lel kifutnak).
- **TTL:** `blocked` 403 esetén 24h; átmeneti hiba (Transport/5xx) esetén 1h
  (rövid, hogy a helyreállás ne maradjon blokkolva egy napig); `no_robots` 24h.
- **Ledger:** a robots-fetch minden kísérlete nem-számláló ledger-sort ír
  (`request_key = {slug}:robots:{origin}`, outcome success/http_error/
  transport_error). A `count_fetch_attempts_today` kizárja a `:robots:`
  kulcsokat.

## 4. Scheduler, evidencia, CLI, identitás (#5, #6, #8, #9, #12, #13)

- **Scheduler:** ismeretlen `event_type` → a job kihagyása + `warn!`;
  `interval_minutes ∉ {1440, 10080}` → kihagyás + `warn!`; a két engedett
  érték változatlanul működik.
- **media_type:** `repo::persist_raw` szignatúrája `media_type: &str`
  paraméterrel bővül; a hívó a `raw.content_type`-ot adja; az
  `raw.artifact` insert ezt bindeli.
- **CLI:** `worker_status`/`doctor` SQL-je storage repo-fn-ökbe kerül
  (`repo::worker_status_rows`, `repo::doctor_counts`), a CLI hívja.
- **Identity:** közös `resolve_team_identity`-szerű feloldás a tick és a
  `footballctl IngestFixture` számára (a CLI hardcode törlése); a demo
  fixture_seed refek egzaktak maradnak, a nem feloldott esetek review-sort
  nyitnak és fail-closed módon withheld állapotot adnak.
- **Cross-kind approve:** az approve a queue-sor `entity_kind`-ját veti
  össze a választott `core.entity.kind`-dal; eltérés → hiba (nem ír).
- **Crosswalk-uniqueness:** az import ellenőrzi, hogy egy forrás-entitás
  kötése egyértelmű (nincs két ellentmondó cél ugyanarra a forrásra, illetve
  egy célra két ellentmondó forrás-kötés a tiltott esetekben) → hiba.

## 5. Tesztelés

- **Fetch:** wiremock 429 `Retry-After` másodperccel és HTTP-date-tel → a job
  pending `due_at` a várt időben; backoff-unit (jitter=0 egzakt, random
  határok); half-open e2e (sikeres fetch + parse-karantén → closed);
  végleges hibák failed, nincs requeue.
- **Robots:** Sitemap-only engedett; BOM tolerált; origin-kulcs (két port →
  két cache-sor); TTL-ágak (1h vs 24h); ledger-sor + kvóta-kizárás.
- **Scheduler:** ismeretlen event_type / rossz interval → kihagyás + warn;
  1440/10080 elfogadva.
- **media_type:** az artifact `media_type` a payload content-type-ja
  (wiremock `text/html`).
- **CLI:** repo-fn-ök smoke a teszt DB-n.
- **Identity:** cross-kind approve hiba; crosswalk-uniqueness hiba; közös
  linkeres e2e (demo exact + nem feloldott → review).
- Kapuk: `./scripts/check.sh` (165+), clippy/fmt, **0 élő kérés**, demo
  20/20 változatlan.

## 6. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| `FetchError::RateLimited` variáns érinti a matcheket | minden `HttpStatus(429)` elaboráció frissül; a tesztek a teljes mátrixot fedik |
| Robots cache-szemantika vált (host → origin) | dokumentált; a régi sorok TTL-lel kifutnak; új tesztek az origin-kulcsra |
| Backoff késleltethet jobokat | 1h sapka + breaker-cooldown; a végleges hibák azonnal failed |
| CLI/tick linkeres feloldás viselkedés-váltás | e2e a demo exact útra és a review-útra is |

## 7. Visszaállítási út

- Kód: branch-revert; **nincs migráció** (séma érintetlen).
- DB: a robots-cache új sorai TTL-lel kifutnak; a ledger-sorok megmaradnak
  (audit-adatok).

## 8. Következő lépés

1. Felhasználói spec-review (ez a dokumentum).
2. writing-plans → `docs/superpowers/plans/2026-11-14-s3a-entry-gates.md`.
3. Implementáció inline execution-nel, zöld task után **committal**.
4. Utána **S3b — footballdata-adapter** (élő forrás, külön ciklus).
