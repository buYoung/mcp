---
title: 지원 언어 및 색인 규칙
description: 지원하는 프로그래밍 언어, 데이터·마크업 형식, 선택형 언어 그룹, 색인 제외 파일과 한도를 정리합니다.
---

## 프로그래밍 언어

| 언어 | 확장자 |
|---|---|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript / TSX | `.ts`, `.tsx`, `.mts`, `.cts` |
| JavaScript / JSX | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Go | `.go` |
| Java | `.java` |
| Kotlin | `.kt`, `.kts` |
| C | `.c` |
| C++ | `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` |
| C# | `.cs` |
| PHP | `.php` |
| Ruby | `.rb` |
| Lua | `.lua` |
| Assembly / GAS | `.s`, `.S`, `.asm` |
| Swift | `.swift` |
| Dart | `.dart` |
| Scala | `.scala`, `.sc` |
| Groovy / Gradle | `.groovy`, `.gradle` |
| PowerShell | `.ps1`, `.psm1` |
| SQL | `.sql` |

## 데이터, 마크업, 컴포넌트

| 형식 | 확장자 | 색인 대상 |
|---|---|---|
| JSON / JSONC | `.json`, `.jsonc` | 키, 중첩 키 경로, 텍스트 값 |
| TOML | `.toml` | 키, 중첩 키 경로, 텍스트 값 |
| YAML | `.yaml`, `.yml` | 키, 중첩 키 경로, 텍스트 값 |
| HTML | `.html`, `.htm` | 태그, `id`, `class` |
| XML과 파생 형식 | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.plist`, `.csproj`, `.props`, `.targets` | XML 태그와 속성 |
| CSS | `.css` | 선택자, 사용자 정의 속성, 키프레임 이름 |
| Less | `.less` | 스타일 규칙과 믹스인(`#identifier(...)`) |
| Sass | `.sass` | 들여쓰기 Sass 규칙 |
| Vue / Astro / Svelte | `.vue`, `.astro`, `.svelte` | 마크업과 내장 JS/TS, CSS/Sass/Less |

JSON5와 SCSS는 색인하지 않습니다.

## 선택형 언어 그룹

`.codemap/config.toml`의 `[index.language_support]`에서 필요한 키를 `true`로 설정하세요(모두 기본값 `false`). 실시간 `find`, `grep`, `read`는 이 설정과 관계없이 동작합니다.

| 키 | 확장자 / 파일명 |
|---|---|
| `is_document_support_enabled` | `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

## 제외 파일과 색인 한도

아래 확장자와 패턴은 항상 색인에서 제외됩니다.

| 구분 | 확장자 / 패턴 |
|---|---|
| 일반 텍스트 | `.txt` |
| 인증정보 | `auth.toml` |
| 잠금 파일 | `*.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `Gemfile.lock`, `composer.lock`, `poetry.lock`, `Pipfile.lock` |
| 소스 맵 | `*.map` |
| 축소 파일(Minified) | `*.min.js`, `*.min.mjs`, `*.min.cjs`, `*.min.css`, `*.min.html` |
| 번들 | `*.bundle.js`, `*.bundle.mjs`, `*.bundle.cjs`, `*.bundle.css` |

- 저장소별 제외 경로는 `.codemapignore`에 gitignore 문법으로 적으세요. 제외된 파일도 `find`와 `grep`에서 `include_ignored: true`로 조회할 수 있습니다.
- 기본값인 `[index].mode = "auto"`에서는 색인 가능 파일이 `50,000`개 이하인 저장소(`.git`, `.hg`, `.jj`)만 자동으로 색인합니다. `50,000`개를 넘는 저장소는 `.codemap/config.toml`에서 `[index].mode = "on"`으로 설정하세요.
- `[index].max_file_bytes`(기본 `1 MiB`)를 넘는 파일은 색인에서 건너뜁니다.
- `RUST_LOG=debug`로 실행하면 상세 진단 로그가 `stderr`로 출력됩니다.

```sh
RUST_LOG=debug codemap-search mcp
```
