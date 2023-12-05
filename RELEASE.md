# Creating a Release

1. Update the version in `Cargo.toml` accordingly to [Semantic Versioning](https://semver.org/).
2. Run `cargo build` to update the version in `Cargo.lock`.
3. Update [CHANGELOG.md](./CHANGELOG.md) accordingly.
4. Commit the changes. (see [this](https://github.com/orhun/rustypaste/commit/79662d64abfe497baa5e9690c0f56ca183391809) example commit)
5. Create a release [on GitHub](https://github.com/orhun/rustypaste/releases/new) with the same entries in `CHANGELOG.md`.
