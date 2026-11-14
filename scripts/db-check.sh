#!/usr/bin/env bash
set -euo pipefail

if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

: "${DATABASE_URL:?DATABASE_URL hiányzik}"
: "${TEST_DATABASE_URL:?TEST_DATABASE_URL hiányzik}"

# Elvárt táblaszám: mindkét DB a 0001–0008 migrációk után 35 tábla
# (34 terméktábla + _sqlx_migrations); a 0008 két új táblát ad (odds_quote, match_stats).
expected_tables=35

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
    (SELECT count(*) FROM core.standing) + (SELECT count(*) FROM fact.odds_snapshot) +
    (SELECT count(*) FROM ops.source_trust);")"

  if [ "$tables" -ne "$expected_tables" ] || [ "$rules" -ne 6 ]; then
    printf 'HIBA: %s nincs az elvárt migrációs állapotban (elvárt táblák: %s, táblák: %s, szabályok: %s, üzleti sorok: %s)\n' "$identity" "$expected_tables" "$tables" "$rules" "$business" >&2
    exit 1
  fi

  # A 0004 worker táblái az üzleti összegben NEM szerepelnek, külön számoljuk
  # őket; a lekérdezés a táblaszám-ellenőrzés UTÁN fut, hogy hiányzó sémánál a
  # fenti barátságos hibaüzenet jelenjen meg, ne nyers psql-hiba.
  worker_rows="$(psql "$url" -v ON_ERROR_STOP=1 -tAc "SELECT
    (SELECT count(*) FROM ops.source_policy) + (SELECT count(*) FROM ops.fetch_ledger) +
    (SELECT count(*) FROM ops.breaker_state) + (SELECT count(*) FROM ops.job) +
    (SELECT count(*) FROM ops.coverage_slot) + (SELECT count(*) FROM ops.robots_cache);")"

  if [ "$name" = "TEST_DATABASE_URL" ] && { [ "$business" -ne 0 ] || [ "$worker_rows" -ne 0 ]; }; then
    printf 'HIBA: %s tesztadatbázis üzleti sorokat tartalmaz (üzleti: %s, worker: %s) — db-reset.sh szükséges\n' "$identity" "$business" "$worker_rows" >&2
    exit 1
  fi

  printf 'OK: %s, táblák: %s, szabályok: %s, üzleti sorok: %s, worker sorok: %s\n' "$identity" "$tables" "$rules" "$business" "$worker_rows"
done
