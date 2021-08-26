<a href="https://github.com/orhun/rustypaste"><img src="img/rustypaste_logo.png" width="500"></a>

**Rustypaste** is a minimal file upload/pastebin service.

```sh
$ echo "some text" > awesome.txt

$ curl -F "file=@awesome.txt" https://paste.site.com
https://paste.site.com/safe-toad.txt

$ curl https://paste.site.com/safe-toad.txt
some text
```

## Features

- File upload & URL shortening
  - supports basic HTTP authentication
  - random file names (optional)
    - pet name (e.g. `capital-mosquito.txt`)
    - alphanumeric string (e.g. `yB84D2Dv.txt`)
  - one shot links (file _disappears_ after accessed)
  - guesses MIME types
    - supports overriding and blacklisting
- Single binary
  - [binary releases](https://github.com/orhun/rustypaste/releases)
- Easy to deploy
  - [docker images](https://hub.docker.com/r/orhunp/rustypaste)
- No database
  - filesystem is used
- Self-hosted
  - _centralization is bad!_
- Written in Rust
  - _blazingly fast!_

## Usage

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

#### One Shot

```sh
curl -F "oneshot=@x.txt" "<server_address>"
```

#### URL Shortening

```sh
curl -F "url=https://example.com/some/long/url" "<server_address>"
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

### Roadmap

- Support setting an expiry date for uploads
- Write a CLI tool in Rust

### Contributing

Pull requests are welcome!

Consider submitting your ideas via issues first. Also, see the [roadmap](#roadmap) and/or run the following command to see what is needed to be done:

```sh
$ grep -nr "TODO:" src/
```

#### License

<sup>
All code is licensed under <a href="LICENSE">The MIT License</a>.
</sup>
