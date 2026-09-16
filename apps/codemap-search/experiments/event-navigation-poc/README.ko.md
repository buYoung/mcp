# 범용 이벤트 연결 분석 독립 PoC 결과

## 18개 개발 언어 확장 결과

2026-09-16 후속 작업에서 독립 PoC를 18개 개발 언어로 확장하고 언어별 미탐지 구문 18개를 보완했다. 현재 회귀 247개와 공통 사례 90개가 통과했다. 공개 양성 37개는 기본 입력에서 호출 후보 30·인자 전달 2·미확정 5이며, 확장 입력까지 반영하면 호출 후보 31·자료 반환 1·인자 전달 2·미확정 3이다. Monix 네 구현이 모두 조건부 연결됐고, 별도 소비 대조에서 Effect Queue 내부 쓰기와 한 bucket의 배열 반환도 확인했다. RPC의 clientId 조회 키 전달도 조건부 연결했다. Bevy 세 경로는 남아 있다. 최신 범위·검증·한계는 [잔여 경로 보완 결과](language-corpus/remaining-routes/README.ko.md), [언어별 PoC 결과](language-corpus/README.ko.md), [회귀 검증 결과](language-corpus/regressions/README.ko.md)에 있다. 아래 본문과 이 디렉터리의 results는 이전 네 언어 단계의 기록이다. 당시 구현·결과의 해시는 [보완 전 보존본](language-corpus/history/before-language-gap-fixes/manifest.json)에 있으며, 현재 Python 코드의 검증 결과와 구분한다.

검증일: 2026-09-15. **후속 보완으로 조건부 호출 후보가 14개에서 16개로 늘었고, 별도로 조회값의 호출 인자 전달 2개를 확인했다. 참고 쌍 23개 중 5개는 여전히 미확정이다.** 제품에 통합할 수준의 범용성·정밀도는 아직 입증하지 못했다.

이번에는 타입 별칭, 생성자·반환 객체, Go의 다중 반환값과 함수 타입 변환, 호출 인자 위치를 보존했다. Vendure의 전역 Set 콜백과 client-go의 Config.Process → DeltaFIFO.Pop 구간이 추가로 연결됐다. LiveStore의 Queue/clientId 전달은 외부 호출 인자까지 확인했으며, 이를 콜백 실행이나 실제 전달 성공으로 집계하지 않는다.

## 범위와 비교 기준

파일 분석은 사용자가 지정한 Sol·medium 하위 에이전트가 rg와 원문 읽기로 수행했다. 설계·구현·실행·결과 종합은 메인 스레드가 수행했다. 이번 작업에서도 codemap-search 도구·CLI를 사용하지 않았고, 대상 애플리케이션·서버·데이터베이스도 실행하지 않았다.

초기 구현과 결과는 [history/initial](history/initial/README.ko.md)에 보존했다. 이번 비교는 초기 구현을 같은 입력으로 다시 실행한 baseline, 보완 구현의 local, 보완 구현의 summaries 세 모드다. 기존 codemap-search 바이너리와의 수치 비교가 아니다.

| 저장소 | 고정 커밋 | 파일 | 소스 바이트 |
| --- | --- | ---: | ---: |
| [LiveStore](https://github.com/livestorejs/livestore) | 287936d11d3e6c7afbaacaf1ece14bf162120219 | 6 | 91,231 |
| [Vendure](https://github.com/vendurehq/vendure) | f78f402f1f706e42ab8e3b47698245846e958081 | 13 | 267,849 |
| [client-go](https://github.com/kubernetes/client-go) | 30803019f93fc7d7fccd93d115927705bf712be7 | 14 | 312,057 |
| [Bevy](https://github.com/bevyengine/bevy) | 29fe519f32a14503c8c0fab0baacafe76842bb76 | 21 | 684,617 |
| 합계 | | **54** | **1,355,754** |

초기의 52개 입력에 Go의 fifo.go와 the_real_fifo.go를 추가했다. 함수 타입 선언, Queue 계약과 실제 생성 대상 정의가 필요했기 때문이다. 초기 구현도 이 54개 파일로 다시 실행했으므로 이번 세 모드의 입력 바이트는 같다. 정확한 경로는 [corpus.json](corpus.json), 코드·입력 목록의 고정 해시는 [freeze.json](results/freeze.json)에 있다.

[참고 쌍](cases.json)은 기존 23개와 오연결 대조 8개를 그대로 사용했다. 새 테스트 파일이나 합성 사례는 작성하지 않았다. 콜백 경로 19개, 이벤트가 아닌 콜백 1개, 데이터 경로 3개가 포함된다. 분석기는 이 자료를 읽지 않고, 실행이 끝난 뒤 평가기가 위치와 관계 종류를 대조한다.

초기 development/evaluation 구분은 출처 기록으로 남겼다. 후속 보완에는 과거 evaluation 사례도 사용했으므로 현재 결과를 독립적인 검증 표본이나 저장소 전체의 재현율·정밀도로 해석하면 안 된다.

## 무엇을 바꿨는가

| 변경 | 해결한 문제 |
| --- | --- |
| 조회값 → 호출 인자 사실과 argument_index 보존 | Queue/clientId/함수가 외부 호출로 넘어가면 값의 흐름이 사라졌다. 이제 인자 전달과 값 자체의 호출을 구분한다. |
| 타입 별칭, imported binding, 생성자 인자·필드 처리 | 선언에서 얻은 타입과 생성자의 필드 대입이 실제 사용 지점으로 충분히 전달되지 않았다. |
| Go 함수 타입 변환과 포함 필드의 메서드 해석 | PopProcessFunc(...)를 보통 호출처럼 취급했고 Config에 포함된 Queue를 건너는 메서드 선택을 놓쳤다. |
| 다중 반환값·반환 대안의 위치 보존 | newQueueFIFO의 return f.logger, f를 하나의 불투명한 값으로 만들면서 실제 queue를 잃었다. |
| 소스에 저장된 실제 할당 타입을 통한 콜백 인자 전달 | 메서드 이름만으로 구현을 선택하지 않고, 저장값에서 찾은 구체 타입의 메서드 본문에 인자를 연결한다. |
| 의미가 같은 사실 중복 제거·관련 할당만 문맥화 | 동일 사실의 호출 경로 차이와 무관한 할당이 사실 한도를 소비했다. 한 번 문맥화됐다는 이유만으로 객체를 버리지 않도록 고쳤다. |
| 호출값에 가까운 관계 우선·전달 대안 묶음 | 전체 config나 컨테이너 초기화를 개별 콜백 저장처럼 출력하는 넓은 관계를 줄였다. 같은 인자 전달의 동적 키·문맥은 대안 수와 최대 3개 예로 표시한다. |

구현은 [syntax.py](syntax.py), [engine.py](engine.py), [model.py](model.py)에 있다. 실행·대조는 [analyze.py](analyze.py), [run.py](run.py), [measure.py](measure.py), [evaluate.py](evaluate.py)가 담당한다. 외부 의존성은 기존 [requirements.txt](requirements.txt)의 Tree-sitter와 TS·Rust·Go 문법 네 패키지로 유지했다. 라이브러리별 이벤트 API 규칙을 추가하지 않았다.

## 결과를 읽는 기준

- **조건부 호출 후보**: 저장된 함수의 호출 또는 저장된 객체의 메서드 호출을 소스 관계로 연결했다. 같은 인스턴스·키·분기 등 필요한 조건을 함께 표시한다.
- **인자 전달만 확인**: 저장값을 조회해 다른 호출의 인자로 넘겼다. 그 호출이 값을 최종 소비하거나 콜백을 실행하는지는 확인하지 않았다.
- **미확정**: 참고 쌍 양쪽을 자동 연결하지 못했다. 실제 관계가 없다는 판정이 아니다.

receiver_schema는 같은 타입의 수신자에 대한 조건부 구조이며 구체 인스턴스의 동일성 증명이 아니다. 모든 결과는 concrete_instance_proven=false, event_classification=not_inferred다. 일반 콜백 자료구조를 이벤트버스로 자동 분류하지 않는다.

stored_value_argument의 조건과 예시는 대표 대안이다. alternative_count가 더 크면 생략된 대안이 있으며, 원시 사실은 같은 결과 파일의 facts에 보존한다. 대표 조건만으로 모든 대안이나 런타임 실행을 확정해서는 안 된다.

## 참고 쌍 대조

| 저장소 | 초기 구현 호출 후보 | 보완 후 호출 후보 | 인자 전달만 확인 | 미확정 | 오연결 / 대조 쌍 |
| --- | ---: | ---: | ---: | ---: | ---: |
| LiveStore | 5 / 7 | 5 / 7 | 2 | 0 | 0 / 4 |
| Vendure | 3 / 5 | 4 / 5 | 0 | 1 | 0 / 2 |
| client-go | 6 / 7 | 7 / 7 | 0 | 0 | 0 / 2 |
| Bevy | 0 / 4 | 0 / 4 | 0 | 4 | 대조 쌍 없음 |
| 합계 | **14 / 23** | **16 / 23** | **2** | **5** | **0 / 8** |

보완 구현의 local 모드는 호출 후보 14개와 인자 전달 2개를 포착했다. summaries 모드에서 추가 호출 후보 2개가 나타났다. 쌍별 근거와 조건은 [baseline](results/baseline-evaluation.json), [local](results/local-evaluation.json), [summaries](results/summaries-evaluation.json)에 있다. 0건은 지정한 오연결 대조 8쌍에 한정되며, 모든 출력 후보를 검수한 정밀도 수치가 아니다.

새로 확인한 구간은 다음과 같다.

1. **Vendure 전역 Set**: globalRegistry의 register/get 구현과 반환값을 따라, defineDashboardExtension의 callback 저장에서 executeDashboardExtensionCallbacks의 호출까지 연결했다. 두 Set의 서로 다른 문자열 키는 유지된다. 등록 Set에서 변경 알림 Set의 호출로 직접 연결되는 결과가 없는 것도 별도로 확인했다. [저장·호출 소스](https://github.com/vendurehq/vendure/blob/f78f402f1f706e42ab8e3b47698245846e958081/packages/dashboard/src/lib/framework/extension-api/define-dashboard-extension.ts#L19).

2. **Go Config.Process → DeltaFIFO.Pop**: Config.Process에 저장한 closure, controller.config, 함수 타입 변환, Queue 인자, DeltaFIFO.Pop의 process(...)를 연결했다. newQueueFIFO의 두 반환값 중 두 번째가 queue라는 위치 정보가 필요했다. 결과에는 runtime_branch_selection_required와 dispatch_instance_unproven이 붙는다. DeltaFIFO 분기의 후보를 확인한 것이며, RealFIFO·PopBatch를 포함한 모든 분기의 완비성을 증명한 결과는 아니다. [큐 선택과 반환](https://github.com/kubernetes/client-go/blob/30803019f93fc7d7fccd93d115927705bf712be7/tools/cache/controller.go#L1063), [콜백 호출](https://github.com/kubernetes/client-go/blob/30803019f93fc7d7fccd93d115927705bf712be7/tools/cache/delta_fifo.go#L603).

3. **LiveStore Queue/clientId 인자 전달**: Map에서 찾은 queue가 Queue.offer의 첫 인자로, clientId가 writeResponse의 첫 인자로 전달되는 사실을 보존했다. 저장 키와 조회 키의 값이 같아야 한다는 조건 및 외부 소비 경계는 남는다. [Queue 전달](https://github.com/livestorejs/livestore/blob/287936d11d3e6c7afbaacaf1ece14bf162120219/packages/@livestore/webmesh/src/node.ts#L229), [응답 라우팅](https://github.com/livestorejs/livestore/blob/287936d11d3e6c7afbaacaf1ece14bf162120219/packages/@livestore/utils/src/effect/RpcClient.ts#L68).

client-go의 workqueue 예제와 shared informer listener 경로는 별개다. NewIndexerInformer는 Config.Process와 processDeltas를 거치며, sharedIndexInformer의 processorListener·채널 경로와 합쳐 설명하지 않는다.

## 언어별 사례는 필요한가

**필요하다. 공통 연결 규칙과 언어별 추출 규칙을 나눠 검증해야 한다.** 공통 모델의 같은 객체·같은 키·콜백 전달 규칙은 공유할 수 있지만, 각 언어의 구문과 선언이 그 모델로 올바르게 변환되는지는 해당 언어의 사례가 있어야 확인할 수 있다.

| 층 | 확인할 내용 | 이번에 사용한 실제 근거 |
| --- | --- | --- |
| 공통 연결 모델 | 서로 다른 객체·키 분리, 저장값의 조회, 호출 인자 위치, 호출과 단순 전달 구분 | LiveStore의 독립 구독 Map, Go의 서로 다른 함수 필드, Queue/clientId 전달 |
| JavaScript·TypeScript | 별칭·전역 binding·클로저·생성자 필드·컨테이너 반환값; JS에서 타입 선언 없이 확보할 수 있는 근거 | SharedService의 nonce Map, Vendure 전역 registry와 두 Set |
| Go | 이름 있는 함수 타입과 타입 변환, 포함 필드의 메서드, 다중 반환값, 값 복사와 포인터·인터페이스 경계 | PopProcessFunc(c.config.Process), return f.logger, f, Config 안의 Queue |
| Rust | 타입 별칭·구조체 분해·참조·Deref, generic·trait object·매크로 경계, 공유와 객체 복제 구분 | RegisteredSystem, MessageInstance, Arc와 load별 Box clone |

Go에서는 같은 CallExpr 형태도 callee가 타입으로 선언됐는지 함수로 선언됐는지에 따라 의미가 달라진다. [Go 변환 규칙](https://go.dev/ref/spec#Conversions). Rust의 필드 접근에도 참조의 자동 역참조가 관여하므로 구문 토큰만으로 값 동일성을 판정하면 부족하다. [Rust 필드 접근 규칙](https://doc.rust-lang.org/reference/expressions/field-expr.html).

현재 참고 쌍의 언어별 분포는 다음과 같다. 언어 전체 지원률이 아니라 선정한 실제 소스 쌍의 결과다.

| 언어 | 호출 후보 | 인자 전달만 확인 | 미확정 | 오연결 대조 쌍 |
| --- | ---: | ---: | ---: | ---: |
| TypeScript | 7 | 2 | 1 | 6 |
| JavaScript | 2 | 0 | 0 | 0 |
| Go | 7 | 0 | 0 | 2 |
| Rust | 0 | 0 | 4 | 0 |

**JavaScript와 Rust에는 오연결 대조가 부족하다.** 특히 Rust의 큰 프레임워크 사례만으로는 구문 추출 실패와 상위 의미론 부족을 충분히 분리하기 어렵다. 후속 검증에는 기존 코드에서 작게 분리한 선언·참조·구조체 분해 사례와 다른 인스턴스·키·clone을 구분하는 반례가 필요하다. 모든 공통 사례를 모든 언어에 복제할 필요는 없지만, 새 언어를 지원할 때 그 언어의 추출 경계를 검증해야 한다. 이번 작업에서는 추가 테스트 파일·합성 사례를 만들지 않았다.

## 남은 다섯 참고 쌍

| 사례 | 현재 남은 경계 |
| --- | --- |
| Vendure telemetry hook | injector.get의 결과와 실제 init 호출, 저장·조회에 쓰이는 class target 및 서비스 인스턴스 동일성을 현재 입력에서 연결하지 못했다. |
| Bevy observer | generic system과 runner 함수 포인터의 관계, Any downcast, trait 호출을 원래 사용자 system까지 연결하지 못했다. |
| Bevy 메시지 버퍼 | Deref/DerefMut, reader cursor, slice·iterator chain을 거친 payload 반환 관계가 부족하다. |
| Bevy SystemId | World/Entity의 component 조회, boxed system과 generic trait 호출 경계를 해결하지 못했다. |
| Bevy glTF handler | 공유 Arc, load별 Box clone, heterogeneous trait-object 호출을 구분해 끝까지 연결하지 못했다. |

이들이 모두 원리적으로 정적 분석 불가능하다는 뜻은 아니다. 현재 입력 범위와 정적 의미 모델이 충분하지 않다는 뜻이다. 특히 Observer에는 임의 runner와 별도 system을 구성하는 경로가 있으므로, runner 호출을 system 호출로 일반화해서 해결하면 안 된다. glTF의 Vec/Box clone도 원본 handler와 같은 인스턴스로 합쳐서는 안 된다.

추가 소스·타입·trait·매크로 해석이나 제한된 실행 관측이 필요한 범위를 구체화해야 한다. 이번에는 대상 프로그램 실행, 외부 라이브러리 전체 분석, 제품 통합으로 범위를 넓히지 않았다. 실제 이벤트 전달·순서·등록 수명은 검증하지 않았다.

## 비용

분석 시간은 파일 읽기·파싱·사실 추출·연결을 포함하고 Python 시작·JSON 직렬화를 제외한다. RSS는 **직렬화 전까지의 프로세스 최대 RSS**다. 세 모드는 baseline → local → summaries 순서로 각 한 번 실행했고, 모두 끝난 뒤 대조했다. 캐시·분산을 통제한 성능 벤치마크는 아니다.

| 저장소 | 초기 구현 분석 ms | 보완 summaries 분석 ms | 초기 RSS MiB | 보완 RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| LiveStore | 108.09 | 130.01 | 32.20 | 33.52 |
| Vendure | 404.27 | 498.78 | 49.36 | 51.47 |
| client-go | 547.70 | 668.08 | 46.66 | 50.47 |
| Bevy | 558.77 | 600.74 | 63.88 | 64.48 |

| 합계 지표 | 초기 구현 | 보완 local | 보완 summaries |
| --- | ---: | ---: | ---: |
| 분석 시간 | 1.619초 | 0.551초 | 1.898초 |
| 시작·직렬화 포함 경과 시간 | 2.062초 | 0.907초 | 2.362초 |
| 추출 사실 | 23,362 | 10,881 | 19,688 |
| 관계·전달 그룹 | 505 | 957 | 1,377 |
| 관계 JSON 바이트 | 656,311 | 1,351,394 | 2,333,622 |
| 조건부 호출 후보 / 참고 쌍 | 14 / 23 | 14 / 23 | 16 / 23 |
| 인자 전달만 확인 | 0 | 2 | 2 |

원시 지표: [baseline](results/baseline/metrics.json), [local](results/local/metrics.json), [summaries](results/summaries/metrics.json). 보완 결과의 1,377개 그룹은 직접 저장값 호출 100개, 객체 메서드 후보 596개, 인자 전달 681개다. 관계 종류와 묶음 방식이 달라졌으므로 전체 그룹 수를 그대로 품질 향상으로 해석할 수 없다.

전달 근거가 늘면서 출력 바이트도 늘었다. 제품에 붙이려면 질의 관련성·이벤트 관련성 선택과 출력 예산 설계가 더 필요하다. 현재 PoC는 모든 선정 파일의 원시 결과를 저장하며, 자동 grep/read 출력이나 제거한 Value relationships 출력을 복원하지 않았다.

## 한도와 미검증 사항

입력 한도는 파일당 512 KiB, 전체 4,096파일·64 MiB다. 함수별 사실 192개, 전체 사실 40,000개, 관계 그룹 2,048개 한도를 유지했다. 이번 최종 summaries 실행에는 입력·전체 사실·관계 출력 한도 초과가 없었다. 함수별 사실 한도는 11개 함수에 적용됐다.

할당 문맥은 실제로 참조되는 할당에 최대 3개 호출 위치를 보존한다. 조회는 깊이 6·방문 96회·반환 대안 16개, 반환값 대안과 다중값은 8개까지로 제한한다. summaries 결과에는 반환 closure 환경 미확정 8건, 값 대안 한도 1건, 지원하지 않는 binding pattern 종류 4개가 기록됐다. 네 저장소 모두 요약 반복 횟수 한도에 도달했으므로 고정점 수렴을 보장하지 않는다.

조건·삭제·여러 쓰기 표시는 보수적이다. 정확한 활성 등록 집합, SSA 수준의 쓰기 순서, Go의 모든 값 복사와 참조 차이, 클로저 캡처 환경과 구체 인스턴스 동일성을 완전히 계산하지 않는다. 구문 분석은 빌드 설정을 적용하지 않으므로 Rust의 조건부 컴파일·테스트 구획도 포함될 수 있다.

전체 후보 정밀도, 이름 변경에 대한 별도 변형 실험, 에이전트 추가 조회량·토큰, 실제 이벤트 전달은 측정하지 않았다. 현재 결과만으로 기존 제품 대비 도구 호출 감소나 전체 성능 향상을 주장할 수 없다.

## 재현과 검증

환경은 macOS arm64, Python 3.14.5, Git 2.50.1이다. 다른 Python 버전·운영체제에서 실행하지 않았다. 작업 디렉터리는 이 파일이 있는 experiments/event-navigation-poc다.

```sh
python3.14 -m venv .venv
.venv/bin/python -m pip install -r requirements.txt
.venv/bin/python prepare.py --sources /tmp/codemap-event-poc-sources
.venv/bin/python measure.py --sources /tmp/codemap-event-poc-sources --output /tmp/codemap-event-poc-results
```

[prepare.py](prepare.py)는 고정 커밋을 별도 디렉터리에 준비하고 기존 경로의 버전이 다르면 덮어쓰지 않는다. run.py는 분석 전에 커밋과 추적 파일 변경을 확인한다. measure.py는 초기 코드 스냅샷과 현재 코드를 검증하고 세 모드의 입력 바이트를 대조한 뒤 일괄 평가한다. 세부 사실·소스 해시·조건은 결과 디렉터리의 저장소별 .json.gz에 있다.

실제로 통과한 검증은 Python 파일 8개의 py_compile, 네 저장소×세 모드의 비교 실행 12회, 동일 입력 및 초기·현재 코드 해시 대조, 기존 참고·대조 쌍 31개 평가다. 평가 절차 통과와 모든 긍정 쌍의 포착은 구분해야 한다. 신규 복제는 초기 git clone으로 수행했고 prepare.py의 새 디렉터리 다운로드 분기는 별도로 실행하지 않았다.

초기 개발에서 발생한 SIGSEGV는 Point.column 대신 바이트 오프셋으로 위치를 계산한 뒤 같은 입력이 통과했다. [공식 이슈 #487](https://github.com/tree-sitter/py-tree-sitter/issues/487)과 유사하지만 동일 원인은 확정하지 않았다. 이 우회와 당시 원시 결과는 초기 스냅샷에도 보존했다.

Rust 제품 코드·기존 테스트·설정·인계 문서는 수정하지 않았다. 기존 이벤트 규칙과 입력 언어 경계는 [rules.rs](../../src/events/rules.rs), [events/mod.rs](../../src/events/mod.rs)의 소스를 읽어 비교했으며, 현재 제품 바이너리를 실행해 검증한 결과는 아니다.
