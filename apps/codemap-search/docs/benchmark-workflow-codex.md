# codemap-search Codex 벤치마크 워크플로우

이 문서는 Claude Code 기준 문서의 모델명 치환본이 아니라, Codex 실행 경로와 사용량 스키마를 별도 통제하는 Codex 기준 사본이다. 절차·격리·지표의 원출처는 `benchmark-workflow.md`이고, Codex 성능 저하의 인과 기록은 `benchmark-evolution.md`와 `docs/briefs/2026-06-14-benchmark2-*.md`를 따른다.

충돌 시 우선순위는 다음과 같다.

1. 이 문서: Codex 기준 실행·해석 규칙
2. `benchmark-workflow.md`: 공통 하니스·격리·메트릭 정의의 원출처
3. `benchmark-evolution.md`: 인과·백로그·판정 기록의 원출처
4. `docs/briefs/2026-06-14-benchmark2-*.md`: 세션 이관용 상태 기록

## 1. 목적과 측정 질문

목적은 사용자가 요청한 로컬 코드베이스에서 codemap-search MCP가 Codex 계열 모델의 탐색 효율을 개선하는지 확인하는 것이다. 기존 deno 4-way 결과에서는 Codex+MCP가 baseline 대비 tool call +20%, 입력 토큰 +10%로 악화했고, 도구 결과 바이트만 -85% 우위였다. 따라서 Codex용 측정은 정확도보다 효율과 행동 차이를 1급 지표로 둔다.

핵심 질문:

- Codex `gpt-5.5` reasoning medium이 같은 과제를 baseline보다 적은 입력 토큰·도구 호출·탐색 단계로 푸는가?
- Codex `gpt-5.4-mini` reasoning medium에서도 같은 방향이 재현되는가?
- Claude `claude -p --model sonnet` MCP-only arm은 같은 제품 표면에서 어떤 참고 행동을 보이는가?
- Codex의 반복 read·윈도 페이징이 제품 출력 문제인지, 모델 습관인지, 과제 설계 문제인지 분리 가능한가?

## 2. Arm 정의

최소 매트릭스는 사용자가 지정한 로컬 코드베이스 하나를 기준으로 구성한다. 같은 과제·같은 스냅샷·같은 rubric을 유지하고, 모델과 도구 표면만 바꾼다.

| arm | 목적 | 모델 | 허용 표면 | 비고 |
|---|---|---|---|---|
| `codex-gpt55-base` | Codex 주 기준선 | `gpt-5.5`, reasoning medium | MCP 없음, 읽기 전용 내장/셸 조회 | within-model 비교의 기준 |
| `codex-gpt55-mcp` | Codex 주 실험군 | `gpt-5.5`, reasoning medium | codemap-search MCP only | baseline 대비 turns·tokens·read paging 비교 |
| `codex-gpt54mini-base` | 소형 Codex 기준선 | `gpt-5.4-mini`, reasoning medium | MCP 없음, 읽기 전용 내장/셸 조회 | mini 가치를 말하려면 필수 |
| `codex-gpt54mini-mcp` | 소형 Codex 실험군 | `gpt-5.4-mini`, reasoning medium | codemap-search MCP only | gpt-5.5 결과의 전이 확인 |
| `claude-sonnet-mcp` | 교차 CLI 참고군 | `claude -p --model sonnet` | codemap-search MCP only | 제품 표면에 대한 참고 행동 |

`claude-sonnet-base`는 Claude 내 효율을 주장할 때만 추가한다. Codex용 문서의 기본 결론은 Codex arm 내부 비교로만 낸다.

## 3. 실행 주체와 리뷰 게이트

실행과 판정은 분리한다. 메인 루프는 계획·브리프·게이트 판정·결과 종합만 맡고, 대량 조회와 검토는 하위 실행 단위에 위임한다.

| 단계 | 주체 | 규칙 |
|---|---|---|
| ground truth 수집 | 하위 에이전트 또는 로컬 `rg`/Read | codemap-search MCP 사용 금지 |
| contestant 실행 | 고정 하니스 또는 하위 에이전트 | 프롬프트 재작성 금지, 파일 수정 금지 |
| Codex MCP arm | Codex sub-agent `gpt-5.5 medium`, `gpt-5.4-mini medium` | codemap-search MCP만 허용 |
| Claude MCP arm | `claude -p --model sonnet` | codemap-search MCP만 허용 |
| 건전한 피드백 | sub-agent `gpt-5.5 xhigh` + `claude -p` default | `claude -p`는 Codex `gpt-5.5 medium` 하위 에이전트에서 CLI로 실행 |
| 적대적 리뷰 | sub-agent `gpt-5.5 xhigh` + `claude -p` default | 측정 설계·누출·해석 비약을 공격하게 함 |

리뷰용 `claude -p` default 모델과 contestant용 `claude -p --model sonnet`은 구분한다. 리뷰는 설계 검토이고, contestant는 측정 대상이다.

## 4. 단계 구성

```text
scope lock -> dataset -> warmup -> run -> verify -> score -> review -> report
```

| 단계 | 완료 조건 |
|---|---|
| scope lock | 대상 로컬 코드베이스 경로, git SHA 또는 스냅샷 mtime, 제외할 로컬 지침 파일 목록 기록 |
| dataset | 과제·정답·distractor·rubric 작성. ground truth는 MCP 없이 확정 |
| warmup | 각 arm 1~2개 에피소드로 도구 노출, purity, 토큰 필드, answer_text 원문 보존, 스키마 환산 가능성 확인 |
| run | 고정 프롬프트 그대로 실행. 에피소드별 원시 transcript와 metrics 저장 |
| verify | MCP 호출 0/only, 오염 문자열, 수정 시도, harness_error, 토큰 이중계산 여부 검사 |
| score | rubric 기계 적용. 정답률이 포화되면 효율 분석으로 피벗 |
| review | 건전한 피드백과 적대적 리뷰를 sub-agent `gpt-5.5 xhigh` 및 `claude -p` default로 분리 수행 |
| report | confirmed/inferred를 구분하고, within-model 비교만 결론으로 승격 |

## 5. 데이터셋 규칙

Codex용 데이터셋은 정답 이름을 프롬프트에 노출하지 않는다. 기존 deno 세트는 유도성 프롬프트가 정답률뿐 아니라 효율도 오염했으므로, 새 과제는 행위·증상·사용자 관찰만으로 묻는다.

필수 항목:

- 과제 유형: 리터럴, 정의, 호출처, depth-2 흐름, 흩어진 N개 위치, 모호한 개념, distractor 분별
- 각 과제마다 `expected`, `acceptable`, `distractors`, `wrong_if`, `line_tolerance` 기록
- ground truth는 `rg`/Read로 확정하고 codemap-search MCP로 만들지 않음
- baseline이 정답과 distractor를 모두 찾을 수 있는 과제를 별도로 포함
- `first_answer_turn`은 단순 파일 등장보다 정답 줄 또는 정답 심볼 노출을 우선한다

deno B-deno-2의 교훈은 두 축으로 분리한다.

- 구조적 grep 사각: deno에서는 음성으로 닫힘. Codex xhigh가 후보 지렛대를 grep으로 깼다.
- distractor 분별: 살아 있는 후보. baseline이 정답과 distractor를 모두 찾은 뒤 약한 모델이 고르는지 시험한다.

## 6. 격리와 purity

측정 사본은 실행 전에 다음을 처리한다.

- `AGENTS.md`, `CLAUDE.md`, `.claude`, `.cursorrules` 격리
- baseline arm에서는 `.codemap`도 격리
- MCP arm은 사전 인덱싱 후 소스 트리 무수정
- Codex 실행은 stdin 대기 방지를 위해 `< /dev/null` 적용
- `approval_policy=never`, 읽기 전용 sandbox, 사용자 설정 무시

purity 검증:

- baseline: transcript 안의 `mcp__codemap` 문자열 0, MCP tool-call 0
- MCP-only: codemap-search MCP 외 파일/셸/웹 도구 사용 0
- Claude: 허용 밖 도구 시도와 확장 도구 카탈로그를 분리해서 기록. pure의 정의는 "도구 목록이 비어 있음"이 아니라 "비허용 도구 사용 0"이다.
- 모든 arm: 오염 문자열, 파일 수정 시도, harness_error 0

## 7. 메트릭

CLI 간 절대 토큰 비교는 금지한다. 결론은 같은 모델의 baseline과 MCP arm 사이에서만 낸다.

| 필드 | 의미 |
|---|---|
| `score` | `correct|partial|wrong|n/a` |
| `turns` | 하니스가 정의한 도구 호출 수. ToolSearch 같은 하니스 메커니즘은 제외 |
| `first_answer_turn` | 정답 줄·정답 심볼이 처음 노출된 도구 호출 순번 |
| `tool_calls` | 도구별 호출 수 |
| `read_window_calls` | 같은 파일의 연속 범위 read 또는 윈도 페이징 수 |
| `mcp_response_bytes_total` | MCP arm에서는 MCP 결과 바이트, baseline에서는 도구 결과 바이트 |
| `input_tokens` | Codex는 cached 포함. Claude와 절대 비교 금지 |
| `output_tokens` | 참고값 |
| `duration_s` | wall clock |
| `answer_text` | 최종 답변 전문. 요약 금지 |
| `purity_violation` | 비허용 도구·MCP 누출·웹 사용·파일 수정 시도 |

Codex sub-agent 실행 결과가 기존 `codex exec --json`과 같은 구조화 이벤트를 제공하지 않을 수 있다. warmup에서 다음 세 가지가 환산되지 않으면 본실행에 들어가지 않는다.

1. 도구 호출 순서와 인자
2. 최종 답변 전문
3. 토큰 또는 토큰 대체 지표

토큰이 추출되지 않으면 그 회차는 토큰 결론을 내지 않고, turns·도구 결과 바이트·read 페이징만 보고한다.

## 8. 고정 실행 계약

프롬프트는 tasks JSON에서 그대로 추출한다. baseline 변환은 MCP 접두 제거 같은 결정적 치환만 허용한다.

Codex CLI형 MCP arm 예시:

```bash
perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  -c "mcp_servers.codemap-search.command=\"$BINARY\"" \
  -c 'mcp_servers.codemap-search.args=["mcp"]' \
  --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
```

Codex CLI형 baseline 예시:

```bash
perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
```

Claude contestant MCP arm 예시:

```bash
(cd "$REPO_PATH" && perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  claude -p --model sonnet --setting-sources "" \
  --strict-mcp-config \
  --allowedTools "$ALLOWED_TOOLS" --disallowedTools "$DISALLOWED_TOOLS" \
  --mcp-config "$MCP_CONFIG" \
  --output-format stream-json --verbose "$PROMPT") < /dev/null > "$JSONL" 2> "$ERRLOG"
```

Sub-agent arm은 위 CLI 예시와 동등한 제약을 prompt와 도구 권한으로 구현한다. 단, 이벤트 스키마가 다르므로 warmup에서 metrics 환산 가능성을 먼저 확정한다.

## 9. 해석 규칙

정답률이 100%에 가까우면 정확도 부가가치를 주장하지 않는다. 기존 deno 4-way와 django/strapi baseline은 baseline도 포화되어, codemap-search의 가치는 효율과 탐색 구조로만 판정할 수 있었다.

Codex 회귀 판단은 다음 순서로 한다.

1. 같은 모델의 baseline 대비 MCP turns와 input tokens가 줄었는가?
2. MCP 결과 바이트가 줄었더라도 read 페이징으로 입력 토큰이 늘지 않았는가?
3. 회귀가 특정 과제 유형, 특정 파일 크기, 특정 도구 출력 형태에 집중되는가?
4. Claude arm은 같은 제품 출력에서 read를 생략하는가?
5. 제품 변경 가설이 Claude 응답 다이어트와 상충하지 않는가?

B-deno-4 기준 판정은 유지한다. `lang/` 리팩터링은 Codex 회귀를 악화시키지 않았다. 다만 동일 HEAD의 turns가 회차마다 크게 흔들렸으므로 개선은 주장하지 않고, "악화 증거 없음"만 확정한다.

## 10. 보고 형식

최종 보고는 아래 순서를 따른다.

1. 한 줄 판정
2. 실행한 arm, 모델, 과제 수, 스냅샷
3. purity와 warmup 결과
4. within-model 표: baseline vs MCP
5. Codex read 페이징·반복 read·first_answer_turn 분석
6. Claude 참고군과의 행동 차이
7. confirmed / inferred 구분
8. 다음 결정: 제품 수정, 데이터셋 보강, 또는 측정 중단

확인된 주장은 파일·라인·명령·산출물 경로를 붙인다. 추론은 무엇을 실행하면 확정되는지 함께 적는다.
