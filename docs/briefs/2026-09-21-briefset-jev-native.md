# 브리프셋: codemap-search의 네이티브 Jev 판단 단계

> 후속 사용자 요청이 아래 최초 계약보다 우선한다. 현재 #1은 공통 overview의 전체 파일별 개요(표시 제한 제거)를 Score로 판단하며 별도의 색인 문서·호출 투영과 Choice 역할 단계를 사용하지 않는다. #2의 도구 범위는 `search`·`read`·`grep`이다. 아래 최초 계약과 완료 체크는 최초 구현 시점의 기록이며, 현 구현과 생략·보존·복원·관측 회귀의 최종 증거는 [07-overview-live-regressions.md](../../apps/codemap-search/validation/jev-native/07-overview-live-regressions.md)를 따른다.

## 목적
- Python PoC에서 조사한 두 판단 위치를 기존 codemap-search Rust 크레이트 안에 구현한다. #1은 인덱스 기반 파일 추천, #2는 검색이 이미 선택한 본문의 보수적 필터링이다.
- 공통 평가기 → 독립 어댑터 → MCP·설정 통합 → 전체 검증 순서로 완결한다. 모델은 의미를 판단하고, Rust는 후보 구성·순위·보호 규칙·출력·실패 처리를 소유한다.
- 수정된 추천 자격 정책과 Noul 필터의 실제 품질은 아직 입증되지 않았다. 오프라인 기능 완성, 과거 Python 측정, 향후 실제 모델 평가를 구분한다.

## 실행 기준과 시작 점검
이 문서는 실행할 계약이다. 과거 브랜치·인계 문서의 존재나 체크 표시가 이 개정 계약의 완료 증거는 아니다.

| 구분 | 문서 개정 시 확인한 상태 | 실행 시 처리 |
| --- | --- | --- |
| 원래 조사 기준 | `main` 계열 `97e3ebc8e` | 당시 코드 설명의 기준이며 강제 reset 대상이 아니다. |
| 문서 작성 체크아웃 | `docs/jev-integrate`, `530e476c3` | 문서 작업 위치와 실제 구현 위치를 구분한다. |
| 기존 구현 후보 | `feat/codemap-jev`, `2feadb26a` | 구현·PoC·01–05 인계 파일이 있다. 현재 계약과 차이를 조사하고 재사용·수정·검증 범위를 정한다. |
| PoC 위치 | 현재 문서 체크아웃에는 `apps/codemap-search/experiments/jev-playground/checkpoint-poc/`가 없고 기존 구현 후보에는 있다. | 경로가 존재한다고 가정하지 않는다. 필요한 소스만 읽기 전용으로 참고하고 자격 증명·사적 벤치마크 입력은 복사하지 않는다. |

구현 실행을 요청받으면 상위 담당자가 Wave 1 전에 다음을 기록한다. 기록 위치는 이 문서의 실행 기록 또는 `01-runtime.md` 인계 문서의 시작 점검 절이다.
1. 실제 저장소 루트, cwd, 브랜치, HEAD, 작업 트리 변경, 사용할 브리프 개정 상태를 확인한다. 기존 변경을 지우거나 기존 브랜치를 재생성하지 않는다.
2. 원래 구현 대상은 `feat/codemap-jev`이다. 현재 승인된 실행 위치를 확인하고, 기존 구현을 보완할지 승인된 새 기준에서 구현할지 기록한다. 이 문서를 읽었다는 이유만으로 checkout/reset/cherry-pick/merge하지 않는다.
3. 기존 구현 후보의 코드와 증거를 현재 계약에 대조한다. 이미 만족하는 부분은 재검증 후 재사용하고, 과거 통과 로그만 복사하지 않는다. 다른 브랜치의 문서 변경을 가져올 때도 완료 체크는 가져오지 않는다.
4. PoC·공식 문서·과거 보고서의 실제 이용 가능 경로와 확인한 버전을 기록한다. PoC가 없어도 아래 계약과 공식 API로 구현 가능한 작업은 계속한다. 빠진 자료 때문에 동작 선택이 달라지는 경우에만 해당 선택을 확인한다.
5. 모든 아래 Cargo 명령은 확인한 저장소 루트에서 실행한다. 필터가 0개 테스트를 선택하거나 fixture가 비어 있는 통과를 증거로 인정하지 않는다.

## 하위 브리프와 완료 추적
이 목록이 하위 작업 완료 상태의 단일 원본이다. 하위 문서는 요구사항 체크 목록을 유지하되 별도의 전체 완료 상태를 만들지 않는다. 담당자는 작업 역할이며, 이 분할 자체가 subagent 실행을 승인하지는 않는다.

- [x] [01 — 재사용 가능한 판단 런타임](2026-09-21-feat-jev-native-01-runtime.md): wire 타입, 전송, 배치, 마감 시각, 취소, 실패 시 계측.
- [x] [02 — 인덱스 기반 overview 추천](2026-09-21-feat-jev-native-02-overview.md): 동일 snapshot의 전체 후보, 근거 투영, Score 자격·순위, Choice 역할.
- [x] [03 — 구조화된 search 본문 필터](2026-09-21-feat-jev-native-03-search-filter.md): 선택된 근거의 정체성·완전성, Noul, 보호 규칙, 렌더링·소스 관측.
- [x] [04 — MCP·설정 통합](2026-09-21-feat-jev-native-04-integration.md): 활성화, 설정·스키마, 비동기 요청 경계, 최종 응답·진단.
- [x] [05 — 전체 기능 검증](2026-09-21-test-jev-native-05-verification.md): 통합 검증 행렬, 실행 증거, 패키징, 향후 품질 평가 방법.

## 실행 순서와 의존성
인계 경로는 모두 `apps/codemap-search/validation/jev-native/` 아래의 실행 산출물이다. 문서 개선 작업에서 미리 성공 보고서를 만들지 않는다.

| 단계 | 시작 조건 | 인계 파일 | 다음 단계가 확인할 증거 |
| --- | --- | --- | --- |
| Wave 1: 01 | 시작 점검 완료 | `01-runtime.md` | 확정 API·제한·실패 계측, `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev`, 실행한 mock 예제 |
| Wave 2: 02 | 01의 현재 계약·검증 통과 | `02-overview.md` | 전체 후보/조각 수, 질문·루브릭, 상태·역할·재사용 규칙, `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview` |
| Wave 2: 03 | 01의 현재 계약·검증 통과 | `03-search-filter.md` | prepare/filter/render API, 근거 정체성, 보호 규칙, 한도·fallback 보존, `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::search` |
| Wave 3: 04 | 02·03의 인계와 공통 계약 일치 | `04-integration.md` | 설정·호출 행렬, 최종 소비자 전달, `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::config` 및 `e2e::mcp` 필터 |
| Wave 4: 05 | 04의 현재 통합 검증 통과 | `05-verification.md` | 전역 완료 조건에 대응하는 실제 결과·로그·미실행 한계 |

01 → 02/03 인계는 컴파일만으로 통과하지 않는다. 다음 명령이 고정 fixture의 Score=2.0, Choice=keep, Noul=0.9와 사용량을 확인하고 exit 0이어야 한다.

```sh
cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock
```

각 인계에는 실제 소스 상태(HEAD와 미커밋 변경 식별), 공개/내부 API 구분, 입력·출력·실패 계약, 질문/근거/정책 버전, 변경 파일, 명령·cwd·exit code·테스트 수·fixture 수·로그 경로, 미확인 사항을 기록한다. 기존 증거를 재사용하면 그 증거의 입력/소스 상태가 왜 여전히 유효한지 설명한다.

## 병렬화와 파일 소유권
- 02와 03만 01 완료 후 병렬 진행할 수 있다. 나머지는 위 의존성 순서로 직렬 진행한다.
- 02는 `src/tools/overview.rs`, `src/tools/overview/`를, 03은 `src/tools/search/`를 소유한다. 공통 parser/redaction/config/runtime 변경이 필요하면 먼저 상위 담당자가 소유자와 인계 변경을 지정한다.
- 병렬 구현을 실제로 위임한 경우에만 독립 작업 트리를 사용하고, 하나의 작업 트리에는 한 명의 작성자만 둔다.
- 결함이 나오면 관련 후속 단계와 완료 판정을 멈추고 소유자에게 수정·재검증을 돌린다. 검증 담당자가 생산 코드에 별도 해결책을 넣지 않는다.

| 충돌 지점 | 최초 소유자 → 후속 소유자 | 규칙 |
| --- | --- | --- |
| `Cargo.toml`, `Cargo.lock`, `src/lib.rs` | 01 → 04 | HTTP/TLS와 공통 모듈 먼저, 이후 통합에 꼭 필요한 변경만 |
| `src/tools/overview.rs` | 02 → 04 | snapshot/prepare 계약 확정 후 dispatch 연결 |
| `src/tools/search/` | 03 → 04 | 구조화·렌더링 계약 확정 후 인자/dispatch 연결 |
| `src/mcp/`, `src/tools/mod.rs`, 설정·템플릿·사용자 문서 | 04 | 기본값·검증·스키마·문서·최종 소비자를 함께 변경 |
| `tests/e2e/helpers.rs` | 04 → 05 | 통합에서 주입 경계를 고정하고 검증에서 필요한 부분만 확장 |
| 공통 parser/redaction/codemap 보조 함수 | 상위 담당자가 지정 | 직렬 변경; 영향받는 02/03 검증을 모두 재실행 |

경로는 이 표에서 `apps/codemap-search/` 기준이다. 런타임·알고리즘 단위 회귀는 01–03, 설정·요청 경계 회귀는 04, 결합된 실행 행렬은 05가 소유한다.

## 공유 구현 계약
각 하위 브리프는 아래 계약을 상속한다. 세부 Rust 이름은 기존 API와 관례를 존중해 소유자가 인계 시 확정한다. 내부 진단 필드를 새 MCP 응답 스키마로 노출하라는 요구는 아니다.

### SC-01 — 범위와 활성화
- 기존 Rust 크레이트 안에 독립 호출 가능한 공통 모듈을 둔다. 별도 크레이트 발행, Python 런타임, stdio proxy, socket broker, 새 MCP 서버·도구군은 만들지 않는다.
- `overview_enabled`와 `search_filter_enabled`는 독립적이며 모두 기본값 false이다. 활성화만으로 background API 요청을 만들지 않는다.
- caller가 명시한 `task_query`만 원래 의도로 사용한다. `search.query`, overview의 `query` 경로 별칭, 이전 요청·transcript로 대체하지 않는다. 누락·공백 의도는 호출 없이 우회한다.
- 기존 도구 이름, 인자 의미, scope/별칭, JSON-RPC content/error 구조, readiness 안내, CLI 출력과 read/find/grep 동작을 유지한다.

### SC-02 — 판단과 입력의 책임
- Score는 근거의 관련성 등급, Choice는 선언의 대표 역할 하나, Noul은 본문의 무관 여부를 판단한다. 완전성·최신성·관계 해석의 확정 사실·계산·생략은 Rust 책임이다.
- 공유 state에는 명명된 작업 의도 등 공통 사실을, 각 질문에는 자신의 후보 근거를 넣는다. 질문 ID는 라우팅용이며 모델이 의미를 읽는 필드가 아니다. instructions에 완전한 판단과 backtick 근거 참조를 쓴다.
- 독립 질문은 같은 state에서 묶는다. 질문은 다른 질문의 instructions/답변을 볼 수 없다. 후속 단계는 앞 단계가 결정한 후보를 명시적 새 입력으로 만든다.
- 소스·문서·파일명·질문에 인용된 지시문은 비신뢰 데이터이다. 모델 결과를 권한이나 보안 증거로 사용하지 않는다. 별도 탐지 모델이나 질문 수 증가는 이 계약의 필수 사항이 아니다.
- 전송할 모든 데이터는 전송 전에 기존 masking 정책을 적용한 복사본이어야 한다. 원본 source/index는 변경하지 않는다. 최종 응답 masking만으로 외부 전송을 보호했다고 간주하지 않는다.

### SC-03 — 하나의 마감 시각과 제한
- 기본 Jev 전체 예산은 45,000ms이다. 활성 어댑터의 준비를 시작할 때 monotonic 마감 시각을 한 번 만들고 준비·배치·큐 대기·간격 대기·HTTP·검증·후속 역할 단계에서 공유한다. 기존 도구의 기본 결과 생성 시간과 전체 MCP 시간은 별도로 계측한다.
- host 제한, 재사용 평가기의 제한, caller 마감 시각 중 더 이른 것을 적용한다. 두 번째 평가 단계나 새 batch가 예산을 초기화해서는 안 된다. 동기 준비가 시간을 다 썼으면 다음 HTTP 요청을 시작하지 않는다.
- 80,000 encoded request bytes, 최대 3개 HTTP 동시 요청, 최소 300ms dispatch 간격, 자동 재시도 없음, 기본 idle timeout 30,000ms를 출발 정책으로 유지한다. 실제 허용 범위와 전송 라이브러리 지원은 01이 고정하고 04가 그대로 노출한다.
- byte 상한은 tokenizer가 아니다. `state + all questions` 64k와 `state + longest question` 32k token 제한을 별도로 다루고 추정의 한계를 기록한다. 입력을 조용히 자르거나 일부 후보만 평가한 뒤 전체 순위라고 주장하지 않는다.
- 재사용 모듈은 취소를 지원하지만 MCP는 기존 sequential request/notification 의미를 유지한다. 프로토콜이 제공하지 않는 취소 동작을 제공한다고 쓰지 않는다.

### SC-04 — 원본 결과와 출력 한도 우선순위
- `base`는 같은 요청 설정·snapshot·소스 캡처에서 기존 도구가 선택하고 한도 처리한 결과이다. Jev 상태 문구 공간을 먼저 빼서 더 작은 결과를 base라고 부르지 않는다.
- 비활성·keyless·intentless·bypass·전체 실패는 base를 반환한다. search의 all-keep도 원래 text와 source observations를 보존한다. overview 추천과 search 생략 표시는 기존 근거를 밀어내지 않는 범위에서만 추가한다.
- 한도 때문에 상태 문구를 넣을 수 없어도 기존 근거를 줄이지 않는다. 04의 비민감 stderr 진단이 canonical 관측 경로이다. stdout에는 기존 JSON-RPC frame만 쓴다.
- source observations의 파일 포함 여부는 실제 전달된 소스를 기준으로 한다. 기존 필드의 계측 단위는 보존하고, 필터·cap·mask와의 정확한 계산 위치를 03/04가 함께 기록한다. 전체 응답 bytes는 최종 마스킹/한도 처리 후 계측한다.

### SC-05 — 상태와 실패 계측
- `applied`, `bypassed`, `fallback`을 분리한다. 추천의 `matched`, `no_match`, `insufficient_evidence`와 역할 단계의 완료/생략/실패도 구분한다.
- #1 파일 점수 단계가 불완전하면 전체 추천 fallback이다. 파일 순위가 완성된 뒤 역할 단계만 실패하면 파일 추천은 유지하고 역할은 unavailable로 표시한다. 부분 역할 답변을 완성된 역할 목록처럼 쓰지 않는다.
- #2 평가 실패는 전체 base 복구이다. 성공한 몇 개 답변만으로 부분 필터링하지 않는다. 정상 평가 결과 전부 유지인 경우는 실패가 아니다.
- 실패 전 완료된 batch와 이미 dispatch한 요청의 정체성·알려진 사용량·시간을 잃지 않는다. 응답을 못 받은 요청의 token 사용량은 미확인이며 0이 아니다. 취소는 이미 전송한 요청의 과금을 취소한다는 뜻이 아니다.
- 진단에는 모델·정책·결과·이유 코드·개수·알려진 사용량/미확인 여부·elapsed time만 둔다. key, 원문 task/source, provider 오류 본문은 기록하지 않는다.

### SC-06 — 재사용과 증거
- 원시 판단과 최종 정책 결정을 분리한다. 입력·질문·후보·모델·근거 버전이 같을 때만 threshold/정렬/표시 정책을 재적용한다. 각 결과를 정확한 전송 입력 fingerprint와 후보 identity에 연결한다.
- #1 전체 Score로 순위를 다시 정할 수 있지만 미평가 파일의 Choice 역할을 만들어낼 수는 없다. #2 보호 이유나 출력 조건이 달라지면 같은 근거에서 Rust 정책을 재계산한다. 새 source/question에는 새 평가가 필요하다.
- 영속 inference cache나 운영 원문 로그는 추가하지 않는다. 재현용 완전한 입력·답변은 비민감 fixture/검증 산출물에만 보존한다.
- offline 입력 조립 검증, fake-answer 정책 검증, 실제 모델 품질 검증을 별도로 기록한다. calibration과 live benchmark는 새 명시적 실행 요청 전까지 미실행이다.

## 공통 작업 경계
- 필요한 회귀 테스트와 최소 비민감 fixture는 기존 승인 범위이다. 이 문서 개선 자체는 Markdown 6개만 변경하며 구현·테스트·증거 생성이나 실제 모델 실행을 수행하지 않는다.
- 기존 테스트 시설을 재사용한다. lint/formatter 설정이나 무관한 검증 작업을 추가하지 않는다. 실행 전 명령과 cwd를 알린다.
- PoC 일회용 key를 읽어 재사용하거나 source·fixture·로그·문서에 넣지 않는다. 자격 증명은 host가 환경 변수에서 해결한다. 유료 API 호출은 별도 명시적 실행 지시가 필요하다.
- 기존 추적/미추적 PoC와 사용자 변경을 보존한다. 이 브리프셋 실행에는 stage, commit, merge, publish, release가 포함되지 않는다.
- parser/index 직렬화 형식, 파일 권한 경계, 크레이트 패키징·릴리스 계약을 유지한다. 외부 HTTP/TLS 의존성 선택은 01 범위이며 별도 Jev 크레이트 발행과 구분한다.
- 새 CLI 제품 표면, 모델 공급자 확장, 일반 middleware registry, 다중 역할 분류, 자동 read/grep/find 필터, 기본 활성화 전환은 범위 밖이다.

## 전역 완료 조건
- [x] 시작 점검과 01–05 인계가 현재 실행 소스 상태를 식별하며 필수 실패가 남아 있지 않다.
- [x] 동일 Rust 크레이트의 공통 평가기를 두 어댑터와 독립 예제가 사용하며 Python/proxy/exported index가 필요 없다.
- [x] mock 예제를 실제 실행해 Score·Choice·Noul·사용량 assertions가 통과한다.
- [x] 네 활성화 조합, keyless/intentless 호출, 기존 CLI·도구가 호환되고 외부 provider 호출 없이 검증된다.
- [x] #1 전체 적격 파일/투영 조각의 coverage가 증명된다. 자격 판정 후 최대 24개, 추천 없음/근거 부족, 최대 두 대표 선언, 역할만 실패하는 경우가 구분된다.
- [x] #2 실제 선택된 근거만 판단하며 최종 threshold가 적용된다. 정체성 불명·부분·보호 근거는 유지되고 모든 출력 소스는 원래 파일/행에 대응한다.
- [x] 두 단계가 하나의 마감 시각을 공유하며 실패·취소 시 알려진 사용량과 미확인 사용량이 구분된다.
- [x] 한도에 찬 all-keep/fallback에서도 base 근거와 관측 정보가 보존되고 진단이 근거를 밀어내지 않는다.
- [x] 질문·근거·정책 버전과 원시 답변 재사용 범위가 고정되고 실제 outbound payload의 masking/참조가 검증된다.
- [x] 최종 소스에서 `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets`가 exit 0이다.
- [x] 최종 소스에서 `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests`와 01–03 소유 library 회귀 그룹이 실제 필수 사례를 실행해 통과한다.
- [x] README/설정 문서·양언어 템플릿·MCP schema·최종 소비자가 실제 기본값·단위·전송 데이터·진단 위치·미검증 품질을 동일하게 설명한다.
- [x] `05-verification.md`에 package list, 실제 실행 결과, 입력/정책/품질 검증 구분, 지표 분모, 보정/평가 분리, 미실행 플랫폼·pooling·live 검증이 기록된다.

## 참고 자료와 해석 원칙
- [TypeSafe 스킬](https://raw.githubusercontent.com/typesafe-ai/skills/refs/heads/main/skills/typesafe-ai/SKILL.md), [문서 색인](https://docs.typesafe.ai/llms.txt), [설계 지침](https://docs.typesafe.ai/concepts/how-to-build-with-system-one.md).
- [API](https://docs.typesafe.ai/api.md), [모델·한도·언어 지원](https://docs.typesafe.ai/models.md), [state](https://docs.typesafe.ai/concepts/state.md), [confidence](https://docs.typesafe.ai/confidence.md), [Jev 1.13 한계](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md).
- [추천 예제](https://docs.typesafe.ai/cookbooks/skill_suggestion.md), [본문 필터 예제](https://docs.typesafe.ai/cookbooks/classifying_rag_passages.md)의 수치·질문 수·캐시는 제품 요구사항이 아니다. 의미에 맞는 기본형과 코드 소유 정책을 채택하고 현재 범위를 유지한다.
- 역사적 보고서 `~/.codex/checkpoints/codemap-search-comparison/JEV-COMPARISON.md`에는 제외 시도와 815/822 인덱스 차이가 있다. Python Choice 측정은 Rust/Noul 품질·속도 증거가 아니다.
- 문서와 모델 버전은 실행 시 다시 확인한다. 공식 계약 변화가 pinned 모델·타입·범위를 깨면 해당 결정만 상위 담당자에게 돌린다.

## 남은 결정의 처리
내부 재사용, 독립 기본 비활성화, 필요한 회귀 검증이라는 제품 방향은 확정되어 있다. 타입 이름·전송 라이브러리·bounded 입력 분할 등 기술 선택은 각 Stage 1에서 실제 코드와 API에 근거해 고정한다. 공개 동작·전송 범위·허용 위험·의존성 구조를 바꾸는 선택만 별도로 확인한다. 그 선택에 의존하지 않는 승인된 작업은 계속한다.
