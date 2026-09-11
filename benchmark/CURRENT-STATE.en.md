# codemap-search current state and format for follow-up reports

Recorded: **2026-09-10 (KST)**. Before starting the next round of candidate exploration, implementation, or evaluation, read this document, the [reporting rules](REPORTING.md), and the [baseline registry](current-baseline.json). Subsequent user instructions take precedence.

## Current execution path — Grafana CUI

`pnpm bench:ready` prepares the pinned Grafana source, a release build of the current product working files, and the index. `pnpm bench:start` uses Inquirer to select candidate exploration (3 questions × 3 runs), error checks (10 questions × 1 run), or formal evaluation (30 questions × 2 runs), along with A, the current version, and registered candidates. Nested subsets of the existing question objects are fixed; they are distinct from the historical 6 preparation questions below. Follow the [current usage, resumption, and preservation rules](README.md).

The Codex version is not pinned; the version actually used is recorded. Guidance for A tools, 30-second usage recovery, citation ranges, and anonymization improvements were extracted into shared code. To respect the grading-input limit for formal questions, evidence, explicit citation ranges, and surrounding source are supplied without changing the original questions, answers, responses, or citation ranges. No new model performance measurements were run as part of this harness implementation work.

Historical result tables and causal analyses are records written at the time. Because the raw data is unavailable, do not treat them as findings newly verified against logs. Preserve the fixed A table, original questions, and new harness. The historical mandatory comparison columns A/B-1/B-4/B-6 were the reporting contract at that time; the new CUI uses the comparison targets selected by the user.

## Formal evaluation target — Grafana only

The user's final formal benchmark target is **Grafana only**, comparing **A with the currently approved product**. Preparing pinned Grafana source through `pnpm bench:ready` matches this scope. Do not expand preparation to 6 repositories to match historical multi-repository results. Fix the questions, repetitions, and execution contract separately; this scope correction alone does not authorize new model runs.

The 54/60 correct answers in `formal-r1-A` below are historical results for Django, Kubernetes API, Axum, Vite, Gson, and fmt; they are **not a formal Grafana A baseline**. Preserve the original values and pending judgments, but do not insert them into Grafana result tables. In this inspection, `formal-r1` in the top-level frozen records covered multiple repositories, and the 3 Grafana `preparation` rounds were preparation runs. Do not relabel Grafana candidate or preparation evaluations as completed formal Grafana results. No completed Grafana-only formal A result was found within the inspected records.

## Preserved historical multi-repository reference — `formal-r1-A`

Keep the [formal A result table](results/formal-r1/A.md) and [raw values and source hashes for 60 runs](results/formal-r1/A.json) fixed, as requested by the user. Continue labeling them as historical A runs from before the guidance improvements; do not automatically rerun, regrade, or overwrite them with new results. Retain the original 2 pending judgments and tool-guidance error. This preservation request neither removes known evaluation errors nor approves these runs as a control proving current product improvements. Distinguish the currently approved A tool descriptions from those actually used in these historical runs.

## Current product integration status — 2026-09-10

The current source is **B-4 + member-scope/both attachments in grep/read + subfolder-scope preservation and regex guidance**. The user-approved configuration was integrated in `ee1774533`, and 4 omitted B-4-based files were recovered in `f27ca5edc`. The current product source reference is `f27ca5edc1258bcdc4483fda15905bfc96e94a89`; the `apps/codemap-search` Git tree is `e09df87e2e0c7ff70237475f550cf479eca467ac`.

- `grep/read` outputs `# symbols` first and `# results` afterward. It supplies class/struct/impl members and callers/callees from the same file, associating Go methods with their receiver's struct.
- The scope/guidance configuration corresponds to `scope_regex-both`. B-6 search changes, Smart routing, the separate language-group patch (`language-both`), and experimental environment-variable branches were not included in the final product.
- At integration, 47 existing e2e checks and 84 direct output comparisons against the approved candidate were checked. After recovering the omitted B-4-based files, 38 related existing e2e checks, `cargo check`, and the commit-hook commands `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed. These verify implementation, not new model performance.
- At this inspection, the local `target/release/codemap-search` matched the historical B-4 binary hash `ee5883514ef219d373adfdf64a453d979f5887fc6841bb3df7fb6405b4430d79`. Do not use it as evidence that the current source was built or deployed. The [baseline registry](current-baseline.json) distinguishes current source from the previously measured binary.

Adoption was a decision approved by the user. The single Go question and 5-run results below do not establish general improvements in accuracy or tokens. B-4 in historical tables means the version pinned for each run; do not replace those values with new measurements of the currently patched product.

### Remaining-change cleanup and verification

The benchmark preparation command (`bench:ready`), optional usage-collection improvements, formal evaluation runner and original data, causal analysis, and resumption records were preserved. Meaningless README strings, IDE mappings for benchmark clones, and the Serena project configuration awaiting deletion were cleaned up. Usage collection also treats non-string answer timestamps as a timestamp-validation failure and excludes the affected answer.

From the repository root, 33 existing offline checks in `python3 -m unittest benchmark.v2.checks`, `python3 -m benchmark.v2.formal validate --experiment benchmark/v2/artifacts/formal-r1`, `py_compile` on 5 changed/new Python files, and existing-clone validation through `benchmark.ready.validate_grafana` passed. The 30 original formal questions, 70 facts, and 94 metrics were preserved, and the dataset JSON matched the experiment originals. This cleanup did not change product source, run new model solutions or grading, or alter original scores. Passing offline and contract checks does not establish successful collection in new model runs or semantic correctness of natural-language answers.

## Latest assessment — established causal findings and unresolved scope

**The currently adopted configuration is described above. Guidance and search expressions were shown to affect tool choice and target-code exposure, but the hypothesis that poor tool use is the main cause of high total token use and low accuracy was not established.** Do not equate tool non-use, information generation, actual delivery, cost reduction, and correct answers. The earlier symbol experiment `r1` differed from the user's intended output order and member hierarchy, so distinguish it from the corrected `r2`.

### Latest completed — 25 follow-up patch runs based on member scope/both (`b4-member-patches-r1`)

- Starting from B-4 `b4-live-symbols-r2/group-both`, scope preservation, regex guidance, language-specific grouping, scope+guidance, and all three were measured 5 times each, for 25 runs. The same `complex-go-1`, Luna medium, and existing prompt, source, index, and limits were retained. The existing 5 group-both runs were reused as a historical reference.
- Completed 25 solution runs, 5 Astra high grading runs, and verification of 20 fixed controls. Total usage was obtained for 23/25 runs; there were 3 empty answers and 1 pending judgment. Missing costs or scores were not filled arbitrarily, and solutions or grading were not rerun.

| Candidate | Confirmed correct | Partially correct | Incorrect | No answer | Pending | Average total tokens |
|---|---:|---:|---:|---:|---:|---:|
| Existing member/both (earlier r2) | 3/5 | 1/5 | 1/5 | 0/5 | 0/5 | 319,993.4 |
| Scope preservation | 0/5 | 4/5 | 0/5 | 1/5 | 0/5 | Unconfirmed, ≥359,856.2 |
| Regex guidance | 3/5 | 1/5 | 0/5 | 1/5 | 0/5 | Unconfirmed, ≥341,719.8 |
| Language-specific grouping | 3/5 | 1/5 | 0/5 | 1/5 | 0/5 | 410,003.6 |
| Scope+guidance | 2/5 | 2/5 | 0/5 | 0/5 | 1/5 | 310,625.6 |
| All three | 1/5 | 3/5 | 1/5 | 0/5 | 0/5 | 383,640.2 |

- **Verified output-level effects:** Of 27 actual searches, the scope patch changed responses in 3 explicit subfolder searches in repetition 4 of scope+guidance. Original responses matched replay results. Direct fixed checks verified implicit and explicit scopes, file-parent scope preservation, and the all reset.
- **Exposure of the language patch in this experiment:** There were 0 on/off response differences across 185 actual grep/read calls. Do not treat this as identifying a Go performance effect. Direct checks for the existing 8 languages recovered missing members in 2 Rust impl-header read/grep cases. They did not verify all language syntax.
- **Regex guidance:** Syntax errors occurred in 0 of 15 runs with guidance and 2 of 10 without it, but both sides of the pre-fixed guidance-addition comparison had 0/5 errors. Do not conclude that all forms of misuse decreased or that accuracy/cost improved causally.
- **Remaining issues:** Some answers omitted facts despite receiving all evidence, omitted supplied constant-definition lines from citations, or used `...` as citation destinations. Distinguish improved search results from complete final answers and successful citations.
- Scope+guidance had the lowest average tokens in this experiment, but only 2 correct answers and 1 pending judgment. Neither quality/cost superiority of all three patches nor stable improvement over the existing reference was established. Do not treat the older reference and new candidates as paired repetitions.
- Records include the [pre-run protocol and resumption rules](B4-MEMBER-PATCHES-R1.md), execution command, final report, 94 metrics, and completion verification. Replay error-type handling and recovery of 27 existing communication responses were also recorded. Active product code was not changed during the experiment. Scope+guidance was subsequently approved and integrated into the product described above; original experimental results remain preserved.

### Previously completed — language/declaration-based Smart routing, 5 new runs (`b4-smart-symbols-r1`)

The user-requested combined candidate was implemented on B-4 and measured in **5 new runs** on the same `complex-go-1`. The existing r2 member-scope/both and function-scope/both results, 5 each, were reused without rerunning or regrading. **This combination was not confirmed as an improvement.** Smart routing achieved 1/5 correct answers and an unconfirmed average total-token count (≥352,163.2), with lower observed quality and at least 10.0% higher average cost than the existing member scope's 3/5 correct answers and 319,993.4 tokens. Active B-4 and existing experiments were preserved.

| Metric | class/struct/impl members · both | method/function · both | Smart routing · both |
|---|---:|---:|---:|
| Run origin | Existing r2, 5 runs | Existing r2, 5 runs | 5 new runs |
| Partially correct | 1/5 | 4/5 | 3/5 |
| Confirmed correct | 3/5 | 0/5 | 1/5 |
| Incorrect | 1/5 | 0/5 | 0/5 |
| No answer | 0/5 | 0/5 | 1/5 |
| Pending judgment | 0/5 | 1/5 | 0/5 |
| Key facts satisfied | 13/15 | Unconfirmed (includes pending) | 10/15 |
| Citation evidence satisfied | 11/15 | Unconfirmed (includes pending) | 9/15 |
| Average total tokens | 319,993.4 | 290,185.6 | Unconfirmed (≥352,163.2) |
| Complete usage | 5/5 | 5/5 | 4/5 |

Function/method declarations and bodies in returned lines use function scope; class/struct/impl declarations, fields, and similar structures use member scope. Mixed windows combine position-specific selections and deduplicate symbols. Routing uses Go receiver associations and existing language-specific declaration types; Rust impl blocks without separate indexed symbols associate existing members through AST boundaries. Visibility is not a routing criterion. The `# symbols`-before-`# results` order and existing byte limits were retained. Indexed content, extractors, and BM25 ranking were unchanged.

Direct checks passed for 59 routing cases × 4 modes using the existing 8-language examples and the User example, and 14 fixed Grafana requests × 6 modes. Both existing fixed modes were byte-identical to the previous r2 binary. In the actual 5 new runs, there were **23 function-only, 0 member-only, 33 mixed, and 25 unclassified responses**; complete delivery was confirmed for 75 of 81 generated symbol sections. Member scope appeared in mixed responses, but no member-only switch was observed in model runs. This distribution alone does not establish mixed output as the cause of higher cost.

Verified 5 new solution runs, 1 anonymous grading run, and 4 fixed controls, with 0 automatic reruns. Complete usage was confirmed for 4/5 runs; 1 limit-terminated run remained unconfirmed, and 1 no-answer run was included. The existing function candidate's 1 pending judgment remained unchanged. These measurements occurred at different times; do not pair repetition numbers for causal tests or generalization. No new A/B-1/unattached B-4/B-6 runs were added.

Final conclusions · Complete table · Fixed conditions · Baseline reuse · Actual routing · Completion verification

### Previously completed — symbols first with member hierarchy, 45 new runs (`b4-live-symbols-r2`)

Completed **45 new solutions of the same difficult question, `complex-go-1` (9 candidates × 5 runs)** with the user-confirmed `# symbols`-before-`# results` structure. Go structs and receiver methods were grouped, and method-local variables and visibility were distinguished from member groups. Before model execution, the contract was checked directly against 154 fixed Grafana responses and 18 candidate responses for the user's `User` example. Initial instructions and complete delivery of at least one symbol section were confirmed in 45/45 actual solution runs. Of 874 generated calls, complete section delivery was confirmed within the corresponding call for 751.

**The most promising observation was member scope+both: 3/5 correct answers and 319,993.4 average tokens.** Function scope+both had the lowest average at 290,185.6 tokens, but 0 confirmed correct answers and 1 pending judgment. Do not automatically adopt a candidate or conclude stable improvement.

- Keeping internal symbols+call relationships, **whole file → function scope** yielded **lower tokens in 5/5 runs**, with an average reduction of **at least 38.5%**. The whole-file average is unconfirmed (≥472,570.8), so the reduction is a lower bound. Whole-file scope had 1/5 correct answers versus 0 confirmed and 1 pending for function scope; do not use this as evidence of quality improvement.
- Providing both, **whole file → member scope** yielded **lower tokens in 4/5 runs** and correct answers **1/5 → 3/5**. The cost direction was not the same in every repetition.
- At member scope, **internal symbols only → both** yielded correct answers **0/5 → 3/5**, with lower tokens in **3/5 runs**.

| Candidate | Partially correct | Correct | Incorrect | No answer | Pending | Key facts | Evidence satisfied | Average total tokens | Complete usage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Whole file · internal symbols | 3/5 | 0/5 | 1/5 | 1/5 | 0/5 | 8/15 | 4/15 | Unconfirmed (≥410,802.0) | 4/5 |
| class/struct/impl members · internal symbols | 2/5 | 0/5 | 2/5 | 1/5 | 0/5 | 9/15 | 2/15 | Unconfirmed (≥325,142.8) | 4/5 |
| method/function · internal symbols | 1/5 | 1/5 | 2/5 | 1/5 | 0/5 | 9/15 | 6/15 | 377,243.2 | 5/5 |
| Whole file · caller/callee | 3/5 | 0/5 | 0/5 | 2/5 | 0/5 | 8/15 | 4/15 | Unconfirmed (≥487,106.2) | 4/5 |
| class/struct/impl members · caller/callee | 3/5 | 1/5 | 0/5 | 1/5 | 0/5 | 12/15 | 6/15 | 392,415.2 | 5/5 |
| method/function · caller/callee | 0/5 | 2/5 | 2/5 | 1/5 | 0/5 | 11/15 | 9/15 | 361,634.6 | 5/5 |
| Whole file · both | 3/5 | 1/5 | 0/5 | 1/5 | 0/5 | 10/15 | 7/15 | Unconfirmed (≥472,570.8) | 4/5 |
| class/struct/impl members · both | 1/5 | 3/5 | 1/5 | 0/5 | 0/5 | 13/15 | 11/15 | 319,993.4 | 5/5 |
| method/function · both | 4/5 | 0/5 | 0/5 | 0/5 | 1/5 | Unconfirmed (pending judgment included) | Unconfirmed (pending judgment included) | 290,185.6 | 5/5 |

The 4 adjudicated answers from function scope+both contained **12/12 correct key facts, but citations satisfied only 4/12**. Facts from the 1 pending answer were not included in confirmed totals. Among all adjudicated answers, **2 explanation omissions in 2 answers occurred after all required evidence sets had been received**: 1 each for the exact allow-all comparison condition and the generated SQL/final GetArgs execution path for team search. Also, **39 correct facts across 23 answers lacked sufficient citations**, and complete required evidence had been delivered beforehand for 20 of those facts. Source delivery does not prove model understanding.

Completed 45 new solution runs, 9 anonymous grading runs, and 36 fixed-control judgments, with 0 automatic reruns. Complete usage was **confirmed for 41/45 runs**; 4 remained unconfirmed, and 8 runs had no answer. Of 37 valid responses, 1 containing an unverifiable citation path was left pending. The initial aggregator's handling of pending values was corrected by propagating uncertainty, preserving original and intermediate data. 2 anonymization warnings were classified as path-format differences where candidate identifiers were already hidden; citations were not edited.

**This experiment did not establish the causal effect of placement before source, symbol attachment itself, or index changes.** Compared with r1, not only order but hierarchy, member scope, signatures, and visibility display changed. Indexed content, extractors, and BM25 ranking stayed fixed; Go display information was constructed from the returned files' syntax trees. No new unattached B-4 or A/B-1/B-6 runs were added. Retain the limitations of one question, 5 runs, missing usage, and pending judgments. Do not combine scores with the preceding 45 runs.

Conclusions · Full report · Repetitions and key values · 18 comparisons · Source-delivery checks before answers · Actual User output · Aggregation corrections · Completion verification

### Previously completed — flat symbol attachments after source, 9 candidates × 5 runs (`b4-live-symbols-r1`)

**Correction to the request interpretation:** Actual r1 output below was a flat list appending `## Indexed internal symbols` and `## Indexed caller/callee context` after the source. The existing `type` scope also included local declarations from methods sharing the receiver. These scores therefore describe that implementation; do not cite them as performance of the user-confirmed symbols-first/struct-member-hierarchy output.

Completed **45 new solutions** of the user-requested difficult question `complex-go-1` and **9 anonymous grading runs**. All **36 fixed-control judgments** passed verification, with 0 automatic solution or grading reruns. Total tokens were **confirmed for 42/45 runs**; the remaining 3 retained observed lower bounds. Both 40 valid final answers and 5 no-answer runs were included in denominators. This was a separate experimental implementation preserving the active product and original B-4, with existing tools, guidance, automatic root overview, and index retained. The user confirmed interpreting Go receiver types as the counterpart of class/impl.

**Key differences repeated across these 5 runs:**

- **Internal symbols only**, narrowing type → function scope: **tokens increased in 5/5 runs**, with a sample-average increase of **at least 57.8%**. The final repetition's unconfirmed cost still established the increase from its lower bound alone. Correct answers changed 0/5 → 1/5, and key facts 11/15 → 8/15.
- **Internal symbols+caller/callee**, narrowing type → function scope: **tokens decreased in 5/5 runs**, with a sample-average reduction of **30.7%**. However, correct answers fell 1/5 → 0/5 and key facts 13/15 → 11/15.
- **Type+internal symbols**, which used the fewest tokens, had **3/5 incorrect answers**. Do not classify low cost alone as successful improvement. The observed direction of the same scope reduction reversed depending on the information combination; this does not establish causal effects across other questions or models.

| Scope | Attachment | Partially correct | Correct | Incorrect | No answer | Key facts | Evidence satisfied | Average total tokens | Complete usage |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Whole file | Internal symbols | 1/5 | 1/5 | 1/5 | 2/5 | 5/15 | 5/15 | Unconfirmed (≥398,566.8) | 3/5 |
| class/impl · Go receiver | Internal symbols | 2/5 | 0/5 | 3/5 | 0/5 | 11/15 | 5/15 | 241,640.2 | 5/5 |
| method/function | Internal symbols | 1/5 | 1/5 | 1/5 | 2/5 | 8/15 | 6/15 | Unconfirmed (≥381,510.6) | 4/5 |
| Whole file | caller/callee | 5/5 | 0/5 | 0/5 | 0/5 | 12/15 | 4/15 | 379,320.8 | 5/5 |
| class/impl · Go receiver | caller/callee | 4/5 | 1/5 | 0/5 | 0/5 | 12/15 | 11/15 | 391,677.4 | 5/5 |
| method/function | caller/callee | 4/5 | 0/5 | 0/5 | 1/5 | 11/15 | 8/15 | 372,191.2 | 5/5 |
| Whole file | Both | 5/5 | 0/5 | 0/5 | 0/5 | 10/15 | 7/15 | 436,927.6 | 5/5 |
| class/impl · Go receiver | Both | 4/5 | 1/5 | 0/5 | 0/5 | 13/15 | 4/15 | 375,266.8 | 5/5 |
| method/function | Both | 4/5 | 0/5 | 1/5 | 0/5 | 11/15 | 6/15 | 260,190.2 | 5/5 |

Of 6 incorrect answers, 4 confused organization-user search with global-user search, and 2 incorrectly generalized wildcard allow-all/deny-all conditions. Also, **37 correct facts across 25 answers failed citation-evidence requirements.** These are grading findings; do not assume all occurred after every required source had been received.

Direct MCP checks confirmed preservation of source, tool inventory, initial instructions, and non-target tool output, along with distinctions between combinations. Initial instructions and complete delivery of at least one attachment were confirmed in 45/45 actual solution runs. Attachment generation and complete delivery within the corresponding call are counted separately, distinguishing empty sections and output-cap omissions. The initial delivery audit searched whole-session strings, which could misattribute repeated text from another call; its original was preserved and supplemented with per-exec/wait comparisons. Complete attachment confirmation for the initial type+both run was corrected from a provisional 18/21 to 16/21.

This experiment compares **9 attachment policies**. No new A/B-1/unattached B-4/B-6 runs were added to the requested 45, and its conditions differ from the earlier required/forbidden overview/search workflows. Do not interpret it as an improvement percentage over historical baselines or the effect of attachment itself. Do not compute averages excluding missing total usage, claim equivalence from 0/5 versus 0/5, or generalize from 5 runs of one question.

Report · Repetitions and key values · 18 comparisons holding other factors fixed · Actual-delivery audit · 94 detailed metrics · Completion verification

The table below is the previously completed workflow experiment. Do not mix it with the new symbol-candidate results.

### Earlier direct comparison — B-4 with indexed tools versus source tools only, 5 runs each

At the user's request, new sessions solved the same difficult question `complex-go-1` from scratch, 5 times per condition. Both used B-4; the non-use condition was not the existing A baseline. **The indexed-tools condition successfully used search and file overview before the first read; the non-use condition used only grep/find/read.** Actual workflow compliance and initial-instruction delivery were confirmed in 5/5 runs on each side. The original question, answer contract, product, and index were fixed, and neither condition received automatic root overview.

| Metric | Using overview/search | grep/find/read only |
|---|---:|---:|
| Partially correct | 3/5 | 1/5 |
| Correct | 0/5 | 0/5 |
| Incorrect | 2/5 | 0/5 |
| No answer | 0/5 | 4/5 |
| Key facts satisfied | 9/15 | 2/15 |
| Average total tokens | 343,224.8 | Unconfirmed (≥634,861.0) |
| Complete usage | 5/5 | 3/5 |

Comparing one confirmed average with the other side's lower bound, **the indexed-tools workflow's average cost in this sample was at least 45.9% lower.** This is neither a subtraction of two lower bounds nor a confidence interval for a general effect. All 5 non-use runs reached the token limit, but 1 left an answer before interruption was detected and was graded; late answers were excluded under predefined rules. Cost includes up to 30 seconds of collection wait time. The indexed-tools condition still had 2 incorrect answers from confusing the target search path, as well as omissions of comparison conditions and citation evidence.

This shows **cost and answer-completion differences between these two assigned workflows**, not the independent effect of `overview` or `search`, or performance on all questions. Do not treat 0/5 versus 0/5 fully correct answers as evidence of equivalence. There were 10 new solution runs, 2 anonymous grading runs, and 0 automatic reruns. The initial direct readiness-check failure during index loading was preserved, and waiting for readiness was improved before model execution. No new product candidate or per-file symbol attachment was implemented here. Report · Key values and individual repetitions · Actual workflow

### What was established and how

| Category | Finding | Method and limitations |
|---|---|---|
| Causal effect of guidance | Guidance → changed first tool choice | 30/30 G-guidance runs started with `grep/find`; 18/18 neutral runs started with `overview/search`. Shared controls were not double-counted; this is not proof of final performance improvement. Controlled experiment |
| Causal effect of misuse | Misused regex metacharacters → failure to find existing code | Holding B-4, code, and other arguments fixed, correcting character handling changed 0 results into the target declaration. Direct tool reproduction was identical in 3/3 runs per condition. This concerns the `grep` regex path, not BM25 or indexed-content changes. Direct-correction experiment |
| Causal effect of guidance | Guidance → more specific search → greater target-implementation exposure in the capped first output | Only the guidance paragraph changed in the same recovery task, with 12 runs per condition. Specific declaration-search choices and target-implementation exposure in the first output each changed 0/12 → 11/12. **Misuse recovery itself was 12/12 on both sides**, and whole-question accuracy was not measured. Guidance control |
| Failure-location verification | Required explanations were omitted even after code delivery | Across 19 existing answers, all 9 omissions in 7 answers had the required code delivered before the final answer: 4 comparison expressions, 2 count-query paths, and 3 final SQL executions. Delivery does not prove understanding. Line/timestamp comparison |
| Grading-result verification | Correct content and sufficient citations are distinct | 18 facts in 10 answers were substantively correct but lacked evidence required by the fixed grading contract: 12 insufficient point-citation ranges, 4 omitted constructor field assignments, and 2 omitted constant definitions. These may overlap with the explanation-omission answers above. Detailed analysis |
| Collection-path verification | Immediate interruption left normal completion unconfirmed → improved with optional completion collection | The 5 historically unconfirmed runs involved failure to confirm normal turn completion after `SIGINT`, not truncated files. Blocking further exploration and waiting up to 30 seconds for collection passed 2/2 low-limit diagnostic runs. This neither recovers historical missing usage nor guarantees collection under every failure. [Collection contract](v2/USAGE-COLLECTION.md) |

Preserve the distinction between observations and established findings. In 12 pairs of full solutions after direct correction, tokens decreased in 9 pairs, increased in 2, and remained unconfirmed in 1, but missing total costs prevented a confirmed average saving. Correct answers changing 1/12 → 2/12 also does not prove improvement. The recent single-search recovery comparison had average tokens of 30,379.6 → 31,463.5, without establishing a cost difference. Do not combine costs from these two different tasks into a product-performance comparison.

### What remains unresolved

1. How much poor tool use contributes to overall cost and accuracy degradation, and whether it is the main cause.
2. Whether additional guidance reduces misuse in natural full solutions without an explicit recovery task.
3. Whether more specific searches and increased target exposure reduce total tokens and improve final accuracy.
4. The independent effects of changes to indexed content, ranking, or symbol extraction. Repeated differences between output-scope/information combinations were observed, but these 9 candidates without an unattached control do not establish attachment's own effect. Do not call guidance/rendering experiments with the same index experiments in changing indexed content.
5. How requirement interpretation, summarization, and citation guidance each affect omissions after evidence delivery. The answer request's file-path/line-number instructions differ in specificity from the grader's sufficient-evidence-range requirements, but the effect of improving guidance remains unmeasured.
6. Generalization to other questions, languages, models, or navigation states, and the independent contribution of `overview/search` itself.
7. The exact cause of unconfirmed usage after large-context limit termination, and complete collection under forced termination or failures. Among token-limit runs, usage remained unconfirmed in 2 of 5 non-use runs in the earlier workflow comparison, 3 of 6 symbol-attachment r1 runs, 4 of 11 r2 runs, and 1 of 1 Smart routing run. Actual total costs of historical unconfirmed runs were not recovered either.

### Follow-up experiment identifiers and completion scope

- `causal-control-r1`: 69 new solution runs. Final interpretation uses separately fixed `grading-r2`; previous grading remains preserved. Do not reduce an explicitly specified Markdown display range to a link's starting line or edit citations after grading to make them pass.
- `b4-misuse-control-r1`: 24 follow-up solutions and 4 grading runs, reconstructing the same displayed history immediately before the error. This was neither resumption replicating the original session's hidden state nor a prompt-training experiment.
- `b4-guidance-audit-r1`: 2 completion-collection diagnostics + 24 guidance-based single-search recovery comparisons = 26 new solution runs, 0 new grading runs. Complete usage for 26/26 runs does not mean 26 limit-termination tests; only 2 were limit diagnostics. The existing 33 checks passed. Completion verification

## History of attempts to attach per-file symbols to `grep/find/read`

On 2026-09-10, the user proposed adding per-file symbols to frequently used `grep/find/read` results, first requesting a review of previous attempts and documentation. **Related features existed at that point, but the inspected records contained no identical attempt to attach returned files' symbol summaries/lists directly to all three tools' responses.** The user later separately requested implementation of 9 `grep/read` combinations and a 45-run experiment. The initial `b4-live-symbols-r1` and `b4-live-symbols-r2`, remeasured with user-confirmed output, are tracked separately above. The following table covers earlier attempts.

| Previous attempt | What it actually did | Difference from this proposal and verification status |
|---|---|---|
| G1 · G2 | Linked files/lines returned by `grep/find/read` to the index and recommended file or folder `overview(...)` calls | Added **follow-up tool-call hints** after source/paths, not per-file symbol lists directly. |
| G3 · G4 | Located the current line or containing function, recommended `overview(path, line)`, then connected to symbol `search` | Used symbols internally, but actual symbol information was obtained through a later `overview`. G3 live hints applied to `grep/read`, not directly to `find`. |
| G5 | Recommended line-based overview for one file, folder overview for multiple files, or scope overview when no path was found | Context-dependent switching hints, not direct per-file symbol-list attachments. |
| R1-C4 | Optional `symbol` feature in `read`, reading the boundaries and source of a selected function/method | Implementation and contract checks were recorded in the preparation report, but the optional argument was used **0 times** across 10 runs. This differs from automatic symbol attachment to `grep/find`, and the feature's performance effect cannot be considered measured. |
| B-6 / R3-C1 | Indicated relevant function locations in large files in `search` results | Targeted `search` output, unlike the proposal to attach symbols directly to the three basic tools. |

The shared G implementation followed returned paths/lines (`LiveAnchor`) → selection from cached file symbols → up to 2 `Navigation hints` within 800 bytes. Actual model records also contain suggestions such as `overview({"line":21,"path":"…"}) — inspect symbols`. **Do not equate hints to inspect symbols with delivery of symbol information itself.** Shared implementation · G3 policy · Actual G workflow

Across the 15 G-candidate sessions at the time, overview and search were each called 4 times in total. Of 373 hints, only 1 exactly matched the immediately following request's tool and arguments. This is neither a utilization rate including indirect/later use nor proof that the tools themselves are ineffective. Implementation and usage results at the time

For R1-C4, the candidate registry, preparation report, and optional-feature usage counts were compared. Cleanup history records a move, but source files were not found at either the original or recorded destination in this inspection. Do not report this as direct revalidation of current source. The inspection covered the post-B-4 candidate registry, remaining G source/output, and related Git history; do not claim that no undocumented attempts ever existed.

No new implementation or model runs were performed during that historical review. The subsequently requested direct-symbol implementation and 45-run measurement are recorded above, while historical evaluation tables, raw data, and adoption status below are preserved.

## Correction of question-set changes and baseline-version reruns

**Changing evaluation question sets for each candidate screen and rerunning A/B-1/B-4/B-6, while presenting the values as fixed performance of those versions, was a procedural and reporting error.** The user required **fixed formal questions → a fixed error-check subset → a smaller fixed candidate-screening subset**. The earlier instruction to maintain a common question set separate from prechecks misrepresented this requirement and is withdrawn. Do not explain changing token values solely through model variability.

The final comparison-baseline correction in the 2026-09-09 session `01a07ebd-d0b2-7e61-a45b-2f48977e2488` was checked against stored artifacts. In the four batches below, **every selected question object matched the original Grafana dataset of 36 questions**. No rewriting of question wording, key facts, evidence, or control answers on each occasion was found. The verified problem was **selecting different question/answer sets and remeasuring baseline products**. Do not extend this finding into a claim that all historical experiments had identical answer contracts.

| Evaluation record | Actual question set | New baseline runs A/B-1/B-4/B-6 | All new model runs | Complete usage |
| --- | --- | ---: | ---: | ---: |
| grafana-four-way-r1 | 6 preparation questions × 1 run each | 24 | 24 | 22/24 |
| candidate-retry-r1 | 10 questions selected from the main evaluation × 1 run each | 40 | 180 | 154/180 |
| navigation-flow-screen-r1 | 1 of those 10 questions × 1 run | 4 | 9 | 8/9 |
| navigation-flow-screen-r2 | 3 of those 10 questions × 1 run each | 12 | 27 | 22/27 |

The four baseline versions alone were **newly run 80 times in total** across these batches. This is neither the total historical cost nor a count of unauthorized runs. In particular, the 24 remeasurements and 180 runs were requested by the user. The error was treating observations from different evaluation rounds as fixed baselines and failing to preserve consistent questions and reference results in later screening. The versions directly confirmed as repeatedly measured in these four recent batches were **A/B-1/B-4/B-6**. B-2/B-3/B-5 have separate histories, but do not claim that all versions B-1 through B-6 were remeasured every time.

The actual selected questions follow. `grafana-four-way-r1/dataset.json` contains all 36 questions; do not confuse the count in that file with the number executed. The first batch's execution targets were verified from the schedule in `frozen.json`.

- 6 preparation questions: `prep-complex-go`, `prep-complex-ts`, `prep-medium-go`, `prep-medium-ts`, `prep-simple-go`, `prep-simple-ts`. [Original 36 questions](v2/data/dataset.json)
- 10 common questions: `simple-ts-1`, `simple-go-4`, `medium-ts-1`, `medium-ts-4`, `medium-go-2`, `medium-go-5`, `complex-ts-2`, `complex-ts-5`, `complex-go-1`, `complex-go-4`. Selected questions and answers
- 1-question screening: `medium-go-2`. Selected questions and answers
- 3-question screening: `simple-ts-1`, `medium-go-2`, `complex-go-1`. Selected questions and answers

The existing 6 preparation questions and 30 main-evaluation questions do not overlap. Therefore, **the actual 6 → 10 → 1 → 3 history does not match the user's required fixed-subset structure**. The fact that the 1-question and 3-question sets belong to the 10-question set does not resolve the changing screening questions between rounds.

Frozen records for the four batches had the same Grafana commit, core harness hash, grading-criteria hash, and A-tool-definition hash. Execution used Luna medium through Codex `0.153.4`, with limits of 300 seconds, 80 navigation calls, and 500,000 input-plus-output tokens, and at most 2 concurrent runs. However, schedule seed `20260907` was not a model-sampling seed. Question selection, schedules, and execution-support procedures differed; the main agent graded the 24 preparation runs, whereas later evaluations used separate grading sessions. B-4/F3/F4 also differed in initial index state in the 1-question screen. A shared core harness does not make evaluations identical.

**The four result tables below are independent observations from their respective execution rounds. Do not compare or combine token/accuracy changes across tables as a product-improvement trend.** For example, A's average tokens of `159,915` and `294,941.667` came from new runs of 6 preparation questions and 3 screening questions, respectively; they are neither edits to a fixed score nor product-regression rates. Do not directly compare an incomplete-usage lower bound with a confirmed average to derive a savings percentage. Preserve the contemporaneous conclusion that the 180 runs and subsequent 9 runs and 27 runs did not establish clear joint improvement, along with **B-4's adopted status**.

Going forward, preselect and retain error-check and candidate-screening questions from the fixed formal set, and reuse baseline results under the same contract with their execution IDs. Do not automatically remeasure A or existing B versions merely because a candidate changed. Explicit remeasurements form separate evaluation rounds and do not overwrite historical tables. Follow the [reporting rules](REPORTING.md). **This work corrected documents and the registry; it did not register new subsets, implement the harness, run models, or regrade.** Continue excluding the historically erroneous formal 30-question results from improvement judgments.

## Navigation-flow instructions and implementation history — 2026-09-09

Latest follow-up instruction on 2026-09-09: create a candidate that **uses grep/find/read for shallow exploration as needed, then follows output hints to inspect symbols with overview and narrow detailed evidence with search**, and update both guidance and functionality in similar candidates. Preserve the earlier overview/search-first instruction as context for the preceding review. The latest flow was implemented and checked in G1–G5, but not adopted into active B-4. Continue respecting explicit scopes and checking original-source evidence.

Flow/output inspection reproduced a problem where both search after `overview apps/codemap-search/src/index` and an explicit matching subfolder `workspace_scope` expanded to all of `apps/codemap-search`. The cause was `WorkspaceCatalog::scope_for_input` returning the workspace root. Guidance inconsistencies, excessive configuration keys/fields in overview, and prioritization of declarations/configuration/tests in behavior searches were also recorded.

That flow review identified defects without model runs. Subsequently, 9 lightweight F1–F5 checks and 27 lightweight G1–G5 checks reflecting the latest instruction were completed. The existing B-4 product and 180-run results were retained.

## Preserved lightweight candidate checks — the same 3 Grafana questions, 1 run each

G1 basic flow, G2 folder-scope narrowing, G3 line-position-based symbol inspection, G4 call-relationship inspection, and G5 context-dependent switching were implemented through both guidance and functionality. **A/B-1/B-4/B-6 and G1–G5, the same 3 Grafana questions, 1 run each, for 27 new sessions total.** Neither the 12 preparation runs nor the formal benchmark was executed, and historical scores were not reused. Report · 94 detailed metrics · Final verification

| Metric | A | B-1 | B-4 | B-6 | G1 | G2 | G3 | G4 | G5 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Partially correct | 1/3 | 0/3 | 1/3 | 0/3 | 1/3 | 0/3 | 1/3 | 0/3 | 0/3 |
| Correct | 2/3 | 2/3 | 2/3 | 1/3 | 1/3 | 2/3 | 1/3 | 2/3 | 2/3 |
| Incorrect | 0/3 | 1/3 | 0/3 | 2/3 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 |
| No answer | 0/3 | 0/3 | 0/3 | 0/3 | 1/3 | 1/3 | 1/3 | 1/3 | 1/3 |
| Key facts satisfied | 6/7 | 5/7 | 7/7 | 6/7 | 4/7 | 4/7 | 3/7 | 4/7 | 4/7 |
| Average token usage | 294,941.667 | 144,141 | 219,122 | 173,852 | Unconfirmed (≥327,815.3) | Unconfirmed (≥256,727.0) | Unconfirmed (≥366,826.3) | Unconfirmed (≥285,157.0) | Unconfirmed (≥287,723.0) |

Complete usage was **confirmed for 22/27 runs**. Each G1–G5 candidate had 1 limit-terminated run without an answer, leaving its overall average unconfirmed. ≥ is the observed token sum divided by all 3 runs, including failures, rounded down to one decimal place. A's complex Go run also hit the limit, but had a final answer and complete usage and was classified as partially correct. Correct answers satisfy all required facts and citation evidence; key-fact satisfaction separately counts substantively correct facts.

**0 new performance candidates selected, 0 adopted; retain B-4.** Every G candidate had a higher observed lower bound for average tokens than B-4, fewer key facts satisfied, and more no-answer runs. These 3 questions with 1 run each do not establish general effects or causality for individual features.

Across 15 candidate sessions, overview and search were each called 4 times in total. Of 373 hints, 1 exactly matched the immediately following request's tool and arguments. This is a strict linkage diagnostic, not a count of all hint usage. Guidance and functionality were implemented, but the intended flow cannot be considered sufficiently used.

3 independent Astra high grading sessions verified 27 actual responses and 12 control answers. Final direct MCP checks passed 59/59 per candidate, 295/295 total; tool declarations at the actual consumer were confirmed for 27/27 runs. Initial verification failures and corrections were preserved in the report. Automatic reruns/regrading were 0; active B-4 source and binary were retained.

## Earlier lightweight candidate checks — the same 1 Grafana question, 1 run each

Following the user's request to screen candidates and run a simple check, F1 subfolder-scope preservation, F2 formal navigation guidance, F3 folder-overview summaries, F4 reduced over-prioritization of declarations, and F5 the combined version were prepared on B-4. **A/B-1/B-4/B-6 plus 5 candidates, for 9 new sessions total.** Neither the 12 preparation runs nor the formal benchmark was executed, and historical scores were not reused. Report · 94 metrics

| Metric | A | B-1 | B-4 | B-6 | F1 | F2 | F3 | F4 | F5 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Partially correct | 1 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 1 |
| Correct | 0/1 | 1/1 | 1/1 | 1/1 | 1/1 | 0/1 | 1/1 | 0/1 | 0/1 |
| Incorrect | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| No answer | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 |
| Key facts satisfied | 2/2 | 2/2 | 2/2 | 2/2 | 2/2 | 2/2 | 2/2 | 0/2 | 2/2 |
| Average token usage | 287,338 | 270,734 | 210,375 | 156,978 | 397,120 | 385,114 | 212,420 | Unconfirmed (≥567,181) | 440,691 |

Complete usage was confirmed for 8/9 runs; F4 hit the token limit without answering. ≥ is the observed lower bound for the 1 scheduled run. Correct answers satisfy both content and citation-evidence requirements; A/F2/F5 had correct content but insufficient evidence and were partially correct. Initial B-4/F3/F4 runs were warming their indexes, while other B variants received root overviews. Folder overview was used 0 times across all B variants. One run of one question and differences in startup state do not establish general effects of individual features.

**0 new performance candidates selected, 0 adopted; retain B-4.** F1 was separately preserved as a fix for the reproduced scope defect, not registered as a verified token saving. Guidance F2 and combined F5 also repeated full searches without sufficiently producing the target flow. 1 existing library-output assertion failed for F3 (8 passed); that failure was preserved, not reported as passed by changing the test. Other selected existing checks, direct MCP checks, actual tool declarations for all 9 conditions, and verification of 9 new anonymous grades and 4 controls passed. Automatic reruns/regrading were 0.

This table records observations from the 1-question screening round; it does not stand in for effects of later candidates. Follow-up work uses fixed subsets and baseline-result reuse. The existing 180-run table is preserved below as the historical common-10 evaluation.

## Completed retry of 15 candidates and correction of evaluation stages

**The 12 runs were prechecks for finding errors before the formal benchmark, not for selecting performance candidates.** The interpretation of the 6 preparation-question results as candidate-performance judgments in B-5–B-7 work is withdrawn under the user's instruction. Preserve the figures below as precheck-query observations, and do not mix them into candidate-performance tables.

The earlier request was to **retry all 15 candidates** across three proposal groups, and it was completed as recorded below. Registered R1-C1–R3-C5 candidates were each prepared on B-4 and evaluated in 1 run per question on 10 main-evaluation questions, totaling 180 runs. This is a record of execution at that time, not an instruction to keep creating separate question sets. Record implementation checks, contract checks, and model performance separately; do not present some candidates' results as conclusions for the entire set.

Work artifacts: candidate-retry-r1 configuration · Implementation/contract preparation report for 15 candidates. Selected existing checks and direct MCP checks passed for all 15 variants; final preservation checks for source, binary, and existing tests also passed 15/15. R1-C5 was narrowed to one file and one block and does not support tracing values across files. Current B-4 source/binary hashes remained fixed.

**All 180 approved runs and grading were completed.** There were 18 actual configurations and 19 displayed columns; R3-C1 shared the same new B-6 result. There were 147 normal completions, 33 token-limit terminations, and 0 environment errors; complete usage was confirmed for 154/180 runs, and 26 runs had no final answer. Verification passed for 180 sessions, at most 2 concurrent runs, 180 tool declarations, 180 grades, and 40 control answers. All 15 candidate results · 94 metrics

## Preserved reference table for 180 candidate-evaluation runs — 10 common questions, 1 run each

This table contains new measurements without reuse of historical 6-question preparation scores. All average token totals are unconfirmed; ≥ denotes an observed lower bound across all 10 runs. Do not interpret differences between lower bounds as total-token savings percentages. Definitions of correct, partially correct, incorrect, no answer, and correct facts follow the [reporting contract](REPORTING.md).

| Metric | A | B-1 | B-4 | B-6 |
|---|---:|---:|---:|---:|
| Partially correct | 0 | 2 | 3 | 2 |
| Correct | 8/10 (80%) | 5/10 (50%) | 6/10 (60%) | 5/10 (50%) |
| Incorrect | 0 | 0 | 0 | 1 |
| No answer | 2 | 3 | 1 | 2 |
| Key facts satisfied | 19/25 | 16/25 | 21/25 | 18/25 |
| Average token usage | Unconfirmed (≥234,049.1) | Unconfirmed (≥303,379.4) | Unconfirmed (≥259,329.5) | Unconfirmed (≥277,594.0) |

Correct answers of 8/10 were observed for A and R1-C4/R2-C1/R2-C5/R3-C5. However, complete average token totals could not be confirmed, and every candidate's exploratory interval for the difference in correct answers included 0, so clear joint improvement was not established. **Retain current B-4.**

Distinguish feature usage too. R1-C1's initial-instruction omission, R1-C4's symbol read, and R1-C5's value_evidence were each used 0 times. R2-C5 patterns was used 1 time but terminated with an output-cap error, and R3-C5 context_lines was used 6 times across 3 runs. Measuring an entire variant's performance differs from establishing a specific feature's causal effect.

Follow-up reports must keep the four A/B-1/B-4/B-6 columns and 6 required rows, appending all candidates on the right. Reuse baseline results compatible with the pre-fixed questions, answer contract, and repetition count; do not copy these 10-question values into evaluations with different compositions. New runs remain within the user's requested scope.

## Current product and work location

- The adopted product and local release binary are **B-4**. **B-6 is not adopted**; its results are preserved.
- B-7's common-description reduction was implemented as a single candidate and underwent 12 precheck runs; it does not replace evaluation of all 15 candidates. Do not reuse existing version names.
- Use **actual model token usage and correct answers** for candidate selection and improvement judgments. Reductions in CPU time, tool time, or internal work do not substitute for improvements in those two metrics.
- The repository scope is **Grafana**. Going forward, use a fixed error-check subset of the formal questions and a smaller fixed candidate-screening subset within it. Preserve the 6 preparation-question results as historical diagnostics. Do not use erroneous historical formal 30-question results or results from other repositories to judge improvement.

## Preserved remeasurement of precheck queries across four versions

The preserved preparation-query remeasurement is **`grafana-four-way-r1`**. A/B-1/B-4/B-6 each received 6 new sessions, for **24 runs total**. Historical scores were not mixed into this table, and it is not designated as a fixed baseline for future candidate-performance evaluations.

| Metric | A | B-1 | B-4 | B-6 |
|---|---:|---:|---:|---:|
| Partially correct | 2 | 0 | 2 | 0 |
| Correct | 4/6 (66.7%) | 5/6 (83.3%) | 4/6 (66.7%) | 5/6 (83.3%) |
| Incorrect | 0 | 0 | 0 | 0 |
| No answer | 0 | 1 | 0 | 1 |
| Key facts satisfied | 15/15 | 13/15 | 14/15 | 13/15 |
| Average token usage | 159,915 | Unconfirmed (≥223,801.1) | 194,298.5 | Unconfirmed (≥216,342.5) |

Complete usage was **confirmed for 22/24 runs**. A and B-4 were complete for 6/6 each, and B-1 and B-6 for 5/6 each. B-1 and B-6 hit the limit on complex Go without a final answer, and total usage was incomplete. `≥` denotes the **average lower bound**, calculated as recorded token sums divided by all 6 scheduled runs, rounded down to one decimal place. Do not interpret it as an exact average or an average excluding failed runs.

Correct answers satisfy all required facts and evidence. Correct content without sufficient evidence is partially correct. Key-fact satisfaction separately counts facts explained correctly. Thus A's fact satisfaction of 15/15 and correct-answer count of 4/6 are different metrics.

Execution used Codex **0.153.4**, **gpt-5.6-luna medium**, at most **2 concurrent runs**, and limits per run of **300 seconds / 80 navigation calls / 500,000 cached-inclusive input-plus-output tokens**. Automatic reruns and additional grading-model calls were **0**. The same main agent compared version-blinded answers against fixed source; this was not independent blind grading.

Checks for raw data, judgments, report regeneration, and preservation of existing results passed. Successful collection and aggregation checks do not establish suitability for candidate-performance evaluation. Do not determine candidate superiority or adoption from these precheck queries, or transfer a historical B-6 token-reduction percentage to another evaluation.

- Remeasurement report
- Requested raw metric values
- 94 detailed metrics
- Frozen conditions, versions, and source hashes
- Final verification

## Format to retain for subsequent candidates

**Use the table format above in candidate-exploration reports, opening summaries of candidate-validation reports, and final conversational result responses.** Do not omit A or show only the two latest versions. A link to a detailed report does not replace this summary table.

1. Default comparison columns are ordered **A / B-1 / B-4 / B-6**. When comparing new candidates, keep those columns and **append candidate columns on the right**. Display all 15 candidates using their fixed registry IDs. If splitting tables, retain the baseline columns and the same six rows and row order.
2. Rows are ordered **partially correct / correct / incorrect / no answer / key facts satisfied / average token usage**. Do not replace average tokens with total tokens or call counts.
3. Label baseline results compatible with the fixed evaluation contract as **reused historical records**, and state execution IDs, question composition, and new model-run counts. Preserve incompatible historical results in separate tables and mark the current comparison cell unmeasured. Do not fill an unmeasured new-candidate cell with an older product's value.
4. In actual verification, identify the execution round for each column and whether it contains **new runs or reused historical results**. Do not mix favorable token/accuracy values from different runs of the same version. Do not automatically remeasure baseline versions. If remeasurement of all four versions is explicitly requested, use only their new runs in a separate round's table and preserve previous tables.
5. Average tokens equal input tokens including cache plus output tokens, divided by all scheduled runs. Do not double-count cache. If totals are incomplete, label them **unconfirmed** and mark any accompanying observed lower bound with **≥**. Do not fill missing values with 0 or exclude failed runs from the denominator.
6. Below the table, state the definition of a correct answer, the number of runs with complete usage, the meaning of lower bounds, and necessary comparison limitations. Do not create a new composite score that offsets fewer correct answers with token savings.
7. Preserve the existing **94 detailed metrics** in appendices/raw data. Use tokens and correct answers for candidate selection and adoption decisions; distinguish other metrics as diagnostic material.

Maintaining this format does not authorize automatic additional model runs or increases to fixed run counts. Execution scope, counts, and rerun limits follow the approved conditions for the relevant task.

## Updating and preserving records

When a new evaluation completes, preserve raw data and reports and append tables and sources by execution round. Updating the latest-observation reference in `current-baseline.json` does not automatically replace a fixed comparison baseline. Do not alter earlier experiments' reports, scores, or hashes. Record product adoption status, fixed comparison baselines, and latest observations separately. Baseline changes or explicit remeasurements require a new evaluation identifier and a reason for the change.

Keep this summary outside the excluded large `artifacts/` area. In a new environment without raw data, this document may be referenced, but do not claim raw-data verification or new model execution.

English translation of the historical Korean record; [한국어 원문](CURRENT-STATE.md).
