## Fixtures

This directory contains the [test fixtures](https://en.wikipedia.org/wiki/Test_fixture) and a simple testing framework for `rustypaste`.

### Running fixtures

1. Build the project in debug mode: `cargo build`
2. Execute the runner script in this directory: `./test-fixtures.sh`

On `macOS` you need to have [coreutils](https://www.gnu.org/software/coreutils/) installed to run the script.
The simplest way is to install it via [Homebrew](https://brew.sh/): `brew install coreutils`

### Adding new fixtures

Create an appropriately named directory for the test fixture you want to add. e.g. `test-file-upload`

Each fixture directory should contain the following files:

```
test-file-upload/
├── config.toml
└── test.sh
```

- `config.toml`: Contains the `rustypaste` configuration. See [the default configuration](../config.toml).
- `test.sh`: Contains the helper functions for testing. The file format is the following:

```sh
#!/usr/bin/env bash

# Optional
custom_env() {
  # setting environment variables such as AUTH_TOKEN or AUTH_TOKENS_FILE
}

setup() {
  # preparation
}

run_test() {
  # assertions
}

teardown() {
  # cleanup
}
```

These functions are executed in the order defined above.

See the [test-file-upload](./test-file-upload/test.sh) fixture for an example.

### Debugging

Set the `DEBUG` environment variable to `true` while executing the runner script:

```sh
$ DEBUG=true ./test-fixtures.sh
```
