# 18개 개발 언어 이벤트 연결 PoC

검증일: 2026-09-16. **회귀 201개와 공통 사례 90개가 통과했다.** 공개 양성 37개는 기본 입력에서 호출 후보 30·인자 전달 2·미확정 5다. 확장 입력까지 반영하면 Monix와 Bevy 메시지가 연결되어 **호출 후보 31·데이터 반환 1·인자 전달 2·미확정 3**이다. 별도 소비 대조에서는 Effect Queue 내부 쓰기와 한 bucket의 배열 반환도 확인했다. 데이터 반환을 콜백 호출로 집계하지 않는다. Monix 네 구현 조합과 Effect 소비 구현의 분석 결과는 [잔여 경로 보완 보고서](remaining-routes/README.ko.md), 앞 단계의 공개 호출 후보 7개 복구 기록은 [1차 보완 보고서](public-gaps/README.ko.md)에 있다.

## 범위

사용자가 확정한 범위는 현재 codemap-search가 지원하는 개발 언어 전체다. Assembly를 포함하고 Bash/Zsh/PowerShell, 문서·마크업·스타일·설정·인프라·빌드·IDL·쿼리 형식은 제외했다. Groovy의 Gradle 파일도 입력으로 삼지 않았다. 제품의 등록부는 [src/lang/mod.rs](../../../src/lang/mod.rs)에 있으며, 정확한 포함·제외 목록은 [languages.json](languages.json)에 있다.

파일 분석은 Sol·medium 하위 에이전트가 원문과 rg로 수행했다. 1차 공개 경로 보완에서는 Sol 용량 부족 실패와 회귀 실패에 대해 사용자 지시에 따라 Astra·max 상세 분석을 2회 사용했다. 이번 잔여 경로 보완에는 추가 Astra 분석을 사용하지 않았다. 구현·사례 구성·측정·종합은 메인 스레드에서 수행했으며 codemap-search 도구·CLI를 사용하지 않았다. 대상 애플리케이션, 저장소의 테스트, 서버 및 외부 서비스는 실행하지 않았다.

공개 저장소 **18개, 서로 다른 소스 파일 96개, 공개 사례 77개**를 고정했다. 별도로 언어별 소스 예제 18개에 공통 계약 사례 90개를 구성했다. 테스트 코드가 공개 사례의 근거에 포함되는 경우에도 읽기와 정적 분석만 수행했다.

별도 회귀는 소스 99파일·201사례다. 체크포인트의 63파일·111사례, 140사례·165사례 단계의 원문과 기대값을 유지했다. 165사례 단계에 Scala/혼합 Java·Rust·TypeScript 재현과 반례 14파일·36사례를 더했다. 이전에 제시한 탐지/미탐지 예시 36파일은 원문을 변경하지 않았다. 그중 미탐지 18개는 `conditional_candidate`를 유지했고 기존 탐지 18개도 유지됐다. [구문별 변경·검증·한계](regressions/README.ko.md)에 언어별 근거와 재현 방법이 있다.

## 언어별 저장소와 결과

모든 언어에서 공통 예제의 양성 2개와 반례 3개가 통과했다. 아래 수치는 **기본 입력 96파일의 공개 소스 양성 쌍**만의 결과이며 `호출 후보 · 인자 전달만 · 미확정` 순서다. 저장소 링크는 선정 커밋으로 고정되어 있다.

| 언어 | 공개 코드 저장소 | 공개 양성 결과 | 확인할 구문·경계 |
| --- | --- | ---: | --- |
| Assembly | [pokered](https://github.com/pret/pokered/tree/a1a22aaf84d1675bcdbaeb194592379d586d838e) | 1 · 0 · 0 | 정적 주소 테이블·간접 점프·테이블 base/인덱스 분리 |
| C | [libuv](https://github.com/libuv/libuv/tree/a1ccce950cd470b131522dda40253e0c4f677900) | 1 · 0 · 0 | 구조체 함수 포인터·포인터 타입·전처리 조건 |
| C++ | [entt](https://github.com/skypjack/entt/tree/85c6bba014049b5de8fad49d25424df2f1f6a8c1) | 1 · 0 · 0 | delegate·템플릿·서로 다른 queue id·RAII 해제 |
| C# | [reactive](https://github.com/dotnet/reactive/tree/94b5d5ab912789f5abe9a72138a25bbd716fe59c) | 1 · 0 · 0 | observer 배열·CAS·IDisposable·종료 sentinel |
| Dart | [flutter](https://github.com/flutter/flutter/tree/27fec0e365a374f1d9c1ffb24b57b4fc69966c9a) | 1 · 0 · 0 | nullable 함수 목록·null-aware call·순회 중 추가/제거 |
| Go | [client-go](https://github.com/kubernetes/client-go/tree/30803019f93fc7d7fccd93d115927705bf712be7) | 7 · 0 · 0 | 함수 필드·함수 타입 변환·포함 필드·다중 반환값 |
| Groovy | [grails-core](https://github.com/apache/grails-core/tree/b449d532af9ee613da956e828405b1afc07393d4) | 1 · 0 · 0 | Closure 필드·wrapper 반환·subscriber와 reply 구분 |
| Java | [eventbus](https://github.com/greenrobot/EventBus/tree/0194926b3bcf70cc0d7bfd3c5da16708dd5ab876) | 1 · 0 · 0 | Class 키·Subscription·reflection·버스 인스턴스 격리 |
| JavaScript | [livestore](https://github.com/livestorejs/livestore/tree/287936d11d3e6c7afbaacaf1ece14bf162120219) | 2 · 0 · 0 | nonce Map·private cleanup 배열 분리 |
| Kotlin | [reaktive](https://github.com/badoo/Reaktive/tree/9d6cb63084bdd5d1fa3261e597b5ccc3d8e9bb3b) | 1 · 0 · 0 | 컬렉션 +=/-=·암시적 it·구독 이전 발행·dispose |
| Lua | [hump](https://github.com/vrld/hump/tree/08937cc0ecf72d1a964a8de6cd552c5e136bf0d4) | 1 · 0 · 0 | 함수인 table key·pairs의 key/value 역할·nil 삭제 |
| PHP | [event-dispatcher](https://github.com/symfony/event-dispatcher/tree/945f0a388a7b5d370f23e9faa1d5d48bff70a909) | 1 · 0 · 0 | 이벤트 키·priority·lazy factory·참조 cache |
| Python | [blinker](https://github.com/pallets-eco/blinker/tree/c3364059663df1ddce32799d6b1922af89a345f6) | 1 · 0 · 0 | sender 필터·generator·weakref 수명 |
| Ruby | [observer](https://github.com/ruby/observer/tree/5c871444fd184e1c55af38c7c3b70b769698c891) | 1 · 0 · 0 | 객체 Hash key·메서드 Symbol·동적 __send__ |
| Rust | [bevy](https://github.com/bevyengine/bevy/tree/29fe519f32a14503c8c0fab0baacafe76842bb76) | 0 · 0 · 4 | trait/generic·World/Entity·임의 runner·clone 객체 구분 |
| Scala | [monix](https://github.com/monix/monix/tree/b88e5331d761f12c41d244fad243cdad8c82ce1d) | 0 · 0 · 1 | immutable State·CAS·subscriber cache·Future/Ack |
| Swift | [rxswift](https://github.com/ReactiveX/RxSwift/tree/3e33f90c1bcd3cdea25bdb49bd4a594a50c3ab84) | 1 · 0 · 0 | bound method·Bag 내부 저장·값 snapshot·구독 key |
| TypeScript | [livestore](https://github.com/livestorejs/livestore/tree/287936d11d3e6c7afbaacaf1ece14bf162120219), [vendure](https://github.com/vendurehq/vendure/tree/f78f402f1f706e42ab8e3b47698245846e958081) | 8 · 2 · 0 | 객체·Map/Set·전역 registry·DI·인자 전달 |

확장 입력의 Rust는 이 표의 미확정 4개 중 메시지 버퍼 1개가 조건부 데이터 반환으로 바뀐다. Monix는 JVM·JavaScript × Scala 2·3 네 입력 모두 새 subscriber의 조건부 호출 후보를 확인했다. 원래 LiveStore 두 끝점은 인자 전달 단계이며, 별도 소비 끝점에서는 Queue 내부 쓰기와 단일 bucket 배열 반환까지 확인했다. RPC의 clientId 조회는 남아 있다. [입력별 결과와 근거](remaining-routes/comparison.json).

## 무엇을 구현했는가

- [analyze_languages.py](analyze_languages.py)가 언어에 따라 JS·TS·Go·Rust 엔진 또는 추가 언어 어댑터를 선택한다. 두 경로 모두 같은 [model.py](../model.py)의 저장 슬롯·키·조건부 연결 모델을 사용한다. 이번에는 두 엔진의 값 해석을 보완했으며, 보완 전 구현과 결과는 [history/before-language-gap-fixes](history/before-language-gap-fixes/manifest.json)에 별도로 보존했다.
- [polyglot.py](polyglot.py)는 추가 언어의 클래스·함수·매개변수·필드 대입·지역 별칭·컨테이너 저장/조회·순회·콜백 호출을 공통 사실로 변환한다. 반환값, generator의 yield, 생성자 저장과 명시된 수신자 타입을 제한된 함수 요약으로 전달한다. Kotlin의 컬렉션 연산, Dart의 selector/null 연산자, Ruby/Lua의 map key 역할은 각 구문에 맞게 처리한다.
- [assembly.py](assembly.py)는 명시적인 주소 테이블과 레지스터를 거친 간접 분기를 추적한다. 기존 RGBDS의 dw/주소 로드/jp hl 사례에 더해 x86의 rax/rbx 주소 로드·복사·간접 호출과 레지스터 덮어쓰기 대조를 검증했다. 상태를 레지스터별로 관리하고 폭이 좁은 쓰기나 해석하지 못하는 변경에서 주소 근거를 버린다. CPU·ABI 전체나 ROM 실행을 모사하지 않는다.
- [grammars.py](grammars.py)는 저장소에 포함된 Groovy 문법만 별도 공유 라이브러리로 빌드한다. 문법 묶음의 Groovy 파서는 클래스·메서드를 충분히 구분하지 못해 제품에 포함된 문법을 사용했다. Rust 제품 바이너리는 빌드·실행하지 않는다.
- [manage.py](manage.py)는 고정 커밋의 소스를 별도 경로에 준비하고, 입력·사례 해시 확인, 언어별 프로세스 실행, 결과 대조를 수행한다. 분석기는 cases.json을 읽지 않으며 평가기는 분석 후에만 기대 쌍을 읽는다. 이벤트 라이브러리 이름에 따른 on/emit 규칙은 추가하지 않았다.

새 외부 의존성은 문법 제공용 tree-sitter-language-pack 0.13.0과 그 의존 패키지다. 버전은 [requirements.txt](requirements.txt)에 고정했다. YAML/embedded-template 패키지는 문법 묶음의 설치 의존성이며 해당 형식을 분석 대상으로 추가한 것은 아니다. Groovy 빌드에는 C 컴파일러가 필요하다.

## 반례를 집계하는 기준

| 구분 | 수 | 실제 판정 |
| --- | ---: | --- |
| 공통 예제의 양성 | 36 | 36개 조건부 후보 확인 |
| 공통 예제의 반례 | 54 | 54개 무연결, 같은 언어의 양성도 연결됨 |
| 회귀 사례의 호출 양성 | 103 | 기존 탐지 18·기존 미탐지 18·추가 호출 양성 67, 모두 조건부 호출 후보 |
| 회귀 사례의 데이터 반환 양성 | 3 | 조건부 자료 반환, 콜백 호출과 별도 집계 |
| 회귀 사례의 데이터 소비 양성 | 4 | 객체 쓰기·읽기·조회 키 소비, 콜백 호출과 별도 집계 |
| 회귀 사례의 반례 | 87 | 87개 무연결, 같은 종류의 지정 양성 대조도 연결됨 |
| 새 회귀 사례의 미확정 유지 대조 | 4 | 이름 가림·미해결 값 등을 확정 연결하지 않음 |
| 공개 소스 양성 | 37 | 기본: 호출 30·인자 2·미확정 5. 확장 포함: 호출 31·반환 1·인자 2·미확정 3 |
| 공개 소스의 무연결 쌍 | 14 | 14개 무연결 확인, 판정 유보 0 |
| 공개 소스의 조건부 실행 시나리오 | 26 | 근거 확보, 실행 검증하지 않음 |

공통 예제는 서로 다른 필드·소유 타입 또는 Assembly 테이블을 합치지 않는지 확인한다. 특정 라이브러리 이름이나 정답 줄 번호를 분석기에 주입하지 않는다. [fixture-cases.json](fixture-cases.json)과 [fixtures](fixtures/)에 소스와 기대 쌍이 있다.

회귀 사례는 [regressions/cases.json](regressions/cases.json)에 소스 SHA-256과 정확한 저장·최종 소비 위치를 고정한다. 튜플 반례는 저장값의 매개변수까지 구분해 다른 위치의 값이 연결되는지 검사한다. C# 구독 해제는 소스 관계를 삭제했다고 주장하지 않고 `removal_may_prevent_call` 조건이 남는지 확인한다. 미확정 유지 대조 4개는 구조적으로 무연결인 87개 반례와 별도 집계한다.

체크포인트에서 판정 유보였던 **Groovy·Java 2개도 이번에 양성 경로와 함께 확인했다.** 공개 무연결 쌍 14개 모두 양성 대조를 갖춘 무연결이다. 미지원·추출 실패 때문에 아무 결과도 나오지 않는 경우를 반례 통과로 집계하지 않는 기준을 유지했다.

구독 해제·GC·종료 상태·스케줄러·다른 인스턴스 같은 시나리오는 generic 함수의 소스 관계 자체를 부정하지 않을 수 있다. 따라서 해당 26개는 source_scenario_not_executed로 남겼다. 기존 테스트의 assert는 기대 동작을 파악하는 근거이며 이번에 실행해 관측한 결과가 아니다.

모든 관계는 conditional_source_relation이며 concrete_instance_proven=false, event_classification=not_inferred다. 함수 값이 컨테이너를 거친다고 자동으로 이벤트버스라고 분류하지 않는다. 인자 전달·자료 반환·객체 쓰기·조회 키 소비는 callback 실행과 구분한다. `stored_value_return`은 `conditional_data_return`으로 평가하며 실제 전달 성공을 뜻하지 않는다.

## 남은 의미 해석 경계

| 미확정 공개 양성 | 남은 경계 |
| --- | --- |
| Rust 3개 | 확장 입력에서 메시지 버퍼의 반환은 연결. Bevy observer·SystemId의 World/Entity·trait와 glTF의 복제 객체 기원은 미확정 |
| TypeScript 인자 전달 2개 | 추가 끝점에서 Queue 쓰기·단일 bucket 배열 반환은 연결. RPC clientId → 실제 handler 조회는 미완성. Queue의 모든 bucket 순회·실제 payload 전달은 미검증 |

이 표는 현재 PoC가 끝까지 연결하지 못한 구간이다. 언어 자체에서 정적 분석이 불가능하다는 뜻은 아니다. 복구된 7개도 CAS 성공, 실제 reflection 대상, lazy factory 전체, Bag의 모든 저장 형태, DI·target 동일성 등은 미확정이다. [경로별 범위](remaining-routes/README.ko.md)에 자동 연결과 원문 확인, 미완료 작업을 나누어 기록했다.

## 구문 복구와 분석 한도

공통 예제 18개는 Tree-sitter 구문 오류·누락 토큰 없이 처리했다. Groovy 예제는 세미콜론을 명시했다. 공개 입력에는 C++ 3파일, Groovy 3파일, C 1헤더, Assembly 2파일, Kotlin 2파일에 구문 복구/누락 토큰이 남는다. Swift는 조건부 컴파일 지시문의 비개행 바이트를 가린 대체 구문 트리가 오류 없이 생성될 때만 복구하며, 빌드 분기 선택은 `conditional_compilation_selection_unproven`으로 남긴다. 원시 위치와 수는 [metrics.json](results/metrics.json)의 notices에 있다. 일부 구간의 후보를 파일 전체 해석 성공으로 해석해서는 안 된다.

C의 async.c는 전처리 분기가 문장 구조를 가르는 문제 때문에 바이트 위치를 유지한 조건부 소스 뷰로 파싱했다. include guard는 보존하지만 매크로 값, 선택한 분기 조합의 일관성, 다른 빌드 설정은 확인하지 않았다. 해당 사실에는 preprocessor_configuration_unproven 조건이 붙는다. Assembly는 원문 문법의 일부가 파서와 맞지 않아도 별도 주소·명령 어댑터가 명시적으로 지원하는 구간을 읽으며, register_provenance_subset과 인덱스 조건을 남긴다.

추가 언어 어댑터는 최대 4회 요약, 함수당 256사실, 전체 40,000사실, 관계 4,096개로 제한한다. 기본 입력은 파일당 512 KiB·전체 64 MiB·4,096파일 이하다. 사용자 선택에 따라 Effect 확장 실행에만 파일당 1 MiB를 적용했다. 생성자 할당 대안 8개, 투영 단계 4회·단계별 32대안 한도가 있다. 기존 엔진은 함수당 192사실·관계 2,048개를 유지하되 모듈은 선언별 사실 예산을 사용한다. TypeScript 실제 인자 해석 깊이 6, Scala/혼합 Java 소스 호출 깊이 10, prototype 문맥 256개·6회 확장으로 제한한다. 한도나 구문 오류는 notices로 노출하며, 정확한 실행 순서·객체 수명·SSA·모든 overload/trait/macro·복사 의미를 해석하지 않는다.

Effect 확장 입력 12파일의 TypeScript 타입 구문은 바이트 위치를 보존한 대체 트리로 복구했다. 전체 대체 트리에 오류가 없을 때만 사용하며 `generic_type_constraints_unproven`을 남긴다. 타입 검사·컴파일 성공과 다르다. [복구 범위와 한계](remaining-routes/README.ko.md).

## 재현

실행 환경은 macOS arm64, Python 3.14.5이다. 다른 운영체제·Python 버전과 대상 프로젝트 자체의 빌드·런타임은 검증하지 않았다. 다음 명령의 작업 디렉터리는 이 README가 있는 language-corpus다.

```sh
python3.14 -m venv .venv
.venv/bin/python -m pip install -r requirements.txt
.venv/bin/python grammars.py --build
.venv/bin/python manage.py prepare --sources /tmp/codemap-event-language-sources
.venv/bin/python manage.py check --sources /tmp/codemap-event-language-sources
.venv/bin/python manage.py measure --sources /tmp/codemap-event-language-sources --output /tmp/codemap-event-language-results
```

빠른 공통 사례 대조는 `manage.py measure --fixtures --output /tmp/codemap-language-fixtures`로 수행한다. 위 전체 소스 준비 후 `--language python` 또는 `--repo blinker`로 측정 범위를 좁힐 수 있다. prepare는 이미 있는 다른 커밋을 덮어쓰지 않으며 저장소의 설치·빌드·테스트 스크립트를 실행하지 않는다.

새 구문 회귀는 `.venv/bin/python regressions/run.py`로 재현한다. 특정 언어만 확인할 때는 뒤에 `python ruby`처럼 언어를 나열한다. 전체 결과는 [회귀 evaluation.json](regressions/results/evaluation.json)에 있으며, 현재 구현·입력·결과의 해시 대조는 [verification.json](regressions/results/verification.json)에 기록했다.

[sources.lock.json](sources.lock.json)은 커밋·파일 SHA-256·각 근거 구간의 SHA-256·영구 링크를 보관한다. 현재 입력은 선택 버전 3이며, [selection-revisions.json](selection-revisions.json)에 Groovy 반례 줄 번호 교정, Swift Bag의 canonical 소스 추가, Groovy 예제 구문 조정을 기록했다. 이전 입력 잠금도 history에 남겼다. 평가에 사용한 사례를 보완 과정에서 보았으므로 독립 holdout 표본으로 주장하지 않는다.

검증 결과와 관계 근거는 [evaluation.json](results/evaluation.json), 실행별 비용과 진단은 [metrics.json](results/metrics.json), 원시 사실·코드 해시·소스 해시·커밋은 [analysis](results/analysis/)에 있다. 각 프로세스는 원문 읽기와 정적 분석만 수행한다. LiveStore는 JS와 TS 실행에 같은 여섯 파일을 제공하므로 실행별 파일 수를 합하면 중복 입력이 포함된다.

최종 수정 후 기본 167개 대조와 회귀 201개를 실행했다. 기본·공통 37개, 회귀 91개, 추가 입력 6개로 총 134개 분석 결과를 저장했다. 기본 사례의 판정은 직전 단계와 같으며, 확장 입력에서 Bevy 자료 반환 1개와 Monix 호출 후보 1개를 확인했다. 원래 공개 37쌍과 분리한 [추가 소비 평가](remaining-routes/results/consumers.evaluation.json)는 Queue 쓰기·단일 bucket 배열 반환·Effect 내부 handler 호출·두 무연결 쌍을 확인하고 RPC 최종 조회를 미확정으로 남긴다. 구현·입력·결과 해시는 `remaining-routes/verify.py --sources /tmp/codemap-language-repro-sources`로 대조한다. 이 명령은 기존 `public-gaps/verify.py`도 실행한다. [확장 입력 준비·재현 절차](remaining-routes/README.ko.md)와 [통합 무결성 결과](remaining-routes/verification.json)에 세부 범위가 있다. 모든 출력 후보의 정밀도·재현율, 임의의 이름 변형에 대한 강건성, 실제 이벤트 전달·순서·성능 개선은 측정하지 않았다.

이번 변경은 experiments/event-navigation-poc 안에 한정했다. Rust 제품 코드·기존 테스트·설정·인계 문서를 수정하지 않았고 기존 MCP/CLI 계약이나 제거했던 Value relationships 출력도 바꾸지 않았다. 기존 네 언어의 비교 결과는 [이전 결과 문서](../README.ko.md)에 보존했다.
