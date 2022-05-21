#!/usr/bin/env bash

FIXTURE_DIR=$(readlink -f "$(dirname "$0")")
PROJECT_DIR="$FIXTURE_DIR/.."
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

run_fixture() {
  cd "$FIXTURE_DIR/$1" || exit 1
  source "test.sh"
  NO_COLOR=1 CONFIG=config.toml "$PROJECT_DIR/target/debug/rustypaste" &
  SERVER_PID=$!
  trap 'kill -9 "$SERVER_PID" && wait "$SERVER_PID" 2> /dev/null' RETURN
  sleep 1
  ( set -e;
    setup
    run_test
    teardown
  )
  result=$?
  return "$result"
}

main() {
  find * -maxdepth 0 -type d -print0 | while IFS= read -r -d '' fixture; do
    run_fixture "$fixture"
    exit_status=$?
    if [ "$exit_status" -eq 0 ]; then
      echo -e "[${GREEN}ok${NC}] $fixture"
    else
      echo -e "[${RED}fail${NC}] $fixture"
      exit "$exit_status"
    fi
  done
}

[ "$DEBUG" == 'true' ] && set -x && export RUST_LOG=debug
main
