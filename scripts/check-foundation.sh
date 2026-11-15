#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi

if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

./scripts/db-check.sh

cd rust
cargo fmt --all --check
# A storage-integrációs tesztek közös teszt-DB-t használnak (TRUNCATE a
# test_pool elején), ezért egy szálon futnak — párhuzamosan versenyhelyzet.
cargo test -p football-domain -p football-storage -- --test-threads=1
cargo clippy -p football-domain -p football-storage --all-targets -- -D warnings
