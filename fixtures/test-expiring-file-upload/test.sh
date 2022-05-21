#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
}

run_test() ( set -e;
  file_url=$(curl -s -F "file=@file" -H "expire:1s" localhost:8000)
  test "$content" = "$(cat upload/file.txt.*)"
  sleep 2s

  result="$(curl -s $file_url)"
  test "file is not found or expired :(" = "$result"
)

teardown() {
  rm file
  rm -r upload
}
