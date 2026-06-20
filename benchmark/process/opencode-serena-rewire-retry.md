# Phase B — opencode-serena 배선 1회 재시도 결과

## 판정: `booted_and_used` (전송 부팅 + serena 실호출 성공)

## 증거
- opencode `opencode.jsonc`의 serena MCP가 `local` 타입으로 부팅: `serena start-mcp-server --project <deno-main> --context ide`, `enabled:true`.
- `serena_search_for_pattern` 도구가 **2회 호출, 둘 다 `status:completed`** — 실제 serena 출력(파일별 매치 카운트) 반환 확인.
- `stderr.txt` 0바이트 (전송/부팅 에러 없음).
- 코드베이스: deno-main (TypeScript, LSP 경량). 모델: opencode-deepseek.

## 해석
- `-02`의 "opencode↔serena 전송 배선 미부팅"은 **단일 격리 실행에선 재현되지 않음**. 동일 배선으로 정상 부팅·호출.
- 다만 opencode는 serena 2회 호출 후 곧 `task` 서브에이전트(explore)로 위임 → serena 활용도가 낮음. 이는 **전송 문제가 아니라 opencode 런타임 행동**.
- 이 episode는 최종 답변 파일을 남기지 않음(전송/호출 검증 목적은 달성).

## 근본원인 (미확정 가설)
격리 retry가 정상 부팅되므로, `-02`의 실패는 하드 설정 버그보다 **동시성/부하**(다수 serena 동시 부팅) 가능성이 높다. 단 `-02` 실패 시점의 정확한 설정과 1:1 대조는 하지 않았으므로 **가설**로 남긴다.

## 사용자 결정에의 함의
- opencode-serena 27 episode 재실행은 **사용자 결정**.
- 재실행한다면: 격리에선 작동하나 **27 동시 신뢰성은 미검증** → `-02`의 serena 동시성 한도(전역 3 / codebase당 1)를 반드시 적용해야 한다.

## 산출물
- 실행 raw: `episode-artifacts/` (stdout.txt, stderr.txt, exact_command.json, opencode-xdg/)
