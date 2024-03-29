#!/usr/bin/env bash

content="test data for space replacement"

setup() {
  echo "$content" > "test file with spaces.txt"
}

run_test() {
  # Upload the file and get the URL.
  replaced_url=$(curl -s -F "file=@test file with spaces.txt" localhost:8000)

  # Ensure the URL contains underscores instead of spaces.
  expected_url="http://localhost:8000/test_file_with_spaces.txt"
  test "$replaced_url" = "$expected_url"
}

teardown() {
  rm "test file with spaces.txt"
  rm -r upload
}
