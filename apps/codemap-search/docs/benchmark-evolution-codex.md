# codemap-search Codex 벤치마크 진화

이 문서는 Codex 기준 벤치마크를 왜 기존 Claude Code 중심 문서와 분리하는지 설명한다. 수치와 실행 규칙의 정본은 `benchmark-workflow-codex.md`와 원문 `benchmark-workflow.md`이고, 이 문서는 인과·판정·남은 질문만 남긴다.

## 1. 출발점: Codex는 같은 제품을 다르게 쓴다

기존 캠페인들은 codemap-search가 정확도를 크게 올리는지보다, 같은 답을 더 적은 탐색 비용으로 찾게 하는지를 확인하는 방향으로 진화했다. 정확도는 여러 세트에서 포화됐다. C/C++ 캠페인, django+strapi, deno 4-way 모두 baseline이 이미 거의 100%에 도달했고, 정확도만으로는 도구 가치를 설명하지 못했다.

Codex용 문서를 따로 만드는 이유는 여기서 시작한다. 같은 codemap-search 출력에 대해 Claude는 search 스니펫을 신뢰하고 read를 줄이는 경향을 보였지만, Codex는 search/grep 결과를 받은 뒤에도 원본 파일을 여러 read 윈도로 다시 확인했다. 따라서 "응답 바이트를 줄이면 효율이 좋아진다"는 단순 명제는 Codex에서 성립하지 않는다.

## 2. deno 4-way에서 확인된 회귀

deno 기준선은 같은 10과제·같은 하니스로 baseline 2종과 MCP 2종을 비교했다. 결과는 Codex용 기준의 출발점이다.

| 비교 | 판정 |
|---|---|
| 정확도 | baseline 포함 전 arm 포화. 정확도 부가가치 없음 |
| Claude | MCP가 baseline 대비 tool call과 입력 토큰을 줄임 |
| Codex | MCP가 baseline 대비 tool call +20%, 입력 토큰 +10%로 악화 |
| Codex 바이트 | MCP가 baseline 대비 도구 결과 바이트 -85%로 우위 |

중요한 해석은 "Codex+MCP가 무조건 나쁘다"가 아니다. 더 적은 도구 결과 바이트가 더 적은 입력 토큰으로 이어지지 않았다는 것이 핵심이다. Codex는 줄 앵커와 스니펫을 보고도 원본 read로 재확인했고, 그 과정에서 입력 토큰과 도구 호출이 늘었다.

## 3. B-deno-1: 안전한 단일 제품 변경으로는 닫히지 않음

B-deno-1은 Codex 회귀를 제품 랭킹 결함으로 좁혀 보려는 시도였다. `inspect default port`류 쿼리에서 `DEFAULT_PORT`의 sub-token을 더 잘 맞추도록 랭킹을 개선했고, 테스트와 다중 repo A/B 가드, Claude 무회귀는 통과했다. 이 변경은 일반 개선으로 유지하기로 했다.

하지만 Codex 집계 회귀는 닫히지 않았다. d1은 국소 개선됐지만 전체 turns와 입력 토큰은 모델 변동과 다른 과제의 read 검증 습관에 묻혔다. 더 중요한 발견은 같은 쿼리가 실제로 모호하다는 점이었다. 정답 파일과 sibling 파일이 모두 `DEFAULT_PORT`를 갖고 있어, 순수 랭킹만으로 "도메인상 올바른 DEFAULT_PORT"를 안정적으로 고르는 것은 과적합 위험이 컸다.

판정:

- sub-token subset exact-match는 일반 개선으로 유지
- "Codex 회귀 해결책"으로 추가 랭킹 해킹을 쌓는 것은 금지
- 문제의 본체는 Codex의 read-검증 습관과 제품 출력 신뢰도 사이의 상호작용

## 4. B-deno-2: 구조적 grep 사각은 음성, distractor 분별은 열림

B-deno-2는 baseline이 100% 밑으로 떨어지는 변별 세트를 만들려는 시도였다. 유도성 프롬프트를 버리고, 행위 설명만으로 정답을 찾게 하는 5개 과제를 설계했다.

사전검증 결과는 두 축으로 갈렸다.

1. 구조적 grep 사각: 닫힘. Codex xhigh 적대 리뷰가 제안된 grep-defeating 지렛대를 직접 grep으로 모두 깼고, Opus baseline도 5/5에 도달했다. deno는 사용자-facing 문자열, `.d.ts`, 주석, 설정 이유, Rust 심볼명 중 하나로 행위가 새는 경우가 많아 구조적 grep-defeat 과제가 잘 성립하지 않았다.
2. distractor 분별: 열림. 같은 grep으로 정답과 distractor가 모두 잡히는 상황에서 약한 모델이 잘못 고르는 사례가 있었다. `--watch` watcher 과제에서 sonnet baseline은 `notify` crate는 맞췄지만 런타임 API용 watcher를 고르는 오답을 냈다. 이는 "grep이 못 찾음"이 아니라 "후보를 구분하지 못함"이다.

따라서 Codex용 다음 변별 세트는 "grep이 못 찾는 답"보다 "grep이 너무 많이 찾는 답"에 집중해야 한다. MCP가 같은 모델을 distractor에서 구하는지 확인하는 2-arm 프로브가 먼저다.

## 5. B-deno-4: 리팩터링 원인 가설 기각

B-deno-4는 `lang/` 리팩터링이 Codex 회귀를 악화시켰는지 확인했다. 같은 deno 코퍼스·같은 v7 인덱스·같은 하니스로 old `33cc7eb`와 new HEAD를 Codex `gpt-5.5 medium` pure MCP로 비교했다.

판정은 닫힘이다. new가 turns와 input tokens에서 old보다 flat 또는 소폭 낮았고, 정확도도 20/20 대 20/20이었다. 같은 HEAD 바이너리도 회차마다 turns가 크게 흔들렸기 때문에 개선은 주장하지 않는다. 확정 가능한 결론은 리팩터링이 Codex 회귀를 악화했다는 증거가 없다는 것이다.

이 판정은 중요하다. Codex 회귀는 리팩터링 산물이 아니라, 리팩터링 이전부터 있던 codemap-search와 Codex 행동의 상호작용 문제로 봐야 한다.

## 6. Codex용 현재 가설

현재 가설은 다음과 같다.

- Codex는 search/grep 스니펫을 최종 근거로 덜 신뢰하고, 원본 read로 검증하려 한다.
- 큰 파일에서 한 줄 답을 확인할 때 read 윈도 페이징이 turns와 입력 토큰을 지배한다.
- 응답 다이어트는 Claude에는 이득이었지만, Codex에는 보상 read를 늘릴 수 있다.
- model-aware 출력 설계가 필요할 수 있으나, 특정 과제에 맞춘 랭킹 해킹은 과적합이다.
- 정확도 포화 세트에서는 효율·distractor 분별·근거 품질로 피벗해야 한다.

## 7. Codex용 백로그

우선순위는 측정 신뢰성과 변별력 확보가 제품 수정보다 앞선다.

1. **sub-agent 메트릭 환산 warmup**: Codex sub-agent 결과에서 도구 호출 순서, 최종 답변, 토큰 또는 대체 지표를 안정적으로 뽑을 수 있는지 확인한다.
2. **gpt-5.4-mini arm 확정**: mini를 contestant arm으로 둘 경우 baseline과 MCP arm을 둘 다 실행한다. baseline 없이 MCP만 실행하면 mini의 도구 가치를 말할 수 없다.
3. **distractor 2-arm 프로브**: sonnet baseline이 틀린 e2류 과제에서 MCP-sonnet이 같은 모델을 구하는지 먼저 확인한다.
4. **Codex read 페이징 계측**: 같은 파일 반복 read, 인접 윈도 read, 전체 파일 read 실패를 별도 메트릭으로 추출한다.
5. **first_answer_turn 강화**: 파일 등장 대신 정답 줄·정답 심볼 등장으로 계산한다.
6. **model-aware 출력 파일럿**: Codex가 재읽지 않을 만큼 충분한 줄 앵커와 주변 맥락을 주되, Claude 응답 다이어트를 역행하지 않는 조건부 렌더링을 검토한다.
7. **토큰 산식 고정**: Codex `input_tokens`는 cached 포함으로만 사용하고, `cached_input_tokens`를 더하지 않는다.
8. **리뷰 게이트 이원화**: 건전한 피드백과 적대적 리뷰를 sub-agent `gpt-5.5 xhigh` 및 `claude -p` default로 각각 받아, 설계 보완과 반증 시도를 분리한다.

## 8. 문서화 원칙

Codex용 보고서는 모든 결론을 `confirmed`와 `inferred`로 나눈다.

- confirmed: 실행한 명령, transcript, metrics, 파일:줄, 산출물 경로가 있는 주장
- inferred: 현재 증거에서 가장 그럴듯하지만, 확정 실행이 남은 주장

예를 들어 "Codex 회귀는 리팩터링 때문이 아니다"는 B-deno-4 old-vs-new로 confirmed다. 반면 "model-aware 출력이 회귀를 닫을 수 있다"는 아직 inferred다. 해당 주장은 파일럿 전까지 제품 수정 브리프가 아니라 실험 가설로만 남긴다.

## 9. 다음 기준선

다음 Codex 기준선은 사용자가 요청한 로컬 코드베이스마다 새로 만든다. deno 수치를 일반화하지 않고, 다음 조건을 만족할 때만 보고한다.

- 로컬 코드베이스 스냅샷 기록
- ground truth MCP 미사용
- baseline 격리와 MCP purity 검증
- Codex `gpt-5.5 medium` baseline vs MCP within-model 비교
- Codex `gpt-5.4-mini medium` baseline vs MCP within-model 비교
- Claude sonnet MCP-only 참고군
- 건전한 피드백과 적대적 리뷰 결과의 별도 요약

이 조건이 충족되지 않으면 "Codex에서 codemap-search가 개선됐다" 또는 "악화됐다"는 일반 결론을 내지 않는다.
