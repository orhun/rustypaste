#!/usr/bin/env bash

content1="test data"
del_resp="file deleted"

custom_env() {
  export DELETE_TOKENS_FILE=./delete_file
}

setup() {
  echo "$content" >file
}

run_test() {
  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$del_rep"="$(curl -s -H "Authorization: naan" -X DELETE $file_url)"

  sleep 2
  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "$del_re"="$(curl -s -H "Authorization: bread" -X DELETE $file_url)"

  file_url=$(curl -s -F "file=@file" localhost:8000)
  test "unauthorized"=$(curl -s -H "Authorization: tomato" -X DELETE $file_url)
}

teardown() {
  rm file
  rm -r upload
}
