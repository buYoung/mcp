# Docker verification

[한국어](./README.ko.md) | English

Build local x86_64 GNU and musl binaries and check whether they start and parse a small Rust file across Linux container images. This tool validates the local build; it does not download or certify a published release.

## Run

You need Bash, a running Docker engine with BuildKit local-output support, and network access for images and build dependencies. On an arm64 host, Docker must support `linux/amd64` emulation. `timeout` or `gtimeout` enables time limits.

Run from `apps/codemap-search`:

```bash
# From the apps/codemap-search directory:
bash docker/verify.sh

# Options:
bash docker/verify.sh --skip-gnu    # only build + test musl
bash docker/verify.sh --skip-musl   # only build + test gnu
bash docker/verify.sh --no-cleanup  # skip `docker image prune` at end
```

| Option | Effect |
|---|---|
| `--skip-gnu` | Build and check musl only |
| `--skip-musl` | Build and check GNU only |
| `--no-cleanup` | Skip the final `docker image prune -f` |

By default, the final cleanup removes dangling Docker images, including unrelated dangling images. Use `--no-cleanup` to keep them.

The script appends logs to `docker/verify-run.log` and writes binaries to `docker/out/`. Check the current run's start/completion markers and build status; the log can contain earlier results.

## Checks and images

Each successfully built binary runs `--version` and parses a generated `/tmp/a.rs` file in every image below. The script records exit codes and loader errors such as `GLIBC_x.xx not found` or `ld-linux` errors.

| Image | libc family |
|---|---|
| `ubuntu:20.04` | glibc |
| `ubuntu:22.04` | glibc |
| `ubuntu:24.04` | glibc |
| `debian:12` | glibc |
| `rockylinux:9` | glibc |
| `alpine:3.20` | musl |

[Dockerfile.build-gnu](./Dockerfile.build-gnu) builds on Ubuntu 24.04; [Dockerfile.build-musl](./Dockerfile.build-musl) uses `rust:alpine`. The GNU image differs from the release workflow's Ubuntu 22.04 build environment, so this check does not establish the released binary's minimum glibc version. musl avoids the glibc dependency.

## Interpret results

A successful pair of checks has this shape; the version shown is only an example:

```
  --version exit=0  output: codemap-search 0.1.0
  smoke    exit=0   output: ...
  STATUS: PASS
```

Loader failures include a diagnostic:

```
  STATUS: FAIL  detail: /lib/x86_64-linux-gnu/libc.so.6: version 'GLIBC_2.38' not found
```

Machine-readable lines start with `RESULT|`:

```
RESULT|ubuntu:20.04|gnu|FAIL|version_exit=1|smoke_exit=1|GLIBC_2.38 not found
RESULT|ubuntu:20.04|musl|PASS|version_exit=0|smoke_exit=0|
```

Read `PASS`/`FAIL` for each image and binary, together with the build summary. The script's final exit code does not summarize all failures. A missing matrix result after a failed or skipped build is not a pass.

With `timeout`/`gtimeout`, each build is limited to `BUILD_CAP_SECONDS=1200` (20 minutes), and the parse check to 30 seconds. Without either utility, those limits do not apply and the script warns. A failed or timed-out build skips that binary's matrix; the other build can still produce partial results.

## Verification limits

All builds and runs target `linux/amd64`. On Apple Silicon, execution uses emulation and may be slower or differ from native x86_64 behavior. Results do not verify arm64 binaries, native hardware behavior, performance, full indexing, or the MCP session. Keep the tested build and environment with any compatibility report.

See [installation channels](../docs/distribution/index.md) for release files.
