# Changelog

codemap-search의 주요 변경 사항을 이 파일에 기록합니다.
형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/)를 따르며,
[유의적 버전](https://semver.org/lang/ko/)을 준수합니다.

## [Unreleased]

## [0.3.0] - 2026-06-30

### 추가
- codemap-search에 Monorepo 감지와 워크스페이스 범위 검색 추가
- MCP 검색 요청에 유효 워크스페이스 범위 제한과 scope 업데이트 동작 추가
- `[update].config_auto_update` 설정으로 config.toml 자동 생성·동기화 제어 추가

### 개선
- 검색 결과와 요약에 워크스페이스 범위 정보 표시 개선
- 워크스페이스 파일 탐색의 필터 처리와 부모-자식 경로 판정 개선
- 툴별 사용법 문서를 독립 Markdown으로 분리하고 config.toml 설명 개선

### 수정
- 저장소 config.toml 동기화가 전역 설정을 덮어쓸 수 있던 문제 수정
- 새 설정 키 동기화 시 기본값을 주석 블록으로만 추가하도록 수정

## [0.2.0] - 2026-06-26

### 추가
- codemap-search에 Tree-sitter 기반 정밀 호출자·호출 대상 추적 추가
- navigation_context_default, navigation_callsite_budget 등 네비게이션 컨텍스트 설정 추가

### 개선
- 검색 쿼리에서 언어·파일 확장자 힌트를 반영한 랭킹과 필터링 개선
- Tree-sitter tags.scm 기반 정의·참조 태깅으로 검색 결과 품질 개선
- codemap-search 초기 안내와 툴 설명 문서 개선
- codemap-search README에 crates.io·로컬 체크아웃 설치 옵션과 Claude Code -s user 등록 안내 개선

## [0.1.6] - 2026-06-22

- 사용자에게 직접 보이는 변경 사항은 없습니다.

## [0.1.5] - 2026-06-22

- 사용자에게 직접 보이는 변경 사항은 없습니다.

## [0.1.4] - 2026-06-21

- 사용자에게 직접 보이는 변경 사항은 없습니다.

## [0.1.3] - 2026-06-21

- 사용자에게 직접 보이는 변경 사항은 없습니다.

## [0.1.2] - 2026-06-21

- 사용자에게 직접 보이는 변경 사항은 없습니다.

## [0.1.1] - 2026-06-21

- 사용자에게 직접 보이는 변경 사항은 없습니다.

## [0.1.0] - 2026-06-21

### 추가
- `codemap-search` MCP 도구 `search`, `overview`, `read`, `grep`, `find`, `initial_instructions` 추가
- BM25 기반 심볼·문서·문자열 리터럴 검색과 매칭 코드 스니펫 추가
- 루트·폴더·파일 단위 코드맵, 파일·심볼 수, 라인 범위 출력 추가
- Rust, Python, TypeScript/JavaScript, Go, Java, Kotlin, C/C++, Assembly 심볼 추출 추가
- 파일 시스템 워처 기반 자동 색인 갱신과 `watch`, `watch_debounce_ms`, `index_staleness_ms` 설정 추가
- `.codemap/config.toml` 자동 생성과 스키마 버전 기반 증분 마이그레이션 추가
- GitHub Release 자산, crates.io, Homebrew, WinGet, POSIX 설치 스크립트 배포 채널 추가

### 개선
- `search` 결과의 호출자·피호출자 문맥, 라인 번호, 매칭 이유, `read <path>:<line>` 제안 개선
- 루트·폴더 코드맵을 디렉터리 중심 접기 출력으로 바꿔 대규모 레포 탐색 개선
- 백그라운드 색인과 `warming up` 상태 알림으로 초기 색인 중 검색 피드백 개선
- `grep` 기본 출력의 `file:line:text` 표시와 웹 번들 파일 노이즈 필터 개선
- Windows·Unix 경로 입력, alias 파라미터, 문자열 숫자 파라미터 처리 개선
- 검색 응답 크기 제한과 caller block 중복 제거로 내비게이션 출력 개선
- Linux x86_64 지원 범위를 Ubuntu 22.04+로 맞추고 musl 정적 바이너리 제공 개선

### 수정
- 서버 instructions와 `search` 도구 설명의 벤치마크 정답 단편을 중립 예시로 교체 수정
- `serverInfo.version`이 Cargo.toml 버전과 동기화되도록 하드코딩된 `0.1.0` 수정
- Windows 릴리스 빌드의 셸 변수 미확장과 `.sha256` 형식 문제 수정

### 변경
- `get_codemap` 도구 이름을 `overview`로 변경
- `register_git_exclude`, `respect_git_exclude` 설정을 `use_git_ignore`로 통합 변경
- 제외 디렉터리 관리를 `.gitignore` 또는 `.git/info/exclude` 직접 편집 방식으로 변경

### 보안
- `.git`, `.codemap` 무조건 제외와 절대 경로·루트 경계 검증 보안 강화
- `find`, `grep`, `read` 권한 정책을 repo-confined 기본값으로 세분화 보안 강화
- POSIX 설치 스크립트의 원자적 설치, 심볼릭 링크 거부, 필수 도구 사전 점검 보안 강화

