#!/usr/bin/env bash

content="0nesh0t"

setup() {
  echo "$content" > file
}

run_test() {
  file_url=$(curl -s -F "oneshot=@file" localhost:8000)
  test "$content" = $(curl -s "$file_url")
  test "$content" = "$(cat upload/oneshot/file.txt.*)"

  result="$(curl -s $file_url)"
  test "file is not found or expired :(" = "$result"
}

teardown() {
  rm file
  rm -r upload
}
