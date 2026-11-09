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

  if [ "$tables" -ne 0 ]; then
    printf 'HIBA: %s nem üres, felhasználói táblák száma: %s\n' "$identity" "$tables" >&2
    exit 1
  fi

  printf 'OK: %s, felhasználói táblák: %s\n' "$identity" "$tables"
done
