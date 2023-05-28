#!/usr/bin/env bash

url="https://orhun.dev/"

setup() {
  :;
}

run_test() {
  file_url=$(curl -s -F "oneshot_url=$url" localhost:8000)
  test "$url" = $(curl -Ls -w %{url_effective} -o /dev/null "$file_url")
  test "$url" = "$(cat upload/oneshot_url/oneshot_url.*)"

  result="$(curl -s $file_url)"
  test "file is not found or expired :(" = "$result"
}

teardown() {
  rm -r upload
}
