#!/usr/bin/env bash

url="https://raw.githubusercontent.com/orhun/rustypaste/master/img/rustypaste_logo.png"

setup() {
  :;
}

run_test() ( set -e;
  file_url=$(curl -s -F "remote=$url" localhost:8000)
  curl -s "$file_url" -o uploaded_file > /dev/null
  curl -s "$url" -o remote_file > /dev/null
  test "$(sha256sum uploaded_file | awk '{print $1}')" = "$(sha256sum remote_file | awk '{print $1}')"
)

teardown() {
  rm uploaded_file remote_file
  rm -r upload
}
