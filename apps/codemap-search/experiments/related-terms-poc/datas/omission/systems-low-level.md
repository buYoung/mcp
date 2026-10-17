# Memory, hardware and low-level systems 누락 단어

- 분야 id: `systems-low-level`
- 원본: [../systems-low-level.md](../systems-low-level.md)
- 누락 단어 32개 · 사전에 없음 22 · 뜻 미배정 0 · 다른 분야에만 있음 10
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Bare-metal hardware access (`t697`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `IRQn` | identifier | 사전에 없음 | 인터럽트 번호와 외부 인터럽트 라인을 가리키는 MCU 헤더와 드라이버 코드의 식별자다. |
| `nested interrupts` | phrase | 사전에 없음 | 인터럽트 지연과 우선순위, 중첩을 다루는 베어메탈 코드와 문서에서 쓰는 말이다. |

### Branch prediction hints (`t2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `superscalar` | word | 사전에 없음 | 분기 예측과 파이프라인, 비순차 실행에 관련된 성능 문서와 코드 주석의 용어다. |

### Byte array storage (`t114.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `bytemuck` | identifier | 사전에 없음 | 바이트 배열을 다른 타입으로 안전하게 해석하게 해 주는 러스트 크레이트 이름이다. |
| `bytes::Bytes` | identifier | 사전에 없음 | 바이트 버퍼를 만들고 바이트 단위로 쓰는 표준 라이브러리 타입과 함수 이름이다. |

### C string buffers (`t57.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `strndup` | identifier | 사전에 없음 | 널 종료 문자열의 길이, 복사, 이어 붙이기, 비교, 검색에 쓰는 C 라이브러리 함수 이름이다. |

### Embedded firmware (`t640`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `defmt` | proper_noun | 사전에 없음 | 임베디드 러스트 생태계의 하드웨어 추상화와 레지스터 접근 크레이트 이름이다. |
| `HAL crate` | phrase | 사전에 없음 | 임베디드 러스트 생태계의 하드웨어 추상화와 레지스터 접근 크레이트 이름이다. |
| `IAR` | proper_noun | 사전에 없음 | MCU 펌웨어 개발에 쓰는 SDK, 코드 생성기, IDE, 드라이버 계층 이름이다. |
| `RTIC` | proper_noun | 사전에 없음 | 임베디드 러스트 생태계의 하드웨어 추상화와 레지스터 접근 크레이트 이름이다. |

### Firmware flashing (`t171.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cargo embed` | phrase | 사전에 없음 | 펌웨어를 기록하고 디버깅할 때 쓰는 디버그 프로브와 플래시 도구, 서버 이름이다. |

### Flash storage translation (`t171.5`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `eMMC` | word | 사전에 없음 | 플래시 종류와 수명 관리, 불량 블록 처리, 카드형 저장 장치를 가리키는 말이다. |

### Heap garbage collection (`t199.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Xmx` | identifier | 사전에 없음 | JVM의 수집기 이름과 세대 영역, 힙 크기 옵션 이름이다. |

### Memory limit enforcement (`t234.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `OutOfMemoryError` | identifier | 다른 분야에만 있음: observability-debugging — Heap memory diagnostics | 자바와 Node.js에서 힙 한도 초과를 나타내는 오류와 옵션 이름이다. |

### Native ABI interfaces (`t200.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `JNI` | proper_noun | 다른 분야에만 있음: language-constructs — Foreign interface boundaries | C와 다른 언어를 연결하는 외부 함수 인터페이스 구문과 바인딩 도구 이름이다. |

### No-std static allocation (`t971`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `panic_halt` | identifier | 사전에 없음 | 표준 라이브러리 없이 동작하는 러스트 임베디드 코드의 어트리뷰트와 크레이트 이름이다. |

### Pointers/addresses (`t1026`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `UnsafeMutablePointer` | identifier | 사전에 없음 | 러스트와 스위프트의 널 포인터와 원시 포인터 래퍼 타입 이름이다. |

### RTOS microcontroller firmware (`t171.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `MSP430` | proper_noun | 사전에 없음 | 마이크로컨트롤러와 보드 제품군 이름이다. |

### Serial console interfaces (`t178`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `/dev/ttyUSB0` | identifier | 사전에 없음 | 직렬 포트 장치 파일과 터미널 프로그램 이름이다. |

### SIMD processing (`t22.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `_mm256_add_ps` | identifier | 사전에 없음 | SIMD 인트린식 함수와 헤더, 언어별 SIMD 모듈 이름이다. |
| `core::arch` | identifier | 사전에 없음 | SIMD 인트린식 함수와 헤더, 언어별 SIMD 모듈 이름이다. |
| `NEON` | abbreviation | 다른 분야에만 있음: cloud-infrastructure — Serverless scale-to-zero | x86과 ARM의 SIMD 명령어 확장 이름이다. |

## 새 주제가 필요한 단어

### Clock and reset circuitry

범위: Microcontroller clock sources, prescalers, PLLs, power-on and brown-out reset, and watchdog timers.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `BOD` | abbreviation | 사전에 없음 | 마이크로컨트롤러의 클록원과 분주기, PLL, 전원 리셋, 워치독 타이머를 가리키는 말이다. |
| `PLL` | abbreviation | 다른 분야에만 있음: time-locale — Clock calibration | 마이크로컨트롤러의 클록원과 분주기, PLL, 전원 리셋, 워치독 타이머를 가리키는 말이다. |
| `watchdog timer` | phrase | 다른 분야에만 있음: os-shell — Watchdog resets | 마이크로컨트롤러의 클록원과 분주기, PLL, 전원 리셋, 워치독 타이머를 가리키는 말이다. |

### CPU caches and locality

범위: CPU cache hierarchy, cache coherence, locality of reference and cache-aware data layout.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cache aligned` | phrase | 사전에 없음 | CPU 캐시 계층과 일관성, 참조 지역성, 캐시를 고려한 자료 배치를 설명하는 용어다. |
| `non-temporal store` | phrase | 사전에 없음 | CPU 캐시 계층과 일관성, 참조 지역성, 캐시를 고려한 자료 배치를 설명하는 용어다. |

### Memory ordering and barriers

범위: Memory ordering semantics, memory barriers and fences, and acquire/release atomic orderings.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `memory barrier` | phrase | 다른 분야에만 있음: concurrency-async — Lock-free atomics | 메모리 순서 의미와 배리어, 펜스, 원자 연산 순서 지정자를 가리키는 저수준 용어다. |
| `memory fence` | phrase | 다른 분야에만 있음: concurrency-async — Lock-free atomics | 메모리 순서 의미와 배리어, 펜스, 원자 연산 순서 지정자를 가리키는 저수준 용어다. |
| `memory order` | phrase | 다른 분야에만 있음: concurrency-async — Lock-free atomics | 메모리 순서 의미와 배리어, 펜스, 원자 연산 순서 지정자를 가리키는 저수준 용어다. |
| `memory ordering` | phrase | 다른 분야에만 있음: concurrency-async — Lock-free atomics | 메모리 순서 의미와 배리어, 펜스, 원자 연산 순서 지정자를 가리키는 저수준 용어다. |

### Trusted execution hardware

범위: Hardware security features for trusted execution, such as TPM, TEE, SGX, TrustZone and secure enclaves.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `TPM` | abbreviation | 다른 분야에만 있음: security — Content integrity hashes | 신뢰 실행 환경과 보안 칩 같은 하드웨어 보안 기능을 가리키는 말이다. |
