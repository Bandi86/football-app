#!/usr/bin/env bash
# S38: a football-research helyi JSON API forrás regisztrálása az adatbázisban.
#
# Idempotens: a `research` forrást create-on-miss identitásmódban, `local_file`
# transzporttal, napi 1 discovery + 4 óránkénti refresh sávval veszi fel.
# Nincs hálózat, nincs robots, nincs kvóta (a tick ezt a transzportnál kikapcsolja).
#
# Használat: scripts/seed-research-source.sh [adatbázis-URL]
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

DB_URL="${1:-${DATABASE_URL:-}}"
if [[ -z "$DB_URL" ]]; then
  echo "HIBA: nincs adatbázis-URL (argumentum vagy DATABASE_URL)" >&2
  exit 1
fi

psql "$DB_URL" -v ON_ERROR_STOP=1 <<'SQL'
INSERT INTO ops.source (id, code, display_name, identity_mode)
VALUES (gen_random_uuid(), 'research', 'Football Research (helyi JSON API)', 'create_on_miss')
ON CONFLICT (code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    identity_mode = 'create_on_miss';

INSERT INTO ops.source_policy (
    source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
    breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
    domains, events, enabled, max_bytes
) VALUES (
    'research', 'local_file', 'live', 0, 0, 0,
    5, 60, '1.0.0', '1.0.0',
    '["day"]'::jsonb,
    '[{"event_type":"discovery","offset_minutes":null,"interval_minutes":1440},
      {"event_type":"refresh","offset_minutes":null,"interval_minutes":360}]'::jsonb,
    true, 8388608
) ON CONFLICT (source_slug) DO UPDATE SET
    transport = EXCLUDED.transport,
    mode = EXCLUDED.mode,
    adapter_version = EXCLUDED.adapter_version,
    parser_version = EXCLUDED.parser_version,
    domains = EXCLUDED.domains,
    events = EXCLUDED.events,
    enabled = EXCLUDED.enabled,
    max_bytes = EXCLUDED.max_bytes,
    updated_at = now();

SELECT s.code, s.identity_mode, p.transport, p.mode, p.enabled, p.daily_budget
FROM ops.source s JOIN ops.source_policy p ON p.source_slug = s.code
WHERE s.code = 'research';
SQL