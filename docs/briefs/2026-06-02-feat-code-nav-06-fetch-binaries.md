# [feat] Download and verify zoekt/ctags binaries from a pinned GitHub release

## Work Type
feat

## Current State (As-Is)
- The shipped `apps/code-nav/src/startup/ensure-required-binaries.ts` resolves zoekt and ctags from PATH and `~/go/bin`, and errors with `go install` / `brew install` guidance when they are missing (DESIGN §5); it assumes the user pre-installs the binaries.
- There is no download, no integrity or authenticity verification, and no build provenance for the binaries.
- These binaries run with the user's privileges (zoekt-webserver is long-running, zoekt-index reads the whole working tree, ctags parses source files), so a tampered binary is arbitrary code execution as the user — verification is load-bearing, not cosmetic.
- The maintainer has created a public GitHub release repo (https://github.com/buYoung/zoetk-ctags-release) and intends to publish zoekt and ctags as release assets there for the MCP to download and verify.

## Desired Outcome (To-Be)
- A binary-acquisition layer downloads the pinned zoekt and ctags release assets from the maintainer's public GitHub repo over HTTPS, verifies them, caches them under restrictive permissions, re-verifies on every startup, and fails closed; `ResolvedBinaries` then point at the verified cached paths.
- The trust anchor is a digest (and/or signing key) pinned in the MCP itself, not a hash file fetched from the release.

## Scope
### In Scope
- A new binary-acquisition module: per-(os, arch) pinned release tag + asset name + expected SHA-256, HTTPS download, digest verification, cache under restrictive permissions, and re-verification of the cached file on every startup.
- Rework `ensure-required-binaries.ts` to source binary paths from the verified cache; the Universal Ctags variant check still runs after verification.
- Fail-closed errors for download failure, missing asset, and digest mismatch.
### Out of Scope
- [hard] fetching or trusting the expected hash from the release itself — the expected digest is pinned in the MCP; a release-hosted hash file is at most a secondary cross-check, never the trust anchor.
- [hard] silent fallback to a PATH / `~/go/bin` binary when verification fails — fail closed (decision 3).
- [deferred] building the binaries — that lives in the release repo's CI (a separate repo); this brief only consumes and verifies the artifacts. Building from pinned upstream source (zoekt commit / universal-ctags tag) is recommended so the pinned binary is trustworthy at first pin, but it is the maintainer's call.
- [deferred] cryptographic signatures (cosign/Sigstore) and build-provenance attestation — pinned-SHA-256 in MCP source is the confirmed v1 scheme and is sufficient against release tampering and account compromise; signatures are only a later operational upgrade (release rotation without an MCP redeploy + transparency-log auditability), not a v1 requirement.

## Constraints
- Pin the expected SHA-256 (and, if signatures are adopted, the signing identity / public key) in the MCP source or committed config; never read the expected digest from the downloaded release.
- HTTPS only, host-pinned to `github.com` and its release CDN `objects.githubusercontent.com` (release-asset downloads 302-redirect there); allow only that redirect target and reject redirects to any other host; never disable TLS certificate verification.
- Pin tag + asset name + digest as a single unit; never resolve `latest` — this also blocks downgrade to a known-vulnerable version.
- SHA-256 minimum (no SHA-1 / MD5).
- Re-verify the cached binary's digest on every startup before executing it, not only on first download.
- Cache directory and files are `0700` and user-owned; verify ownership; verify the exact file that will be executed (avoid TOCTOU — no verify-then-redownload, no verify-temp-execute-other).
- Distribution format: if release assets are archives (`.tar.gz` / `.zip`), verify the archive's digest before extraction and guard extraction against path traversal (zip-slip — reject any entry that resolves outside the cache directory); if assets are raw per-platform executables, state that and skip extraction. This brief assumes raw executables unless decided otherwise.
- Any dev / escape-hatch path override (a retained PATH/go-bin fallback or a `CODE_NAV_*_PATH`-style env var) is a verification-bypass surface: keep it off by default, gate it behind an explicit dev-only flag, and still digest-check the override target — never let an env var silently point execution at an unverified binary.
- Fail closed on any download or verification failure; never execute an unverified binary.

## Related Files / Entry Points
- `apps/code-nav/src/startup/ensure-required-binaries.ts` — rework to source paths from the verified cache instead of PATH/go-bin; keep the Universal Ctags variant check post-verification.
- `apps/code-nav/src/startup/binary-availability.ts` — the PATH / go-bin resolver becomes a dev-mode fallback or is removed.
- `apps/code-nav/src/config/defaults.ts` — add the pinned release repo (https://github.com/buYoung/zoetk-ctags-release), the pinned release tag, per-platform asset names, and expected SHA-256 digests.
- `apps/code-nav/DESIGN.md` — §5 (startup binary check + install guidance) must be rewritten from the PATH/install-guidance model to the download-verify model.
- `apps/code-nav/src/startup/binary-acquisition.ts` (proposed) — download, digest verify, cache with restrictive perms, re-verify.

## Side Effect Checkpoints
- [ ] DESIGN §5 is rewritten from "PATH check + go install/brew guidance" to the download-verify model.
- [ ] No code path executes a binary that failed verification (grep for the spawn/exec sites).
- [ ] The Universal Ctags variant check still runs, after verification.
- [ ] zoekt-webserver still binds only to `127.0.0.1` (unchanged by this work).

## Acceptance Criteria
- [ ] On a clean machine, first startup downloads the pinned assets over HTTPS, verifies SHA-256 against the MCP-pinned digest, caches under `0700`, and proceeds.
- [ ] A cached binary whose digest no longer matches is rejected on the next startup and the server fails closed without executing it.
- [ ] A digest mismatch on download fails closed with a clear error and no fallback to PATH.
- [ ] Re-verification runs on every startup, demonstrated by tampering with the cached file between two startups.

## Open Questions
- None — the v1 scheme is confirmed: pinned-SHA-256 in MCP source (signatures/attestation deferred as an optional later upgrade, not required), distribution via the maintainer's GitHub releases, and binary-origin risk accepted by self-management (building assets from pinned upstream source is recommended to keep that valid but is the maintainer's call). If signatures are ever adopted, the only hard add-on is pinning the cosign signer identity (OIDC identity + issuer), since verifying a Sigstore signature without asserting identity accepts any valid signature.
