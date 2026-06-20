# Starlight Docs Site Structure

이 문서는 codemap-search 문서 웹사이트를 Astro Starlight로 만들 때의 정보 구조 초안이다. 각 문서의 실제 본문, 예시, 수치, 표현 방식은 별도 결정한다. 여기서는 어떤 화면과 섹션을 어떤 순서로 보여줄지만 정의한다.

## 목표

- codemap-search를 단순 도구 목록이 아니라, 에이전트가 코드를 탐색하는 방식으로 설명한다.
- 첫 사용자가 설치 후 성공 여부를 확인할 수 있게 한다.
- 이미 사용하는 사용자가 상황별로 어떤 도구를 선택해야 하는지 빠르게 판단할 수 있게 한다.
- 설정, 한계, 벤치마크를 공개 문서에서 과장 없이 다룰 수 있게 한다.

## 기본 방향

사이트의 중심은 `Tool Reference`가 아니라 `How to Navigate Code`다. codemap-search의 핵심 사용 모델은 `overview -> search -> read/find/grep` 흐름이므로, 사용자는 먼저 "어떤 도구가 있나"보다 "지금 상황에서 무엇을 호출해야 하나"를 이해해야 한다.

벤치마크는 성능 홍보가 아니라 측정 체계와 제품 개선 기록으로 다룬다. 결과를 말할 때는 측정 조건, 과제 세트, 클라이언트, 색인 상태, 한계를 함께 표시한다.

## 권장 사이트 구조

```text
/
├─ Getting Started
│  ├─ Installation
│  ├─ First Successful Run
│  └─ MCP Client Setup
│     ├─ Claude Code
│     ├─ Codex
│     └─ opencode
├─ How to Navigate Code
│  ├─ Orient in a New Repository
│  ├─ Find Symbols and Definitions
│  ├─ Search Concepts
│  ├─ Enumerate Exact Matches
│  └─ Read Large Files Safely
├─ Tool Reference
│  ├─ initial_instructions
│  ├─ overview
│  ├─ search
│  ├─ read
│  ├─ find
│  ├─ grep
│  └─ CLI
├─ Configuration
│  ├─ Files and Precedence
│  ├─ Loader Behavior and Validation
│  ├─ Indexing
│  ├─ Search Output
│  ├─ Tool Output Limits
│  ├─ Caller/Callee Context
│  ├─ Ignore Handling
│  └─ Index Freshness
├─ Benchmarks
│  ├─ Why Benchmark
│  ├─ Methodology
│  ├─ Metrics
│  ├─ Results
│  ├─ What Changed Because of Benchmarks
│  ├─ Limits
│  └─ Reproduction Notes
├─ Limits and Troubleshooting
│  ├─ Supported Languages
│  ├─ Large Files
│  ├─ Index Freshness
│  ├─ Large Output
│  ├─ Missing Files
│  ├─ MCP Tool Approval
│  ├─ String Literal Search
│  ├─ Caller/Callee Accuracy
│  ├─ Concurrency and Process Model
│  └─ Logging and Diagnostics
└─ Development
   ├─ Repository Structure
   ├─ Architecture
   ├─ Local Development
   └─ Contributing
```

## Section Notes

Section Notes는 트리의 모든 노드를 1:1로 풀어 쓰지 않고, 구성 원칙 설명이 필요한 핵심 묶음만 다룬다. 별도 노트가 없는 노드(`Installation`, `First Successful Run` 등)는 상위 섹션 노트의 흐름을 따른다. 트리 루트 `/`는 아래 `Home` 노트에 대응한다.

### Home (/)

홈은 긴 설명보다 진입점을 제공한다.

- 한 문장 가치 제안
- `overview -> search -> read/find/grep` 탐색 흐름
- `Getting Started`, `How to Navigate Code`, `Tool Reference`, `Benchmarks`로 가는 주요 링크

### Getting Started

처음 설치하는 사용자를 위한 영역이다.

- 설치 방법
- 첫 실행
- `initial_instructions` 호출
- 정상 동작 확인 체크리스트
- MCP 클라이언트 등록으로 이어지는 흐름

### MCP Client Setup

클라이언트별 등록 예시는 탭으로 보여준다.

- Claude Code
- Codex
- opencode

공통 원칙은 별도로 강조한다. codemap-search는 서버가 실행되는 현재 작업 디렉터리를 기준으로 저장소를 읽고 색인한다.

### How to Navigate Code

사이트의 핵심 섹션이다. 도구별 설명이 아니라 사용자의 탐색 상황별로 구성한다.

- 처음 보는 저장소에서 구조를 파악하기
- 심볼, 함수, 타입 정의 찾기
- 정확한 이름을 모를 때 개념으로 검색하기
- 문자열, 오류 메시지, 설정값을 전수 조사하기
- 큰 파일을 필요한 범위만 읽기

각 문서는 "상황 -> 추천 도구 -> 예시 흐름 -> 다음 확인 단계"의 형식을 따른다.

### Tool Reference

도구별 상세 설명 영역이다. 각 페이지는 같은 형식을 유지한다.

- 언제 쓰는가
- 쓰지 말아야 할 때
- 주요 인자
- 예시
- 자주 나오는 다음 단계
- 관련 설정

MCP 도구 외에 CLI 페이지를 둔다. `codemap-search` 바이너리의 서브커맨드(`mcp`, `parse`, `tokenize`, `codemap`, `search`, `index`, `benchmark`)를 다룬다.

### Configuration

기존 `configuration.md`의 내용을 사이트 문서로 옮기되, 단순 키 나열보다 사용 목적별로 묶는다.

- 설정 파일 위치와 우선순위
- 로더 동작과 검증 (never-exit 동작, 자동 생성 템플릿, 값 검증)
- 색인 관련 설정
- 검색 출력 관련 설정
- 도구 출력 제한
- caller/callee 주석
- ignore 처리
- 색인 갱신 설정 (`watch`, `watch_debounce_ms`, `index_staleness_ms`, `indexer_auto_restart`)

`configuration.md`의 전체 키 표와 예시 `config.toml`, `.codemap/` 디렉터리 설명은 위 묶음 페이지 안에 본문·표·예시로 녹여 싣는다(별도 트리 노드로 두지 않는다).

색인 갱신은 여기서 설정 키 관점으로 다룬다. 같은 주제를 증상과 대응 관점으로 다루는 `Limits and Troubleshooting > Index Freshness`와 역할을 구분한다.

### Benchmarks

벤치마크는 수치표만 제공하지 않는다. 측정이 어떤 제품 판단으로 이어졌는지를 함께 보여준다.

- 왜 측정했는가
- 어떤 방법론을 썼는가
- 어떤 메트릭을 보았는가
- 어떤 결과를 관측했는가
- 어떤 제품 변경이 벤치마크에서 나왔는가
- 무엇을 아직 주장하지 않는가
- 어떤 조건에서만 재현 가능한가

주의할 표현:

- "가장 빠르다"라고 단정하지 않는다.
- "정확도가 더 높다"라고 일반화하지 않는다.
- "내장 Read/Grep보다 낫다"라고 주장하지 않는다.
- "재현 가능하다"를 조건 없이 말하지 않는다.
- "벤치마크가 개선을 증명했다"보다 "수정 전후 같은 조건에서 변화를 관측했다"처럼 쓴다.

권장 표현:

- "이 저장소, 이 과제 세트, 이 설정에서 관측했다."
- "병목 식별과 회귀 감지에 사용했다."
- "수정 전후 같은 조건에서 변화를 비교했다."
- "하드웨어, 캐시, 색인 상태, 클라이언트 버전에 따라 결과가 달라질 수 있다."

### Limits and Troubleshooting

한계와 문제 해결은 분리하지 않고 한 축에 둔다. 사용자가 실패 상황에서 바로 원인과 다음 행동을 찾게 하는 것이 목적이다.

- 지원 언어 범위
- 큰 파일 처리
- 색인 신선도
- 결과가 너무 클 때
- 파일이 보이지 않을 때
- MCP 클라이언트에서 도구 호출이 취소될 때
- 문자열 리터럴 검색 (BM25 색인 비대상, `grep` 사용)
- caller/callee 주석의 근사성
- 동시성과 프로세스 모델 (단일 클라이언트 순차 stdio, 교차 프로세스 색인 잠금 없음)
- 로깅과 진단 (stderr 전용, `RUST_LOG`로 조정)

### Development

초기 공개 사이트에서는 뒤쪽에 둔다. 사용자는 먼저 설치, 사용 흐름, 도구 선택을 봐야 한다.

- 저장소 구조
- 아키텍처 개요
- 로컬 개발
- 기여 전 확인할 것

## Starlight Components

Starlight 구성 요소는 다음 용도로 사용한다.

- `Tabs`: MCP 클라이언트별 등록 예시
- `Steps`: 설치, 첫 성공 확인, 벤치마크 재현 절차
- `Aside`: 도구 선택 규칙, 주의 사항, 측정 한계
- 카드형 링크: 홈의 주요 진입점
- 코드 블록 제목: 명령, 설정 파일, 출력 예시 구분

## Open Decisions

다음 항목은 이 문서에서 결정하지 않는다.

- 각 페이지의 실제 본문
- 홈의 최종 카피
- 벤치마크에서 공개할 구체 수치
- 실제 사이트 경로와 배포 위치
- Starlight 프로젝트를 둘 디렉터리
- 한국어/영어 문서 우선순위
