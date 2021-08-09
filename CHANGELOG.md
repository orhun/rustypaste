# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2021-08-10
### Fixed
- Switch to [upload-release-action](https://github.com/svenstaro/upload-release-action) for uploading releases

## [0.3.0] - 2021-08-09
### Added
- Support overriding MIME types (config: `mime_override`)
- Support blacklisting MIME types (config: `mime_blacklist`)

## [0.2.0] - 2021-08-04
### Added
- Support shortening URLs (via `url=` form parameter)
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
