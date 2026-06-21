# Changelog

codemap-search의 주요 변경 사항을 이 파일에 기록합니다.
형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/)를 따르며,
[유의적 버전](https://semver.org/lang/ko/)을 준수합니다.

## [Unreleased]

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

