# 범용 이벤트 연결 분석 독립 PoC 결과

검증일: 2026-09-15. **소스에 드러난 일부 콜백 저장·호출은 라이브러리별 이벤트 API 규칙 없이 연결할 수 있었다. 하지만 이번 구현으로 범용 이벤트 탐색이 해결됐다고 판단할 근거는 부족하다.** 현재 형태의 제품 통합은 권하지 않는다.

공개 저장소 네 곳의 참고 연결 쌍 23개 중 **14개에서 조건부 후보**, 9개에서 미확정 결과를 얻었다. 구현 보완에 사용한 사례는 13/15, 별도 평가 사례는 1/8이었다. 함수 본문 요약을 호출 지점에 대입하는 처리를 추가해도 포착한 참고 쌍은 같았다. 지정한 오연결 대조 쌍 8개에서는 교차 연결이 없었다. 이 수치는 저장소 전체의 재현율·정밀도나 실제 이벤트 전달 성공률이 아니다.

## 질문과 작업 범위

검증 질문은 객체·키·콜백의 저장, 조회, 호출을 공통 사실로 추출하면 특정 이벤트 라이브러리 이름을 계속 추가하지 않고도 탐색 연결을 얻을 수 있는가였다. 인계 문서의 가설을 확인하기 위한 독립 실행 PoC이며, 기존 `events`나 `flow` 코드를 가져오거나 제품의 탐색 경로에 연결하지 않았다.

사용자 요청에 따라 대상 파일 분석은 Sol·medium 하위 에이전트가 `rg`와 원문 읽기로 수행했다. 설계, 구현, 실행, 결과 종합은 메인 스레드에서 수행했다. 이번 PoC의 파일 분석과 비교 실행에는 codemap-search 도구·CLI를 사용하지 않았다. 공개 대상의 애플리케이션, 서버, 데이터베이스도 실행하지 않았다.

## 소스와 대조 자료

| 저장소 | 고정 커밋 | 입력 파일 | 소스 바이트 |
| --- | --- | ---: | ---: |
| [LiveStore](https://github.com/livestorejs/livestore) | `287936d11d3e6c7afbaacaf1ece14bf162120219` | 6 | 91,231 |
| [Vendure](https://github.com/vendurehq/vendure) | `f78f402f1f706e42ab8e3b47698245846e958081` | 13 | 267,849 |
| [client-go](https://github.com/kubernetes/client-go) | `30803019f93fc7d7fccd93d115927705bf712be7` | 12 | 270,890 |
| [Bevy](https://github.com/bevyengine/bevy) | `29fe519f32a14503c8c0fab0baacafe76842bb76` | 21 | 684,617 |
| 합계 | | **52** | **1,314,587** |

전체 저장소 대신 분석자가 선정한 등록자, 저장소 구현, 호출자·소비자 파일을 입력했다. 정확한 목록은 [corpus.json](corpus.json)에 있다. 두 비교 모드는 동일 파일·동일 바이트·동일 코드로 실행됐으며, 각 원시 결과에 소스 SHA-256이 들어 있다.

[cases.json](cases.json)의 참고 쌍은 소스를 대조해 정한 위치 관계다. 콜백 경로 19개, 이벤트가 아닌 콜백 1개, 데이터 전달 경로 3개를 포함한다. 별도로 다른 Map·필드·레지스트리를 연결하면 안 되는 대조 쌍 8개를 두었다. 분석기는 이 파일을 읽지 않는다. 평가기는 저장된 결과의 저장 위치와 호출 위치가 참고 쌍의 범위에 모두 속하는지 확인한다.

추가 평가 8개는 개발용 실행 결과에 맞춰 구현을 조정하는 데 사용하지 않았다. 다만 사례의 소스 분석 요약은 구현 전에 알고 있었으므로 완전한 블라인드 검증은 아니다. 수동으로 고른 소규모 사례이며, 저장소 전체의 정답 목록도 아니다. 특히 일부 참고 쌍은 가까운 저장·호출 구간이고 다른 쌍은 여러 중간 계층을 포함하므로 난도가 같지 않다.

## 구현과 비교 방법

| 파일 | 역할 |
| --- | --- |
| [syntax.py](syntax.py) | TS/JS·Rust·Go 구문, 선언·타입·소스 위치 추출 |
| [engine.py](engine.py) | 별칭, 필드, 컨테이너, 함수별 사실과 제한된 호출 지점 대입 |
| [model.py](model.py) | 언어 공통 값·저장 사실, 조건부 관계 연결 |
| [analyze.py](analyze.py) | 분석 실행, 소스 해시·비용·원시 결과 기록 |
| [prepare.py](prepare.py), [run.py](run.py) | 고정 소스 준비와 저장소별 별도 프로세스 실행 |
| [measure.py](measure.py), [evaluate.py](evaluate.py) | 코드 고정, 두 모드 순차 측정, 측정 완료 후 일괄 대조 |

Tree-sitter와 세 언어 문법만 외부 의존성으로 사용했다. 버전은 [requirements.txt](requirements.txt)에 고정했다. Map·Set·배열·Go의 map/append처럼 언어·표준 컨테이너의 기본 동작은 모델링한다. `EventBus`, `subscribeToRefresh`, Bevy의 observer처럼 개별 제품이나 이벤트 프레임워크의 API 이름에 연결 규칙을 추가하지 않았다.

`local` 모드는 함수별 구문 사실을 추출한 뒤 공유 필드·컨테이너의 저장과 호출을 연결한다. 따라서 서로 다른 함수의 같은 필드를 연결하는 처리는 이 모드에도 있다. 호출한 함수의 본문 요약을 호출 지점에 대입하지 않는다는 뜻이다.

`summaries` 모드는 같은 처리에 함수 요약 계산을 최대 4회 반복한다. 호출 인자·수신자를 대입하고, 명시적인 객체 필드를 저장된 별칭에 투영한다. 직접 팩터리의 할당 지점에는 호출 위치를 추가한다. 더 깊은 할당 문맥과 반환된 클로저의 캡처 환경을 증명하지 못하면 미확정으로 남긴다.

결과의 `conditional_source_relation`은 실행 보장이 아니다. 같은 인스턴스, 동적 키의 일치, 분기·순서·삭제 등의 조건을 함께 표시한다. 타입이 같은 수신자를 하나의 실제 객체로 확정하지 않는다. 저장된 객체의 메서드를 호출하는 후보도 직접 저장 함수 호출과 구분한다. **모든 결과의 `event_classification`은 `not_inferred`**이므로 일반 콜백 자료구조를 이벤트버스로 자동 분류하지 않는다.

최종 비교는 `local` 전체 네 저장소, `summaries` 전체 네 저장소 순서로 각 한 번 실행했다. 모든 실행이 끝난 뒤 두 결과를 대조했다. [freeze.json](results/freeze.json)에 코드·입력 목록·대조 자료의 해시가 있고, 측정 도중 변경이 없었음을 실행기가 확인했다.

## 연결 포착 결과

| 저장소 | 개발용 후보 / 참고 쌍 | 추가 평가 후보 / 참고 쌍 | 미확정 합계 | 오연결 / 대조 쌍 |
| --- | ---: | ---: | ---: | ---: |
| LiveStore | 5 / 5 | 0 / 2 | 2 | 0 / 4 |
| Vendure | 3 / 3 | 0 / 2 | 2 | 0 / 2 |
| client-go | 5 / 5 | 1 / 2 | 1 | 0 / 2 |
| Bevy | 0 / 2 | 0 / 2 | 4 | 대조 쌍 없음 |
| 합계 | **13 / 15** | **1 / 8** | **9** | **0 / 8** |

두 모드의 결과가 이 표와 같았다. 연결 쌍별 상태와 조건은 [local 대조 결과](results/local-evaluation.json), [summaries 대조 결과](results/summaries-evaluation.json)에 있다. 오연결 0건은 지정한 8개 쌍에 한정된다. 출력된 모든 후보의 정밀도를 검수한 결과는 아니다.

포착한 대표적인 소스 구간은 다음과 같다.

- LiveStore: `refreshCallbacks.add(cb)`에서 같은 인스턴스 Set을 순회하는 `cb()`까지 연결했다. 구독 해지 함수 Map과 nonce별 `resolve/reject` 저장·호출도 후보로 연결했다. [refresh 호출](https://github.com/livestorejs/livestore/blob/287936d11d3e6c7afbaacaf1ece14bf162120219/packages/@livestore/livestore/src/reactive.ts#L518), [등록·삭제](https://github.com/livestorejs/livestore/blob/287936d11d3e6c7afbaacaf1ece14bf162120219/packages/@livestore/livestore/src/reactive.ts#L576).
- Vendure: blocking handler의 옵션 저장과 `options.handler(event)`, Job별 progress 배열, queueName별 process 함수 저장·호출을 포착했다. 동일 객체와 동적 키 일치가 필요한 후보이며 NestJS의 DI나 Bull worker의 실행까지 증명한 결과는 아니다. [blocking 저장](https://github.com/vendurehq/vendure/blob/f78f402f1f706e42ab8e3b47698245846e958081/packages/core/src/event-bus/event-bus.ts#L186), [호출](https://github.com/vendurehq/vendure/blob/f78f402f1f706e42ab8e3b47698245846e958081/packages/core/src/event-bus/event-bus.ts#L215).
- client-go: 실제 caller의 `ResourceEventHandlerFuncs` 함수 필드와 adapter 내부 호출을 다른 파일 사이에서 연결했다. 추가 평가에서는 `ListWatch`의 함수 필드도 포착했다. `Indexers` 역시 콜백 형태를 가지지만 이벤트가 아닌 자료구조다. [필드 저장](https://github.com/kubernetes/client-go/blob/30803019f93fc7d7fccd93d115927705bf712be7/examples/workqueue/main.go#L187), [adapter 호출](https://github.com/kubernetes/client-go/blob/30803019f93fc7d7fccd93d115927705bf712be7/tools/cache/controller.go#L414).

client-go의 두 경로는 구분해야 한다. workqueue 예제의 `NewIndexerInformer`는 `Config.Process → Queue.Pop → processDeltas → handler` 경로를 사용한다. shared informer의 `processorListener`·채널 경로는 별개이며 leasecandidate가 실제 등록자다. 이번 포착 수치가 이 두 전체 경로를 모두 복원했다는 뜻은 아니다.

## 미확정으로 남은 구간

| 구간 | 이번 구현에서 남은 경계 |
| --- | --- |
| LiveStore Webmesh, RPC | Map에서 읽은 값이 함수 자체가 아니라 Queue나 clientId이고, 외부 호출의 인자로 전달된다. 현재 연결기는 이 데이터 흐름을 수신까지 이어 주지 못한다. |
| Vendure telemetry hook | class/method 키, 별도 서비스 객체, decorator와 외부 instrumentation callback을 거친다. 소스 요약만으로 구체 레지스트리 인스턴스를 연결하지 못했다. |
| Vendure dashboard Set | 전역 registry를 거쳐 Set을 회수하며 `globalThis`와 bundle 경계가 있다. 해당 객체 동일성을 해결하지 못했다. |
| client-go FIFO Process | Config에 저장한 closure가 controller 필드, 인터페이스, 형변환, Pop 인자를 거쳐 호출된다. 가까운 함수 필드 사례의 성공이 이 경로로 이어지지 않았다. |
| Bevy observer·SystemId·glTF | World/Entity 저장소, 타입 소거, boxed system, query, 공유 Arc와 객체 복제 경계가 필요하다. 이름이나 타입만으로 연결하지 않아 참고 쌍 네 개 모두 미확정으로 남았다. |
| Bevy 메시지 버퍼 | 저장된 콜백을 호출하는 구조가 아니라 World-local resource와 reader cursor를 통해 데이터를 소비한다. 현재 콜백 연결 표현만으로는 충분하지 않다. |

위 구간은 소스에서 관련 흐름을 확인했지만 분석기가 자동으로 연결하지 못한 사례다. 연결이 실제로 없다는 판정이 아니다. source/model/candidate를 최종 제품에 어떻게 표시할지는 이 PoC에서 확정하지 않았다.

## 비용과 해석

단위는 밀리초와 MiB다. 아래 RSS는 **결과 JSON 직렬화 전까지 기록된 프로세스 최대 RSS**이며 전체 프로세스의 최종 최대 메모리로 해석하면 안 된다. 분석 시간은 파일 읽기·파싱·사실 추출·연결을 포함하고, Python 시작과 결과 직렬화는 제외한다.

| 저장소 | local 분석 ms | summaries 분석 ms | local RSS MiB | summaries RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| LiveStore | 46.65 | 107.42 | 30.56 | 33.73 |
| Vendure | 104.25 | 405.64 | 37.77 | 50.00 |
| client-go | 101.33 | 442.19 | 37.31 | 44.92 |
| Bevy | 214.02 | 553.13 | 56.69 | 64.22 |

| 합계 지표 | local | summaries |
| --- | ---: | ---: |
| 분석 시간 | 0.466초 | 1.508초 |
| 시작·직렬화 포함 자식 프로세스 경과 시간 | 0.792초 | 1.957초 |
| 추출 사실 | 9,356 | 22,186 |
| 전체 출력 후보 | 314 | 478 |
| 관계 JSON 바이트 | 357,904 | 622,833 |
| 참고 쌍에서 포착한 조건부 후보 | 14 / 23 | 14 / 23 |

원시 지표: [local](results/local/metrics.json), [summaries](results/summaries/metrics.json). 상세 사실·소스 해시·조건은 각 디렉터리의 저장소별 `.json.gz`에 있다. 압축은 보관 방식이며 관계 출력량 측정은 압축 전 UTF-8 바이트다.

이번 표본에서 요약 확장은 사실과 출력 후보를 늘렸지만 참고 쌍 포착 수는 늘리지 못했다. 이것이 모든 함수 요약 기법이 불필요하다는 뜻은 아니다. 이번의 제한된 객체·타입·문맥 표현과 반복 방식에 대해 추가 비용에 상응하는 효과를 확인하지 못했다는 결론이다. 순차 실행 한 번의 측정이며 캐시·실행 순서·분산을 통제한 성능 벤치마크도 아니다.

## 분석 한도와 미검증 사항

입력 한도는 파일당 512 KiB, 전체 4,096파일·64 MiB다. 함수별 사실은 192개, 전체 사실은 40,000개, 관계 출력은 2,048개로 제한했다. 이번 입력에서 파일·전체 입력·전체 사실·관계 출력 한도에 걸린 항목은 없었다. 반면 함수별 사실 한도는 local에서 1개 함수, summaries에서 8개 함수에 적용됐다.

summaries 결과에는 할당 문맥 깊이 제한 88건, 반환 closure 환경 미확정 7건, 지원하지 않는 Rust binding pattern 종류 3개가 기록됐다. 네 저장소 모두 반복 횟수 한도에 도달했으므로 고정점 수렴을 보장하지 않는다. 조건·삭제·여러 쓰기 표시는 보수적인 근거 목록이며, 활성 등록 집합이나 정확한 덮어쓰기·실행 순서를 계산하지 않는다.

일반적인 alias 해석, 생성자 본문과 캡처 환경의 완전한 인스턴스 추적, Rust generic·trait·매크로 전개, Go 인터페이스의 모든 구현 선택, 동적 DI·팩터리는 지원 범위 밖이거나 부분적이다. 구문 분석은 실제 빌드 설정을 적용하지 않으므로 Rust 파일 안의 조건부 컴파일·테스트 구획도 포함될 수 있다.

실행 관측, 실제 이벤트 전달 순서, 이름 변경에 대한 별도 변형 실험, 전체 후보 정밀도, 에이전트 추가 조회량·토큰 사용량은 측정하지 않았다. 기존 codemap-search와의 수치 비교도 사용자 제한에 따라 실행하지 않았다. 따라서 도구 호출 감소나 기존 제품 대비 성능 향상을 주장할 수 없다.

## 기존 구현과의 관계 및 판단

기존 구현의 [내장 이벤트 규칙](../../../../src/events/rules.rs)은 Node EventEmitter와 Tauri 중심이며, [입력 언어 경계](../../../../src/events/mod.rs)에는 Go 이벤트 추출이 없다. [JavaScript 해석](../../../../src/events/javascript.rs)은 일반 인스턴스 멤버와 불투명한 객체 전달을 제한한다. 이는 소스를 읽어 확인한 비교 기준이며 기존 바이너리의 실측 결과는 아니다.

이번 PoC는 이벤트 API 목록 없이도 일부 저장·호출 구간을 찾는 공통 기반의 가능성을 보여 준다. 제품에 적용하려면 먼저 **구체 객체·캡처 환경과 호출 인자 전달을 근거로 연결하고, 근거가 부족한 경계를 명확하게 남기는 모델**이 필요하다. 이 부분이 해결되지 않은 상태에서 요약 반복 횟수나 입력 한도만 높이는 방향은 이번 결과로 뒷받침되지 않는다.

후속 검증을 한다면 현재 미확정 사례 중 `Config.Process → Pop`의 함수 인자 전달과 전역 registry를 통한 객체 회수를 우선 분리해 검증하는 것이 타당하다. Bevy의 World/Entity·타입 소거 경계는 별도 난도로 관리해야 한다. 이것은 이번 결과에 근거한 후속 제안이며, 제품 코드에 반영한 변경은 아니다.

## 재현과 실제 검증 기록

실행 환경은 macOS arm64, Python 3.14.5, Git 2.50.1이다. 다른 Python 버전·운영체제에서 실행하지 않았다. 작업 디렉터리는 이 파일이 있는 `experiments/event-navigation-poc`다.

```sh
python3.14 -m venv .venv
.venv/bin/python -m pip install -r requirements.txt
.venv/bin/python prepare.py --sources /tmp/codemap-event-poc-sources
.venv/bin/python measure.py --sources /tmp/codemap-event-poc-sources --output /tmp/codemap-event-poc-results
```

`prepare.py`는 고정 커밋을 별도 디렉터리에 준비한다. 이미 존재하는 경로의 버전이 다르면 덮어쓰지 않고 중단한다. `run.py`는 분석 전에 커밋과 추적 파일의 변경 여부를 확인한다. 대상의 의존성을 설치하거나 프로그램을 실행하는 단계는 없다.

이번에 실제로 실행해 통과한 검증은 Python 파일 8개의 `py_compile`, 네 저장소×두 모드의 독립 분석, 동일 소스·코드 해시 대조, 참고 쌍 31개의 결과 대조다. 대조 절차가 끝났다는 의미와 모든 긍정 참고 쌍을 포착했다는 의미는 구분해야 한다. 고정 커밋의 신규 복제는 초기 `git clone`으로 수행했으며 `prepare.py`의 새 디렉터리 다운로드 분기는 별도로 실행하지 않았다.

초기 개발 실행에서는 Tree-sitter 0.26.0을 사용하는 프로세스가 `SIGSEGV`로 종료됐다. 소스 위치를 `Point.column` 대신 원문 바이트 오프셋으로 계산한 뒤 같은 입력과 최종 비교가 통과했다. [공식 이슈 #487](https://github.com/tree-sitter/py-tree-sitter/issues/487)과 유사한 증상이나 동일 원인인지는 확정하지 않았다. 실패한 개발 실행은 최종 측정 수치에 포함하지 않았다.

Rust 제품 코드·기존 테스트·설정·인계 문서는 수정하지 않았다. 이 PoC 결과는 `grep/read/search`의 기존 출력이나 `Value relationships` 제외 결정을 변경하지 않는다.
