#!/usr/bin/env bash

duplicate_content="test data"

setup() {
  echo "$duplicate_content" > file
  date +%s > unique_file1
  sleep 1
  date +%s > unique_file2
}

run_test() {
  first_file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$duplicate_content" = "$(cat upload/${first_file_url/http:\/\/localhost:8000\//})"

  second_file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$first_file_url" = "$second_file_url"
  for url in "$first_file_url" "$second_file_url"; do
    test "$duplicate_content" = "$(curl -s $url)"
  done

  first_file_url=$(curl -s -F "file=@unique_file1" localhost:8000)
  second_file_url=$(curl -s -F "file=@unique_file2" localhost:8000)
  test "$first_file_url" != "$second_file_url"
}

teardown() {
  rm file unique_file1 unique_file2
  rm -r upload
}
