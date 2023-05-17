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

The public instance is available at [https://rustypaste.shuttleapp.rs](https://rustypaste.shuttleapp.rs) 🚀

## Features

- File upload & URL shortening & upload from URL
  - supports basic HTTP authentication
  - random file names (optional)
    - pet name (e.g. `capital-mosquito.txt`)
    - alphanumeric string (e.g. `yB84D2Dv.txt`)
  - supports expiring links
    - auto-expiration of files (optional)
    - auto-deletion of expired files (optional)
  - supports one shot links (can only be viewed once)
  - guesses MIME types
    - supports overriding and blacklisting
    - supports forcing to download via `?download=true`
  - no duplicate uploads (optional)
- Single binary
  - [binary releases](https://github.com/orhun/rustypaste/releases)
- Simple configuration
  - supports hot reloading
- Easy to deploy
  - [docker images](https://hub.docker.com/r/orhunp/rustypaste)
- No database
  - filesystem is used
- Self-hosted
  - _centralization is bad!_
- Written in Rust
  - _blazingly fast!_

## Installation

### From crates.io

```sh
cargo install rustypaste
```

### Arch Linux

```sh
pacman -S rustypaste
```

### Alpine Linux

`rustypaste` is available for [Alpine Edge](https://pkgs.alpinelinux.org/packages?name=rustypaste&branch=edge). It can be installed via [apk](https://wiki.alpinelinux.org/wiki/Alpine_Package_Keeper) after enabling the [testing repository](https://wiki.alpinelinux.org/wiki/Repositories).

```sh
apk add rustypaste
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
cargo build --release --features openssl
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

(supported units: `ns`, `us`, `ms`, `sec`, `min`, `hours`, `days`, `weeks`, `months`, `years`)

#### One shot

```sh
$ curl -F "oneshot=@x.txt" "<server_address>"
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

Configure `delete_expired_files` to set an interval for deleting the expired files automatically.

On the other hand, following script can be used as [cron](https://en.wikipedia.org/wiki/Cron) for cleaning up the expired files manually:

```sh
#!/bin/env sh
now=$(date +%s)
find upload/ -maxdepth 2 -type f -iname "*.[0-9]*" |
while read -r filename; do
	[ "$(( ${filename##*.} / 1000 - "${now}" ))" -lt 0 ] && rm -v "${filename}"
done
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

To enable basic HTTP auth, set the `AUTH_TOKEN` environment variable (via `.env`):

```sh
$ echo "AUTH_TOKEN=$(openssl rand -base64 16)" > .env
$ rustypaste
```

See [config.toml](./config.toml) for configuration options.

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

### Contributing

Pull requests are welcome!

Consider submitting your ideas via [issues](https://github.com/orhun/rustypaste/issues/new) first and check out the [existing issues](https://github.com/orhun/rustypaste/issues).

#### License

<sup>
All code is licensed under <a href="LICENSE">The MIT License</a>.
</sup>
