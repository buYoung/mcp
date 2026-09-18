# [feat] Add read-only tokei statistics to overview

## Work Type
feat

## Current State (As-Is)
- [confirmed] The author inspected the working tree at `3ce8fb7` on 2026-09-18; dependency, configuration, MCP, and redaction files already have uncommitted edits — Evidence: authoring-session `git status --short`; preserve those changes when integrating this feature.
- [confirmed] `ExtractedFile` stores `total_lines`, symbols, literals, docstrings, and navigation data, but no code/comment/blank line breakdown — Evidence: `apps/codemap-search/src/parser/types.rs`, `ExtractedFile`; `src/parser/mod.rs`, assignments using `file_content.lines().count()`.
- [confirmed] `overview::run` reads one published snapshot, resolves workspace aliases, and renders root/folder/file views; its empty warming/dead branch returns a notice rather than a complete map — Evidence: `apps/codemap-search/src/tools/overview.rs`, `run`.
- [confirmed] Workspace summaries derive file/symbol counts and leading languages from indexed files — Evidence: `apps/codemap-search/src/codemap/summary.rs`, `build_directory_summaries`; `src/codemap/monorepo.rs`, `WorkspaceCatalog::new`.
- [confirmed] `PublishedIndexSnapshot` is an immutable generation replaced after successful indexing; `EngineSupervisor::published_snapshot` exposes it to tools — Evidence: `apps/codemap-search/src/index/indexer.rs`, `PublishedIndexSnapshot` and the `Arc::new(published_snapshot)` publication branch; `src/index/supervisor.rs`, `published_snapshot`.
- [confirmed] The overview MCP dispatch invokes `ensure_alive` and `trigger_refresh`; this existing indexing lifecycle is retained by the user's decision — Evidence: `apps/codemap-search/src/mcp/mod.rs`, overview dispatch arm; user decision on 2026-09-18.
- [confirmed] The current overview schema advertises `path` and `format`, with `readOnlyHint=true`; centralized response redaction runs after tool dispatch — Evidence: `apps/codemap-search/src/tools/mod.rs`, overview registration; `src/mcp/mod.rs`, `handle_request`.
- [confirmed] Tokei is not a current package dependency; upstream provides in-memory `LanguageType::parse_from_str`/`parse_from_slice` and `CodeStats` with nested-language blobs — Evidence: `apps/codemap-search/Cargo.toml`; [Tokei LanguageType API](https://docs.rs/tokei/latest/tokei/enum.LanguageType.html#method.parse_from_str) and [CodeStats API](https://docs.rs/tokei/latest/tokei/struct.CodeStats.html).
- [inferred] A tokei-owned directory walk or unqualified disk reread could count files excluded from the codemap or mix newer source with an older published snapshot — Confirm by: Stage 1 traces indexed membership, source-version evidence, and the selected tokei entry point before integration.

## Desired Outcome (To-Be)
- Add opt-in language-level code, comment, blank, and total line statistics to the existing overview tool using an embedded tokei library.
- Count only the indexed physical files inside the resolved overview scope and identify the result explicitly as indexed-file statistics, not whole-repository statistics.
- Keep default overview behavior and its existing indexing lifecycle unchanged; retain statistics only in process memory.
- Make unavailable, stale, unsupported, or partial statistics explicit and prevent stale cache entries from being reported as current complete totals.
- Provide no runtime write, command execution, report export, or persistent-statistics capability.

## Scope
### In Scope
- Add `include_stats` as an optional boolean on overview, defaulting to false; invalid explicit types produce the existing invalid-parameter error convention.
- Include a compact statistics section for resolved root, workspace/folder, and indexed-file views when requested. Support the existing `llms-txt` root format with a bounded plain-text statistics section.
- Return language-level code/comment/blank/total counts, a total row, and physical-file coverage counts. Preserve existing `total_lines` fields and their meaning.
- Derive the candidate population from the same published codemap snapshot and resolved scope used by the current overview response. Do not widen an explicit path or add a whole-repository scan.
- Count original physical source contents once per canonical indexed path; do not run macro expansion or count synthetic expansion buffers as additional files.
- Add a process-local cache tied to workspace, source identity, the relevant snapshot, language classification, and counting options. Reuse completed counts and evict obsolete entries without storing source text indefinitely.
- Handle nested-language blobs without double counting. Use one physical-file language bucket per file in this slice, roll embedded counts into that file's bucket once, and document that bucketing choice.
- Add the compatible tokei crate and lockfile entries, update overview schemas/instructions, and align both READMEs with the actual scope and read-only behavior.
- Reuse existing package validation and bounded manual inspection/observations; do not add automated test cases or fixtures.
### Out of Scope
- [hard] Do not count non-indexed files, bypass index exclusions, alter supported-indexing languages, or change index file-size/encoding rules to make statistics larger.
- [hard] Do not install or execute the tokei CLI, call external commands, load arbitrary tokei user configuration, or let tokei discover its own filesystem population.
- [hard] Do not write statistics to the filesystem, persist a cache, add a report destination, create a database, or add stats fields to serialized index documents solely to retain statistics.
- [hard] Do not add a new MCP tool, a CLI statistics command, a configuration migration, or a default statistics block to existing overview calls.
- [hard] Do not replace the existing indexer/watch/recovery lifecycle, mutate source files, or reimplement fd work owned by child 01.
- [hard] Do not create or modify test files/cases, fixtures, lint rules, formatter configuration, or verification automation without an explicit later user request.
- [deferred] Whole-repository counting, separate embedded-language reporting, per-file ranking tables, history/trends, complexity metrics, eza, and ast-grep are outside this slice.

## Constraints
- Start after consuming `docs/briefs/evidence/2026-09-18-fd-tokei-find.md`; preserve the final find options and shared-file edits recorded there.
- The user selected indexed-file scope and preservation of existing indexing writes. New statistics code must have no independent runtime disk-write path; the cache is memory-only and disappears on restart.
- Preserve `path`/`file_path`/`file`/`query` aliases, workspace ambiguity errors, scope activation, out-of-workspace rejection, file-not-in-codemap errors, and existing warming/dead notices.
- When `include_stats` is absent or false, preserve normal overview output and do not initiate statistics-only source reads or computation.
- Reuse the current immutable snapshot for membership. Treat files missing on disk, unreadable files, unsupported tokei languages, and source-version mismatches as explicit coverage gaps instead of silently recording zero lines.
- Keep successfully cached snapshot-aligned counts attached to their generation; never label live rereads from a different version as statistics for that generation. Compare available source fingerprints/stamps before publishing new counts and define the fallback in Stage 1.
- Limit cache entries to the active indexed population and compact numeric data. Bound per-request work and temporary source buffers using existing file limits; keep expensive collection off the sequential MCP request thread when required by the chosen integration.
- If collection is pending, return an explicit pending/partial notice with the original overview rather than invented totals. A completed result must state indexed, counted, and unavailable-file coverage.
- Keep the statistics section bounded within the applicable overview output policy without removing existing declarations to make room. If language rows are omitted, state the omission and keep totals scoped consistently.
- Use tokei classification/counting as a defined measurement convention, not as a complexity or code-quality score. Preserve current language labels where a mapping exists and document any supported fallback.
- Select a published tokei version compatible with the project's current toolchain and supported targets; verify its APIs/features and minimize unnecessary features. Do not upgrade the project toolchain or tree-sitter family as a side effect.
- Preserve `readOnlyHint=true`, `openWorldHint=false`, the JSON text-content envelope, and centralized redaction for all overview paths, including errors and coverage notices.
- Preserve the user's uncommitted dependency/configuration/redaction changes. Announce exact existing verification commands and working directories before execution.

## Related Files / Entry Points
- `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` (proposed) — consume the predecessor's options, shared-file edits, existing-check results, and read-only contract before starting.
- `apps/codemap-search/Cargo.toml` — add only the tokei library dependency and required features after compatibility review.
- `apps/codemap-search/Cargo.lock` — resolve the selected library without discarding current dependency edits.
- `apps/codemap-search/src/tools/overview.rs` — integrate `include_stats` using the same resolved path and published snapshot as the existing response.
- `apps/codemap-search/src/tools/overview/stats.rs` (proposed) — colocate numeric counting, scope aggregation, coverage reporting, and access to the bounded in-memory cache; split local responsibilities if needed.
- `apps/codemap-search/src/tools/mod.rs` — add the overview flag while preserving child 01's find schema and all existing tool registrations.
- `apps/codemap-search/src/mcp/mod.rs` — trace scope activation, existing index triggers, and centralized response redaction without adding stats-specific writes.
- `apps/codemap-search/src/index/indexer.rs` — inspect immutable generation publication and any necessary memory-only cache attachment/invalidation.
- `apps/codemap-search/src/index/supervisor.rs` — inspect snapshot ownership and restart/drop behavior if process-local cache ownership needs to live here.
- `apps/codemap-search/src/index/engine.rs` — inspect source-version evidence and persisted-document boundaries; do not persist stats there.
- `apps/codemap-search/src/parser/types.rs` — preserve `ExtractedFile::total_lines` and existing serialized shapes.
- `apps/codemap-search/src/workspace.rs` — reuse source read/encoding limits and path identity; do not launch a second independent repository walker.
- `apps/codemap-search/src/codemap/monorepo.rs` — reuse workspace identity and language labels for scoped statistics.
- `apps/codemap-search/src/tools/instructions/tools/overview.md` — document opt-in statistics and indexed-file coverage.
- `apps/codemap-search/src/tools/instructions/tools/overview.monorepo.md` — document the same behavior for explicit workspace scopes.
- `apps/codemap-search/README.md` — update the public overview contract and library integration description.
- `apps/codemap-search/README.ko.md` — keep the Korean contract aligned.
- `apps/codemap-search/tests/e2e/codemap.rs` — reuse existing root/folder/file and format coverage.
- `apps/codemap-search/tests/e2e/mcp.rs` — reuse existing protocol/registration coverage.
- `apps/codemap-search/tests/e2e/tools.rs` — rerun existing find/grep checks after shared schema integration.
- `apps/codemap-search/tests/e2e/helpers.rs` — reference real-stdio framing, indexed readiness, and existing fixture paths for bounded manual observations.
- `apps/codemap-search/AGENTS.md` — follow the package-local cargo-check requirement.
- `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md` (proposed) — record the dependency selection, population/version/cache contract, final options, numeric observations, commands/results, write-boundary evidence, and limitations for global acceptance.

## Execution Plan
### Stage 1 — Pin the indexed statistics and cache contract
- Starts when: `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` is available with the integrated find contract and verification results; the user has selected indexed-file-only statistics and preservation of existing indexing.
- Work: Consume the predecessor record, recheck current shared-file edits, and trace overview scope resolution to published membership and source-version evidence. Select a compatible tokei release and an in-memory counting API. Define cache ownership, invalidation, pending/partial behavior, language bucketing, output limits, and the exact read-only call path before editing. Bound this investigation to the named integration surfaces; record technical choices in the evidence artifact instead of interviewing the requester about implementation details.
- No-op when: The current tree already supplies the complete opt-in indexed statistics contract, in-memory-only cache, preserved defaults, and all existing-check/read-only evidence required here.
- No-op handoff: Record the completed evidence at `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md` and send it to parent-level global acceptance without unnecessary feature edits. If the proof fails, continue this child's bounded correction/verification route and stop global completion until evidence is renewed.
- Deliverable: The integration contract in `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md`, naming revision, chosen dependency/version/features, indexed population, source identity, cache bounds/lifecycle, counting convention, pending/partial output, and affected consumers.
- Verify: `Inspect overview::run, PublishedIndexSnapshot, EngineSupervisor::published_snapshot, source-version evidence in src/index/engine.rs, and the selected tokei LanguageType/CodeStats APIs`; Inputs: the predecessor record and the named source/API surfaces; Expected: one explicit route from resolved indexed scope to numeric counts exists without an independent walker, CLI process, persisted stats field, or disk cache.
- Ends when:
  - [ ] The dependency/API choice is compatible with the current toolchain or any incompatibility has been routed to a bounded replan.
  - [ ] Indexed membership, source version, physical-file deduplication, nested-language bucketing, and coverage gaps have explicit contracts.
  - [ ] Cache ownership, invalidation, bounds, and pending behavior are fixed and can be verified through named consumers.
- Handoff: Stage 2 receives the complete integration contract from `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md`.
- Replan when: Available source evidence cannot prevent mixed-version totals, the dependency requires a toolchain change, or the only proposed integration requires disk persistence or a wider population; return to the parent with a bounded alternative before changing those boundaries.
- Worker decision: Choose lazy versus existing-background-thread collection and memory-only cache placement within the stated scope, responsiveness, freshness, and default-off constraints; do not create a new persistence scheme.

### Stage 2 — Integrate counting and the overview consumer
- Starts when: Stage 1 provides the dependency, population, freshness, cache, and output contracts.
- Work: Add the selected tokei crate, implement counting over the approved physical-source inputs, and integrate the bounded cache with overview. Carry `include_stats` from the published schema to the final renderer; aggregate only the selected indexed scope and preserve normal output when disabled. Keep ready counts, pending work, and unavailable-file coverage distinguishable without exposing raw source in the statistics section.
- Deliverable: A compiled opt-in statistics path plus a source-to-count-to-render trace in `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the resolved dependency/lockfile and modified statistics/overview/cache integration; Expected: exit code 0. Bounded inspection of every new counting/cache call and storage field; Expected: in-memory numeric state only, no new write/process capability, and no changes to persisted stats-free index shapes.
- Ends when:
  - [ ] Requested counts use the approved indexed population and cannot silently mix source versions.
  - [ ] Repeated requests can reuse cached numeric results, obsolete entries are bounded/evicted, and process restart does not load a statistics file.
  - [ ] Disabled calls preserve existing output and do not initiate statistics-only work.
  - [ ] Cargo check and the full new-call-path inspection are complete.
- Handoff: Stage 3 receives the integrated statistics path and its numeric, cache, and read-only trace.
- Replan when: Counting changes the existing index population, default response, scope behavior, or index serialization, or new call paths perform filesystem writes; correct this child and re-verify before documenting the feature as available.

### Stage 3 — Verify numeric results and shared contracts
- Starts when: Stage 2 provides the compiled opt-in implementation and recorded data-flow trace.
- Work: Update both overview instruction files and READMEs. Run `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::codemap::`, `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp::`, and `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools::` from the repository root. Observe `include_stats` omitted/false/true on existing root, scoped directory, and indexed-file inputs; use existing checked-in source files and the established real-stdio protocol rather than adding cases or fixtures. Use an indexed existing file such as `apps/codemap-search/tests/fixtures/event_navigation/known.ts` and its containing scope. Record actual eligible paths, source contents/counting convention, and response values for this small non-empty population. Compare scalar totals and confirm that false/omitted output has no statistics block. Inspect cache invalidation and partial-result paths against existing source-change/index-publication behavior, distinguishing inspected guarantees from runtime observations.
- Deliverable: Completed `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md` with final dependency/options, exact indexed inputs, observed numeric results, default-output comparison, cache/freshness evidence, existing-check commands/results, write-boundary trace, and remaining limitations.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::codemap::`; Inputs: final integrated source/schema/docs and the existing codemap module plus the separately specified MCP/tools/manual checks; Expected: all selected existing checks have non-zero populations and pass, observed statistics match the approved indexed inputs/counting convention, and disabled output has no statistics block.
- Ends when:
  - [ ] Numeric observations name their actual non-empty inputs and do not count excluded or non-indexed files as zero-valued successes.
  - [ ] Cache freshness, pending/unavailable coverage, existing output formats, and the central redaction route have recorded evidence.
  - [ ] Existing package checks pass and all Side Effect Checkpoints have been evaluated on the final integrated state.
  - [ ] Public documentation matches the final library/options/scope behavior and makes no unmeasured speed claim.
- Handoff: Parent-level global acceptance consumes `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md` together with the predecessor record; no extra verification child is required.
- Replan when: Final integration breaks find, default overview, snapshot freshness, counting integrity, or the write boundary; stop global completion, correct the responsible child, repeat affected checks, and refresh both handoffs before the parent closes.

## Side Effect Checkpoints
- [ ] Preserve child 01's find parameters, default behavior, read-only annotation, instructions, and README content after editing shared files.
- [ ] Preserve overview aliases, workspace scope selection/ambiguity, `llms-txt`, file-not-in-codemap handling, and warming/dead notices when statistics are disabled.
- [ ] Confirm statistics candidate paths come from the same scope-filtered snapshot as the response, never from an independent tokei directory traversal.
- [ ] Confirm original physical files are counted once, generated macro buffers are not separately counted, and child blobs are not added twice.
- [ ] Keep existing `total_lines` values and persisted `ExtractedFile`/Tantivy schemas unchanged by statistics storage.
- [ ] Trace cache invalidation for new snapshots, changed/deleted files, changed language/counting options, and engine restart; stale/unsupported/unreadable inputs remain visibly incomplete.
- [ ] Inspect memory ownership and bounded collection so cached raw source and obsolete generations cannot accumulate indefinitely.
- [ ] If supervisor/indexer ownership is touched, preserve watcher/config-watcher-before-indexer shutdown ordering and reuse existing watcher verification only where those paths changed.
- [ ] Inspect the full non-empty set of new stats call paths and schema fields; none may invoke filesystem mutation, external process execution, report export, or disk-cache persistence.
- [ ] Preserve the central response-redaction boundary and keep diagnostics off MCP stdout; statistics expose counts and coverage, not raw source.
- [ ] Preserve existing uncommitted dependency/configuration/redaction work and do not add a stats-driven config migration.

## Acceptance Criteria
- [ ] After every stage and checkpoint finishes, existing overview calls with `include_stats` absent/false keep their current behavior and do not initiate statistics-only work.
- [ ] With `include_stats=true`, ready root/folder/indexed-file views provide tokei-derived counts for their indexed physical-file population with explicit scope and coverage.
- [ ] Totals follow the documented counting convention, nested content is not double counted, and incomplete coverage is distinguishable from a complete zero result.
- [ ] Repeated calls reuse memory-only cached counts where valid; new generations and source-version mismatches cannot silently reuse incompatible totals.
- [ ] Tokei works as an embedded compatible library with no CLI dependency, new filesystem-write path, serialized stats storage, or statistics export capability; existing indexing remains intact.
- [ ] Package cargo check and the existing codemap/MCP/tools checks pass on the final integrated state, with actual observations and unverified limits accurately separated.
- [ ] `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md` provides the final dependency, schema, numeric/cache evidence, read-only trace, and shared-feature verification needed to close the parent.

## Open Questions
- None — the user selected indexed-file scope and asked to match the existing indexing-only write boundary; implementation choices and technical uncertainties have bounded execution routes.
