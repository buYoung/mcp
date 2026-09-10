# codemap-search 하네스 V2

**현재 제품 소스 — 2026-09-10:** B-4에 `grep/read`의 **멤버 범위·둘 다**와 **하위 폴더 범위 보존·정규식 안내**를 반영했다(`ee1774533`, B-4 기반 커밋 누락 복구 `f27ca5edc`). [현재 반영 상태](CURRENT-STATE.md)와 [소스·측정 바이너리 구분](current-baseline.json)을 확인한다. 과거 B-4 측정값은 현재 패치 적용본의 성능 측정값이 아니다.

**계속 유지할 A 정식 결과:** [formal-r1-A 고정 결과표](results/formal-r1/A.md)와 [60회 원값·출처 해시](results/formal-r1/A.json)를 보존한다. 설명 보완 전 A의 과거 실행이며, 보류·안내 오류를 유지하고 새 결과로 덮어쓰거나 자동 재측정하지 않는다.

**최신 사용자 정정 — 2026-09-09:** 정식 문항에서 고정 오류 검증 부분집합을 선택하고 그 안에서 더 작은 고정 후보 탐색 부분집합을 사용한다. 최근에는 문항 구성을 **6 → 10 → 1 → 3개**로 바꾸고 **A·B-1·B-4·B-6도 각 평가에서 새로 실행**했다. 이를 같은 버전의 고정 성능 추이처럼 제시한 것은 절차·보고상의 오류다. 선택된 질문·정답 객체는 기존 원본과 같았으며, 확인된 문제는 개별 정답 재작성이 아닌 문항·정답 묶음 변경이다. [실행별 이력과 비교 한계](CURRENT-STATE.md) · [고정 문항·기준 결과 재사용 규칙](REPORTING.md)

**다음 후보 작업을 시작할 때 [현재 상태·최신 재측정 표·필수 보고 형식](CURRENT-STATE.md)을 먼저 읽는다.** 기본 비교 열은 **A / B-1 / B-4 / B-6**, 필수 행은 **부분 정답 / 정답 / 오답 / 답변 없음 / 핵심 사실 충족 / 평균 토큰 사용량**이다. 후보 탐색 보고와 최종 답변에서도 같은 표를 사용한다. 최신 두 버전만 보여주거나 A를 생략하지 않는다.

보존한 [네 버전 재측정](v2/artifacts/grafana-four-way-r1/report.md)은 Grafana 준비 6문제씩 총 24회이며 전체 사용량은 22회 확정됐다. 준비 6문제×2의 **12회는 정식 벤치 전 오류 찾기용**이며 후보 성능 선별용이 아니다. 이 비교의 기반 버전은 **B-4**이며, 현재 소스에는 위 승인 패치가 추가되어 있다. 이후 같은 평가 계약의 기준 결과는 실행 ID와 함께 재사용하고 후보가 바뀌었다는 이유로 자동 재측정하지 않는다. 다른 문항 구성의 과거 결과는 별도로 보존한다. 아래 과거 이력·명령은 현재 요청의 범위나 새 실행 승인을 대신하지 않는다.

**보존한 15개 후보 평가:** [후보15개·180회 비교](v2/artifacts/candidate-retry-r1/report.md)는 완료된 작업이다. 정상 종료 147회·한도 종료 33회, 정답 119개·부분 정답 27개·오답 8개·답변 없음 26개다. 전체 사용량은 154/180회만 확정돼 모든 버전의 평균은 미확정이다. 확실한 토큰·정답 공동 개선은 확정하지 못했다. [94개 상세 지표](v2/artifacts/candidate-retry-r1/candidate-metrics-detail.md)

**과거 탐색 관측:** 이후 [1문제·9회 탐색](v2/artifacts/navigation-flow-screen-r1/report.md)과 [3문제·27회 탐색](v2/artifacts/navigation-flow-screen-r2/report.md)을 완료했다. 당시 27회는 A/B-1/B-4/B-6과 G1~G5를 각각 3문제에 새로 실행했으며 전체 사용량은 22/27회 확정됐다. G1~G5는 각각 답변 없음 1회가 있었고 새 성능 후보 선별·채택은 0개여서 B-4를 유지한다. 이 값으로 이전 180회나 준비 24회의 기준값을 대체하지 않는다. [네 차수의 원래 결과표](CURRENT-STATE.md) · [당시 94개 상세 지표](v2/artifacts/navigation-flow-screen-r2/metrics-detail.md)

## 현재 개선 기준

아래 버전별 수치와 당시 판단은 과거 실행·채택 이력이다. 문항 구성·실행 차수가 다른 수치를 하나의 성능 추이로 연결하지 않으며, 준비 실험의 관측을 정식 성능 확증으로 해석하지 않는다. B-4의 사용자 채택 상태와 각 평가의 통계적 한계는 구분한다.

2026-09-08 사용자 승인으로 B-4를 소스와 로컬 release 바이너리에 반영했다. 그 채택 시점에 측정 당시 B-4와 소스·바이너리 해시가 일치했다. 현재 패치 적용 소스까지 같은 해시라는 뜻은 아니다. B-1 및 이전 실험은 비교·복원 자료로 보존한다. [채택 확인](v2/artifacts/b4-adoption-r1/build-verification.json)

현재 채택 구성은 **B-4 + 멤버·둘 다 + 범위·안내**이며, 후속 개선은 **Grafana만** 대상으로 [상세 메트릭·버전·보존 기준](REPORTING.md)을 따른다. 오류가 확인된 정식 결과와 다른 저장소 결과는 개선 판정에서 제외하고 기록으로만 보존한다. [현재 기준 제품](current-baseline.json) · [94개 지표 정의](metrics-catalog.json)

읽기 추천과 표시 원문을 연결한 **B-3(B-1 기반)**은 구현·Grafana 준비 12회 검증을 마쳤지만 성능 목표에 미달해 채택하지 않았다. 후보 자료를 보존하고 B-1으로 복원했다. [A/B/B-1/B-2/B-3 상세 보고서](v2/artifacts/grafana-b3-r1/report.md)에는 94개 지표와 실제 모델 전달 대조, 중단 실행의 결측, 복원 확인을 포함한다.

이후 [개선 후보 5개 경량 선별](v2/artifacts/candidate-screen-r1/report.md)은 독립 하위 에이전트 5개로 진행했다. 이름 일치 계산 재사용·렌더 원문 재사용의 중복 작업 감소만 확인해 다음 후보로 남겼다. 전체 응답 속도 개선은 확정하지 않았으며 새 모델 벤치마크·본제품 반영은 없다.

사용자가 유효하다고 확인한 위 2개는 [보존 후보 목록](RETAINED-IMPROVEMENTS.md)에 기록했다. [추가 5개 병렬 선별](v2/artifacts/candidate-screen-r2/report.md)에서는 호출자 임시 맵·문서 중복 해석·폴더 개요 요약을 새 우선 후보로 남겼다. 선별 당시에는 폴더 개요 3경로의 로컬 응답/CPU 감소만 관측했고, 전체 모델 성능과 후보 결합 효과는 미측정이어서 B-1을 유지했다.

선별한 5개를 합친 **B-4**는 [새 B-1/B-4 각 6회 비교](v2/artifacts/grafana-b4-r1/report.md)를 완료했다. 중단 포함 평균 실행 시간은 13.0%, 탐색 호출은 9.6% 감소했고 완전 정답은 양쪽 4/6이었다. 한도 종료 3건으로 전체 토큰 비교는 미확정이다. 비교 당시에는 B-1을 유지했으며, 이후 사용자 승인으로 B-4를 채택했다. 과거 보고서의 당시 상태와 점수는 보존한다.

B-4 채택 후 기존 B-4·A 답변과 탐색 기록을 분석한 [새 후보 5개와 근거](v2/artifacts/b4-next-candidates-r1/report.md)를 보존했다. 후속 구현 버전은 B-5(B-4 기반)이며, 이 분석에서 새 프로토타입·모델 벤치마크·채점 실행은 없다. [채택 작업 보고](v2/artifacts/b4-adoption-r1/report.md)

이후 승인된 [5개 후보 경량 검증](v2/artifacts/candidate-screen-r3/report.md)을 독립 하위 에이전트 5개로 진행했다. 4개 프로토타입과 1개 구현 전 진단, 직접 MCP 시간·CPU·RSS 48표본을 보존했다. exact 판정 교정에서 일부 국소 속도·Notebook 근거 노출 이득이 있었으나, 전체 개선을 확정할 후보는 없어 B-4를 유지한다. 새 모델 solver·채점은 0회이며 [상세 메트릭](v2/artifacts/candidate-screen-r3/metrics-detail.md)에 기존 94개 정의와 이번 미측정 항목을 구분했다.

그 결과와 B-4 소스를 바탕으로 [다음 후보 5개](v2/artifacts/b4-next-candidates-r2/report.md)를 새로 선정했다. 이름 빈도 재집계·설명 문자열 변환·caller 파일 조회·빈 문맥 준비의 중복 작업 감소와 선택형 grep 순회 공유다. 이 목록은 분석 단계이며 새 구현·벤치마크 실행은 없다.

위 5개의 [후속 격리 검증](v2/artifacts/candidate-screen-r4/report.md)을 완료했다. 직접 56표본에서 빈 caller 준비 생략의 적용 요청은 시간 13.23%·CPU 12.54% 감소, 다섯 grep 순회 공유는 세 모드의 전체 연산 시간 25.26~27.20% 감소를 관측했다. 두 후보를 다음 평가에 남겼으며 모델 토큰·정답 개선은 미측정이다. B-4와 과거 결과를 유지했고 새 solver·채점은 0회다. [작업량·CPU·RSS·94개 지표 상세](v2/artifacts/candidate-screen-r4/metrics-detail.md)

사용자가 핵심 지표 누락을 지적한 뒤 두 후보를 합친 [B-5의 실제 모델 12회 평가](v2/artifacts/grafana-b5-r1/report.md)를 수행했다. 새 B-4 대비 완전 정답은 4/6→3/6, 전체 토큰은 양쪽 복잡 Go 중단으로 미확정이다. 실제 Codex의 grep 선언에서 구체적인 인자 안내가 사라지는 회귀가 발견됐고 새 patterns 사용은 0회였다. B-5는 채택하지 않고 B-4를 유지한다. 최근 경량 선별 1~4차의 직접 속도 수치는 전체 모델 성능 개선율이 아니다. 이후 완료 기준은 [전체 토큰·최종 정답 우선 규칙](REPORTING.md)을 따른다.

기본 탐색 도구(A)와 codemap-search(B)를 비교한다. **완전 정답률을 먼저 평가하고 토큰·호출 비용은 별도로 보고한다.** 품질과 비용을 합친 점수는 만들지 않는다. 기존 Grafana 실험과 다중 저장소 정식 실행은 별도 계약·결과로 보존한다.

구현과 데이터는 [v2](v2/)에 있다. [고정 데이터셋](v2/data/dataset.json)은 준비 6문제와 본평가 30문제, 질문별 핵심 사실·원문·정답/부분 정답/오답 대조 사례를 포함한다. [선택된 PR 출처](v2/data/provenance/)에는 본문, 병합 커밋, 변경 파일, 패치, 고정 커밋의 조상 관계를 보존한다. 원문 행과 정답의 의미는 함께 검토해야 한다. 구조 검사만으로 자연어 정답의 의미가 입증되는 것은 아니다.

이 기존 데이터셋의 준비 6문제와 본평가 30문제는 서로 겹치지 않는다. 따라서 앞으로 적용할 **정식 → 오류 검증 → 후보 탐색의 중첩 부분집합** 정책이 이미 구현된 데이터로 설명하지 않는다. 이번 문서 정정에서는 새 문항·부분집합을 등록하거나 실행기를 수정하지 않았다. 오류가 확인된 과거 정식 30문제의 결과·점수는 개선 판정에 사용하지 않는다.

2026-09-07 검증에서는 자동검사 23개, A/B 런타임 검사, 준비 실행 12회와 Astra 채점·보고 생성을 수행했다. 제한 종료 4회의 전체 토큰을 확정하지 못해 본평가 120회는 시작하지 않았다. [검증 기록](v2/data/verification-status.json)과 로컬 [준비 보고서](v2/artifacts/preparation/report/report.md)를 확인할 수 있다.

준비 재실험 `preparation-r2`는 **12/12회 종료**했다. 자동검사 33개와 별도 런타임 검사 A/B 각 1회가 통과했고, 사용량 일관성은 12회, 전체 비용은 11회에서 확인했다. Astra 채점은 정상 종료했지만 오답 대조 사례 1건에서 고정 계약과 다른 판정을 반환해 묶음 검증에 실패했다. 정확도 차이·신뢰구간은 `null`로 보존하고 제품 비교를 보류했다. [재실험 검증 기록](v2/data/verification-status-r2.json), [새 준비 보고서](v2/artifacts/preparation-r2/report/report.md), [이전 실험과의 비교·채점 실패 분석](v2/artifacts/preparation-r2/report/preparation-comparison.md)을 함께 확인할 수 있다. 평가·채점을 자동 재실행하거나 본평가를 시작하지 않았다.

개선 제품의 에이전트 재평가 `preparation-r3`는 **12/12회 정상 완료**했고 전체 사용량도 모두 확보했다. 기존 R2 답변과 새 답변을 같은 최종 지시로 공동 블라인드 채점해 6묶음·대조 사례 24개가 검증을 통과했다. B의 정답은 **3/6 → 5/6**, 평균 총토큰은 **38.4%**, 탐색 호출은 **40.6%**, 시간은 **40.8% 감소**했다. 시간·호출 감소의 95% 구간은 0을 제외하지만 정답률·토큰 변화의 구간은 0을 포함한다. [개선 검증 보고서](v2/artifacts/improvement-r3/report.md)와 [검증 상태](v2/data/verification-status-r3.json)에 소규모 준비 실험의 한계, 채점 지시 보완과 추가 채점 비용을 함께 기록했다.

## 다중 저장소 정식 실행 결과

2026-09-08 `formal-r1`은 Django·Kubernetes API·Axum·Vite·Gson·fmt에서 5문제씩, A/B 각 2회 **120/120회 정상 완료**했다. 전체 토큰과 실행 조건은 모두 확인됐고 재집계 변경은 0건이었다. 블라인드 채점 30개 배치·대조 답안 120개는 검증을 통과했지만, 실제 답변 4개는 채점 입력에 대체 인용 원문이 없어 판정보류됐다. 전체 정확도 차이와 신뢰구간은 `null`로 유지한다.

B는 전체 토큰이 21.4% 증가했고 호출은 19.2%, 시간은 1.1% 감소했다. 다만 A의 빈 경로·읽기 행 번호 안내 부족으로 오류 95건이 발생했고, 생성 코드 사례도 기본 색인 크기 상한 밖이어서 감점 순위 규칙의 직접 검증에 부족했다. **실행·계측 완료와 공정한 범용 성능 개선의 확증을 구분하며, 이번 결과로 제품 개선을 확정하지 않는다.**

[결과와 한계](v2/artifacts/formal-r1/report/analysis.md), [실제 답변 120개](v2/artifacts/formal-r1/report/report.md), [검증 상태](v2/data/verification-status-formal-r1.json), [고정 계약·별도 CLI](v2/data/formal-r1/README.md), [질문 목록](v2/data/formal-r1/questions.md)을 확인할 수 있다. 기존 준비 자료와 합산하지 않았고, 동결 후 제품·질문·채점 지시는 변경하지 않았다.

아래 실행 조건과 `python3 -m benchmark.v2` 명령은 기존 Grafana 경로다. 다중 저장소 경로는 `python3 -m benchmark.v2.formal`과 해당 고정 계약을 따른다.

## 기존 Grafana 고정 실행 조건

아래 표는 기준 제품의 기본 계약이다. 후보 제품 비교에서는 `frozen.json`의 `verification.product_build`가 실제 제품의 출처와 바이너리를 식별한다.

| 항목 | 값 |
| --- | --- |
| Codex CLI | `0.153.4`, 새 `codex exec` 세션 |
| 실행 모델 | `gpt-5.6-luna`, `medium` |
| 채점 모델 | `gpt-6-astra`, `low` |
| Grafana | `c6fad8695a96577eb466d425e6ac4a759ca30f47` |
| B 제품 | `0.7.0`, 소스 `146d779a7d186327e765c2837637ad0543f802d1` |
| A 도구 | `rg`, `grep`, `find`, `read` |
| B 도구 | `initial_instructions`, `overview`, `search`, `grep`, `read`, `find` |
| 공통 호스트 | MCP 중계, `exec`·`wait` 래퍼. 다른 도구 namespace 제외 |
| 제한 | 실행당 300초, 실제 탐색 80회, 누적 입력+출력 500,000토큰 |
| 실행 수 | 준비 12회, 본평가 120회 |
| 순서 | 시드 `20260907`, 최대 2개 동시 실행, 2회차 A/B 순서 반전 |
| 통계 | 난이도 안에서 문제 단위 10,000회 부트스트랩, 95% 구간 |

`core.py`의 `SPEC`이 기계적 고정 계약이다. 모델이나 추론 강도를 자동으로 바꾸지 않는다. 각 실행은 독립된 Codex 설정 디렉터리, 소스 복사본, MCP 프로세스, 제품 전역 설정을 사용한다. 기존 Codex 로그인 파일은 실행 중 링크로 참조하고 링크를 제거한다. 인증 내용을 읽어 출력하거나 결과에 복사하지 않는다.

현재 구현은 Python 3.12 이상, macOS 또는 Unix 환경을 대상으로 한다. Python 외부 패키지는 필요 없다. `codex`, `gh`, `git`, `rg`, `grep`, `find`, Rust/Cargo와 정상적인 Codex 로그인이 필요하다. macOS에서는 `cp -cR`로 소스를 복제하고, 다른 Unix에서는 일반 복사를 사용한다.

## Grafana 소스 준비

저장소 루트에서 실행한다. `pnpm`, Python 3.12 이상, Git이 필요하며 최초 준비에는 GitHub 다운로드 연결이 필요하다.

```sh
pnpm bench:ready
```

[준비 스크립트](ready.py)는 [고정 계약](v2/core.py)의 `SPEC.repository`와 `SPEC.source_commit`을 사용해 해당 커밋 한 개만 가져오고, `benchmark/v2/artifacts/grafana`에 브랜치와 분리된 상태로 체크아웃한다. 완료 시 경로와 커밋 SHA를 출력한다. 이 명령은 Grafana 소스만 준비하며 제품 빌드나 모델 평가를 실행하지 않는다.

복제본이 이미 있으면 고정 커밋과 추적 파일의 변경 여부를 확인해 재사용한다. Git 복제본이 아니거나 커밋이 다르거나 추적 파일에 로컬 변경이 있으면 기존 파일을 덮어쓰지 않고 중단한다. 이 경우 기존 경로를 다른 곳에 보존하거나 변경을 정리한 뒤 다시 실행한다. 최초 준비는 임시 디렉터리에서 검증을 마친 후 대상 경로로 옮기므로 다운로드 실패 시 다시 실행할 수 있다.

Grafana 소스는 `benchmark/.gitignore`의 `v2/artifacts/` 규칙으로 이 저장소의 Git 추적에서 제외된다. 로컬 복제본은 사용하지 않을 때 제거해도 이 명령으로 다시 준비할 수 있다. 벤치마크가 커밋과 변경 여부를 검증하므로 사용하는 동안에는 복제본 안의 `.git`을 유지한다.

## 기존 실행 순서 — 과거 명세

모든 명령의 작업 디렉터리는 저장소 루트다. 큰 소스·색인·실행 결과는 Git에서 제외한 `benchmark/v2/artifacts/` 아래에 저장한다. 이미 실행한 결과 디렉터리와 고정 데이터셋 파일을 덮어쓰지 않는다.

아래 명령은 준비/본평가가 분리된 기존 하네스의 기록이다. 새 고정 부분집합 정책을 구현한 절차가 아니며, 후보 탐색마다 데이터를 다시 만들라는 지침도 아니다. 다음 실행 전에 [보고 기준](REPORTING.md)의 문항 계층·참조 실행·실행 범위를 먼저 고정해야 한다. 이번 문서 작업에서는 아래 명령을 실행하지 않았다.

### 1. 공개 PR과 소스 준비

현재 선택한 문제는 80개 후보를 검토한 결과다. 추가 후보 수집의 상한은 400개다. 수집은 `gh api`로 공개 Grafana 자료를 읽으며 외부 시스템을 변경하지 않는다.

```sh
python3 -m benchmark.v2 collect --output benchmark/v2/artifacts/collection --max-candidates 80
git clone --filter=blob:none --no-checkout https://github.com/grafana/grafana.git benchmark/v2/artifacts/grafana
git -C benchmark/v2/artifacts/grafana checkout --detach c6fad8695a96577eb466d425e6ac4a759ca30f47
```

`collect`는 병합 기간·조상 관계·백포트 제목·코드 파일 유무를 선별한다. 실제 코드에서 동작이 유지되는지와 변경/이슈 중복은 문제 작성 단계에서 확인한다. 검색 API 페이지는 최초 응답 그대로 보존하므로 나중의 검색 순서 변화를 소급 적용하지 않는다.

### 2. 데이터셋 검증 또는 새로 생성

포함된 `dataset.json`을 사용할 때는 재생성하지 않고 검증한다.

```sh
python3 -m benchmark.v2 verify --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana
```

과거 데이터셋 생성은 [selection.json](v2/data/selection.json)의 형식을 따랐다. `requirement`는 질문 안의 실제 부분문자열이고, `evidence_sets`는 해당 사실을 완전히 증명하는 원문 묶음들이다. 대체 묶음도 허용한다. 기존 본평가는 각 난이도에 TypeScript/TSX 5개와 Go 5개를 요구했고, 준비/본평가는 PR·변경을 중복 사용하지 않으며 명시적으로 닫는 이슈도 교차 검사했다. 이 분리 규칙은 과거 계약으로 보존하며 새 후보 탐색의 고정 부분집합 정책과 구분한다.

아래 명령의 출력 파일은 아직 존재하지 않아야 한다. 원문과 대조 사례가 검증되지 않으면 생성에 실패한다.

```sh
python3 -m benchmark.v2 build-dataset --selection benchmark/v2/data/selection.json --collection benchmark/v2/artifacts/collection --source benchmark/v2/artifacts/grafana --output benchmark/v2/artifacts/new-dataset.json
```

### 3. 자동검사·제품 빌드·실제 런타임 검사

```sh
python3 -m benchmark.v2 verify --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --build-product benchmark/v2/artifacts/pinned-product --runtime-probe benchmark/v2/artifacts/probe-final
```

`--build-product`는 고정 제품 커밋을 별도 경로에 추출하여 `cargo build --release --locked`로 빌드하고 소스 아카이브·명령·바이너리 해시를 기록한다. 설치된 MCP와 제품 작업 트리는 바꾸지 않는다. 빌드 및 런타임 검사 경로는 새 경로여야 한다.

오프라인 검사에는 요청별 토큰 중복 제거, 캐시 이중 합산 방지, 루프·병렬 요청 집계, 범위 밖 읽기, 잘린 출력의 근거 판정, 결측 분모, 대조 사례와 참조, 25% 경계, 통계 재현성, 보고서 생성이 포함된다. 실제 런타임 검사는 A/B 도구 목록, 직접 파일·네트워크 API 부재, 허용/차단 읽기, 최종 답변과 요청별 토큰을 확인한다. 이 검사는 준비 문제 12회의 대체가 아니다.

`verify`를 옵션 없이 다시 실행하면 오프라인 검사만 기록한다. 정식 실행에는 현재 코드 버전의 제품 빌드와 실제 런타임 검증 기록이 필요하다. 실행 옵션이 같은 기존 기록은 모델 호출 없이 다시 검사할 수 있다.

```sh
python3 -m benchmark.v2 verify --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --product-build benchmark/v2/artifacts/pinned-product/build.json --runtime-evidence benchmark/v2/artifacts/probe-sealed
```

### 4. 준비 실행 12회, 이후 채점·보고

```sh
python3 -m benchmark.v2 run --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --product benchmark/v2/artifacts/pinned-product/target/release/codemap-search --experiment benchmark/v2/artifacts/preparation --phase preparation
python3 -m benchmark.v2 grade --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/preparation
python3 -m benchmark.v2 report --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/preparation
```

실행 전에 예정 슬롯 전체와 명세·데이터셋·하네스·제품 해시를 고정한다. 실행 실패를 삭제하거나 모델을 자동 재실행하지 않는다. `run`을 같은 디렉터리에서 다시 호출하면 아직 `scheduled`인 슬롯만 시작한다. `running` 상태라도 종료된 모델의 `execution.json`이 남아 있으면 원자료 후처리만 복구하며 이전 상태를 `run-before-recovery.json`에 보존한다. 모델 종료 근거가 없으면 자동 성공·재실행으로 바꾸지 않는다.

모든 실행의 종료 상태가 있어야 `grade`가 진행한다. A/B 이름과 비용을 가린 결정적 묶음에 공통 원문과 사전 판정 대조 사례를 넣는다. 한 묶음은 최대 10문제·실제 답변 40개·160KiB다. 채점 실패·참조 오류·대조 사례 실패는 묶음 전체의 불완전 상태로 보존하며 Luna를 다시 실행하지 않는다.

### 5. 본평가 120회

준비 보고서의 `harness.ready_for_main`이 true일 때만 시작한다. 모델 실행 코드·명세·정답 계약을 바꿨다면 이전 고정 명세로 이어 실행할 수 없다. 순수 집계 수정은 모델 실행 함수·도구 중계·공통 계약의 서명이 동일한지 확인하고 별도의 `aggregation_sha256`을 기록해 기존 원자료에 적용한다. 초기 고정본에 실행 서명이 없으면 `harness-at-freeze/`가 원래 하네스 해시와 일치해야 한다.

```sh
python3 -m benchmark.v2 run --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --product benchmark/v2/artifacts/pinned-product/target/release/codemap-search --experiment benchmark/v2/artifacts/main --phase main --preparation-experiment benchmark/v2/artifacts/preparation
python3 -m benchmark.v2 grade --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/main
python3 -m benchmark.v2 report --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/main
```

## 지표와 판정

완전 정답은 유효한 최종 답변, 모든 핵심 사실의 정확성, 각 사실의 유효한 원문 근거, 중대한 오류 없음이 모두 충족될 때다. Astra는 사실·근거·오류만 반환하고 Python이 정답/부분 정답/오답/답변 없음/판정 불가를 도출한다. `import`나 파일명 일치만으로 소비 관계를 인정하지 않는다. 충분한 검색 발췌가 있으면 추가 `read`가 필수는 아니다.

전체 평균은 각 문제의 반복 평균을 구한 뒤 문제를 동일하게 가중한다. 실패와 제한 종료를 비용 분모에서 빼지 않는다. 환경·계측·채점 결측은 `null`과 사유로 남기고, 확인된 부분 합계는 `observed_partial_sum`으로만 표시한다. 토큰이 누락되어도 유효한 품질 자료는 별도로 보고한다. 캐시 입력은 입력 토큰의 부분집합이다.

총토큰은 `token_usage_record.response_id`별 입력+출력의 합으로 계산하고 누적 이벤트와 대조한다. 식별자가 같은 상충 기록은 무효다. 종료되지 않은 모델 요청의 비용을 확정할 수 없으면 전체 토큰을 `null`로 표시한다. 실행 시간은 실제 경과 시간이며 도구 시간을 합산하지 않는다.

준비 재실험 `preparation-r2`에서는 같은 응답의 `turn_token_usage`·`thread_token_usage`를 해당 시점의 중복 제거 합계와 대조한다. 구형 `token_count`가 이전 응답의 합계와 정확히 일치하면 지연 기록으로 구분한다. 어느 응답 시점과도 맞지 않는 값, 상충 중복, 누락과 합계 불일치는 무효다. `usage.consistency`는 기록의 일관성이고 `usage.cost_completeness`는 전체 실행 비용의 확보 여부다. 정상 턴 완료·정상 프로세스 종료·기록 수집·미완료 호스트 호출 부재가 확인돼야 전체 토큰을 확정한다. 종료 코드나 중단 이벤트만으로 처리 중 요청의 사용량을 추정하지 않는다.

제한 감지 시 `stop.json`에 사유·시각·관측 사용량을 한 번 고정하고 신규 탐색 접수를 차단한다. Codex 프로세스에 `SIGINT`를 보내 최대 3초간 기록을 수집하고, 미종료 시 프로세스 그룹에 `SIGTERM`, 추가 3초 후 `SIGKILL`을 적용한다. 중계·제품 프로세스 그룹을 정리한 뒤 원자료를 봉인한다. `runtime.json`의 `shutdown`에는 신호 단계·정리 시간·미완료 호출이, `budget`에는 관측된 상한 초과분과 종료 수집 중 추가 토큰이 남는다. 실행 종료 시각과 정리 완료 시각을 구분하며 종료 뒤 도착한 결과는 모델 전달로 세지 않는다.

예산 감시는 캐시를 포함한 입력+출력 합계만 사용한다. 내장 `rollout_budget` 옵션은 제거했다. 고정 Codex 버전은 서버가 제공하는 예산 단위를 우선하고, 없으면 캐시를 제외한 입력에 가중치를 적용하므로 이번 실험과 계산 단위가 다르다([고정 버전 소스](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/rollout_budget.rs#L42)). 요청 완료 단위의 사용량 보고 때문에 상한을 넘을 수 있으며, 미완료 요청이 있으면 기록된 초과분도 관측한 최소치다.

A의 `read`는 B와 같은 `N→원문` 행 번호 형식을 사용하고 파일 경로는 요청 인자로 유지한다. 읽기 범위와 출력 제한은 그대로다. 근거 분석은 기존 `path:N:원문`과 새 형식을 모두 지원하며, 출력 끝에서 잘린 행은 원문 근거에서 제외한다.

제한 감지와 최종 응답 완료가 겹칠 수 있다. 이때도 정상 턴 완료·정상 프로세스 종료·기록 수집·미완료 호출 부재를 모두 확인하면 전체 비용을 확정하며, 제한 감지 상태와 초과분은 보존한다. 최초 집계와 재집계에서 전체 토큰 판정이 달라졌다면 `usage_reaggregation`에 두 값을 남긴다. 채점용 답변의 절대 파일 링크에 포함된 실행 ID는 익명 ID로 치환하고, 채점 인용문은 원본 답변으로 복원한 뒤 다시 검증한다. 원본 답변과 Astra 원출력은 바꾸지 않는다.

탐색 호출은 중계기가 받은 A/B 요청별로 센다. 한 `exec`에 요청 6개가 있으면 6회다. 오류·재호출·B 초기 안내도 포함한다. 모델에게 실제 전달된 `custom_tool_call_output`을 제품 원출력과 구분하며, 일치하지 않는 변형·생략·잘림의 근거는 관측 판별 불가로 남긴다. 파일명만으로 원문 노출을 인정하지 않는다. 검색/개요의 제품 내부 잘림 표시를 확정할 수 없는 경우에도 `null`을 사용한다.

A/B 정확도 차이는 pp, 비용 변화는 **평균의 비율**이다. A 평균이 0이면 변화율 대신 절대 차이를 기록한다. 평균 총토큰 또는 탐색 호출이 반올림 전 25% 이상 증가하면 비용 급증이다. 정확도 부트스트랩 구간이 0을 포함하는 상승은 확정 근거 부족으로 표시한다. 문제별 퇴행·중대 오류와 양의 추가 토큰 기여도를 따로 공개한다.

양쪽 모두 정답인 동일 문제·반복의 비용과 전체 유효 토큰/완전 정답 수는 보조 지표다. 포함된 대응쌍·문제 수를 표시하며 재시도 기대 비용으로 해석하지 않는다. 30문제 결과는 이 데이터셋에 한정한다.

## 결과 확인과 보존

각 실험의 `report/report.md`에 전체, 난이도·언어, 문제별, 비용 증가·실패, 하네스 유효성의 다섯 표가 생성된다. `report/summary.json`과 `normalized.json`에는 값·단위·분자·분모·유효성·결측 사유·원자료 위치가 있다. 재집계는 검증한 원시 로그에서 관측값을 다시 만들며 봉인한 기록을 덮어쓰지 않는다.

Codex 종료 뒤 이미 요청된 도구의 결과가 도착할 수 있다. 원래 해시와 정확히 일치하는 JSONL 접두사, 기존 미완료 요청 ID, 종료 뒤 시각이 모두 확인된 추가분만 `terminal-seal.json`으로 별도 고정한다. 이를 모델이 받은 결과로 세지 않는다. 기존 바이트 수정이나 새 요청 추가는 계속 검증 실패다.

| 경로 | 내용 |
| --- | --- |
| `frozen.json`, `schedule.json` | 고정 명세와 빠짐없는 예정 슬롯 |
| `preparation/` | 별도 색인 준비 시간·크기·소스 해시 |
| `runs/<id>/` | 원시 Codex/중계 기록, 답변, 실행 상태, 계측, 파일 해시 |
| `grading/` | 블라인드 묶음, 비공개 대응표, 실제 Astra 기록·비용, 대조 검사 |
| `report/` | 실행별 정규화 지표, 집계, 불확실성, 판정, 보고서 |

하네스 구축/사전 검증 완료 여부와 제품의 정확도·효율 목표 달성 여부는 별도 결론이다. 보고서 생성 자체가 제품 개선을 뜻하지 않는다. 자료가 불완전하면 해당 비교를 보류한다. 비용 원인에 대해서는 관측된 반복·출력·오류와 성공의 연관성을 인과관계로 단정하지 않는다.

### 준비 재실험 결과

`preparation-r2`는 같은 준비 6문제를 A/B 각 1회, 총 12회 실행하는 별도 실험이다. 기존 `preparation` 원자료·답변·채점·보고서는 보존하고 합산하지 않는다. `probe-r2`의 런타임 검사 2회와 Astra 채점 비용도 평가 비용과 분리한다. 질문·정답 계약, 제품과 Grafana 커밋, 모델·추론 강도, 시드, 실행 상한은 유지한다. 실행 조건이 개선된 준비 실험 사이의 관측이며 본평가 30문제·120회를 대체하지 않는다.

결과 경로는 [새 준비 보고서](v2/artifacts/preparation-r2/report/report.md), [기존 제한 종료 4건의 메모리 재집계](v2/artifacts/reaggregation-r2/legacy-four.json), [별도 런타임 검사 재판독](v2/artifacts/probe-r2/reinspection.json)이다. 런타임 검사의 최초 판정과 원자료도 보존한다. 새 실험은 `run`·`grade`·`report`의 `--experiment`에 `benchmark/v2/artifacts/preparation-r2`를 지정한다. 실패·제한 종료를 지우거나 자동 재실행하지 않으며 본평가는 시작하지 않는다.

기존 하네스 폐기 내역은 [폐기 기록](RETIRED.md), 제품 후보 패치는 [패치 보관 안내](experiments/codex-codemap/README.md)에 남긴다. Codex 설정의 외부 참고는 [공식 설정 문서](https://developers.openai.com/codex/config-reference/)이며, 실제 옵션·이벤트 형식은 고정된 로컬 CLI의 검증 기록을 따른다.

### 후보 제품과 공동 채점

`verify --product-build`는 기존 고정 커밋 빌드와 `source_kind: "snapshot"` 빌드를 구분한다. 스냅샷 빌드는 `source_commit: null`, `base_product_commit`, `source_snapshot`, `source_manifest_sha256`, 실제 빌드 명령과 바이너리 해시를 기록하며 소스 파일·패키지 버전·빌드 명령·바이너리를 다시 대조한다. 후보를 기준 릴리스 커밋의 바이너리로 표시하지 않는다. `improvement-r3`에는 완전한 제품 소스와 재빌드 기록을 보존했다.

공동 채점은 기존·새 답변의 실행 ID와 절대 경로에 포함된 시점 정보를 익명화하고, 사실별 인용문을 원본 답변으로 복원해 검증한다. 근거는 답변 전체의 인용을 공유할 수 있으며, 같은 문장에 반복해서 인용할 필요는 없다. 답변에 없는 근거를 정답 자료에서 가져와 인용한 것으로 간주하지는 않는다. `correct=false`이면 `supported=false`이고, 중대한 잘못된 주장과 모순은 별도로 판정한다. 기존 정답 계약·대조 사례의 기대값은 유지한다.

각 보고서의 `shared_grading` 참조는 공동 채점의 파일 해시·봉인·입력 답변·판정 매핑을 검증한다. 이미 검증된 공동 채점 참조가 있으면 `grade`를 다시 호출해도 모델을 추가 실행하지 않는다. R3의 첫 공동 채점 실패와 근거 범위를 명확히 한 최종 공동 채점은 모두 보존했으며, 평가 12회는 재실행하지 않았다.
