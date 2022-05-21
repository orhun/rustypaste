#!/usr/bin/env bash

url="https://orhun.dev/"

setup() {
  :;
}

run_test() {
  curl -s -F "url=$url" localhost:8000 > /dev/null
  test "$url" = "$(cat upload/url/url)"

  result=$(curl -s -F "url=invalidurl" localhost:8000)
  test "relative URL without a base" = "$result"
}

teardown() {
  rm -r upload
}
