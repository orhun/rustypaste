#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
}

run_test() {
  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$file_url" = "http://localhost:8000/file.txt"
  test "" = "$(curl -s -H "Authorization: may_the_force_be_with_you" -X DELETE http://localhost:8000/file.txt)"
  test "file is not found or expired :(" = "$(curl -s -H "Authorization: may_the_force_be_with_you" -X DELETE http://localhost:8000/file.txt)"
}

teardown() {
  rm file
  rm -r upload
}
