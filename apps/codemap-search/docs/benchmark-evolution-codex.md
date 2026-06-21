# codemap-search Codex Benchmark Evolution / codemap-search Codex 벤치마크 진화

This document explains why the Codex benchmark track is separate from the Claude Code benchmark track, what the deno follow-up runs proved, and what must happen before the next Codex result can be reported. The runnable workflow and metric definitions live in `benchmark-workflow-codex.md` and the shared source document `benchmark-workflow.md`; this file records cause, decision, and remaining risk.

한국어 요약: 이 문서는 Codex 기준 벤치마크를 Claude Code 중심 문서와 왜 분리했는지, deno 후속 실행에서 무엇이 확정됐는지, 다음 Codex 기준선을 내기 전에 무엇을 확인해야 하는지 정리한다. 실행 규칙과 수치의 정본은 `benchmark-workflow-codex.md`와 `benchmark-workflow.md`이고, 이 문서는 인과와 판정만 다룬다.

## How To Read This Document / 읽는 법

- Treat `confirmed` as evidence-backed: it needs an executed command, transcript, metrics file, file:line reference, or artifact path.
- Treat `inferred` as a live hypothesis: it must name the run or probe that would confirm it.
- Compare `input_tokens` only within the same CLI/model family. Do not compare Claude and Codex token totals as absolute values.
- Use this document to decide the next experiment. Use `benchmark-workflow-codex.md` to run it.

한국어 요약: `confirmed`는 명령·전사·메트릭·파일:줄·산출물 경로가 있는 주장만 뜻한다. `inferred`는 아직 실험이 남은 가설이다. `input_tokens`는 같은 CLI와 같은 모델 계열 안에서만 비교한다.

## 1. Starting Point: Codex Uses The Same Product Differently / 출발점: Codex는 같은 제품을 다르게 쓴다

The benchmark series moved away from asking whether codemap-search raises accuracy on already-solvable tasks. In C/C++, django+strapi, and deno 4-way runs, baseline arms were already near or at 100% accuracy. Accuracy alone could no longer explain product value.

The Codex-specific track exists because the same codemap-search output led to different agent behavior. Claude tended to trust search snippets and avoid extra reads. Codex often treated search/grep output as a lead, then re-opened source files through read windows. Therefore the simple claim "응답 바이트를 줄이면 효율이 좋아진다" is not reliable for Codex.

한국어 요약: 기존 세트에서는 baseline 정확도가 이미 포화되어 정확도만으로 codemap-search 가치를 말하기 어려웠다. Claude는 스니펫을 신뢰해 read를 줄였지만, Codex는 search/grep 뒤에도 원본 read로 재확인하는 경향이 있어 바이트 절감이 곧 토큰 절감으로 이어지지 않았다.

## 2. Confirmed deno 4-way Regression / deno 4-way에서 확인된 회귀

The deno baseline compared two baseline arms and two MCP arms on the same 10 tasks and harness. These results are the starting point for Codex-specific rules.

| Area | Confirmed reading | Why it matters |
|---|---|---|
| Accuracy | All arms, including baseline, saturated | Accuracy had no remaining discriminating value |
| Claude | MCP reduced tool calls and input tokens versus baseline | Claude benefited from smaller, trusted snippets |
| Codex | MCP was worse than baseline by tool call +20% and input tokens +10% | Codex did extra verification work |
| Codex bytes | MCP reduced tool result bytes by -85% versus baseline | Smaller output did not guarantee lower `input_tokens` |

The key conclusion is not "Codex+MCP is always bad." The confirmed conclusion is narrower: in this deno run, fewer tool-result bytes did not lead to fewer input tokens because Codex re-read source files after seeing line anchors and snippets.

한국어 요약: 핵심은 Codex+MCP가 항상 나쁘다는 뜻이 아니다. 이 deno 실행에서는 도구 결과 바이트가 -85% 줄어도 Codex가 원본 파일을 다시 읽으면서 tool call과 `input_tokens`가 늘었다는 점만 확정한다.

## 3. B-deno-1: One Safe Ranking Change Did Not Close The Regression / B-deno-1: 안전한 단일 제품 변경으로는 닫히지 않음

B-deno-1 tested whether the Codex regression could be explained as a product ranking defect. The change improved sub-token matching for `DEFAULT_PORT`-style names in queries like `inspect default port`. It passed tests, multi-repo A/B guards, and Claude no-regression checks, so it remains a general product improvement.

The Codex aggregate regression did not close. d1 improved locally, but total turns and input tokens were still dominated by model variance and read-verification behavior on other tasks. The run also exposed a real ambiguity: both the expected file and a sibling file contained `DEFAULT_PORT`, so a pure ranking tweak could not reliably choose the domain-correct one without overfitting.

Decision:

- Keep sub-token subset exact-match as a general improvement.
- Do not add more ranking hacks as a "Codex 회귀 해결책".
- Treat the main problem as the interaction between Codex read-verification behavior and trust in product output.

한국어 요약: `DEFAULT_PORT` sub-token 랭킹 개선은 일반 개선으로 유지한다. 하지만 Codex 집계 회귀는 닫히지 않았고, 같은 이름이 여러 파일에 있는 모호성도 확인됐다. 추가 랭킹 해킹은 과적합 위험이 커서 금지한다.

## 4. B-deno-2: Structural Grep Blind Spots Closed Negative, Distractor Discrimination Stays Open / B-deno-2: 구조적 grep 사각은 음성, distractor 분별은 열림

B-deno-2 tried to build a discriminating set where baseline would fall below 100%. The task prompts removed answer names and asked only about behavior. Precheck results split into two different claims.

| Claim | Status | Evidence and next action |
|---|---|---|
| Structural grep blind spot | `confirmed` negative | Codex xhigh adversarial review broke the proposed grep-defeating levers with direct grep, and Opus baseline reached 5/5 |
| Distractor discrimination | `inferred` open | A weaker model chose the wrong candidate when grep found both the answer and distractor |

The next Codex discriminating set should focus less on "grep cannot find the answer" and more on "grep finds too many plausible answers." The cheapest next step is a 2-arm probe: check whether MCP helps the same model avoid the distractor.

한국어 요약: deno에서는 구조적으로 grep이 못 찾는 답을 만들기 어렵다는 음성 결론이 확정됐다. 반면 grep이 정답과 distractor를 모두 찾은 뒤 약한 모델이 잘못 고르는 축은 아직 열려 있다. 다음은 같은 모델의 baseline과 MCP를 비교하는 2-arm 프로브다.

## 5. B-deno-4: Refactor-Cause Hypothesis Rejected / B-deno-4: 리팩터링 원인 가설 기각

B-deno-4 checked whether the `lang/` refactor made the Codex regression worse. It compared old `33cc7eb` and new HEAD on the same deno corpus, same v7 index, same harness, and Codex `gpt-5.5 medium` pure MCP.

Confirmed result: there is no evidence that the refactor worsened the Codex regression. New HEAD was flat or slightly lower than old in turns and `input_tokens`, and both were 20/20 accurate. Because the same HEAD binary varied noticeably across runs, this does not prove an improvement.

The practical implication is important: the Codex regression should be treated as an existing interaction between codemap-search output and Codex behavior, not as a regression introduced by the refactor.

한국어 요약: `lang/` 리팩터링이 Codex 회귀를 악화했다는 증거는 없다. 다만 회차 변동이 커서 개선도 주장하지 않는다. 남은 문제는 리팩터링 산물이 아니라 Codex의 read 검증 습관과 제품 출력 신뢰도의 상호작용이다.

## 6. Current Codex Hypotheses / Codex용 현재 가설

Current working hypotheses:

- `confirmed`: Codex often treats search/grep snippets as leads, not final evidence.
- `confirmed`: Large-file read window paging can dominate turns and `input_tokens` when the answer is one line.
- `confirmed`: Response diet helped Claude but can trigger compensating reads in Codex.
- `inferred`: model-aware output may reduce Codex re-reads without undoing Claude response diet gains.
- `inferred`: saturated accuracy sets should pivot to efficiency, distractor discrimination, and evidence quality.

한국어 요약: 확정된 부분은 Codex가 스니펫을 최종 근거로 덜 신뢰하고, 큰 파일 read 페이징이 비용을 지배할 수 있다는 점이다. 아직 가설인 부분은 model-aware 출력 설계가 이 회귀를 닫을 수 있는지다.

## 7. Codex Backlog / Codex용 백로그

Measurement reliability and discriminating power come before product changes.

| Priority | Item | Done condition |
|---|---|---|
| 1 | **sub-agent metric warmup** | Codex sub-agent results expose tool-call order, final answer text, and token or substitute metrics |
| 2 | **gpt-5.4-mini arm confirmation** | Run both baseline and MCP arms; MCP-only mini results cannot support a product-value claim |
| 3 | **distractor 2-arm probe** | On e2-like tasks, verify whether MCP helps the same sonnet-level model choose the right candidate |
| 4 | **Codex read paging metrics** | Extract repeated reads of the same file, adjacent read windows, and whole-file read failures |
| 5 | **`first_answer_turn` hardening** | Count first exposure of the answer line or answer symbol, not merely the expected file path |
| 6 | **model-aware output pilot** | Give Codex enough line anchors and context to avoid re-reading, without reversing Claude response diet |
| 7 | **token formula lock** | Use Codex `input_tokens` as cached-inclusive and do not add `cached_input_tokens` again |
| 8 | **split review gates** | Separate constructive feedback and adversarial review across sub-agent `gpt-5.5 xhigh` and `claude -p` default |

한국어 요약: 제품 수정 전에 측정 신뢰성, mini 기준선, distractor 프로브, read 페이징 계측을 먼저 고정한다. baseline 없는 MCP-only 결과나 토큰 이중계산 결과는 제품 가치 주장에 쓰지 않는다.

## 8. Reporting Rules / 문서화 원칙

Every Codex report must separate `confirmed` and `inferred` claims.

- `confirmed`: claim backed by executed command, transcript, metrics, file:line, or artifact path.
- `inferred`: plausible claim that still needs a named confirming run or probe.

Examples:

- `confirmed`: "Codex 회귀는 리팩터링 때문이 아니다" when backed by the B-deno-4 old-vs-new run.
- `inferred`: "model-aware 출력이 회귀를 닫을 수 있다" until a pilot proves it.

Do not turn an `inferred` claim into a product brief. Keep it as an experiment hypothesis until the confirming run exists.

한국어 요약: 모든 결론은 `confirmed`와 `inferred`를 분리한다. 확정 근거가 없는 가설은 제품 수정 브리프가 아니라 실험 가설로 남긴다.

## 9. Next Baseline Gate / 다음 기준선

Create a fresh Codex baseline for each requested local codebase. Do not generalize deno numbers unless the new run satisfies all of these conditions:

- Local codebase snapshot recorded.
- Ground truth built without MCP.
- Baseline isolation and MCP purity verified.
- Codex `gpt-5.5 medium` baseline vs MCP compared within-model.
- Codex `gpt-5.4-mini medium` baseline vs MCP compared within-model.
- Claude sonnet MCP-only reference arm included.
- Constructive feedback and adversarial review summarized separately.

If these conditions are not met, do not claim that codemap-search improved or worsened Codex generally. Report only what the run directly measured.

한국어 요약: 다음 로컬 코드베이스마다 새 기준선을 만든다. 스냅샷, MCP 없는 ground truth, purity, Codex within-model 비교, mini baseline, Claude 참고군, 리뷰 분리가 없으면 "Codex에서 codemap-search가 개선됐다" 또는 "악화됐다"는 일반 결론을 내지 않는다.
