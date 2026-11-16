# Grafana v5 사전 스모크 후보 패키지

고정 소스: `c0a598f5a49abf2014f67e806075d8ad267c9efb`. revision `2`, 3문항·핵심 15개·확장 0개. 최종 실행용 4문항이 아니라 감독자가 승인한 **사전 스모크 3후보**다.

| ID | 언어 | 질문 |
| --- | --- | --- |
| `app-resource-admission` | go | 앱 자원 생성 구조 중 입력 검증기와 연결되는 구간 다 확인해줘. |
| `app-legacy-storage` | go | 앱 자원 조회 구조 중 기존 저장소와 연결되는 구간 다 확인해줘. |
| `user-browser-unified-index` | typescript → go | 통합 색인 조회 구조 중 브라우저의 사용자 계정 검색 요청과 연결되는 구간 다 확인해줘. |

## 승인과 관문

최초 12개 실측 후 감독자가 G1을 변경했다. 내용·경로 노출은 기록만 한다. 질문 용어별 내용 일치 ∪ 경로 일치 파일 집합의 단어 1개/2개 AND 조합 중, 파일 수 60 이하에서 핵심 파일 30% 이상을 찾는 조합이 없어야 한다. 30으로 v4 보정을 했을 때 HTTP bridge가 통과하여 승인된 규칙에 따라 60으로 올렸다. 60에서는 v4 두 문항이 모두 탈락한다.

- app-resource-admission: G3 108/200행(54.00%), 일반 통과.
- app-legacy-storage: **G3 예외(25.00%), 사전 스모크 비교 자료용, 최종 선정 대상은 기억 측정 통과 시에만**. 40/160행이다. 검토에 따라 설치 배경을 core에서 내리고 실제 요청별 읽기 선택을 넣은 뒤 신규 탈락하여 감독자 예외를 승인받았다.
- user-browser-unified-index: **G3 예외 유지(수정 후 38.89%, 최초 승인 46.92%), TS→Go 다양성 확보 목적**. 대체 진입점 근거까지 포함해 112/288행이며 50% 통과로 표시하지 않는다.
- G6는 사전 스모크 단계 유예. 최종 우선 2문항은 app-resource-admission(Go) + user-browser-unified-index(TS→Go)다.
- 기억 측정: **미측정(사용자 승인 대기)**. 모델 요청·풀이·채점·런타임 요청은 수행하지 않았다.

## 파일과 채점

`questions.json`만 풀이에 제공한다. `dataset.json`의 scope/facts와 `evidence.json`, `grading.json`, `manifest.json`은 비공개 정답 자료다. `SHA256SUMS`는 자기 자신을 제외한 6파일을 고정한다.

각 fact의 required_links를 모두 만족하면 correct=true다. RLn 대체는 n번째 연결만 대체한다. supported는 답변이 인용한 경로와 위치(행 또는 심볼)가 그 연결을 입증하는지 독립 판정한다. 정답 발췌가 답변 인용을 대신하지 않는다. 모든 core가 fulfilled일 때만 core_complete다. 인용 누락은 false이며 원문 접근 불능만 해당 차원의 indeterminate다.

앱 admission/legacy storage의 shorturl은 로컬 배선의 구체 예다. 같은 계약의 다른 실제 앱 증명도 허용하며 모든 앱별 규칙을 열거하지 않는다. admission은 chain→serverConfig.AdmissionControl 연결과 로컬 provider/검증 콜백 등록을 채점하며 외부 SDK 내부 인용을 요구하지 않는다. legacy의 설치 설명은 context_notes로 내렸고 dualWriter.Get의 요청별 선택을 core로 넣었다. 사용자 계정 검색은 loadUsers·관리자 fetchUsers·UserPicker 중 어느 한 실제 요청 경로로 충족한다. single-org·UsersAPI·users redirect·Kubernetes user service·unified read의 조건부 경로이지 모든 배포의 기본 경로가 아니다. core와 context-only 근거를 분리하여 G1/G3 핵심 분모에 배경 설명을 넣지 않는다.

## 독립 검토 반영

r1의 BLOCK 검토를 반영했다. 질문의 호환/사용자 검색 중의성, 다른 실제 진입점의 오답 처리, A1의 부정 사실 필수화, A4/A5의 로컬 근거 공백, L1의 설치 배경, U21 호출 행 누락과 URL 접두사/등록 조건을 수정했다. 질문 id와 순서는 유지했다. r1 패키지·빌드·보고서는 `dataset-v5/snapshots/r1/`에 보존했다. r2는 수정 후 독립 재검토 대기 상태이며 통과 판정을 자가 발급하지 않는다.

## 검증과 결합

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B ../build/collect_provenance.py
PYTHONDONTWRITEBYTECODE=1 python3 -B ../build/build_dataset.py
PYTHONDONTWRITEBYTECODE=1 python3 -B ../build/verify_dataset.py
PYTHONDONTWRITEBYTECODE=1 python3 -B ../build/test_dataset.py
PYTHONDONTWRITEBYTECODE=1 python3 -B ../build/measure_packets.py
```

`build/verify_dataset.py`는 bench-v4-pi의 route_contract를 읽기 전용으로 import하고, scratch에 만든 dataset/lock 결합 fixture를 메모리에 바인딩하여 load_dataset을 실행한다. 이는 실제 worktree 결합 승인·실행이 아니다.

`build/sync_into_repo.py`는 부모 실행용이며 이 작업에서는 실행하지 않는다. 고정 대상은 `/Volumes/Samsung_990_PRO/worktrees/buyong-mcp/bench-v4-pi`다. 기존 toolchain을 보존하면서 DATA·QUESTION_IDS·dataset manifest·lock dataset 결합과 체크섬만 갱신한다. 독립 내용 검토·기억 측정·실행 승인은 별도 게이트다.
