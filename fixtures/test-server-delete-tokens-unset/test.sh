#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
}

run_test() {
  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$file_url" = "http://localhost:8000/file.txt"

  result=$(curl -s --write-out "%{http_code}" -X DELETE http://localhost:8000/file.txt)
  test "404" = "$result"
}

teardown() {
  rm file
  rm -r upload
}
