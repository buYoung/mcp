# [fix] Enforce source boundaries through final consumers

## Work Type
fix

## Current State (As-Is)
- [confirmed] At revision `8b8222003584a3875ce3804557d3d5e70113ea4b`, full indexing uses `entry_path.is_file()` and then `read_to_string()` — Evidence: `index_files_changed_with_reload()` and `apply_index_updates()` in `apps/codemap-search/src/index/engine.rs`.
- [confirmed] The walker filters excluded directories but does not reject every file symlink by its resolved destination — Evidence: `entry_allowed_by_excluded_dirs()` in `src/workspace.rs` and the locked `ignore` 0.4.33 walk semantics.
- [confirmed] Live filesystem tools resolve paths through configured permissions, while search rendering opens stored paths directly — Evidence: `resolve_for_filesystem_tool()` in `src/workspace.rs`, `RenderSource::content()` in `src/tools/search/render.rs`, and `FileOutput::start_symbol()` in `src/tools/search/grouped.rs`.
- [confirmed] A symlinked workspace root is a supported behavior — Evidence: `test_mcp_symlink_workspace_compatibility()` in `tests/e2e/mcp.rs`.
- [inferred] A file link escaping the workspace can feed external source into indexing/search, and a path replacement after indexing can bypass an ingestion-only fix — Confirm by the synthetic external-target and post-index swap cases below.

## Reproduction
- Use an isolated workspace and a sibling directory containing synthetic Rust source with a unique non-secret symbol/body. Create a workspace file symlink to the sibling source and start MCP. Compare `read`, `search`, and `overview`. Expected: workspace-scoped operations do not read or expose the sibling source. Record the pre-fix result rather than claiming the audit executed it.
- Index an ordinary workspace file, then replace it with an external symlink before asking for a stored symbol's source. Expected: no external read or rendered source, even before the watcher refreshes.
- Exercise an internal file link, a symlinked workspace root, broken links, and alias paths. Expected: existing safe navigation remains available and denied/unavailable paths receive explicit diagnostics.
- Repeat swaps between resolution and open under controlled synchronization. On Windows include reparse-point/junction equivalents with a runner configured to create them. Expected: a path race never grants access to a target outside the effective authorized roots.

## Desired Outcome (To-Be)
- Index admission and every affected final source reader enforce the same effective root boundary, including path replacement races.
- MCP search/overview remain workspace-scoped regardless of live-tool `allowed_roots` or `anywhere` settings.
- Live read/find/grep retain their independently configured permissions and safe in-root links remain usable.

## Scope
### In Scope
- Source identity/authorization from walker admission through index parsing, search snippets, grouped declarations, live context, and directly connected caller source reads.
- Auditing and migrating the affected final readers to a shared safe-open contract without losing their caller-specific permission policy.
- Regression fixtures for external links, internal/root links, post-index swaps, path aliases, and platform path semantics.
### Out of Scope
- [hard] Expanding search/overview to allowed external roots or removing the existing explicit CLI indexing-root capability.
- [hard] Changing redaction rules, source-ranking behavior, tool names, argument aliases, or JSON-RPC envelope keys.
- [deferred] A general sandbox for compiler subprocesses and arbitrary third-party tools; preserve their existing documented opt-in boundaries.

## Constraints
- Consume Child 01's owned-index contract without weakening its directory protections.
- Distinguish the authorized source root from the index storage directory. MCP uses its workspace root; CLI `index <dir>` retains explicitly selected roots.
- Do not rely only on string-prefix validation before an unchecked open. Tie validation to the opened source or use platform-safe root-relative opening, and fail closed if safety cannot be established. An identity-only handle/metadata inspection may precede authorization, but no source byte may be read, mapped, parsed, or emitted before the effective-root check succeeds.
- Preserve `workspace`, `allowed_roots`, and `anywhere` for the specific live tool; caller overrides must reach the final consumer rather than be replaced with one global policy.
- Keep source identity, mtime/digest freshness, UTF-8 coordinates, and redaction attached to the exact opened buffer.
- The user authorized targeted tests. Use synthetic external files only; do not probe the developer's real home or credentials.
- Run commands from the repository root. Target-native permission/link behavior is replayed by Child 08 across all seven release targets.

## Related Files / Entry Points
- `apps/codemap-search/src/workspace.rs` — start at path resolution, canonicalization, and walker admission before designing the shared safe-open contract.
- `apps/codemap-search/src/index/engine.rs` — apply the source-root contract at collection and actual content reads.
- `apps/codemap-search/src/tools/search/render.rs` — migrate `RenderSource::content()` without changing snippet/masking semantics.
- `apps/codemap-search/src/tools/search/grouped.rs` — migrate `FileOutput::start_symbol()` and its Rust scope-source fallback.
- `apps/codemap-search/src/tools/live_symbols/` — trace source acquisition for context/relations through final readers.
- `apps/codemap-search/src/callers/` — inspect directly connected source reads for the same authorization gap.
- `apps/codemap-search/tests/e2e/tools.rs` — extend permission and live-result regressions.
- `apps/codemap-search/tests/e2e/mcp.rs` — preserve root-symlink support and verify index-backed outputs.
- `apps/codemap-search/docs/configuration.md` — preserve and clarify the filesystem-permission versus indexing-scope contract.
- `apps/codemap-search/docs/configuration.ko.md` — align the Korean permission guidance.

## Execution Plan
### Stage 1 — Trace and reproduce every affected read route
- Starts when: `docs/briefs/evidence/codemap-prod/01-index-recovery.md` provides a verified local ownership/lock contract and a target replay recipe, and the parent has closed the shared MCP-test write window for Child 07.
- Work: Map caller policy, resolution, open, source identity, masking, and output consumers. Pin external-link and replacement failures, including the race window and positive internal/root-link cases.
- No-op when: Every mapped route already enforces handle-consistent authorization and the negative/positive fixtures prove all behavior without edits.
- No-op handoff: Record the complete proof in `docs/briefs/evidence/codemap-prod/02-path-boundary.md` (proposed); the parent validates it before releasing Child 03, Child 05, and Child 08.
- Deliverable: `docs/briefs/evidence/codemap-prod/02-path-boundary.md` (proposed), containing the reader/policy map, fixture identities, expected/actual results, race synchronization, and chosen safe-open API contract.
- Verify: `Inspect the reader map against the external-link and swap fixture transcripts`; Inputs: index admission, search render/grouped reads, live context readers, internal/root-link positives, and synthetic external negatives; Expected: a nonempty reader inventory and an observed authorization result for every route, not just absent text in an empty response.
- Ends when:
  - [ ] The final read sites and their effective roots are enumerated.
  - [ ] Failure reproduction or a complete no-change proof distinguishes ingestion from final-consumer enforcement.
- Handoff: Stage 2 receives the complete source-open contract from `docs/briefs/evidence/codemap-prod/02-path-boundary.md`.
- Replan when: A supported external CLI root or platform path model cannot be preserved; stop dependent work, return to the parent for a bounded compatibility correction, re-verify, and recalculate handoffs.

### Stage 2 — Integrate the source-open contract atomically
- Starts when: Stage 1 identifies all affected readers and their policies.
- Work: Implement and migrate source admission/readers together. Preserve positive safe links and caller-selected live permissions while rejecting escaped/raced paths before source reaches parsing, masking, or presentation.
- Deliverable: Integrated safe source acquisition and the final reader/API map in `docs/briefs/evidence/codemap-prod/02-path-boundary.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: migrated index/search/live/caller readers and their platform-specific opening code; Expected: exit 0 on the current host with no remaining unchecked route in the recorded reader map.
- Ends when:
  - [ ] Every affected consumer uses the authorized opened-source identity.
  - [ ] An internal/root link and explicitly allowed live external file still reach their correct final consumers.
- Handoff: Stage 3 receives the integrated reader map and fixtures.
- Replan when: Only a pre-open path check is possible or migration would leave an unchecked consumer; stop the handoff and return to the parent for bounded platform-safe implementation work.
- Worker decision: Choose a small colocated capability/validated-open abstraction; add a dependency only if existing platform facilities cannot satisfy the checked-handle contract.

### Stage 3 — Verify final-output boundaries
- Starts when: Stage 2's reader migration is complete.
- Work: Run positive and negative source routes, inspect actual open/deny evidence for the synchronized race, and verify source/masking behavior through MCP. Produce native-target replay instructions without pretending a single-host run covers all targets.
- Deliverable: `docs/briefs/evidence/codemap-prod/02-path-boundary.md`, with integrated revision, root-policy/API contract, nonzero case counts, final-output and open/deny evidence, exact commands, and native replay cases.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests -- e2e::tools:: e2e::mcp:: --test-threads=1`; Inputs: existing tool/MCP populations plus added link/swap cases; Expected: selected tests execute and pass, authorized positives return their known symbols, and denied targets provide zero source bytes to reading/mapping/parsing/output; identity-only metadata inspection is recorded separately.
- Ends when:
  - [ ] Local fixtures, existing compatibility cases, and side-effect checkpoints have executed results.
  - [ ] Remaining platform executions are assigned explicitly to Child 08.
- Handoff: Child 03, Child 05, and Child 08 receive `docs/briefs/evidence/codemap-prod/02-path-boundary.md` and the stable source identity/open contract.
- Replan when: Any final source route leaks an external target or a safe link regresses; return to Stage 2, stop dependent consumers, and re-verify the complete reader map.

## Side Effect Checkpoints
- [ ] Workspace-root symlinks, relative/backslash aliases, and safe internal links retain navigable identities without duplicate documents.
- [ ] Live `allowed_roots`/`anywhere` do not widen search/overview, and restrictive read permissions are not overwritten by shared helpers.
- [ ] CLI explicit indexing roots, index-directory exclusion, freshness notices, line coordinates, and caller relation evidence retain their contracts.
- [ ] Jev and redaction receive only authorized exact-source buffers; fail-closed errors do not contain external file content.

## Acceptance Criteria
- [ ] Every route in the complete reader inventory passes its positive, escaped-target, and replacement-race checks on the host used for implementation.
- [ ] A denied target is proven not to have been read; empty search results alone are not the proof.
- [ ] Existing safe-link and permission regressions pass, and the stable source-open contract and all target-native replay cases are delivered to Child 08.
- [ ] Both configuration documents describe the preserved live-tool policies and the unchanged narrower indexing boundary.

## Open Questions
- None — The workspace/indexing boundary is an existing contract; the implementation and platform verification route are explicit above.
