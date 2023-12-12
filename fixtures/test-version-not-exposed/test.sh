#!/usr/bin/env bash

setup() {
  echo "$content" > file
}

run_test() {
  result=$(curl -s --write-out "%{http_code}" http://localhost:8000/version)
  test "404" = "$result"
}

teardown() {
  rm file
  rm -r upload
}
