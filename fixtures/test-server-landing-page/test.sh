#!/usr/bin/env bash

landing_page="awesome_landing"

setup() {
  :;
}

run_test() {
  result=$(curl -s localhost:8000)
  test "$landing_page" = "$result"
}

teardown() {
  :;
}
