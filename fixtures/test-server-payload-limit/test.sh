#!/usr/bin/env bash

setup() {
  touch emptyfile
  truncate -s 9KB smallfile
  # On Linux, `fallocate -l 10000 normalfile` can be used for a better precision.
  dd if=/dev/random of=normalfile count=10000 bs=1024 status=none
  truncate -s 11KB bigfile
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
