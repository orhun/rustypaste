<a href="https://github.com/orhun/rustypaste"><img src="img/rustypaste_logo.png" width="500"></a>

[![GitHub Release](https://img.shields.io/github/v/release/orhun/rustypaste?style=flat&labelColor=823213&color=2c2c2c&logo=GitHub&logoColor=white)](https://github.com/orhun/rustypaste/releases)
[![Crate Release](https://img.shields.io/crates/v/rustypaste?style=flat&labelColor=823213&color=2c2c2c&logo=Rust&logoColor=white)](https://crates.io/crates/rustypaste/)
[![Coverage](https://img.shields.io/codecov/c/gh/orhun/rustypaste?style=flat&labelColor=823213&color=2c2c2c&logo=Codecov&logoColor=white)](https://codecov.io/gh/orhun/rustypaste)
[![Continuous Integration](https://img.shields.io/github/actions/workflow/status/orhun/rustypaste/ci.yml?branch=master&style=flat&labelColor=823213&color=2c2c2c&logo=GitHub%20Actions&logoColor=white)](https://github.com/orhun/rustypaste/actions?query=workflow%3A%22Continuous+Integration%22)
[![Continuous Deployment](https://img.shields.io/github/actions/workflow/status/orhun/rustypaste/cd.yml?style=flat&labelColor=823213&color=2c2c2c&logo=GitHub%20Actions&logoColor=white&label=deploy)](https://github.com/orhun/rustypaste/actions?query=workflow%3A%22Continuous+Deployment%22)
[![Docker Builds](https://img.shields.io/github/actions/workflow/status/orhun/rustypaste/docker.yml?style=flat&labelColor=823213&color=2c2c2c&label=docker&logo=Docker&logoColor=white)](https://hub.docker.com/r/orhunp/rustypaste)
[![Documentation](https://img.shields.io/docsrs/rustypaste?style=flat&labelColor=823213&color=2c2c2c&logo=Rust&logoColor=white)](https://docs.rs/rustypaste/)

**Rustypaste** is a minimal file upload/pastebin service.

```sh
$ echo "some text" > awesome.txt

$ curl -F "file=@awesome.txt" https://paste.site.com
https://paste.site.com/safe-toad.txt

$ curl https://paste.site.com/safe-toad.txt
some text
```

</details>

<details>
  <summary>Table of Contents</summary>

<!-- vim-markdown-toc GFM -->

- [Features](#features)
- [Installation](#installation)
  - [From crates.io](#from-cratesio)
  - [Arch Linux](#arch-linux)
  - [Alpine Linux](#alpine-linux)
  - [FreeBSD](#freebsd)
  - [Binary releases](#binary-releases)
  - [Build from source](#build-from-source)
    - [Feature flags](#feature-flags)
    - [Testing](#testing)
      - [Unit tests](#unit-tests)
      - [Test Fixtures](#test-fixtures)
- [Usage](#usage)
  - [CLI](#cli)
    - [Expiration](#expiration)
    - [One shot files](#one-shot-files)
    - [One shot URLs](#one-shot-urls)
    - [URL shortening](#url-shortening)
    - [Paste file from remote URL](#paste-file-from-remote-url)
    - [Cleaning up expired files](#cleaning-up-expired-files)
    - [Delete file from server](#delete-file-from-server)
    - [Override the filename when using `random_url`](#override-the-filename-when-using-random_url)
  - [Server](#server)
    - [Authentication](#authentication)
    - [List endpoint](#list-endpoint)
    - [HTML Form](#html-form)
    - [Docker](#docker)
    - [Nginx](#nginx)
  - [Third Party Clients](#third-party-clients)
  - [Contributing](#contributing)
    - [License](#license)

<!-- vim-markdown-toc -->

</details>

## Features

- File upload & URL shortening & upload from URL
  - supports basic HTTP authentication
  - random file names (optional)
    - pet name (e.g. `capital-mosquito.txt`)
    - alphanumeric string (e.g. `yB84D2Dv.txt`)
    - random suffix (e.g. `file.MRV5as.tar.gz`)
  - supports expiring links
    - auto-expiration of files (optional)
    - auto-deletion of expired files (optional)
  - supports one shot links/URLs (can only be viewed once)
  - guesses MIME types
    - supports overriding and blacklisting
    - supports forcing to download via `?download=true`
  - no duplicate uploads (optional)
  - listing/deleting files
  - custom landing page
- Single binary
  - [binary releases](https://github.com/orhun/rustypaste/releases)
- Simple configuration
  - supports hot reloading
- Easy to deploy
  - [docker images](https://hub.docker.com/r/orhunp/rustypaste)
  - [appjail images](https://github.com/AppJail-makejails/rustypaste)
- No database
  - filesystem is used
- Self-hosted
  - _centralization is bad!_
- Written in Rust
  - _blazingly fast!_

## Installation

<details>
  <summary>Packaging status</summary>

[![Packaging status](https://repology.org/badge/vertical-allrepos/rustypaste.svg)](https://repology.org/project/rustypaste/versions)

</details>

### From crates.io

```sh
cargo install rustypaste
```

### Arch Linux

```sh
pacman -S rustypaste
```

### Alpine Linux

`rustypaste` is available for [Alpine Edge](https://pkgs.alpinelinux.org/packages?name=rustypaste&branch=edge). It can be installed via [apk](https://wiki.alpinelinux.org/wiki/Alpine_Package_Keeper) after enabling the [community repository](https://wiki.alpinelinux.org/wiki/Repositories).

```sh
apk add rustypaste
```

### FreeBSD

```sh
pkg install rustypaste
```

### Binary releases

See the available binaries on the [releases](https://github.com/orhun/rustypaste/releases/) page.

### Build from source

```sh
git clone https://github.com/orhun/rustypaste.git
cd rustypaste/
cargo build --release
```

#### Feature flags

- `shuttle`: enable an entry point for deploying on Shuttle
- `openssl`: use distro OpenSSL (binary size is reduced ~20% in release mode)
- `rustls`: use [rustls](https://github.com/rustls/rustls) (enabled as default)

To enable a feature for build, pass `--features` flag to `cargo build` command.

For example, to reuse the OpenSSL present on a distro already:

```sh
cargo build --release --no-default-features --features openssl
```

#### Testing

##### Unit tests

```sh
cargo test -- --test-threads 1
```

##### Test Fixtures

```sh
./fixtures/test-fixtures.sh
```

## Usage

The standalone command line tool (`rpaste`) is available [here](https://github.com/orhun/rustypaste-cli).

### CLI

```sh
function rpaste() {
  curl -F "file=@$1" -H "Authorization: <auth_token>" "<server_address>"
}
```

**\*** consider reading authorization headers from a file. (e.g. `-H @rpaste_auth`)

```sh
# upload a file
$ rpaste x.txt

# paste from stdin
$ rpaste -
```

#### Expiration

```sh
$ curl -F "file=@x.txt" -H "expire:10min" "<server_address>"
```

supported units:

- `nsec`, `ns`
- `usec`, `us`
- `msec`, `ms`
- `seconds`, `second`, `sec`, `s`
- `minutes`, `minute`, `min`, `m`
- `hours`, `hour`, `hr`, `h`
- `days`, `day`, `d`
- `weeks`, `week`, `w`
- `months`, `month`, `M`
- `years`, `year`, `y`

#### One shot files

```sh
$ curl -F "oneshot=@x.txt" "<server_address>"
```

#### One shot URLs

```sh
$ curl -F "oneshot_url=https://example.com" "<server_address>"
```

#### URL shortening

```sh
$ curl -F "url=https://example.com/some/long/url" "<server_address>"
```

#### Paste file from remote URL

```sh
$ curl -F "remote=https://example.com/file.png" "<server_address>"
```

#### Cleaning up expired files

Configure `[paste].delete_expired_files` to set an interval for deleting the expired files automatically.

On the other hand, following script can be used as [cron](https://en.wikipedia.org/wiki/Cron) for cleaning up the expired files manually:

```sh
#!/bin/env sh
now=$(date +%s)
find upload/ -maxdepth 2 -type f -iname "*.[0-9]*" |
while read -r filename; do
	[ "$(( ${filename##*.} / 1000 - "${now}" ))" -lt 0 ] && rm -v "${filename}"
done
```

#### Delete file from server

Set `delete_tokens` array in [config.toml](./config.toml) to activate the [`DELETE`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods/DELETE) endpoint and secure it with one (or more) auth token(s).

```sh
$ curl -H "Authorization: <auth_token>" -X DELETE "<server_address>/file.txt"
```

> The `DELETE` endpoint will not be exposed and will return `404` error if `delete_tokens` are not set.

#### Override the filename when using `random_url`

The generation of a random filename can be overridden by sending a header called `filename`:

```sh
curl -F "file=@x.txt" -H "filename: <file_name>" "<server_address>"
```

### Server

To start the server:

```sh
$ rustypaste
```

If the configuration file is not found in the current directory, specify it via `CONFIG` environment variable:

```sh
$ CONFIG="$HOME/.rustypaste.toml" rustypaste
```

#### Authentication

To enable basic HTTP auth, set the `AUTH_TOKEN` environment variable (via `.env`):

```sh
$ echo "AUTH_TOKEN=$(openssl rand -base64 16)" > .env
$ rustypaste
```

There are 2 options for setting multiple auth tokens:

- Via the array field `[server].auth_tokens` in your `config.toml`.
- Or by writing a newline separated list to a file and passing its path to rustypaste via `AUTH_TOKENS_FILE` and `DELETE_TOKENS_FILE` respectively.

> If neither `AUTH_TOKEN`, `AUTH_TOKENS_FILE` nor `[server].auth_tokens` are set, the server will not require any authentication.
>
> Exception is the `DELETE` endpoint, which requires at least one token to be set. See [deleting files from server](#delete-file-from-server) for more information.

See [config.toml](./config.toml) for configuration options.

#### List endpoint

Set `expose_list` to true in [config.toml](./config.toml) to be able to retrieve a JSON formatted list of files in your uploads directory. This will not include oneshot files, oneshot URLs, or URLs.

```sh
$ curl "http://<server_address>/list"

[{"file_name":"accepted-cicada.txt","file_size":241,"expires_at_utc":null}]
```

This route will require an `AUTH_TOKEN` if one is set.

#### HTML Form

It is possible to use an HTML form for uploading files. To do so, you need to update two fields in your `config.toml`:

- Set the `[landing_page].content_type` to `text/html; charset=utf-8`.
- Update the `[landing_page].text` field with your HTML form or point `[landing_page].file` to your html file.

For an example, see [examples/html_form.toml](./examples/html_form.toml)

#### Docker

Following command can be used to run a container which is built from the [Dockerfile](./Dockerfile) in this repository:

```sh
$ docker run --rm -d \
  -v "$(pwd)/upload/":/app/upload \
  -v "$(pwd)/config.toml":/app/config.toml \
  --env-file "$(pwd)/.env" \
  -e "RUST_LOG=debug" \
  -p 8000:8000 \
  --name rustypaste \
  orhunp/rustypaste
```

- uploaded files go into `./upload` (on the host machine)
- set the `AUTH_TOKEN` via `-e` or `--env-file` to enable auth

You can build this image using `docker build -t rustypaste .` command.

If you want to run the image using [docker compose](https://docs.docker.com/compose/), simply run `docker-compose up -d`. (see [docker-compose.yml](./docker-compose.yml))

#### Nginx

Example server configuration with reverse proxy:

```nginx
server {
    listen 80;
    location / {
        proxy_pass                         http://localhost:8000/;
        proxy_set_header Host              $host;
        proxy_set_header X-Forwarded-For   $remote_addr;
        proxy_set_header X-Forwarded-Proto $scheme;
        add_header X-XSS-Protection        "1; mode=block";
        add_header X-Frame-Options         "sameorigin";
        add_header X-Content-Type-Options  "nosniff";
    }
}
```

If you get a `413 Request Entity Too Large` error during upload, set the max body size in `nginx.conf`:

```nginx
http {
    # ...
    client_max_body_size 100M;
}
```

### Third Party Clients

- [dobohdan/ferripaste](https://github.com/dbohdan/ferripaste) - Alternative rustypaste CLI client
- [rukh-debug/droidypaste](https://github.com/rukh-debug/droidypaste) - Android client built with React Native and Expo
- [rukh-debug/rustypaste-gui.sh](https://gist.github.com/rukh-debug/cc42900f86e39cacef6f7a6ba77ebf58) - Linux's Minimal GUI client powered by zenity

### Contributing

Pull requests are welcome!

Consider submitting your ideas via [issues](https://github.com/orhun/rustypaste/issues/new) first and check out the [existing issues](https://github.com/orhun/rustypaste/issues).

#### License

<sup>
All code is licensed under <a href="LICENSE">The MIT License</a>.
</sup>
