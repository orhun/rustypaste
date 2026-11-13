#!/usr/bin/env bash

setup() {
  :;
}

run_test() {
  # The server must refuse to start without an endpoint,
  # so run it directly to check the error message and the exit code.
  status=0
  output=$(CONFIG=config.toml "$PROJECT_DIR/target/debug/rustypaste" 2>&1) || status=$?
  test "$status" -ne 0
  grep -q "no server endpoint is configured" <<<"$output"
}

teardown() {
  rm -r upload
}
