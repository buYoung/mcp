# 개발 언어 품질 확인 명령

설치 위치는 `/Users/buyong/.local/bin/codemap-search`다. 현재 `cm`은 이 바이너리의 MCP `read`·`grep`을 호출한다. `overview`는 CLI의 `codemap`, 파일 파싱은 `parse`로도 확인할 수 있다. MCP `search`의 상세 출력과 CLI `search`의 파일 목록 출력은 서로 다르다.

## 원래 설정 로딩 사례

```sh
cd /Users/buyong/workspace/private/buyong-mcp/apps/codemap-search

cm grep '{"path":"src/config.rs","pattern":"pub fn load\\(","head_limit":5}'
cm read '{"file_path":"src/config.rs","offset":377,"limit":8}'

cm grep '{"path":"src/config.rs","pattern":"CODEMAP_DIR_NAME|CONFIG_FILE_NAME","head_limit":20}'
cm read '{"file_path":"src/config.rs","offset":388,"limit":20}'
```

`load`의 호출 목록에는 `read_layer — …/src/config.rs:388`, `merge — …/src/config.rs:651`, `canonicalize_path_lenient — …/src/workspace.rs:226`이 나온다. 두 상수의 정의는 82·84줄이고 값은 각각 `".codemap"`, `"config.toml"`이다. `join`은 정의 링크 없이 `unresolved`로 나온다. 이후 소스가 변경돼 줄이 이동하면 첫 `grep`으로 현재 위치부터 확인한다.

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

## 선언·검색 출력과 남은 상수 누락

```sh
cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/typescript-structural/worktree
codemap-search codemap --path _codemap_language_probe/probe.ts
codemap-search search 'cm_validation_target' --limit 5

cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/dart-default/worktree
cm read '{"file_path":"_codemap_language_probe/probe.dart","offset":2,"limit":1}'

cd /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/csharp-default/worktree
cm read '{"file_path":"_codemap_language_probe/probe.cs","offset":3,"limit":1}'
```

Dart와 C#의 마지막 두 사례는 현재 남은 **상수 참조 문맥 누락**을 확인하는 명령이다. C# 클래스 멤버 목록에 상수 이름이 표시돼도, 참조 목록의 정의 위치·값이 제공되는 것과는 다르다.

MCP의 실제 `overview`·`search`·`find` 계약을 직접 호출하려면 저장소 루트에서 다음과 같이 실행한다. `query` 뒤 도구 이름과 JSON만 바꿀 수 있다.

```sh
cd /Users/buyong/workspace/private/buyong-mcp
python3 apps/codemap-search/scripts/public_validation.py \
  --binary /Users/buyong/.local/bin/codemap-search query \
  --root /Users/buyong/tmp/codemap-public-validation/runs/development-constant-probes-20260913/typescript-structural/worktree \
  search '{"query":"cm_validation_caller","workspace_scope":"all","caller_context":true}'
```

전체 대상, 결과 해석과 자동 재실행 명령은 [검증 보고서](development-language-validation.ko.md)에 있다. 대형 공개 저장소의 작업 트리는 해당 자동 검사가 끝난 뒤 사용한다. Azure PowerShell은 첫 자동 문맥 요청이 오래 걸리고 Zinit은 파서 시간 초과가 남아 있으므로, 빠른 수동 비교에는 위 작은 대조 작업 트리를 사용한다.
