#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
}

run_test() ( set -e;
  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$file_url" = "http://localhost:8000/file.txt"
  test "$content" = "$(cat upload/file.txt)"
  test "$content" = "$(curl -s $file_url)"
)

teardown() {
  rm file
  rm -r upload
}
