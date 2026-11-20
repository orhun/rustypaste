#!/usr/bin/env bash

setup() {
  touch emptyfile
  dd if=/dev/zero of=smallfile bs=1024 count=9 status=none
  dd if=/dev/zero of=normalfile bs=1024 count=10 status=none
  dd if=/dev/zero of=bigfile bs=1024 count=11 status=none
}

run_test() {
  result=$(curl -s -F "file=@emptyfile" localhost:8000)
  test "invalid file size" = "$result"

  result=$(curl -s -F "file=@bigfile" localhost:8000)
  test "upload limit exceeded" = "$result"

  result=$(curl -s -F "file=@normalfile" localhost:8000)
  test "upload limit exceeded" = "$result"

  result=$(curl -s -F "file=@smallfile" localhost:8000)
  test "upload limit exceeded" != "$result"
}

teardown() {
  rm emptyfile smallfile normalfile bigfile
  rm -r upload
}
