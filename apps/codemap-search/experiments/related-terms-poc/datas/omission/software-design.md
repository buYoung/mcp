# Software design and architecture 누락 단어

- 분야 id: `software-design`
- 원본: [../software-design.md](../software-design.md)
- 누락 단어 16개 · 사전에 없음 7 · 뜻 미배정 0 · 다른 분야에만 있음 9
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Data access objects (`t646.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `active record` | phrase | 다른 분야에만 있음: databases-storage — ORM repositories | 모델 객체가 영속화를 맡는 데이터 접근 패턴 이름으로 ORM 설계 문서에 쓰입니다. |
| `data mapper` | phrase | 다른 분야에만 있음: databases-storage — Persistence contexts | 도메인 객체와 저장소 스키마를 분리해 매핑하는 데이터 접근 패턴 이름입니다. |
| `unit of work` | phrase | 다른 분야에만 있음: databases-storage — ORM repositories | 변경 내용을 모아 한 번에 저장하는 영속화 패턴으로 저장소 코드와 ORM 문서에 쓰입니다. |

### IoC container wiring (`t43.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `injection-js` | proper_noun | 사전에 없음 | Angular 의존성 주입을 독립시킨 DI 라이브러리 이름입니다. |
| `Swinject` | proper_noun | 사전에 없음 | Swift용 DI 컨테이너 라이브러리 이름입니다. |

### JVM injection frameworks (`t43.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `HK2` | proper_noun | 사전에 없음 | Jersey가 쓰는 JVM 의존성 주입 프레임워크 이름입니다. |
| `Weld` | proper_noun | 사전에 없음 | CDI 참조 구현인 JVM 의존성 주입 프레임워크 이름입니다. |

### Object design patterns (`t100.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `factory pattern` | phrase | 다른 분야에만 있음: language-constructs — Factory methods | 객체 생성을 팩토리에 맡기는 생성 패턴의 일반 이름으로 코드와 문서에 널리 쓰입니다. |
| `memento pattern` | phrase | 사전에 없음 | 객체 상태 저장·복원을 위한 GoF 패턴의 전체 이름입니다. |

### Object relationships (`t646.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `inheritance` | word | 다른 분야에만 있음: language-constructs — Method resolution | UML 클래스 관계와 객체지향 설계 설명에서 핵심으로 쓰이는 말로, 언어 구문 분야에는 있지만 관계 모델링 쪽 뜻이 따로 있습니다. |

### Responsibility boundaries (`t472.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `feature sliced design` | phrase | 사전에 없음 | 프런트엔드 코드를 기능 단위 계층으로 나누는 구조 방법론 이름입니다. |

## 새 주제가 필요한 단어

### Design smells and anti-patterns

범위: Naming recurring structural problems such as god objects, code smells and anti-patterns.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `shotgun surgery` | phrase | 사전에 없음 | 한 변경이 여러 클래스 수정을 요구하는 코드 스멜 이름입니다. |

### Observer event publishing

범위: Notifying subscribed observers of changes through observer pattern and event publishers or dispatchers.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `observer` | word | 다른 분야에만 있음: concurrency-async — Observable stream operators | 상태 변화를 통지받는 객체를 가리키는 관찰자 패턴의 핵심 용어로, 다른 분야는 DOM·스트림 관찰 뜻만 다룹니다. |
| `observer pattern` | phrase | 다른 분야에만 있음: distributed-systems — Loose coupling patterns | 구독 객체에 변화를 통지하는 GoF 행위 패턴 이름으로 이 분야 범위에 명시된 패턴입니다. |

### State machines

범위: Modeling behavior as states and transitions with state machines, statecharts and the state pattern.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `state machine` | phrase | 다른 분야에만 있음: distributed-systems — State machine transitions | 이 분야 범위에 명시된 설계 방식이며 다른 분야는 분산 사가·토큰 처리 맥락만 다룹니다. |
| `XState` | proper_noun | 다른 분야에만 있음: distributed-systems — State machine transitions | JavaScript 상태 기계·상태 차트 라이브러리로 상태 기계 설계 코드에 쓰입니다. |
