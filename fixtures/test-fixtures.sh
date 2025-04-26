#!/usr/bin/env bash

FIXTURE_DIR=$(readlink -f "$(dirname "$0")")
PROJECT_DIR="$FIXTURE_DIR/.."
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

run_fixture() {
  cd "$FIXTURE_DIR/$1" || exit 1
  source "test.sh"
  type custom_env &>/dev/null && custom_env
  NO_COLOR=1 CONFIG=config.toml "$PROJECT_DIR/target/debug/rustypaste" &
  SERVER_PID=$!
  trap 'kill -9 "$SERVER_PID" && wait "$SERVER_PID" 2> /dev/null' RETURN
  sleep 1
  (
    set -e
    setup
    run_test
    teardown
  )
  result=$?
  return "$result"
}

# Run the fixture and print the result
process_fixture() {
  # Since we are creating a subshell, all environment variables created by custom_env will be lost
  # Return code is preserved
  fixture="$1"
  (run_fixture "$fixture")
  exit_status=$?
  if [ "$exit_status" -eq 0 ]; then
    echo -e "[${GREEN}ok${NC}] $fixture"
  else
    echo -e "[${RED}fail${NC}] $fixture"
    exit "$exit_status"
  fi
}

main() {
  # If arguments are passed, run only those fixtures
  if [ $# -ne 0 ]; then
    for fixture in "$@"; do
      process_fixture "$fixture"
    done
  else
  # Otherwise, run all fixtures
    find * -maxdepth 0 -type d -print0 | while IFS= read -r -d '' fixture; do
      process_fixture "$fixture"
    done
  fi
}

[ "$DEBUG" == 'true' ] && set -x && export RUST_LOG=debug
main "$@"
