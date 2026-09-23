# Jev overview recommendation handoff (child 02)

Status: implemented and verified offline on 2026-09-23 on the current `opus-5.5` branch
(nothing staged or committed). It consumes the runtime contract in `01-runtime.md`.

## Entry points

| Item | Signature | Notes |
|------|-----------|-------|
| `tools::overview::run` | `fn run(&ToolContext) -> Result<String, (i64, String)>` | Unchanged behavior; now `prepare(ctx).map(|p| p.text)`. `initial_instructions` keeps using it and never triggers recommendations. |
| `tools::overview::prepare` | `fn prepare(&ToolContext) -> Result<PreparedOverview, (i64, String)>` | Same aliases, errors, notices, stats, and text as `run`. |
| `PreparedOverview` | `{ text: String, root_snapshot: Option<Arc<PublishedIndexSnapshot>>, is_index_warming: bool }` | `root_snapshot` is the exact generation the text was rendered from; `is_index_warming` is the readiness observed before that snapshot was read (added by child 04 for the readiness bypass). |
| `tools::overview::jev::RootCandidates::project` | `fn project(&Arc<PublishedIndexSnapshot>) -> RootCandidates` | Owned projection of every indexed file; `snapshot_identity()` is the snapshot address. |
| `tools::overview::jev::recommend` | `async fn recommend(&RootCandidates, overview_text: &str, &dyn JevEvaluator, RecommendOptions) -> RecommendationOutcome` | Both stages, selection, and rendering. |
| `RecommendOptions` | `{ task_query: String, deadline: tokio::time::Instant, cancellation: Option<CancellationToken>, output_room_bytes: Option<usize> }` | One deadline covers both stages. `output_room_bytes` is the overview cap minus the base text, `None` when uncapped. |
| `RecommendationOutcome` | `Applied(Box<Recommendation>)`, `OutputBudgetBypass`, `Fallback(RecommendationFallback)` | See outcomes below. |
| Replay helpers | `judge_files(&RootCandidates, &BTreeMap<String, Answer>)`, `select_files(&[FileJudgment])`, `recommended_files(...)` | Pure functions over raw answers; no inference. |
| `append_note` | `fn append_note(&mut String, note: &str, output_cap: Option<usize>) -> bool` | Added by child 04: appends a bypass/fallback note only when it fits the overview cap with the same 256-byte redaction margin, so a note can never make the final cap check reject the call. |

## Activation (root scope rules)

`PreparedOverview.root_snapshot` is `Some` only for the default-format root view: `path`
(or `file_path`/`file`/`query`) empty, `"."`, or a root alias such as `all`, and `format`
other than `llms-txt`. It is `None` for folder, file, workspace-folder, `llms-txt`, and the
warming/stopped-indexer notices, and every error path returns before it exists. The
integration must call `recommend` only when `root_snapshot` is `Some`, so those paths keep
their existing output with no API call. An empty committed snapshot (no indexable files,
indexer not warming) yields `Some` with zero candidates; `recommend` then reports
`insufficient_evidence` without sending anything.

Active workspace handling is untouched: the MCP overview arm still calls
`update_active_workspace_scope_from_overview(arguments)` after the tool runs, so a root
request (with or without recommendations) resets the scope as before. The integration must
keep that call after the recommendation step and must not derive scope from Jev output.

## Candidate coverage and evidence

- Every `ExtractedFile` in the snapshot is a candidate (the snapshot is path-sorted);
  there is no BM25, name, or hand-picked prefilter, no filesystem walk, and no source read.
- Declarations follow the PoC evidence rules: skip `mod` symbols, test-flagged symbols, and
  unexported declarations nested inside a callable. Ranges use `end_line_inclusive()`.
- Per file, the Score evidence is JSON with `path`, `lines`, `is_test_file`
  (`index::is_test_like_path`), `declarations` (`Owner.name (kind) Lstart-end`, first 400),
  `docs` (`Owner.name: first doc line`, ≤160 chars, first 64), `calls` (unique
  `receiver.name` call texts, first 24), and `omitted_declarations`/`omitted_docs`/
  `omitted_calls` counts, so caps are explicit rather than silent.
- Evidence is split structurally into fragments of at most 10,000 encoded bytes, each
  repeating the file identity plus `part`/`parts`. Question ids are `f{file:05}p{part:03}`;
  role ids are `f{file:05}d{declaration:04}`. Ids never contain paths.
- Calls are attributed to the smallest enclosing callable and linked to a target only when
  the PoC name/receiver/owner rules leave exactly one candidate. Links are presented as
  possible calls, not verified targets.

## Redaction boundary

Everything sent to the evaluator or rendered passes `redact::presentation_text`, the same
masking the response pass applies to tool text (new `pub(crate)` helper in
`redact/response.rs`, approved as a parent-routed shared edit):

- file paths, and each evidence list (joined per list; per-entry masking if the joined
  result changes line structure), before fragment sizes are measured;
- the task query and the base overview used as `repository_overview` state (≤8,192 bytes,
  cut at a line boundary with `[overview truncated for evaluation]`);
- role-stage declarations via `IndexedDeclaration::masked()` (name, owner, doc, call texts,
  targets, callers), which is also the copy kept in `RecommendedFile`.

Raw index metadata stays in memory only. The snapshot, index, and sources are not modified.
The normal response redaction still runs over the final text.

## Evaluation and selection

- Stage A state: `{task_query, repository_overview, evaluation_scope}`. One Score question per
  fragment with four standalone levels (no useful evidence; tangential/generic wrapper;
  important supporting implementation, configuration, caller or consumer; direct
  implementation). Code keeps the 0–3 order; the level text carries the meaning.
- Qualification: a fragment qualifies when P(2)+P(3) > P(0)+P(1); masses within 1e-9 are a
  tie (uncertain). A file qualifies when any fragment qualifies. Only qualified files are
  ranked, by maximum fragment `score` descending then path, and at most 24 are kept;
  unqualified files never fill unused slots.
- Status: `matched` when any file qualifies; otherwise `insufficient_evidence` when any
  fragment tied or there were no candidates, else `no_match`. Empty results keep the base
  overview and say that indexed evidence did not establish a recommendation, not that the
  implementation is absent.
- Stage B runs only when files were selected and they have declarations. State:
  `{task_query}`. Candidates per selected file: leaf declarations (not containers or `key`),
  or every declaration of a leafless file, ordered by the score of the fragment listing
  them, capped at 32. One Choice question each over `entry`, `producer`, `consumer`,
  `contract`, `configuration`, `support`, `unrelated`, with the declaration's file, range,
  doc (≤768 chars), up to 12 outgoing calls, and up to 5 possible callers.
- Display: at most two non-`unrelated` declarations per file, ordered by lower
  `P(unrelated)`, non-container, shorter span. A qualified file with no supported role stays
  listed with a note. Each declaration shows its range, role, first doc line, up to four
  calls, up to two possible callers, and `read {"file_path":…,"offset":start,"limit":≤180,"view":"source"}`.
- Versions: `overview-file-score-v1`, `overview-declaration-role-v1`, and the experimental,
  uncalibrated selection policy `overview-qualify-max-v1` (`Recommendation::versions()`).
- `Recommendation` keeps `judgments`, `recommended`, both raw `Evaluation`s, the snapshot
  identity, usage, timing, and request count; selection or display changes can be replayed
  from those answers (see the replay test).

## Output reservation and outcomes

- Room: `output_room_bytes − 256` (redaction margin). Below 1,024 bytes the adapter returns
  `OutputBudgetBypass` before any request.
- Applied: the section is rendered within the room minus its note; files that do not fit
  are summarized as `N more recommended files omitted by the overview output cap`. The
  section ends with `[jev overview: applied · status=… · requests=… · input_tokens=… ·
  output_tokens=… · elapsed_ms=… · http_ms=…]`.
- Fallback: any failure in either stage (including deadline, cancellation, provider errors,
  and invalid answers) returns `Fallback { error, stage, usage, timing, request_count }`.
  Usage includes the completed first stage; `request_count` counts completed HTTP
  exchanges only. The integration appends `RecommendationFallback::note()`
  (`[jev overview: fallback (<label>) · original output preserved · …]`) to the unchanged
  base text. A partial evaluation is never turned into a ranking.
- The adapter never retries and holds no cache.

## Verification (2026-09-23)

| Command | Result |
|---------|--------|
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview` | 15 passed, 0 failed |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml` | exit 0; existing `overview::run` callers (MCP overview arm, `initial_instructions`) compile unchanged |

Scenarios (all against the real `Evaluator` with an offline transport): mixed fit with
ranking, roles, call links, and read windows; all unrelated/tangential (`no_match`, no role
stage); tied fragment (`insufficient_evidence`); empty snapshot (no request); 30
high-scoring unqualified plus 26 qualified files (qualification before the 24 cap); a
1,200-declaration file split into fragments with the best fragment driving score and role
order; qualified file without a supported role; snapshot replaced during a delayed
evaluation (result keeps the held generation); small output room (bypass before any
request); rendered section within a tight room; role-stage 529 fallback with both stages
accounted; deadline during file scores; replay from raw answers; explicit `task_query` and
overview state; projection rules and call linking.

Not covered here: `prepare` on folder/file/warming paths needs a live `EngineSupervisor`,
so those paths are verified end to end by the integration and verification children.
Projection cost on very large indexes (hundreds of thousands of symbols) was not measured.
