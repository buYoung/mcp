#!/usr/bin/env bash
# verify.sh — codemap-search Linux distribution verification harness
#
# Builds the gnu (x86_64-unknown-linux-gnu) and musl (x86_64-unknown-linux-musl)
# release binaries under --platform linux/amd64 emulation, then runs each binary
# against a matrix of Linux distributions to confirm:
#   (a) the binary loads (no "GLIBC_x.xx not found" / loader error)
#   (b) --version exits 0
#   (c) a real smoke test (tokenize) exits 0
#
# ARM64-HOST / X86_64-EMULATION CAVEAT:
# On Apple Silicon (arm64) Docker runs x86_64 binaries via QEMU emulation.
# The gnu/musl binaries ARE real x86_64 ELFs (matching the GitHub release target),
# but they execute under emulation. Behaviour differences from native x86_64
# hardware are cosmetically possible but in practice absent for pure-Rust + C
# compiled workloads.
#
# Time budget: each emulated build is capped at 20 minutes. If a build exceeds
# the cap, the script records the timeout and continues with whatever binaries
# were successfully produced.
#
# Usage:
#   cd apps/codemap-search
#   bash docker/verify.sh [--skip-gnu] [--skip-musl] [--no-cleanup]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$SCRIPT_DIR/out"
LOG_FILE="$SCRIPT_DIR/verify-run.log"
BUILD_CAP_SECONDS=1200  # 20 minutes per build

# Locate a `timeout` implementation. macOS ships without GNU coreutils by
# default; Homebrew installs it as `gtimeout`. Fall back to no-timeout if
# neither is available (the Docker build will run uncapped in that case).
TIMEOUT_CMD=""
if command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_CMD="gtimeout"
elif command -v timeout >/dev/null 2>&1; then
    TIMEOUT_CMD="timeout"
else
    echo "WARNING: neither 'timeout' nor 'gtimeout' found — builds run uncapped." >&2
fi

SKIP_GNU=false
SKIP_MUSL=false
NO_CLEANUP=false
for arg in "$@"; do
    case "$arg" in
        --skip-gnu)  SKIP_GNU=true ;;
        --skip-musl) SKIP_MUSL=true ;;
        --no-cleanup) NO_CLEANUP=true ;;
    esac
done

# Redirect all output to both terminal and log file
exec > >(tee -a "$LOG_FILE") 2>&1

echo "======================================================================"
echo "codemap-search Linux verification harness"
echo "Started: $(date -u)"
echo "Crate:   $CRATE_DIR"
echo "Out:     $OUT_DIR"
echo "======================================================================"

mkdir -p "$OUT_DIR"

# ------------------------------------------------------------------
# Helper: run a binary inside a distro image and capture results
# ------------------------------------------------------------------
# Args: $1=distro-image, $2=binary-host-path, $3=binary-label
check_binary_on_distro() {
    local image="$1"
    local binary_path="$2"
    local label="$3"
    local binary_name
    binary_name="$(basename "$binary_path")"

    # Pull the image first (suppresses pull noise in results)
    docker pull --platform linux/amd64 "$image" >/dev/null 2>&1 || true

    # Test 1: --version
    # Capture stdout+stderr to a temp file and record the real exit code.
    # Use `|| exit_version=$?` so a non-zero exit is captured without aborting
    # under `set -e`, and without forcing the code to 0 as `|| true` would.
    local version_out exit_version
    local _ver_tmp
    _ver_tmp="$(mktemp)"
    exit_version=0
    docker run --rm --platform linux/amd64 \
        -v "$binary_path:/usr/local/bin/$binary_name:ro" \
        "$image" \
        /usr/local/bin/"$binary_name" --version >"$_ver_tmp" 2>&1 || exit_version=$?
    version_out="$(cat "$_ver_tmp")"
    rm -f "$_ver_tmp"

    # Test 2: smoke — parse a small Rust file (exercises tree-sitter C grammars;
    # no index needed, no MCP stdio loop, exits immediately).
    local smoke_out exit_smoke
    local _smoke_tmp
    _smoke_tmp="$(mktemp)"
    exit_smoke=0
    ${TIMEOUT_CMD:+$TIMEOUT_CMD 30} docker run --rm --platform linux/amd64 \
        -v "$binary_path:/usr/local/bin/$binary_name:ro" \
        "$image" \
        sh -c 'printf "fn foo() { let x = 1; }\n" > /tmp/a.rs && /usr/local/bin/'"$binary_name"' parse /tmp/a.rs' >"$_smoke_tmp" 2>&1 || exit_smoke=$?
    smoke_out="$(cat "$_smoke_tmp")"
    rm -f "$_smoke_tmp"

    # Classify result
    local status="PASS"
    local failure_detail=""
    if echo "$version_out$smoke_out" | grep -qiE "GLIBC|libc.so|ld-linux|not found|cannot execute|Exec format error|Illegal instruction"; then
        status="FAIL"
        failure_detail=$(echo "$version_out$smoke_out" | grep -iE "GLIBC|libc.so|ld-linux|not found|cannot execute|Exec format error|Illegal instruction" | head -3)
    elif [ "$exit_version" -ne 0 ] || [ "$exit_smoke" -ne 0 ]; then
        status="FAIL"
        failure_detail="version exit=$exit_version smoke exit=$exit_smoke"
    fi

    echo ""
    echo "--- [$label] $image ---"
    echo "  --version exit=$exit_version  output: $(echo "$version_out" | head -1)"
    echo "  smoke    exit=$exit_smoke     output: $(echo "$smoke_out" | head -2 | tr '\n' ' ')"
    if [ "$status" = "FAIL" ]; then
        echo "  STATUS: FAIL  detail: $failure_detail"
    else
        echo "  STATUS: PASS"
    fi

    # Output machine-readable line for result aggregation
    echo "RESULT|$image|$label|$status|version_exit=$exit_version|smoke_exit=$exit_smoke|$(echo "$failure_detail" | tr '\n' ' ')"
}

# ------------------------------------------------------------------
# Distro matrix (x86_64)
# ------------------------------------------------------------------
DISTROS=(
    "ubuntu:20.04"
    "ubuntu:22.04"
    "ubuntu:24.04"
    "debian:12"
    "rockylinux:9"
    "alpine:3.20"
)

# ------------------------------------------------------------------
# Build: x86_64-unknown-linux-gnu
# ------------------------------------------------------------------
GNU_BINARY="$OUT_DIR/codemap-search-gnu"
GNU_BUILD_SUCCESS=false
GNU_BUILD_SECONDS=0

if [ "$SKIP_GNU" = false ]; then
    echo ""
    echo "======================================================================"
    echo "BUILD: x86_64-unknown-linux-gnu (ubuntu:24.04 + rustup, emulated amd64)"
    echo "Cap: ${BUILD_CAP_SECONDS}s"
    echo "======================================================================"

    GNU_START=$(date +%s)
    if ${TIMEOUT_CMD:+$TIMEOUT_CMD "$BUILD_CAP_SECONDS"} docker build \
        --platform linux/amd64 \
        -f "$SCRIPT_DIR/Dockerfile.build-gnu" \
        --output "type=local,dest=$OUT_DIR" \
        "$CRATE_DIR"; then
        GNU_BUILD_SUCCESS=true
        GNU_END=$(date +%s)
        GNU_BUILD_SECONDS=$((GNU_END - GNU_START))
        if [ -f "$OUT_DIR/codemap-search-gnu" ]; then
            GNU_SIZE=$(du -h "$OUT_DIR/codemap-search-gnu" | cut -f1)
            echo "BUILD SUCCESS: gnu binary at $OUT_DIR/codemap-search-gnu  size=$GNU_SIZE  elapsed=${GNU_BUILD_SECONDS}s"
        else
            echo "BUILD: docker succeeded but output file not found — checking..."
            ls -la "$OUT_DIR/" || true
            GNU_BUILD_SUCCESS=false
        fi
    else
        GNU_EXIT=$?
        GNU_END=$(date +%s)
        GNU_BUILD_SECONDS=$((GNU_END - GNU_START))
        if [ "$GNU_EXIT" -eq 124 ]; then
            echo "BUILD TIMEOUT: gnu build exceeded ${BUILD_CAP_SECONDS}s cap. Elapsed=${GNU_BUILD_SECONDS}s."
        else
            echo "BUILD FAILED: gnu build exit=$GNU_EXIT  elapsed=${GNU_BUILD_SECONDS}s"
        fi
    fi
fi

# Run distro matrix for gnu if build succeeded
if [ "$GNU_BUILD_SUCCESS" = true ] && [ -f "$GNU_BINARY" ]; then
    echo ""
    echo "======================================================================"
    echo "DISTRO MATRIX: gnu binary"
    echo "======================================================================"
    for distro in "${DISTROS[@]}"; do
        check_binary_on_distro "$distro" "$GNU_BINARY" "gnu"
    done
fi

# ------------------------------------------------------------------
# Build: x86_64-unknown-linux-musl
# ------------------------------------------------------------------
MUSL_BINARY="$OUT_DIR/codemap-search-musl"
MUSL_BUILD_SUCCESS=false
MUSL_BUILD_SECONDS=0

if [ "$SKIP_MUSL" = false ]; then
    echo ""
    echo "======================================================================"
    echo "BUILD: x86_64-unknown-linux-musl (rust:alpine, emulated amd64)"
    echo "Cap: ${BUILD_CAP_SECONDS}s"
    echo "======================================================================"

    MUSL_START=$(date +%s)
    if ${TIMEOUT_CMD:+$TIMEOUT_CMD "$BUILD_CAP_SECONDS"} docker build \
        --platform linux/amd64 \
        -f "$SCRIPT_DIR/Dockerfile.build-musl" \
        --output "type=local,dest=$OUT_DIR" \
        "$CRATE_DIR"; then
        MUSL_BUILD_SUCCESS=true
        MUSL_END=$(date +%s)
        MUSL_BUILD_SECONDS=$((MUSL_END - MUSL_START))
        if [ -f "$OUT_DIR/codemap-search-musl" ]; then
            MUSL_SIZE=$(du -h "$OUT_DIR/codemap-search-musl" | cut -f1)
            echo "BUILD SUCCESS: musl binary at $OUT_DIR/codemap-search-musl  size=$MUSL_SIZE  elapsed=${MUSL_BUILD_SECONDS}s"
        else
            echo "BUILD: docker succeeded but output file not found — checking..."
            ls -la "$OUT_DIR/" || true
            MUSL_BUILD_SUCCESS=false
        fi
    else
        MUSL_EXIT=$?
        MUSL_END=$(date +%s)
        MUSL_BUILD_SECONDS=$((MUSL_END - MUSL_START))
        if [ "$MUSL_EXIT" -eq 124 ]; then
            echo "BUILD TIMEOUT: musl build exceeded ${BUILD_CAP_SECONDS}s cap. Elapsed=${MUSL_BUILD_SECONDS}s."
        else
            echo "BUILD FAILED: musl build exit=$MUSL_EXIT  elapsed=${MUSL_BUILD_SECONDS}s"
        fi
    fi
fi

# Run distro matrix for musl if build succeeded
if [ "$MUSL_BUILD_SUCCESS" = true ] && [ -f "$MUSL_BINARY" ]; then
    echo ""
    echo "======================================================================"
    echo "DISTRO MATRIX: musl binary"
    echo "======================================================================"
    for distro in "${DISTROS[@]}"; do
        check_binary_on_distro "$distro" "$MUSL_BINARY" "musl"
    done
fi

# ------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------
echo ""
echo "======================================================================"
echo "SUMMARY"
echo "======================================================================"
echo "gnu build:  success=$GNU_BUILD_SUCCESS  elapsed=${GNU_BUILD_SECONDS}s"
echo "musl build: success=$MUSL_BUILD_SUCCESS  elapsed=${MUSL_BUILD_SECONDS}s"
echo ""
echo "Result lines:"
grep "^RESULT|" "$LOG_FILE" | tail -20 || echo "(no results captured)"

echo ""
echo "Completed: $(date -u)"
echo "Log: $LOG_FILE"

# ------------------------------------------------------------------
# Cleanup intermediate images (leave named outputs for inspection)
# ------------------------------------------------------------------
if [ "$NO_CLEANUP" = false ]; then
    echo ""
    echo "Pruning dangling images..."
    docker image prune -f >/dev/null 2>&1 || true
fi
