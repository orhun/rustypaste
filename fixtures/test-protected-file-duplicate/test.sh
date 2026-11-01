#!/usr/bin/env bash

content="secret_protected_content"

setup() {
  echo "$content" > file
}

run_test() {
  # Upload same file twice as protected
  response1=$(curl -s -F "protected=@file" localhost:8000)
  response2=$(curl -s -F "protected=@file" localhost:8000)

  url1=$(echo "$response1" | head -1)
  url2=$(echo "$response2" | head -1)
  password1=$(echo "$response1" | tail -1 | sed 's/Password: //')
  password2=$(echo "$response2" | tail -1 | sed 's/Password: //')

  test "$url1" != "$url2"
  test "$password1" != "$password2"
}

teardown() {
  rm file
  rm -r upload
}
