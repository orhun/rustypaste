#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
}

run_test() {
  file_url=$(curl -s -F "file=@file" -H "expire:2s" localhost:8000)
  file_url=$(curl -s -F "file=@file" -H "expire:1s" localhost:8000)
  sleep 2
  file_url=$(curl -s -F "file=@file" -H "expire:1m" localhost:8000)
  test "$content" = "$(curl -s $file_url)"
}

teardown() {
  rm file
  rm -r upload
}
