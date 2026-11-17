# Footballdata forrás — üzemeltetési útmutató (S3b-2)

A `footballdata` (football-data.co.uk) az első éles forrás: 5 liga
(E0, SP1, I1, D1, F1), 2026/27 szezon, statikus CSV-k
(`mmz4281/2627/{LIGA}.csv`), napi 1 kérés/liga. A napi munka a canary
módban fut; az élő kör külön, explicit engedéllyel indul.

## 1. Policy- és trust-seed (dev DB)

```sql
-- Forrás + policy: canary, 10/nap, 2 s + 1 s jitter, breaker 3/60 s.
INSERT INTO ops.source (id, code, display_name)
VALUES (gen_random_uuid(), 'footballdata', 'Football-Data')
ON CONFLICT (code) DO NOTHING;

INSERT INTO ops.source_policy
  (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
   breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
   domains, events, enabled, max_bytes)
VALUES
  ('footballdata', 'rust_http', 'canary', 10, 2000, 1000, 3, 60, '1.0', '1.0',
   '["day"]'::jsonb,  -- S9: egy napi job, ligánként 1 kérés (5 CSV)
   ARRAY['{"event_type":"discovery","offset_minutes":null,"interval_minutes":1440}'::jsonb],
   true, 10485760)
ON CONFLICT (source_slug) DO UPDATE SET
  transport = EXCLUDED.transport, mode = EXCLUDED.mode,
  daily_budget = EXCLUDED.daily_budget, enabled = EXCLUDED.enabled;

INSERT INTO ops.source_trust (source_id, domain, trust_class, provider)
SELECT id, d, 'medium', NULL FROM ops.source, (VALUES ('odds'),('result')) AS t(d)
WHERE code = 'footballdata'
ON CONFLICT (source_id, domain) DO NOTHING;
```

## 2. Kétfázisú bootstrap (első élő kör)

Az élő forrás csapatai még nincsenek kanonikus entitásként — a fail-closed
linker az első körben review-sorokat nyit, a meccsek kimaradnak:

1. **1. kör:** `footballctl worker tick` (vagy a daemon egy tickje) →
   `identity review` listázza a sorokat; minden csapatot
   `identity approve-create <id>`-dal (új entitás) vagy
   `identity approve <id> --entity <uuid>`-tal (meglévőhöz) kell lezárni.
2. **2. kör:** a következő tick a feloldott csapatokkal kanonikus meccseket
   ír; a napi lista a gate szerint `verified_publish`/`provisional_publish`
   állapotot kap.

Új idény/csapat esetén ugyanez a minta ismétlődik (review → jóváhagyás).

## 3. Napi működés

- A policy `discovery` eventje naponta **egy jobot** generál (`day`); a
  job 5 CSV-kérést tartalmaz (ligánként egy), a worker a kérések között
  kivárja a 2 s delay-t — 5 CSV-kérés/nap, egy coverage-slot.
- Robots: a worker robots-kapuja a valódi robots.txt-t ellenőrzi
  (a robots-kérés nem számít a büdzsébe, S3a).
- Hiba: 429 → Retry-After szerinti requeue; átmeneti hiba → exponenciális
  backoff; 4xx → failed; a breaker 3 hiba után nyílik (60 s cooldown).

## 4. Canary → live

A felhasználó döntése: a policy `mode` mezőjét `'live'`-ra állítani, ha a
canary körök zöldek. Visszaállítás: `mode='canary'` vagy `enabled=false`.

## 5. Visszaállítási út

- Policy/trust sorok törlése (a kanonikus + stage adatok forrás-kötöttek).
- Kód: branch-revert; séma érintetlen.
- Teszt DB: `./scripts/db-reset.sh`.

## 6. Ligalefedettség (S22, 2026-11-17)

A demo kör 5 ligája mellett a fő divíziók is be vannak kötve
(22 kérés/nap, policy `daily_budget=40`, `min_delay 2000 ms`):

- Anglia: Premier League (E0), Championship (E1), League One (E2),
  League Two (E3), National League (EC)
- Skócia: Premiership (SC0), Championship (SC1), League One (SC2),
  League Two (SC3)
- Másodosztályok: 2. Bundesliga (D2), Serie B (I2), LaLiga 2 (SP2),
  Ligue 2 (F2)
- További első osztályok: Eredivisie (N1), Belgian Pro League (B1),
  Primeira Liga (P1), Süper Lig (T1), Super League Greece (G1)

A CSV-k a `mmz4281/2627/{kód}.csv` útvonalon érhetők el (www→apex
redirect, a fetcher követi). A parser a job dátuma ±7 napjára szűr, ezért
egy futás az adott hét meccseit materializálja; válogatott szünetben a
ligák CSV-jei a szünet előtti fordulóig frissülnek (2026-11-17: az új
ligák ablakában nem volt sor → nincs új meccs, ez helyes).
A kiemelt kör a priority ≤10, ezért az új divíziók alapból a „Minden
verseny" nézetben látszanak.
