# Homebrew installation

[한국어](./homebrew.ko.md) | English

The repository contains a macOS formula for Apple Silicon and Intel Macs. It installs prebuilt release archives. Linux users can use [Cargo](./crates-io.md) or the [install script](./curl-installer.md).

## Availability and installation

Public availability in homebrew-core could not be confirmed during the 2026-09-11 documentation review. Check the [official formula directory](https://formulae.brew.sh/formula/codemap-search). Once the formula is available, install with Homebrew:

```sh
brew install codemap-search
```

Homebrew manages the installation location. Verify with `codemap-search --version` and make sure the client can find Homebrew's executable directory on `PATH`. If the formula is unavailable, use Cargo, the install script, or a matching archive from [GitHub Releases](https://github.com/buYoung/mcp/releases). For an exact release tag or custom installation directory, use the install script.

The local [formula](../../packaging/homebrew/codemap-search.rb) still specifies version `0.1.0` and zero-filled checksum placeholders. It is not ready for installation as checked in. This does not establish whether a separate public formula has been accepted.

## Maintainer submission

1. Choose the release and update the formula's version, URLs, and both checksums from the corresponding `.sha256` files.
2. Verify the formula with Homebrew in the appropriate tap environment. Its checks exercise `--version` and `tokenize`.
3. Submit the formula to `homebrew/homebrew-core` at `Formula/c/codemap-search.rb`, and address the maintainers' review before advertising `brew install` availability.

The packaged archives are `codemap-search-aarch64-apple-darwin.tar.gz` and `codemap-search-x86_64-apple-darwin.tar.gz`. This repository provides no Linux formula or separate tap. A local formula check does not prove public acceptance or that its download URLs and checksums work.

See [installation channels](./index.md) for alternatives.
