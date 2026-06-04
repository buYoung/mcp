# `@buyong-mcp/scout` 설계 문서

> 내부 벤치마크 `results-combos.md`의 우승 파이프라인 **C4(zoekt+ctags→read)** 를 제품화한 독립 stdio MCP.
> 본 문서는 사용자 확정 결정을 반영한 정본 설계다. 미확정 항목은 9절에 기본값과 함께 명시한다.

## 확정된 핵심 결정 (사용자 승인)

| # | 결정 | 값 |
|---|---|---|
| 1 | 텍스트 검색 = zoekt, 심볼 = ctags **둘 다 필수** | ripgrep 폴백 없음 |
| 2 | 도구 표면 = **primitive 4개만 (v1)** | 라우터 `navigate_code`는 후속 |
| 3 | 시작 시 바이너리 미설치면 **에러 + 설치 안내 후 종료** | 조용한 폴백 금지 |
| 4 | READ 계층 = Claude Code Read/Glob/Grep 충실 모사 | 단 텍스트 엔진은 zoekt |
| 5 | zoekt 질의 모드 = **zoekt-webserver 자식 프로세스** | CLI shell-out 아님 |
| 6 | 도구 이름 = **snake_case** | 형제 앱 일관성 |
| 7 | 이미지/PDF/노트북 = **v1 미지원** | 명시적 에러 |
| 8 | 색인 대상 = **작업 트리(미커밋 포함)**, 자동 증분 | 커밋-온리 아님. 증분은 *무변경 skip + 디바운스 재색인*이라는 투명한 최적화(§6) |

---

## 1. 목적 및 acp-bridge와의 차이

벤치마크에서 우승한 `zoekt + ctags → read` 코드 내비게이션 파이프라인을 어느 코딩 에이전트든 호출할 수 있는 stdio MCP 도구 표면으로 노출한다. 핵심 가치는 "광범위 후보 탐색(zoekt) → 정의 앵커링(ctags) → 정밀 확인(read)"을 **정확한 역할 분리**로 제공하는 것이다.

형제 앱 `@buyong-mcp/acp-bridge`와의 차이: acp-bridge는 다른 코딩 에이전트(Codex, Gemini 등)를 ACP 자식 프로세스로 띄워 의견을 중계하는 "에이전트 간 페어 브리지"인 반면, 본 MCP는 외부 에이전트를 전혀 띄우지 않고 로컬 외부 바이너리(zoekt, ctags)와 파일시스템만으로 코드를 탐색·읽어 주는 "단일 에이전트용 코드 내비게이션 도구"다. 둘은 형제 앱이며 acp-bridge는 수정하지 않는다.

---

## 2. 도구 표면 — primitive 4개 (v1)

결정 2에 따라 v1은 라우터 없이 primitive 4개만 노출한다. 라우팅 판단(정의 vs 호출부)은 에이전트가 직접 수행하며, **그 판단을 유도하는 가이드를 각 도구의 description에 명시**한다. 도구 이름은 결정 6에 따라 형제 앱(`list_agents`/`ask_pair`) 규약과 일치하는 snake_case다.

| 도구 | 백엔드 | 역할 | Claude Code 대응 |
|---|---|---|---|
| `search_text` | zoekt(webserver) | 색인 텍스트(정규식) 검색. 광범위 후보·**호출부(call-site)**·교차 언어 탐색의 주력 | Grep 인터페이스 인체공학 + zoekt 엔진 |
| `lookup_symbol` | ctags | 심볼 **정의(definition)** 조회 | (ctags 고유) |
| `read_file` | read | 단일 파일을 cat -n 형식으로 읽기 | Read(FileReadTool) |
| `find_files` | glob(picomatch) | glob 패턴으로 파일 경로 탐색 | Glob(GlobTool) |

### 2.0 라우터 없는 v1에서 M3-W 통찰을 보존하는 법

벤치마크 핵심 발견(§4-2, §5): ctags와 LSP 심볼 검색은 **호출부 대신 선언부**(`TreeSitterWasmRuntime.kt:20-23`)를 잘못 잡아 교차 언어 경계(M3-W) 정밀도를 0.50~0.667로 떨어뜨린다(텍스트는 0.727). 라우터가 없으므로 이 통찰은 **도구 description의 명시적 가이드**로 에이전트에게 전달한다.

- `lookup_symbol.description`: "함수/클래스/타입 등의 **선언 위치**를 찾을 때만 사용. 호출부·사용처(call-site)나 교차 언어 사용을 찾으려면 `search_text`를 써라 — ctags는 호출부가 아니라 선언을 잡으므로 호출부 탐색에 부정확하다."
- `search_text.description`: "광범위 후보, 호출부(call-site), 교차 언어 탐색에 사용. 심볼 '정의' 위치만 원하면 `lookup_symbol`을 써라."
- `read_file`은 "확인 단계" — 두 검색 도구가 돌려준 후보 위치를 실제 코드로 검증하라고 안내(M3-L enum 오탐을 정밀도 1.00으로 교정하는 품질 승수).

### 2.1 `search_text` (zoekt) — JSON Schema 스케치

Grep의 인터페이스 인체공학(output_mode/context/head_limit/glob/type)을 빌리되 내용 검색 엔진은 zoekt다. ripgrep 전용 플래그는 zoekt 의미로 매핑하거나 드롭한다.

```jsonc
{
  "name": "search_text",
  "description": "색인 기반 텍스트(정규식) 내용 검색 (zoekt webserver 백엔드). Claude Code의 Grep에 대응하나 엔진은 ripgrep이 아닌 zoekt다. 광범위 후보, 호출부(call-site), 교차 언어 탐색에 사용. 심볼 '정의' 위치만 원하면 lookup_symbol을 써라.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "pattern":      { "type": "string", "description": "검색할 정규식. zoekt RE2 문법." },
      "path":         { "type": "string", "description": "검색 범위 디렉터리. 미지정 시 색인 루트(cwd). zoekt 'file:' 어트리뷰트로 변환." },
      "glob":         { "type": "string", "description": "파일 필터 glob. 예 '*.{ts,tsx}'. zoekt 'file:' 정규식으로 변환." },
      "type":         { "type": "string", "description": "파일 타입 약칭(js,py,go...). 내부 확장자 매핑 후 zoekt 'lang:'/'file:'로 변환." },
      "output_mode":  { "type": "string", "enum": ["content","files_with_matches","count"], "description": "기본 files_with_matches." },
      "case_insensitive":     { "type": "boolean", "description": "대소문자 무시. 기본 false." },
      "show_line_numbers":    { "type": "boolean", "description": "content 모드에서 줄번호 표시. 기본 true." },
      "context_lines":        { "type": "number", "description": "각 매치 앞뒤 컨텍스트 줄 수(-C 별칭). content 모드만. webserver NumContextLines로 전달." },
      "context_before_lines": { "type": "number", "description": "매치 앞 컨텍스트(-B). content 모드만." },
      "context_after_lines":  { "type": "number", "description": "매치 뒤 컨텍스트(-A). content 모드만." },
      "head_limit":   { "type": "number", "description": "결과 상한(전 모드). 미지정 시 250, 0이면 무제한." },
      "offset":       { "type": "number", "description": "head_limit 적용 전 건너뛸 엔트리 수. 기본 0." }
    },
    "required": ["pattern"]
  }
}
```

매핑 규칙과 드롭 항목:
- 컨텍스트 줄(-A/-B/-C)은 **zoekt-webserver의 `NumContextLines` 검색 옵션으로 네이티브 전달**한다(결정 5의 직접 이점 — CLI 모드에서 필요했던 ReadProvider 합성이 불필요해짐). 우선순위 `context_lines > context_before/after_lines`, content 모드만 유효.
- VCS 디렉터리(`.git .svn .hg .bzr .jj .sl`)와 `node_modules`/`dist`/`.turbo`는 **색인 시점에 제외**하므로 검색 쿼리에 별도 제외 불필요(Grep의 `--glob !dir`에 상응하는 동작을 색인 단계로 이동).
- output_mode 3종의 텍스트 렌더는 Grep 명세를 재현: 0건 `No matches found`/`No files found`, `Found N file(s)`, `Found N total occurrence(s) across M file(s).` + 절단 시에만 `[Showing results with pagination = ...]` 푸터.
- `head_limit` 기본 250 / 0=무제한, `offset` 기본 0.
- multiline 검색은 zoekt가 기본 줄 단위 매처라 1차 드롭(9절 기본값).

### 2.2 `lookup_symbol` (ctags) — JSON Schema 스케치

```jsonc
{
  "name": "lookup_symbol",
  "description": "심볼 '정의(definition)' 조회 (Universal Ctags 백엔드). 함수/클래스/타입 등의 선언 위치를 찾을 때만 사용. 호출부/사용처(call-site)나 교차 언어 사용을 찾으려면 search_text를 써라 — ctags는 호출부가 아니라 선언을 잡으므로 호출부 탐색에 부정확하다.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "symbol_name":     { "type": "string", "description": "찾을 심볼 이름. 정확 일치 기본." },
      "kind":            { "type": "string", "description": "심볼 종류 필터(function/class/struct/method/...). 미지정 시 전체." },
      "path":            { "type": "string", "description": "스코프 제한 디렉터리/파일. 미지정 시 cwd." },
      "language":        { "type": "string", "description": "언어 필터(ctags --languages=)." },
      "is_prefix_match": { "type": "boolean", "description": "접두 일치 허용. 기본 false(정확 일치)." },
      "head_limit":      { "type": "number", "description": "결과 상한. 기본 250." }
    },
    "required": ["symbol_name"]
  }
}
```

ctags는 영속 tags 파일을 만들지 않고 on-demand 스코프 실행한다(`ctags --output-format=json --fields=+nKsSa -f - <scoped files>`). 출력은 `{symbol, kind, language, file(상대경로), line, scope, signature}` 배열로 구조화한다.

### 2.3 `read_file` / 2.4 `find_files` — 4절에서 상세

---

## 3. 제공자 아키텍처 (Provider Architecture)

> **핵심 경계**: 읽기 계층(`read_file`, `find_files`)은 파일시스템 직접 접근이며 색인·webserver에 의존하지 않는다. 오직 `search_text`만 zoekt 색인 + webserver에 의존한다. 따라서 Read/Glob은 색인이 빌드되기 전에도, webserver가 죽어 있어도 동작해야 한다.

### 3.1 TextSearchProvider (zoekt + webserver) — 결정 5

- 책임: 색인 lifecycle(6절) + **zoekt-webserver 자식 프로세스 lifecycle** + 쿼리 빌드/HTTP 질의/결과 렌더.
- 바이너리: 색인기 `zoekt-index`(또는 `zoekt-git-index`), 서버 `zoekt-webserver`. 둘 다 startup 점검 대상.
- webserver lifecycle:
  - **lazy 기동**: 첫 `search_text` 호출 시 색인이 준비되면 `zoekt-webserver -index <indexDir> -listen 127.0.0.1:<port>`를 자식으로 띄운다.
  - **보안**: 반드시 `127.0.0.1`(루프백)에만 바인드하고 **OS가 할당한 임의 포트** 사용. 외부 노출 금지.
  - **헬스체크**: 기동 후 검색 가능 상태가 될 때까지 폴링(타임아웃·재시도). 준비되면 포트를 보관해 이후 질의 재사용(웜 상태 유지가 결정 5의 성능 이점).
  - **종료**: MCP 프로세스 종료(SIGINT/SIGTERM/`process.on("exit")`) 시 자식에 SIGTERM 후 정리. 좀비 프로세스 방지.
  - **크래시 복구**: 질의 시 webserver 무응답이면 1회 재기동.
  - **재색인 반영**: zoekt-webserver는 색인 디렉터리의 shard 변경을 폴링·자동 리로드한다(6.4). 자동 리로드가 불충분하면 재기동으로 강제.

### 3.2 SymbolProvider (ctags)

- 책임: Universal Ctags JSON 출력으로 심볼 정의 조회.
- 영속 tags 파일은 만들지 않되, **on-demand는 "전체 트리 `ctags -R`"가 아니라 "소스 파일 목록 주입(`ctags -L`)"이어야 한다**(실측: `-R`은 node_modules·JSON까지 훑어 52s/1.2GB. `-L` 소스만이면 0.09~2.38s. `docs/briefs/lookup-symbol.md` §3). 반복 호출 비용은 **작업 트리 fingerprint 키 인메모리 캐시**로 제거한다(영속 파일은 선택) — 이 점이 초기 "순수 on-demand" 서술의 정정이다.
- 바이너리: `ctags`. startup에서 **Universal Ctags 변형 검증** 필수(`ctags --version`에 `Universal Ctags` 포함) — 단순 PATH 확인만으론 BSD/Exuberant ctags가 통과해 `--output-format=json`에서 런타임 실패.

### 3.3 ReadProvider (Claude Code Read/Glob 모사)

- 책임: 파일 읽기(`read_file`), 파일명 검색(`find_files`).
- 파일시스템 직접 접근, 색인·webserver 독립. 4절 상세.
- `find_files`의 백엔드는 zoekt도 ripgrep도 아닌 **JS glob 라이브러리(picomatch 계열, fast-glob/globby)** — zoekt는 내용 색인이라 파일명 매처가 아님. 결정 1의 "ripgrep 금지"는 텍스트 내용 검색에 적용되며 파일명 매칭은 별도 메커니즘이다.

### 3.4 공통 읽기 권한 게이트

세 읽기 진입점(`read_file`, `find_files`, `search_text`의 path)은 공통 경계 검증을 통과한다. Claude Code의 `checkReadPermissionForTool` 결정 순서를 단순화 재현: UNC 경로(`\\`/`//`)는 파일 I/O 전 거부, `expandPath` 정규화(~ 확장, 상대경로 cwd 확장) 후 cwd 경계 내부면 허용·외부면 거부. 형제 앱의 `validateFilesWithinCwd`(realpath 기반 경계 검사)를 복사·확장한다(결정: acp-bridge 수정 금지이므로 공유 승격 대신 복사, 9절).

---

## 4. READ 계층 명세 — Claude Code 충실 매핑 (결정 4)

읽기 계층은 Read(파일 읽기)·Glob(파일명 검색) 의미를 충실히 모사하고, Grep의 인터페이스 인체공학만 빌리되 내용 검색 엔진은 zoekt다(2.1절). 아래는 참조 코드(`/Users/buyonglee/Downloads/claude-code-main/src/tools/FileReadTool`·`GlobTool`)에서 추출한 정확한 상수와 명시 판정이다.

### 4.1 `read_file` (Read 모사) 판정표

| 항목 | 판정 | 재현 상수/근거 |
|---|---|---|
| 입력 스키마 | `{file_path(필수), offset?, limit?}` strict 검증, 추가 키 거부 | FileReadTool strictObject. `pages`는 PDF 전용이라 제거(4.3) |
| offset/limit 기본값 | 스키마 default 아닌 런타임 기본: `offset=1`, `limit=undefined` | `lineOffset = offset===0 ? 0 : offset-1`(0과 1 모두 첫 줄), 줄번호는 raw offset부터 |
| 줄 형식 | compact `<번호><TAB><내용>`을 `\n`으로 연결 | 기본 compact. 레거시 `padStart(6,' ') + →`는 미구현(GrowthBook 게이팅 제거) |
| 바이트 캡 | **256KB(262144) 유지** | `limit` 미지정 시에만 전 파일 크기에 적용, 초과 시 절단 아닌 throw(`FileTooLargeError`). 명시적 `limit` 있으면 미적용(비대칭성 보존) |
| 토큰 캡(25000) | **드롭** | Claude API(`countTokensWithAPI`) 의존이라 독립 stdio MCP에서 불가. 유일 상한은 256KB 바이트 캡. 로컬 추정 캡은 9절 기본값 |
| file_unchanged dedup | **유지** | `readFileState` 유사 맵에 `{content, timestamp=Math.floor(mtimeMs), offset, limit}` 저장, 동일 offset&limit + `Math.floor(현재 mtimeMs)===저장 timestamp`일 때 `FILE_UNCHANGED_STUB` 반환. 텍스트만 적용. `Math.floor` 누락 시 동등 비교 깨짐 |
| CYBER_RISK_MITIGATION_REMINDER | **드롭** | Claude Code 내부 멀웨어 가드 문구. 범용 MCP에 부적합 |
| MAX_LINES_TO_READ(2000) | 강제 안 함 | 프롬프트 광고 문구로만 사용. limit 미지정 시 실제 제한은 256KB 바이트 캡 |
| 빈 파일 | `<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>` 정확 재현 | 필수 |
| offset 초과 | `<system-reminder>Warning: the file exists but is shorter than the provided offset (N). The file has M lines.</system-reminder>` | 필수 |
| 디렉터리 | `EISDIR` throw(디렉터리 읽기 불가, ls 안내) | 필수 |
| 바이너리 확장자 | 거부(SVG는 텍스트 허용, PDF/이미지는 미지원 에러 4.3) | |
| 차단 디바이스 경로 | `/dev/zero,/dev/random,/dev/urandom,/dev/full,/dev/stdin,/dev/tty,/dev/console,/dev/stdout,/dev/stderr,/dev/fd/0-2,/proc/*/fd/0-2` 거부 | 보안 필수 |
| 경로 정규화 | `expandPath`(~ 확장, 상대경로 cwd 확장, 공백 트림), UNC 거부 | 보안 필수 |
| 대용량 동작 | 10MB(FAST_PATH_MAX_SIZE) 미만 메모리 분할, 이상 스트리밍(highWaterMark 512KB) | 성능 동작 재현 |

### 4.2 `find_files` (Glob 모사) 판정표

| 항목 | 판정 | 재현 상수/근거 |
|---|---|---|
| 입력 스키마 | `{pattern(필수), path?}` strict, 추가 키 거부 | path 미지정 시 cwd, 제공 시 expandPath 후 존재·디렉터리 검증 |
| 백엔드 | **JS glob 라이브러리(picomatch 계열)** | ripgrep `--files --glob` 동작을 라이브러리로 대체. `**`,`*`,`?`,`[]`,`{}` 지원 |
| 정렬 | mtime 오래된 순(oldest first) | Glob `--sort=modified` 재현. 절단 시 오래된 100개 유지가 의도된 동작 |
| 결과 상한 | **100** | 초과 시 절단, 마지막 줄에 정확 문구 `(Results are truncated. Consider using a more specific path or pattern.)` |
| 출력 형식 | 0건이면 `No files found`. 1건+면 `\n` join 경로(cwd 하위 상대화), 헤더·줄번호 없음 | |
| .gitignore/숨김 | Claude Code 기본 재현: .gitignore 무시 + 숨김 포함 | 설정은 TOML(`<repo>/.scout/config.toml` > `~/.scout/config.toml` > 기본값)로 받으며 **env는 설정 수단이 아니다**(`SCOUT_GLOB_*` 토글은 폐기). 동작은 코드 고정 기본(.gitignore 무시·숨김 포함)으로 유지 |

### 4.3 이미지/PDF/Jupyter 노트북 처리 — 결정 7

- **이미지: v1 미지원**. `IMAGE_EXTENSIONS(png/jpg/jpeg/gif/webp)`에 대해 `Image reading is not supported in v1.` 에러. sharp 의존·토큰 추정·압축 폴백 회피.
- **PDF/Jupyter 노트북: 미지원**. document 블록/멀티모달 결과 + 외부 의존(poppler 등) 회피. `PDF/Jupyter reading is not supported by this MCP.` 에러. `pages` 파라미터도 스키마에서 제거.
- 결과적으로 4.1 바이너리 확장자 거부에서 SVG만 텍스트 허용 예외로 남기고 PDF/이미지는 미지원 에러 경로로 보낸다.

### 4.4 드롭하는 Claude Code 내부 관심사

`memoryFileFreshnessPrefix`, 모든 `logEvent`/telemetry(`tengu_*`), `logFileOperation`, 스킬 디스커버리, `nestedMemoryAttachmentTriggers`, `fileReadListeners`, GrowthBook/Statsig 플래그, UI.tsx(터미널 chrome). 모두 호스트/UI/텔레메트리 종속이라 동작 무관.

---

## 5. Startup 필수 바이너리 점검 + 저하 모드 + 자동 설치 도구 (결정 3, 갱신)

`main()` 진입 직후, transport 연결 전 점검(`resolveBinaries`). **종료(`process.exit`)하지 않는다.** 누락이면 안내를 stderr에 출력하고 **저하 모드(degraded)로 부팅**한다 — 서버는 정상 기동하되 `search_text`는 검색 대신 설치 안내 텍스트를 반환한다. **조용한 폴백 금지**의 의미는 "프로세스를 죽인다"가 아니라 "누락을 명시적으로 보고하고 잘못된 결과를 내지 않는다"로 갱신한다.

복구 경로(트리거 모델 = MCP 설치 도구):
- stdio MCP는 런타임에 대화형 동의 프롬프트가 불가하므로, 에이전트가 사용자에게 다운로드 동의를 구한 뒤 **`install_binaries` 도구**를 호출한다. 시작 시 자동 다운로드는 하지 않는다.
- `install_binaries` → `installManagedBinaries()`: 핀 고정 태그(`BINARY_RELEASE_TAG`)에서 현재 플랫폼 자산(`zoekt-ctags-<plat>.{tar.gz,zip}`)과 `<asset>.sha256`를 받아 **SHA-256 검증**(전송 무결성; 서명/출처 인증은 아님) 후 관리형 bin 디렉터리에 설치한다. 안정성 장치: ① 본문 스트리밍 + 다운로드 타임아웃을 본문 수신 내내 유지 + 최대 크기 제한, ② **스테이징 디렉터리에서 추출·정리 후 원자적 교체(swap)** — 실패해도 기존 설치를 건드리지 않고 실행 중 자식이 쓰는 파일을 도중에 지우지 않음, ③ 관리형 디렉터리는 항상 `~/.scout/bin/<tag>`(`os.homedir()` 기준, env 오버라이드 없음)로 고정해 소유 하위 경로만 rm 하도록 스코프를 좁히고(루트 자체를 rm 하지 않도록), ④ 재설치 전 기존 provider/webserver를 먼저 종료하고 설치 중 search를 대기시킴. 추출은 `tar --strip-components=1`(Windows zip은 절대경로 `System32\tar.exe`), `universal-ctags`→`ctags` rename, Unix chmod. 성공 시 재해석 후 provider 지연 생성. (남은 한계: 동일 머신의 **여러 프로세스가 동시에** 설치하면 중복 다운로드 후 마지막 교체가 이기는 정도로 수렴 — 교차 프로세스 잠금은 개인 도구 범위상 도입하지 않음.)
- 릴리스 저장소: `buYoung/zoetk-ctags-release`. 아카이브 1개에 zoekt(index/webserver/git-index) + Universal Ctags가 모두 번들된다. v0.0.3부터 6종 플랫폼(linux/macos/windows × amd64/arm64)이 모두 제공된다. 매핑에 없는 조합은 자산 부재 시 404 graceful 폴백(수동 설치 안내)한다.
- 관리형 bin 디렉터리는 자식 PATH 앞에 prepend(`prependManagedBinToPath`)하여 `zoekt-index`가 색인 중 내부 호출하는 ctags도 찾게 한다.

기존 수동 설치 점검 단계는 그대로 유지(누락 판정 기준):

점검 단계:
1. 색인기(필수): `isCommandAvailable("zoekt-index")` — 결정 8에 따라 **작업 트리(미커밋 포함)를 색인**하므로 일반 디렉터리 색인기 `zoekt-index`가 필수다. `zoekt-git-index`는 대형 리포 커밋-델타 보강 경로(§6.6)에서만 선택적으로 쓰이므로 필수 아님(있으면 활용).
2. 서버: `isCommandAvailable("zoekt-webserver")` (결정 5 — 질의 모드가 webserver이므로 `zoekt` CLI 대신 webserver를 필수로 점검).
3. `isCommandAvailable("ctags")` 존재 + **Universal 변형 검증**(`ctags --version`에 `Universal Ctags` 포함).

`resolveExecutablePath`은 형제 앱의 PATH + 실행 권한 프로브 헬퍼를 복사 재사용하되, **`~/go/bin` / `$(go env GOPATH)/bin` 폴백과 관리형 bin 디렉터리 탐색을 추가**한다. (실측: `go install`이 zoekt를 `~/go/bin`에 깔지만 이 경로가 PATH에 없는 환경이 흔하다 — PATH만 보면 설치돼 있어도 누락으로 오판한다.) 폴백에서 찾으면 그 절대 경로를 색인기·서버 실행에 사용하고, 그래도 없으면 안내 텍스트에 install_binaries 도구와 수동 설치 항목을 포함한다.

안내 텍스트(`buildInstallationGuidance`; stderr·저하 `search_text`·설치 실패 응답 공용, 누락 항목만 동적 출력):

```
[scout] 필수 외부 바이너리가 누락되었습니다. 이 MCP는 폴백 없이 zoekt와 Universal Ctags를 모두 요구합니다.

누락 항목:
  - zoekt-webserver (텍스트 검색 질의 서버)   상태: 미설치
  - ctags (Universal Ctags, 심볼 색인)        상태: 설치됨이나 Universal 변형 아님

해결 방법:
  1) (권장) install_binaries 도구를 호출하면 사전 빌드된 바이너리를 자동으로 내려받습니다.
     - 사용자에게 다운로드 동의를 먼저 구한 뒤 호출하세요. SHA-256으로 무결성을 검증합니다.
  2) 수동 설치:
     zoekt:  go install github.com/sourcegraph/zoekt/cmd/...@latest
     ctags:  brew install universal-ctags  /  apt-get install universal-ctags
```

---

## 6. zoekt 색인 lifecycle (엔지니어링 크럭스) — 작업 트리 신선도 (결정 8)

벤치마크 비용 수치는 색인/워밍업 시간을 제외하므로, 색인 lifecycle이 실제 핵심이다. ctags는 저렴(on-demand)하지만 zoekt는 색인 + webserver 둘 다 관리 대상이다.

### 6.0 원칙 — 색인 대상은 작업 트리이고, 증분은 투명한 최적화다

**`search_text`는 디스크의 현재 상태(미커밋 편집 포함)를 반영해야 한다.** 작업 트리에 있는 코드를 검색이 못 찾으면 거짓 음성(예: "이 심볼은 호출처가 없다")이 발생해 벤치마크 핵심 케이스(M1·M2)를 깨뜨린다. 따라서 색인 대상은 git 커밋 상태가 아니라 **작업 트리**다.

"증분"은 사용자에게 부담을 주는 제약이 아니라 보이지 않는 성능 최적화여야 한다. 즉 **수정 전엔 커밋을 강요하지 않고, 수동 재색인도 강요하지 않는다.** "증분"이 의미하는 것은 "변경 시에만, 그것도 자동으로 재색인하고, 무변경이면 아무 일도 안 함"이다.

> ⚠ stock zoekt의 정직한 제약(실측 확정): 작업 트리 **파일 단위 델타**는 불가능하다. 두 갈래뿐이다 — (a) `zoekt-git-index -delta`는 파일 단위 델타지만 **커밋 기준**(작업 트리에 stale), (b) `zoekt-index`는 **작업 트리**를 색인하지만 **`-delta`도 `-incremental`도 없어**(실측: 두 플래그 모두 부재) 매 실행이 전체 재색인이다. 결정 8(작업 트리 신선도)이 요구사항이므로 (b)를 기본으로 채택한다. 따라서 v1의 "자동 증분"은 정확히 **디바운스된 작업 트리 전체 재색인 + (우리가 직접 구현하는) 무변경 skip + 변경 합치기(coalesce)**를 뜻한다(파일 델타가 아님). 무변경 skip은 zoekt가 안 해주므로 **우리 lifecycle이 색인 대상 파일들의 mtime/콘텐츠 해시를 직전 빌드와 비교해 직접 판단**한다.

### 6.1 빌드/질의 메커니즘 (결정 5 = webserver, 결정 8 = 작업 트리) — 실측 확정

zoekt v0.0.0-20260528(go 1.25.10 빌드)로 실측한 결과를 반영한다.

**색인 빌드** — `zoekt-index -index <캐시> -ignore_dirs <제외목록> <작업트리>`:
- `zoekt-index` 플래그(실측): `-delta`/`-incremental` **없음**(전체 재색인 전용), `-index`(출력 shard 디렉터리), `-ignore_dirs`(기본 `.git,.hg,.svn`뿐 → node_modules 등 **우리가 명시 전달 필수**), `-file_limit`(기본 2MB), `-parallelism`(기본 4), `-disable_ctags`/`-require_ctags`, `-large_file`.
- zoekt-index는 색인 시 **ctags를 내부 호출해 심볼 랭킹 메타를 넣는다**(기본 ON). 우리 `lookup_symbol`은 ctags를 직접 호출하므로 이와 별개다. 비용은 미미(아래 측정).

**질의** — lazy 기동된 `zoekt-webserver`에 HTTP JSON 요청(실측 확정):
- 엔드포인트: `GET /search?q=<쿼리>&format=json&num=<최대건수>&ctx=<컨텍스트줄>` → `Content-Type: application/json`.
- 응답 스키마: `result.{ QueryStr, Query(파싱된 쿼리), Stats.{MatchCount, FileCount, Duration(ns)}, FileMatches[].{ FileName, Repo, Language, Matches[].{ LineNum, Fragments[].{Pre, Match, Post} }, Before, After } }`. → content/files_with_matches/count 3모드를 이 한 응답으로 렌더 가능(`Stats`로 count·files, `FileMatches`로 content).
- `q`는 **풀 zoekt 쿼리 문법**을 받음(실측): `lang:Kotlin`·`file:src`·정규식(RE2)·부정 `-file:test` 모두 `(and ...)`로 결합 파싱됨. → `zoekt-query-builder`가 glob→`file:`, type→`lang:`, 패턴→정규식/substr로 매핑해 `q` 한 문자열로 전달.
- 단 이 JSON은 **웹 UI의 JSON 변형**이다(응답에 `URL`/`ResultID`/`AutoFocus` 등 UI 필드 포함). 더 "공식" 경로인 `-rpc`는 go/net RPC(gob)라 Node에서 부르기 어려우므로, Node MCP엔 이 `format=json` 엔드포인트가 실용적 선택이다.
- 주의(실측): `q=sym:formatJson`은 0건 — **zoekt 심볼 검색은 신뢰 불가**. 심볼 정의는 반드시 `lookup_symbol`(ctags 직접)로 처리(2.2 설계 재확인).

**실측 재색인 시간**(ctags ON, mac arm64):

| 리포 | 추적 파일 | 재색인(콜드/웜) | 색인 크기 |
|---|---|---|---|
| buyong-mcp | 65 | 0.17s / 0.10s | 832K |
| intellij-json-helper2 | 409 | 0.53s / 0.40s | 15M |

ctags OFF 차이는 0.14s 수준(미미). → **전 구간 1초 미만, 6.6 단순 경로 채택 근거 확보.**

### 6.2 저장 위치

- **색인은 레포 안에 고정**: `<repo>/.scout/zoekt/`(설정 불가). 그 아래 `shards/`와 `meta.json`을 둔다. 작업 트리 색인이라 리포별 분리가 자연스럽고, 부팅 시 `<repo>/.scout/`를 `.git/info/exclude`에 등록해 git 추적에서 숨겨 작업 트리 오염을 막는다(§6.5). 전역 캐시 + 리포 경로 SHA 해시 디렉터리 방식과 `SCOUT_INDEX_DIR` 오버라이드는 **폐기**했다.
- **관리형 바이너리는 전역 공유**: 다운로드한 바이너리만 `~/.scout/bin/<tag>`(= `os.homedir()` 기준, 핀 태그별 하위 디렉터리)에 두어 여러 레포가 공유한다. XDG·`SCOUT_BIN_DIR`·env 오버라이드는 제거했다.

### 6.3 staleness 신호 (작업 트리 기준)

- 색인 대상이 작업 트리이므로 staleness 1차 신호는 **HEAD sha가 아니라 작업 트리 변경**이다.
- 신호: 색인 빌드 시각(`builtAtMs`)을 메타파일(`<repoHash>/meta.json: {builtAtMs, repoPath, port?}`)에 기록하고, 색인 대상 파일들의 **최신 mtime이 `builtAtMs`보다 크면 stale**로 본다. 전체 mtime 스캔은 비싸므로 짧은 TTL(`stalenessCheckTtlMs`) throttle로 재검사 빈도를 제한한다.
- watcher 모드(6.4)에서는 fs 이벤트가 1차 신호가 되고 mtime 스캔은 폴백/검증용이다.

### 6.4 자동 재색인 트리거 (결정 8 = 자동)

- **기본: 백그라운드 watcher.** 색인 대상 경로를 fs watch(또는 짧은 주기 폴링)로 감시하다 변경 이벤트가 오면 **디바운스**(예: `reindexDebounceMs`) 후 `zoekt-index`를 백그라운드로 1회 실행한다(전체 재색인 — zoekt-index에 증분 플래그 없음, 6.1 실측). 디바운스 창 안의 연속 편집은 한 번으로 합쳐(coalesce) 폭주를 막고, **직전 빌드 대비 변경이 없으면(mtime/해시 비교) 아예 zoekt-index를 호출하지 않는다**(우리가 구현하는 무변경 skip). 동시/중복 빌드는 단일 빌드 프라미스(빌드 lock)로 합친다.
- **부트스트랩: lazy build.** 색인이 아직 없으면 첫 `search_text` 호출 시 빌드(watcher가 아직 안 돈 콜드 스타트 대비).
- 재색인 진행 중에도 질의는 **기존 색인으로 응답(stale 플래그 부착)** 한다 — 자동 즉시 동기 재색인으로 질의를 막지 않는다.
- 재색인 완료 후 webserver는 shard 변경을 폴링·자동 리로드한다(불충분 시 재기동, 3.1).
- ctags는 영속 색인 불필요 — 매 `lookup_symbol`마다 스코프 실행(항상 디스크 최신).
- 명시적 `rebuild_index` 도구는 9절 기본값(v1 제외, 자동 처리로 충분).

### 6.5 색인 범위/제외

- 색인 시 VCS(`.git .svn .hg .bzr .jj .sl`), `node_modules`, `dist`, `.turbo`, 기타 표준 빌드/벤더 디렉터리를 제외한다.
- 작업 트리 색인(`zoekt-index`)은 git을 거치지 않으므로 **.gitignore가 자동 적용되지 않는다.** 따라서 제외 목록을 명시적으로 관리하고(`config/defaults.ts`의 `EXCLUDED_DIRECTORY_NAMES`, 설정으로 replace 가능), 리포의 `.gitignore`를 읽어 제외 집합에 union 한다(미커밋 산출물·빌드 결과가 색인에 새지 않도록).
- **.gitignore 합류는 구현됨(디렉터리-이름 수준만)**: `config/gitignore-excludes.ts`가 `<repo>/.gitignore`를 읽어 `name`/`name/` 형태의 단순 디렉터리 이름만 수집한다. 부정(`!`)·slash 포함 경로(`a/b`, `/x`)·glob 메타(`* ? [ ]`) 라인은 skip하며 — 즉 **glob/경로/negation은 미반영**이다. `respect_gitignore`(기본 true)가 켜져 있을 때만 적용되고, 수집한 이름은 replace가 아니라 제외 집합에 union 한다(별개 소스).

### 6.6 대형 리포 성능 분기 (측정으로 확정 — v1은 단순 경로)

판가름 사실은 **리포 전체 재색인 벽시계 시간**이고, 실측(6.1)에서 단순 경로가 확정됐다.

- **초 단위(실측: 65파일 0.1s·409파일 0.5s)** → 6.1~6.4의 **단순 경로로 충분**. 작업 트리 전체 재색인 + 디바운스 + 무변경 skip. 델타/하이브리드를 만들지 않는다. 이 규모의 10~20배(수천 파일)도 수 초라 백그라운드 디바운스로 무감하다.
- **수십 초+**(대형 모노레포) → 그때만 복잡도를 추가 검토:
  - 커밋 base는 `zoekt-git-index -delta`(파일 델타, 저렴)로 빠르게 유지하고, **더티 파일만** 별도로 색인해 오버레이.
  - 또는 `git stash create`로 작업 트리(미커밋 포함) 상태의 tree/commit 객체를 만들어(작업 트리는 건드리지 않음) `zoekt-git-index -delta`에 먹이는 트릭 — 조사 대상이지 기본값 아님.
- v1은 단순 경로로 출하하고, 측정 결과 대형 리포 부담이 확인되면 위 보강을 후속으로 둔다.

---

## 7. 파일 레이아웃 (`apps/scout/src/**`)

형제 앱 구조(`index.ts` 엔트리, `tools/`, provider별 디렉터리, `config/`)를 따른다. 네이밍은 풀 워드·약어 금지·동사+명사 규약. 아래는 레이아웃이며 v1 구현 현황은 §10을 따른다 — `symbol/`·`read/`는 구현됐고(다만 `security/`는 계획의 `path-guard.ts` 외에 공통 읽기 게이트 `read-guard.ts`가 추가됐다), `index-watcher.ts`만 미구현이다.

```
apps/scout/
  DESIGN.md                                  # 이 문서
  package.json                               # @buyong-mcp/scout, bin scout, ESM
  tsconfig.json                              # @repo/typescript-config/node.json 확장
  src/
    index.ts                                 # #!/usr/bin/env node, loadScoutConfig + gitignore union + .git/info/exclude 등록, (저하 모드) resolveBinaries 후 registerTools + McpServer Stdio 연결, 종료 훅
    startup/
      ensure-required-binaries.ts            # zoekt-index/webserver + Universal Ctags 점검, 안내 텍스트
      binary-availability.ts                 # 형제 앱 헬퍼 복사
      git-exclude.ts                         # <repo>/.scout/ 를 .git/info/exclude에 멱등 등록(전역 토글)
    tools/
      index.ts                               # registerTools: McpServer.registerTool(zod raw shape) 검증, search_text·install_binaries, textResult
    providers/
      text-search/
        text-search-provider.ts              # 인터페이스 + zoekt 진입
        zoekt-query-builder.ts               # pattern/glob/type/case → zoekt 쿼리
        zoekt-webserver-lifecycle.ts         # 자식 기동/포트/헬스/종료/재기동 (결정 5)
        zoekt-search-client.ts               # webserver HTTP JSON 질의 (NumContextLines)
        zoekt-result-renderer.ts             # output_mode 3종 렌더 + 페이지네이션 푸터
        index-lifecycle.ts                   # 빌드 lock, lazy build, 작업트리 staleness 검사, 메타파일
        index-watcher.ts                     # fs watch + 디바운스 → 자동 증분 재색인 트리거 (결정 8)
        index-storage.ts                     # 인덱스 경로 <repo>/.scout/zoekt (shards/ + meta.json) — 고정·설정 불가
      symbol/
        symbol-provider.ts                   # 인터페이스 + ctags 구현
        ctags-runner.ts                      # ctags --output-format=json 스코프 실행·파싱
      read/
        read-file.ts                         # Read 모사(offset/limit, 256KB 캡, dedup, system-reminder)
        find-files.ts                        # Glob 모사(picomatch, mtime 정렬, 100 절단)
        line-numbering.ts                    # compact <번호><TAB><내용> 직렬화
        read-state-store.ts                  # file_unchanged dedup 맵
    security/
      path-guard.ts                          # expandPath, UNC 거부, cwd 경계, 차단 디바이스, 바이너리 확장자
    config/
      defaults.ts                            # 튜닝 상수(타임아웃·제외목록·출력 기본값·바이너리/릴리스 식별자·.scout 경로명)
      scout-config.ts                        # TOML 설정 로더/검증/병합(repo>global>default), ResolvedScoutConfig
      gitignore-excludes.ts                  # repo .gitignore의 디렉터리-이름 추출(index 제외 union)
```

`package.json` 의존성: `@modelcontextprotocol/sdk`, `smol-toml`(TOML 설정 파싱), `zod`(MCP 도구 입력 검증). glob 라이브러리(globby/fast-glob)는 read 계층 구현 시 추가 예정. devDep `@repo/typescript-config`(workspace:*), tsx, typescript 6, @types/node. pnpm workspace(`apps/*`)에 자동 포함, turbo build/dev/check-types 태스크 상속.

---

## 8. 솔직한 위험과 반론

- **zoekt 설치 장벽이 최대 채택 리스크**: `go install`은 Go 툴체인 필요. 폴백 금지(결정 1)이므로 Go 미설치 사용자는 못 쓴다 — 결정의 결과이므로 안내를 최대한 친절히.
- **webserver lifecycle이 새 복잡도**(결정 5의 대가): 포트 충돌, 좀비 프로세스, MCP 비정상 종료 시 자식 누수, 크래시 복구 — 모두 명시 처리 필요. CLI shell-out보다 코드량·실패 모드가 많지만, 웜 서버 재사용으로 빈번 질의 성능은 우수.
- **첫 색인 워밍업 latency가 첫 질의 경험을 지배**: 대형 모노레포 첫 색인은 수십 초~분. lazy build + lock + 백그라운드 재색인으로 완화하나 첫 호출은 느림. 진행 신호 제공 검토.
- **작업 트리 신선도의 비용은 전체 재색인 시간**(결정 8의 대가): stock zoekt는 작업 트리 파일 델타가 없어 변경 시 `zoekt-index` 전체 재색인이 필요하다. 디바운스 + 무변경 skip + watcher로 자동·무감하게 만들지만, 대형 모노레포에선 재색인이 무거울 수 있다. 이는 측정으로 판가름하며(§6.6), 부담 확인 시에만 커밋-델타 base + 더티 오버레이를 후속 보강한다. **커밋-온리로 회피하지 않는다** — 검색이 작업 트리를 놓치는 건 거짓 음성이라 잘못된 동작이기 때문(§6.0).
- **토큰 캡 드롭**: 거대 단일 줄(minified) 파일을 `limit` 지정으로 읽으면 256KB 우회 + 토큰 캡 부재로 큰 출력 가능. zoekt 색인은 거대 파일 자연 배제하나 `read_file` 직접 호출엔 미적용.
- **라우터 없는 v1**(결정 2): 정의 vs 호출부 판단을 에이전트에 위임. description 가이드로 유도하나 에이전트가 무시하면 M3-W 오탐 재현 가능. 라우터는 후속에서 이 판단을 코드화할 여지로 남김.
- **이미지/PDF/노트북 미지원**(결정 7): Read의 완전 대체를 기대하면 갭. 코드 내비게이션 도구로서는 합리적.

---

## 9. 남은 항목 — 기본값으로 진행 (이의 시 변경)

확정 7개 외 나머지는 아래 기본값으로 진행한다. 이의가 있으면 알려주면 변경한다.

| 항목 | 기본값 | 비고 |
|---|---|---|
| 앱 이름 | `@buyong-mcp/scout`, bin `scout` | |
| 색인 저장 위치 | 레포-로컬 `<repo>/.scout/zoekt/` | `.git/info/exclude` 등록으로 숨김(§6.2·§6.5). 관리형 바이너리만 전역 `~/.scout/bin/<tag>` |
| 명시적 `rebuild_index` 도구 | v1 제외 | staleness 자동 처리로 충분 |
| 공유 헬퍼(`binary-availability`/`files-validation`) | **복사** | acp-bridge 수정 금지 제약 — `packages/` 승격 대신 복사가 안전 |
| multiline 검색 | v1 드롭 | zoekt 줄 단위 매처 한계 |
| 로컬 토큰 캡 | 미도입(256KB 바이트 캡만) | 거대 출력 방지 필요 시 후속 |
| 테스트/린트 | 추가 안 함 | 사용자 요청 시에만 |

### 9.1 설정 모델 확정 — TOML 파일 + zod 도구 검증 (구현됨)

초기 설계의 env 토글(`SCOUT_*`) 기반 설정을 폐기하고, 사용자 승인 하에 다음을 확정·구현했다.

- **설정은 TOML 파일로만 받는다.** 우선순위는 키 단위로 `<repo>/.scout/config.toml`(1순위) → `~/.scout/config.toml`(2순위) → built-in 기본값이며, 각 키는 가장 높은 우선순위에 **존재하는** 값을 통째로 채택한다(배열도 append 아닌 per-key replace). 파서는 `smol-toml`, 로더는 `config/scout-config.ts`(`loadScoutConfig`)이고, 노출 키는 `[output]`/`[index]`/`[limits]` 세 테이블로 한정한다(릴리스 태그·바이너리 이름·경로·server name 등은 `defaults.ts` 상수로 고정해 노출하지 않음).
- **env는 설정 수단이 아니다.** `SCOUT_BIN_DIR`/`SCOUT_INDEX_DIR`/`SCOUT_GLOB_*`는 모두 제거했다. 단 OS 실행 모델 env(`PATH`/`GOBIN`/`GOPATH`/`PATHEXT`/`SystemRoot`/`windir`)는 바이너리 탐색용으로 그대로 둔다.
- **never-exit 검증 철학.** 깨진/알 수 없는 키·타입 오류는 stderr 한국어 경고 후 해당 값만 무시하고 기본값으로 진행하며 절대 `process.exit` 하지 않는다(형제 앱의 throw 방식과 의도적으로 다름). `register_git_exclude`는 전역 설정에서만 적용한다(repo 레이어 값은 경고 후 무시). 전역 `~/.scout/config.toml`은 없으면 주석 처리된 템플릿을 자동 생성하나, repo 설정은 opt-in(읽기 전용)이다.
- **MCP 도구 인자 검증을 zod로 전환했다.** 저수준 `Server` + `setRequestHandler` + 수작업 인자 파싱(`tools/arguments.ts`, 삭제됨)을 버리고, 고수준 `McpServer.registerTool(name, config, cb)` + zod raw shape로 등록한다. SDK가 인자를 zod로 검증하고 ListTools용 JSON Schema를 자동 생성하므로, 스키마와 검증 로직의 이중 관리가 사라진다.

---

## 10. 구현 현황 (v1 진행)

### 구현 완료 + 실측 검증

- **스캐폴딩**: `package.json`(`@buyong-mcp/scout`)·`tsconfig.json`·`src/index.ts`(엔트리+종료 훅). 전체 레포 `check-types` 통과.
- **Startup 점검 + 저하 모드 + 자동 설치**(`startup/`): zoekt-index·zoekt-webserver·Universal Ctags 점검, `~/go/bin`/`$(go env GOPATH)/bin`/관리형 bin 폴백 탐색. 누락 시 종료하지 않고 저하 모드 부팅 → `install_binaries` 도구가 핀 태그(`v0.0.3`, 6종 플랫폼 전부 제공) 릴리스에서 플랫폼 자산을 받아 **SHA-256 검증** 후 설치(`tar --strip-components=1`, `universal-ctags`→`ctags` rename). darwin/arm64에서 degraded→install→search E2E 실측 검증.
- **TextSearchProvider**(`providers/text-search/`): 작업 트리 lazy 색인(+무변경 skip+빌드 lock) → zoekt-webserver 자식 lifecycle(임의 포트·헬스·종료·크래시 1회 재기동) → JSON 질의 → output_mode 3종 렌더.
- **`search_text` 도구**: MCP `tools/list`/`tools/call` 등록. stdio 핸드셰이크로 노출 확인.
- **E2E 스모크**: files_with_matches·content(줄번호·페이지네이션)·count·path/type 스코프·no-match 전부 동작.
- **구(phrase) 검색 수정**: 패턴 내 미이스케이프 공백을 `\ `로 escape해 zoekt 파서의 AND 분할을 막고 RE2 정규식 의미 보존(실측 확정).
- **공통 읽기 권한 게이트**(`security/read-guard.ts`): UNC 거부 → `expandPath` → 차단 디바이스 경로 거부 → realpath 기준 cwd 경계 검사(형제 앱 경계 로직 복사). read_file 전용 파일-종류 게이트(이미지/PDF·노트북/바이너리 거부, SVG 텍스트 허용).
- **`read_file` 도구**(`providers/read/{read-file,line-numbering,read-state-store}.ts`): cat -n compact 줄 포맷, offset/limit 슬라이스, 256KB 바이트 캡(limit 미지정 시에만, 초과 시 throw), file_unchanged dedup(`Math.floor(mtimeMs)`), 빈 파일·offset 초과 system-reminder 정확 문구, 10MB 기준 메모리/스트리밍 분기. 바이너리 독립으로 무조건 생성.
- **`find_files` 도구**(`providers/read/find-files.ts`): globby 백엔드(.gitignore 무시·숨김 포함), mtime 오래된 순 정렬, 100건 절단 + 정확 절단 문구, cwd 상대 POSIX 경로 출력. 바이너리 독립.
- **`lookup_symbol` 도구**(`providers/symbol/{symbol-provider,ctags-runner}.ts`): 작업 트리 walk로 소스 목록 수집 후 `-R` 대신 stdin(`-L -`)로 ctags 주입, `--output-format=json --fields=+nKsSa`(+`+l` 언어), 이름(정확/접두)·kind·언어 필터, head_limit 절단, fingerprint 키 인메모리 캐시. ctags 지연 해석(누락 시 search_text와 동일 저하 안내).
- **3종 primitive MCP 등록 + 와이어링**(`tools/index.ts`·`src/index.ts`): zod raw shape(snake_case 키·한국어 description) 등록, `ToolDependencies` 확장, ReadFileProvider/FindFilesProvider 무조건 생성·SymbolProvider ctags 지연 해석. stdio 핸드셰이크로 `tools/list`에 5종 노출 + read_file/find_files/lookup_symbol 호출 동작 확인(macOS).

### 미구현 (후속)

- 라우터 `navigate_code`(결정 2 — primitive 4종 외 라우팅은 후속).
- 백그라운드 fs watcher(`index-watcher.ts`) — 현재는 질의 시 staleness 체크(throttled)로 자동 증분을 충족. watcher는 사전적(proactive) 최적화로 후속.
- 컨텍스트 줄(`-A/-B/-C`)은 zoekt `ctx`(대칭)로 전달만 하며, content 렌더는 매치 줄 중심(컨텍스트 블록 합성은 ReadProvider 도입 후 보강).

### 알려진 한계

- 패턴의 공백만 escape한다(탭 등 다른 공백은 미처리 — JSON 패턴에 사실상 없음).
- `offset`은 webserver가 skip을 지원하지 않아 반환 결과에 클라이언트 측으로 적용(반환 집합을 넘는 offset은 빈 결과).
- staleness는 mtime 기반이라 **mtime을 보존하는 편집**(`git stash pop`, `cp -p` 등)은 놓칠 수 있다(v1 허용). 콘텐츠 해시 보강은 후속.
- 클라이언트가 응답 대기 중 stdin을 닫으면 in-flight 요청 응답은 드롭되고 즉시 종료한다(정상 종료·webserver 정리는 보장 — 실측 검증).
