#!/usr/bin/env bash
set -euo pipefail

if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

: "${TEST_DATABASE_URL:?TEST_DATABASE_URL hiányzik}"
export DATABASE_URL="$TEST_DATABASE_URL"

./scripts/db-reset.sh
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1