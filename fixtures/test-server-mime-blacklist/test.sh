#!/usr/bin/env bash

content="test data"

setup() {
  echo "<html></html>" > file.html
  echo '<?xml version="1.0" encoding="UTF-8" standalone="no" ?>' > file.xml
  echo "$content" > file.txt
}

run_test() {
  test "this file type is not permitted" = "$(curl -s -F "file=@file.html" localhost:8000)"
  test "this file type is not permitted" = "$(curl -s -F "file=@file.xml" localhost:8000)"
  test "415" = "$(curl -s -F "file=@file.xml" -w "%{response_code}" -o /dev/null localhost:8000)"
  file_url=$(curl -s -F "file=@file.txt" localhost:8000)
  test "$content" = "$(curl -s $file_url)"
}

teardown() {
  rm file.*
  rm -r upload
}
