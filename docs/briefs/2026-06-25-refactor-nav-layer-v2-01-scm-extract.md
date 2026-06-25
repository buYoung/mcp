# [refactor] Extract inline query strings to per-language symbols.scm files

## Work Type
refactor

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), every language spec in `apps/codemap-search/src/lang/` embeds its tree-sitter query as an inline Rust string constant: `TS_QUERY_STR` (L10–64 in `typescript.rs`), `RUST_QUERY_STR` (L8–56 in `rust.rs`), `PYTHON_QUERY_STR` (L8–26 in `python.rs`), `GO_QUERY_STR` (L12 in `go.rs`), `JAVA_QUERY_STR` (L8 in `java.rs`), `KOTLIN_QUERY_STR` (L14 in `kotlin.rs`), `C_QUERY_STR` (L21 in `c_family/c.rs`), `CPP_QUERY_STR` (L24 in `c_family/cpp.rs`), `ASM_QUERY_STR` (L21 in `asm.rs`).
- No `queries/` directory exists in the repository; all nine query constants live in their respective `.rs` source files.
- Each constant is passed directly to `Query::new(...)` inside a `OnceLock`-based getter (`get_ts_query`, `get_rust_query`, etc.). No `include_str!` is used anywhere in `src/lang/`.
- `apps/codemap-search/src/parser/mod.rs` routes `@symbol.*` and `@literal.*` capture prefixes in a single `QueryCursor` pass (`TreeSitterExtractor::extract`, L34–288). The capture-prefix dispatch — `symbol.`, `literal.string`, `symbol.name`, per-kind `match` arms including `symbol.cfn` and `symbol.asmfn` — is frozen and must not change.
- `apps/codemap-search/src/index/engine.rs` sets `EXTRACTION_FORMAT_VERSION = "v7-owner-tokens-indexed"`. No navigation field exists; the version constant must **not** be bumped in this child.
- No `queries/<lang>/` directories exist on disk for any of the nine languages.

## Behavior Contract
- **Locked behavior**: The `symbols` and `literals` fields in every `ExtractedFile` produced by `TreeSitterExtractor::extract` must be byte-for-byte identical before and after this refactor, for all nine language grammars. Observable extraction output — symbol name, kind, range, owner, export status, flags, and literal text — must not change for any fixture input.
- **Contract artifacts**: The existing inline tests in `apps/codemap-search/src/parser/mod.rs` (L292–865) cover extraction correctness for TypeScript, TSX, JavaScript, Rust, Python, Go, Java, Kotlin, C, C++, and ASM. All of these tests must pass unchanged — no test body may be modified. `cargo test` in `apps/codemap-search/` is the primary verification command.
- **Verification method**: Run `cargo test` in `apps/codemap-search/` before and after the change; the test output must be identical. Additionally, confirm that `cargo build` produces no new warnings. The `EXTRACTION_FORMAT_VERSION` constant in `src/index/engine.rs` must remain `"v7-owner-tokens-indexed"` — any change to that value is a regression indicator for this child.
- **Runtime pass count**: The number of `Query::new(...)` calls and `QueryCursor` passes per file must not increase. Each language spec creates exactly one query at startup via its `OnceLock` getter; after this refactor the getter reads from `include_str!` but otherwise behaves identically.

## Desired Outcome (To-Be)
- A `queries/` directory exists at `apps/codemap-search/queries/` containing nine subdirectories, one per language, each with a `symbols.scm` file holding the content previously held by its corresponding inline constant.
- The nine `symbols.scm` paths (proposed): `queries/typescript/symbols.scm`, `queries/python/symbols.scm`, `queries/go/symbols.scm`, `queries/rust/symbols.scm`, `queries/java/symbols.scm`, `queries/kotlin/symbols.scm`, `queries/c/symbols.scm`, `queries/cpp/symbols.scm`, `queries/asm/symbols.scm`.
- Each language spec file replaces the inline `const *_QUERY_STR: &str = r#"..."#` with `const *_QUERY_STR: &str = include_str!("../../queries/<lang>/symbols.scm");` (or the equivalent relative path that resolves correctly from `src/lang/`). The constant name, type, and usage at the `Query::new(...)` call site remain identical.
- `apps/codemap-search/src/parser/mod.rs` is unchanged — capture-prefix routing, kind dispatch, and `OnceLock` getter call sites require no modification.
- `apps/codemap-search/src/index/engine.rs` is unchanged — `EXTRACTION_FORMAT_VERSION` is not bumped.
- No new fields are added to `ExtractedFile`, `ExtractedSymbol`, or any other type in `src/parser/types.rs`.
- The compiled binary embeds all nine `.scm` files via `include_str!` so no query file path is required at runtime.
- `queries/asm/symbols.scm` is present with the `ASM_QUERY_STR` content, but `queries/asm/` contains only `symbols.scm` — no `tags.scm` or `navigation.scm` is created for ASM in this child.

## Scope
### In Scope
- Create `queries/<lang>/symbols.scm` for all nine languages: typescript, python, go, rust, java, kotlin, c, cpp, asm.
- Replace the nine inline `const *_QUERY_STR` string bodies in `src/lang/typescript.rs`, `src/lang/rust.rs`, `src/lang/python.rs`, `src/lang/go.rs`, `src/lang/java.rs`, `src/lang/kotlin.rs`, `src/lang/c_family/c.rs`, `src/lang/c_family/cpp.rs`, `src/lang/asm.rs` with `include_str!` references to the new `.scm` files.
- Verify that all existing `cargo test` cases in `src/parser/mod.rs` pass unchanged after the substitution.

### Out of Scope
- [hard] Do not create `tags.scm` or `navigation.scm` for any language — those belong to child 02 and children 07–13.
- [hard] Do not create `queries/asm/tags.scm` or `queries/asm/navigation.scm` — ASM is excluded from navigation layer entirely (design doc §8, §9).
- [hard] Do not bump `EXTRACTION_FORMAT_VERSION` in `src/index/engine.rs` — no extraction output changes occur in this child.
- [hard] Do not add, remove, or rename any capture names in the moved query content — the capture strings must be copied verbatim.
- [hard] Do not modify `src/parser/mod.rs` capture-prefix routing — the frozen dispatch table (`symbol.*`, `literal.*`, `symbol.cfn`, `symbol.asmfn`, etc.) is unchanged.
- [hard] Do not touch `src/parser/types.rs`, `src/index/engine.rs`, `src/callers/`, or `src/tools/`.
- [deferred] Adding `tags.scm` and `navigation.scm` for TypeScript — child 02 (`nav-types-callee`) scope.
- [deferred] Adding `tags.scm` and `navigation.scm` for Python, Go, Rust, Java, Kotlin, C, C++ — children 07–13 scope.

## Constraints
- The `include_str!` path must be a compile-time literal resolvable relative to the source file. Each top-level `src/lang/*.rs` file must use a path like `include_str!("../../queries/typescript/symbols.scm")` (two `../` steps: `src/lang/*.rs` → `src/` → crate root `apps/codemap-search/`, then `queries/`). `src/lang/c_family/c.rs` and `src/lang/c_family/cpp.rs` require three `../` steps: `include_str!("../../../queries/c/symbols.scm")` (one extra level because the file lives under `src/lang/c_family/`). Verify the path depth at each call site.
- The content of each `symbols.scm` must be exactly the body of the corresponding inline constant (the text between the `r#"` and `"#` delimiters), with no additions, removals, or whitespace changes. Extraction output must be byte-identical.
- `cargo build` and `cargo test` must both succeed with zero new warnings after the change.
- Child 02 (`nav-types-callee`) depends on `queries/typescript/symbols.scm` existing from this child and reads it via `include_str!` in the concat pass. Do not rename or restructure the path.

## Related Files / Entry Points
- `apps/codemap-search/src/lang/typescript.rs` (existing) — `TS_QUERY_STR` at L10–64; replace body with `include_str!` referencing `queries/typescript/symbols.scm` (proposed).
- `apps/codemap-search/src/lang/rust.rs` (existing) — `RUST_QUERY_STR` at L8–56; replace body with `include_str!` referencing `queries/rust/symbols.scm` (proposed).
- `apps/codemap-search/src/lang/python.rs` (existing) — `PYTHON_QUERY_STR` at L8–26; replace body with `include_str!` referencing `queries/python/symbols.scm` (proposed).
- `apps/codemap-search/src/lang/go.rs` (existing) — `GO_QUERY_STR` at L12; replace body with `include_str!` referencing `queries/go/symbols.scm` (proposed).
- `apps/codemap-search/src/lang/java.rs` (existing) — `JAVA_QUERY_STR` at L8; replace body with `include_str!` referencing `queries/java/symbols.scm` (proposed).
- `apps/codemap-search/src/lang/kotlin.rs` (existing) — `KOTLIN_QUERY_STR` at L14; replace body with `include_str!` referencing `queries/kotlin/symbols.scm` (proposed).
- `apps/codemap-search/src/lang/c_family/c.rs` (existing) — `C_QUERY_STR` at L21; replace body with `include_str!` referencing `queries/c/symbols.scm` (proposed); use three `../` steps (`../../../queries/c/symbols.scm`) because the file lives under `c_family/`, one level deeper than top-level `src/lang/*.rs` which use two `../` steps. See Constraints for the exact path literal.
- `apps/codemap-search/src/lang/c_family/cpp.rs` (existing) — `CPP_QUERY_STR` at L24; replace body with `include_str!` referencing `queries/cpp/symbols.scm` (proposed); use three `../` steps (`../../../queries/cpp/symbols.scm`), same extra level as `c.rs`. See Constraints for the exact path literal.
- `apps/codemap-search/src/lang/asm.rs` (existing) — `ASM_QUERY_STR` at L21–37; replace body with `include_str!` referencing `queries/asm/symbols.scm` (proposed).
- `apps/codemap-search/src/parser/mod.rs` (existing) — capture-prefix dispatch at L83–264; read-only reference; must not be modified.
- `apps/codemap-search/queries/typescript/symbols.scm` (proposed) — new file; content is the body of `TS_QUERY_STR`.
- `apps/codemap-search/queries/rust/symbols.scm` (proposed) — new file; content is the body of `RUST_QUERY_STR`.
- `apps/codemap-search/queries/python/symbols.scm` (proposed) — new file; content is the body of `PYTHON_QUERY_STR`.
- `apps/codemap-search/queries/go/symbols.scm` (proposed) — new file; content is the body of `GO_QUERY_STR`.
- `apps/codemap-search/queries/java/symbols.scm` (proposed) — new file; content is the body of `JAVA_QUERY_STR`.
- `apps/codemap-search/queries/kotlin/symbols.scm` (proposed) — new file; content is the body of `KOTLIN_QUERY_STR`.
- `apps/codemap-search/queries/c/symbols.scm` (proposed) — new file; content is the body of `C_QUERY_STR`.
- `apps/codemap-search/queries/cpp/symbols.scm` (proposed) — new file; content is the body of `CPP_QUERY_STR`.
- `apps/codemap-search/queries/asm/symbols.scm` (proposed) — new file; content is the body of `ASM_QUERY_STR`.
- `docs/briefs/2026-06-25-briefset-nav-layer-v2.md` (proposed) — parent brief for this briefset; see for execution order and set-level acceptance criteria.

## Side Effect Checkpoints
- [ ] All existing tests in `apps/codemap-search/src/parser/mod.rs` (L292–865) pass with no changes to test bodies or assertions — run `cargo test` in `apps/codemap-search/`.
- [ ] `cargo build` in `apps/codemap-search/` produces no new warnings after replacing inline constants with `include_str!`.
- [ ] `EXTRACTION_FORMAT_VERSION` in `src/index/engine.rs` remains `"v7-owner-tokens-indexed"` — confirm the value is unmodified.
- [ ] The `include_str!` path for `c_family/c.rs` and `c_family/cpp.rs` resolves correctly at compile time — three `../` steps (`../../../queries/<lang>/symbols.scm`), one extra `../` level compared to the two-step path (`../../queries/<lang>/symbols.scm`) used by top-level `src/lang/*.rs` files.
- [ ] No capture name, whitespace, or comment is added or removed from the moved query content — the `.scm` file body is an exact copy of the inline constant body.
- [ ] Child 02 (`nav-types-callee`) can read `queries/typescript/symbols.scm` via `include_str!` in its concat pass — the file must exist at the path child 02 expects.
- [ ] The `queries/asm/` directory contains only `symbols.scm` — no stray `tags.scm` or `navigation.scm` is created.
- [ ] `OnceLock` getter functions (`get_ts_query`, `get_rust_query`, etc.) are called and behave identically after the `include_str!` substitution — no getter signatures or call sites in extractor code are changed.

## Acceptance Criteria
- [ ] `cargo build` succeeds with zero errors and zero new warnings in `apps/codemap-search/` after all nine `include_str!` substitutions.
- [ ] `cargo test` in `apps/codemap-search/` passes with the same test results as before the change — no test case fails, no test is added or removed.
- [ ] All nine `queries/<lang>/symbols.scm` files exist on disk at their canonical paths under `apps/codemap-search/queries/`.
- [ ] The content of each `symbols.scm` is byte-identical to the body of its corresponding inline `*_QUERY_STR` constant as it existed before this change.
- [ ] `queries/asm/symbols.scm` exists; `queries/asm/tags.scm` and `queries/asm/navigation.scm` do not exist.
- [ ] `src/index/engine.rs` `EXTRACTION_FORMAT_VERSION` value is unchanged from `"v7-owner-tokens-indexed"`.
- [ ] No field is added to `ExtractedFile`, `ExtractedSymbol`, or any type in `src/parser/types.rs`.
- [ ] A manual extraction run on a TypeScript fixture before and after the change produces identical `symbols` and `literals` arrays in the output JSON.

## Open Questions
- None — this child is a pure file-split refactor with no behavioral decisions. The nine source files, their query constant names and line numbers, the `queries/<lang>/` path layout, the `include_str!` wiring rule, and the ASM exclusion from navigation are all determined by the design doc §3 and the entry-point verification log in `final_plan.md`.
