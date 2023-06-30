#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
  echo "$content" > .file
}

run_test() {
  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$file_url" = "http://localhost:8000/file.txt"
  test "$content" = "$(cat upload/file.txt)"
  test "$content" = "$(curl -s $file_url)"
  file_url2=$(curl -s -F "file=@.file" localhost:8000)
  test "$file_url2" = "http://localhost:8000/.file.txt"
}

teardown() {
  rm file .file
  rm -r upload
}
