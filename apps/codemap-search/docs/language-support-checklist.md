# Supported languages and file formats

[한국어](./language-support-checklist.ko.md) | English

This reference lists files codemap-search can index, the structure it extracts, and the limits of static analysis. For setup, see the [README](../README.md); for switches and exclusions, see [configuration](./configuration.md).

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

Programming languages expose declarations and searchable symbols, documentation, and literals. Call support depends on the language and whether the target can be identified; SQL extracts declarations and literals without caller/callee relationships.

## Data, markup, and components

| Format | Extensions | Extracted structure and limits |
|---|---|---|
| JSON / JSONC | `.json`, `.jsonc` | Keys and nested key paths; JSONC requires quoted keys. Arrays and scalar values remain searchable text |
| TOML | `.toml` | Keys and nested key paths; values remain searchable text |
| YAML | `.yaml`, `.yml` | Keys and nested key paths; values remain searchable text |
| HTML | `.html`, `.htm` | Tags, `id`, and `class` |
| XML and derivatives | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.plist`, `.csproj`, `.props`, `.targets` | XML tags and attributes; derivative formats are understood at XML syntax level |
| CSS | `.css` | Selectors, custom properties, and keyframe names |
| Less | `.less` | Style structure and mixins, including recognizable `#identifier(...)` definitions |
| Sass | `.sass` | Indented Sass structure; an unrecoverable error limits extraction to the valid prefix |
| Vue / Astro / Svelte | `.vue`, `.astro`, `.svelte` | Markup with embedded JavaScript/TypeScript and CSS/Sass/Less; original line/column ranges retained and duplicate symbols merged |

Data and standalone markup/style formats do not create ordinary caller/callee relationships. **JSON5 and SCSS are not supported by the format registry.** Component support does not enable SCSS extraction.

## Optional groups

All five switches default to `false` under `[language_support]`:

| Key | Group |
|---|---|
| `is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

When disabled, these files are absent from indexing, search, overview, codemap, and file-change refreshes. Live `find`/`grep`/`read` and direct CLI `parse` remain available. Changing a switch requests a full index refresh after config reload; source files need no edit.

| Format | Extracted structure | Limits |
|---|---|---|
| Markdown / MDX | Full text, ATX/Setext headings, inline/reference/autolinks, fenced/indented code blocks, original ranges | No link relationships, imports, references, or caller/callee graph; fenced code is not reparsed. MDX JSX/JavaScript expressions are text-only |
| Bash / Zsh | Functions, variables, environment variables, literal `source` and `.` imports | No ordinary call graph, including static shell calls. Dynamic execution, `eval`, indirect expansion, and imports containing variable/command substitution do not produce calls, references, or imports |
| HCL / Terraform | Declarations, resources, unnamed `terraform` blocks, identifiable `var`/`local`/`module`/`data` references, literal module `source` | Dynamic or ambiguous relationships omitted; no ordinary call graph |
| Dockerfile | ARG, ENV, LABEL, stages, and base-image dependencies | Exact `Dockerfile` filename; no ordinary call graph |
| Nix | Static attribute paths, `let`, `inherit`, function bindings, direct `derivation`/`mkDerivation` targets, literal `import`/`builtins.import`/`callPackage` paths, static references and direct function applications | Confirmed direct applications can be `precise` calls. No evaluator execution, dynamic attribute/import resolution, or special semantic interpretation of flake `inputs`/`outputs` and nixpkgs idioms |
| Protocol Buffers | Declarations, services/types, `import`, field and RPC types | Dynamic or ambiguous relationships omitted; no ordinary call graph |
| GraphQL | Types, fragments, unnamed `schema` definitions, fragment spreads, named type references | Dynamic or ambiguous relationships omitted; no ordinary call graph |
| Make / CMake / Starlark-Bazel | Targets, rules, variables, and target dependencies | Target dependencies are represented separately from ordinary function calls |

## Extraction details

Call relationships are approximate by default. With `navigation_context_default = true`, a uniquely confirmed call target is marked `precise`. Paths and call targets determined at runtime, including reflection, are not treated as confirmed. `navigation_store_references` controls optional storage of reference locations other than function calls; see the [configuration reference](./configuration.md#output-and-call-relationships).

| Language | Visibility, test, or deprecation rules |
|---|---|
| Go | Initial uppercase means exported; `*_test.go` and `Test`/`Benchmark`/`Example`/`Fuzz` identify tests; `// Deprecated:` documentation marks deprecation |
| Java | `public`, `@Test` / `*Test.java`, and `@Deprecated` / javadoc `@deprecated` |
| Kotlin | Exported unless `private`/`internal`/`protected`; recognizes `@Test` and `@Deprecated` |
| C / C++ | `static` means file-local; C++ members follow access specifiers, with public struct members and private class members by default |
| Assembly / GAS | `.globl` / `.global` marks exported symbols |
| C# | Explicit `public` and implicitly public interface members |
| PHP | Top-level declarations and members without `private`/`protected` are public |
| Ruby | Class/module visibility sections |
| Lua | File-level declarations are exposed, including `local` |

C#, PHP, Ruby, and Lua extract statically identifiable declarations, literal imports, references, and calls, and recognize supported conventional test paths/names and deprecation attributes/comments. Swift, Dart, Scala, Groovy, and PowerShell also extract static declarations, imports, references, and calls with language-specific visibility/test/deprecation rules. PowerShell dynamic execution is not a confirmed call.

`.gradle` additionally extracts literal `task`/`tasks.register`/`tasks.create` targets, task dependency/order relationships, plugin IDs, and `group:artifact:version` coordinates. Interpolated values and custom DSLs remain unstructured. `.gradle.kts` uses ordinary Kotlin support.

## Files excluded from indexing

These rules take priority over supported extensions and ignore ASCII case:

| Category | Names or patterns |
|---|---|
| Plain text | `.txt` |
| Lockfiles | `*.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `Gemfile.lock`, `composer.lock`, `poetry.lock`, `Pipfile.lock` |
| Source maps | `*.map` |
| Minified files | `*.min.js`, `*.min.mjs`, `*.min.cjs`, `*.min.css`, `*.min.html` |
| Bundles | `*.bundle.js`, `*.bundle.mjs`, `*.bundle.cjs`, `*.bundle.css` |

For example, JSON support does not include `package-lock.json`. These file exclusions apply to indexing, codemap, and caller scans and cannot be disabled there. `find`/`grep` can include them with `include_ignored: true`; direct `read`/`parse` remains available. Add repository-specific exclusions in `.codemapignore`.

Directory exclusions, ignore files, and `max_file_size` also affect which files are indexed. VCS internals and codemap's own directories remain excluded even with `include_ignored`. See [directory exclusions](./configuration.md#directory-exclusions) for the distinction between optional and mandatory rules.

The user home directory itself cannot be the MCP workspace or an explicit indexing target; `~/work/project` is valid, `~` is not. Paths are normalized before comparison, including symlinks and `..`. The check uses `HOME`/`USERPROFILE` and runs before creating repo config or index data. If neither home location can be determined, it warns on stderr and continues. The global `~/.codemap` configuration remains allowed.

Parse errors do not stop the MCP server or background indexing; an affected file may have incomplete or no extracted structure.
