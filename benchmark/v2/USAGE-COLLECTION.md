# 한도 종료 시 사용량 수집

`runner.execute_codex(..., usage_drain_seconds=30)`은 한도 도달 후 최대 30초 동안 정상 종료와 사용량 기록을 기다리는 선택 사항이다. 기본값 `0`은 기존 즉시 중단 방식을 유지한다. 범위는 0~30초다.

한도 감지 시 `stop.json`을 먼저 기록해 새 탐색을 차단한다. 수집 대기를 사용한 실행의 후속 도구 요청에는 추가 호출을 중단하고 턴을 끝내라는 종료 안내를 반환한다. 대기 시간이 지나도 끝나지 않으면 `SIGINT`, 3초 뒤 `SIGTERM`, 다시 3초 뒤 `SIGKILL` 순으로 중단한다.

수집 대기는 풀이 제한을 연장해 점수를 얻는 구간이 아니다. 최종 답변의 저장 시각이 한도 감지 이후이거나 시각을 확인할 수 없으면 평가용 `answer.txt`는 비우고 원문을 `post-cutoff-answer.txt`에 보존한다. 실행 상태는 계속 `token_limit`, `call_limit`, `timeout` 중 감지한 상태다.

전체 비용에는 수집 대기 중 추가 입력·출력 토큰이 포함된다. `budget.observed_tokens_at_cutoff`, `observed_tokens_after_collection`, `observed_cleanup_tokens`로 이를 구분한다. 캐시 입력은 입력 토큰에 이미 포함되어 있으므로 다시 합산하지 않는다.

`recorded_usage`의 인증 조건은 완화하지 않았다. 정상 종료 이벤트, 응답 ID별 합계와 누적값의 일치, 완전한 로그 수집 등을 확인한 경우에만 전체 사용량을 확정한다. 대기 후에도 강제 종료하거나 기록이 맞지 않으면 계속 미확정으로 남긴다. 이 방식은 추가 비용을 사용하며 모든 장애에서 완전한 수집을 보장하지 않는다.

2026-09-10 실제 진단에서는 토큰 한도 1,000과 20,000을 둔 두 실행 모두 정상 종료 사용량을 확보했다. 한도 이후 답변은 두 실행 모두 평가에서 제외됐다. 기존 검증 33개도 통과했다. 과거 중단된 실행의 누락 사용량을 복구한 것은 아니다.

새 평가에 사용한다면 실행 전 수집 대기 시간, 답변 시각 기준, 추가 비용의 보고 방식을 고정하고 두 비교 조건에 동일하게 적용한다. 기존 즉시 중단 평가와 수치를 합치지 않는다.

근거: [진단 원자료](artifacts/b4-guidance-audit-r1/collection-results.json), [고정 실험 계약](artifacts/b4-guidance-audit-r1/policy.json). Codex의 정상 종료 JSON 이벤트와 사용량 형식은 [공식 문서](https://learn.chatgpt.com/docs/non-interactive-mode)에 설명되어 있다.
