#!/usr/bin/env bash
# Napközbeni research szinkron: friss envelope gyűjtés + discovery job a v2-nek.
#
# A research napi gyűjtő launchd-ágense (com.footballresearch.daily) le van
# tiltva, ezért a `api/v1/daily/<év>/<hó>/<nap>.json` csak kézi futásra
# frissül. Ez a script egyetlen, idempotens belépőpontot ad:
#
#   1) lefuttatja a research projekt collect-only pipeline-ját a mai napra;
#   2) berak egy `research` discovery jobot a v2 adatbázisba, hogy a worker
#      betöltse az eseményeket/csapatstatisztikát.
#
# Használat:
#   scripts/research-sync.sh                # gyűjtés + job a mai napra
#   scripts/research-sync.sh 2026-11-21     # adott napra
#   scripts/research-sync.sh --enqueue-only # csak job (envelope már friss)
#
# Ütemezés (opcionális, külön üzemeltetési döntés): a dokumentációban leírt
# launchd minta óráról órára hívja ezt a scriptet. A script maga nem telepít
# launchd agentet.
set -euo pipefail

cd "$(dirname "$0")/.."
REPO="$PWD"
DATE_ARG=""
ENQUEUE_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --enqueue-only) ENQUEUE_ONLY=1 ;;
    -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
    *) DATE_ARG="$arg" ;;
  esac
done
DATE="${DATE_ARG:-$(TZ=Europe/Budapest date +%F)}"
RESEARCH_ROOT="${FOOTBALL_RESEARCH_ROOT:-$HOME/Desktop/football-research}"
LOG_DIR="$HOME/Library/Logs/FootballAppV2"
LOCK_DIR="$HOME/Library/Application Support/FootballAppV2/.research-sync.lock"
LOG="$LOG_DIR/research-sync.log"

mkdir -p "$LOG_DIR"
log() { printf '[%s] %s\n' "$(date '+%F %H:%M:%S')" "$*" | tee -a "$LOG"; }

if [ ! -f "$REPO/.env" ]; then
  log "HIBA: hiányzik a .env ($REPO/.env)"
  exit 1
fi
set -a
# shellcheck disable=SC1091
source "$REPO/.env"
set +a
: "${DATABASE_URL:?DATABASE_URL hiányzik a .env-ből}"

cleanup() { rmdir "$LOCK_DIR" 2>/dev/null || true; }
if ! mkdir "$LOCK_DIR" 2>/dev/null; then
  log "már fut egy research-sync (lock: $LOCK_DIR) — kilépés"
  exit 0
fi
trap cleanup EXIT

if [ "$ENQUEUE_ONLY" -eq 0 ]; then
  if [ ! -f "$RESEARCH_ROOT/scripts/collect-only.sh" ]; then
    log "HIBA: nincs research collect-only.sh itt: $RESEARCH_ROOT/scripts"
    exit 1
  fi
  log "research gyűjtés indul: $DATE (root: $RESEARCH_ROOT)"
  if ! bash "$RESEARCH_ROOT/scripts/collect-only.sh" "$DATE" >>"$LOG" 2>&1; then
    log "HIBA: a research gyűjtés elhasalt (részletek: $LOG)"
    exit 1
  fi
  log "research gyűjtés kész: $DATE"
fi

# Ha az elmúlt 20 percben már beraktunk egy discovery jobot erre a napra,
# ne duplikáljunk (a worker soronként egy napot dolgoz fel).
RECENT=$(psql "$DATABASE_URL" -X -q -tA -c "
  SELECT count(*) FROM ops.job
  WHERE source_slug='research' AND event_type='discovery' AND time_window='$DATE'
    AND status IN ('pending','leased') AND created_at > now() - interval '20 minutes';")
if [ "$RECENT" != "0" ]; then
  log "már van friss discovery job erre a napra ($DATE) — nincs új job"
  exit 0
fi

ENTITY_KEY="research-sync-${DATE}-$(TZ=Europe/Budapest date +%H%M)"
JOB_ID=$(psql "$DATABASE_URL" -X -q -tA -c "
  INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type, time_window, due_at, priority, status)
  VALUES (gen_random_uuid(), 'research', 'day', '$ENTITY_KEY', 'discovery', '$DATE', now(), 5, 'pending')
  RETURNING id;")
log "discovery job berakva: $JOB_ID ($ENTITY_KEY, nap: $DATE)"