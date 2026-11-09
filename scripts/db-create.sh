#!/usr/bin/env bash
set -euo pipefail

psql -d postgres -v ON_ERROR_STOP=1 <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'football_v2') THEN
    CREATE ROLE football_v2 LOGIN CREATEDB PASSWORD 'football_v2_dev';
  END IF;
END
$$;
SQL

for db in football_v2_dev football_v2_test; do
  if [ "$(psql -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = '${db}'")" != "1" ]; then
    createdb -O football_v2 "${db}"
    printf 'Létrehozva: %s\n' "${db}"
  else
    printf 'Már létezik: %s\n' "${db}"
  fi
done
