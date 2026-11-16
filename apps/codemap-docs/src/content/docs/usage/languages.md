---
title: Supported Languages & Indexing
description: Supported programming languages, data and markup formats, optional language groups, excluded files, and indexing limits.
---

## Programming languages

| Language | Extensions |
|---|---|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript / TSX | `.ts`, `.tsx`, `.mts`, `.cts` |
| JavaScript / JSX | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Go | `.go` |
| Java | `.java` |
| Kotlin | `.kt`, `.kts` |
| C | `.c` |
| C++ | `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` |
| C# | `.cs` |
| PHP | `.php` |
| Ruby | `.rb` |
| Lua | `.lua` |
| Assembly / GAS | `.s`, `.S`, `.asm` |
| Swift | `.swift` |
| Dart | `.dart` |
| Scala | `.scala`, `.sc` |
| Groovy / Gradle | `.groovy`, `.gradle` |
| PowerShell | `.ps1`, `.psm1` |
| SQL | `.sql` |

## Data, markup, and components

| Format | Extensions | Indexed structure |
|---|---|---|
| JSON / JSONC | `.json`, `.jsonc` | Keys, nested key paths, and text values |
| TOML | `.toml` | Keys, nested key paths, and text values |
| YAML | `.yaml`, `.yml` | Keys, nested key paths, and text values |
| HTML | `.html`, `.htm` | Tags, `id`, and `class` |
| XML and derivatives | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.plist`, `.csproj`, `.props`, `.targets` | XML tags and attributes |
| CSS | `.css` | Selectors, custom properties, and keyframe names |
| Less | `.less` | Rules and mixins (`#identifier(...)`) |
| Sass | `.sass` | Indented Sass rules |
| Vue / Astro / Svelte | `.vue`, `.astro`, `.svelte` | Markup plus embedded JS/TS and CSS/Sass/Less |

JSON5 and SCSS are not indexed.

## Optional language groups

Set a key to `true` under `[index.language_support]` in `.codemap/config.toml` to index that group (all default to `false`). `find`, `grep`, and `read` work regardless of these settings.

| Key | Extensions / filenames |
|---|---|
| `is_document_support_enabled` | `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

## Excluded files and indexing limits

The following extensions and patterns are always skipped during indexing:

| Category | Extensions / patterns |
|---|---|
| Plain text | `.txt` |
| Credentials | `auth.toml` |
| Lockfiles | `*.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `Gemfile.lock`, `composer.lock`, `poetry.lock`, `Pipfile.lock` |
| Source maps | `*.map` |
| Minified files | `*.min.js`, `*.min.mjs`, `*.min.cjs`, `*.min.css`, `*.min.html` |
| Bundles | `*.bundle.js`, `*.bundle.mjs`, `*.bundle.cjs`, `*.bundle.css` |

- `.gitignore`, global gitignore, and `.git/info/exclude` (when `[index.exclude].use_git_exclude = true`, the default) are respected automatically. Use `.codemapignore` (gitignore syntax) only for extra paths tracked by Git that you still want `codemap-search` to skip. Pass `include_ignored: true` in `find` or `grep` to query ignored files.
- By default (`[index].mode = "auto"`), `codemap-search` automatically indexes repositories (`.git`, `.hg`, `.jj`) with up to `50,000` indexable files. For larger repositories, set `[index].mode = "on"` in `.codemap/config.toml`.
- Files larger than `[index].max_file_bytes` (default `1 MiB`) are skipped during indexing.
- Run with `RUST_LOG=debug` to write verbose diagnostics to `stderr`:

```sh
RUST_LOG=debug codemap-search mcp
```
