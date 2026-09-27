# Install with Cargo

[한국어](./crates-io.ko.md) | English

Build and install codemap-search with Rust/Cargo and a native C toolchain for the bundled parsers. Cargo's default executable directory is `~/.cargo/bin`; keep it on `PATH`.

## Install and verify

```sh
cargo install codemap-search
codemap-search --version
```

Check published versions on the [crates.io package page](https://crates.io/crates/codemap-search). For a specific release, select a published version with Cargo's `--version` option. If it is unavailable, build from the source checkout below.

To install or build the local source tree, run these commands from the monorepo root:

```sh
cargo install --path apps/codemap-search
# Or build without installing:
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
# Default output: apps/codemap-search/target/release/codemap-search
```

If Cargo cannot find the package or version, check the registry page and network access, or build from the local source. Compiler/linker failures require checking the Rust and native C toolchains. If installation succeeds but the client cannot launch the binary, check its `PATH`.

For prebuilt binaries, see [installation channels](./index.md). Release preparation and publishing belong to the [maintainer guide](./releasing.md).
