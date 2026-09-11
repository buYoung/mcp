# WinGet installation

[한국어](./winget.ko.md) | English

Use Windows Package Manager (WinGet) or install a release archive manually. The package identifier is `com.livteam.codemap-search`.

## Availability and installation

Public availability could not be confirmed during the 2026-09-11 documentation review. Check the [official WinGet repository](https://github.com/microsoft/winget-pkgs/tree/master/manifests/c/com/livteam/codemap-search). When the package is available:

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

## Local manifest installation

The [manifest directory](../../packaging/winget/) contains version, default-locale, and installer manifests for x64 and arm64. The checked-in version is `0.1.0`, the manifest schema is `1.12.0`, and `InstallerSha256` values remain zero-filled placeholders. Replace the URLs/version and checksums with values for an available release before installing.

From the monorepo root on Windows, with local manifest installation enabled in WinGet:

```powershell
winget install --manifest apps/codemap-search/packaging/winget
```

A placeholder checksum causes installation to fail. Schema validation alone does not download the archive or confirm its hash. For download errors, check the release files; for hash errors, compare the manifest with the release's `.sha256` file.

## Maintainer submission

1. Update all three manifests to the intended version and put real checksums in the installer manifest.
2. On Windows, validate from the monorepo root:

```powershell
   winget validate apps/codemap-search/packaging/winget
   ```

3. Verify installation and CLI startup using the local manifest.
4. Submit the manifests to `microsoft/winget-pkgs` under the package's version directory, following the existing `manifests/c/com/livteam/codemap-search/0.1.0/` layout with the intended version.
5. Confirm acceptance before advertising public WinGet installation.

See [installation channels](./index.md) for other options.
