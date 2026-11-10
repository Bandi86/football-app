#!/usr/bin/env bash
set -euo pipefail

if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

: "${TEST_DATABASE_URL:?TEST_DATABASE_URL hiányzik — hozd létre a .env fájlt a .env.example alapján}"

if [[ "$TEST_DATABASE_URL" =~ ://[^:]+:([^@]+)@ ]]; then
  db_password="${BASH_REMATCH[1]}"
else
  printf 'HIBA: nem sikerült a jelszót kiolvasni a TEST_DATABASE_URL-ból\n' >&2
  exit 1
fi

psql -d postgres -v ON_ERROR_STOP=1 -v pw="$db_password" <<'SQL'
SELECT format('CREATE ROLE football_v2 LOGIN CREATEDB PASSWORD %L', :'pw')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'football_v2')
\gexec
SQL

for db in football_v2_dev football_v2_test; do
  if [ "$(psql -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = '${db}'")" != "1" ]; then
    createdb -O football_v2 "${db}"
    printf 'Létrehozva: %s\n' "${db}"
  else
    printf 'Már létezik: %s\n' "${db}"
  fi
done
