# Install with Cargo

[한국어](./crates-io.ko.md) | English

Build and install codemap-search with Rust/Cargo and a native C toolchain for the bundled parsers. Cargo's default executable directory is `~/.cargo/bin`; keep it on `PATH`.

## Install and verify

```sh
cargo install codemap-search
codemap-search --version
```

The [crates.io package page](https://crates.io/crates/codemap-search) is the source for published versions. Its version data could not be confirmed during the 2026-09-11 documentation review. Choose a published version with Cargo's `--version` option when you need a specific release.

To install or build the local source tree, run these commands from the monorepo root:

```sh
cargo install --path apps/codemap-search
# Or build without installing:
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
# Default output: apps/codemap-search/target/release/codemap-search
```

If Cargo cannot find the package or version, check the registry page and network access, or build from the local source. Compiler/linker failures require checking the Rust and native C toolchains. If installation succeeds but the client cannot launch the binary, check its `PATH`.

## Maintainer publishing

The [release workflow](../../../../.github/workflows/codemap-search-release.yml) runs on `codemap-vX.Y.Z` tags. Both `publish-crate` and `publish-release` wait for `build` and then publish independently. A successful GitHub release does not establish that the crate was published.

1. Create a crates.io API token with permission to publish this crate and register it as the repository secret `CARGO_REGISTRY_TOKEN`.
2. Check that the tag version and `Cargo.toml` version match.
3. From the monorepo root, validate packaging and compilation without uploading:

```sh
   cargo publish --dry-run --manifest-path apps/codemap-search/Cargo.toml
   ```

4. Push the intended release tag to trigger publication. Publishing makes that crate version permanent; it cannot be replaced with another upload.
5. Check `publish-crate` and the registry's version listing. The job skips an already published version and rechecks the registry after a failed upload before reporting failure.

The dry run checks packaging and compilation; it does not verify the token or actual upload. Use [installation channels](./index.md) for prebuilt binaries.
