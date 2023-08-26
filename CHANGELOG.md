# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.0] - 2023-08-26

### Added

- Support handling spaces in filenames (#107)

Now you can replace the whitespaces with either underscore or encoded space (`%20`) character in the filenames.

For example:

```toml
[server]
handle_spaces = "replace"
```

```sh
$ curl -F "file=@test file.txt" <server_address>

<server_address>/test_file.txt
```

Or you can use encoded spaces:

```toml
[server]
handle_spaces = "encode"
```

```sh
$ curl -F "file=@test file.txt" <server_address>

<server_address>/test%20file.txt
```

Please note that `random_url` should not be configured to use the original file names.

### Changed

- Improve random_url config handling (#122)

`[paste].random_url.enabled` is deprecated. You can now disable random URLs by commenting out `[paste].random_url`.

```toml
# enabled
random_url = { type = "petname", words = 2, separator = "-" }

# disabled
# random_url = { type = "petname", words = 2, separator = "-" }
```

- Replace unmaintained actions (#116)
- Bump Shuttle to `0.24.0`
- Bump dependencies

### Fixed

- Don't log invalid token in release builds (#112)

Before, invalid tokens were logged as follows:

```
[2023-08-13T19:24:30Z WARN  rustypaste::auth] authorization failure for a.b.c.d (header: invalid_token)
```

Now, we print the token only in debug mode. In release mode, the log entry will look like this:

```
[2023-08-13T19:24:30Z WARN  rustypaste::auth] authorization failure for a.b.c.d
```

## [0.12.1] - 2023-08-11

### Fixed

- Do not list expired files (#109)

## [0.12.0] - 2023-08-07

### Added

- Add an endpoint for retrieving a list of files (#94)

Set the `expose_list` option to `true` in the configuration file for enabling this feature. It is disabled as default.

```toml
[server]
expose_list = true
```

Then you can receive the list of files as JSON via `/list` endpoint:

```sh
$ curl "http://<server_address>/list" | jq .

[
  {
    "file_name": "accepted-cicada.txt",
    "file_size": 241,
    "expires_at_utc": null
  },
  {
    "file_name": "evolving-ferret.txt",
    "file_size": 111,
    "expires_at_utc": "2023-08-07 10:51:14"
  }
]
```

- Support multiple auth tokens (#84)

`auth_token` option is now deprecated and replaced with `auth_tokens` which supports an array of authentication tokens. For example:

```toml
[server]
auth_tokens = [
  "super_secret_token1",
  "super_secret_token2",
]
```

- Add new line character to most prominent messages (#97)

This is a follow-up to #72 for making the terminal output better:

```sh
$ curl http://localhost:8000/sweeping-tahr
unauthorized
```

### Changed

- Bump Shuttle to `0.23.0`
- Bump dependencies

### Fixed

- Deploy the Shuttle service when a new tag is created

## [0.11.1] - 2023-07-01

This is a hotfix release for supporting the use of deprecated `[server].landing_page*` fields.

### Fixed

- Allow using deprecated landing page fields

## [0.11.0] - 2023-07-01

### Added

- Add a new section for the landing page
  - Also, support a file for the landing page (#64)

Migration path:

Old:

```toml
[server]
landing_page = "Landing page text."
landing_page_file = "index.html"
landing_page_content_type = "text/html; charset=utf-8"
```

New:

```toml
[landing_page]
text = "Landing page text."
file = "index.html"
content_type = "text/html; charset=utf-8"
```

The configuration is backwards compatible but we recommend using the new `landing_page` section as shown above since the other fields are now deprecated.

- Add random suffix mode (#69)
  - Support appending a random suffix to the filename before the extension. For example, `foo.tar.gz` will result in `foo.eu7f92x1.tar.gz`

To enable, set `suffix_mode` to `true`:

```toml
[paste]
random_url = { enabled = true, type = "alphanumeric", length = 6, suffix_mode = true }
```

- Honor X-Forward-\* headers (`X-Forwarded-For` / `X-Forwarded-Host` / `X-Forwarded-Proto`) (#61)

  - This would be really useful to have for setups where the service is running behind a reverse-proxy or gateway and the possibility to adjust the logging output based on their availability, to have the real IP addresses of the clients available in the log.

- Add new line character to the 404 message (#72)

Terminal output will look better when the file is not found:

```sh
$ curl http://localhost:8000/sweeping-tahr
file is not found or expired :(
```

- Add editorconfig for correctly formatting the test fixture files
- Add pull request template

### Changed

- Bump Shuttle to `0.20.0`
- List all the supported units in the documentation (#63)
- Note that the Alpine Linux package is moved to the community

  - <https://pkgs.alpinelinux.org/packages?name=rustypaste>

- Bump dependencies

### Fixed

- Use the static folder for the Shuttle config (#70)
  - There was a regression in the previous release that has caused the static folder to be not present in Shuttle deployments. This shouldn't be an issue anymore and the deployment should be live.
  - Also, it is now possible to trigger a deployment manually via GitHub Actions.

Thanks to [@tessus](https://github.com/tessus) for his contributions to this release!

## [0.10.1] - 2023-06-05

### Added

- Add a middleware for checking the content length
  - Before, the upload size was checked after full upload which was clearly wrong.
  - With this change, total amount of bytes to upload is checked via [`Content-Length`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Length) header before the upload.

### Changed

- Bump Shuttle to `0.18.0`
- Bump hotwatch to 0.5.0
  - Fixes [`RUSTSEC-2020-0016`](https://rustsec.org/advisories/RUSTSEC-2020-0016.html)

### Fixed

- Do not drop the config watcher
  - Since `0.9.0`, the configuration watcher was dropped early which caused for it to not work and resulted in mysterious spikes in CPU usage.
  - With this version, this issue is fixed.

## [0.10.0] - 2023-05-31

### Added

- Support one shot URLs

With using the `oneshot_url` multipart field, you can now shorten an URL and make it disappear after viewed once:

```sh
curl -F "oneshot_url=https://example.com" "<server_address>"
```

- Allow configuring the content type for the landing page

`landing_page_content_type` is added as a configuration option for setting the [`Content-Type`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type) header:

```toml
[server]
landing_page = ""
landing_page_content_type = "text/plain; charset=utf-8"
```

- Add information/example about using HTML forms

With utilizing the newly added option for the content type, you can now use HTML forms for the landing page:

```toml
[server]
landing_page = "<html>"
landing_page_content_type = "text/html; charset=utf-8"
```

There is an example added to the repository: [html_form.toml](https://github.com/orhun/rustypaste/blob/1a8958966972f2afb04a12cb2f5537a1d971561c/examples/html_form.toml)

Also, there is an ongoing discussion about refactoring the usage of landing page fields in the configuration file. See [#52](https://github.com/orhun/rustypaste/issues/52)

- An informative log message is added for showing the server address at startup

## [0.9.1] - 2023-05-24

### Changed

- Bump Shuttle to `0.17.0`
- Tweak public instance settings
  - Increase the default expiry time to 24 hours
  - Increase the max content length to 20MB
- Bump dependencies

## [0.9.0] - 2023-05-17

The public instance is now available at [https://rustypaste.shuttleapp.rs](https://rustypaste.shuttleapp.rs) 🚀

Read the blog post about `rustypaste` and Shuttle deployments: [https://blog.orhun.dev/blazingly-fast-file-sharing](https://blog.orhun.dev/blazingly-fast-file-sharing)

### Added

- Deploy on Shuttle.rs
- Support setting a default expiry time

You can now specify a expiry time for uploaded files. For example, if you want all the files to expire after one hour:

```toml
[paste]
default_expiry = "1h"
```

- Support overriding the server URL

If you are using `rustypaste` with a redirect or reverse proxy, it is now possible to set a different URL for the returned results:

```toml
[server]
url = "https://rustypaste.shuttleapp.rs"
```

- Add instructions for installing on Alpine Linux

`rustypaste` is now available in [testing](https://pkgs.alpinelinux.org/packages?name=rustypaste&branch=edge) repositories.

- Add new crate features

  - `shuttle`: enable an entry point for deploying on Shuttle
  - `openssl`: use distro OpenSSL (binary size is reduced ~20% in release mode)
  - `rustls`: use [rustls](https://github.com/rustls/rustls) (enabled as default)

### Changed

- Make the default landing page fancier
- Generate SBOM attestation for the Docker image

### Updated

- Bump dependencies
- Update the funding options
  - Consider donating if you liked `rustypaste`: [https://donate.orhun.dev](https://donate.orhun.dev) 💖

## [0.8.4] - 2023-01-31

### Added

- Allow downloading files via `?download=true` parameter
  - If you specify this for a file (e.g. `<server_address>/file?download=true`), `rustypaste` will override the MIME type to `application/octet-stream` and this will force your browser to download the file.
  - This is useful when e.g. you want to be able to share the link to a file that would play in the browser (like `.mp4`) but also share a link that will auto-download as well.

## [0.8.3] - 2023-01-30

### Updated

- Bump dependencies
- Switch to [Rust](https://hub.docker.com/_/rust) image for the Dockerfile
- Remove unused `clap` dependency

## [0.8.2] - 2022-10-04

### Updated

- Don't expose version endpoint in default config
  - Set `expose_version` to `false` in the configuration file

## [0.8.1] - 2022-10-04

### Added

- Add `<server_address>/version` endpoint for retrieving the server version

```toml
[server]
expose_version=true
```

If `expose_version` entry is not present in the configuration file, `/version` is not exposed. It is recommended to use this feature with authorization enabled.

### Fixed

- Replace unmaintained `dotenv` crate with `dotenvy`
  - Fixes [RUSTSEC-2021-0141](https://rustsec.org/advisories/RUSTSEC-2021-0141.html)

## [0.8.0] - 2022-10-03

### Added

- Support adding a landing page

You can now specify a landing page text in the configuration file as follows:

```toml
[server]
landing_page = """
boo 👻
======
welcome!
"""
```

If the landing page entry is not present in the configuration file, visiting the index page will redirect to the repository.

### Updated

- Do not check for duplicate files by default
  - Set `duplicate_files` to `true` in the configuration file
  - It is an expensive operation to do on slower hardware and can take an unreasonable amount of time for bigger files
- Enable [GitHub Sponsors](https://github.com/sponsors/orhun) for funding
  - Consider supporting me for my open-source work 💖

## [0.7.1] - 2022-05-21

### Added

- Aggressively test everything
  - Add the missing unit tests for the server endpoints (code coverage is increased to 84%)
  - Create a custom testing framework (written in Bash) for adding [test fixtures](https://github.com/orhun/rustypaste/tree/master/fixtures)

## [0.7.0] - 2022-03-26

### Added

- Support auto-deletion of expired files

`rustypaste` can now delete the expired files by itself. To enable this feature, add the following line to the `[paste]` section in the configuration file:

```toml
# expired files will be cleaned up hourly
delete_expired_files = { enabled = true, interval = "1h" }
```

For users who want to have this feature disabled, there is an alternative [shell script](README.md#cleaning-up-expired-files) recommended in the documentation.

- Add systemd service files
  - [systemd files](./extra/systemd/) have been added to serve files from `/var/lib/rustypaste`, create `rustypaste` user automatically via `systemd-sysusers` and configure `AUTH_TOKEN` via `rustypaste.env`.
  - For the installation and usage, see the Arch Linux [PKGBUILD](https://github.com/archlinux/svntogit-community/blob/packages/rustypaste/trunk/PKGBUILD).

### Updated

- Upgrade Actix dependencies
  - `actix-web` is updated to [`4.0.*`](https://github.com/actix/actix-web/blob/master/actix-web/CHANGES.md#401---2022-02-25)
- Strip the binaries during automated builds
  - Size of the Docker image is reduced by ~20%

### Fixed

- Prevent invalid attempts of serving directories
  - This fixes an issue where requesting a directory was possible via e.g. `curl --path-as-is 0.0.0.0:8080/.`
  - This issue had no security impact (path traversal wasn't possible) since internal server error was returned.

## [0.6.5] - 2022-03-13

### Added

- Add instructions for installing [rustypaste](https://archlinux.org/packages/community/x86_64/rustypaste/) on Arch Linux
  - `pacman -S rustypaste` 🎉

### Fixed

- Fix a bug where the use of `CONFIG` environment variable causes a conflict between the configuration file path and `[config]` section

## [0.6.4] - 2022-03-11

### Added

- Support setting the refresh rate for hot-reloading the configuration file.

```toml
[config]
refresh_rate="1s"
```

- Support setting the timeout for HTTP requests.

```toml
[server]
timeout="30s"
```

### Security

- Bump [regex crate](https://github.com/rust-lang/regex) to **1.5.5**
  - Fixes [CVE-2022-24713](https://github.com/advisories/GHSA-m5pq-gvj9-9vr8)

## [0.6.3] - 2022-02-24

### Added

- Support setting the authentication token in the configuration file.
  - This is an alternative (but not recommended) way of setting up authentication when the use of `AUTH_TOKEN` environment variable is not applicable.

```toml
[server]
auth_token="hunter2"
```

## [0.6.2] - 2021-12-05

### Updated

- Improve the concurrency
  - Shrink the scope of non-suspendable types (`#[must_not_suspend]`) for dropping them before reaching a suspend point (`.await` call). This avoids possible deadlocks, delays, and situations where `Future`s not implementing `Send`.
  - Reference: https://rust-lang.github.io/rfcs/3014-must-not-suspend-lint.html

## [0.6.1] - 2021-11-16

### Fixed

- Gracefully handle the hot-reloading errors.
  - Errors that may occur while locking the [Mutex](https://doc.rust-lang.org/std/sync/struct.Mutex.html) are handled properly hence a single configuration change cannot take down the whole service due to [poisoning](https://doc.rust-lang.org/std/sync/struct.Mutex.html#poisoning).

## [0.6.0] - 2021-11-07

### Added

- Support pasting files from remote URLs (via `remote=` form field)

  - `{server.max_content_length}` is used for download limit
  - See [README.md#paste-file-from-remote-url](https://github.com/orhun/rustypaste#paste-file-from-remote-url)

- Hot reload configuration file to apply configuration changes instantly without restarting the server

### Changed

- Library: Switch to Rust 2021 edition

### Security

- Prevent serving an already expired file

In the previous versions, it was possible to view an expired file by using the correct extension (timestamp). e.g. `paste.com/expired_file.txt.1630094518049` will serve the file normally although `paste.com/expired_file.txt` says that it is expired. This version fixes this vulnerability by regex-checking the requested file's extension.

reference: [f078a9afa74f8608ee3f2a6e705159df15915c78](https://github.com/orhun/rustypaste/commit/f078a9afa74f8608ee3f2a6e705159df15915c78)

## [0.5.0] - 2021-10-12

### Added

- Added an entry in the configuration file to disable "duplicate uploads":

```toml
[paste]
# default: true
duplicate_files = false
```

Under the hood, it checks the SHA256 digest of the uploaded files.

## [0.4.1] - 2021-09-19

### Changed

- Update README.md:
  - Mention the new standalone tool: [rustypaste-cli](https://github.com/orhun/rustypaste-cli)
  - Add [installation](https://github.com/orhun/rustypaste#installation) section.

## [0.4.0] - 2021-08-27

### Added

- Support [expiring links](README.md#expiration) (via `expire:` header)
  - Timestamps are used as extension for expiring files
  - Expired files can be cleaned up with [this command](README.md#cleaning-up-expired-files)
- Support [one shot links](README.md#one-shot) (via `oneshot=` form field)
  - `{server.upload_path}/oneshot` is used for storage

## [0.3.1] - 2021-08-10

### Fixed

- Switch to [upload-release-action](https://github.com/svenstaro/upload-release-action) for uploading releases

## [0.3.0] - 2021-08-09

### Added

- Support overriding MIME types (config: `mime_override`)
- Support blacklisting MIME types (config: `mime_blacklist`)

## [0.2.0] - 2021-08-04

### Added

- Support shortening URLs (via `url=` form field)
  - `{server.upload_path}/url` is used for storage

## [0.1.3] - 2021-07-28

### Fixed

- Prevent sending empty file name and zero bytes
- Prevent path traversal on upload directory ([#2](https://github.com/orhun/rustypaste/issues/2))
- Check the content length while reading bytes for preventing OOM ([#1](https://github.com/orhun/rustypaste/issues/1))

## [0.1.2] - 2021-07-27

### Changed

- Update Continuous Deployment workflow to publish Docker images

## [0.1.1] - 2021-07-27

Initial release.
