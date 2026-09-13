# Rust·Go 공개 저장소 검증과 호출 판정 수정

이 검증은 큰 실제 저장소에서 `overview`, `search`, `read`, `grep`, `find`의 동작과 호출·참조 문맥을 확인하고, 같은 커밋과 설정으로 재실행하기 위한 기준이다. Rust·Go 각 두 저장소를 대상으로 한다. 모든 언어와 모든 호출 관계의 정확도를 보장하는 검사는 아니다.

후속 25개 개발 언어의 대상, 수정 결과와 남은 문제는 [전체 언어 확대 검증](development-language-validation.ko.md)에 기록했다. 이 문서의 최초 기준선과 당시 바이너리별 결과는 그대로 보존한다.

최초 검증에서 확인한 Rust·Go의 호출 오연결과 상수 누락을 수정했다. 전체 재검증 470항목과 작은 회귀 12항목이 통과했고, 이후 범위·최신 원문 검사와 사용자가 선택한 ‘미해결’ 표시 정책을 반영한 최종 바이너리에서 관련 82항목과 회귀 12항목을 다시 통과했다. 서로 다른 바이너리의 검사 범위를 합쳐 전체 언어의 정확도로 해석하지 않는다.

## 수정 결과와 최종 재검증

Rust·Go의 호출자와 호출 대상이 같은 원문 해석기를 사용한다. Rust의 crate·모듈·import 별칭·glob과 명시적 로컬 Cargo 의존성, Go의 package·go.mod·import 별칭을 확인하며, 이름이 같은 유일 후보라는 이유만으로 다른 정의에 연결하지 않는다. 지역 변수·매개변수·클로저·제네릭의 가림과 현재 파일의 변경도 확인한다. 외부 테스트 package의 동명 선언은 일반 package 정의로 취급하지 않는다.

호출 대상의 모듈이나 수신 타입을 확정할 수 없으면 `name (unresolved)`만 표시한다. 추정한 반환 타입, 외부·미색인 정의에도 정의 링크나 `(precise)`를 붙이지 않는다. 실제 호출 이름은 출력 한도 안에서 유지하며 문자열·주석 속 호출처럼 보이는 텍스트는 제외한다. 명시적 타입으로 확인한 호출은 `name — file:line`을 유지한다.

Go의 값을 생략한 그룹 상수 선언을 찾지 못하던 문제를 고쳤다. Rust의 명시적 import와 Go의 같은 package에서 가져온 상수도 정의 위치를 표시한다. 이 변경이 `module::CONST`·`module.CONST` 같은 한정된 참조 표현 전부를 지원하는 것은 아니다. import 추출 형식은 `v19-rust-go-source-imports`로 바뀌어 이전 색인을 재추출한다.

최종 증거는 [rust-go-fix-results.json](../validation/rust-go-fix-results.json)에 기록했다. 최종 바이너리 SHA-256은 `c4f54486a473c8dd96ab4f197d9f2a32a2cf57fb74f7556e1382e84bc6da222f`다. 기준 커밋과 수정한 소스의 해시를 함께 보존한다.

| 검증 | 실제 실행 범위 | 결과 |
| --- | --- | --- |
| 전체 공개 저장소 검사 | `rust-go-final-source-20260913`, 마지막 범위 검사·미해결 정책 반영 전 바이너리 | 470 통과, 회귀 12 통과 |
| 최종 정책 재검증 | `rust-go-unresolved-cap-20260913`, 네 저장소·두 설정의 실제 호출/참조 사례와 테스트 정책 | 82 통과, 회귀 12 통과 |
| 로컬 검사 | 최종 소스의 라이브러리, 파일 도구 e2e, `cargo check`, 전체 target Clippy | 175 통과, 24 통과, check·Clippy 통과 |

Bevy의 `AssetPlugin::build`에서는 추정한 수신 타입의 `init_default_source`·`register_source` 연결을 미해결로 바꾸는 정책을 명시적으로 검사했다. 기본 출력 한도 5개로는 `register_source`가 생략되므로 이 사례에서만 `callee_list_cap=30`으로 검사한 후 원래 설정으로 복원한다. 명시적 `App` 타입의 `world_mut` 정의 위치는 유지해야 한다. 이전 기대값과 변경 사유는 결과의 `case_changes`에 남아 있다.

원시 요청·응답, 보존 바이너리와 실행 결과는 `/Users/buyong/tmp/codemap-public-validation/runs/rust-go-unresolved-cap-20260913/`에 있다. [recheck_public_cases.py](../scripts/recheck_public_cases.py)는 앞선 전체 검증의 Git SHA·추적 파일·설정 해시가 유지된 작업 트리에서 관련 사례만 다시 실행한다. 새 전체 검증과 구분해서 사용한다.

## 대상과 측정 기준

| 언어 | 저장소 | 고정 커밋 | 대상 언어 코드 줄 | 커밋 수 |
| --- | --- | --- | ---: | ---: |
| Rust | [Bevy](https://github.com/bevyengine/bevy) | `96e3bcfd87f4cb6372dd9da8b5318f3e64899a01` | 455,742 | 12,180 |
| Rust | [Nushell](https://github.com/nushell/nushell) | `9a251561a90277d0af466ee845dff706bbf3c3d9` | 358,894 | 11,853 |
| Go | [Prometheus](https://github.com/prometheus/prometheus) | `46ef3704189798f58af5dd4ac7d2f87ac8880f30` | 289,673 | 18,628 |
| Go | [Kubernetes](https://github.com/kubernetes/kubernetes) | `def9a441f53ae837ee5abfb1a7b9b5e4c314948e` | 2,201,458 | 141,099 |

각 저장소에서 Git이 추적하는 `.rs` 또는 `.go` 파일을 `tokei 14.0.0`으로 측정했다. 공백·주석, 의존성/빌드 디렉터리, 심볼릭 링크, 명시적 생성 파일명·헤더를 제외했다. 사람이 작성한 테스트는 포함한다. Rust 문서 주석 안의 Markdown도 Rust 코드 합계에 넣지 않는다. 각 저장소의 원시 파일별 측정값과 제외 사유는 실행 결과의 `*-qualification.json`에 남는다.

제외 디렉터리는 `vendor`, `vendored`, `node_modules`, `target`, `third_party`, `third-party`, `dist`, `build`다. 생성 파일 판정은 `.pb.go`, `.gen.go`, `_generated.go` 또는 첫 4,096바이트의 생성 표식과 편집 금지 표식으로 제한한다. 표식 없는 생성 파일을 완전히 식별했다는 뜻은 아니다. 코드 줄 수는 프로젝트 전체 크기를 평가하기 위한 조건이며, 사람이 전수 검토한 줄 수가 아니다.

커밋 수는 shallow clone이 아닌 저장소에서 `git rev-list --count HEAD`로 측정했다. 최초 후보였던 etcd는 같은 기준에서 Go 153,922줄·25,221커밋으로 줄 수 조건에 미달해 제외했다. Bevy는 엔진의 다중 crate, Nushell은 셸과 플러그인의 다중 crate, Prometheus는 서버·도구·라이브러리, Kubernetes는 본체와 `staging` 모듈을 함께 포함한다.

## 고정 조건과 대조 방법

- 저장소 주소·SHA·실제 사례: [repositories.json](../validation/repositories.json)
- 설정 전체: [config.toml](../validation/config.toml)
- 실행기: [public_validation.py](../scripts/public_validation.py)
- Go 대조 파서: [go_oracle.go](../validation/go_oracle.go)
- 작은 회귀 사례: [Rust](../tests/fixtures/public_validation/rust/lib.rs), [Go](../tests/fixtures/public_validation/go/probe.go)

각 조합은 별도의 detached worktree와 빈 `.codemap/index`에서 시작한다. 원본 프로젝트의 추적 파일은 수정하지 않는다. 대조용 파일과 설정은 검증 작업 트리에만 추가한다. `config_auto_update=false`로 설정 자동 변경을 끈다.

두 설정의 차이는 다음과 같다. `default`라는 이름은 아래 두 탐색 옵션의 기본값을 의미한다. 디렉터리 제외 목록을 포함한 나머지 설정은 공통 검증 설정으로 고정한다.

| 설정 | `navigation_context_default` | `navigation_store_references` |
| --- | --- | --- |
| `default` | `false` | `false` |
| `structural` | `true` | `true` |

기본 실험에서는 `[exclude].should_include_test_code=false`를 사용한다. 이후 테스트 포함, 내장 목록을 사용자 목록으로 교체, 원래 설정 복원까지 검사한다. `.gitignore`, `.codemapignore`, Git 제외 규칙은 별도로 적용된다. 줄 수 선정에서 제거한 생성 파일을 색인에서도 모두 제거하는 설정은 아니다. 색인에는 설정상 허용되는 생성 파일이 포함될 수 있다.

대조 정답을 codemap-search의 출력에서 복사하지 않는다.

| 검사 | 독립 기준 | 판정 범위 |
| --- | --- | --- |
| 선언과 줄 범위 | Rust는 `rust-analyzer parse --json`, Go는 표준 `go/parser` | 지정한 실제 파일의 함수 본문·메서드·Go interface 메서드·Rust 매크로 선언 |
| `read` | 디스크 원문의 지정 줄 구간 | 줄 번호와 원문이 정확히 같은지 |
| `grep` | Python 정규식으로 구한 실제 줄 목록 | content/files/count, 문자열·주석 포함 원문 검색, 페이지 경계 |
| `find` | Git 추적 파일과 `git check-ignore --no-index` | 지정 하위 디렉터리의 파일 집합 |
| `search` | 원문 검토로 정한 구현 파일 | 정해진 개념 질의에서 구현 파일이 출력되는지 |
| 호출·상수 참조 | 실제 선언·import·수신 타입 검토 및 작은 재현 코드 | 특정 올바른 연결의 존재, 특정 잘못된 연결의 부재 |
| 제외와 갱신 | 검증 파일의 생성·수정·삭제 및 설정 변경 | 직접 파일 지정, `include_ignored`, 필수 제외, 색인 반영 |

`overview`의 기존 계약에 따라 비공개 함수 내부의 지역 선언과 본문 없는 Rust trait 선언은 대조 집합에서 제외한다. Rust 매크로는 실제 종류를 대조 자료에 보존하되, 현재 출력의 `fn` 정규화를 허용한다. Go의 receiver 메서드와 이름 있는 interface 메서드는 선언 범위 대조에 포함한다.

명시적으로 선택한 파일 13개에 저장소당 20개의 고정 표본을 추가했다. 표본은 줄 수 측정에 포함된 파일에서 기존 사례, Git 제외 파일, 코드가 없는 파일, 색인 크기 한도인 1 MiB를 초과하는 파일을 뺀 뒤 `SHA-256("codemap-public-v1:<저장소 이름>:<경로>")` 순으로 선정한다. 총 93개의 서로 다른 파일을 두 설정에서 대조했다. 추가 표본은 선언 검사이며, 모든 표본의 호출 관계까지 정답화한 것은 아니다.

각 MCP 요청·응답과 처리시간을 `mcp.jsonl`, 서버 진단을 `server.stderr.log`에 기록한다. 구문 파서 자료에는 원본 파일의 SHA-256을 붙인다. `summary.json`은 저장소 SHA, 바이너리·설정·실행기·사례 목록의 SHA-256, 도구 버전, 검사 결과를 포함한다.

## 결과 해석

결과는 통과(`pass`), 실패(`fail`), 판정 보류(`unverified`)로 구분한다. 필요한 호출 대상이 출력되지 않았고 응답이 모호성이나 출력 한도를 명시했다면 보류로 기록한다. 표시되지 않은 관계를 올바른 관계로 계산하지 않는다. 프로토콜 오류와 색인 준비 시간 초과는 실행 오류로 별도 기록한다.

작은 대조 항목을 많이 넣었으므로 전체 통과율을 언어별 정확도로 해석하면 안 된다. 실제 저장소의 표본과 인위적으로 추가한 대조 파일은 목적이 다르다. 표준 파서는 구문을 검증하며, 전체 프로젝트의 타입 추론·매크로 확장·동적 호출에 대한 정답기는 아니다.

## 최초 기준선의 측정 결과

측정일은 2026-09-12 UTC다. 측정 대상은 `codemap-search 0.8.1`, 소스 커밋 `f928f0e6353dcaadca638c1b53eb0a0c8c92bad9`의 설치 바이너리다. SHA-256은 `f2f79e45e9eb4c58cc9adf9544bf93b19cfa87f87551ffaf616a4980ef79299d`다. 실행 환경은 macOS arm64, Python 3.14.5, Go 1.26.3, rust-analyzer 1.98.1, tokei 14.0.0이다.

| 저장소 | 설정 | 통과 | 실패 | 보류 |
| --- | --- | ---: | ---: | ---: |
| Bevy | default | 58 | 1 | 0 |
| Bevy | structural | 58 | 1 | 0 |
| Nushell | default | 55 | 3 | 0 |
| Nushell | structural | 55 | 3 | 0 |
| Prometheus | default | 54 | 4 | 1 |
| Prometheus | structural | 55 | 4 | 0 |
| Kubernetes | default | 55 | 4 | 0 |
| Kubernetes | structural | 55 | 4 | 0 |
| **합계** | **470항목** | **445** | **24** | **1** |

실패 24항목은 서로 다른 버그 24개를 의미하지 않는다. 호출 대상 오연결 12항목, 호출자 오연결 2항목, 같은 파일의 상수 누락 2항목, 파일 간 상수 참조 미지원 8항목이다. Prometheus 기본 설정의 `LoadFile → Load`는 모호성·생략 표시 때문에 보류였고 구조 분석 설정에서는 정의 위치가 출력됐다.

확인된 정상 범위는 다음과 같다.

- 실제 파일 93개의 선언 1,182개가 두 설정 모두에서 줄 범위까지 일치했다. 파일별 대조 186항목이 통과했다.
- 원문 `read`, `grep`의 출력 방식·페이지 경계 58항목과 파일 집합 대조 8항목이 통과했다.
- 실제 구현을 찾는 검색 질의 8항목이 통과했다. 지정한 파일의 출력 여부를 확인했으며 모든 질의의 검색 정확도를 측정한 것은 아니다.
- 제외 규칙 72항목, 생성·수정·삭제 갱신 56항목, 테스트 정책 교체·복원 24항목이 통과했다.
- 문자열·주석·테스트를 호출자에서 제외하는 작은 대조 사례는 두 언어·네 저장소·두 설정에서 통과했다.

별도의 작은 회귀 사례 6개를 두 설정에서 실행한 **12항목은 모두 실패**했다. 의도한 실패를 통과로 뒤집지 않는다. 이 사례들은 오연결과 상수 누락을 대형 저장소 없이 다시 드러내기 위한 것이다. 회귀 소스는 `rustc --emit=metadata`와 `go test <단일 파일>`로 컴파일을 확인했다. Go 명령에는 실행할 테스트 함수가 없었으며, 사용자 지정 attribute를 넣은 대조 파일 전체를 컴파일했다는 의미는 아니다.

| 저장소 | 빈 색인 준비 시간 default / structural | 개별 `read` 중앙값 default / structural | 설정별 요청 수 |
| --- | ---: | ---: | ---: |
| Bevy | 9.158 / 9.136초 | 4,028.874 / 4,037.957ms | 10 |
| Nushell | 5.116 / 5.114초 | 2,238.229 / 2,267.997ms | 10 |
| Prometheus | 18.224 / 18.218초 | 71.331 / 100.204ms | 11 |
| Kubernetes | 213.246 / 211.499초 | 781.538 / 848.363ms | 11 |

`read` 시간에는 자동 호출·참조 문맥 생성 비용이 포함된다. 개별 요청 시간만 집계했고 설정 반영 대기 전체를 한 요청으로 계산하지 않았다. OS 캐시·CPU 부하를 격리하지 않았고 일부 보정 실행이 다른 색인 작업과 겹쳤으므로, 이 수치는 재현한 작업량의 관찰값이다.

커밋에 포함하는 요약은 [baseline.json](../validation/baseline.json)이다. 원문은 `/Users/buyong/tmp/codemap-public-validation/runs/rust-go-phase1-verified/`의 `mcp.jsonl`, `followup/`, `go-final/`과 `validated-summary.json`, 작은 회귀 결과는 `runs/final-regressions/`에 있다. 최초 요청의 잘못된 Go 검색 범위와 Go 대조기의 interface 메서드 누락은 보정 후 재실행했다. 원래 응답은 보존하고 최종 판정에는 유효한 질의와 보완한 대조 결과를 사용했다. 원본·보정 실행기의 해시와 보정 내역도 결과에 남겼다.

## 확인한 문제와 수정 지점

아래 내용은 최초 기준선에서 관측한 결함과 당시 원인이다. 위의 수정 결과 및 최종 재검증과 구분해 보존한다.

### 이름이 같은 다른 대상에 호출을 연결한다

- Nushell `crates/nu-command/src/filesystem/open.rs:329`의 문자열 분리·iterator 수집이 `ByteStream::split`, `NuLazyFrame::collect`로 연결된다. 같은 파일 323줄의 `matches!`도 `Pattern::matches`로 연결된다.
- Prometheus `config/config.go:96`의 `os.Expand`가 `template/template.go`의 `Expander.Expand`에 연결된다. `LoadFile` 안의 내장 `len`도 `tsdb/head.go`의 `memChunk.len`에 연결된다.
- Kubernetes `pkg/util/tolerations/tolerations.go:61`의 동등성 검사와 `staging/src/k8s.io/apimachinery/pkg/util/intstr/intstr.go:81`의 `strconv.ParseInt`가 관계없는 code-generator 코드로 연결된다.

공통 원인은 [callees.rs](../src/callers/callees.rs)의 후보 선택이다. 구조 분석에서도 같은 파일에 후보가 하나 있거나 전체 후보가 하나이면 `is_precise=true`를 반환한다. 유일한 이름 후보라는 사실만으로 해당 import나 수신 타입의 대상임이 증명되지는 않는다. [symbols.rs](../src/callers/symbols.rs)의 호환성 필터는 언어군과 일부 Rust/C 멤버 접근 구분까지 수행하지만 Go의 패키지·내장 함수 및 수신 타입 전체를 해결하지 않는다.

작은 Rust·Go 회귀 사례에서도 같은 문제가 재현된다. 구조 분석 설정의 Go 응답에는 실제로 `Unrelated.len — probe.go:10 (precise)`가 표시됐다. 수정할 때는 실제 패키지·모듈·수신 타입·범위로 확인한 연결만 `precise`로 인정하고, 해소하지 못한 호출을 무관한 유일 후보로 확정하지 않아야 한다.

### 호출자도 다른 종류의 동명 호출을 포함한다

Prometheus의 설정 `Load` 호출자에 atomic 값의 `Load`를 호출하는 코드가 포함된다. 작은 Go 사례의 `Load`와 `(*atomic.Int64).Load`도 이를 분리해서 검사한다. 호출자 판정과 호출 대상 판정을 함께 수정해야 같은 문제가 방향만 바뀌어 남지 않는다.

### 상수 문맥의 지원 범위가 제한돼 있다

Kubernetes의 `FromString`은 같은 파일의 `String` 상수를 사용하지만 정의 문맥에 표시되지 않는다. Go에서 값을 생략한 그룹 상수 선언만 남긴 작은 사례도 실패했다. 이 사례에는 동명 메서드가 없으므로 메서드 이름 충돌이 없어도 재현된다. [structure.rs](../src/tools/live_symbols/structure.rs)의 `symbol_node`가 같은 범위의 식별자 노드를 반환할 수 있고, [references.rs](../src/tools/live_symbols/references.rs)의 상수 수집은 `name` 자식이 있는 선언 노드를 요구한다. 이 연결부가 수정 지점이다.

다른 파일에서 가져오는 상수는 Rust·Go 대조 사례 모두에서 정의 위치가 출력되지 않는다. 이는 [references.rs](../src/tools/live_symbols/references.rs)가 같은 파일의 상수만 수집하는 현재 지원 범위와 일치한다. `navigation_store_references=true`로 바꿔도 `read`의 상수 문맥이 파일 간 참조로 확장되지는 않는다.

이 항목은 사용자에게 필요한 문맥의 누락으로 추적하되, 같은 파일만 지원한다고 밝힌 출력 계약과는 구분한다. 지원 확대 시 재수출, 별칭, 같은 이름의 지역 변수, package/crate 경계를 함께 검증해야 한다.

## 재실행과 직접 확인

Python 3.10 이상, Git, `tokei`의 JSON 출력, Go, `rust-analyzer`, 실행할 `codemap-search` 바이너리가 필요하다. 최초 준비는 공개 저장소 이력을 내려받으므로 네트워크와 디스크 공간을 사용한다. 각 실행은 새 작업 트리와 색인을 추가하고 기존 결과를 덮어쓰지 않는다.

다음 명령의 작업 디렉터리는 이 모노레포 루트다. `--binary`를 생략하면 PATH의 `codemap-search`를 사용한다.

대형 저장소 복제 없이 작은 회귀 사례만 확인하려면 다음 명령을 사용한다. 이 경로에는 Python과 `codemap-search`만 필요하다.

```bash
python3 apps/codemap-search/scripts/public_validation.py \
  --cache /Users/buyong/tmp/codemap-public-validation \
  regressions --profile structural
```

전체 검증 또는 일부 저장소 재검증은 다음과 같다.

```bash
python3 apps/codemap-search/scripts/public_validation.py \
  --cache /Users/buyong/tmp/codemap-public-validation prepare

python3 apps/codemap-search/scripts/public_validation.py \
  --cache /Users/buyong/tmp/codemap-public-validation \
  --binary /Users/buyong/.local/bin/codemap-search \
  run --repo nushell --profile structural

python3 apps/codemap-search/scripts/public_validation.py \
  --cache /Users/buyong/tmp/codemap-public-validation \
  run
```

`run`을 생략 없이 실행하면 네 저장소와 두 설정을 모두 검사한다. `--repo`와 `--profile`은 반복 지정할 수 있다. 실행 결과가 알려 주는 `worktree`로 이동하면 기존 `cm`으로 직접 확인할 수 있다. 같은 작업 트리의 자동 검사가 종료된 후 실행해야 색인 writer가 충돌하지 않는다.

```bash
# Nushell 작업 트리에서
cm read '{"file_path":"crates/nu-command/src/filesystem/open.rs","offset":328,"limit":19}'
cm read '{"file_path":"crates/nu-command/src/filesystem/open.rs","offset":321,"limit":6}'
cm grep '{"path":"crates/nu-command/src/filesystem/open.rs","pattern":"extract_extensions","head_limit":10}'

# Prometheus 작업 트리에서
cm read '{"file_path":"config/config.go","offset":132,"limit":27}'
cm read '{"file_path":"config/config.go","offset":75,"limit":1}'

# Kubernetes 작업 트리에서
cm read '{"file_path":"staging/src/k8s.io/apimachinery/pkg/util/intstr/intstr.go","offset":80,"limit":7}'
```

설치된 `cm`은 `read`, `grep`을 제공한다. 나머지 도구는 실행기의 `query`로 같은 MCP 계약을 직접 호출할 수 있다. `--root`는 결과의 작업 트리 경로로 바꾼다.

```bash
python3 apps/codemap-search/scripts/public_validation.py query \
  --root /경로/nushell-structural/worktree \
  --ready-file crates/nu-command/src/filesystem/open.rs \
  overview '{"path":"crates/nu-command/src/filesystem/open.rs"}'

python3 apps/codemap-search/scripts/public_validation.py query \
  --root /경로/nushell-structural/worktree \
  --ready-file crates/nu-command/src/filesystem/open.rs \
  search '{"query":"open file detect content type","workspace_scope":"crates/nu-command","caller_context":false}'

python3 apps/codemap-search/scripts/public_validation.py query \
  --root /경로/nushell-structural/worktree \
  find '{"path":"crates/nu-path/src","pattern":"*.rs"}'
```

실행이 끝나면 `summary.json`의 개별 실패 항목에서 요청 인자와 누락·오연결 내용을 확인하고, 같은 디렉터리의 `mcp.jsonl`에서 원문을 찾는다. 미지원 관계를 기대값에서 삭제해 통과시키지 말고, 지원 범위와 수정 여부를 따로 판단한다.

`run`과 `regressions`는 검사 실패·보류가 있으면 종료 코드 1을 반환한다. `run`의 저장소별 실행 오류도 기록 후 1을 반환하며, 실행기 자체의 입력·시작·프로토콜 예외는 2로 보고한다. 최초 기준선의 종료 코드 1은 위에 기록한 제품 문제를 검출했다는 뜻이다. 최종 관련 재검증과 회귀 검사는 종료 코드 0이었다.

## 남은 범위

이번 범위는 Rust·Go 1차 검증이다. Java·Kotlin·Python·TypeScript 등 다른 언어, 프로젝트 전체에서 의미 분석으로 해소한 참조의 정확도, 플랫폼별 조건부 컴파일, proc macro 확장, 외부 의존성의 모든 정의, 대규모 동시 클라이언트 부하는 확인하지 않았다. 응답시간은 단일 개발 장비의 실행 관찰값이며 독립 반복 측정한 성능 보증값이 아니다.
