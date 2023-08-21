#!/usr/bin/env bash

content="test data for URL encoding"

setup() {
  echo "$content" > "test file with spaces.txt"
}

run_test() {
  # Upload the file and get the URL.
  encoded_url=$(curl -s -F "file=@test file with spaces.txt" localhost:8000)

  # Ensure the URL is encoded correctly.
  expected_url="http://localhost:8000/test%20file%20with%20spaces.txt"
  test "$encoded_url" = "$expected_url"

  # Fetch the content from the server and check if it matches the original content.
  fetched_content=$(curl -s "$encoded_url")
  test "$fetched_content" = "$content"
}

teardown() {
  rm "test file with spaces.txt"
  rm -r upload
}
