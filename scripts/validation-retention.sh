#!/usr/bin/env bash
# Validációs run/finding retenció (dev adatbázis).
#
# A `reverify` minden futtatáskor új ops.validation_run + ops.validation_finding
# sorokat ír. Ezeknek nincs API/UI fogyasztója (a kanonikus audit a
# ops.audit_eventben él), ezért a 3 napnál régebbiek törölhetők. A törlés a
# findingekre ON DELETE CASCADE-vel hat; a fact.verification.last_run_id
# hivatkozást (nincs cascade) a script előbb NULL-ra állítja és auditálja.
#
# A created_at index nélkül a DELETE és az előzményszámlálás is teljes
# seq scant végzett a 3M+ soros táblán (1h34m, 99% CPU). Az indexet
# CONCURRENTLY hozzuk létre, tranzakción kívül, a törlés előtt.
#
# Használat:
#   scripts/validation-retention.sh            # száraz futtatás, csak mérés
#   scripts/validation-retention.sh --apply    # tényleges törlés
#   scripts/validation-retention.sh --days 7 --apply
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi
: "${DATABASE_URL:?DATABASE_URL hiányzik}"

DAYS=3
APPLY=0
for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --days=*) DAYS="${arg#--days=}" ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) printf 'ismeretlen argumentum: %s\n' "$arg" >&2; exit 2 ;;
  esac
done

# A cutoffot EGYSZER számoljuk ki: a "now() - N days" mozgó célpont, és a
# közben futó reverify újabb sorokat toln át a határon (4 órás próbafuttatásnál
# 890 ezerből 1,75 millióra nőtt a törlendő halmaz).
CUTOFF_TS=$(psql "$DATABASE_URL" -X -q -tA -c "SELECT now() - interval '$DAYS days';")
CUTOFF="'$CUTOFF_TS'::timestamptz"
if [ "$APPLY" -eq 1 ]; then
  HAS_INDEX=$(psql "$DATABASE_URL" -X -q -tA -c "
    SELECT count(*) FROM pg_indexes WHERE schemaname='fact' AND tablename='verification'
      AND indexdef LIKE '%last_run_id%';")
  if [ "$HAS_INDEX" -eq 0 ]; then
    echo 'HIBA: hiányzik a fact.verification(last_run_id) index (0023 migráció) — törlés nélkül kilépés' >&2
    exit 1
  fi
fi
METRICS=$(psql "$DATABASE_URL" -X -q -tA -F '|' -c "
SELECT
  (SELECT count(*) FROM ops.validation_run WHERE created_at < $CUTOFF),
  (SELECT count(*) FROM ops.validation_finding f JOIN ops.validation_run r ON r.id=f.run_id WHERE r.created_at < $CUTOFF),
  (SELECT count(*) FROM fact.verification v JOIN ops.validation_run r ON r.id=v.last_run_id WHERE r.created_at < $CUTOFF);")
IFS='|' read -r RUNS FINDINGS VERIFS <<<"$METRICS"
printf 'kornyezet: cutoff=%s (%s nap) | torlendo run=%s finding=%s verification-hivatkozas=%s\n' "$CUTOFF_TS" "$DAYS" "$RUNS" "$FINDINGS" "$VERIFS"

if [ "$APPLY" -ne 1 ] || [ "$RUNS" -eq 0 ]; then
  echo 'száraz futtatás — nincs írás (--apply a tényleges törléshez)'
  exit 0
fi

# Tranzakción kívül kell lennie: CREATE INDEX CONCURRENTLY nem futtatható BEGIN-ben.
# A meglévő index nem érintődik; ha már van, ez no-op.
psql "$DATABASE_URL" -X -q -v ON_ERROR_STOP=1 -c \
  "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_validation_run_created_at ON ops.validation_run (created_at);"

psql "$DATABASE_URL" -X -q -v ON_ERROR_STOP=1 <<SQL
BEGIN;
UPDATE fact.verification v SET last_run_id=NULL
  WHERE v.last_run_id IN (SELECT r.id FROM ops.validation_run r WHERE r.created_at < $CUTOFF);
DELETE FROM ops.validation_run WHERE created_at < $CUTOFF;
INSERT INTO ops.audit_event (id,actor,action,subject_kind,subject_id,detail)
VALUES (gen_random_uuid(),'ops-retention','validation_retention','database',gen_random_uuid(),
        jsonb_build_object('scope','validation', 'cutoff',$CUTOFF,
                           'days', $DAYS, 'runs_deleted', $RUNS, 'findings_deleted', $FINDINGS,
                           'verification_refs_nulled', $VERIFS, 'reason',
                           'regenerálható validációs előzmény; kanonikus audit az ops.audit_eventben'));
COMMIT;
VACUUM (ANALYZE) ops.validation_finding;
VACUUM (ANALYZE) ops.validation_run;
SQL
echo 'retenció kész'
