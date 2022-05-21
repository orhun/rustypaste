#!/usr/bin/env bash

content="test"

setup() {
  for ext in "txt" "tar" "png"; do
    echo "$content" > "file.$ext"
  done
}

run_test() {
  file_url=$(curl -s -F "file=@file.txt" localhost:8000)
  test "application/x-shockwave-flash" = "$(curl -s --write-out %{content_type} $file_url | tail -n 1)"

  file_url=$(curl -s -F "file=@file.tar" localhost:8000)
  test "image/gif" = "$(curl -s --write-out %{content_type} $file_url | tail -n 1)"

  file_url=$(curl -s -F "file=@file.png" localhost:8000)
  test "image/png" = "$(curl -s --write-out %{content_type} $file_url | tail -n 1)"
  test "$content" = "$(curl -s $file_url)"
}

teardown() {
  rm file.*
  rm -r upload
}
