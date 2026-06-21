# codemap-search Codex Benchmark Workflow / codemap-search Codex 벤치마크 워크플로우

This is the Codex-specific execution contract for codemap-search benchmark work. It is not a model-name rewrite of the Claude Code workflow. It controls Codex execution paths, Codex usage-schema caveats, and the interpretation rules for Codex-vs-Codex comparisons.

한국어 요약: 이 문서는 Claude Code 기준 문서의 이름만 바꾼 사본이 아니라, Codex 실행 경로와 사용량 스키마를 별도로 통제하는 Codex 기준 문서다.

## Source Priority and Evidence Status

When sources conflict, use this order:

1. This document: Codex-specific execution and interpretation rules
2. `benchmark-workflow.md`: source of truth for shared harness, isolation, and metric definitions
3. `benchmark-evolution.md`: source of truth for causal history, backlog, and judgment records
4. `docs/briefs/2026-06-14-benchmark2-*.md`: session handoff state

Evidence status:

- Confirmed from the source documents above: previous deno 4-way measurements showed Codex+MCP worse than baseline on tool calls and `input_tokens`, while tool-result bytes improved. Therefore Codex benchmark claims must focus on efficiency and behavior, not accuracy alone.
- Confirmed from the source documents above: CLI-to-CLI token totals are not comparable. Promote only within-model comparisons, such as `codex-gpt55-base` vs `codex-gpt55-mcp`.
- Must be confirmed in each run: local repository path, git SHA or snapshot mtime, isolated instruction files, available token fields, exact tool-call schema, and whether `answer_text` is stored verbatim.
- Inferred until measured: whether a product change reduces Codex token cost, whether fewer response bytes reduce episode-level cost, and whether Claude behavior predicts Codex behavior.

한국어 요약: 확인된 주장에는 출처를 붙이고, 실행마다 다시 확인해야 하는 값은 본문에서 분리한다. 특히 토큰 비교는 같은 모델의 baseline과 MCP arm 사이에서만 결론으로 승격한다.

## 1. Purpose and Measurement Questions

The goal is to test whether codemap-search MCP improves exploration efficiency for Codex-family models on a user-selected local codebase. The benchmark should answer efficiency questions before accuracy questions, because prior runs often reached accuracy saturation while still showing meaningful differences in tool calls, read paging, and token use.

Core questions:

- Does Codex `gpt-5.5` reasoning medium solve the same tasks with fewer `input_tokens`, tool calls, and exploration steps than its baseline?
- Does Codex `gpt-5.4-mini` reasoning medium show the same direction?
- What reference behavior does Claude `claude -p --model sonnet` show on the same codemap-search MCP surface?
- Can repeated Codex read calls and window paging be separated into product-output issues, model habit, or task-design issues?

한국어 요약: 이 측정은 “정답을 맞혔는가”보다 “같은 정답을 더 적은 탐색 비용으로 얻었는가”를 먼저 본다.

## 2. Arm Definitions

Use one local codebase snapshot. Keep the tasks, expected answers, distractors, and rubric fixed. Change only the model and allowed tool surface.

| arm | purpose | model | allowed surface | interpretation |
|---|---|---|---|---|
| `codex-gpt55-base` | primary Codex baseline | `gpt-5.5`, reasoning medium | no MCP, read-only built-in or shell lookup | baseline for within-model comparison |
| `codex-gpt55-mcp` | primary Codex treatment | `gpt-5.5`, reasoning medium | codemap-search MCP only | compare against baseline on `turns`, tokens, and read paging |
| `codex-gpt54mini-base` | small Codex baseline | `gpt-5.4-mini`, reasoning medium | no MCP, read-only built-in or shell lookup | required before claiming mini-model value |
| `codex-gpt54mini-mcp` | small Codex treatment | `gpt-5.4-mini`, reasoning medium | codemap-search MCP only | checks whether `gpt-5.5` behavior transfers |
| `claude-sonnet-mcp` | cross-CLI reference | `claude -p --model sonnet` | codemap-search MCP only | reference behavior on the same product surface |

Add `claude-sonnet-base` only when the report makes a Claude-internal efficiency claim. The default conclusion of this document is Codex-internal only.

한국어 요약: Codex 결론은 Codex arm 내부 비교에서만 낸다. Claude arm은 같은 제품 표면에서의 참고 행동이지, Codex 토큰 결론의 근거가 아니다.

## 3. Execution Roles and Review Gates

Separate execution from judgment. The main loop owns planning, briefs, gate decisions, and final synthesis. Bulk lookup, contestant execution, and review should run in bounded worker phases.

| phase | owner | rule |
|---|---|---|
| ground truth collection | worker agent or local `rg`/Read | do not use codemap-search MCP |
| contestant execution | fixed harness or worker agent | do not rewrite prompts; do not edit files |
| Codex MCP arm | Codex sub-agent `gpt-5.5 medium`, `gpt-5.4-mini medium` | allow only codemap-search MCP |
| Claude MCP arm | `claude -p --model sonnet` | allow only codemap-search MCP |
| friendly feedback | sub-agent `gpt-5.5 xhigh` + `claude -p` default | run `claude -p` from a Codex `gpt-5.5 medium` worker when needed |
| adversarial review | sub-agent `gpt-5.5 xhigh` + `claude -p` default | attack measurement design, leaks, and over-interpretation |

Do not merge review and contestant roles. Review `claude -p` default is a design reviewer. Contestant `claude -p --model sonnet` is a measured arm.

한국어 요약: 실행자와 판정자를 분리한다. 리뷰 모델은 설계를 검토하고, contestant 모델은 측정 대상이다.

## 4. Workflow

```text
scope lock -> dataset -> warmup -> run -> verify -> score -> review -> report
```

| phase | exit criteria |
|---|---|
| scope lock | record local codebase path, git SHA or snapshot mtime, and instruction files excluded from the measurement copy |
| dataset | write tasks, expected answers, distractors, and rubric; confirm ground truth without MCP |
| warmup | run 1-2 episodes per arm to confirm tool exposure, purity, token fields, verbatim `answer_text`, and schema conversion |
| run | execute fixed prompts unchanged; store raw transcript and metrics for every episode |
| verify | check MCP zero/only constraints, contamination strings, edit attempts, `harness_error`, and token double counting |
| score | apply rubric mechanically; pivot to efficiency analysis when accuracy saturates |
| review | split friendly feedback and adversarial review across sub-agent `gpt-5.5 xhigh` and `claude -p` default |
| report | separate confirmed and inferred claims; promote only within-model comparisons |

Abort before the full run if warmup cannot recover tool-call order, tool arguments, final answer text, or a token metric or token proxy. A benchmark without those fields can still produce raw transcripts, but it cannot support the efficiency claims this document is designed to test.

한국어 요약: warmup은 선택 단계가 아니다. 토큰, 도구 호출 순서, 최종 답변 전문을 환산하지 못하면 본실행에 들어가지 않는다.

## 5. Dataset Rules

Codex tasks must not leak the answer name in the prompt. Prior deno task design showed that suggestive prompts can contaminate efficiency as well as accuracy. Ask from behavior, symptoms, or user-observable facts instead.

Required task metadata:

- task type: literal, definition, callers, depth-2 flow, scattered N locations, ambiguous concept, distractor discrimination
- per task: `expected`, `acceptable`, `distractors`, `wrong_if`, `line_tolerance`
- ground truth confirmed with `rg`/Read, not codemap-search MCP
- at least some tasks where the baseline can find both the expected answer and the distractor
- `first_answer_turn` based on first exposure of the answer line or answer symbol, not mere file mention

Keep the deno B-deno-2 lesson split into two claims:

- Structural grep blind spot: closed negative for deno. Codex xhigh broke the candidate lever with grep.
- Distractor discrimination: still live. Test whether weaker models choose correctly after baseline finds both the expected answer and the distractor.

한국어 요약: 정답 이름을 프롬프트에 넣지 않는다. baseline이 정답과 함정을 모두 찾는 과제를 포함해야 “찾기”와 “고르기”를 분리할 수 있다.

## 6. Isolation and Purity

Prepare the measurement copy before execution:

- quarantine `AGENTS.md`, `CLAUDE.md`, `.claude`, `.cursorrules`
- also quarantine `.codemap` for baseline arms
- pre-index MCP arms, then leave the source tree unchanged
- apply `< /dev/null` to Codex runs to prevent stdin waits
- use `approval_policy=never`, read-only sandboxing, and ignored user settings

Purity checks:

- baseline: zero `mcp__codemap` strings in transcript, zero MCP tool calls
- MCP-only: zero file, shell, or web tools outside codemap-search MCP
- Claude: record disallowed tool attempts separately from the expanded tool catalog. Pure means "zero disallowed tool use", not "empty tool list".
- all arms: zero contamination strings, zero edit attempts, zero `harness_error`

한국어 요약: 격리의 핵심은 도구 목록을 비우는 것이 아니라 비허용 도구 사용을 0으로 만드는 것이다.

## 7. Metrics

Do not compare absolute token totals across CLIs. Draw conclusions only between the baseline and MCP arm of the same model.

| field | meaning |
|---|---|
| `score` | `correct|partial|wrong|n/a` |
| `turns` | harness-defined tool-call count; exclude harness mechanisms such as ToolSearch |
| `first_answer_turn` | tool-call index where the answer line or answer symbol first appears |
| `tool_calls` | calls per tool |
| `read_window_calls` | sequential range reads or window paging on the same file |
| `mcp_response_bytes_total` | MCP result bytes for MCP arms; tool-result bytes for baseline arms |
| `input_tokens` | Codex includes cached tokens; do not compare absolutely with Claude |
| `output_tokens` | reference value |
| `duration_s` | wall clock |
| `answer_text` | full final answer; do not summarize |
| `purity_violation` | disallowed tool use, MCP leakage, web use, or edit attempt |

Codex sub-agent runs may not expose the same structured events as `codex exec --json`. Warmup must prove that these three items can be converted:

1. tool-call order and arguments
2. full final answer
3. tokens or a token proxy

If tokens cannot be extracted, do not make token conclusions for that episode. Report only `turns`, tool-result bytes, and read paging.

한국어 요약: 토큰 필드를 못 얻으면 토큰 결론을 내지 않는다. 대신 `turns`, 도구 결과 바이트, read 페이징만 보고한다.

## 8. Fixed Execution Contract

Extract prompts from tasks JSON unchanged. Baseline conversion may use only deterministic edits, such as removing the MCP prefix.

Codex CLI-style MCP arm example:

```bash
perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  -c "mcp_servers.codemap-search.command=\"$BINARY\"" \
  -c 'mcp_servers.codemap-search.args=["mcp"]' \
  --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
```

Codex CLI-style baseline example:

```bash
perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
```

Claude contestant MCP arm example:

```bash
(cd "$REPO_PATH" && perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  claude -p --model sonnet --setting-sources "" \
  --strict-mcp-config \
  --allowedTools "$ALLOWED_TOOLS" --disallowedTools "$DISALLOWED_TOOLS" \
  --mcp-config "$MCP_CONFIG" \
  --output-format stream-json --verbose "$PROMPT") < /dev/null > "$JSONL" 2> "$ERRLOG"
```

Sub-agent arms must implement equivalent constraints through prompt text and tool permissions. Because their event schema may differ from CLI runs, confirm metric conversion in warmup before the full run.

한국어 요약: 명령 예시는 실행 계약이다. 프롬프트를 다시 쓰지 말고, sub-agent 방식도 같은 제약을 도구 권한과 prompt로 구현한다.

## 9. Interpretation Rules

Do not claim accuracy value when accuracy is near 100%. Prior deno 4-way and django/strapi baseline results saturated enough that codemap-search value had to be judged by efficiency and exploration structure.

Judge Codex regression in this order:

1. Did MCP reduce `turns` and `input_tokens` compared with the same model's baseline?
2. If MCP result bytes dropped, did read paging still increase `input_tokens`?
3. Is the regression concentrated in a task type, file size, or tool-output shape?
4. Does the Claude arm skip reads on the same product output?
5. Does the product-change hypothesis conflict with the Claude response-diet results?

Keep the B-deno-4 judgment: the `lang/` refactor did not worsen Codex regression. Because `turns` varied heavily between runs on the same HEAD, do not claim improvement from that evidence. The confirmed statement is only "악화 증거 없음".

한국어 요약: 정확도가 포화되면 효율만 결론으로 남긴다. 개선 주장은 반복 측정이 받쳐야 하며, 단일 흔들림은 “악화 증거 없음” 이상으로 승격하지 않는다.

## 10. Report Format

Write the final report in this order:

1. one-line judgment
2. arms, models, task count, and snapshot
3. purity and warmup results
4. within-model table: baseline vs MCP
5. Codex read paging, repeated reads, and `first_answer_turn`
6. behavior difference from the Claude reference arm
7. confirmed / inferred separation
8. next decision: product change, dataset hardening, or measurement stop

Attach file, line, command, or artifact path to every confirmed claim. For every inference, state what run or artifact would confirm it.

한국어 요약: 보고서는 결론보다 증거 상태가 먼저 읽히게 쓴다. 확인된 주장과 추론을 섞지 않는다.
