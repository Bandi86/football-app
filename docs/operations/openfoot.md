# OpenFoot API forrás — üzemeltetési útmutató (S13)

A `openfoot` Bearer-kulcsos JSON API (`https://openfootapi.com/v1`), plan
**starter**, kvóta **5000/hó** (~166/nap). Nap-feed (fixture + lezárt
eredmény) és verseny-registry; **oddsot nem ad**. Kulcs a macOS
Keychainben: account `football-app-v2`, service `openfoot-api-key`.

## 1. Kulcs

- Kézi futtatás: `export OPENFOOT_API_KEY="$(security find-generic-password -a football-app-v2 -s openfoot-api-key -w)"`.
- Az agent burkoló (`agent/run.sh`) ezt automatikusan elvégzi, ha az env
  nincs beállítva. A kulcs soha nem kerül fájlba, naplóba vagy a nyers
  capture-be (csak a kérés `Authorization` fejlécébe).

## 2. Policy-seed (dev DB)

```sql
INSERT INTO ops.source (id, code, display_name)
VALUES (gen_random_uuid(), 'openfoot', 'OpenFoot API')
ON CONFLICT (code) DO NOTHING;

INSERT INTO ops.source_policy
  (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
   breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
   domains, events, enabled, max_bytes)
VALUES
  ('openfoot', 'rust_http', 'canary', 8, 1000, 300, 3, 120, '1.0', '1.0',
   '["day"]'::jsonb,
   '[{"event_type":"discovery","offset_minutes":null,"interval_minutes":1440}]'::jsonb,
   true, 8388608)
ON CONFLICT (source_slug) DO UPDATE SET enabled = EXCLUDED.enabled;

INSERT INTO ops.source_trust (source_id, domain, trust_class, provider)
SELECT id, 'result', 'medium', 'OpenFoot' FROM ops.source
WHERE code = 'openfoot'
ON CONFLICT (source_id, domain) DO NOTHING;
```

## 3. Működés

- Napi job, **4 kérés**: `/v1/competitions?limit=200` (registry-cache) +
  `/v1/matches?date=…&limit=100&page=1..3` (max 300 sor/nap).
- A registry a parse-láncban cache-be kerül; a nap-feed csak
  `competitionId`-t ad, a nevet/országot innen oldjuk fel. Ismert
  top-5 + NB I/II kanonikus nevekkel (footballdata-egyezés).
- Dátum-guard: minden sor kickoff-dátuma a kért UTC-nap, különben
  `DateGuardMismatch` (fail-closed).
- Eredmény csak `finished` + két pontszám esetén (`decided_by=ft`);
  élő részeredmény nem kanonikus eredmény.
- Csapat-ország: hazai ligában a `team.country` (angol név → ISO),
  nemzetközi versenyben a csapatnév → ISO.
- Kvóta: napi 4 kérés × ~2 (retry) = 8; havi ~240 < 5000. A havi
  fogyást `/v1/account/quota` mutatja (1 kérés).

## 4. Kockázatok

- `meta.environment=beta`; a lefedettség `default_competitions`,
  `partial=true` (120 verseny), napfüggő.
- Kereszt-forrás meccs-duplikáció lehetséges: a natural key része a
  verseny és a forduló; az OpenFoot „UEFA Nations League” vs Soccerway
  „UEFA Nations League - League C” külön verseny → külön kanonikus sor.
  A teljes backfill/crosswalk projektben deduplikálandó.
- A plan-gátolt endpointok (lineups/xG/events) starter csomagban 403.

## 5. Visszaállítás

- `UPDATE ops.source_policy SET enabled=false WHERE source_slug='openfoot';`
  vagy a policy-sor törlése; a stage/kanonikus sorok forrás-kötöttek.
