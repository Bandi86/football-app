#!/usr/bin/env bash
# Napi read-only ellenőrzés: tegnapi adat integritása, mai adat készültsége, egyéb hibák.
# Használat: ./scripts/daily-check.sh [DÁTUM]   (alap: tegnap, Europe/Budapest)
# Kilépés: 0 = kemény hiba nincs; 1 = daemon/integritás/API hiba.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
set -a && . ./.env && set +a

CTL=""
for c in "$ROOT/rust/target/debug/footballctl" \
         "$HOME/Library/Application Support/FootballAppV2/agent/bin/footballctl"; do
  [ -x "$c" ] && { CTL="$c"; break; }
done
[ -n "$CTL" ] || CTL="$(command -v footballctl || true)"
RUNTIME="$HOME/Library/Application Support/FootballAppV2/runtime.json"

MAI="$(TZ=Europe/Budapest date +%F)"
MA="${1:-$(TZ=Europe/Budapest date -v-1d +%F)}"

if [ ! -f "$RUNTIME" ]; then
  echo "[HIBA] hiányzó runtime konfig: $RUNTIME" >&2
  exit 1
fi
PORT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["port"])' "$RUNTIME")"
TOKEN="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["read_token"])' "$RUNTIME")"

hiba=0
echo "=== Napi check — $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
echo "Tegnap: $MA | Ma: $MAI"

# 1. Daemon health
if curl -fsS --max-time 5 "http://127.0.0.1:$PORT/v1/health" >/dev/null 2>&1; then
  echo "[OK] daemon health (127.0.0.1:$PORT)"
else
  echo "[HIBA] daemon nem elérhető a $PORT porton"
  hiba=1
fi

# 2. Tegnapi integritás — kemény kapu
echo
echo "--- Integritás: $MA ---"
if [ -n "$CTL" ]; then
  "$CTL" integrity --from "$MA" --to "$MA" 2>&1 || hiba=1
  echo
  echo "--- Reverify: $MA ---"
  "$CTL" reverify --from "$MA" --to "$MA" 2>&1 || hiba=1
  echo
  echo "--- Coverage: $MA ---"
  "$CTL" coverage-day --from "$MA" --to "$MA" 2>&1
  echo
  echo "--- Coverage: $MAI ---"
  "$CTL" coverage-day --from "$MAI" --to "$MAI" 2>&1
else
  echo "[HIBA] footballctl nem található"
  hiba=1
fi

# 3. API napi végpontok — státuszok, hiányok, frissítési idő
for DAY in "$MA" "$MAI"; do
  OUT="/tmp/fb-daily-check-$DAY.json"
  echo
  echo "--- API /v1/day/$DAY ---"
  if curl -fsS --max-time 15 -H "Authorization: Bearer $TOKEN" \
      "http://127.0.0.1:$PORT/v1/day/$DAY" -o "$OUT" 2>/dev/null; then
    python3 - "$DAY" "$OUT" <<'PY'
import collections, json, sys
day, path = sys.argv[1], sys.argv[2]
d = json.load(open(path))
matches = d.get("matches", [])
status = collections.Counter(m.get("status") for m in matches)
publish = collections.Counter(m.get("publish_state") for m in matches)
gaps = {k: v for k, v in (d.get("gaps") or {}).items() if v}
print(f"meccsek: {len(matches)}")
print("státusz:", dict(status))
print("publikáció:", dict(publish))
print("frissítve:", d.get("refreshed_at"))
print("hiányok:", gaps if gaps else "nincs")
PY
  else
    echo "[HIBA] /v1/day/$DAY nem kérhető le"
    hiba=1
  fi
done

# 4. Egyéb hibák — review, job, breaker, publish gate
echo
echo "--- Egyéb ---"
psql "$DATABASE_URL" -Atc "
select 'függő review: ' || count(*) from identity.review_queue where status='pending'
union all select 'failed job: ' || count(*) from ops.job where status='failed'
union all select 'pending job: ' || count(*) from ops.job where status='pending'
union all select 'nyitott breaker: ' || coalesce(string_agg(source_slug || ' (' || failure_count || ')', ', '), 'nincs') from ops.breaker_state where state <> 'closed'
union all select 'withheld gate (' || '$MA' || '): ' || count(*) from ops.publish_gate where publish_date = '$MA' and state ilike '%withheld%'
union all select 'verified gate (' || '$MA' || '): ' || count(*) from ops.publish_gate where publish_date = '$MA' and state = 'verified_publish';
"

# 5. Soccerway tiltás (S116) — a policy-t kézzel állítottuk false-ra; ha egy
#    jövőbeli seed/script visszaállítaná, a napi check jelezze. Read-only: nem ír.
SOCCERWAY_ENABLED="$(psql "$DATABASE_URL" -Atc "select enabled from ops.source_policy where source_slug='soccerway'" 2>/dev/null)"
if [ "$SOCCERWAY_ENABLED" = "t" ]; then
  echo "[HIBA] soccerway újra engedélyezve — S116 szerint tiltva kell maradnia" >&2
  hiba=1
else
  echo "[OK] soccerway tiltva (enabled=${SOCCERWAY_ENABLED:-nincs sor})"
fi

echo
if [ "$hiba" -eq 0 ]; then
  echo "VÉGEREDMÉNY: ZÖLD — kemény hiba nincs (a figyelmeztetéseket fent ellenőrizd)."
else
  echo "VÉGEREDMÉNY: PIROS — kemény hiba történt."
fi
exit "$hiba"
