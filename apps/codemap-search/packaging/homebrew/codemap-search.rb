# typed: strict
# frozen_string_literal: true

# Sigil note: homebrew-core disables `Sorbet/StrictSigil` for `Formula/**` in its
# own `.rubocop.yml`, so the core convention is `# typed: false`. A standalone
# `brew style` run (no core rubocop) still enforces StrictSigil and rejects
# anything below `strict`, so `strict` is kept here to keep the local style gate
# green; flip to `# typed: false` at homebrew-core submission time.

# Homebrew formula for codemap-search (macOS only).
#
# Distribution model: this formula installs the prebuilt darwin release tarballs
# (Option B — homebrew-core path, no self-owned tap). homebrew-core generally
# prefers building from source; if that is required at review time, this becomes
# a source build. Until the first `codemap-v0.1.0` release exists, the `sha256`
# values below are placeholders — see the comment on each `sha256`.
class CodemapSearch < Formula
  desc "Self-contained MCP code search: BM25, codemap, read/find/grep"
  homepage "https://github.com/buYoung/mcp"
  version "0.1.0"
  license "MIT"

  # macOS-only formula: the standard top-level marker so homebrew-core's Linux
  # CI does not try to build it (the `on_macos` block alone is not a platform
  # constraint). No minimum is pinned — the prebuilt darwin tarballs are not
  # known to require a specific macOS version.
  depends_on :macos

  on_macos do
    on_arm do
      url "https://github.com/buYoung/mcp/releases/download/codemap-v0.1.0/codemap-search-aarch64-apple-darwin.tar.gz"
      # placeholder — fill from the sibling codemap-search-aarch64-apple-darwin.tar.gz.sha256
      # after the first codemap-v0.1.0 release publishes the assets.
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end

    on_intel do
      url "https://github.com/buYoung/mcp/releases/download/codemap-v0.1.0/codemap-search-x86_64-apple-darwin.tar.gz"
      # placeholder — fill from the sibling codemap-search-x86_64-apple-darwin.tar.gz.sha256
      # after the first codemap-v0.1.0 release publishes the assets.
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    bin.install "codemap-search"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/codemap-search --version")

    # Exercise a real, offline, deterministic subcommand beyond `--version`.
    # `tokenize` splits an identifier into lowercase sub-tokens (one per line)
    # with no filesystem, network, or prebuilt-index dependency, so it proves
    # the binary actually works in homebrew-core's sandboxed test environment.
    output = shell_output("#{bin}/codemap-search tokenize handleLoginError")
    assert_match "handle\nlogin\nerror", output
  end
end
