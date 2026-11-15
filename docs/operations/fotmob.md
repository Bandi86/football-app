# FotMob forrás — üzemeltetési útmutató (S4b)

Napi meccs-feed a FotMob nyílt JSON API-jából:
`https://www.fotmob.com/api/data/matches?date=YYYYMMDD&timezone=UTC&locale=en`.

- **Nincs böngésző:** a nap-feed sima JSON → `rust_http` transport (az Obscura
  csak a JS-renderelt oldalakhoz kell, pl. Soccerway).
- **Robots — ÉLESBEN TILTOTT:** a fotmob.com robots.txt `Disallow: /api/*`
  szabályt tartalmaz (mellette ellentmondásos `Allow: /api/*` sorok); a worker
  robots-kapuja fail-closed módon **blokkolja** az API-utat. Az adapter kódja
  kész és tesztelt, de a forrás **csak explicit engedéllyel/alternatív
  hozzáféréssel** használható élesben; a policy alapból `enabled=false`.
- **Liga-szűrő:** policy-domainenként egy top-5 liga
  (epl/laliga/seriea/bundesliga/ligue1 → liganév+országkód); a feed a
  megadott napot adja, dátum-guard véd.
- **Idő:** `status.utcTime` közvetlenül UTC — nincs időzóna-konverzió.
- **Identitás:** stabil FotMob csapat-id-k (pl. `8492`), a crosswalk/review
  a meglévő fail-closed úton megy.

## Policy-seed (dev DB)

```sql
INSERT INTO ops.source (id, code, display_name)
VALUES (gen_random_uuid(), 'fotmob', 'FotMob')
ON CONFLICT (code) DO NOTHING;

INSERT INTO ops.source_policy
  (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
   breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
   domains, events, enabled, max_bytes)
VALUES ('fotmob', 'rust_http', 'canary', 10, 2000, 1000, 3, 60, '1.0', '1.0',
   '["epl","laliga","seriea","bundesliga","ligue1"]'::jsonb,
   '[{"event_type":"discovery","offset_minutes":null,"interval_minutes":1440}]'::jsonb,
   false, 5242880)
ON CONFLICT (source_slug) DO UPDATE SET enabled = EXCLUDED.enabled;
```

## Visszaállítás

Policy/source sorok törlése; a stage/kanonikus adatok forrás-kötöttek.
