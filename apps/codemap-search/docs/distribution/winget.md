# WinGet installation

[한국어](./winget.ko.md) | English

Use Windows Package Manager (WinGet) or install a release archive manually. The package identifier is `com.livteam.codemap-search`.

## Availability and installation

Check package availability in the [official WinGet repository](https://github.com/microsoft/winget-pkgs/tree/master/manifests/c/com/livteam/codemap-search). If it is listed, install with:

```powershell
winget install com.livteam.codemap-search
```

WinGet manages the installation location and registers the portable command alias `codemap-search`. Run `codemap-search --version` in a new terminal and ensure the MCP client can find it on `PATH`.

If the package is unavailable, select a tag in [GitHub Releases](https://github.com/buYoung/mcp/releases), download the archive for your architecture and its checksum, verify the checksum, extract `codemap-search.exe`, and place it in a directory on `PATH`. Choosing the release tag selects the version.

| Architecture | Archive |
|---|---|
| x64 | `codemap-search-x86_64-pc-windows-msvc.zip` |
| arm64 | `codemap-search-aarch64-pc-windows-msvc.zip` |

The release workflow allows Windows build failures to omit their files without blocking other targets. Check that your chosen release includes the required archive. The arm64 target is cross-built on an x64 runner and is not run on arm64 hardware by this workflow. The [install script](./curl-installer.md) covers macOS and Linux only.

The checked-in [local manifests](../../packaging/winget/) contain placeholder checksums and cannot be installed as-is. Updating, validating and submitting them is covered in the [maintainer guide](./releasing.md#winget).

For download errors, check the selected release's files. For hash errors, compare against its `.sha256` file rather than bypassing verification. See [installation channels](./index.md) for other options.
