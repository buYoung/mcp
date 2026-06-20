# 건설적 검토 — cms-official-benchmark-20260619-03

> 검토 유형: constructive (accept/reject 판단 없음, 개선점 생성만)
> 검토 대상: `report.md`, `analysis/detailed_report.md`, `analysis/limitations_and_integrity.md`
> 검증 데이터: `analysis/scored_episodes.180.json` (node 표본 확인)
> 검증 기준: `analysis/key_facts_digest.md`

---

## 요약

| 심각도 | 건수 |
|--------|------|
| 주요   | 3    |
| 경미   | 4    |
| 참고   | 3    |
| 합계   | 10   |

---

## 주요 (Major)

### F1 — codex deno codemap "win(밴드 경계)"는 규칙과 모순 — 실측 tie

**위치**: `analysis/detailed_report.md` §3 codex-gpt54 표 "no-mcp 대비" 셀

**문제**:
`codemap=win(+0.04, 밴드 경계)`라고 표기했으나 실측 delta = 0.7708 − 0.7292 = 0.0416이며, 동률 밴드는 ±0.125(deno F=8 기준)이다. 0.0416 < 0.125이므로 밴드 규칙대로라면 **tie**여야 한다. "win(밴드 경계)"는 동률 밴드 규칙과 직접 모순된다.

**근거(confirmed)**:
`scored_episodes.180.json` 직접 확인:
- `codex-gpt54` / `deno-main` / `codemap` mean = 0.7708 (n=3)
- `codex-gpt54` / `deno-main` / `no-mcp` mean = 0.7292 (n=3)
- delta = 0.0416
- 동률 밴드 ±0.125(`key_facts_digest.md §A`, `limitations_and_integrity.md §1`)
- 0.0416 < 0.125 → **tie**

**영향**:
헤드라인 §1 "두 모델 모두 codemap 최강"에서 claude deno codemap(delta=+0.48, 명확한 win)과 codex deno codemap(delta=+0.04, tie)이 동일한 "교차복제"로 묶인다. 2-모델 교차복제의 강도가 실제보다 과장될 수 있다.

**권장개선**:
1. 표 셀: `codemap=win(+0.04, 밴드 경계)` → `codemap=tie(+0.04, 밴드 내)`
2. 헤드라인 또는 헤드라인 직후: "단, codex deno codemap은 delta=+0.04로 밴드 내 tie; claude(delta=+0.48)와 강도 비대칭"을 명시
3. report.md §1·§3의 "두 모델 모두 codemap 최강" 표현을 "방향 일치(codex는 tie-edge)"로 완화

---

### F2 — opencode-serena "27 episode 평균 0.151" — 분모 n=22 미명시

**위치**: `report.md §3`, `analysis/detailed_report.md §3`, `analysis/key_facts_digest.md §E`

**문제**:
"opencode-serena 신규 27 episode 평균 **0.151**(deepseek 0.188 / mimo 0.143 / minimax 0.125)" 및 코드베이스별 "CH 0.047 / deno 0.167 / angular 0.242"는 수치 자체는 정확하나 **분모가 valid n=22**임을 명시하지 않는다. 본문이 "신규 27 episode 평균"이라고 써서 분모=27로 오독 가능. 전수 27 기준 실측 평균은 0.1227로 다르다.

**근거(confirmed)**:
`scored_episodes.180.json` 직접 확인:
- opencode-serena 전수 27개 score 평균: **0.1227**
- `timed_out=true` 3개(score=null): opencode-deepseek deno, opencode-mimo angular, opencode-minimax ClickHouse
- valid(`harness_valid && !timed_out`) n=22 평균: **0.1506 ≈ 0.151** ✓
- runtime별 valid mean: deepseek n=7 → 0.1875 ≈ 0.188 ✓, mimo n=7 → 0.1429 ≈ 0.143 ✓, minimax n=8 → 0.1250 = 0.125 ✓
- 코드베이스별 valid mean: CH n=8 → 0.0469 ≈ 0.047 ✓, deno **n=6** → 0.1667 ≈ 0.167 ✓, angular n=8 → 0.2422 ≈ 0.242 ✓

**영향**:
"신규 27 episode 평균 0.151"을 전수 기준으로 읽으면 timeout 에피소드가 암묵적으로 평균 0(또는 제외됐음)을 모르는 독자에게 혼동. deno n=6이 유독 적은 이유(timeout 1건)도 미설명.

**권장개선**:
1. 모든 출처에서 "신규 27 episode 중 valid 22개 평균 0.151(timeout 3개 제외)"로 분모 명시
2. 코드베이스별 n 병기: "CH n=8 / deno n=6 / angular n=8"
3. `key_facts_digest.md §E` 첫 줄에 "(valid n=22 기준; timeout 3개 제외)" 추가(서술 에이전트가 올바른 분모로 인용하도록)

---

### F3 — backend_off 24 구성 경로 — runtime별 내역만 있고 serena/non-serena 분해 누락

**위치**: `report.md §2`, `analysis/limitations_and_integrity.md §5`

**문제**:
`report.md §2`에서 backend_off 24를 "deepseek 7 / mimo 12 / minimax 5"라고만 기술하며, 이것이 non-serena carryover 12개 + 신규 opencode-serena 중 12개의 합임을 독자가 연역해야 한다. "153 기준 39→12 / 180 기준 24로 재산출" 설명도 중간 단계(153 carryover 12 + 신규 12)가 누락돼 있다. `limitations_and_integrity.md §5` 표는 이 구조를 담고 있으나 `report.md`에는 연결이 없다.

**근거(confirmed)**:
`scored_episodes.180.json` 직접 확인:
- backend_exercised=false 24개 분포:
  - non-serena 12개: deepseek-codegraph 3, mimo-codegraph 4, mimo-codemap 4, minimax-codemap 1
  - opencode-serena 12개: deepseek-serena 4, mimo-serena 4, minimax-serena 4
- runtime별 합계: deepseek 3+4=7 ✓, mimo 4+4+4=12 ✓, minimax 1+4=5 ✓
- 153 기준(opencode-serena 제외) backend_exercised=false: 12개 ✓

**영향**:
독자가 "deepseek 7"이 어느 backend들의 합인지 추적 불가. 신규 opencode-serena 27개 중 12개가 backend_off임을 파악하려면 §5 표를 별도로 찾아야 한다.

**권장개선**:
`report.md §2` backend_off 설명을:
```
backend_off 24 = 153 carryover 12(opencode non-serena) + 신규 opencode-serena 중 12개
  · deepseek 7(non-serena 3 + serena 4)
  · mimo 12(non-serena 8 + serena 4)
  · minimax 5(non-serena 1 + serena 4)
```
로 세분화하거나, 최소한 `limitations_and_integrity.md §5` 교차참조 추가.

---

## 경미 (Minor)

### F4 — 헤드라인 "2-모델 교차복제" — n=3·descriptive 한계 선제 고지 미흡

**위치**: `report.md §1`, `analysis/detailed_report.md §1`

**문제**:
"claude·codex 둘 다 usable 비교이며, 독립적으로 같은 패턴을 복제한다"는 헤드라인이 전면에 나오고 n=3/cell descriptive only 단서는 §4 이후로 지연된다. F1에서 확인했듯 codex deno codemap은 실제로 tie-edge여서 교차복제의 강도가 비대칭임에도 헤드라인에서는 동일 강도처럼 읽힌다.

**권장개선**:
헤드라인 또는 §1 첫 단락에 "n=3/cell descriptive — 방향 일치이며 강도 비대칭(claude deno codemap win vs codex deno codemap tie-edge)" 단서를 선제 제공. 독자가 §4까지 읽지 않아도 제한을 인지하도록.

---

### F5 — codex ClickHouse codegraph "tie-high" — 비표준 표현

**위치**: `analysis/detailed_report.md §3` codex-gpt54 ClickHouse 표

**문제**:
`codegraph=tie-high`라고 표기했으나 "tie-high"는 밴드 규칙에 없는 비표준 표현이다. 실측 delta = 0.7917 − 0.6667 = 0.125, 동률 밴드 ±0.25(ClickHouse F=4) 기준 0.125 < 0.25 → tie. "tie-high"가 "거의 win" 또는 "상단 tie"로 오인될 수 있다.

**근거**: `scored_episodes.180.json`: codex ClickHouse codegraph mean=0.7917, no-mcp=0.6667, delta=0.1250. 밴드=±0.25 → tie.

**권장개선**: `tie-high` → `tie(+0.13)` 또는 단순 `tie`. "tie-high"라는 비표준 표현 전체 문서에서 제거하고 밴드 규칙을 일관 적용.

---

### F6 — -02 수락 결론 정정 통지 — detailed_report 내 맥락 부족

**위치**: `analysis/detailed_report.md §1`

**문제**:
`detailed_report.md §1`에서 "–02 보고서에서 핵심 걸림돌이었던 codex behavioral null 결론은 정정된다"고 기술하나, **사용자가 이미 수락한 결론**임을 명시하지 않는다. `report.md §0`과 `limitations_and_integrity.md §4-3`에는 명시돼 있어 파일 간 일관성이 없다. -03을 처음 보는 독자가 `detailed_report.md`만 읽으면 정정의 무게를 과소 인지할 수 있다.

**권장개선**:
`detailed_report.md §1` 첫 단락: "-02 보고서에서 핵심 걸림돌이었던 **"codex behavioral null"** 결론" → "-02 HALT2에서 **사용자가 수락한** **"codex behavioral null"** 결론"으로 수정.

---

### F7 — Angular 앵커 variance — §11 무결성 요약에 누락

**위치**: `analysis/limitations_and_integrity.md §3 vs §11`

**문제**:
§3에서 "0.5625/0.1875 값들이 이번 180 executed 집계 밖의 앵커-검증 시도값"임을 명시했으나 §11 무결성 한 줄 요약에는 이 구분이 없다. 독자가 §11만 읽으면 이번 런 angular 점수에 약 3배 스윙이 있다고 오독할 수 있다.

**권장개선**:
§11 무결성 한 줄 요약에 "앵커-검증 variance(0.5625/0.1875)는 이번 런 집계 밖의 선행 시도값이므로 이번 런 angular 실측값과 별개(§3 참조)"를 추가.

---

## 참고 (Nit)

### F8 — "5 model 풀링 backend 순위" 표 — 제목이 내용과 불일치

**위치**: `analysis/detailed_report.md §3` 보조 풀링 표

**문제**:
표 제목이 "5 model 풀링 backend 순위"이나 실제로는 claude-sonnet 수치만 나열한다. "claude 기준"이라고 각 셀에 명시돼 있고 본문에 "풀링은 보조 지표로만"이라는 경고가 있으나 제목이 오해 유발.

**권장개선**: 제목을 "claude-sonnet 기준 backend 순위(clean 비교 기준선)"로 변경.

---

### F9 — key_facts_digest §E — 분모 미명시로 서술 에이전트 오기재 위험

**위치**: `analysis/key_facts_digest.md §E`

**문제**:
digest §E가 "opencode-serena(신규 27)"라고 표기하며 수치 0.151을 제공하나 분모가 valid n=22임을 명시하지 않는다. 이미 report.md §3이 이 digest를 소스로 하여 "신규 27 episode 평균 0.151"이라고 오기재했다(F2). 향후 서술 에이전트도 동일 오류를 반복할 수 있다.

**권장개선**: digest §E 첫 줄을 "(valid n=22 기준; timeout 3개 제외)"로 수정 후 수치 제공.

---

### F10 — §11 무결성 요약 — "측정된 0 / 미계측 0" 구분과 §4-4 교차참조 누락

**위치**: `analysis/limitations_and_integrity.md §11`

**문제**:
§11에서 "미계측(not instrumented)이라 0을 측정값으로 보지 않음"이라고 기술하나 §4-4(텔레메트리 버그 클래스: codex 하드코딩 교정 완료 / opencode task 위임 미교정)와의 교차참조가 없다. 독자가 두 절을 연결하려면 본문 전체를 읽어야 한다.

**권장개선**:
§11 요약에 "도구수 0이 미계측(not instrumented) 케이스 2종: ① codex toolEvents 하드코딩(교정 완료) ② opencode task 위임 과소집계(미교정) — §4-4 참조"를 추가.

---

## 검증 메모

| 항목 | 방법 | 결과 |
|------|------|------|
| 셀 평균 전수 | `scored_episodes.180.json` node 직접 계산 | claude·codex 12 셀 전부 보고서 수치와 ±0.01 이내 일치(confirmed) |
| backend_off 24 구성 | `backend_exercised=false` 필터링 | deepseek 7/mimo 12/minimax 5 = 24(confirmed) |
| codex deno codemap delta | mean 차이 직접 계산 | 0.0416 → ±0.125 밴드 내 = tie(F1 근거) |
| opencode-serena 평균 분모 | valid 필터 적용 | n=22 평균 0.151 = 보고서 수치(F2 근거) |
| 수치 라벨 | confirmed / inferred 구분 | 계산값 전부 scored_episodes.180.json 직접 확인(confirmed); confound 원인 귀속은 inferred로 유지 |

