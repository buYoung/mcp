# Homebrew installation

[한국어](./homebrew.ko.md) | English

The repository contains a macOS formula for Apple Silicon and Intel Macs. It installs prebuilt release archives. Linux users can use [Cargo](./crates-io.md) or the [install script](./curl-installer.md).

## Availability and installation

Check whether the formula is available in the [official formula directory](https://formulae.brew.sh/formula/codemap-search). If it is listed, install with Homebrew:

```sh
brew install codemap-search
```

Homebrew manages the installation location. Verify with `codemap-search --version` and make sure the client can find Homebrew's executable directory on `PATH`. If the formula is unavailable, use Cargo, the install script, or a matching archive from [GitHub Releases](https://github.com/buYoung/mcp/releases). For an exact release tag or custom installation directory, use the install script.

The local [formula](../../packaging/homebrew/codemap-search.rb) still specifies version `0.1.0` and zero-filled checksum placeholders. It is not ready for installation as checked in. This does not establish whether a separate public formula has been accepted.

See [installation channels](./index.md) for alternatives. Formula updates, validation and submission are covered in the [maintainer guide](./releasing.md#homebrew).
