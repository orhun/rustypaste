#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file
}

run_test() {
  file_url=$(curl -s -F "file=@file" -H "filename:fn_from_header.txt" localhost:8000)
  test "$file_url" = "http://localhost:8000/fn_from_header.txt"
  test "$content" = "$(cat upload/fn_from_header.txt)"
  test "$content" = "$(curl -s $file_url)"
  file_url=$(curl -s -F "file=@file" -H "filename:fn_from_header.txt" localhost:8000)
  test "$file_url" = "file already exists"
  status_code=$(curl -s -F "file=@file" -H "filename:fn_from_header.txt" -w "%{response_code}" -o /dev/null localhost:8000)
  test "$status_code" = "409"
}

teardown() {
  rm file
  rm -r upload
}
