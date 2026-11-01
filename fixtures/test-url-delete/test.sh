#!/usr/bin/env bash

url="https://orhun.dev/"

setup() {
  :;
}

run_test() {
  url_url=$(curl -s -F "url=$url" localhost:8000)
  test "$url" = "$(cat upload/url/url)"
  test "$url_url" = "http://localhost:8000/url"
  test "file deleted" = "$(curl -s -H "Authorization: may_the_force_be_with_you" -X DELETE http://localhost:8000/url)"
  test "file is not found or expired :(" = "$(curl -s -H "Authorization: may_the_force_be_with_you" -X DELETE http://localhost:8000/url)"
}

teardown() {
  rm -r upload
}
