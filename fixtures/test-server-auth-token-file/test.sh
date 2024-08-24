#!/usr/bin/env bash

content1="test data"
content2="other test"

custom_env() {
  export AUTH_TOKENS_FILE=./auth_file
}

setup() {
  echo "$content1" >file1
  echo "$content2" >file2
}

run_test() {
  file_url=$(curl -s -F "file=@file1" -H "Authorization: bread" localhost:8000)
  test "$content1" = "$(cat upload/file1.txt)"
  sleep 2

  result="$(curl -s $file_url)"
  test "$content1" = "$result"

  file_url=$(curl -s -F "file=@file2" -H "Authorization: naan" localhost:8000)
  test "$content2" = "$(cat upload/file2.txt)"
  sleep 2

  result="$(curl -s $file_url)"
  test "$content2" = "$result"

  result=$(curl -s -F "file=@file2" -H "Authorization: tomato" localhost:8000)
  test "$result" = "unauthorized"
}

teardown() {
  rm file1
  rm file2
  rm -r upload
}
