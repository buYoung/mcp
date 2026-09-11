# Install script (`install.sh`)

[한국어](./curl-installer.ko.md) | English

Install a prebuilt codemap-search binary on macOS or Linux. The script selects an archive for your operating system and architecture, verifies its `.sha256` checksum before extraction, and installs to `~/.local/bin`. For Windows, see [WinGet and manual installation](./winget.md).

## Install

You need a POSIX shell, standard system utilities, `curl` or `wget`, `tar`, and `sha256sum` or `shasum`. No additional runtime is required. The one-line command below uses `curl`:

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
```

After installation, run `codemap-search --version`. If the command is not found, add the installed directory to `PATH`. The script prints the required `export PATH` line; add it to `~/.zshrc` or `~/.bashrc` to keep it across sessions.

The selected release must contain the archive and its `.sha256` file. Live download availability was not confirmed during the 2026-09-11 documentation review; check [GitHub Releases](https://github.com/buYoung/mcp/releases) for the tag and files you intend to use.

## Installation directory and version

To change the directory, pass `INSTALL_DIR` to the shell running the script. The directory must be writable; the default user directory needs no `sudo`.

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | INSTALL_DIR=/usr/local/bin sh
```

To select a release tag:

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh -s -- --version codemap-v0.1.6
```

`codemap-v0.1.6` is a version-selection example; use a tag with the required files.

| Option | Meaning |
|---|---|
| `INSTALL_DIR` | Installation directory; default `$HOME/.local/bin` |
| `--version` / `CODEMAP_VERSION` | Exact release tag; `--version` takes precedence |
| `CODEMAP_LINUX_LIBC` | `musl` by default; `gnu` for Linux x86_64 only |
| `--print-target` | Show the selected target, download URLs, and installation path without downloading |

Without a tag, the script uses `releases/latest/download/<asset>`. This points to the repository's latest release, which may belong to another product. Pin a tag for repeatable installation. Installer variables do not configure the running MCP server.

## Supported systems

| System | Architecture | Archive |
|---|---|---|
| macOS | arm64 / aarch64 | `codemap-search-aarch64-apple-darwin.tar.gz` |
| macOS | x86_64 / amd64 | `codemap-search-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64 / amd64, default musl | `codemap-search-x86_64-unknown-linux-musl.tar.gz` |
| Linux | x86_64 / amd64, `CODEMAP_LINUX_LIBC=gnu` | `codemap-search-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | arm64 / aarch64, musl only | `codemap-search-aarch64-unknown-linux-musl.tar.gz` |

The musl build has no glibc dependency. Set `CODEMAP_LINUX_LIBC=gnu` on `sh` in the same way as `INSTALL_DIR` to select the x86_64 GNU build. The script rejects Linux arm64 with `gnu`, unsupported architectures, and unsupported operating systems.

## Troubleshooting

- Download error: check the release tag, archive name, checksum file, and network connection. A missing file stops installation.
- Checksum mismatch: installation stops before extraction and does not create the installation directory. Download again and compare with the release's checksum; do not skip verification.
- Write error: choose a writable `INSTALL_DIR`.
- Command not found: check the client's `PATH`, as well as the terminal's.

## Maintainer publishing

The script is at [install.sh](../../install.sh). It reads public release files and needs no API token. Keep archive names and checksum files aligned with the [release workflow](../../../../.github/workflows/codemap-search-release.yml). Each `.sha256` file must contain `<hash>  <basename>`. Publish both the archive and checksum for every supported target.

See [installation channels](./index.md) for source builds and other package managers.
