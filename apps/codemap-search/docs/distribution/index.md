# Installation channels

[한국어](./index.ko.md) | English

Choose a source build or a prebuilt binary for your operating system. After installation, run `codemap-search --version` and follow the [MCP client setup](../../README.md#register-with-an-mcp-client).

| System | Source build | Prebuilt installation |
|---|---|---|
| Linux | [Cargo](./crates-io.md) | [Install script](./curl-installer.md) or GitHub release archive |
| macOS | [Cargo](./crates-io.md) | [Install script](./curl-installer.md), [Homebrew](./homebrew.md), or GitHub release archive |
| Windows | [Cargo](./crates-io.md) | [WinGet or manual archive installation](./winget.md) |

Cargo requires Rust and a native C toolchain. The macOS/Linux script requires standard POSIX tools, `curl` or `wget`, `tar`, and a SHA-256 utility; it installs to `~/.local/bin` by default. Homebrew and WinGet manage their own installation directories. Each guide covers version selection, `PATH`, troubleshooting, and maintainer publishing.

## Channel availability

During the 2026-09-11 documentation review, the official pages could not be used to confirm published versions or package acceptance. The commands in these guides depend on the selected release or package being available. Check the [crates.io package](https://crates.io/crates/codemap-search), [GitHub Releases](https://github.com/buYoung/mcp/releases), [Homebrew formula directory](https://formulae.brew.sh/formula/codemap-search), or [WinGet repository](https://github.com/microsoft/winget-pkgs/tree/master/manifests/c/com/livteam/codemap-search) before choosing a public channel.

The checked-in Homebrew formula and WinGet manifests still contain checksum placeholders. They require release metadata updates before local installation. If public installation is unavailable, use the [local source-build commands](./crates-io.md#install-and-verify).

## Release archive names

These are the targets configured in the repository's [release workflow](../../../../.github/workflows/codemap-search-release.yml). Confirm the files present in the specific release you select.

| System | Architecture | Archive |
|---|---|---|
| Linux | x86_64, default musl | `codemap-search-x86_64-unknown-linux-musl.tar.gz` |
| Linux | x86_64, GNU | `codemap-search-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | arm64, musl | `codemap-search-aarch64-unknown-linux-musl.tar.gz` |
| macOS | Apple Silicon | `codemap-search-aarch64-apple-darwin.tar.gz` |
| macOS | Intel | `codemap-search-x86_64-apple-darwin.tar.gz` |
| Windows | x64 | `codemap-search-x86_64-pc-windows-msvc.zip` |
| Windows | arm64 | `codemap-search-aarch64-pc-windows-msvc.zip` |

Linux uses musl by default and needs no glibc. GNU is an explicit x86_64 option; Linux arm64 GNU is not configured. Windows build failures can omit their archives without blocking the other release targets. Linux arm64 is cross-built; Windows arm64 is cross-built without native execution in this workflow. A built archive alone does not establish runtime compatibility. The [Docker guide](../../docker/README.md) explains the Linux checks and their limits.
