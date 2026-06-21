#!/bin/sh
# install.sh — codemap-search prebuilt-binary installer for macOS / Linux.
#
# Detects OS/arch, resolves the matching GitHub Release asset, downloads it,
# VERIFIES the sibling .sha256 BEFORE extracting, then installs the
# `codemap-search` binary to a PATH directory (default ~/.local/bin).
#
# Pure POSIX sh — no bashisms. Needs only: curl OR wget, tar, uname, mktemp,
# and sha256sum OR `shasum -a 256`. No sudo by default.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version codemap-v0.1.0   # pin a version
#
# Environment overrides:
#   INSTALL_DIR        install target dir (default: $HOME/.local/bin)
#   CODEMAP_VERSION    pin a release tag, e.g. codemap-v0.1.0 (default: latest)
#   CODEMAP_LINUX_LIBC linux libc flavor: musl (default) or gnu (x86_64 only)
#
# Verification seam (no download, prints resolved target + URLs and exits):
#   ./install.sh --print-target
#   CODEMAP_OS_OVERRIDE=Linux CODEMAP_ARCH_OVERRIDE=aarch64 ./install.sh --print-target

set -eu

REPO="buYoung/mcp"
BIN="codemap-search"

# ---- diagnostics -----------------------------------------------------------

fail() {
	# Print the message to stderr and exit non-zero. Installs nothing.
	printf 'error: %s\n' "$1" >&2
	exit 1
}

info() {
	printf '%s\n' "$1" >&2
}

# ---- platform detection ----------------------------------------------------

# Resolve the target triple from OS + arch. uname output is read through
# override vars so the mapping can be exercised without a real host.
#
# Sets the global `target` to the resolved triple.
resolve_target() {
	os="${CODEMAP_OS_OVERRIDE:-$(uname -s)}"
	arch="${CODEMAP_ARCH_OVERRIDE:-$(uname -m)}"

	# Normalize arch aliases to the canonical names used in asset triples.
	case "$arch" in
		x86_64 | amd64) arch="x86_64" ;;
		arm64 | aarch64) arch="aarch64" ;;
		*) fail "unsupported architecture: $arch (only x86_64 and arm64/aarch64 are released)" ;;
	esac

	case "$os" in
		Darwin)
			case "$arch" in
				aarch64) target="aarch64-apple-darwin" ;;
				x86_64) target="x86_64-apple-darwin" ;;
			esac
			;;
		Linux)
			# Generic Linux prefers the fully static musl asset. The gnu asset
			# exists for x86_64 only; arm64 ships musl-only (no gnu asset).
			libc="${CODEMAP_LINUX_LIBC:-musl}"
			case "$libc" in
				musl) ;;
				gnu)
					[ "$arch" = "x86_64" ] || fail "no Linux gnu asset for $arch — only x86_64-unknown-linux-gnu is published; use musl (the default) for arm64"
					;;
				*) fail "unknown CODEMAP_LINUX_LIBC: $libc (expected musl or gnu)" ;;
			esac
			target="${arch}-unknown-linux-${libc}"
			;;
		*)
			fail "unsupported OS: $os (this installer covers macOS and Linux only; Windows is served by WinGet)"
			;;
	esac
}

# ---- URL construction ------------------------------------------------------

# Build the asset + checksum URLs from the resolved target and version.
# A pinned tag uses .../releases/download/<tag>/<asset>; latest uses
# .../releases/latest/download/<asset> (a repo-global redirect — see note).
#
# Sets globals: asset, archive_url, sha_url.
resolve_urls() {
	asset="${BIN}-${target}.tar.gz"
	version="${CODEMAP_VERSION:-}"
	if [ -n "$version" ]; then
		base="https://github.com/${REPO}/releases/download/${version}"
	else
		base="https://github.com/${REPO}/releases/latest/download"
	fi
	archive_url="${base}/${asset}"
	sha_url="${archive_url}.sha256"
}

# ---- downloading -----------------------------------------------------------

# Download $1 (url) to $2 (path) using curl or wget. -f/--fail equivalents are
# load-bearing: a 404 must error, not silently write the HTML body to the file.
download() {
	url="$1"
	dest="$2"
	if command -v curl >/dev/null 2>&1; then
		curl -fsSL -o "$dest" "$url" || fail "download failed: $url"
	elif command -v wget >/dev/null 2>&1; then
		wget -q -O "$dest" "$url" || fail "download failed: $url"
	else
		fail "need curl or wget to download releases"
	fi
}

# ---- checksum verification (BEFORE extract) --------------------------------

# Compute the sha256 of $1 and print just the lowercase hex hash.
compute_sha256() {
	file="$1"
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$file" | awk '{print $1}'
	elif command -v shasum >/dev/null 2>&1; then
		shasum -a 256 "$file" | awk '{print $1}'
	else
		fail "need sha256sum or shasum to verify the download"
	fi
}

# Verify $1 (archive) against $2 (.sha256 file in "<hash>  <name>" format).
# Compute-and-compare avoids `shasum -c`'s cwd/basename resolution. On mismatch
# this fails non-zero so the caller installs nothing.
verify_sha256() {
	archive="$1"
	sha_file="$2"
	expected="$(awk '{print $1}' "$sha_file")"
	[ -n "$expected" ] || fail "checksum file is empty or malformed: $sha_file"
	actual="$(compute_sha256 "$archive")"
	if [ "$expected" != "$actual" ]; then
		fail "checksum mismatch (expected $expected, got $actual) — refusing to install"
	fi
}

# ---- install ---------------------------------------------------------------

# Warn if $1 (the install dir) is not an entry on PATH. Substring-safe by
# wrapping both PATH and dir in colons.
warn_if_not_on_path() {
	dir="$1"
	case ":${PATH}:" in
		*":${dir}:"*) ;;
		*)
			info ""
			info "note: ${dir} is not on your PATH."
			info "      add it for this session:  export PATH=\"${dir}:\$PATH\""
			info "      to persist it, append that line to your shell profile"
			info "      (zsh: ~/.zshrc, bash: ~/.bashrc), then restart your shell."
			;;
	esac
}

# ---- main ------------------------------------------------------------------

main() {
	print_target=0
	# Argument parsing: --version <tag> pins a release; --print-target dry-runs
	# the detection/mapping/URL seam without downloading.
	while [ $# -gt 0 ]; do
		case "$1" in
			--version)
				[ $# -ge 2 ] || fail "--version needs a tag argument (e.g. codemap-v0.1.0)"
				CODEMAP_VERSION="$2"
				shift 2
				;;
			--version=*)
				CODEMAP_VERSION="${1#--version=}"
				shift
				;;
			--print-target)
				print_target=1
				shift
				;;
			-h | --help)
				info "usage: install.sh [--version <codemap-vX.Y.Z>] [--print-target]"
				info "env: INSTALL_DIR, CODEMAP_VERSION, CODEMAP_LINUX_LIBC=musl|gnu"
				exit 0
				;;
			*) fail "unknown argument: $1" ;;
		esac
	done

	resolve_target
	resolve_urls

	if [ "$print_target" -eq 1 ]; then
		# Dry mode: report what would be fetched, then exit before any network.
		printf 'os:          %s\n' "${CODEMAP_OS_OVERRIDE:-$(uname -s)}"
		printf 'arch:        %s\n' "${CODEMAP_ARCH_OVERRIDE:-$(uname -m)}"
		printf 'target:      %s\n' "$target"
		printf 'asset:       %s\n' "$asset"
		printf 'archive_url: %s\n' "$archive_url"
		printf 'sha_url:     %s\n' "$sha_url"
		printf 'install_dir: %s\n' "${INSTALL_DIR:-$HOME/.local/bin}"
		exit 0
	fi

	# Preflight: the header promises tar/uname/mktemp. curl/wget and
	# sha256sum/shasum are checked at their call sites; check the remaining
	# tools here, BEFORE any network call, so a missing one fails early with
	# the real cause instead of a late, mis-attributed "could not extract".
	for required_tool in tar mktemp uname; do
		command -v "$required_tool" >/dev/null 2>&1 || fail "need $required_tool to install"
	done

	# Temp workspace, removed on any exit (success, failure, or signal).
	tmpdir="$(mktemp -d)" || fail "could not create a temp directory"
	trap 'rm -rf "$tmpdir"' EXIT INT TERM HUP

	archive="${tmpdir}/${asset}"
	sha_file="${archive}.sha256"

	info "resolving ${BIN} (${target})..."
	download "$archive_url" "$archive"
	download "$sha_url" "$sha_file"

	# HARD ORDERING: verify the checksum BEFORE extracting anything.
	info "verifying sha256..."
	verify_sha256 "$archive" "$sha_file"

	# Only now, on a verified archive, extract the binary.
	tar -xzf "$archive" -C "$tmpdir" "$BIN" || fail "could not extract $BIN from the archive"
	[ -f "${tmpdir}/${BIN}" ] || fail "$BIN not found in the archive"

	install_dir="${INSTALL_DIR:-$HOME/.local/bin}"
	mkdir -p "$install_dir" || fail "could not create install dir: $install_dir"
	chmod +x "${tmpdir}/${BIN}"
	# cp then rm (not mv) so a cross-filesystem tmpdir does not break install.
	cp "${tmpdir}/${BIN}" "${install_dir}/${BIN}" || fail "could not install to ${install_dir}"

	info "installed ${BIN} to ${install_dir}/${BIN}"
	warn_if_not_on_path "$install_dir"
}

main "$@"
