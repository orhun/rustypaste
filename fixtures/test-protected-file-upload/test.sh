#!/usr/bin/env bash

content="secret_protected_content"

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

  # Access without auth should fail (404)
  result=$(curl -s -w '%{http_code}' -o /dev/null "$file_url")
  test "$result" = "404"

  # Access with wrong password should fail (404)
  result=$(curl -s -w '%{http_code}' -o /dev/null -H "Authorization: Bearer wrongpassword" "$file_url")
  test "$result" = "404"

  # Access with correct Bearer auth should succeed
  result=$(curl -s -H "Authorization: Bearer $password" "$file_url")
  test "$content" = "$result"

  # Access with correct Basic auth should succeed
  basic_auth=$(echo -n "user:$password" | base64)
  result=$(curl -s -H "Authorization: Basic $basic_auth" "$file_url")
  test "$content" = "$result"
}

teardown() {
  rm file
  rm -r upload
}
