# [feat] Extend read-only find with fd-style navigation

## Work Type
feat

## Current State (As-Is)
- [confirmed] The author inspected the working tree at `3ce8fb7` on 2026-09-18; existing uncommitted work touches `Cargo.toml`, `Cargo.lock`, configuration, MCP dispatch, and redaction. Preserve that work — Evidence: authoring-session `git status --short` and `git rev-parse --short HEAD`.
- [confirmed] `find_files` accepts `pattern`, `path`, and `include_ignored`, resolves filesystem permissions, walks sequentially, and returns regular files only — Evidence: `apps/codemap-search/src/tools/find.rs`, `find_files` and `build_walker(...).build()`.
- [confirmed] Absolute glob patterns split their static directory prefix into the search base; relative parent-directory components are rejected and Windows separators are normalized — Evidence: `find.rs`, `split_static_prefix`, `resolve_absolute_pattern`, and `find_files`.
- [confirmed] Matching uses the shared gitignore-style `GlobMatcher`, including basename matching, brace alternatives, and leading negation. Its existing `is_match` passes `false` as the directory flag and is also used by grep — Evidence: `apps/codemap-search/src/tools/mod.rs`, `GlobMatcher::is_match` and `build_glob_matcher`.
- [confirmed] Matching paths are sorted by descending mtime, then ascending path, with a 100-result cap and an explicit truncation message — Evidence: `find.rs`, `FIND_FILES_RESULT_LIMIT`, `FIND_FILES_TRUNCATION_MESSAGE`, and the final comparator.
- [confirmed] The shared walker includes hidden entries, honors ignore/configured exclusions, and keeps mandatory internal/index exclusions even with `include_ignored=true` — Evidence: `apps/codemap-search/src/workspace.rs`, `apply_ignore_settings`, `directory_is_excluded`, and `build_walker`.
- [confirmed] `grep`, `ignore`, `globset`, and `regex` are already dependencies; the find MCP annotation is read-only and the dispatcher calls `find_files` directly — Evidence: `apps/codemap-search/Cargo.toml`, embedded-engine dependency block; `src/tools/mod.rs`, find registration; `src/mcp/mod.rs`, find dispatch arm.
- [confirmed] Existing real-stdio coverage exercises find ignore rules, Windows-style patterns, permission rejection, and shared grep behavior — Evidence: `apps/codemap-search/tests/e2e/tools.rs`, `test_find_respects_gitignore_and_excludes_node_modules`, `test_find_include_ignored_bypass`, `test_find_accepts_windows_style_relative_pattern`, and `test_find_path_param_escape_is_rejected`.

## Desired Outcome (To-Be)
- Extend the existing `find` tool with fd-style directory selection, depth limits, and an explicit regex mode while preserving omitted-option behavior.
- Use the existing embedded `ignore` and `regex` libraries; retain a self-contained server without installing, launching, or vendoring the fd CLI.
- Use parallel traversal with deterministic result selection and preserve filesystem permissions and ignore behavior through the final returned paths.
- Return discovery results only; provide no file mutation, command execution, or result-file export capability.

## Scope
### In Scope
- Add `entry_type` with `file`, `directory`, and `all`; default to `file`. `all` means regular files plus directories, not device nodes or a new symlink-following mode.
- Add `max_depth` as an optional non-negative integer, with the supplied base at depth zero and no limit when omitted. Never emit the search base itself; depth zero therefore produces the existing empty-result response.
- Add `pattern_type` with `glob` and `regex`; default to `glob`. Keep `pattern` required and keep existing glob input/output behavior.
- In regex mode, match entry basenames case-sensitively using `regex`; use `path` to select the search root. Preserve regex escapes verbatim and do not run absolute-glob splitting or Windows-separator rewriting on regex text.
- Extend directory-aware glob matching without changing the behavior of existing file-only grep callers.
- Integrate parallel walking and newest-first capped selection without changing default result contents or tie-breaking.
- Update MCP schemas, find usage instructions, and the English/Korean README descriptions together.
- Verify compatibility with existing package checks and trace the new options from MCP input to the final walker, matcher, and output selection.
### Out of Scope
- [hard] Do not add the `fd-find` crate, fd/fdfind executable discovery, binary downloads, shell wrappers, or external process execution.
- [hard] Do not expose fd command execution, deletion, move, rename, permission changes, file creation, or output-file options.
- [hard] Do not add a separate MCP tool or CLI subcommand, change the `find` name, or change its text-content JSON-RPC envelope.
- [hard] Do not add persistent search caches or change existing background index/configuration lifecycle behavior.
- [hard] Do not create or modify test files/cases, fixtures, lint rules, formatter configuration, or verification automation without an explicit later user request.
- [deferred] Regex full-path selection, smart case, symlink following, extension/size/time/owner filters, and configurable result counts require a separate scope decision.
- [deferred] Tokei statistics belong to child 02; eza, ast-grep, and other tool integrations are excluded from this set.

## Constraints
- Preserve the current defaults for `pattern`, `path`, `include_ignored`, glob case sensitivity, basename/path matching, absolute glob prefixes, hidden-file inclusion, and slash-normalized output paths.
- Preserve the exact existing empty-result and truncation messages, newest 100 selection, and ascending-path tie break. Apply the same ordering/cap to explicitly requested directory/all results and display directories with a trailing slash only in the new modes.
- Reject invalid values/types for the newly added parameters with the existing invalid-parameter error convention. Do not silently fall back to defaults for an explicitly invalid new value.
- Do not stop traversal merely because 100 matches have arrived; that would lose newer entries and make parallel results nondeterministic.
- Keep traversal filtering separate from result matching: an unmatched directory must remain traversable when its descendants can match. Apply actual directory flags when matching a directory result.
- Keep existing permission resolution, mandatory exclusions, symlink containment, and ancestor visibility checks authoritative. A depth or regex option must not widen permitted roots or override ignore choices.
- Configure parallelism locally for find; do not globally parallelize grep, index traversal, or the MCP request loop. Keep worker counts bounded and avoid a thread per directory.
- Preserve `readOnlyHint=true`, `openWorldHint=false`, and the central response-redaction path. Treat these annotations as descriptions, not proof of read-only implementation.
- The user's write restriction applies to feature execution: no new runtime disk writes or mutating commands. Existing indexing remains unchanged. Implementation edits, build artifacts, and the required development handoff record are not product write capabilities.
- Announce the exact command and working directory before running existing checks. Record pre-existing failures separately and do not broaden this feature into fixing unrelated redaction/configuration work.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/find.rs` — start with argument parsing, permission resolution, walking, matching, and final ordering.
- `apps/codemap-search/src/tools/mod.rs` — extend only the find schema and any directory-aware matcher API while preserving grep callers.
- `apps/codemap-search/src/workspace.rs` — reuse filesystem permission and ignore/exclusion boundaries; prefer per-call builder configuration.
- `apps/codemap-search/src/mcp/mod.rs` — confirm direct find dispatch and the shared response-redaction envelope survive integration.
- `apps/codemap-search/src/tools/instructions/tools/find.md` — document canonical options, defaults, regex scope, and read-only semantics.
- `apps/codemap-search/README.md` — update the public find description and examples.
- `apps/codemap-search/README.ko.md` — keep Korean public behavior documentation aligned.
- `apps/codemap-search/Cargo.toml` — confirm existing libraries support the implementation without adding fd.
- `apps/codemap-search/tests/e2e/tools.rs` — reuse existing find/grep behavior coverage; do not add cases.
- `apps/codemap-search/tests/e2e/mcp.rs` — reuse existing MCP registration and response coverage.
- `apps/codemap-search/tests/e2e/helpers.rs` — use the existing stdio framing and warm-up behavior as the reference for bounded manual observations.
- `apps/codemap-search/AGENTS.md` — follow the package-local cargo-check requirement.
- `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` (proposed) — record the final options contract, changed paths, inspection evidence, commands/results, timing observations, and residual limitations for child 02.

## Execution Plan
### Stage 1 — Pin defaults and the additive options contract
- Starts when: This brief and the inspected working tree are available; the user has approved fd-style find extensions and preservation of existing indexing.
- Work: Recheck the named entry points against current edits, trace all existing matcher callers, and lock the new options listed in Scope. Capture existing default output and the named e2e baseline before changing traversal. If timing is compared, record the same non-empty repository subtree, query, ignore settings, build mode, machine, and returned population before and after; no speedup is assumed or numerically promised.
- No-op when: The current tree already implements every desired option, preserves all listed contracts, and supplies the complete existing-check and read-only evidence required by this brief.
- No-op handoff: Record that evidence with the same minimum fields at `docs/briefs/evidence/2026-09-18-fd-tokei-find.md`; tell the parent that child 02 may consume it without find edits. If proof fails, retain this child's bounded implementation/correction route and stop its successor until re-verification.
- Deliverable: A baseline and option-contract section in `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` with revision, pre-existing edits, schema defaults, query inputs, observed outputs, and available verification commands.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools::`; Inputs: existing `tests/e2e/tools.rs` coverage in the current checkout; Expected: selected tests are non-empty and pass, or pre-existing failures are explicitly recorded before implementation.
- Ends when:
  - [ ] Default glob, ignore, permissions, ordering, messages, and shared matcher callers are enumerated against the actual working tree.
  - [ ] New option parsing, regex basename semantics, directory display, and depth-zero behavior are fixed without a compatibility break.
  - [ ] The baseline distinguishes observed signals from unavailable measurements.
- Handoff: Stage 2 receives the baseline and fixed options contract from `docs/briefs/evidence/2026-09-18-fd-tokei-find.md`.
- Replan when: Current code differs materially from the documented contracts or implementing directory matching requires changing existing grep behavior; return the affected decision to the parent and revise this child before proceeding.
- Worker decision: Choose bounded worker counts and a local matching/collection design; preserve the observable contract and measure any claimed speed improvement.

### Stage 2 — Implement the read-only traversal path
- Starts when: Stage 1 has pinned the baseline and additive options contract.
- Work: Implement the new argument validation, file/directory filtering, maximum-depth propagation, explicit regex matching, and parallel traversal. Preserve permission resolution and default glob handling through the final consumer. Keep only deterministic newest-first results and accurate overflow reporting; do not introduce an execution or write path.
- Deliverable: Integrated find behavior in the named source files plus an option-to-consumer trace in `docs/briefs/evidence/2026-09-18-fd-tokei-find.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the modified find/matcher/walker code and existing dependencies; Expected: exit code 0. Bounded inspection: trace `entry_type`, `max_depth`, and `pattern_type` through the MCP arguments to traversal/matching and inspect every newly added call reachable from `find_files`; Expected: all options apply and none reaches file writes or process execution.
- Ends when:
  - [ ] All three new options reach their final consumers and invalid explicit values produce parameter errors.
  - [ ] Matching does not prune otherwise searchable descendants, and parallel scheduling cannot change the selected newest 100 paths.
  - [ ] Cargo check passes and the bounded call-path inspection identifies no added mutation or execution capability.
- Handoff: Stage 3 receives the compiled find implementation and the option/write-boundary trace.
- Replan when: An existing library cannot provide the behavior without a new runtime dependency, changed permissions, changed default results, or unbounded traversal resources; return to the parent instead of adding fd or relaxing constraints.

### Stage 3 — Publish the contract and verify the handoff
- Starts when: Stage 2 provides the compiled implementation and recorded call-path trace.
- Work: Align the find MCP schema, instructions, and both READMEs with the implemented options. Run `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools::` and `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp::` from the repository root. Use bounded manual MCP observations on existing repository paths to confirm the advertised options are usable; follow `McpClient` line-framing conventions and the existing `codemap-search mcp` entry point, without adding test cases or automation. Use `path=apps/codemap-search/src/tools` with glob `*.rs`, regex `^find[.]rs$`, and explicit directory/depth options. Confirm that the regex identifies `find.rs` and the requested entry type/depth reaches final output. Record the exact calls and outputs observed rather than claiming existing tests cover new arguments.
- Deliverable: Completed `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` containing revision/changed paths, the final option/default table, existing-check commands and results, observed MCP inputs/outputs, read-only evidence, any timing results, and remaining limitations.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools::`; Inputs: final find/schema/docs state and the existing tools module plus the separately specified MCP/manual checks; Expected: non-empty existing selections pass and recorded manual results preserve default file output while applying regex, entry type, and depth options.
- Ends when:
  - [ ] Schemas, instructions, and both READMEs describe the same canonical parameters and defaults.
  - [ ] Existing checks and observed new-option behavior are recorded separately and accurately.
  - [ ] Every Side Effect Checkpoint is evaluated and the addressable handoff record is complete.
- Handoff: The parent and child 02 consume `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` before changing shared schemas or documentation.
- Replan when: The final integration changes default outputs, redaction, ignore/permission behavior, or tool registration; keep child 02 stopped, correct this child, re-run affected verification, and update the parent's handoff before continuing.

## Side Effect Checkpoints
- [ ] Confirm existing grep calls to `GlobMatcher::is_match` still use unchanged file semantics; existing tools e2e output must remain passing after any shared matcher edit.
- [ ] Trace `include_ignored=false/true` through `build_walker` and preserve mandatory VCS/index exclusions, ancestor visibility, and default generated-file exclusions.
- [ ] Preserve absolute glob support, relative parent-escape rejection, Windows-style glob normalization, and relative/absolute display behavior for allowed external roots.
- [ ] Confirm existing symlink handling and workspace containment are not widened by directory results or parallel callbacks.
- [ ] Verify omitted new options preserve default newest-first contents, tie ordering, the 100-result limit, and exact existing messages.
- [ ] Inspect the full non-empty population of new find call paths and schema properties; there must be no newly reachable file mutation, command runner, export destination, or disk cache.
- [ ] Confirm the shared MCP content envelope and central redaction handler still receive every find response, including directory paths and errors.
- [ ] Keep logs off MCP stdout and keep the existing number and names of registered tools.
- [ ] Preserve the user's existing redaction/configuration changes and avoid incidental Cargo/configuration edits.

## Acceptance Criteria
- [ ] After all stages and checkpoints finish, existing clients omitting the new options receive the same find behavior on the baseline inputs.
- [ ] A caller can select files, directories, or both, constrain depth, and request basename regex matching through the published find schema; the final output demonstrates those choices applied.
- [ ] Parallel traversal preserves deterministic newest-100 selection and respects the same filesystem/ignore boundaries as the baseline.
- [ ] The implementation uses existing embedded libraries and requires no fd executable, fd crate, subprocess, or new runtime disk-write path.
- [ ] Package cargo check and the existing tools/MCP checks pass on the final implementation state; the handoff identifies any verification that was not run rather than marking it successful.
- [ ] `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` gives child 02 the final contract, changed shared files, reproducible checks, observed results, and read-only boundary evidence.

## Open Questions
- None — the user approved the recommended integration direction and preservation of existing indexing; bounded implementation choices are assigned inside this brief.
