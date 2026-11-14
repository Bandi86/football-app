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
   ARRAY['e0','sp1','i1','d1','f1'],
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

- A policy `discovery` eventje naponta generál egy jobot ligánként
  (`domains`: e0…f1) → 5 CSV-kérés/nap, 2 s delay + jitter.
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
