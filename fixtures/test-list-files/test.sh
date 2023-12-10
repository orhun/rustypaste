#!/usr/bin/env bash

auth_token="rustypasteisawesome"
content="test data"
file_count=3

setup() {
  echo "$content" > file
}

run_test() {
  seq $file_count | xargs -I -- curl -s -F "file=@file" -H "Authorization: $auth_token" localhost:8000 >/dev/null
  test $file_count = $(curl -s -H "Authorization: $auth_token" localhost:8000/list | grep -o 'file_name' | wc -l)
  test "unauthorized" = "$(curl -s localhost:8000/list)"
}

teardown() {
  rm file
  rm -r upload
}
