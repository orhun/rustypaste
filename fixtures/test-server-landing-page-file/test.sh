#!/usr/bin/env bash

landing_page="awesome_landing_from_file"

setup() {
  echo $landing_page >page.txt
}

run_test() {
  result=$(curl -s localhost:8000)
  test "$landing_page" = "$result"
}

teardown() {
  rm page.txt
}
