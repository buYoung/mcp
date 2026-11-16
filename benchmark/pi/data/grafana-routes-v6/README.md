# Grafana v6 스모크 후보 패키지

고정 소스: `c0a598f5a49abf2014f67e806075d8ad267c9efb`. revision `1`, 3문항, 핵심 21개, 확장 15개.
rg와 codemap-search만 비교하는 **스모크 후보 3개**다. 최종 실행용 확정 문항이 아니다.

| ID | 언어 | 핵심 | 확장 | G3 | 질문 |
| --- | --- | --- | --- | --- | --- |
| `git-sync-job-dispatch` | typescript → go | 7 | 5 | 82.96% | 메시지 버스 패턴 중 Git 저장소 동기화 작업이 그 작업을 실행하는 처리기까지 전달되는 구간 다 확인해줘. |
| `alertmanager-config-autosync` | typescript → go | 7 | 5 | 26.21% | Kubernetes 방식 API 중 외부 Alertmanager 설정을 가져오는 자동 동기화와 연결되는 구간 다 확인해줘. |
| `scope-annotation-filter` | typescript → go | 7 | 5 | 5.19% | 스코프 필터 중 대시보드 주석 저장·조회와 연결되는 구간 다 확인해줘. |

## 출처

- 질문 탐색 과정과 판정 기록: `v6/question-discovery.md`, `v6/scout/ledger.json`.
- 문항 내용: `git-sync-job-dispatch`는 `v6/scout/judge-output.md`의 Q3, 나머지 두 문항은 `v6/scout/judge-r2-output.md`의 질문 1·2를 옮겼다. 핵심·확장 사실의 근거 줄은 Opus 선정 단계에서 저장소와 대조했다.
- G3: 핵심 사실 근거의 고유 (파일, 행) 중 고정 커밋 blame committer-time이 2026-07-01 이후인 비율. `git-sync-job-dispatch`만 50% 이상이다. 나머지 두 문항은 G3 기준 미달이며 스모크 비교 자료용이다.
- 기억 측정: 미측정.

## 파일과 채점

`questions.json`만 풀이에 제공한다. `dataset.json`, `evidence.json`, `grading.json`, `manifest.json`은 비공개 정답 자료다. `SHA256SUMS`는 자기 자신을 제외한 6파일을 고정한다.

각 fact의 required_links를 모두 만족하면 correct=true다. RLn 대체는 n번째 연결만 대체한다. supported는 답변이 인용한 경로와 위치(행 또는 심볼)가 그 연결을 입증하는지 독립 판정한다. 모든 core가 fulfilled일 때만 core_complete다. 확장 사실은 진단 전용이며 핵심 분모에 더하지 않는다. context_notes는 방해 요소와 전제 교정 설명이며 필수 사실이 아니다.

## 다시 만들기

```sh
cd dataset-v6/build
PYTHONDONTWRITEBYTECODE=1 python3 -B build_dataset.py
PYTHONDONTWRITEBYTECODE=1 python3 -B sync_into_repo.py   # 부모 실행 전용
```
