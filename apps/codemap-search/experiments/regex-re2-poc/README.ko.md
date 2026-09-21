# codemap-search 정규식 엔진 비교 PoC

`regex`, RE2, `fancy-regex`를 같은 Rust 프로세스에서 비교하는 독립 실험이다. `codemap-search`의 엔진 선택에 사용할 성능 자료와 동작 호환성을 함께 확인한다.

이 문서의 측정 요약과 실행 코드는 저장소에 보관한다. `results/`의 원자료·자동 요약·부하 로그와 `.cache/`, `target/`은 로컬 생성물로 Git에서 제외한다. 아래 생성 파일 경로는 측정 당시 로컬 출력 위치이며 새 체크아웃에는 포함되지 않는다. `Cargo.lock`은 같은 의존성 버전으로 재현하기 위해 포함한다.

## 재측정 결과와 권장 선택

**부하가 낮은 상태로 다시 측정해도 `regex`와 `grep-regex` 유지 권장은 같다.** 절대 시간은 달라졌고, 반복 검색의 큰 상대 성능 차이는 재현되었다. `fancy-regex`는 lookaround·역참조가 반드시 필요한 별도 기능의 후보로 판단한다.

2026-09-21 15시대에 같은 입력·실행 파일·의존성·측정 코드로 세 번 재측정했다. 각 실행은 46개 조합, 후보별 20회, 목표 배치 40 ms로 초기 측정과 같다. 두 번째 실행에서 다른 작업의 부하가 다시 커져 같은 seed 43을 한 번 더 실행했다. 아래 주 결과는 부하 기록을 기준으로 첫 번째와 세 번째 실행을 사용한다. 제외한 두 번째 원자료도 로컬에 보존했다.

| 실행 | 시간(KST) | CPU 유휴율 최솟값–최댓값 | CPU 유휴율 중앙값 | 주 결과 사용 |
|---|---|---:|---:|---|
| 재측정 seed 42 | 15:24:59–15:27:13 | 59.56–70.70% | 63.95% | 사용 |
| 재측정 seed 43 | 15:27:13–15:29:29 | 4.79–61.16% | 38.14% | 부하 재발로 제외 |
| 추가 seed 43 | 15:30:44–15:32:57 | 49.73–71.43% | 60.39% | 사용 |

유휴율은 `top -l 0 -s 10 -n 0`의 구간 표본이다. 각 모니터의 첫 표본은 제외했으며 실행별 13개 표본을 사용했다. 전체 부하 기록(`results/recheck/system-samples.json`), 처음 두 실행 로그(`results/recheck/system.log`), 추가 실행 로그(`results/recheck/system-retry.log`)에서 확인할 수 있다.

### CPU 부하와 시간 변화

같은 seed 42의 이메일 캡처에서 세 엔진의 경과 시간 중앙값이 모두 약 28–30% 줄었다. 이는 초기 절대 시간을 확정값으로 해석하면 안 된다는 근거다. 스케줄러 대기 외의 메모리·캐시·주파수 영향을 각각 격리한 실험은 아니므로 감소분 전체를 CPU 점유율 하나에 귀속하지는 않는다.

| 엔진 | 초기 seed 42 경과 시간 | 재측정 seed 42 경과 시간 | 변화 |
|---|---:|---:|---:|
| regex | 4.684 ms | 3.288 ms | -29.8% |
| re2 | 9.167 ms | 6.598 ms | -28.0% |
| fancy_regex | 198.411 ms | 137.947 ms | -30.5% |

실행 조합 32개 × 후보 3개 × 실행 2회인 192개 항목에서, 각 항목의 `p90/p10` 비율을 구한 뒤 그 중앙값을 비교했다. CPU 시간은 1.040 → 1.024, 경과 시간은 1.046 → 1.027로 전형적인 표본 편차가 줄었다. 다만 모든 사례가 안정된 것은 아니며, RE2 URL 캡처는 아래 한계에 별도로 기술한다. 전후 비교 원자료(`results/recheck/comparison.json`)

### 현재 사용할 수치

세 번의 재측정에서 컴파일 14개 조합은 모두 성공했고, 실행 32개 조합의 매치·캡처·행 위치는 후보 간 모두 일치했다. 주 결과 두 실행에서 RE2는 실행 32개 중 30개에서 기준보다 CPU 시간이 길었고, 2개에서 짧았다. `fancy-regex`가 기준보다 CPU 시간 중앙값이 5% 이상 작은 실행 조합은 두 실행 모두 없었다. 5%는 작은 차이를 구분하기 위한 표시 기준이며 통계적 유의성 판정이 아니다.

아래는 **스레드 CPU 시간의 중앙값**이다. 상대 시간은 `후보 ÷ 기준`이므로 1보다 작으면 후보가 빠르다. 범위는 재측정 seed 42와 추가 seed 43의 두 중앙값 또는 비율의 최솟값–최댓값이며 신뢰구간이 아니다. `grep` 기준은 현재 `grep-regex`; 그 밖의 기준은 `regex`다.

| 사례·연산 | 기준 µs/op | RE2 상대 시간 | fancy-regex 상대 시간 |
|---|---:|---:|---:|
| 리터럴 · `grep` | 110.92–111.02 | 1.29–1.30배 | 1.00–1.01배 |
| 없는 리터럴 · `grep` | 107.69–107.85 | 7.96–8.03배 | 약 1.01배 |
| 호출자 이름·단어 경계 · `grep` | 1,220.14–1,224.64 | 4.30–4.32배 | 74.82–75.10배 |
| 대소문자 무시 · `grep` | 502.14–519.53 | 9.68–9.97배 | 약 1.02배 |
| 한글 선택식 · `grep` | 328.81–332.97 | 15.12–15.17배 | 약 1.00배 |
| 할당문 · `captures` | 18,811.39–18,854.50 | 약 0.67배 | 3.39–3.40배 |
| Bearer · `captures` | 2,466.62–2,532.05 | 1.85–1.87배 | 15.07–15.35배 |
| 이메일 · `captures` | 3,267.13–3,295.82 | 약 2.01배 | 41.89–42.04배 |
| URL · `captures` | 6,639.69–6,762.59 | 3.74–5.94배 | 1.00–1.04배 |
| 파일명 · `is_match` | 4.33–4.36 | 1.92–1.93배 | 1.07–1.09배 |
| 파일명 · `request` | 56.76–57.45 | 0.50–0.51배 | 약 1.02배 |

RE2의 장점은 컴파일 비용과 특정 패턴에서 드러났다. 14개 컴파일 사례와 주 결과 두 실행에 동일 가중치를 준 상대 시간의 기하평균은 RE2 0.217, `fancy-regex` 0.762였다. 이는 전체 서비스 성능 향상률이 아니다. 파일명 276개를 요청마다 새로 컴파일해 검사하는 경우 RE2가 약 절반의 CPU 시간을 썼지만, 컴파일한 매처를 재사용하면 `regex`가 빨랐다. 할당문 캡처에서 RE2의 CPU 시간은 약 33% 짧았다.

RE2를 제한적으로 검토할 근거는 위 두 사례에 있지만, Unicode 경계 계약과 C++/Abseil 빌드·배포 비용이 추가된다. 특히 이메일처럼 `\w`를 쓰는 패턴의 빠른 컴파일은 같은 Unicode 계약을 구현한 비교가 아니다. 현재의 여러 정규식 사용처를 한꺼번에 교체할 근거로 삼지 않는다.

`fancy-regex`는 리터럴·일부 문자 클래스에서 기준과 가까웠지만, 단어 경계가 있는 호출자 검색은 기준의 74.82–75.10배, 이메일 캡처는 41.89–42.04배의 CPU 시간을 썼다. 고급 문법이 필요하다면 이 버전의 동작과 실행 오류를 명시적으로 다루는 별도 경로를 평가해야 한다.

주 결과의 전체 수치는 재측정 CPU 요약(`results/recheck/summary-cpu.md`), 재측정 경과 시간 요약(`results/recheck/summary-wall.md`), seed 42 원자료(`results/recheck/run-42.json`), 추가 seed 43 원자료(`results/recheck/retry-43.json`)에 있다. 부하가 재발한 seed 43(`results/recheck/run-43.json`)은 주 결과에서 제외한 근거와 함께 남겼다. 초기 자료도 CPU 요약(`results/summary-cpu.md`), 경과 시간 요약(`results/summary-wall.md`), seed 42(`results/run-42.json`), seed 43(`results/run-43.json`)에 보존한다. 요약의 bootstrap 구간은 같은 실행 안의 반복 변동을 보여 주며, 다른 시스템·작업 부하에 대한 보장은 아니다.

### 해석의 한계

초기 1분 load average는 두 실행 시작/종료에 각각 137.04/57.27, 33.49/13.67이었다. 이번 주 결과에서 모니터가 기록한 1분 부하 범위는 각각 5.14–7.65, 6.67–9.01로 낮았다. 다만 완전한 무부하 환경은 아니며, 메모리 압축과 일부 swap-in도 관측했다. CPU 시간으로 스케줄러 대기를 분리했지만 메모리·캐시 경쟁이나 주파수 차이까지 제거한 것은 아니다.

RE2 URL 캡처는 이번에도 CPU 시간 중앙값이 40.20 ms와 24.87 ms로 달랐고, 실행 내 `p90/p10`이 각각 3.56과 2.52였다. 부하가 낮은 상태에서도 변동이 남았으므로 초기 CPU 부하만으로 설명할 수 있다고 단정하지 않는다. 원인을 분리하지 않았으며 이 사례의 정밀한 배율은 미확정이다. 결과는 이 장비, 버전, 입력, 단일 스레드, 기본 옵션에 한정되며 전체 MCP 지연·실제 사용 빈도·최대 메모리·멀티스레드 처리량의 결론은 아니다.

## 측정 범위와 현재 사용처

| 사용처 | 현재 동작 | PoC에서 확인하는 부분 |
|---|---|---|
| [`tools/grep.rs`](../../src/tools/grep.rs) | `RegexMatcherBuilder` → `Searcher` → 결과 행 수집 | 기존 `grep-regex`와 후보 어댑터에 동일한 `Searcher` 적용 |
| [`callers/scan.rs`](../../src/callers/scan.rs) | 이름의 선택식·단어 경계 → 파일 검색 → 호출 위치 분류 | 이름 선택식 검색. AST 분류·호출자 예산은 제외 |
| [`tools/find.rs`](../../src/tools/find.rs) | 컴파일한 `regex::Regex`로 basename 검사 | 실제 소스 파일 276개의 basename `is_match` |
| [`redact/rules.rs`](../../src/redact/rules.rs), [`redact/text.rs`](../../src/redact/text.rs) | 재사용하는 정규식의 `captures_iter` → 탐지 범위 | Bearer·할당문 캡처. 후속 마스킹 처리는 제외 |
| [`redact/pii/patterns.rs`](../../src/redact/pii/patterns.rs) | `RegexSet` 후보 선별 → 경계·맥락·검증기 | 이메일·URL 개별 패턴 캡처와 560개 생성 패턴의 컴파일 호환성 |
| [`events/source_routes/conditional_syntax.rs`](../../src/events/source_routes/conditional_syntax.rs) | 전처리 조건문 식별 | 실제 패턴의 컴파일 비용 |

17개 패턴에 필요한 연산을 배정하여 총 46개 측정 조합을 만든다. PII 전체 파이프라인, `RegexSet` 처리량, 병렬 파일 탐색, 파일 I/O, AST 분석, MCP 응답 생성은 이 실험의 측정 범위에 포함하지 않는다.

## 환경과 방법

- 측정일: 2026-09-21. Apple M4 Pro, macOS 15.7.1, 메모리 64 GiB.
- Rust 1.98.1, Python 3.14.5, Apple Clang 17.0.0. Rust와 C++ 모두 최적화 수준 3.
- `regex 1.13.1`, `fancy-regex 0.19.2`, `regex-automata 0.4.18`.
- RE2 `2025-11-05`, Abseil `20250512.1`. 공식 소스 압축파일을 SHA-256으로 고정하고 PoC 내부에 빌드한다. C++17, ICU 비활성, 기본 RE2 옵션을 사용한다.
- 기존 검색기: `grep-regex 0.1.14`, `grep-searcher 0.1.17`, `grep-matcher 0.1.9`.
- 작업 시작 시 앱은 `regex 1.12.4`였으나 다른 작업에서 의존성이 갱신되었다. 본 측정은 갱신된 앱 잠금파일에 맞춘 위 버전을 기준으로 한다.

| 코퍼스 | 입력 수 | 바이트 | 구성 |
|---|---:|---:|---|
| repository | 276 | 3,252,558 | Git 추적 중인 `apps/`, `packages/`의 `src/` 소스. 실험·vendor·target 제외 |
| basenames | 276 | 3,108 | 위 파일들의 파일명 |
| ascii_generated | 1 | 2,092,712 | 번호가 다른 함수·이메일·URL·가짜 Bearer 문자열 8,192묶음 |
| unicode_generated | 1 | 702,292 | 한글·라틴·그리스 문자와 비ASCII 숫자 8,192행 |
| long_nonmatch | 1 | 2,097,152 | `a`가 반복되는 미일치 입력 |

각 파일 경계는 유지한다. 모든 입력은 UTF-8이며 메모리에 미리 읽는다. 실제 입력 파일별 SHA-256, 생성 코퍼스의 SHA-256, Git 커밋, 잠금파일, 빌드 환경, 측정 코드 및 실행 바이너리 해시를 원자료에 기록한다. 두 번째 실행은 첫 번째와 동일한 입력 스냅샷을 재사용한다.

| 연산 | 한 번의 처리(op) |
|---|---|
| `compile` | 정규식 한 개 생성·해제 |
| `find` | 컴파일한 정규식으로 코퍼스 전체의 비중첩 매치 위치 수집 |
| `captures` | 코퍼스 전체의 매치와 모든 캡처 그룹 범위 수집 |
| `is_match` | 코퍼스의 모든 파일명에 일치 여부 검사 |
| `grep` | 컴파일한 매처와 새 `Searcher`로 모든 파일의 일치 행 수집 |
| `request` | 컴파일·위 사례에 해당하는 검색·해제. 실제 파일 I/O와 MCP 처리는 제외 |

측정 전에 모든 파일/매치/캡처/행의 바이트 위치를 정확히 비교한다. 불일치나 오류가 있으면 해당 조합의 성능 비교를 제외하고 이유를 기록한다. 컴파일 조합에서는 각 후보의 컴파일 성공 여부를 확인한다. 실제 측정 중에는 공통 위치 체크섬만 계산하여 결과 벡터 할당 비용을 제외하고 최적화에 의한 제거를 방지한다.

각 후보를 3회 워밍업하고 배치 반복 수를 보정한다. 목표 배치 경과 시간은 40 ms이며, 실제 반복 수와 시간은 원자료에 남긴다. 각 조합을 20회 측정하고 매 반복의 후보 순서를 섞는다. 전체 조합 순서도 섞으며, seed 42와 43으로 독립 프로세스를 실행한다. 패턴 생성·해제 비용, 재사용 후 처리 비용, 요청마다 컴파일하는 비용을 구분한다. 프로세스 시작 직후의 초기 지연은 측정하지 않는다.

`Instant`로 경과 시간을, `CLOCK_THREAD_CPUTIME_ID`로 스레드 CPU 시간을 함께 측정한다. 높은 시스템 부하 때문에 두 값의 의미를 구분해야 한다. CPU 시간은 스케줄러 대기를 제외하지만 메모리·캐시 경쟁이나 주파수 변화까지 제거하지는 않는다. 시스템의 다른 프로세스를 중단하거나 CPU를 고정하지 않았다.

RE2는 C ABI를 통해 원본 버퍼를 빌려서 검색한다. 매치/캡처 호출마다 Rust↔C++ 경계를 지나는 비용도 포함한다. 두 후보의 `grep` 어댑터는 이 실험의 LF를 소비하지 않는 패턴만 지원한다. 기존 `grep-regex`의 후보 행 최적화 전체를 재구현하지 않았으므로 `grep` 결과는 이 어댑터를 사용한 통합 비용이다. 다중 행 패턴은 `find`로 별도 측정한다.

각 라이브러리의 기본 기능·메모리 한도를 사용했으며 같은 메모리 예산으로 맞추지는 않았다. RE2 캡처 버퍼는 단일 스레드에서 재사용하며 최대 128그룹이다. 이 FFI 코드는 제품용 멀티스레드 어댑터가 아니다.

## 의미와 기능 차이

세 후보는 이 실험의 공통 입력에서 결과가 같더라도 다른 입력에서 같은 계약을 제공하지 않을 수 있다. 특히 ASCII 생성 자료의 PII 속도는 Unicode 마스킹의 동등성을 증명하지 않는다.

- RE2의 `\w`, `\d`, `\s`, `\b`는 Rust 기본 Unicode 처리와 차이가 있다. 예를 들어 `\bfoo\b`로 `한foo글 foo`를 검색하면 Rust 계열은 마지막 `foo`만, RE2는 두 위치를 반환한다. RE2는 `a{1001}`도 거부한다. [RE2 문법](https://github.com/google/re2/blob/2025-11-05/doc/syntax.txt), [Rust regex 문서](https://docs.rs/regex/1.13.1/regex/)
- `fancy-regex`는 lookaround와 역참조를 지원하고 일반 패턴 또는 그 부분은 하위 정규식 엔진에 위임한다. 고급 문법에는 역추적이 필요할 수 있어 `Result` 오류 처리가 필요하다. `^(a|aa)+\1$`에 `a` 32개와 `!`를 준 별도 확인에서 기본 역추적 한도 오류를 관측했다. 이 한 번의 확인 시간은 성능 순위에 사용하지 않는다. [fancy-regex 문서](https://docs.rs/fancy-regex/0.19.2/fancy_regex/)
- 일반 문법이라고 반드시 같은 실행 경로를 쓰는 것은 아니다. 설치된 `fancy-regex 0.19.2`의 `Assertion::is_always_hard`는 `WordBoundary`를 자체 처리 대상으로 분류하고, 이 속성이 있는 식은 `RegexImpl::Fancy`로 컴파일된다. 설치 소스와 공식 커밋 원본의 바이트 일치도 확인했다. 이는 단어 경계가 있는 사례의 지연을 해석할 구현 근거이며, CPU 프로파일링으로 세부 비용을 분리한 결과는 아니다. [분류 구현](https://github.com/fancy-regex/fancy-regex/blob/e2684857abcfd723562864077c6bc5f3f70ecedf/src/lib.rs#L2599-L2615), [컴파일 분기](https://github.com/fancy-regex/fancy-regex/blob/e2684857abcfd723562864077c6bc5f3f70ecedf/src/lib.rs#L1213-L1284)
- 현재 PII 카탈로그의 `search`, `at_start`, `after_character`, 필요한 `left_boundary` 형태 560개는 세 후보 모두 컴파일된다. 이는 컴파일 호환성 결과이며, 모든 실제 PII 입력의 탐지 정확도를 검증한 결과는 아니다.
- `fancy-regex::RegexSet`은 현재 `regex::RegexSet`의 대체 API가 아니다. 전자는 가장 이른 위치의 매치를 반환하고 후자는 어느 패턴들이 입력 전체에서 일치했는지를 반환한다. PII 후보 선별 계층을 교체하려면 이 차이를 별도로 다뤄야 한다. [공식 RegexSet 설명](https://docs.rs/fancy-regex/0.19.2/fancy_regex/#regexset-notes-for-regex-crate-users)

## 재현

아래는 최초 실행 예시다. 이번 재측정은 빌드를 생략하고 입력을 재사용하는 `--skip-build --reuse-input`으로 실행했으며, 결과는 `results/recheck/`에 저장했다. 기존 원자료를 보존하려면 다른 `--output` 경로를 지정한다.

Python 3.12 이상, Cargo/Rust, C++17 컴파일러, CMake 3.22 이상이 필요하다. 최초 실행은 공식 GitHub 원본과 crates.io 의존성을 다운로드한다. 전역 RE2 설치는 필요 없다. `.cache/`와 `target/`만 로컬 빌드·입력 파일을 보관한다. macOS arm64에서 검증했으며 다른 플랫폼은 미검증이다.

저장소 루트에서 실행한다.

```sh
python3 apps/codemap-search/experiments/regex-re2-poc/run.py \
  --samples 20 --sample-ms 40 --seed 42 \
  --output apps/codemap-search/experiments/regex-re2-poc/results/run-42.json

python3 apps/codemap-search/experiments/regex-re2-poc/run.py \
  --skip-build --reuse-input --samples 20 --sample-ms 40 --seed 43 \
  --output apps/codemap-search/experiments/regex-re2-poc/results/run-43.json

python3 apps/codemap-search/experiments/regex-re2-poc/summarize.py \
  apps/codemap-search/experiments/regex-re2-poc/results/run-42.json \
  apps/codemap-search/experiments/regex-re2-poc/results/run-43.json \
  --metric cpu --output apps/codemap-search/experiments/regex-re2-poc/results/summary-cpu.md

python3 apps/codemap-search/experiments/regex-re2-poc/summarize.py \
  apps/codemap-search/experiments/regex-re2-poc/results/run-42.json \
  apps/codemap-search/experiments/regex-re2-poc/results/run-43.json \
  --metric wall --output apps/codemap-search/experiments/regex-re2-poc/results/summary-wall.md
```

`cargo`가 PATH에 없다면 실행 전에 실제 Rust 도구체인 디렉터리를 PATH에 추가한다. 코드나 의존성을 바꿨으면 `--skip-build`를 사용하지 않는다. `--reuse-input`은 기존 `.cache/input.json`과 그 메타데이터가 있어야 한다. 다른 코퍼스를 측정하려면 이 옵션을 빼고 입력을 다시 생성한다. 이전 코퍼스를 재현할 때는 원자료의 파일별 해시와 새 입력 해시를 비교해야 한다.

검증만 실행하려면 `run.py --validate-only --output <결과 경로>`를 사용한다. 원자료에는 각 반복의 경과/CPU 시간, 반복 횟수, 후보 순서, 패턴, 결과 동일성, 호환성 사례가 들어간다. 요약 스크립트는 입력·의존성·핵심 측정 코드가 다른 실행의 통합을 거부한다.

## 수행한 검증

- 초기 PoC 작성 시 `cargo check --manifest-path apps/codemap-search/Cargo.toml`: 성공.
- 초기 PoC 작성 시 `cargo build --release --locked --manifest-path apps/codemap-search/experiments/regex-re2-poc/Cargo.toml`: 성공. 재측정은 동일한 바이너리를 재사용했다.
- 초기 2회와 재측정 3회 모두 46개 조합 완료, 각 후보 20개 시간 표본 확인. 14개 컴파일 조합 성공, 32개 실행 조합의 위치 결과 일치.
- 560개 PII 생성 패턴의 세 후보 컴파일 성공. 14개 별도 기능·Unicode 사례는 차이와 실행 오류를 원자료에 보존.
- 초기 2회와 재측정 3회의 입력·잠금파일·Rust 실행 파일·RE2 공유 라이브러리 해시 일치. 현재 측정 코드와 원자료의 소스 해시, 보고서의 로컬 링크도 확인.
