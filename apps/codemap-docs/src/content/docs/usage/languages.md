---
title: Supported Languages & Indexing
description: Supported programming languages, data and markup formats, optional language switches, extraction details, file exclusions, indexing limits, and stderr diagnostics.
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

Programming languages expose declarations, searchable symbols, docstrings/comments, and string literals. Call-graph support varies by language and whether call targets resolve statically; SQL extracts declarations and literals without caller/callee edges.

## Data, markup, and components

| Format | Extensions | Extracted structure and limits |
|---|---|---|
| JSON / JSONC | `.json`, `.jsonc` | Keys and nested key paths (JSONC requires quoted keys); arrays and scalars stay searchable as text |
| TOML | `.toml` | Keys and nested key paths; values stay searchable as text |
| YAML | `.yaml`, `.yml` | Keys and nested key paths; values stay searchable as text |
| HTML | `.html`, `.htm` | Tags, `id`, and `class` |
| XML and derivatives | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.plist`, `.csproj`, `.props`, `.targets` | XML tags and attributes (parsed at XML syntax level) |
| CSS | `.css` | Selectors, custom properties, and keyframe names |
| Less | `.less` | Style rules and mixins, including `#identifier(...)` definitions |
| Sass | `.sass` | Indented Sass structure (on syntax errors, extraction stops at the valid prefix) |
| Vue / Astro / Svelte | `.vue`, `.astro`, `.svelte` | Markup plus embedded JavaScript/TypeScript and CSS/Sass/Less, keeping original line/column ranges and merging duplicate symbols |

Data and standalone markup or stylesheet formats do not build function call graphs. JSON5 and SCSS are not supported (including inside `.vue`, `.astro`, and `.svelte` components).

## Optional groups

All five switches under `[index.language_support]` in [configuration](/configuration/) default to `false`:

| Key | Group |
|---|---|
| `is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

Disabled groups stay out of indexing, `search`, `codemap`, and file-watcher updates, while live `find`, `grep`, `read`, and CLI `parse` still work on them. Changing a switch queues a full index refresh after config reload.

| Format | Extracted structure | Limits |
|---|---|---|
| Markdown / MDX | Full text, ATX/Setext headings, inline/reference/autolinks, code blocks, original ranges | No link graphs, imports, references, or call graphs; code blocks are not reparsed, and MDX JSX/JS expressions are text-only |
| Bash / Zsh | Functions, variables, environment variables, literal `source` and `.` imports | No call graph (even for static shell calls); `eval`, indirect expansion, and dynamic imports using variable or command substitution produce no calls, references, or imports |
| HCL / Terraform | Declarations, resources, unnamed `terraform` blocks, static `var`/`local`/`module`/`data` references, literal module `source` | Dynamic or ambiguous edges omitted; no call graph |
| Dockerfile | `ARG`, `ENV`, `LABEL`, stages, and base-image dependencies | Matches exact `Dockerfile` filename; no call graph |
| Nix | Static attribute paths, `let`, `inherit`, function bindings, direct `derivation`/`mkDerivation` targets, literal `import`/`builtins.import`/`callPackage` paths, static references, and direct applications | Uniquely resolved direct applications can mark `precise` calls; no evaluator execution, dynamic attribute/import resolution, or custom flake/nixpkgs semantics |
| Protocol Buffers | Declarations, services/types, `import`, field and RPC types | Dynamic or ambiguous edges omitted; no call graph |
| GraphQL | Types, fragments, unnamed `schema` definitions, fragment spreads, named type references | Dynamic or ambiguous edges omitted; no call graph |
| Make / CMake / Starlark-Bazel | Targets, rules, variables, and target dependencies | Target dependencies stay separate from function call graphs |

## Extraction details

Call relationships default to approximate matching, so verify approximate edges in the source. With `[output.navigation].is_enabled = true`, uniquely resolved targets are marked `precise`, whereas paths and call targets determined at runtime (such as reflection or PowerShell dynamic execution) are never marked `precise`. Use `[index].store_references` to store non-call reference locations; see [output and call relationships](/configuration/#output-and-call-relationships).

| Language | Visibility, test, or deprecation rules |
|---|---|
| Go | Uppercase initial marks exported; `*_test.go` and `Test`/`Benchmark`/`Example`/`Fuzz` mark tests; `// Deprecated:` doc comments mark deprecation |
| Java | `public`, `@Test` / `*Test.java`, `@Deprecated` and Javadoc `@deprecated` |
| Kotlin | Exported unless `private`, `internal`, or `protected`; recognizes `@Test` and `@Deprecated` |
| C / C++ | `static` marks file-local; C++ members follow access specifiers (structs public and classes private by default) |
| Assembly / GAS | `.globl` / `.global` marks exported symbols |
| C# | Explicit `public` and implicitly public interface members |
| PHP | Top-level declarations and members without `private` or `protected` are public |
| Ruby | Class and module visibility sections |
| Lua | File-level declarations are exposed, including `local` |

`.gradle` files also extract literal `task`/`tasks.register`/`tasks.create` targets, task dependencies and ordering, plugin IDs, and `group:artifact:version` coordinates; interpolated strings and custom DSLs stay unstructured. `.gradle.kts` files use standard Kotlin support.

## Files excluded from indexing

These filename and extension rules override language support (ASCII case-insensitive):

| Category | Names or patterns |
|---|---|
| Plain text | `.txt` |
| Credentials | `auth.toml` |
| Lockfiles | `*.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `Gemfile.lock`, `composer.lock`, `poetry.lock`, `Pipfile.lock` |
| Source maps | `*.map` |
| Minified files | `*.min.js`, `*.min.mjs`, `*.min.cjs`, `*.min.css`, `*.min.html` |
| Bundles | `*.bundle.js`, `*.bundle.mjs`, `*.bundle.cjs`, `*.bundle.css` |

- These exclusions apply to indexing, `codemap`, and caller scans (for example, JSON support skips `package-lock.json`). `find` and `grep` can include them with `include_ignored: true`, and direct `read` or CLI `parse` still works.
- Add repository-specific exclusions in `.codemapignore`. VCS internals and `.codemap` directories remain hidden even when `include_ignored` is set; see [directory exclusions](/configuration/#directory-exclusions).
- The user home directory (`~`) cannot be used as an MCP workspace or explicit indexing target, whereas subdirectories like `~/work/project` and global config in `~/.codemap` are allowed.
- Syntax or parse errors never crash the MCP server or background indexer; affected files store partial or empty structure.

## Indexing, diagnostics, and limits

By default, the MCP server stores its index in `.codemap/index`. Under the default `[index] mode = "auto"`, indexing starts on the first `search`, `read`, `grep`, `synonyms`, or monorepo `initial_instructions` call, and only inside a repository with at most `50,000` indexable files. In other folders, such as a parent directory holding multiple repositories, `search` reports why nothing was indexed; set `mode = "on"` to force indexing (see [When indexing starts](/configuration/#when-indexing-starts)).

When file watching is active, edits are batched for `500ms` and only affected, non-ignored paths refresh; Git `HEAD` switches, ignore-file changes, or large batches trigger a full walk. When watching is off or unavailable, `search` refreshes based on `index.refresh.index_staleness_ms`, while `read`, `find`, and `grep` always read disk directly.

- Files larger than `index.max_file_bytes` (default `1 MiB`) are skipped during indexing and `codemap`.
- `codemap-search mcp` is a single-client sequential stdio server, so do not run multiple servers concurrently against the same index directory.

Logs and diagnostics go to `stderr` (default filter `warn,codemap_search=info`):

```sh
RUST_LOG=debug codemap-search mcp
```
