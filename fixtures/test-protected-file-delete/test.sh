#!/usr/bin/env bash

content="protected delete test"

setup() {
  echo "$content" > file
}

run_test() {
  # Upload protected file
  response=$(curl -s -F "protected=@file" localhost:8000)

  # Parse URL (first line) and password (second line after "Password: ")
  file_url=$(echo "$response" | head -1)
  password=$(echo "$response" | tail -1 | sed 's/Password: //')

  # Verify file exists in protected directory
  test "$content" = "$(cat upload/protected/file.txt)"

  # Verify password file exists
  test -f "upload/protected/file.txt.password"

  # Delete the file
  test "file deleted" = "$(curl -s -H "Authorization: may_the_force_be_with_you" -X DELETE "$file_url")"

  # Verify both files are gone
  test ! -f "upload/protected/file.txt"
  test ! -f "upload/protected/file.txt.password"

  # Second delete should fail
  test "file is not found or expired :(" = "$(curl -s -H "Authorization: may_the_force_be_with_you" -X DELETE "$file_url")"
}

teardown() {
  rm file
  rm -r upload
}
