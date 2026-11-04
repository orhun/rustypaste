#!/usr/bin/env bash

setup() {
    :
}

run_test() {
    "$PROJECT_DIR/target/debug/rustypaste" healthcheck 127.0.0.1 8000 || return 1
}

teardown() {
    :
}
