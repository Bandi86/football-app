# clubelo forrás — üzemeltetési útmutató (S4a)

Napi Elo-pillanatkép a clubelo API-ból
(`http://api.clubelo.com/{YYYY-MM-DD}`, CSV: Rank,Club,Country,Level,Elo,From,To).

## Policy-seed (dev DB)

```sql
INSERT INTO ops.source (id, code, display_name)
VALUES (gen_random_uuid(), 'clubelo', 'clubelo')
ON CONFLICT (code) DO NOTHING;

INSERT INTO ops.source_policy
  (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
   breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
   domains, events, enabled, max_bytes)
VALUES ('clubelo', 'rust_http', 'canary', 5, 2000, 1000, 3, 60, '1.0', '1.0',
   '["elo"]'::jsonb,
   '[{"event_type":"discovery","offset_minutes":null,"interval_minutes":1440}]'::jsonb,
   true, 5242880)
ON CONFLICT (source_slug) DO UPDATE SET enabled = EXCLUDED.enabled;
```

## Szabályok

- **Provenance kötelező:** a 2025-06-01 előtti dátum csak `provisional`
  (a DB CHECK kényszeríti); az adapter a kérés dátumából állítja be.
- As-of: soronként a rating dátuma; a napi feed upserttel frissül.
- A klub-entitás kapcsolás (crosswalk) külön lépés — `club_entity_id` NULL
  marad, amíg a linker/import ki nem tölti.
- Robots: a worker robots-kapuja fut a clubelo.com ellen is (fail-closed).

## Visszaállítás

- Policy/source sorok törlése; a `fact.club_elo` sorok forrás-kötöttek.
