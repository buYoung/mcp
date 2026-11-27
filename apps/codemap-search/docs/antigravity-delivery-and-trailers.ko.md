# Antigravity 전달 상한 및 소스 트레일러

Antigravity CLI(`antigravity-client`)의 약 10 KiB MCP 응답 절단 한계에 대응하기 위한 연결 단위 전달 예산 제어와, `read(view="source")` 트레일러 보강 내용·실측 결과·후속 개선 과제를 기록한다.

## 1. 배경

- `antigravity-client`는 약 10 KiB를 넘는 MCP 도구 응답의 중간 구간을 강제로 생략한다.
- 구현 관계 트레일러와 대용량 `read(ranges)` 응답이 늘어나면서 클라이언트 측 중간 절단 비율이 `16.5%`(`cm-1.0.0`)에서 `30.1%`(`cm-wave3`)까지 상승했고, `read(ranges)` 중간 원문이 통째로 유실되는 문제가 발생했다.
- 사후 바이트 분할(Generic pagination) 방식은 호출 수 증가와 트레일러·`## Other matches`의 후속 페이지 고립을 유발해 오히려 정확도가 하락(`30/36`)했다.

## 2. 구현 요약

비-Antigravity 클라이언트(예: Codex, Claude Code, 일반 CLI)에는 세션 전달 상한을 걸지 않으며 기존 동작을 유지한다.

### 연결 단위 전달 상한 (`antigravity-client`)

- **세션 감지 및 상한 고정** (`src/config.rs`, `src/mcp/mod.rs`): MCP `initialize`에서 클라이언트 이름이 `antigravity-client`이면 연결 수명 동안 `9,900 B` 세션 전달 상한을 고정하고, 마지막 안전망(`trim_to_session_cap`)으로 10 KiB 초과를 차단한다.
- **`read` / `read(ranges)`** (`src/tools/read.rs`, `src/mcp/read_ranges.rs`, `src/tools/live_symbols.rs`):
  - `view="source"`에서 고정 reserve 대신 실제 측정한 트레일러 크기만큼만 말미 원문 행을 동적으로 잘라(`DynClip`) 빈 공간을 줄인다.
  - `read(ranges)`는 첫 번째 범위를 보호하고, 남은 공간이 `2,400 B` 이상이면 후속 범위를 부분 클립해 채운 뒤 미포함 범위의 정확한 좌표(`path:start-end`)를 안내한다.
- **`grep`** (`src/tools/grep.rs`, `src/tools/grep/expansion.rs`):
  - `source_grouped`/`full`에서 실제로 생략되는 파일 경로 접두사를 바이트 계산에서 제외해 조기 절단을 방지한다.
  - 세션 상한 활성 상태에서 `grep_max_columns == 0`이면 행당 `600 B` 상한을 적용해 단일 생성 JSON/OpenAPI 행(예: 14 KB)이 전체 결과를 막는 현상을 차단한다.
- **`search` / `find`** (`src/tools/search/compact.rs`, `src/tools/search/display.rs`, `src/tools/search/mod.rs`, `src/tools/find.rs`):
  - 세션 상한 아래에서 `search`는 단일 호출에 컴팩트 상세 스니펫 + 밀려난 파일의 `## Other matches` 행 목록 + 잔여 `connected_locations`를 함께 담는다.
  - `find`는 세션 상한 내에서 경로 목록을 바이트 제한한다.

### `read(view="source")` 트레일러 보강

- **Track 1A (`src/tools/live_symbols/shared_keys.rs`)**: 문자열 리터럴 키 수집 대상을 호출 인자(`(`, `,`) 외에 대입·비교(`=`, `:=`, `==`, `===`, `!=`, `=>`), 맵/구조체 엔트리(`:`), `case "..."` 분기까지 일반화하고 `WINDOW_KEY_LIMIT`을 `24 → 40`으로 확대했다.
- **Track 1B (`src/tools/live_symbols.rs`)**: 연결 상한 클라이언트에서 `collect_fair_trailers`로 `outside_callers`·`implementations`·`shared_keys`·`importers` 4종 트레일러에 공정 예산을 배분해 앞선 트레일러가 뒤쪽 트레일러를 굶기지 않게 했다.
- **Track 2 (`src/implementations/render.rs`)**: `ImplementationIndex::source_trailer`에 계약 경유 호출 지점(`called via contract:` / `called at:`)을 추가해 구현 메서드↔인터페이스/trait 계약 호출 위치를 바로 확인할 수 있게 했다.

## 3. 실측 결과

- **측정 환경**: Grafana 고정 커밋(`c0a598f5a49abf2014f67e806075d8ad267c9efb`), `grafana-routes-v6` 3문항(총 36개 채점 항목: Core 21 + Extended 15), 실행 에이전트 `agy` (`gemini-4-neon-medium`), 블라인드 채점기 `pi / openai-codex / gpt-6-astra / high`.
- **인덱싱 규칙**: 기존 준비 인덱스(`prepared/wave3`)의 APFS 복제본을 재사용하고 `codemap-search` 바이너리만 교체해 측정했다.

### 변형별 진화 (`n=1` 비교 및 `cm-opt3` `n=3` 평균)

| Arm | `CC` (/3) | Core (/21) | Ext (/15) | Total (/36) | Diag | `agy_trunc` | Tools | Tokens | 비고 |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| `rg` (`n=1`) | 2/3 | 20 (21,20) | 12 | 32 | 3 | 7.3% | 179 | 444,778 | 기준선 |
| `cm-1.0.0` (`n=1`) | 2/3 | 20 (20,20) | 12 | 32 | 1 | 16.5% | 176 | 417,105 | 기존 릴리스 |
| `cm-wave3` (`n=1`) | 2/3 | 20 (20,20) | 10 | 30 | 3 | 30.1% | 136 | 376,687 | 10 KiB 절단 폭증 |
| `cm-paged` (`n=1`) | 1/3 | 19 (20,19) | 11 | 30 | 1 | 0.0% | 171 | 423,290 | 사후 60% 분할 |
| `cm-opt2` (`n=1`) | 1/3 | 17 (18,17) | 12 | 29 | 2 | 0.0% | 159 | 373,006 | `grep` 바이트/컬럼·프롬프트 결함 |
| `cm-opt2b` (`n=1`) | 1/3 | 19 (20,19) | 12 | 31 | 2 | 0.0% | 144 | 378,576 | 3중 결함 제거 후 회복 |
| **`cm-opt3` (`n=3` 평균)** | **2.00/3** | **19.67** (`19~21`) | **12.33** (`12~13`) | **32.00** (`31~34`) | **0.67** | **0.0%** | **142.3** | **387,622** | Run 1 `34`, Run 2 `31`, Run 3 `31` |

### `cm-opt3` 3회 반복 측정 요약 (`n=3`, 9슬롯)

- **안정성**: 전체 36개 항목 중 **29개(80.6%)가 3/3 통과**, Run 2~3 전체 419개 인용 중 `invalid=0`, 로그 확인 8슬롯 전체 `agy_trunc = 0/427 (0.0%)`.
- **문항별 평균**:
  - `git-sync-job-dispatch`: `CC 3/3`, Core `7.00/7`, Ext `4.67/5`, Total `11.67/12` (`12, 11, 12`)
  - `alertmanager-config-autosync`: `CC 2/3`, Core `6.67/7`, Ext `4.00/5`, Total `10.67/12` (`11, 10, 11`), `A7` 통과율 `2/3` (이전 6개 암 `1/6`)
  - `scope-annotation-filter`: `CC 1/3`, Core `6.00/7`, Ext `3.67/5`, Total `9.67/12` (`11, 10, 8`)

## 4. 확인된 후속 개선 과제 및 한계

### 도구 측 후속 개선 과제 (미구현 백로그)

1. **트레일러 중복 재스캔 제거 및 `shared_keys` 역인덱스화**:
   - 현재 `live_symbols.rs`의 `collect_fair_trailers`가 예산 초과 시 재조회하고, `DynClip`으로 원문을 자른 뒤 4개 트레일러를 처음부터 다시 조회한다. 동시 부하가 걸린 6-pane 실행에서 `read` 트레일러 타임아웃이 다수 발생했다.
   - `shared_keys.rs`가 매 `read`마다 전체 색인 파일을 알파벳 순(`apps/` → `pkg/` → `public/`)으로 선형 순회하며 `100ms`에서 중단하므로, 대형 모노레포 부하 시 뒤쪽 경로(`public/` TS 코드)에 도달하기 전에 스캔이 끊길 수 있다.
   - 스냅샷 구축 시 리터럴 역인덱스를 1회 생성하고, `collect_fair_trailers`는 1회 수집 후 메모리에서 예산별 슬라이싱하도록 개선하면 타임아웃과 알파벳 절단을 제거할 수 있다.
2. **`shared_keys` 교차 언어·희귀 키 우선 정렬**:
   - 현재 윈도우 등장 순으로 키를 자르기 때문에 동일 언어 UI 문자열이 슬롯을 먼저 차지해 교차 언어 계약 키가 생략 목록으로 밀릴 수 있다. 다른 파일 확장자(교차 언어) 및 등장 파일 수가 적은 키를 우선 배치할 필요가 있다.
3. **`grep(expand=callable)` 트레일러 연계 검토**:
   - 모델이 핵심 분기를 `read(view="source")` 대신 `grep(expand=callable)`로만 확인하는 경우 현재 `shared_keys` 트레일러가 붙지 않는다.

### 구조적·모델 한계

- **인용 범위 누락**: 모델이 원문과 트레일러를 모두 수신하고도 최종 답변에서 인접 행만 인용해 근거 인정이 탈락하는 사례가 확인됐다.
- **전달된 트레일러 미추적**: `shared_keys`가 정확한 교차 언어 소비자 위치(`ImportSettingsPage.tsx:66`)를 전달해도 모델이 후속 `read`를 수행하지 않고 탐색을 종료하는 경우가 있었다.
- **외부 패키지 제네릭 인터페이스**: 워크스페이스 밖 라이브러리 인터페이스(예: `k8s.io/apiserver/pkg/registry/rest.Creater`)를 경유하는 DI 호출은 정적 워크스페이스 `ImplementationIndex` 범위 밖이다.
