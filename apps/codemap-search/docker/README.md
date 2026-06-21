# codemap-search Docker Verification Harness

This directory contains a reusable harness to verify that the
`codemap-search` release binary actually runs across common Linux distributions.

## What it verifies

1. **gnu build** (`x86_64-unknown-linux-gnu`) — mirrors the GitHub release
   workflow (runs on `ubuntu-latest` = Ubuntu 24.04, glibc 2.39). Confirms
   the minimum glibc version required by the released binary.
2. **musl build** (`x86_64-unknown-linux-musl`) — fully static binary with no
   glibc dependency. Expected to run on all Linux distros including Alpine.

For each binary and each distro, the harness checks:
- **`--version` exit code** — confirms the binary loads and runs at all.
- **Smoke test** (`tokenize helloWorld`) — exercises the CLI path without
  requiring any index files or MCP stdio loop. Exits immediately.
- **Loader errors** — captures `GLIBC_x.xx not found` / `ld-linux` errors.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile.build-gnu` | Builds `x86_64-unknown-linux-gnu` binary on Ubuntu 24.04 + rustup |
| `Dockerfile.build-musl` | Builds `x86_64-unknown-linux-musl` static binary on rust:alpine |
| `verify.sh` | Orchestrates builds + runs distro matrix; writes `verify-run.log` |
| `.dockerignore` | Excludes `target/`, docs, fixtures from build context |
| `README.md` | This file |

## Distro matrix

| Image | glibc / libc |
|-------|-------------|
| `ubuntu:20.04` | glibc 2.31 |
| `ubuntu:22.04` | glibc 2.35 |
| `ubuntu:24.04` | glibc 2.39 |
| `debian:12` | glibc 2.36 |
| `rockylinux:9` | glibc 2.34 |
| `alpine:3.20` | musl 1.2.x (no glibc) |

## How to run

```bash
# From the apps/codemap-search directory:
bash docker/verify.sh

# Options:
bash docker/verify.sh --skip-gnu    # only build + test musl
bash docker/verify.sh --skip-musl   # only build + test gnu
bash docker/verify.sh --no-cleanup  # skip `docker image prune` at end
```

The script writes a log to `docker/verify-run.log` and extracted binaries to
`docker/out/`.

## How to read results

Each distro gets two lines in the log:
```
  --version exit=0  output: codemap-search 0.1.0
  smoke    exit=0   output: ...
  STATUS: PASS
```

or on failure:
```
  STATUS: FAIL  detail: /lib/x86_64-linux-gnu/libc.so.6: version 'GLIBC_2.38' not found
```

Machine-readable result lines start with `RESULT|`:
```
RESULT|ubuntu:20.04|gnu|FAIL|version_exit=1|smoke_exit=1|GLIBC_2.38 not found
RESULT|ubuntu:20.04|musl|PASS|version_exit=0|smoke_exit=0|
```

## ARM64 host / x86\_64 emulation caveat

This harness is designed for Apple Silicon (arm64) hosts where Docker uses QEMU
to emulate x86\_64. Both Dockerfiles are built with `--platform linux/amd64`,
producing real x86\_64 ELF binaries that match the GitHub release artifacts.
Emulated builds are significantly slower (budget 10-20 minutes per build for the
first run; subsequent runs use Docker layer cache).

The emulation caveat: while the binaries are real x86\_64 ELFs, they execute
under QEMU on the host. Performance characteristics differ from native x86\_64
hardware, but correctness (loader behaviour, glibc symbol resolution) is
accurately replicated.

## Time budget

Each build is capped at 20 minutes (`BUILD_CAP_SECONDS=1200` in `verify.sh`).
If a build times out, its result is recorded as a cap-hit (not a work failure)
and the harness proceeds with whatever binaries were produced. Partial results
(gnu-only or musl-only) are valid and reported.
