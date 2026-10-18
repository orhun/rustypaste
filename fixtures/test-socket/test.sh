#!/usr/bin/env bash

setup() {
  :;
}

run_test() {
  ver1=$(curl -s localhost:8000/version)
  ver2=$(curl -s --unix-socket ./rustypaste.sock http://localhost/version)
  test "$ver1" = "$ver2"
}

teardown() {
  rm rustypaste.sock
  rm -r upload
}
