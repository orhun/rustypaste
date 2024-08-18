#!/usr/bin/env bash

setup() {
  truncate -s 9KB bigfile1 bigfile2 bigfile3
}

run_test() {
  result=$(curl -s -F "file=@bigfile1" localhost:8000)
  result=$(curl -s -F "file=@bigfile2" localhost:8000)
  curl -s "$result"

  result=$(curl -s -F "file=@bigfile3" localhost:8000)
  test "upload directory size limit exceeded" = "$result"
}

teardown() {
  rm bigfile* 
  rm -r upload
}
