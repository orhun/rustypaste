#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
  echo "<html></html>" > file.html
}

run_test() {
  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$file_url" = "http://localhost:8000/file.bin"
  test "$content" = "$(cat upload/file.bin)"
  test "$content" = "$(curl -s $file_url)"

  test "http://localhost:8000/file.html" = "$(curl -s -F file=@file.html localhost:8000)"
}

teardown() {
  rm file*
  rm -r upload
}
