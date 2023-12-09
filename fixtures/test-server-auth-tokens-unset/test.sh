#!/usr/bin/env bash

content="topsecret"

setup() {
  echo "$content" > file
}

run_test() {
  result=$(curl -s -F "file=@file" localhost:8000)
  test "unauthorized" != "$result"
  test "$content" = "$(cat upload/file.txt)"
  test "$content" = "$(curl -s $result)"
}

teardown() {
  rm file
  rm -r upload
}
