#!/usr/bin/env bash

content="test"

setup() {
  echo "$content" > file
}

run_test() {
  result=$(curl -s --path-as-is localhost:8000/.)
  test "file is not found or expired :(" = "$result"

  result=$(curl -s --write-out "%{http_code}" --path-as-is localhost:8000/../test.sh)
  test "404" = "$result"

  result=$(curl -s -X POST -F "file=@file;filename=../." localhost:8000)
  test "$content" = "$(cat upload/file.txt)"
}

teardown() {
  rm file
  rm -r upload
}
