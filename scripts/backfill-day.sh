#!/usr/bin/env bash
# Egy nap research backfill -> v2 betöltés -> ellenőrzés, egy parancsban (S119).
#
# Használat:
#   scripts/backfill-day.sh 2026-10-27          # research már kész: csak v2-lánc
#   scripts/backfill-day.sh --fetch 2026-10-27  # backfill + teljes lánc (hálózat)
#
# A lánc lépései (mindegyik saját kapuval, fail-closed):
#   1. [--fetch] research backfill (fetch + enrich + odds + verify + deep_verify)
#   2. api-export (lokális, a research dokumentumokból)
#   3. research discovery job + várakozás a workerre (legfeljebb ~10 perc)
#   4. match-insight-import --apply
#   5. player-import --apply
#   6. integrity + coverage-day
#
# Kilépés: 0 = a nap kész és tiszta; 1 = bármely lépés hibát jelzett.
set -euo pipefail

FETCH=0
if [[ "${1:-}" == "--fetch" ]]; then
  FETCH=1
  shift
fi
DATE="${1:?használat: backfill-day.sh [--fetch] YYYY-MM-DD}"
if [[ ! "$DATE" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] \
   || ! date -j -f '%Y-%m-%d' "$DATE" '+%F' >/dev/null 2>&1; then
  echo "[HIBA] érvénytelen dátum: $DATE (YYYY-MM-DD kell)" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
if [[ ! -f .env ]]; then
  echo "[HIBA] hiányzó .env: $ROOT/.env" >&2
  exit 1
fi
set -a
# shellcheck disable=SC1091
. ./.env
set +a
: "${DATABASE_URL:?DATABASE_URL hiányzik a .env-ből}"

RESEARCH="${FOOTBALL_RESEARCH_ROOT:-$HOME/Desktop/football-research}"
CTL=""
for c in "$ROOT/rust/target/debug/footballctl" \
         "$HOME/Library/Application Support/FootballAppV2/agent/bin/footballctl"; do
  if [[ -x "$c" ]]; then CTL="$c"; break; fi
done
[[ -n "$CTL" ]] || CTL="$(command -v footballctl || true)"
if [[ -z "$CTL" ]]; then
  echo "[HIBA] footballctl nem található" >&2
  exit 1
fi

log() { printf '[%s] %s\n' "$(date '+%F %H:%M:%S')" "$*"; }
fail() { log "HIBA: $*"; exit 1; }

# 1. Opcionális research backfill (hálózat, lassú — ezért külön kapcsoló).
if [[ "$FETCH" -eq 1 ]]; then
  log "backfill indul: $DATE"
  (cd "$RESEARCH" && python3 tools/football.py backfill \
      --from "$DATE" --to "$DATE" --order desc --limit 1) || fail "backfill $DATE"
  log "backfill kész: $DATE"
fi

# 2. Lokális export — egyszer. A backfill maga NEM exportál (csak letölt +
# ellenőriz), ezért --fetch módban vagy hiányzó dokumentumoknál itt gyártjuk
# őket; egyébként a meglévő dokumentumokat frissítjük. Az export egyben a napi
# envelope-ot is létrehozza, külön lépés nem kell.
YEAR="${DATE:0:4}"; MONTH="${DATE:5:2}"; DAY="${DATE:8:2}"
MATCH_DIR="$RESEARCH/api/v1/matches/$YEAR/$MONTH/$DAY"
log "api-export: $DATE"
(cd "$RESEARCH" && python3 docs/api-export.py "$DATE") || fail "api-export $DATE"
[[ -d "$MATCH_DIR" ]] || fail "nincs research dokumentum: $MATCH_DIR (előbb --fetch)"

# 3. Discovery job + várakozás a workerre.
log "discovery job: $DATE"
"$ROOT/scripts/research-sync.sh" "$DATE" --enqueue-only || fail "research-sync $DATE"

JOB_ID="$(psql "$DATABASE_URL" -X -q -tA -c \
  "SELECT id FROM ops.job WHERE source_slug='research' AND event_type='discovery' \
   AND time_window='$DATE' ORDER BY created_at DESC LIMIT 1")"
[[ -n "$JOB_ID" ]] || fail "nem található discovery job a napra: $DATE"

log "worker várakozás: $JOB_ID"
STATUS=""
for i in $(seq 1 40); do
  STATUS="$(psql "$DATABASE_URL" -X -q -tA -c \
    "SELECT status FROM ops.job WHERE id='$JOB_ID'")"
  case "$STATUS" in
    done)
      log "worker kész: $DATE"
      break
      ;;
    failed)
      ERR="$(psql "$DATABASE_URL" -X -q -tA -c \
        "SELECT coalesce(last_error,'-') FROM ops.job WHERE id='$JOB_ID'")"
      fail "worker failed ($DATE): $ERR"
      ;;
  esac
  if [[ "$i" -eq 40 ]]; then
    fail "worker timeout (10 perc) a napra: $DATE (utolsó státusz: ${STATUS:-nincs})"
  fi
  sleep 15
done

# 4-5. Offline importok (idempotensek: upsert + DELETE/INSERT a source-ra).
log "match-insight import: $DATE"
"$CTL" match-insight-import --from "$DATE" --to "$DATE" --apply || fail "match-insight-import $DATE"
log "player import: $DATE"
"$CTL" player-import --from "$DATE" --to "$DATE" --apply || fail "player-import $DATE"

# 6. Saját kapu: kemény integritási hibák és lefedettség.
log "integrity + coverage: $DATE"
"$CTL" integrity --from "$DATE" --to "$DATE" || fail "integrity $DATE"
"$CTL" coverage-day --from "$DATE" --to "$DATE" || fail "coverage-day $DATE"

log "KÉSZ: $DATE"
