# 18개 개발 언어 이벤트 연결 PoC

검증일: 2026-09-16. **18개 개발 언어에서 추가로 재현한 미탐지 예시 18개를 모두 최종 호출까지 연결하도록 보완했다.** 새 회귀 사례 111개와 기존 공통 사례 90개가 통과했다. 공개 저장소의 양성 참고 쌍 37개는 수정 전과 동일하게 조건부 호출 후보 23개, 인자 전달 2개, 미확정 12개다. 이번 구문 보완과 복잡한 프레임워크 경로 전체 지원은 구분해야 한다.

## 범위

사용자가 확정한 범위는 현재 codemap-search가 지원하는 개발 언어 전체다. Assembly를 포함하고 Bash/Zsh/PowerShell, 문서·마크업·스타일·설정·인프라·빌드·IDL·쿼리 형식은 제외했다. Groovy의 Gradle 파일도 입력으로 삼지 않았다. 제품의 등록부는 [src/lang/mod.rs](../../../src/lang/mod.rs)에 있으며, 정확한 포함·제외 목록은 [languages.json](languages.json)에 있다.

파일 분석은 Sol·medium 하위 에이전트가 원문과 rg로 수행했다. 구현·사례 구성·측정·종합은 메인 스레드에서 수행했으며 codemap-search 도구·CLI를 사용하지 않았다. 대상 애플리케이션, 저장소의 테스트, 서버 및 외부 서비스는 실행하지 않았다.

공개 저장소 **18개, 서로 다른 소스 파일 96개, 공개 사례 77개**를 고정했다. 별도로 언어별 소스 예제 18개에 공통 계약 사례 90개를 구성했다. 테스트 코드가 공개 사례의 근거에 포함되는 경우에도 읽기와 정적 분석만 수행했다.

이번 보완에는 별도 회귀 소스 63파일·111사례를 추가했다. 이전에 제시한 탐지/미탐지 예시 36파일은 원문을 변경하지 않았다. 그중 미탐지 18개가 `unresolved`에서 `conditional_candidate`로 바뀌었고, 기존 탐지 18개는 유지됐다. [구문별 변경·검증·한계](regressions/README.ko.md)에 언어별 근거와 재현 방법이 있다.

## 언어별 저장소와 결과

모든 언어에서 공통 예제의 양성 2개와 반례 3개가 통과했다. 아래 수치는 **공개 소스 양성 쌍**만의 결과이며 `호출 후보 · 인자 전달만 · 미확정` 순서다. 저장소 링크는 선정 커밋으로 고정되어 있다.

| 언어 | 공개 코드 저장소 | 공개 양성 결과 | 확인할 구문·경계 |
| --- | --- | ---: | --- |
| Assembly | [pokered](https://github.com/pret/pokered/tree/a1a22aaf84d1675bcdbaeb194592379d586d838e) | 1 · 0 · 0 | 정적 주소 테이블·간접 점프·테이블 base/인덱스 분리 |
| C | [libuv](https://github.com/libuv/libuv/tree/a1ccce950cd470b131522dda40253e0c4f677900) | 1 · 0 · 0 | 구조체 함수 포인터·포인터 타입·전처리 조건 |
| C++ | [entt](https://github.com/skypjack/entt/tree/85c6bba014049b5de8fad49d25424df2f1f6a8c1) | 0 · 0 · 1 | delegate·템플릿·서로 다른 queue id·RAII 해제 |
| C# | [reactive](https://github.com/dotnet/reactive/tree/94b5d5ab912789f5abe9a72138a25bbd716fe59c) | 0 · 0 · 1 | observer 배열·CAS·IDisposable·종료 sentinel |
| Dart | [flutter](https://github.com/flutter/flutter/tree/27fec0e365a374f1d9c1ffb24b57b4fc69966c9a) | 1 · 0 · 0 | nullable 함수 목록·null-aware call·순회 중 추가/제거 |
| Go | [client-go](https://github.com/kubernetes/client-go/tree/30803019f93fc7d7fccd93d115927705bf712be7) | 7 · 0 · 0 | 함수 필드·함수 타입 변환·포함 필드·다중 반환값 |
| Groovy | [grails-core](https://github.com/apache/grails-core/tree/b449d532af9ee613da956e828405b1afc07393d4) | 0 · 0 · 1 | Closure 필드·wrapper 반환·subscriber와 reply 구분 |
| Java | [eventbus](https://github.com/greenrobot/EventBus/tree/0194926b3bcf70cc0d7bfd3c5da16708dd5ab876) | 0 · 0 · 1 | Class 키·Subscription·reflection·버스 인스턴스 격리 |
| JavaScript | [livestore](https://github.com/livestorejs/livestore/tree/287936d11d3e6c7afbaacaf1ece14bf162120219) | 2 · 0 · 0 | nonce Map·private cleanup 배열 분리 |
| Kotlin | [reaktive](https://github.com/badoo/Reaktive/tree/9d6cb63084bdd5d1fa3261e597b5ccc3d8e9bb3b) | 1 · 0 · 0 | 컬렉션 +=/-=·암시적 it·구독 이전 발행·dispose |
| Lua | [hump](https://github.com/vrld/hump/tree/08937cc0ecf72d1a964a8de6cd552c5e136bf0d4) | 1 · 0 · 0 | 함수인 table key·pairs의 key/value 역할·nil 삭제 |
| PHP | [event-dispatcher](https://github.com/symfony/event-dispatcher/tree/945f0a388a7b5d370f23e9faa1d5d48bff70a909) | 0 · 0 · 1 | 이벤트 키·priority·lazy factory·참조 cache |
| Python | [blinker](https://github.com/pallets-eco/blinker/tree/c3364059663df1ddce32799d6b1922af89a345f6) | 1 · 0 · 0 | sender 필터·generator·weakref 수명 |
| Ruby | [observer](https://github.com/ruby/observer/tree/5c871444fd184e1c55af38c7c3b70b769698c891) | 1 · 0 · 0 | 객체 Hash key·메서드 Symbol·동적 __send__ |
| Rust | [bevy](https://github.com/bevyengine/bevy/tree/29fe519f32a14503c8c0fab0baacafe76842bb76) | 0 · 0 · 4 | trait/generic·World/Entity·임의 runner·clone 객체 구분 |
| Scala | [monix](https://github.com/monix/monix/tree/b88e5331d761f12c41d244fad243cdad8c82ce1d) | 0 · 0 · 1 | immutable State·CAS·subscriber cache·Future/Ack |
| Swift | [rxswift](https://github.com/ReactiveX/RxSwift/tree/3e33f90c1bcd3cdea25bdb49bd4a594a50c3ab84) | 0 · 0 · 1 | bound method·Bag 내부 저장·값 snapshot·구독 key |
| TypeScript | [livestore](https://github.com/livestorejs/livestore/tree/287936d11d3e6c7afbaacaf1ece14bf162120219), [vendure](https://github.com/vendurehq/vendure/tree/f78f402f1f706e42ab8e3b47698245846e958081) | 7 · 2 · 1 | 객체·Map/Set·전역 registry·DI·인자 전달 |

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
| 새 회귀 사례의 양성 | 67 | 기존 탐지 18·기존 미탐지 18·추가 양성 31, 모두 조건부 호출 후보 |
| 새 회귀 사례의 반례 | 40 | 40개 무연결, 지정 양성 대조도 연결됨 |
| 새 회귀 사례의 미확정 유지 대조 | 4 | 이름 가림·미해결 값 등을 확정 연결하지 않음 |
| 공개 소스 양성 | 37 | 호출 후보 23, 인자 전달 2, 미확정 12 |
| 공개 소스의 무연결 쌍 | 14 | 12개 무연결 확인, 2개 판정 유보 |
| 공개 소스의 조건부 실행 시나리오 | 26 | 근거 확보, 실행 검증하지 않음 |

공통 예제는 서로 다른 필드·소유 타입 또는 Assembly 테이블을 합치지 않는지 확인한다. 특정 라이브러리 이름이나 정답 줄 번호를 분석기에 주입하지 않는다. [fixture-cases.json](fixture-cases.json)과 [fixtures](fixtures/)에 소스와 기대 쌍이 있다.

새 회귀 사례는 [regressions/cases.json](regressions/cases.json)에 소스 SHA-256과 정확한 저장·최종 호출 위치를 고정한다. 튜플 반례는 저장값의 매개변수까지 구분해 다른 위치의 값이 연결되는지 검사한다. C# 구독 해제는 소스 관계를 삭제했다고 주장하지 않고 `removal_may_prevent_call` 조건이 남는지 확인한다. 미확정 유지 대조 4개는 구조적으로 무연결인 40개 반례와 별도 집계한다.

공개 무연결 쌍의 **Groovy·Java 2개는 판정 유보**다. 오연결 출력은 없지만 같은 저장소의 양성 경로를 아직 연결하지 못했으므로 성공으로 집계하지 않는다. 미지원·추출 실패 때문에 아무 결과도 나오지 않는 경우가 반례 통과로 둔갑하지 않도록 한 기준이다.

구독 해제·GC·종료 상태·스케줄러·다른 인스턴스 같은 시나리오는 generic 함수의 소스 관계 자체를 부정하지 않을 수 있다. 따라서 해당 26개는 source_scenario_not_executed로 남겼다. 기존 테스트의 assert는 기대 동작을 파악하는 근거이며 이번에 실행해 관측한 결과가 아니다.

모든 관계는 conditional_source_relation이며 concrete_instance_proven=false, event_classification=not_inferred다. 함수 값이 컨테이너를 거친다고 자동으로 이벤트버스라고 분류하지 않는다. 인자 전달은 최종 소비나 callback 실행과 구분한다.

## 남은 의미 해석 경계

| 미확정 공개 양성 | 남은 경계 |
| --- | --- |
| C++ 1개 | sink의 포인터·템플릿 별칭에서 sigh의 delegate 목록과 최종 호출까지 |
| C# 1개 | CAS로 교체한 배열과 SubjectDisposable.Observer 접근을 통한 실제 callback 연결 |
| Groovy 1개 | 구독 wrapper의 Closure가 별도 trigger 객체로 넘어가 호출되는 경로 |
| Java 1개 | Subscription/SubscriberMethod를 거쳐 reflection의 Method와 수신 객체를 결합하는 경로 |
| PHP 1개 | priority 배열에서 최적화·참조 cache 및 lazy callable 해석까지 |
| Scala 1개 | immutable State·CAS·cache 배열·Future/Ack를 거친 subscriber 흐름 |
| Swift 1개 | generic Bag의 복수 저장 형태·bound method·반환 snapshot·모듈/타입 경계 |
| Rust 4개 | 기존 Bevy observer, 메시지 버퍼, SystemId, glTF trait/clone 경로 |
| TypeScript 1개 | 기존 Vendure telemetry의 DI와 서비스·target 동일성 |

이 표는 현재 PoC가 끝까지 연결하지 못한 구간이다. 언어 자체에서 정적 분석이 불가능하다는 뜻은 아니다. 언어별 작은 저장 슬롯 사례 통과와 프레임워크 경로 전체 지원은 별개다.

## 구문 복구와 분석 한도

공통 예제 18개는 Tree-sitter 구문 오류·누락 토큰 없이 처리했다. Groovy 예제는 세미콜론을 명시했다. 공개 입력에는 C++ 3파일, Groovy 3파일, C 1헤더, Assembly 2파일, Kotlin 2파일, Swift 1파일에 구문 복구/누락 토큰이 남는다. 원시 위치와 수는 [metrics.json](results/metrics.json)의 notices에 있다. 정상적으로 파싱된 일부 구간의 후보를 파일 전체 해석 성공으로 해석해서는 안 된다.

C의 async.c는 전처리 분기가 문장 구조를 가르는 문제 때문에 바이트 위치를 유지한 조건부 소스 뷰로 파싱했다. include guard는 보존하지만 매크로 값, 선택한 분기 조합의 일관성, 다른 빌드 설정은 확인하지 않았다. 해당 사실에는 preprocessor_configuration_unproven 조건이 붙는다. Assembly는 원문 문법의 일부가 파서와 맞지 않아도 별도 주소·명령 어댑터가 명시적으로 지원하는 구간을 읽으며, register_provenance_subset과 인덱스 조건을 남긴다.

추가 언어 어댑터는 최대 4회 요약, 함수당 256사실, 전체 40,000사실, 관계 4,096개로 제한한다. 입력은 파일당 512 KiB·전체 64 MiB·4,096파일 이하다. 생성자 할당 대안 8개, 투영 단계 4회·단계별 32대안 한도가 있다. 기존 네 언어는 이전 엔진의 한도를 유지한다. 한도나 구문 오류는 notices로 노출하며, 정확한 실행 순서·객체 수명·SSA·모든 overload/trait/macro·복사 의미를 해석하지 않는다.

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

이번 수정 후 기존 167개 대조와 새 회귀 111개를 실행했다. 이미 별도 경로에 준비해 둔 18개 저장소의 입력 잠금을 확인하고 기존 37개 분석 프로세스와 회귀 62개 분석 프로세스를 실행했다. 기존 167개 사례의 판정 변화는 0이며, 새 회귀 111개도 통과했다. Python 구현 구문과 원시 결과의 코드·소스·커밋 해시도 대조했다. 미확정·판정 유보·미실행 시나리오는 위 숫자로 별도 기록했다. 모든 출력 후보를 검수한 저장소 전체 정밀도·재현율, 임의의 이름 변형에 대한 강건성, 실제 이벤트 전달·순서·성능 개선은 측정하지 않았다.

이번 변경은 experiments/event-navigation-poc 안에 한정했다. Rust 제품 코드·기존 테스트·설정·인계 문서를 수정하지 않았고 기존 MCP/CLI 계약이나 제거했던 Value relationships 출력도 바꾸지 않았다. 기존 네 언어의 비교 결과는 [이전 결과 문서](../README.ko.md)에 보존했다.
