#!/usr/bin/env bash

content="test data"

setup() {
  echo "$content" > file.txt
  echo "$content" > file
}

run_test() {
  file_url1=$(curl -s -F "file=@file.txt" localhost:8000)
  file_name1=$(echo $file_url1 |cut -d'/' -f4)
  file_url2=$(curl -s -F "file=@file" localhost:8000)
  file_name2=$(echo $file_url2 |cut -d'/' -f4)

  test "${#file_name1}" = "6"
  test "${#file_name2}" = "6"
}

teardown() {
  rm file.txt file
  rm -r upload
}
