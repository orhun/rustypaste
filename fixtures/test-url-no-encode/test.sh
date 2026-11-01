#!/usr/bin/env bash

content="test data for URL encoding"

setup() {
  echo "$content" > "test%file#-🤯.txt"
}

run_test() {
  # Upload the file and get the URL.
  encoded_url=$(curl -s -F "file=@test%file#-🤯.txt" localhost:8000)

  # Ensure the URL is encoded correctly.
  expected_url="http://localhost:8000/test%file#-🤯.txt"
  test "$encoded_url" = "$expected_url"
}

teardown() {
  rm "test%file#-🤯.txt"
  rm -r upload
}
