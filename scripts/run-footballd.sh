#!/usr/bin/env bash
# A napi worker-daemon indító burka (launchd agent és kézi indítás).
# A .env helyben marad; a titkok nem kerülnek a repóba.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

export DATA_DIR="${DATA_DIR:-$PWD/data}"
export WORKER_ENABLED=true
export RUST_LOG="${RUST_LOG:-info}"
export OBSCURA_BIN="${OBSCURA_BIN:-$PWD/../football-app/tools/obscura/obscura}"

BIN="$PWD/rust/target/debug/footballd"
if [ ! -x "$BIN" ]; then
  echo "hiányzó bináris: $BIN — építsd: cd rust && cargo build -p footballd --features obscura" >&2
  exit 1
fi
exec "$BIN"
