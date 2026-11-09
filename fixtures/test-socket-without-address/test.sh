#!/usr/bin/env bash

setup() {
  :;
}

run_test() {
  status=$(curl -s -o /dev/null -w '%{http_code}' --unix-socket ./rustypaste.sock http://localhost/version)
  test "$status" = "200"
}

teardown() {
  rm rustypaste.sock
  rm -r upload
}
