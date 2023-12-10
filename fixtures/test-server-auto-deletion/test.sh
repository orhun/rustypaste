#!/usr/bin/env bash

content="test content"

setup() {
  echo "$content" > file
}

run_test() {
  first_file_url=$(curl -s -F "file=@file" -H "expire:1s" localhost:8000)
  second_file_url=$(curl -s -F "file=@file" -H "expire:4s" localhost:8000)
  test "$content" = "$(curl -s $first_file_url)"
  test "$content" = "$(curl -s $second_file_url)"
  sleep 3
  test "file is not found or expired :(" = "$(curl -s $first_file_url)"
  test "$content" = "$(curl -s $second_file_url)"
  sleep 1
  test "file is not found or expired :(" = "$(curl -s $second_file_url)"
}

teardown() {
  rm file
  rm -r upload
}
