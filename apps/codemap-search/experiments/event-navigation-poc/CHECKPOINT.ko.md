# 이벤트 연결 PoC 체크포인트

기록일: 2026-09-16. 저장점 이름은 `checkpoint/event-navigation-poc-18-languages-20260916`이다. 사용자가 현재 상태의 체크포인트와 남은 작업 정리를 요청한 시점의 기록이다. 아래 후속 우선순위는 제안이며, 이 저장점에서는 추가 구현을 진행하지 않았다.

## 저장한 상태

18개 개발 언어를 지원하는 독립 Python PoC의 코드, 고정 사례, 입력 잠금, 의존성 버전, 원시 분석 결과, 이전 구현·결과 이력을 보존했다. [인계 문서](../../docs/event-navigation-handoff.ko.md)도 함께 저장했다. 제품 코드 기준은 부모 커밋 `b6416baf3d6e28e58badb460152b8b46e314c441`이며, 이번 PoC는 Rust 제품의 MCP/CLI 계약에 통합하지 않았다.

| 확인 항목 | 체크포인트 상태 |
| --- | --- |
| 기존 탐지·미탐지 예시 | 원문 36개 유지, 모두 최종 호출까지 조건부 탐지 |
| 새 회귀 사례 | 111개 통과: 양성 67, 구조적 반례 40, 미확정 유지 4 |
| 기존 공통 사례 | 양성 36·반례 54, 총 90개 통과 |
| 기존 전체 사례 | 공개 77·공통 90, 총 167개 판정 변화 없음 |
| 공개 양성 37개 | 조건부 호출 후보 23, 인자 전달만 2, 미확정 12 |
| 공개 무연결 대조 14개 | 양성 대조와 함께 확인 12, 판정 유보 2 |
| 공개 실행 경계 시나리오 26개 | 소스 근거 확보, 실행 검증 안 함 |
| 저장 직전 무결성 확인 | 현재 분석 결과 99개와 코드·입력 해시 일치, 원본 예시 36개·이전 보존본 74파일 유지 |

이번 저장 직전에는 분석기 전체를 다시 실행하지 않고, 기존 결과의 코드·소스·커밋 해시와 Python 구현 구문을 대조했다. 실제 측정 결과와 검증 한계는 [전체 보고서](language-corpus/README.ko.md), [구문 보완 보고서](language-corpus/regressions/README.ko.md), [무결성 기록](language-corpus/regressions/results/verification.json)에 있다.

가상환경, Python 캐시, 생성된 Groovy 공유 라이브러리는 Git 제외 규칙을 따른다. `/tmp`에 준비한 공개 저장소 원문은 Git에 복제해 넣지 않았다. 커밋·소스 해시는 [repositories.json](language-corpus/repositories.json)과 [sources.lock.json](language-corpus/sources.lock.json)에 고정되어 있으며 `manage.py prepare`로 다시 준비할 수 있다. 검증 환경은 Python 3.14.5·macOS arm64다.

## 남은 작업

공개 저장소에서 끝까지 연결하지 못한 양성 12개가 우선적인 개선 대상이다.

| 대상 | 미확정 수 | 남은 연결 경계 |
| --- | ---: | --- |
| C++ / EnTT | 1 | delegate·템플릿·포인터 별칭에서 저장 목록과 호출까지 |
| C# / Reactive | 1 | CAS로 교체하는 배열과 observer wrapper |
| Groovy / Grails | 1 | Closure wrapper에서 별도 trigger 객체로 전달 |
| Java / EventBus | 1 | reflection의 Method와 수신 객체 결합 |
| PHP / EventDispatcher | 1 | priority 배열·참조 cache·lazy callable |
| Scala / Monix | 1 | immutable State·CAS·cache·Future/Ack |
| Swift / RxSwift | 1 | generic Bag·bound method·반환 snapshot |
| Rust / Bevy | 4 | observer·메시지 버퍼·SystemId·trait/clone 경로 |
| TypeScript / Vendure | 1 | DI 서비스와 target 객체의 동일성 |

그 밖의 개선·검증 범위는 다음과 같다.

1. **구문·의미 해석 확장:** 상속 getter, descriptor/proxy, 동적 키, 다양한 lambda 캡처, 일반 메서드 참조·reflection·trait/wrapper, named tuple/record, C# 사용자 event accessor, Assembly 명령·제어 흐름의 지원 범위를 넓힌다. 일부 공개 파일의 파싱 복구·누락 토큰과 분석 한도도 남아 있다.
2. **독립 평가:** 구현하면서 보지 않은 저장소·예시로 정밀도와 재현율을 측정하고, 이름 변경·동일 타입의 다른 객체·서로 다른 키에 대한 강건성을 확인한다. 현재 통과 수치가 저장소 전체 정확도를 뜻하지는 않는다.
3. **실행 의미 검증:** 인스턴스 동일성, 등록·해제 순서, weakref/GC 수명, 복사와 공유, 비동기 전달을 확인한다. 공개 반례 중 Groovy·Java 2개는 양성 연결이 확보되어야 판정할 수 있고, 실행 경계 26개는 실제 실행과 분리해 관리한다. 대상 프로그램 실행을 포함할지는 후속 작업에서 범위를 결정해야 한다.
4. **제품 반영 검토:** PoC 근거가 충분해진 뒤 Rust 구현, 인덱스 저장·갱신, MCP/CLI 출력, 출력 예산과 성능을 검토한다. 제품 통합은 이 체크포인트의 완료 범위에 포함하지 않았다.

다음 구현은 공개 미확정 12개를 작은 재현 사례로 나누고, 저장값이 wrapper·반환값·별칭을 통과하는 공통 흐름부터 보완하는 순서를 권장한다. 기존 반례와 조건부 판정을 유지하면서 실제 저장소에서 개선됐는지 함께 확인해야 한다.

## 다시 확인하는 방법

저장점은 별도 작업 디렉터리에서 열 수 있다. 다음은 저장소 내부에서 실행하는 예다.

```sh
git worktree add --detach /tmp/codemap-event-poc-checkpoint checkpoint/event-navigation-poc-18-languages-20260916
```

그 경로의 `apps/codemap-search/experiments/event-navigation-poc/language-corpus`에서 [전체 보고서의 재현 절차](language-corpus/README.ko.md#재현)를 따른다. 원문 준비 후 기본 대조는 `manage.py measure`, 구문 회귀는 `regressions/run.py`로 재실행한다. 결과는 조건부 소스 관계이며 실제 이벤트 전달을 보장하지 않는다.
