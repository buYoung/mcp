# 개발 언어 품질 확인 명령

설치 위치는 `/Users/buyong/.local/bin/codemap-search`다. 현재 `cm`은 이 바이너리의 MCP `read`·`grep`을 호출한다. `overview`는 CLI의 `codemap`, 파일 파싱은 `parse`로도 확인할 수 있다. MCP `search`의 상세 출력과 CLI `search`의 파일 목록 출력은 서로 다르다.

## 한 명령으로 자동 검증

소스 저장소의 `apps/codemap-search`에서 `./verify`를 실행한다. Python 3.10 이상이 필요하다. 다른 작업 디렉터리에서는 `apps/codemap-search/verify`처럼 경로로 실행할 수 있으며, 내부 작업 디렉터리는 항상 패키지 루트로 맞춘다.

```sh
cd /Users/buyong/workspace/private/buyong-mcp/apps/codemap-search

./verify                         # 현재 코드 빌드 + 25개 언어의 작은 검증
./verify --language rust --language typescript --profile structural
./verify test                    # 기존 cargo check + cargo test

./verify public --language python --repository django/django
./verify public --language rust --language go --jobs 2
./verify public --language dart  # 공개 저장소와 파서 준비도 자동 실행
./verify public --dry-run        # 실행 예정 명령만 확인

# 앞선 공개 검증 캐시로 다시 실행: 준비가 완료된 경우에만 --skip-prepare 사용
./verify public --language python --repository django/django \
  --cache /Users/buyong/tmp/codemap-public-validation --skip-prepare

# 현재 소스 빌드 대신 설치한 바이너리와 비교
./verify --binary /Users/buyong/.local/bin/codemap-search
```

| 선택 사항 | 적용 |
| --- | --- |
| `--language` | `quick`·`public`의 대상 언어. 반복 지정 가능. Rust·Go도 같은 진입점으로 실행한다. |
| `--profile default` / `structural` | 생략하면 두 설정 모두. `test`에는 적용하지 않는다. |
| `--repository owner/repository` | `public`에서 고정된 후보 중 선택. 다른 언어의 저장소나 오타는 실행 전에 거절한다. |
| `--cache` | 결과·공개 저장소 캐시. 기본 `~/.cache/codemap-public-validation`; `CODEMAP_VALIDATION_CACHE` 환경 변수로도 지정한다. |
| `--binary` | 이 실행 파일을 사용하며 자동 빌드를 생략한다. 기본 실행은 Cargo가 알려 준 빌드 결과를 사용해 이전 설치본을 검증하는 일을 방지한다. |
| `--jobs 1` … `4` | Rust·Go를 포함한 공개 저장소 준비·검증의 동시 작업 수. 기본 2이며, 1이면 순차 실행한다. |
| `--skip-prepare` | 준비된 공개 저장소 측정·파서 캐시를 그대로 사용한다. 검증 자체를 생략하는 옵션이 아니다. |
| `--dry-run` | 빌드·다운로드·결과 파일 생성 없이 실제 실행할 명령을 표시한다. |

결과는 `<cache>/runs/verify-<실행 ID>/summary.json`에 합산하고 단계별 로그와 하위 실행 결과 경로를 함께 기록한다. 검사할 바이너리는 사본과 SHA-256을 보존해 실행 도중 다른 빌드로 바뀌지 않게 한다. `quick`·`public`은 통과·실패·판정 보류와 실패 사례 이름을 출력한다. `test`의 실제 테스트 개수는 Cargo 로그에서 확인한다. 모든 실행은 새 결과 디렉터리를 사용한다.

병렬 단위는 저장소다. 각 저장소는 별도 작업 트리·색인·MCP 프로세스를 사용하며, 그 안에서는 설정별 검사와 파일 생성·수정·삭제 검사를 순차로 진행한다. Rust·Go 단계와 나머지 언어 단계는 기존 순서로 실행하며, 두 단계 모두 같은 `--jobs` 값을 적용한다. `quick`과 작은 회귀 검사는 순차 실행을 유지한다. Rust·Go 통합 결과는 동시에 덮어쓰지 않도록 보호하며, 실패한 작업은 오류로 기록하고 완료한 검증 결과를 보존한다.

검사 실패·판정 보류·실행 오류가 있으면 종료 코드는 0이 아니다. 작은 대조의 현재 결과는 522항목 통과이며, 공개 검증의 알려진 실패·보류도 그대로 드러낸다. `quick`은 공개 저장소나 별도 언어 파서를 준비하지 않으며 Git과 검사할 바이너리만 사용한다. 기본 자동 빌드에는 Rust/Cargo와 프로젝트 의존성이 필요하다. `public`은 선택한 언어에 따라 `tokei`, `go`, `rust-analyzer`, `ctags`, Node/TypeScript, Swift 또는 Java/Dart 파서 도우미가 필요하다. Dart·Scala·Groovy 도우미는 함께 준비하며, 고정 lockfile과 맞지 않는 의존성은 실패로 처리하고 저장소의 lockfile을 바꾸지 않는다.

아래의 `cm` 예시는 개별 응답을 직접 확인할 때 사용한다.

## 원래 설정 로딩 사례

```sh
cd /Users/buyong/workspace/private/buyong-mcp/apps/codemap-search

cm grep '{"path":"src/config.rs","pattern":"pub fn load\\(","head_limit":5}'
cm read '{"file_path":"src/config.rs","offset":391,"limit":8}'

cm grep '{"path":"src/config.rs","pattern":"CODEMAP_DIR_NAME|CONFIG_FILE_NAME","head_limit":20}'
cm read '{"file_path":"src/config.rs","offset":402,"limit":20}'
```

`load`의 호출 목록에는 `read_layer — …/src/config.rs:393`, `merge — …/src/config.rs:657`, `canonicalize_path_lenient — …/src/workspace.rs:226`이 나온다. 두 상수의 정의는 84·86줄이고 값은 각각 `".codemap"`, `"config.toml"`이다. `join`은 정의 링크 없이 `unresolved`로 나온다. 이후 소스가 변경돼 줄이 이동하면 첫 `grep`으로 현재 위치부터 확인한다.

## 문자열·주석·미확정 수신 객체

아래 대조 작업 트리는 이번 검증에서 만들었다. 큰 저장소 없이 바로 확인할 수 있다.

```sh
cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/typescript-structural/worktree

# 호출자와 상수 참조
cm read '{"file_path":"_codemap_language_probe/probe.ts","offset":2,"limit":1}'

# 확인된 호출 대상: probe.ts:2
cm read '{"file_path":"_codemap_language_probe/probe.ts","offset":3,"limit":1}'

# 수신 객체를 확정할 수 없는 호출: unresolved, 정의 링크와 precise 표시 없음
cm read '{"file_path":"_codemap_language_probe/probe.ts","offset":6,"limit":1}'

# 원문 검색에서는 문자열·주석도 보여야 한다
cm grep '{"path":"_codemap_language_probe/probe.ts","pattern":"cm_validation_target","head_limit":20}'
```

첫 `read`의 호출자는 3줄의 `cm_validation_caller`다. 4줄의 문자열, 5줄의 주석, 6줄의 미확정 객체 호출을 이 정의의 호출자로 연결하면 오류다. 반면 마지막 `grep`의 원문 결과에 4·5줄이 포함되는 것은 정상이다.

## 제외 규칙과 명시적 우회

위 TypeScript 작업 트리에서 이어서 실행한다.

```sh
cm grep '{"path":"_codemap_language_probe/node_modules","pattern":"cm_validation_target","output_mode":"files_with_matches"}'
cm grep '{"path":"_codemap_language_probe/_validation_excluded","pattern":"cm_validation_target","output_mode":"files_with_matches"}'
cm grep '{"path":"_codemap_language_probe/ignored","pattern":"cm_validation_target","output_mode":"files_with_matches"}'

cm grep '{"path":"_codemap_language_probe/node_modules","pattern":"cm_validation_target","output_mode":"files_with_matches","include_ignored":true}'
```

앞의 세 명령은 제외 경로를 출력하지 않아야 한다. 마지막 명령은 사용자가 우회를 명시했으므로 `node_modules/probe.ts`가 나와야 한다.

## 컴포넌트와 PowerShell

```sh
cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/vue-default/worktree
cm read '{"file_path":"_codemap_language_probe/probe.vue","offset":2,"limit":1}'
cm grep '{"path":"_codemap_language_probe/probe.vue","pattern":"cm_validation_target","head_limit":20}'

cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/powershell-default/worktree
cm read '{"file_path":"_codemap_language_probe/probe.ps1","offset":2,"limit":1}'
cm read '{"file_path":"_codemap_language_probe/probe.ps1","offset":9,"limit":1}'
codemap-search parse _codemap_language_probe/probe.ps1
```

Vue의 호출자는 script의 3줄이며, 마크업인 5줄은 호출자로 나오지 않아야 한다. 작업 트리 폴더를 `astro-default` 또는 `svelte-default`로, 파일 확장자를 `astro` 또는 `svelte`로 바꾸면 같은 2·3·5줄 구조로 비교할 수 있다. 실제 파일은 각 `worktree/_codemap_language_probe/` 아래에 있다.

PowerShell의 함수 종류는 `fn`이며, 9줄의 객체 호출은 `unresolved`로 표시한다. `parse` 결과의 전역 변수 범위는 파일 전체가 아닌 1줄이다.

## 선언·검색 출력과 상수 정의·값

```sh
cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/typescript-structural/worktree
codemap-search codemap --path _codemap_language_probe/probe.ts
codemap-search search 'cm_validation_target' --limit 5

cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/dart-default/worktree
cm read '{"file_path":"_codemap_language_probe/probe.dart","offset":2,"limit":1}'

cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/csharp-default/worktree
cm read '{"file_path":"_codemap_language_probe/probe.cs","offset":3,"limit":1}'
```

Dart의 참조 목록에는 `CM_VALIDATION_LIMIT — _codemap_language_probe/probe.dart:1 = 7`, C#에는 `CM_VALIDATION_LIMIT — _codemap_language_probe/probe.cs:2 = 7`이 나와야 한다. C# 클래스 멤버 목록의 이름과 별개로 참조 목록의 정의 위치·값까지 확인한다. 같은 수정 범위는 Java·PHP·Ruby·Kotlin·Swift·Scala·Groovy·C·C++이며, 전체 작은 대조에서 두 설정 모두 통과했다.

MCP의 실제 `overview`·`search`·`find` 계약을 직접 호출하려면 저장소 루트에서 다음과 같이 실행한다. `query` 뒤 도구 이름과 JSON만 바꿀 수 있다.

```sh
cd /Users/buyong/workspace/private/buyong-mcp
python3 apps/codemap-search/scripts/public_validation.py \
  --binary /Users/buyong/.local/bin/codemap-search query \
  --root /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/typescript-structural/worktree \
  search '{"query":"cm_validation_caller","workspace_scope":"all","caller_context":true}'
```

`query`는 새 MCP 연결의 색인 준비를 기다리지 않는다. 처음 실행해 준비 중이라는 응답이 나오면 이를 검색 실패로 판정하지 말고 준비 후 다시 호출한다. 자동 공개 검증은 색인 준비를 기다린 뒤 검사한다.

## C·NASM 매크로 생성 선언

다음 디렉터리는 이번 검증에서 만든 대조 파일·설정·컴파일 데이터베이스를 보존한다. 설치한 Clang과 검증용 NASM 2.16.03 경로가 설정돼 있다. 자기 저장소에서 사용하려면 [매크로 설정](configuration.ko.md#매크로-확장)의 빌드 문맥과 실행 파일 경로를 맞춘다. 매크로 확장의 기본값은 꺼짐이다.

```sh
cd /Users/buyong/tmp/cm-remaining/macro-manual

# DECLARE(chosen) -> chosen_expanded, 선언 위치는 원본 2줄
cm read '{"file_path":"src/main.c","offset":2,"limit":2}'
cm grep '{"path":"src/main.c","pattern":"DECLARE","head_limit":5}'

# INTERNAL이 static으로 확장되므로 helper는 not exported
cm read '{"file_path":"src/visibility.c","offset":1,"limit":2}'

# FFmpeg x86inc.asm의 cglobal -> ff_cm_public_probe, 원본 3줄
cm read '{"file_path":"src/public.asm","offset":1,"limit":4}'

# 구조화된 확장 상태·입력 의존성과 선언 좌표
codemap-search parse src/main.c
codemap-search parse src/public.asm
```

확장된 선언에는 `macro expansion` 표식이 나온다. 원문을 보여 주는 `# results`에는 `DECLARE(chosen)`·`cglobal cm_public_probe`가 그대로 있어야 한다. 전처리 파일은 호출·상수 참조를 정의에 연결하지 않고 미해결 안내를 낸다. 생성 토큰의 열 좌표를 `(precise)`로 표시하면 오류다.

## 비 UTF-8 입력과 동명 파일

```sh
cd /Users/buyong/tmp/codemap-public-validation/runs/verify-20260913T140647072947-93020-languages/asm-gnutools__glibc/worktree

# read는 대체 문자를 허용하되 UTF-8 색인 제외 이유를 함께 표시
cm read '{"file_path":"sysdeps/i386/fpu/e_atanhl.S","offset":1,"limit":12}'
codemap-search codemap --path sysdeps/i386/fpu/e_atanhl.S

# 아키텍처까지 좁혀 실제 ENTRY 위치 확인
cm grep '{"path":"sysdeps/unix/sysv/linux/sparc/sparc64/____longjmp_chk.S","pattern":"ENTRY","head_limit":10}'
codemap-search search 'ENTRY sysdeps/unix/sysv/linux/sparc/sparc64/____longjmp_chk.S' --limit 10
```

`ENTRY ____longjmp_chk`만 사용하면 여러 아키텍처의 동명 파일이 경쟁해 특정 파일이 출력 한도 밖으로 밀릴 수 있다. 원래 넓은 질의의 실패 기록과 경로를 좁힌 MCP 질의의 성공 기록은 모두 보존했다.

전체 대상과 결과 해석은 [검증 보고서](development-language-validation.ko.md), 남은 판정 보류·지원 범위는 [이슈 목록](../validation/development-issues.json)에 있다. Groovy 공개 대조는 254개 모두 통과했고 Zsh 무한 처리는 수정했지만 나머지 문법 보류가 있다. Azure PowerShell의 일반 코드 첫 문맥은 이번 환경에서 약 3.6~3.7초였으며 새 색인 준비에는 약 8~9분이 들었다. 기존 MCP 연결은 프로세스를 다시 시작해야 설치한 새 바이너리를 사용하고, `cm`은 명령마다 새 프로세스를 실행한다.


## 원문·정의·관계와 함수 단위 읽기

다음 명령은 이 브리프에서 구현한 실시간 출력 제어를 확인합니다. 먼저 grep으로 현재 위치를 확인하면 줄 이동에도 대응할 수 있습니다.

```sh
cd /Users/buyong/workspace/private/buyong-mcp/apps/codemap-search
cm read '{"file_path":"src/config.rs","offset":391,"limit":8,"view":"source"}'
cm read '{"file_path":"src/config.rs","offset":391,"limit":8,"view":"definitions"}'
cm read '{"file_path":"src/config.rs","offset":391,"limit":8,"view":"relations","unresolved":"count"}'
cm read '{"file_path":"src/config.rs","offset":392,"expand":"callable","view":"source"}'
cm grep '{"path":"src/config.rs","pattern":"pub fn load","expand":"callable","view":"source","head_limit":2}'
```

`expand=callable`의 grep 페이지는 중복을 제거한 함수 묶음 단위입니다. 함수가 여러 개면 응답의 next_offset으로 이어 갑니다. 큰 함수는 안내된 줄 범위에 `expand=none`을 넣어 나눠 읽습니다. 옵션 전체 계약은 [요청별 출력 제어](configuration.ko.md#readgrep-요청별-출력-제어)에 있습니다.


## 이벤트 탐색과 통합 회귀 검증

이벤트 탐색은 기본으로 켜져 있으며 `include_events`를 생략해도 관련 이벤트가 자동으로 표시됩니다. 실제 앱을 바꾸지 않고 확인하려면 이번 작업에서 만든 예제 복사본을 사용하세요. 원본 예제와 사용자 규칙은 `tests/fixtures/event_navigation/`에 있습니다. 복사본에는 `KnownBus`의 정확한 정의 경로를 사용한 규칙이 들어 있습니다. 기존 설정에 `is_enabled=false`를 명시했다면 그 값은 유지됩니다.

```sh
cd /Users/buyong/tmp/cm-nav/event-cli-delivered

# 발행 위치에서 구독 등록·핸들러·상수 정의를 함께 확인
cm read '{"file_path":"src/users.ts","offset":2,"limit":1,"view":"relations"}'

# 등록 위치에서 반대편 발행 위치 확인
cm grep '{"path":"src/events.ts","pattern":"appBus\\.on","view":"relations"}'

# 원문을 보존하면서 이벤트 관계 추가
cm read '{"file_path":"src/users.ts","offset":2,"expand":"callable"}'

# 이번 요청에서만 이벤트 관계 생략
cm read '{"file_path":"src/users.ts","offset":2,"expand":"callable","include_events":false}'

# source 모드는 include_events=true여도 이벤트 조회·출력을 생략
cm read '{"file_path":"src/users.ts","offset":2,"expand":"callable","view":"source","include_events":true}'

# 불투명한 버스·동적 키·불명확한 핸들러·once·제거 조건
cm read '{"file_path":"src/controls.ts","offset":3,"limit":8,"view":"relations"}'

# 문자열과 주석만 있는 구간은 이벤트 섹션과 빈 결과 안내를 생략
cm read '{"file_path":"src/controls.ts","offset":12,"limit":2,"view":"relations"}'
```

같은 키를 쓰는 `otherBus`는 별도 할당이므로 위 정방향·역방향 관계에 합쳐지면 안 됩니다. 상수 근거는 `SAVED — src/events.ts:4`, 구독은 `src/events.ts:6`, 핸들러는 `handleSaved — src/events.ts:5`, 발행은 `src/users.ts:2`로 표시되어야 합니다.

MCP `search`에서는 `{"query":"saved","event_key":"saved"}`로 정확한 키의 전체 지도를 보거나 `{"query":"save users","caller_context":false}`로 직접 호출 문맥 없이 관련 이벤트를 자동으로 볼 수 있습니다. `cm`은 read/grep 명령을 제공하므로 이 요청은 MCP search에서 실행합니다. 작업공간 범위는 기존 `workspace_scope`를 따릅니다. 일반 코드는 이벤트가 없을 때 출력 공간을 그대로 사용하며, 인식된 미해결 이벤트는 기존 사유를 유지합니다.

반복 가능한 회귀 검증은 패키지 디렉터리에서 실행합니다.

```sh
cd /Users/buyong/workspace/private/buyong-mcp/apps/codemap-search
cargo test --locked --test e2e_tests test_live_
cargo test --locked --test e2e_tests test_rust_target_reload_and_reexport_identity_reach_all_tool_views
cargo test --locked --test e2e_tests test_events_
cargo test --locked --lib events::
```

검증에서만 쓰는 임시 앱의 생성·편집·삭제, import한 버스·키의 변경, 규칙·대상·Git/디렉터리 제외 설정의 재로드, 테스트 코드 포함 전환, 재시작을 확인합니다. 원본 Corral에는 이 예제 설정을 적용하지 않았습니다. Corral에서 확인한 사용자 규칙의 `platform:window-op` 연결은 명시적으로 선언한 버스 가정이며 실제 이벤트 전달을 보장하지 않습니다. 자세한 근거와 남은 한계는 저장소 루트의 `docs/briefs/evidence/codemap-nav/event-map.json`과 `integration.json`에 기록합니다.

## 추상 선언과 구현 후보

검증용 예제는 `/Users/buyong/tmp/cm-nav/implementations`에 보존합니다. 별도 기능 설정 없이 선언→구현, 구현→선언, 호출→후보를 확인합니다.

```sh
cd /Users/buyong/tmp/cm-nav/implementations

# 추상 선언 → EmailSender.send·SmsSender.send의 정의 위치
cm read '{"file_path":"src/sender.ts","offset":2,"limit":1}'

# 구현 → Sender.send 선언 위치
cm read '{"file_path":"src/email.ts","offset":3,"limit":1,"view":"relations"}'

# 타입이 있는 호출 → 선언·구현 후보, 실제 실행 대상은 unresolved
cm grep '{"path":"src/notify.ts","pattern":"sender\\.send","view":"relations"}'

# 문자열 또는 타입을 알 수 없는 호출만 읽으면 구현 섹션 없음
cm read '{"file_path":"src/notify.ts","offset":5,"limit":1}'
cm read '{"file_path":"src/notify.ts","offset":6,"limit":1}'

# 관계 없이 원문 확인
cm read '{"file_path":"src/sender.ts","offset":1,"limit":3,"view":"source"}'
```

지원 문법, 자동 표시 조건과 미해결 범위는 [추상 선언·구현 탐색](implementation-navigation.ko.md)에 정리했습니다. 이 기능의 언어별 회귀 검증은 `cargo test --locked --lib implementations::tests::`, 실제 MCP 검증은 `cargo test --locked --test e2e_tests test_implementation_`로 다시 실행할 수 있습니다.
