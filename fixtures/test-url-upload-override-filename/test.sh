#!/usr/bin/env bash

url="https://orhun.dev/"

url2="https://orhun.dev/does/not/exist"

setup() {
  :;
}

run_test() {
  curl -s -F "url=$url" -H "filename: abc" localhost:8000 > /dev/null
  test "$url" = "$(cat upload/url/abc)"

  curl -s -F "url=$url2" -H "filename: abc" localhost:8000 > /dev/null
  test "$url2" = "$(cat upload/url/abc)"

  curl -s -F "url=$url2" -H "filename: what-a-great-link" localhost:8000 > /dev/null
  test "$url2" = "$(cat upload/url/what-a-great-link)"
}

teardown() {
  rm -r upload
}
