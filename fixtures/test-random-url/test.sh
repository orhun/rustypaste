#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
}

run_test() {
  first_file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$first_file_url" != "http://localhost:8000/file.txt"
  test "$content" = "$(curl -s $first_file_url)"

  second_file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$first_file_url" != "http://localhost:8000/file.txt"
  test "$content" = "$(curl -s $first_file_url)"

  test "$first_file_url" != "$second_file_url"

  test "$(cat upload/${first_file_url/http:\/\/localhost:8000\//})" \
    = "$(cat upload/${second_file_url/http:\/\/localhost:8000\//})"
}

teardown() {
  rm file
  rm -r upload
}
