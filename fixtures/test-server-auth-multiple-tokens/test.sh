#!/usr/bin/env bash

auth_tokens="rustypasteisawesome token1 token2 token4"

content="topsecret"

setup() {
  echo "$content" > file
}

run_test() {
  result=$(curl -s -F "file=@file" localhost:8000)
  test "unauthorized" = "$result"

  for auth_token in $auth_tokens
  do
    result=$(curl -s -F "file=@file" -H "Authorization: $auth_token" localhost:8000)
    test "unauthorized" != "$result"
    test "$content" = "$(cat upload/file.txt)"
    test "$content" = "$(curl -s $result)"
  done
}

teardown() {
  rm file
  rm -r upload
}
