# Release maintainer guide

[한국어](./releasing.ko.md) | English

Use this guide to prepare and verify a codemap-search release. End-user installation is covered by [installation channels](./index.md). The [release workflow](../../../../.github/workflows/codemap-search-release.yml) defines the build and publishing jobs; a checked-in manifest or Git tag alone does not prove public availability.

## 1. Prepare the release

Work from the monorepo root with the intended release commit selected. Confirm:

- The version in `apps/codemap-search/Cargo.toml` matches the intended `codemap-vX.Y.Z` tag, and that tag/version is not already in use.
- The release commit contains the intended source, lockfile, documentation and installer.
- GitHub Actions has the required runners and secrets. crates.io publishing uses `CARGO_REGISTRY_TOKEN`; the macOS jobs also require the signing/notarization environment configured in the workflow. Keep credentials out of files and command arguments.

Validate packaging and compilation without uploading:

```sh
cargo publish --dry-run --manifest-path apps/codemap-search/Cargo.toml
```

A successful dry run does not verify credentials, upload, cross-platform execution or package-manager acceptance.

## 2. Trigger publishing and inspect both results

**Pushing a release tag starts external publication. A published crate version cannot be replaced with another upload.** After approving the release commit and version, push the intended `codemap-vX.Y.Z` tag through your release procedure. Do not reuse the historical first-release tag or development branch.

The workflow does not publish on branch pushes or pull requests:

1. `build` creates target-specific archives and `.sha256` files.
2. `publish-release` and `publish-crate` each wait for `build`, then publish independently.
3. Check both job results. GitHub release success does not establish crates.io success, or vice versa.

Linux/macOS build failures block publishing. Windows jobs are best-effort and may omit their files while other targets publish. Cross-building a target does not prove it runs on that hardware. Confirm the actual files for each advertised target using the [archive list](./index.md#release-archive-names).

`publish-crate` checks whether the exact manifest version is already published and skips it if found. After a failed upload it checks again to distinguish a completed upload from failure. Before retrying a failed release, inspect the registry and release assets; do not assume that neither destination changed.

## 3. Verify archives and the installer

For every advertised target, confirm that both the archive and its checksum are attached to the selected tag. Each `.sha256` file uses `<hash>  <basename>` and must match the downloaded archive.

The [installer](../../install.sh) reads public release files without an API token. Keep its asset names aligned with the workflow and verify the [tag-pinned installation path](./curl-installer.md#installation-directory-and-version). Without a tag, the repository's latest release may belong to another product.

On each platform actually tested, confirm `codemap-search --version` and an MCP connection to the intended repository. Record platforms that were built but not executed separately; see the [Docker guide](../../docker/README.md) for the Linux verification scope.

## 4. Update package-manager submissions

### Homebrew

The [local macOS formula](../../packaging/homebrew/codemap-search.rb) contains version `0.1.0` and zero-filled checksums; it is not installation-ready as checked in.

1. Set the intended version, both Darwin archive URLs and checksums from the matching release files.
2. Validate the formula in the appropriate Homebrew tap environment. Its checks exercise `--version` and `tokenize`; also verify real downloads and checksums.
3. Submit to `homebrew/homebrew-core` at `Formula/c/codemap-search.rb` and address its review. The checked-in formula uses prebuilt binaries; acceptance of that packaging approach is not guaranteed.
4. Confirm public acceptance before advertising `brew install codemap-search`.

This repository provides no Linux formula or separate tap. A local formula check is not proof of public acceptance.

### WinGet

The [manifest directory](../../packaging/winget/) contains version, default-locale and installer manifests, currently using version `0.1.0`, schema `1.12.0` and zero-filled installer checksums.

1. Update all three version values, x64/arm64 URLs and actual release checksums. Keep the identifier `com.livteam.codemap-search`.
2. From the monorepo root on Windows, validate:

```powershell
winget validate apps/codemap-search/packaging/winget
```

3. With local manifest installation enabled, verify installation and CLI startup:

```powershell
winget install --manifest apps/codemap-search/packaging/winget
codemap-search --version
```

4. Submit under `manifests/c/com/livteam/codemap-search/<version>/` in `microsoft/winget-pkgs` and confirm acceptance before advertising public installation.

Schema validation alone does not download an archive or verify its checksum. Do not bypass hash failures.

## Completion

Record the release commit/tag, both publishing results, actual archives and checksums, tested installation paths and package-manager acceptance separately. Report unavailable targets or pending submissions as such; do not infer them from another channel's success.

The [initial channel design](../release-distribution-strategy.md) and [first-release checklist](./first-release-checklist.md) are historical records, not current release instructions.
