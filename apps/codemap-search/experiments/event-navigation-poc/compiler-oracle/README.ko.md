# 컴파일러 보조 검증과 제품 의존성 경계

**컴파일러 보조 입력은 독립 PoC의 대조 자료로만 사용한다. codemap-search 제품이나 기본 소스 분석기의 실행 조건으로 추가하지 않는다.** 사용자 결정에 따라 MIR을 읽는 별도 검증 명령을 만들었으며, 이 디렉터리는 기본 분석 경로에서 import하지 않는다. 컴파일러가 확인한 사실을 AST 자동 연결 성공으로 합산하지 않는다.

검증일은 2026-09-16이다. Bevy observer·SystemId·glTF의 컴파일러 관측 근거를 확보했지만, 세 경로의 **소스 AST 자동 연결은 여전히 미완료**다. 컴파일러 출력만으로 서로 다른 등록·호출 시점의 같은 World·Entity·handler 인스턴스나 실제 이벤트 전달을 증명하지 않는다. [AST 평가](../language-corpus/remaining-routes/results/bevy-modules.evaluation.json), [컴파일러 대조 결과](results/evaluation.json).

## 입력과 실행 범위

| 항목 | 실제 사용 범위 |
| --- | --- |
| Bevy 소스 | `29fe519f32a14503c8c0fab0baacafe76842bb76`, 별도 worktree |
| 컴파일러 | Rust 1.98.1, `48a229ceaefd4985c50990b14116b6d856af0985`, aarch64-apple-darwin |
| ECS | `--lib --no-default-features --features std`, MIR·rustdoc JSON |
| glTF | `--lib`, crate 기본 특성, MIR |
| 포인터 | `--lib`, crate 기본 특성, MIR·rustdoc JSON |
| 의존성 | 이 검증에서 생성해 고정한 [Cargo.lock.gz](Cargo.lock.gz), 이후 `--locked` 사용 |
| 실행 여부 | 라이브러리 컴파일과 그 과정의 build script·proc-macro 실행. 앱·서버·테스트·이벤트 시나리오는 실행하지 않음 |

원래 revision에는 Cargo.lock이 없으므로 이 잠금 파일을 upstream의 공식 의존성 고정본으로 부르지 않는다. 컴파일러 전체 버전, 실제 활성 특성, 정확한 명령, MIR·메타데이터·로그 해시는 [build.json](results/build.json)에 기록했다. ECS의 `std` 구성과 glTF의 기본 구성을 같은 빌드 구성으로 합치지 않는다.

`RUSTC_BOOTSTRAP=1`로 `--emit=mir -Zmir-include-spans=yes`와 rustdoc JSON을 수집했다. 컴파일러가 다르면 build 명령은 중단하며, rustdoc JSON은 format 60만 읽는다. ECS rustdoc의 제외된 특성 관련 링크 경고는 [로그](results/bevy_ecs.rustdoc.txt)에 보존했다. 다른 컴파일러·타깃·특성 조합은 검증하지 않았다.

## 무엇이 확인됐는가

소스 위치와 기대 컴파일러 표현은 [inputs.json](inputs.json)에 고정한다. 이는 관측 결과를 대조하는 평가 자료이며 소스 분석기에 전달되지 않는다. [collect.py](collect.py)는 MIR의 함수·명령·타입·원문 위치와 rustdoc의 선택된 trait 구현을 추출한다. Bevy API 이름에 따른 연결 규칙이나 저장 효과 요약을 만들지 않는다. 원시 근거는 [observations.json.gz](results/observations.json.gz)에 있다.

| 구간 | 컴파일러에서 확인한 근거 | 여전히 증명되지 않은 것 |
| --- | --- | --- |
| Observer | `IntoObserverSystem` 변환, Box unsize, 구체 runner 함수 포인터, `Observer` 조회, `Any` 다운캐스트와 system 호출 | 캐시가 선택한 Entity와 원래 등록 인스턴스의 동일성, custom runner와 default runner의 실행 선택 |
| SystemId | `RegisteredSystem<I,O>`의 생성, typed spawn→Entity→SystemId, typed component 조회·Option take·system 호출. derive 위치에서 생성된 `Component` 구현 | 서로 다른 register/run 호출 사이의 World·Entity·component 저장 슬롯 동일성, 실제 재삽입 순서 |
| glTF | Arc 공유 참조 clone, lock guard→load별 Vec clone, Box의 dyn pointer projection·`dyn_clone`, erased hook→generic handler 호출 | 특정 예제 handler의 런타임 vtable, 등록 객체와 load별 복제 객체 사이의 정확한 기원 연결 |
| 포인터 보조 | `MaybeUninit` 참조→NonNull→typed cast, raw pointer의 typed read | byte offset·필드·row별 물리 저장 위치, 수명·정렬·aliasing과 값의 유효성 |

MIR 관측 12항목과 derive trait 구현 1항목을 대조한다. 이 13항목은 공개 양성 37쌍이나 AST 회귀 수에 더하지 않는다. 출력에 `analyzer_input=false`, `product_dependency=false`, `ast_coverage_credit=false`, `end_to_end_connection_proven=false`를 명시한다.

## 제품에 적용할 수 있는 변경

제품에 이식할 후보는 소스만 읽는 [rust_types.py](../rust_types.py), [rust_values.py](../rust_values.py), [rust_macros.py](../rust_macros.py)와 공통 값 분석의 보완이다. 현재 제품 Rust/MCP 코드는 변경하지 않았다. 컴파일러 자료가 필요한 경로를 제품의 지원 범위로 표시하지 않는다.

소스 분석은 제한된 단일 식별자 매크로의 실제 본문을 치환하고, 원문 해시와 위치를 보존한다. 명시적 제네릭 인자·구체 타입 키·tuple Map 키·검증 가능한 Any 다운캐스트·초기화된 포인터 출처를 추적한다. 임의 proc-macro 실행, 반복 matcher 전체, generic trait 선택, ECS 물리 저장소와 clone 기원 분석은 별도 미구현 경계다.

[verify.py](verify.py)는 기본 분석기의 import와 저장된 결과를 대조하고, `rustc`·`cargo`를 찾을 수 없는 PATH에서 외부 소스 매크로→포인터→최종 호출 회귀를 실행한다. 이 검증은 **기본 분석이 컴파일러 보조 자료에 의존하지 않음**을 확인하는 표본이며, Bevy 세 경로의 해결을 뜻하지 않는다. [독립성·무결성 결과](results/verification.json).

## 재현

작업 디렉터리는 `apps/codemap-search`다. 고정 revision의 별도 Bevy worktree와 위 컴파일러가 필요하다. 처음에는 고정 Cargo.lock을 복원하며, 이미 다른 lock이 있으면 덮어쓰지 않고 중단한다. 기본 PoC를 실행하는 데 아래 명령은 필요하지 않다.

```sh
/tmp/codemap-event-poc-venv/bin/python experiments/event-navigation-poc/compiler-oracle/build.py --root /tmp/codemap-bevy-compiler-worktree
/tmp/codemap-event-poc-venv/bin/python experiments/event-navigation-poc/compiler-oracle/collect.py --root /tmp/codemap-bevy-compiler-worktree
/tmp/codemap-event-poc-venv/bin/python experiments/event-navigation-poc/compiler-oracle/verify.py
```

새 환경에서의 전체 다운로드·빌드 재현, 다른 플랫폼, 대상 프로그램의 런타임 전달은 확인하지 않았다. 저장된 결과와 지금 사용한 worktree·컴파일러·입력 해시를 확인한 범위다.
