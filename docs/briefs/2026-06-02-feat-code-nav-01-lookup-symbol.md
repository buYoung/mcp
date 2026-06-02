# [feat] Add lookup_symbol tool (ctags symbol definitions)

## Work Type
feat

## Current State (As-Is)
- `apps/code-nav` exposes only `search_text` (zoekt); no symbol-definition lookup tool exists.
- The shipped `search_text` description tells the agent to use a "심볼 정의 도구" (symbol-definition tool) that does not exist yet — a dangling cross-reference.
- Universal Ctags is verified at startup and its absolute path is available as `ResolvedBinaries.ctagsPath`, but no provider consumes it.
- Verified this session: `ctags --output-format=json` on a single file emits `name`, `path`, `pattern`, `line`, `kind` (full word), `scope`, `scopeKind`, `access`; Kotlin / Rust / TypeScript are all supported.
- Verified this session: `ctags -R` over the repo root (even with `--exclude`) walks node_modules and JSON data files — 52.3s / 3.27M tags / 1.2GB on a 65-file repo. A scoped file list (`ctags -L`) of source files only is 0.09s (65 files) to 2.38s (398 files).

## Desired Outcome (To-Be)
- A `lookup_symbol` MCP tool returns symbol **definition** sites (relative `file:line`, kind, scope) for a given symbol name, ctags-backed.
- The tool's description routes definition intent here and call-site / cross-language intent to `search_text`.
- Repeated lookups on an unchanged working tree return from a fingerprint-keyed cache without re-running ctags.

## Scope
### In Scope
- New `SymbolProvider` plus a `ctags-runner` that consumes `ResolvedBinaries.ctagsPath`.
- Tool registration and dispatch in `apps/code-nav/src/tools/index.ts`.
- Input fields: `symbol_name` (required), `kind`, `path`, `language`, `is_prefix_match`, `head_limit` (default 250).
- Generate tags via `ctags -L <source-file-list>` over working-tree source files (uncommitted included), excluding `EXCLUDED_DIRECTORY_NAMES`; restrict kinds at generation with `--kinds-<lang>=`.
- A working-tree fingerprint-keyed in-memory cache (reuse the `IndexLifecycle` fingerprint walk) to avoid re-running ctags on unchanged trees.
- Update the `search_text` description to name `lookup_symbol`, and make `lookup_symbol`'s description point call-sites back to `search_text`.
### Out of Scope
- [hard] `ctags -R` whole-tree scan — measured 52s / 1.2GB; the file-list path is mandatory.
- [hard] persistent on-disk tags file — DESIGN §3.2; cache is in-memory.
- [deferred] using zoekt `sym:` for symbols — verified unreliable this session (`q=sym:formatJson` returned 0).
- [deferred] cross-tool symbol ranking or fuzzy matching beyond exact / prefix.

## Constraints
- Never invoke `ctags -R`; always pass an explicit source-file list via `ctags -L`.
- Share the working-tree source-file enumeration with `IndexLifecycle` (extract the directory walk into a shared helper); do not maintain a second, divergent enumeration. The `path` argument scopes that list to files under the resolved path (only those files are fed to `ctags -L`).
- Derive the per-language definition kind whitelist from `ctags --list-kinds-full=<lang>` for the supported languages and record it in `config/defaults.ts`. Default (no `kind` arg) keeps class/interface/struct/enum/function/method/typedef and language equivalents; import-alias, local, constant, and data kinds are excluded by default.
- Parse ctags NDJSON line-by-line; skip and continue on a malformed/partial line rather than aborting the whole lookup.
- Reuse `ResolvedBinaries.ctagsPath`, `path-guard`, `tools/arguments.ts`, and the `textResult` helper rather than re-implementing.

## Related Files / Entry Points
- `apps/code-nav/src/tools/index.ts` — register `lookup_symbol` and dispatch here (shared conflict hotspot).
- `apps/code-nav/src/startup/ensure-required-binaries.ts` — source of `ResolvedBinaries.ctagsPath`.
- `apps/code-nav/src/providers/text-search/index-lifecycle.ts` — reuse its working-tree walk for the source-file list and as the cache fingerprint.
- `apps/code-nav/src/security/path-guard.ts` — validate and scope the `path` argument.
- `apps/code-nav/src/config/defaults.ts` — add ctags fields argument, per-language definition kind map, source extensions.
- `apps/code-nav/DESIGN.md` — §2.2 and §3.2 carry the ctags design and the `-L` correction.
- `apps/code-nav/src/providers/symbol/symbol-provider.ts` (proposed) — provider entry + fingerprint cache.
- `apps/code-nav/src/providers/symbol/ctags-runner.ts` (proposed) — source-file enumeration, `ctags -L` execution, NDJSON parse and filter.

## Side Effect Checkpoints
- [ ] `search_text` description updated to name `lookup_symbol`; the previously dangling "심볼 정의 도구" reference resolves.
- [ ] Startup binary check is unchanged and still passes when ctags is present.
- [ ] No `ctags -R` anywhere; node_modules and other excluded dirs never appear in the source-file list.
- [ ] `search_text` path validation still works after any shared `path-guard` changes.

## Acceptance Criteria
- [ ] `lookup_symbol {symbol_name:"WebserverLifecycle", kind:"class"}` returns the definition at `apps/code-nav/src/providers/text-search/zoekt-webserver-lifecycle.ts` with its line number.
- [ ] An absent symbol returns `No symbols found`.
- [ ] A whole-tree lookup on a ~400-file repo completes within a few seconds with no `ctags -R`, and a repeated lookup on an unchanged tree returns from cache without spawning ctags.
- [ ] With no `kind` argument, a lookup of an imported type name does not return the import-alias line (definition whitelist is the default).

## Open Questions
- The ctags `signature` field letter varies by language and build — confirm the `--fields` letter during implementation, and fall back to `pattern` when signature is absent.
- Cache placement: default to **in-memory for the server lifetime** (recommended, simplest). A cache directory shared with the zoekt index storage is a possible later optimization if symbol lookups dominate; not required for v1.
