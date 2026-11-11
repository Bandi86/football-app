#!/usr/bin/env bash
set -euo pipefail

if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

: "${DATABASE_URL:?DATABASE_URL hiányzik}"
: "${TEST_DATABASE_URL:?TEST_DATABASE_URL hiányzik}"

for name in DATABASE_URL TEST_DATABASE_URL; do
  url="${!name}"
  identity="$(psql "$url" -v ON_ERROR_STOP=1 -tA -F '|' -c 'SELECT current_database(), current_user;')"
  tables="$(psql "$url" -v ON_ERROR_STOP=1 -tAc "SELECT count(*) FROM pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema');")"
  rules="$(psql "$url" -v ON_ERROR_STOP=1 -tAc "SELECT count(*) FROM ops.validation_rule;")"
  business="$(psql "$url" -v ON_ERROR_STOP=1 -tAc "SELECT
    (SELECT count(*) FROM raw.artifact) + (SELECT count(*) FROM raw.capture) +
    (SELECT count(*) FROM stage.observation) + (SELECT count(*) FROM ops.source) +
    (SELECT count(*) FROM ops.validation_run) + (SELECT count(*) FROM ops.validation_finding) +
    (SELECT count(*) FROM ops.audit_event) + (SELECT count(*) FROM core.entity) +
    (SELECT count(*) FROM core.country) + (SELECT count(*) FROM core.competition) +
    (SELECT count(*) FROM core.season) + (SELECT count(*) FROM core.team) +
    (SELECT count(*) FROM core.match) + (SELECT count(*) FROM core.match_result) +
    (SELECT count(*) FROM identity.source_entity_ref) + (SELECT count(*) FROM fact.provenance) +
    (SELECT count(*) FROM fact.conflict) + (SELECT count(*) FROM fact.verification) +
    (SELECT count(*) FROM ops.publish_gate) +
    (SELECT count(*) FROM core.standing) + (SELECT count(*) FROM fact.odds_snapshot);")"

  if [ "$tables" -ne 23 ] || [ "$rules" -ne 6 ]; then
    printf 'HIBA: %s nincs a 0003 migráció utáni elvárt állapotban (táblák: %s, szabályok: %s, üzleti sorok: %s)\n' "$identity" "$tables" "$rules" "$business" >&2
    exit 1
  fi

  if [ "$name" = "TEST_DATABASE_URL" ] && [ "$business" -ne 0 ]; then
    printf 'HIBA: %s tesztadatbázis üzleti sorokat tartalmaz (%s) — db-reset.sh szükséges\n' "$identity" "$business" >&2
    exit 1
  fi

  printf 'OK: %s, táblák: %s, szabályok: %s, üzleti sorok: %s\n' "$identity" "$tables" "$rules" "$business"
done