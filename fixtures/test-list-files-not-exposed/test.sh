#!/usr/bin/env bash

setup() {
  :;
}

run_test() {
  result=$(curl -s --write-out "%{http_code}" http://localhost:8000/list)
  test "404" = "$result"
}

teardown() {
  rm file
  rm -r upload
}
