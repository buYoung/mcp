# 네이티브 Jev 전체 검증

2026-09-23, Rust 내부 공통 평가기와 독립 기본 비활성 두 모드를 구현하고 오프라인 검증했다. 최종 전체 e2e는 종료 코드 0으로 214개 통과, 기존 Clang 의존 테스트 1개 ignored로 완료했다. 실제 TypeSafe 평가 요청, 유료 비교, Noul 품질 보정, 다른 플랫폼 빌드와 게시 작업은 수행하지 않았다.

## 검증 대상과 식별

로컬 환경은 macOS 15.7.1 arm64, `rustc 1.98.1 (48a229cea 2026-09-01)`이다. crate 버전은 기존 `0.10.0`, 공급자 모델 계약은 `jev-1.13.0`이다. 사용자 제한에 따라 worktree, 다른 브랜치와 Git revision을 조회하지 않았다. 커밋·스테이징·병합·게시도 하지 않았다.

현재 `src`, `vendor`, `examples`, Cargo manifest/lock, build script의 290개 파일을 SHA-256으로 기록했다. [전체 목록](source-manifest.json)의 `source_sha256`은 경로·파일 해시를 정렬해 합친 값이다.

- 소스 집합: `20561440938f64b4cb9b0d970173a464bd14feff8b200006222a2265975dc156`
- 디버그 MCP 바이너리: `0d8e68b2652fd6f3382093fb16569daf11a646708f531853ffd16925ee892f8d`
- 직접 호출 예제 바이너리: `3a0ef78feda081df488f391d599b025718a54399f458497b87498da42ed64489`

네이티브 검증은 명시적으로 주입한 transport만 사용한다. 실제 HTTPS 구현의 연결 재사용은 동일 client 설정을 이용한 loopback HTTP 서버에서 확인했다. 일반 운영 요청으로 mock이나 대체 공급자 주소를 선택할 수 없다. 키 없음 사례는 테스트 전용 환경변수 이름을 사용하고 해당 변수를 제거한 별도 프로세스에서 검사했다.

## 실제 명령 결과

모든 명령의 작업 디렉터리는 저장소 루트다. 각 로그는 실제 실행 결과이며 초기 실패도 보존했다.

| 명령 | 결과 | 근거 |
| --- | --- | --- |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets` | 종료 0 | [all-targets.log](all-targets.log) |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests` | 종료 0, 214개 통과·기존 ignored 1개, 211.16초 | [e2e.log](e2e.log) |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev` | 종료 0, 27개 통과 | [jev-unit.log](jev-unit.log) |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib config` | 종료 0, 36개 통과 | [config-unit.log](config-unit.log) |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev` | 종료 0, 6개 통과 | [jev-integration.log](jev-integration.log) |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::exclusions` | 종료 0, 4개 통과 | [exclusions.log](exclusions.log) |
| `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock` | 종료 0, 세 원시형과 usage 검사 통과 | [example.log](example.log) |
| `cargo package --manifest-path apps/codemap-search/Cargo.toml --list --allow-dirty` | 종료 0, 포함/제외 경계 확인 | [package.log](package.log) |

27개 Jev 필터 단위 검증에는 런타임 11개, overview 7개, search 7개, 설정 2개가 포함된다. 앞선 개별 `--lib tools::overview`, `--lib tools::search`, `e2e::search`, `e2e::config`, `e2e::mcp`도 각각 7/7/24/9/26개가 통과했다. 기본 e2e 실행에서 `test_macro_expansion_reaches_search_read_and_refreshes_header_changes`는 기존 `Requires an installed Clang preprocessor` 사유로 ignored 상태다. 이를 실행하거나 다른 플랫폼에서 통과했다고 보고하지 않는다.

## 회귀 행렬과 실제 모집단

| 경계 | 실제 시나리오와 입력 | 확인한 결과 |
| --- | --- | --- |
| 공통 평가기 | 혼합 Score/Choice/Noul, 구조화된 지시문/기준, 2–10 Score 단계, 255 Choice 상한 | Noul confidence 없이 수용; ID·모델·형·확률·가중 Score·누락 검증 |
| 전송 정책 | 120개 포장 질문, 100개 aggregate 경계 질문, 별도 6배치 동시성 입력 | 요청 바이트와 두 context 추정 경계, 동시 최대 3개·시작 간격 300ms 확인 |
| 취소·실패 | 짧은 deadline, 전송 전/중 취소, 두 context 거절, 429/529, malformed 응답 | 재시도 없음, permit 반환, 알려진 usage 보존, 부분 답변 성공 없음 |
| 연결 재사용 | loopback 서버, 짧게 설정한 40ms 유휴 수명 | 연속 두 요청은 한 연결, 100ms 대기 후 새 연결 |
| 전체 root 후보 | 30/31개 파일, 긴 한글 문서의 여러 조각, 별도 세 선언 파일 | 전체 평가 후 적합성 판정, 최대 24개, 경로 동점 정렬, 파일당 두 역할, read 최대 180줄 |
| 무추천·불확실성 | unrelated/tangential 분포와 확률 동률 | 무추천과 insufficient_evidence 구분, role 단계 생략, 소스 부재 주장 없음 |
| 호스트 필터 정책 | Noul 0/0.5/0.69/0.70/0.71/1, 동일 0.8을 threshold 0.7/0.9로 재판정 | 원시 답변 재사용, threshold의 실제 retention 적용, 잘못된 답변 전체 복구 |
| 보호 근거 | 부분/누락/과대/알 수 없는 선언, Rust impl/struct/constant, TypeScript 중첩 메서드, 3함수 연결과 동명 후보 | 불완전·구조·중첩·연결·모호한 근거 유지 |
| 실제 parser/index/MCP | 별도 프로세스당 Rust 2파일·TypeScript 1파일, 모든 모드 조합 | 비활성 출력 동일, 독립 활성화, no-key/no-intent/공백/잘못된 형 처리, read/find/grep/event-only 미평가 |
| 실제 Noul=1 보호 | Rust struct/impl/run/constant와 TypeScript class/run/finish | 실제 선택 본문을 1.0으로 판단해도 보호 원문과 기본 결과 유지; 평가 질문 0개로 통과하지 않음 |
| 요청별 설정 고정 | 전송을 지연한 동안 0.70→0.90 변경, 답변 0.80 | 진행 중 요청은 생략, 다음 요청은 유지; 실제 메타데이터 threshold도 각각 일치 |
| 실제 스냅샷 갱신 | 3파일 root 평가 중 4번째 파일을 쓰고 인덱스 갱신 | 진행 중 결과·질문은 3파일, 다음 호출은 4파일; 세대 혼합 없음 |
| 마스킹·원문 보존 | 테스트용 password 값, 마스킹 on/off, 보관한 원본 index 사본 | 전송 전 탐지값 마스킹, 명시적 off 존중, 파일·보관한 index 내용 동일 |
| 최종 관측·cap | 생략/유지 요청과 240바이트 검색 한도, 실제 SQLite 기록 | 전부 생략된 source는 파일 관측 0개; 유지 source만 기록; 응답 바이트는 최종 텍스트와 일치 |
| 같은 줄의 선언 | 한 줄의 `discard`와 `neighbor` 함수 | 재현 실패 후 소유 단계에서 보완; 같은 표시 줄의 다른 선언까지 보존 |
| 스키마·문서·패키지 | 두 모드 조합별 tools/list·initial_instructions·initialize, 양 언어 설정 예시 10키 | 선택적 task_query, 활성 도구만 openWorldHint, 부수 추론 없음; 템플릿과 예시 일치 |

직접 예제는 engine/workspace 없이 Score=2.0, Choice=keep, Noul=0.9, 입력 123/출력 11 fixture 토큰을 검사한다. 패키지 목록에는 공통 Rust 모듈·두 어댑터·예제가 포함되고 `experiments/`, `validation/`, `tests/fixtures/`는 제외된다. package 목록 검사는 게시·릴리스 archive 생성·플랫폼별 빌드 검증을 의미하지 않는다.

## 실패와 소유 단계 보정

1. 최초 MCP 행렬의 Rust 선언 종류 기대값을 `function`으로 잘못 둬 실패했다. 기존 파서의 `fn` 표기로 고친 후 통과했다.
2. 같은 물리적 줄의 이웃 선언까지 한 source 조각에 들어가는 재현에서 이웃 본문이 사라졌다. 부모가 03을 다시 열어 선언 owner 비교와 다른 선언이 겹친 표시 줄의 보호를 보완했다. 관련 search 단위와 실제 MCP 재현을 통과한 뒤 전체 검증을 재개했다.
3. 첫 전체 e2e는 210개 통과, 3개 실패, 기존 ignored 1개였다. 세 실패 모두 실제 schema 24와 기존 테스트 기대 schema 23의 불일치였다. 04 범위에서 기대값만 24로 변경했고 제외 동작 검증 4개가 통과했다. [초기 실행 로그](e2e-initial.log)를 성공 로그와 분리했다.

## 과거 측정은 별도 참고 자료

아래 값은 `/Users/buyong/.codex/checkpoints/codemap-search-comparison/JEV-COMPARISON.md`의 기존 관측을 옮긴 것이다. 새 Rust Jev 구현의 측정값이 아니다. rg B는 1회, 나머지는 같은 과제의 3회 평균이다. #11.1은 822파일, Python 프록시 #1/#2는 815파일이며 CLI 버전·캐시 조건도 달라 네이티브 비교 기준으로 대체할 수 없다.

| 과거 지표 | rg B | 기존 #11.1 | Python #1 개선 | Python #2 개선 |
| --- | ---: | ---: | ---: | ---: |
| 전체 시간(초) | 461.539 | 338.053 | 335.575 | 309.498 |
| 메인 입력 | 1,262,380 | 1,044,685.3 | 935,599 | 848,363.7 |
| 캐시 입력 | 1,110,144 | 916,138.7 | 808,960 | 720,981.3 |
| 캐시 제외 입력 | 152,236 | 128,546.7 | 126,639 | 127,382.3 |
| 출력·추론 포함 | 12,933 | 9,736.3 | 9,319.3 | 9,050 |
| 추론·출력의 부분집합 | 5,833 | 3,997.7 | 3,352.7 | 3,429.3 |
| Jev 입력 | 0 | 0 | 702,208.7 | 41,269 |
| Jev 출력 | 0 | 0 | 32,894.7 | 2,129 |
| 메인+Jev 토큰 처리량 | 1,275,313 | 1,054,421.7 | 1,680,021.7 | 900,811.7 |
| 전체 11항목 충족 | 9/11 | 24/33 | 21/33 | 23/33 |

과거 #1 핵심 파일의 상위 24개 포함은 14/15, #2의 검색 품질 근거 줄 보존은 15/15였다. 같은 입력에서도 #1의 비교 48쌍 중 모든 답변 필드가 동일한 경우는 0쌍이었다. 이는 확인한 과거 관측이며 전체 입력의 정확도·결정성·Rust 속도에 대한 주장이 아니다.

과거 12세션 중 채택은 6세션이었다. 연결 초기화 오류로 중단한 3세션은 확인된 Jev 입력 55,625/출력 3,111토큰, 닫는 fence와 다음 파일 제목이 붙은 출력 오류로 제외한 3세션은 입력 122,334/출력 6,156토큰이었다. 실패 요청의 미보고 usage는 알 수 없다. 인증 확인 3회도 비교 본체에서 제외했다. 상세 메인 모델 중단 사용량과 원본 시도 기록은 기존 외부 근거에 남아 있으며 이번 저장소에 사적인 benchmark source나 키를 복사하지 않았다.

## 이후 별도 승인된 live 비교 방법

동일 Rust 바이너리·소스·인덱스 해시, 원래 task_query, workspace 범위, 모델, 출력 한도, threshold, 관계/인덱스 캐시 상태를 고정한다. 네이티브 off/#1/#2를 같은 조건에서 각각 3회 실행하고 실제 applied 여부를 기록한다. 과거 Python/rg 값은 참고 열로만 둔다. 별도 명시적 실행 지시와 운영자 자격 증명 없이 유료 평가를 시작하지 않는다.

메인 입력·캐시 입력·캐시 제외 입력·출력·추론 부분집합과 Jev 입력/출력을 각각 기록한다. `캐시 제외 입력 = 입력 - 캐시 입력`, 메인 총량은 입력+출력이다. 추론은 출력에 이미 포함되므로 다시 더하지 않는다. 합산 토큰은 처리량이며 비용과 같지 않다. 비용은 해당 실행의 실제 모델·캐시·청구 기준으로 따로 계산하고 미보고 실패 usage는 미확인으로 둔다.

전체 과제 시간, Jev 단계의 실제 벽시계 시간, HTTP 누적 시간, 도구/read 호출 수, 응답 바이트, source 감소량, API·fallback 실패를 별도 기록한다. `metrics.elapsed_ms`는 대기·포장을 포함한 평가기 호출 시간(overview 두 단계 합산)이고 `http_elapsed_ms`는 완료된 전송 누적 시간이다. 병렬 HTTP 누적 시간을 과제 시간에 다시 더하지 않는다. 모든 시도와 제외 사유·알려진 usage를 남기며 품질 점수를 보고 실행을 골라내지 않는다.

실행 전에 품질 항목·정답 근거·채점 규칙을 고정한다. root 파일 recall은 고정한 필요 파일 집합 대비 추천 포함, source 보존은 파일 경로+원래 줄 내용과 선언 범위·파일 헤더·fence로 검사한다. 본문을 보존했다는 사실을 최종 답변 정확도와 동일시하지 않는다. 반복 요청은 요청/증거 해시와 원시 Score·Choice·Noul을 함께 비교한다. threshold 보정은 별도 대표 데이터와 보류 질의를 사용하며 아직 미수행이다. 3회 표본으로 p95·통계적 유의성·품질 동등성·일반 정확도를 주장하지 않는다.
