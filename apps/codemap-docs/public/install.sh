#!/bin/sh
# install.sh — codemap-search prebuilt-binary installer for macOS / Linux.
#
# Detects OS/arch, resolves the matching GitHub Release asset, downloads it,
# VERIFIES the sibling .sha256 BEFORE extracting, then installs the
# `codemap-search` binary to a PATH directory (default ~/.local/bin).
#
# Pure POSIX sh — no bashisms. Needs only standard POSIX utilities plus
# curl OR wget, and sha256sum OR `shasum -a 256`. No sudo by default.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version 0.14.2   # pin a version
#
# Environment overrides:
#   INSTALL_DIR             install target dir (default: $HOME/.local/bin)
#   CODEMAP_VERSION         pin a release tag or version (default: latest)
#   CODEMAP_NO_MODIFY_PATH  set to 1/true/yes to skip updating shell profile PATH
#   CODEMAP_LINUX_LIBC      linux libc flavor: musl (default) or gnu (x86_64 only)
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

cleanup_install() {
	[ -z "${tmpdir:-}" ] || rm -rf "$tmpdir"
	[ -z "${install_tmpdir:-}" ] || rm -rf "$install_tmpdir"
	[ -z "${profile_tmp:-}" ] || rm -f "$profile_tmp"
}

# Normalize version strings ("" | latest | codemap-v* | v<num>* | <num>*) to
# exact release tag names, or empty string for latest release.
normalize_version() {
	raw="$1"
	case "$raw" in
		"" | latest)
			printf '%s\n' ""
			return
			;;
		*[[:space:]]* | *" "* | *"	"* | */*)
			fail "invalid version: $raw"
			;;
		codemap-v*)
			[ "${#raw}" -gt 9 ] || fail "invalid version: $raw"
			printf '%s\n' "$raw"
			;;
		v[0-9]*)
			printf 'codemap-%s\n' "$raw"
			;;
		[0-9]*)
			printf 'codemap-v%s\n' "$raw"
			;;
		*)
			fail "invalid version: $raw (expected e.g. 0.14.2, v0.14.2, codemap-v0.14.2, or latest)"
			;;
	esac
}

normalize_dir() {
	d="$1"
	while [ "${#d}" -gt 1 ]; do
		case "$d" in
			*/) d="${d%/}" ;;
			*) break ;;
		esac
	done
	printf '%s\n' "$d"
}

is_truthy() {
	case "${1:-}" in
		1 | true | TRUE | yes | YES) return 0 ;;
		*) return 1 ;;
	esac
}

# ---- platform detection ----------------------------------------------------

# Resolve the target triple from OS + arch. uname output is read through
# override vars so the mapping can be exercised without a real host.
#
# Sets globals: detected_os, detected_arch, target.
resolve_target() {
	detected_os="${CODEMAP_OS_OVERRIDE:-$(uname -s)}"
	detected_arch="${CODEMAP_ARCH_OVERRIDE:-$(uname -m)}"
	os="$detected_os"
	arch="$detected_arch"

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
			fail "unsupported OS: $os (install.sh covers macOS and Linux; on Windows run: powershell -ExecutionPolicy ByPass -c \"irm https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.ps1 | iex\")"
			;;
	esac
}

# ---- URL construction ------------------------------------------------------

# Build the asset + checksum URLs from the resolved target and normalized tag.
#
# Sets globals: asset, archive_url, sha_url.
resolve_urls() {
	asset="${BIN}-${target}.tar.gz"
	if [ -n "$resolved_version" ]; then
		base="https://github.com/${REPO}/releases/download/${resolved_version}"
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

is_on_path() {
	target_dir="$(normalize_dir "$1")"
	remaining="${PATH:-}:"
	while [ -n "$remaining" ]; do
		entry="${remaining%%:*}"
		remaining="${remaining#*:}"
		[ -n "$entry" ] || continue
		if [ "$(normalize_dir "$entry")" = "$target_dir" ]; then
			return 0
		fi
	done
	return 1
}

# Print 2 lines via globals: profile_file and profile_kind (posix|fish).
pick_profile() {
	case "${SHELL:-}" in
		*/zsh | zsh)
			profile_file="${HOME}/.zshrc"
			profile_kind="posix"
			;;
		*/bash | bash)
			if [ "$detected_os" = "Darwin" ]; then
				profile_file="${HOME}/.bash_profile"
			else
				profile_file="${HOME}/.bashrc"
			fi
			profile_kind="posix"
			;;
		*/fish | fish)
			profile_file="${HOME}/.config/fish/conf.d/codemap-search.fish"
			profile_kind="fish"
			;;
		*)
			profile_file="${HOME}/.profile"
			profile_kind="posix"
			;;
	esac
}

print_manual_path_note() {
	dir="$1"
	info ""
	info "note: ${dir} is not on your PATH."
	info "      add it for this session:  export PATH=\"${dir}:\$PATH\""
}

# Idempotently configure PATH in the user's shell profile unless skipped or already present.
ensure_path() {
	dir="$1"
	skip_modify="$2"

	if is_on_path "$dir"; then
		return 0
	fi

	if [ "$skip_modify" -eq 1 ]; then
		print_manual_path_note "$dir"
		return 0
	fi

	if [ -z "${HOME:-}" ]; then
		print_manual_path_note "$dir"
		return 0
	fi

	pick_profile
	if [ "$profile_kind" = "fish" ]; then
		path_line="fish_add_path -g \"${dir}\""
	else
		path_line="export PATH=\"${dir}:\$PATH\""
	fi

	marker_start="# >>> codemap-search installer >>>"
	marker_end="# <<< codemap-search installer <<<"

	has_marker=0
	has_dir_ref=0
	if [ -f "$profile_file" ]; then
		if awk -v pat="$marker_start" '$0 == pat { f=1; exit } END { exit (f ? 0 : 1) }' "$profile_file" 2>/dev/null; then
			has_marker=1
		elif awk -v pat="$dir" 'index($0, pat) > 0 { f=1; exit } END { exit (f ? 0 : 1) }' "$profile_file" 2>/dev/null; then
			has_dir_ref=1
		fi
	fi

	if [ "$has_marker" -eq 0 ] && [ "$has_dir_ref" -eq 1 ]; then
		info "note: ${dir} is already referenced in ${profile_file}; open a new terminal or run: export PATH=\"${dir}:\$PATH\""
		return 0
	fi

	profile_dir="$(dirname "$profile_file")"
	if ! mkdir -p "$profile_dir" 2>/dev/null; then
		info ""
		info "note: could not update ${profile_file}; add ${dir} to PATH manually:"
		info "      export PATH=\"${dir}:\$PATH\""
		return 0
	fi

	profile_tmp="$(mktemp "${profile_dir}/.codemap-profile.tmp.XXXXXX" 2>/dev/null)" || {
		info ""
		info "note: could not update ${profile_file}; add ${dir} to PATH manually:"
		info "      export PATH=\"${dir}:\$PATH\""
		return 0
	}

	write_ok=1
	if [ "$has_marker" -eq 1 ]; then
		if ! awk -v start="$marker_start" -v end="$marker_end" -v entry="$path_line" '
			BEGIN { in_block = 0; replaced = 0 }
			$0 == start {
				if (!replaced) {
					print start
					print entry
					print end
					replaced = 1
				}
				in_block = 1
				next
			}
			in_block {
				if ($0 == end) {
					in_block = 0
				}
				next
			}
			{ print }
		' "$profile_file" > "$profile_tmp"; then
			write_ok=0
		fi
	else
		if [ -f "$profile_file" ] && [ -s "$profile_file" ]; then
			if ! awk '{ print }' "$profile_file" > "$profile_tmp" || ! printf '\n' >> "$profile_tmp"; then
				write_ok=0
			fi
		fi
		if [ "$write_ok" -eq 1 ]; then
			if ! {
				printf '%s\n' "$marker_start"
				printf '%s\n' "$path_line"
				printf '%s\n' "$marker_end"
			} >> "$profile_tmp"; then
				write_ok=0
			fi
		fi
	fi

	if [ "$write_ok" -eq 1 ] && mv -f "$profile_tmp" "$profile_file" 2>/dev/null; then
		profile_tmp=""
		info "Added ${dir} to PATH in ${profile_file} (open a new terminal or run: export PATH=\"${dir}:\$PATH\")"
	else
		rm -f "$profile_tmp" 2>/dev/null || true
		profile_tmp=""
		info ""
		info "note: could not update ${profile_file}; add ${dir} to PATH manually:"
		info "      export PATH=\"${dir}:\$PATH\""
	fi
}

# ---- main ------------------------------------------------------------------

main() {
	print_target=0
	has_opt_version=0
	opt_version=""
	has_opt_install_dir=0
	opt_install_dir=""
	no_modify_path=0
	if is_truthy "${CODEMAP_NO_MODIFY_PATH:-}"; then
		no_modify_path=1
	fi

	while [ $# -gt 0 ]; do
		case "$1" in
			--version)
				[ $# -ge 2 ] && [ -n "$2" ] || fail "--version needs a version or tag (e.g. 0.14.2 or codemap-v0.14.2)"
				opt_version="$2"
				has_opt_version=1
				shift 2
				;;
			--version=*)
				opt_version="${1#--version=}"
				[ -n "$opt_version" ] || fail "--version needs a version or tag (e.g. 0.14.2 or codemap-v0.14.2)"
				has_opt_version=1
				shift
				;;
			--install-dir)
				[ $# -ge 2 ] && [ -n "$2" ] || fail "--install-dir needs a directory path"
				opt_install_dir="$2"
				has_opt_install_dir=1
				shift 2
				;;
			--install-dir=*)
				opt_install_dir="${1#--install-dir=}"
				[ -n "$opt_install_dir" ] || fail "--install-dir needs a directory path"
				has_opt_install_dir=1
				shift
				;;
			--no-modify-path)
				no_modify_path=1
				shift
				;;
			--print-target)
				print_target=1
				shift
				;;
			-h | --help)
				info "usage: install.sh [--version <version>] [--install-dir <dir>] [--no-modify-path] [--print-target] [-h|--help]"
				info "env: INSTALL_DIR, CODEMAP_VERSION, CODEMAP_NO_MODIFY_PATH, CODEMAP_LINUX_LIBC=musl|gnu"
				exit 0
				;;
			*) fail "unknown argument: $1" ;;
		esac
	done

	if [ "$has_opt_version" -eq 1 ]; then
		raw_version="$opt_version"
	else
		raw_version="${CODEMAP_VERSION:-}"
	fi
	resolved_version="$(normalize_version "$raw_version")"

	if [ "$has_opt_install_dir" -eq 1 ]; then
		install_dir_raw="$opt_install_dir"
	elif [ -n "${INSTALL_DIR:-}" ]; then
		install_dir_raw="$INSTALL_DIR"
	elif [ -n "${HOME:-}" ]; then
		install_dir_raw="${HOME}/.local/bin"
	else
		fail "HOME is not set; pass --install-dir or set INSTALL_DIR"
	fi
	install_dir="$(normalize_dir "$install_dir_raw")"
	[ -n "$install_dir" ] || fail "install directory must not be empty"

	resolve_target
	resolve_urls

	version_label="${resolved_version:-latest}"

	if [ "$print_target" -eq 1 ]; then
		# Dry mode: report what would be fetched, then exit before any network.
		printf 'os:          %s\n' "$detected_os"
		printf 'arch:        %s\n' "$detected_arch"
		printf 'target:      %s\n' "$target"
		printf 'version:     %s\n' "$version_label"
		printf 'asset:       %s\n' "$asset"
		printf 'archive_url: %s\n' "$archive_url"
		printf 'sha_url:     %s\n' "$sha_url"
		printf 'install_dir: %s\n' "$install_dir"
		exit 0
	fi

	# Preflight: check standard POSIX tools BEFORE any network call.
	for required_tool in awk chmod cp dirname mkdir mktemp mv rm tar uname; do
		command -v "$required_tool" >/dev/null 2>&1 || fail "need $required_tool to install"
	done

	# Temp workspace, removed on any exit (success, failure, or signal).
	tmpdir="$(mktemp -d)" || fail "could not create a temp directory"
	install_tmpdir=""
	profile_tmp=""
	trap 'cleanup_install' EXIT
	trap 'cleanup_install; exit 1' INT TERM HUP

	archive="${tmpdir}/${asset}"
	sha_file="${archive}.sha256"

	info "Downloading ${BIN} (${target}, ${version_label})..."
	download "$archive_url" "$archive"
	download "$sha_url" "$sha_file"

	# HARD ORDERING: verify the checksum BEFORE extracting anything.
	info "Verifying sha256 checksum..."
	verify_sha256 "$archive" "$sha_file"

	# Only now, on a verified archive, extract the binary.
	tar -xzf "$archive" -C "$tmpdir" "$BIN" || fail "could not extract $BIN from the archive"
	extracted_bin="${tmpdir}/${BIN}"
	[ -f "$extracted_bin" ] || fail "$BIN not found in the archive"
	[ ! -L "$extracted_bin" ] || fail "$BIN in the archive is a symlink — refusing to install"

	mkdir -p "$install_dir" || fail "could not create install dir: $install_dir"
	install_tmpdir="$(mktemp -d "${install_dir}/.${BIN}.tmp.XXXXXX")" || fail "could not create a temp install dir in ${install_dir}"
	install_tmp="${install_tmpdir}/${BIN}"
	cp "$extracted_bin" "$install_tmp" || fail "could not stage install in ${install_dir}"
	chmod +x "$install_tmp" || fail "could not mark $BIN executable"
	# Stage on the target filesystem, then atomically replace the installed path.
	installed_bin="${install_dir}/${BIN}"
	mv -f "$install_tmp" "$installed_bin" || fail "could not install to ${install_dir}"

	installed_label=""
	if ver_out="$("${installed_bin}" --version 2>/dev/null)"; then
		installed_label="$(printf '%s\n' "$ver_out" | awk 'NR==1 { print; exit }')"
	fi
	if [ -z "$installed_label" ]; then
		installed_label="${BIN}"
	fi

	info "Installed ${installed_label} to ${installed_bin}"
	ensure_path "$install_dir" "$no_modify_path"
}

main "$@"
