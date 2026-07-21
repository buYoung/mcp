# codemap-search 언어 및 파일 형식 지원 체크리스트

이 문서는 `codemap-search`가 추가로 인덱싱할 언어와 파일 형식의 우선순위를 정리한다. 문서·일반 텍스트, 잠금 파일, 압축·번들 파일은 검색 잡음을 줄이기 위해 명시적으로 제외한다.

## 지원 단계

파일 형식마다 필요한 기능 수준을 다음과 같이 구분한다.

1. **텍스트 검색**: 파일 내용과 경로를 BM25 검색 대상으로 포함한다.
2. **심볼 추출**: 선언, 키, selector, target 등 구조화된 항목을 codemap과 상세 검색 결과에 노출한다.
3. **탐색 지원**: import, 참조, 호출자·피호출자 또는 형식에 맞는 의존 관계를 추출한다.

설정·데이터·마크업 파일에 프로그래밍 언어와 같은 호출자·피호출자 모델을 강제하지 않는다. 각 형식에서 실제로 유용한 관계만 선택적으로 제공한다.

## 0순위 — 명시적 제외 정책

### 문서 및 일반 텍스트

- [x] Markdown 제외: `.md`, `.mdx`
- [x] 일반 텍스트 제외: `.txt`

### 사용자 홈을 인덱싱 루트로 사용 금지

작업공간 또는 명시적 인덱싱 대상이 사용자 홈 디렉터리 자체이면 설정·인덱스·watcher를 만들기 전에 거부한다. 홈 경로를 확인할 수 없으면 MCP 프로토콜에 영향을 주지 않는 `stderr` 경고를 남기고 실행을 허용한다.

- [x] 작업공간 루트가 사용자 홈 디렉터리 자체와 같으면 인덱싱 시작을 거부
- [x] CLI의 명시적 인덱싱 대상이 사용자 홈 디렉터리 자체와 같으면 거부
- [x] 홈의 하위 프로젝트 디렉터리는 정상적으로 허용
  - 예: `~/work/project`는 허용한다.
  - 예: `~` 자체는 거부한다.
- [x] 심볼릭 링크와 `..` 경로를 우회할 수 없도록 정규화한 절대 경로로 비교
- [x] Unix의 `HOME`과 Windows의 `USERPROFILE`을 모두 고려
- [x] 사용자 홈을 확인할 수 없으면 `stderr` 경고 후 인덱싱 허용
- [x] `.codemap/config.toml`, `.codemap/index` 또는 watcher를 만들기 전에 검사
- [x] 거부 시 원인과 허용되는 실행 위치를 설명하는 명확한 오류 반환
- [x] 전역 설정 저장소인 `~/.codemap` 사용은 작업공간 인덱싱과 구분하여 허용
- [x] 거부된 실행이 부분 인덱스나 설정 파일을 남기지 않는지 확인

### 잠금 파일

- [x] 모든 `*.lock` 파일 제외
- [x] 다음 잠금 파일을 정확한 파일명으로 제외
  - `package-lock.json`
  - `npm-shrinkwrap.json`
  - `pnpm-lock.yaml`
  - `yarn.lock`
  - `bun.lock`
  - `bun.lockb`
  - `Cargo.lock`
  - `Gemfile.lock`
  - `composer.lock`
  - `poetry.lock`
  - `Pipfile.lock`
- [x] 지원 확장자와 일치하더라도 잠금 파일 제외 규칙을 우선 적용
  - 예: JSON을 지원해도 `package-lock.json`은 인덱싱하지 않는다.
  - 예: YAML을 지원해도 `pnpm-lock.yaml`은 인덱싱하지 않는다.

### 압축·번들 및 파생 파일

- [x] Source map 제외: `*.map`
- [x] 압축된 JavaScript 제외
  - `*.min.js`
  - `*.min.mjs`
  - `*.min.cjs`
- [x] 압축된 CSS·HTML 제외
  - `*.min.css`
  - `*.min.html`
- [x] 번들 파일 제외
  - `*.bundle.js`
  - `*.bundle.mjs`
  - `*.bundle.cjs`
  - `*.bundle.css`
- [x] 제외 규칙은 ASCII 대소문자를 구분하지 않도록 처리
- [x] `.codemapignore`로 저장소별 제외 파일을 추가할 수 있도록 유지
- [x] 기본 제외 규칙은 인덱스·codemap·호출 탐색에서 해제할 수 없도록 유지
  - `find`와 `grep`은 `include_ignored: true`로 명시적 접근할 수 있다.
  - `read`와 `parse`의 직접 접근은 유지한다.

## 1순위 — 설정 및 구조화 데이터

- [ ] JSON 지원: `.json`
- [ ] JSON with Comments 지원: `.jsonc`
- [ ] JSON5 지원: `.json5`
- [ ] TOML 지원: `.toml`
- [ ] YAML 지원: `.yaml`, `.yml`
- [ ] 파일을 텍스트 검색 대상으로 포함
- [ ] 키와 중첩 키 경로를 심볼로 추출
- [ ] 배열과 스칼라 값은 텍스트 검색 대상으로만 처리
- [ ] 호출자·피호출자 탐색은 비활성화

## 2순위 — 웹, 스타일 및 마크업

### 독립 파일

- [ ] HTML 지원: `.html`, `.htm`
- [ ] XML 지원: `.xml`
- [ ] XML 파생 형식 지원 여부 결정
  - `.xsd`, `.xsl`, `.xslt`
  - `.plist`
  - `.csproj`, `.props`, `.targets`
- [ ] CSS 지원: `.css`
- [ ] Sass/SCSS 지원: `.sass`, `.scss`
- [ ] Less 지원: `.less`
- [ ] HTML/XML의 태그, `id`, `class`를 심볼로 추출
- [ ] CSS 계열의 selector, 변수, mixin, animation 이름을 심볼로 추출
- [ ] 호출자·피호출자 탐색은 비활성화

### 복합 컴포넌트

현재 Vue, Astro, Svelte는 내부 JavaScript/TypeScript 영역을 추출한다. HTML/CSS 문법을 등록하는 것만으로 `<template>`과 `<style>` 영역이 자동 지원되지는 않는다.

- [ ] Vue의 `<template>` 및 `<style>` 영역 검색 지원
- [ ] Astro의 마크업 및 `<style>` 영역 검색 지원
- [ ] Svelte의 마크업 및 `<style>` 영역 검색 지원
- [ ] 복합 파일에서 추출한 결과의 원본 줄·열 좌표 보존
- [ ] 같은 심볼이 여러 영역에서 중복 노출되지 않도록 병합 규칙 정의

## 3순위 — 범용 스크립트

- [ ] Shell 지원: `.sh`, `.bash`, `.zsh`
- [ ] 함수, 변수, 환경 변수를 심볼로 추출
- [ ] `source` 및 `.` 명령으로 불러오는 파일을 import로 추출
- [ ] 정적으로 확인 가능한 함수 호출 관계 지원 여부 결정
- [ ] 동적 명령 실행을 호출 관계에서 제외하거나 낮은 신뢰도로 처리

## 4순위 — 인프라 및 인터페이스

- [ ] HCL/Terraform 지원: `.hcl`, `.tf`, `.tfvars`
- [ ] Dockerfile 지원
- [ ] Protocol Buffers 지원: `.proto`
- [ ] GraphQL 지원: `.graphql`, `.gql`
- [ ] 확장자뿐 아니라 정확한 파일명으로 형식을 등록하는 기능 추가
  - `Dockerfile`
  - `Makefile`
  - `CMakeLists.txt`
  - `BUILD`, `BUILD.bazel`
- [ ] 선언, 리소스, 서비스, 타입을 형식별 심볼로 추출
- [ ] 호출 관계 대신 참조·의존 관계를 제공할 형식 결정

## 5순위 — 추가 프로그래밍 언어

- [ ] C# 지원: `.cs`
- [ ] PHP 지원: `.php`
- [ ] Ruby 지원: `.rb`
- [ ] Lua 지원: `.lua`
- [ ] 각 언어의 심볼, import, 참조, 호출 정보를 추출
- [ ] 테스트 코드, 공개 심볼, deprecated 상태 판별
- [ ] 호출자·피호출자 탐색 활성화

## 6순위 — 플랫폼 특화 언어

- [ ] Swift 지원: `.swift`
- [ ] Dart 지원: `.dart`
- [ ] Scala 지원: `.scala`, `.sc`
- [ ] Groovy 지원: `.groovy`, `.gradle`
- [ ] PowerShell 지원: `.ps1`, `.psm1`
- [ ] 실제 사용 수요를 확인한 뒤 구현 순서 결정

## 7순위 — 빌드 시스템 및 기타 형식

- [ ] CMake 지원: `.cmake`, `CMakeLists.txt`
- [ ] Make 지원: `Makefile`, `.mk`
- [ ] Starlark/Bazel 지원: `.bzl`, `BUILD`, `BUILD.bazel`
- [ ] Nix 지원: `.nix`
- [ ] WebAssembly text 지원: `.wat`, `.wast`
- [ ] target, rule, task, 변수를 형식별 심볼로 추출
- [ ] target 간 의존성을 일반 호출 관계와 별도로 표현

## SQL 정책

일반 SQL에는 프로그래밍 언어와 같은 호출자·피호출자 모델을 적용하지 않는다. 저장 프로시저 중심 저장소에서 수요가 확인될 때만 프로시저 호출 관계를 선택적으로 확장한다.

- [x] SQL 심볼 추출 유지
- [x] 일반 호출자·피호출자 탐색 비활성화
- [ ] 테이블 및 뷰 참조 추출
- [ ] 뷰에서 원본 테이블로 이어지는 의존 관계 추출
- [ ] `INSERT`, `UPDATE`, `DELETE` 대상 추출
- [ ] 함수, 프로시저, 트리거 정의 추출
- [ ] 프로시저 호출 관계는 필요성이 확인될 때만 선택적으로 지원

## 공통 완료 조건

새로운 언어나 파일 형식을 완료 처리하려면 다음 조건을 확인한다.

- [ ] 지원 확장자 또는 정확한 파일명이 중앙 등록부에서 최종 파일 탐색기로 전달된다.
- [ ] 잠금·압축·번들 파일 제외 규칙이 지원 확장자보다 우선한다.
- [ ] 선택한 지원 단계가 실제 검색, codemap, 심볼 상세 정보 또는 탐색 결과까지 전달된다.
- [ ] 지원하지 않는 단계는 빈 결과와 오류 중 어떤 동작을 사용할지 명확히 정의한다.
- [ ] 파서 오류가 인덱서나 MCP 서버 종료로 이어지지 않는다.
- [ ] 생성 파일과 대용량 파일에 대한 기존 보호 정책을 유지한다.
- [ ] 사용자 문서의 지원 언어·확장자 목록과 실제 등록부를 일치시킨다.
