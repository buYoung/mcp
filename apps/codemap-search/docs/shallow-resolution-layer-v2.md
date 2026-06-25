# Tree-sitter Navigation Layer v2

> 상태: 구현 설계 · 대상 독자: code-agent · 범위: caller/callee 귀속 정밀도 개선
>
> 목표: 언어별 tree-sitter navigation query로 호출/참조/import/scope 관측치를 뽑고, Rust 후보 축소 정책으로 1개 후보가 확정될 때만 정밀 귀속한다. 확정할 수 없으면 기존 name-match + `approximate` 동작으로 폴백한다.
>
> 주 사용 목적: 함수 변경 시 부작용 확인에 필요한 영향면을 더 정확히 보여준다. caller는 "이 함수를 바꾸면 누가 영향을 받는가"를, callee는 "이 함수가 내부에서 무엇에 의존하는가"를 보여준다.
>
> 1차 이득: call site 관측 자체를 정확히 만든다. 문자열, 주석, 정의 헤더, 단순 텍스트 매칭 오탐을 줄이는 것이 먼저이고, call → definition precise 귀속은 metrics로 효과가 확인된 범위에서만 켠다.

---

## 1. 구현 목표

현재 심볼 추출은 tree-sitter query 기반이지만, caller/callee 탐색은 텍스트 기반이다.

- `src/parser/mod.rs`: tree-sitter query로 정의 심볼과 literal을 추출한다.
- `src/callers/scan.rs`: 함수명 목록으로 정규식을 만들고 workspace를 한 번 훑는다.
- `src/callers/callees.rs`: 함수 본문 텍스트에서 `identifier(` 꼴을 직접 찾는다.

v2의 목표는 caller/callee 입력을 tree-sitter 기반 관측치로 바꾸는 것이다. query는 구조를 추출하고, Rust 로직은 그 구조를 이용해 후보를 줄인다.

저장과 해석은 분리한다.

- `NavigationFile`에는 파일 단위 raw 관측치만 저장한다.
- 실제 caller/callee 귀속은 query 시점의 최신 snapshot에 대해 수행한다.
- 파일 A의 call site가 파일 B의 정의를 가리키는지는 파일 B의 현재 심볼 상태에 따라 달라질 수 있으므로, 파일 A 추출 시점에 콜그래프로 고정하지 않는다.

---

## 2. 핵심 원칙

1. **확정 가능한 경우에만 정밀 귀속**
   - 후보가 정확히 1개로 좁혀질 때만 `Precise`로 처리한다.
   - 후보가 0개 또는 2개 이상이면 기존 name-match 결과와 `approximate` 라벨을 유지한다.
   - 후보 1개 판정은 kind/export/status/budget 조건을 모두 통과해야 한다. 단순 후보 수만으로 `Precise`를 붙이지 않는다.

2. **파일 단위 계산**
   - import, local definition, call/reference, receiver 관측치는 파일 하나의 소스 텍스트에서 계산한다.
   - 기존 watcher의 파일 단위 증분 모델을 유지한다.

3. **언어별 문법은 query와 `LanguageSpec`에 둔다**
   - 공통 귀속 알고리즘은 언어 중립으로 둔다.
   - 언어별 차이는 `queries/<language>/navigation.scm`과 `src/lang/*.rs` 훅으로 제한한다.
   - 새 언어를 켤 때는 표준 tree-sitter tags 규약 또는 tags-compatible query를 직접 구성하고 fixture에서 매칭을 확인한 뒤 활성화한다.

4. **query 파일 분리 기준**
   - `.scm` 파일은 query 가독성, editor tooling, 심볼 추출 query와의 목적 분리를 위해 사용한다.
   - `.scm` 파일은 `include_str!`로 바이너리에 포함한다.
   - 배포된 바이너리가 query 파일을 런타임에 찾지 않아도 동작해야 한다.

5. **기존 실패 모델 유지**
   - query compile 실패는 개발 단계에서 잡는다.
   - 파일별 parse 실패, 예외적 문법, 모듈 해석 실패는 응답 실패가 아니라 폴백으로 처리한다.

6. **영속 포맷 변경은 재인덱싱 강제**
   - `ExtractedFile`에 navigation 필드를 추가하거나 navigation 직렬화 구조를 바꾸면 `EXTRACTION_FORMAT_VERSION`을 올린다.
   - `#[serde(default)]`는 이전 JSON 역직렬화 호환용이며, 기존 인덱스에 navigation 데이터를 채우지는 않는다.
   - 버전 범프로 sidecar 불일치를 만들고 일회성 재인덱싱을 강제한다.

---

## 3. Query 파일 계약

언어별 query는 Rust 문자열 상수에서 `.scm` 파일로 분리하고 `include_str!`로 바이너리에 포함한다. 런타임에 query 파일 경로를 찾지 않는다.

```text
src/lang/
  typescript.rs
queries/
  typescript/
    symbols.scm
    tags.scm
    navigation.scm
```

`symbols.scm`은 현재 Rust 코드에 인라인 문자열로 들어 있는 기존 `@symbol.*`/`@literal.*` 추출 query를 옮긴 파일이다. 기존 심볼 추출 동작은 유지하고, 파일 위치만 바꾼다.

단계 0 분리 대상은 `src/lang/` 아래 독립적인 인라인 query 상수를 갖는 모든 언어이다. 일반 규칙: **모든 `src/lang/*.rs` 및 `src/lang/c_family/*.rs`의 인라인 query 상수를 동일한 규칙으로 `queries/<language>/symbols.scm`으로 분리한다.** 9개 언어의 경로 목록:

```text
queries/
  typescript/symbols.scm   (TS_QUERY_STR)
  python/symbols.scm       (PYTHON_QUERY_STR)
  go/symbols.scm           (GO_QUERY_STR)
  rust/symbols.scm         (RUST_QUERY_STR)
  java/symbols.scm         (JAVA_QUERY_STR)
  kotlin/symbols.scm       (KOTLIN_QUERY_STR)
  c/symbols.scm            (C_QUERY_STR)
  cpp/symbols.scm          (CPP_QUERY_STR)
  asm/symbols.scm          (ASM_QUERY_STR)
```

> **ASM 처리**: `asm/symbols.scm`은 단계 0 분리 대상에 포함된다. 단, ASM은 call site 캡처 불가(§8 단계 8 참조)로 `tags.scm`/`navigation.scm` 작성 대상이 아니다. `queries/asm/` 디렉토리는 `symbols.scm`만 포함하고, navigation layer 활성화 대상에서 제외한다.

`tags.scm`은 언어별 기본 매칭 검증 계약이다. 런타임 필수 pass가 아니라, 설정한 언어와 grammar에서 정의/참조 매칭이 실제 node shape와 맞는지 확인하는 gate로 사용한다.

`navigation.scm`은 caller/callee 귀속에 필요한 추가 관측치 계약이다.

런타임 인덱싱에서는 pass 수를 늘리지 않는다. 구현은 파일을 분리하되 `symbols.scm`과 필요한 `navigation.scm`을 concat해 하나의 `Query::new(...)`와 하나의 `QueryCursor` pass로 처리한다. `tags.scm`은 검증 gate로 사용하고 런타임 pass에는 포함하지 않는다.

| 파일 | 역할 |
|------|------|
| `symbols.scm` | 기존 심볼/literal 추출. 런타임 사용, `include_str!`로 포함 |
| `tags.scm` | 표준 tags-compatible 정의/참조 매칭 검증. 언어 활성화 전 fixture로 확인 |
| `navigation.scm` | call, receiver, import, local, scope 등 귀속용 추가 관측치 |

`symbols.scm` 캡처 이름은 기존 parser 계약을 유지한다.

```scheme
@symbol.name
@symbol.fn
@symbol.method
@symbol.class
@symbol.interface
@symbol.type
@symbol.enum
@symbol.struct
@symbol.field
@symbol.variable
@literal.string
@literal.number
```

`tags.scm` 권장 캡처 이름:

```scheme
@definition.function
@definition.method
@definition.class
@definition.interface
@definition.type
@definition.constant
@definition.variable

@reference.call
@reference.identifier
```

tags 규칙:

- `LanguageSpec::extensions()`에 들어간 확장자는 해당 언어의 tags fixture를 통과해야 한다.
- `tags.scm`은 표준 tree-sitter tags 규약에 맞춘 최소 정의/참조 매칭 검증용이다. 정확한 caller/callee 귀속은 `navigation.scm`과 Rust 후보 축소 로직이 담당한다.
- tags 매칭이 불안정한 언어는 navigation layer를 활성화하지 않는다. 기존 symbol extraction과 name-match 폴백만 사용한다.
- 같은 언어가 여러 grammar를 쓰면 grammar별로 tags compile과 fixture를 나눈다. 예: TypeScript grammar와 TSX grammar.
- `tags.scm`은 런타임 인덱싱 pass를 추가하지 않는다. runtime extraction은 concat된 `symbols.scm` + 최소 `navigation.scm`만 수행한다.

`navigation.scm` 권장 캡처 이름:

```scheme
@nav.call
@nav.call.name
@nav.call.receiver

@nav.reference
@nav.reference.name

@nav.import
@nav.import.source
@nav.import.name
@nav.import.alias
@nav.import.namespace
@nav.import.default
@nav.import.glob

@local.scope
@local.definition
@local.reference
@local.type
@local.value_type
```

캡처 의미:

| 캡처 | 의미 |
|------|------|
| `nav.call` | 호출 표현식 전체 |
| `nav.call.name` | 호출되는 이름. `getUser()`, `obj.save()`의 `getUser`/`save` |
| `nav.call.receiver` | selector/member call의 receiver. `obj.save()`의 `obj` |
| `nav.reference` | 호출은 아니지만 함수 참조가 될 수 있는 식별자 |
| `nav.import.source` | import source 문자열. 예: `"./user"` |
| `nav.import.name` | 원래 export 이름 |
| `nav.import.alias` | local alias 이름 |
| `nav.import.namespace` | namespace import 이름. 예: `import * as api`의 `api` |
| `nav.import.default` | default import local 이름 |
| `nav.import.glob` | glob import 표시 |
| `local.scope` | local binding과 call을 묶을 lexical scope |
| `local.definition` | 파일 또는 함수 scope 안의 local binding 이름 |
| `local.reference` | local binding 참조 |
| `local.type` | 명시 타입 힌트. 예: `const user: User`의 `User` |
| `local.value_type` | 생성자 힌트. 예: `new User()`의 `User` |

캡처 이름은 Rust 소비 코드와의 계약이다. 새 캡처를 추가하면 fixture에서 compile과 추출 결과를 함께 고정한다.

---

## 4. Tags 구성 및 검증

지원 언어는 tags를 직접 구성하고 테스트한 뒤 navigation layer에 연결한다. 목표는 해당 언어에서 최소 정의/참조 매칭이 실제 node shape와 맞는지 구현 전에 고정하는 것이다. 이 검증은 언어 활성화 gate이며, 런타임 비용을 늘리는 필수 pass가 아니다.

언어별 구성 파일:

```text
queries/
  rust/
    symbols.scm
    tags.scm
    navigation.scm
  typescript/
    symbols.scm
    tags.scm
    navigation.scm
  python/
    symbols.scm
    tags.scm
    navigation.scm
  go/
    symbols.scm
    tags.scm
    navigation.scm
  java/
    symbols.scm
    tags.scm
    navigation.scm
  kotlin/
    symbols.scm
    tags.scm
    navigation.scm
  c/
    symbols.scm
    tags.scm
    navigation.scm
  cpp/
    symbols.scm
    tags.scm
    navigation.scm
  asm/
    symbols.scm     (단계 0 분리만. tags.scm/navigation.scm 작성 대상 아님)
```

확장 언어 grammar 이름:

| 언어 | grammar 상수 | 확장자 | 비고 |
|------|-------------|--------|------|
| Python | `tree_sitter_python::LANGUAGE` | `.py` | 단일 grammar |
| Go | `tree_sitter_go::LANGUAGE` | `.go` | 단일 grammar |
| Rust | `tree_sitter_rust::LANGUAGE` | `.rs` | 단일 grammar |
| Java | `tree_sitter_java::LANGUAGE` | `.java` | 단일 grammar |
| Kotlin | `tree_sitter_kotlin_ng::LANGUAGE` | `.kt`, `.kts` | 단일 grammar; crate 이름 `tree_sitter_kotlin_ng` (표준 `tree_sitter_kotlin`과 다름) |
| C | `tree_sitter_c::LANGUAGE` | `.c` | 단일 grammar; `.h`는 C++ grammar |
| C++ | `tree_sitter_cpp::LANGUAGE` | `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` | 단일 grammar; `.h`를 포함 |
| ASM | `tree_sitter_asm::LANGUAGE` | `.s`, `.S`, `.asm` | 단일 grammar; navigation 제외 언어 |

언어별 fixture:

```text
tests/fixtures/navigation/
  typescript/
    basic.ts
    expected.tags.json
    expected.navigation.json
  tsx/
    basic.tsx
    expected.tags.json
    expected.navigation.json
  python/
    basic.py
    expected.tags.json
    expected.navigation.json
  go/
    basic.go
    expected.tags.json
    expected.navigation.json
  rust/
    basic.rs
    expected.tags.json
    expected.navigation.json
  java/
    basic.java
    expected.tags.json
    expected.navigation.json
  kotlin/
    basic.kt
    expected.tags.json
    expected.navigation.json
  c/
    basic.c
    expected.tags.json
    expected.navigation.json
  cpp/
    basic.cpp
    expected.tags.json
    expected.navigation.json
```

검증 항목:

1. grammar별 `symbols.scm`, `tags.scm`, `navigation.scm` compile이 성공한다.
2. 설정된 확장자와 grammar가 일치한다.
3. `symbols.scm`이 기존 인라인 query와 동일한 symbol/literal 결과를 낸다.
4. `tags.scm`이 최소 정의/참조를 기대한 이름과 range로 캡처한다.
5. `navigation.scm`이 call/import/local/receiver를 기대한 이름과 range로 캡처한다.
6. 같은 fixture에서 symbols/tags/navigation 결과가 서로 모순되지 않는다.

언어 활성화 조건:

| 조건 | 기준 |
|------|------|
| compile | 해당 언어의 모든 grammar에서 `symbols.scm`, `tags.scm`, `navigation.scm` compile 성공 |
| fixture | 설정된 확장자별 fixture가 expected JSON과 일치 |
| fallback | capture 누락 또는 모호한 node shape에서 기존 동작으로 폴백 |
| coverage | 최소 function definition, method definition, call reference를 캡처 |

TypeScript/TSX처럼 하나의 spec이 여러 grammar를 쓰는 언어는 둘 다 검증한다.

```text
ts/js   -> LANGUAGE_TYPESCRIPT -> queries/typescript/tags.scm
tsx/jsx -> LANGUAGE_TSX        -> queries/typescript/tags.scm
```

query가 한 grammar에서는 compile되지만 다른 grammar에서 실패하면 해당 확장자에는 navigation layer를 켜지 않는다.

`symbols.scm` 분리는 별도 기능 변경이 아니다. 첫 변경은 Rust 문자열 query를 `.scm` 파일로 옮기고 `include_str!`로 읽게 만드는 무동작 리팩터로 끝낸다. 이 단계에서 `EXTRACTION_FORMAT_VERSION`은 올리지 않는다. 추출 결과가 바뀌는 navigation 필드를 추가하는 단계에서만 올린다.

---

## 5. Rust 자료 구조

`ExtractedFile`에 navigation 관측치를 추가한다. 기존 serialized 문서와 호환되도록 새 필드는 `#[serde(default)]`를 둔다.

```rust
pub struct ExtractedFile {
    pub file_path: String,
    pub total_lines: usize,
    pub symbols: Vec<ExtractedSymbol>,
    pub literals: Vec<ExtractedLiteral>,
    pub docstrings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<NavigationFile>,
}
```

권장 타입:

```rust
pub struct NavigationFile {
    pub imports: Vec<ImportEntry>,
    pub calls: Vec<CallSite>,
    pub locals: Vec<LocalBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ReferenceSite>,
}

pub struct ImportEntry {
    pub local_name: String,
    pub imported_name: Option<String>,
    pub source: Option<String>,
    pub kind: ImportKind,
    pub range: CodeRange,
}

pub enum ImportKind {
    Named,
    Default,
    Namespace,
    Glob,
}

pub struct CallSite {
    pub name: String,
    pub receiver: Option<String>,
    pub range: CodeRange,
    pub scope_id: Option<usize>,
}

pub struct ReferenceSite {
    pub name: String,
    pub range: CodeRange,
    pub scope_id: Option<usize>,
}

pub struct LocalBinding {
    pub name: String,
    pub type_hint: Option<String>,
    pub value_type_hint: Option<String>,
    pub range: CodeRange,
    pub scope_id: Option<usize>,
}
```

`navigation: None`은 해당 파일에서 navigation 추출이 실행되지 않았거나 실패했다는 뜻이다. `Some(NavigationFile { calls: vec![], ... })`는 추출은 성공했고 관측치가 없었다는 뜻이다. 이 구분은 기존 인덱스를 `#[serde(default)]`로 읽을 때 조용한 영구 폴백을 막기 위해 필요하다.

1차 구현에서 `references`는 저장하지 않거나 낮은 cap을 둔다. caller/callee 부작용 확인의 1차 입력은 direct call이므로, 모든 identifier reference를 저장하면 `extracted_json` 크기와 snapshot 역직렬화 비용이 먼저 커진다.

`scope_id`는 첫 단계에서 생략 가능하다. 생략 시 같은 파일 line range 기반 우선순위만 적용한다.

초기 구현에서 `scope_id`가 없으면 lexical shadowing을 정밀하게 판정하지 않는다. shadowing 처리는 두 단계로 나눈다.

1. 같은 파일 line range 기반의 거친 local 우선순위
2. `scope_id` 도입 후 lexical scope 기준 shadowing 판정

### Snapshot 파생 색인

caller 역방향은 snapshot 전체의 call site를 매 요청마다 전수 순회하지 않는다. snapshot 발행 시 owned 파생 색인을 만들거나, annotation 시작 시 요청된 이름에 한정된 임시 색인을 만든다. 장기적으로 snapshot에 캐시하려면 self-referential reference 구조를 피하고 file index/call index 기반의 owned 색인을 쓴다.

```rust
pub struct NavigationIndex {
    pub calls_by_name: HashMap<String, Vec<CallSiteAddress>>,
    pub files_by_path: HashMap<String, usize>,
}

pub struct CallSiteAddress {
    pub file_index: usize,
    pub call_index: usize,
}
```

사용 규칙:

- caller 후보는 `calls_by_name[definition.name]`에서 시작한다.
- import alias 때문에 local call name과 definition name이 다를 수 있는 케이스는 import alias 단계 이후 별도 alias lookup으로 보강한다.
- 한 annotation pass에서 확인할 call site 수는 config budget으로 제한한다.
- budget을 넘으면 precise caller 판정을 중단하고 기존 approximate caller 표시로 폴백한다.
- `CallSite`에는 enclosing function 이름을 저장하지 않는다. query 시점에 기존 symbol range 기반 helper로 enclosing function을 계산한다.

저장 비용:

- `NavigationFile`은 `extracted_json`에 저장되므로 디스크 크기와 snapshot 역직렬화 비용에 직접 반영된다.
- snapshot 재발행은 저장된 모든 `ExtractedFile` JSON을 다시 읽는다.
- navigation 필드 추가 전후로 `extracted_json` 총량, snapshot load 시간, annotation 시간을 측정한다.

---

## 6. 귀속 알고리즘

호출 귀속은 다음 순서로 후보를 줄인다.

```text
resolve_call(call, file_context, symbol_index):
    local_shadow = find_local_binding(call.name, call.scope_id)
    if local_shadow exists and local_shadow is not a known function:
        return Fallback

    normalized_name = call.name
    source_hint = None

    import_entry = find_import_by_local_name(call.name, file_context.imports)
    if import_entry exists:
        normalized_name = import_entry.imported_name.unwrap_or(call.name)
        source_hint = import_entry.source

    owner_hint = infer_owner_hint(call.receiver, file_context.locals)

    candidates = lookup_definitions(
        name = normalized_name,
        allowed_kinds = [fn, method],
        current_file = file_context.file_path,
        source_hint = source_hint,
        owner_hint = owner_hint,
        order = [same_file, imported_source, global]
    )

    if candidates.len == 1:
        return Precise(candidates[0])

    return Fallback
```

후보 우선순위:

1. 같은 파일 안의 local/top-level function
2. import source가 가리키는 파일의 exported function
3. owner hint가 일치하는 method
4. 전역 name-match

후보 필터:

- 후보 kind는 `fn` 또는 `method`만 허용한다.
- import `source_hint`를 경유한 후보는 `flags.is_exported == true`여야 한다.
- same-file 후보는 local helper일 수 있으므로 export 여부를 요구하지 않는다.
- non-exported imported 동명이인, variable/function 동명이인, field/method 동명이인은 후보가 1개처럼 보여도 precise 처리하지 않는다.

전역 name-match는 최후 폴백이다. 전역 후보가 1개일 때만 qualified display를 쓰고, 여러 개면 bare name과 `approximate`를 유지한다.

caller와 callee는 같은 귀속 규칙을 쓴다.

- callee 방향: 현재 함수의 `navigation.calls`를 해석해 이 함수가 호출하는 후보를 좁힌다.
- caller 방향: `NavigationIndex.calls_by_name`에서 현재 정의 이름과 맞을 수 있는 call site를 찾고, 실제로 현재 정의를 가리키는 call site만 caller로 보여준다.
- caller 방향도 미리 계산된 콜그래프를 저장하지 않는다. query 시점에 최신 snapshot, `SymbolIndex`, `NavigationIndex`로 해석한다.
- caller 해석은 budget 안에서만 precise 후보 축소를 시도한다. budget을 넘으면 기존 approximate caller 표시로 폴백한다.

### Precise 허용 조건

`Precise`는 후보 수만으로 결정하지 않는다. 다음 조건이 모두 참일 때만 허용한다.

1. 초기 indexing이 끝났고 warming 상태가 아니다.
2. 마지막 background refresh 오류가 없다.
3. 해당 파일의 navigation 추출이 성공했다.
4. 후보 탐색이 budget 안에서 완료됐다.
5. 후보가 정확히 1개다.
6. 후보가 포함된 언어/확장자의 tags fixture와 navigation fixture가 통과한 상태다.

하나라도 만족하지 않으면 기존 approximate 폴백을 사용한다. precise 경로에서도 direct call 관측 범위 경고는 유지한다.

현재 `annotate_results`는 engine 상태를 인자로 받지 않으므로, precise 억제를 구현하려면 호출부에서 annotation runtime state를 전달해야 한다.

```rust
pub struct AnnotationRuntimeState {
    pub is_warming: bool,
    pub has_refresh_error: bool,
    pub is_dead_or_stale: bool,
}
```

---

## 7. 비용 예산과 관측성

navigation layer는 기본값으로 켜기 전에 비용과 효과를 측정한다.

필수 설정:

| 설정 | 목적 |
|------|------|
| `navigation_context_default` | navigation 기반 precise 후보 축소 기본 활성화 여부 |
| `navigation_callsite_budget` | 한 annotation pass에서 해석할 최대 call site 수 |
| `navigation_store_references` | `ReferenceSite` 저장 여부. 초기 기본값은 false |

필수 카운터:

| 카운터 | 의미 |
|------|------|
| `navigation_precise_count` | precise로 확정된 caller/callee 수 |
| `navigation_fallback_count` | approximate로 폴백한 수 |
| `navigation_fallback_reason` | warming, stale, budget, ambiguous, parse_failed, unsupported_language, source_unresolved, glob_import, local_shadow, scope_unconfirmed 등 |
| `navigation_callsite_count` | snapshot의 전체 direct call site 수 |
| `navigation_snapshot_bytes` | 저장된 navigation JSON 크기 |
| `navigation_snapshot_load_ms` | snapshot 역직렬화 및 파생 색인 생성 시간 |
| `navigation_annotation_ms` | annotation 단계에서 navigation 해석에 쓴 시간 |

benchmark gate:

- navigation 필드 추가 전후의 `extracted_json` 총량을 비교한다.
- initial index 시간과 incremental refresh 시간을 비교한다.
- `load_extracted_files` 이후 snapshot 발행 시간을 비교한다.
- caller/callee annotation 시간을 비교한다.
- precise 전환율과 fallback 사유 분포를 기록한다.

대형 저장소에서 비용이 기준을 넘으면 navigation precise는 config로 끄고 기존 caller/callee 동작을 유지한다.

---

## 8. 단계별 구현

### 단계 0: 기존 query 문자열을 `.scm`으로 분리

목표:

- Rust 코드 안의 기존 tree-sitter query 문자열을 `queries/<language>/symbols.scm`으로 옮긴다.
- `include_str!`로 읽어 기존 `Query::new(...)`에 전달한다.
- 심볼/literal 추출 결과는 바꾸지 않는다.

변경 위치:

| 파일 | 작업 |
|------|------|
| `queries/<language>/symbols.scm` | 기존 `*_QUERY_STR` 내용 이동 |
| `src/lang/*.rs` | `const *_QUERY_STR: &str = include_str!(...)`로 변경 |
| `src/parser/mod.rs` | 기존 capture prefix 라우팅 유지 |

완료 조건:

- 모든 기존 추출 fixture 결과가 동일하다.
- runtime query pass 수가 늘지 않는다.
- 새 field가 추가되지 않으므로 `EXTRACTION_FORMAT_VERSION`은 올리지 않는다.

### 단계 1: TypeScript/JavaScript tags 및 navigation call 추출

목표:

- `tags.scm`으로 TypeScript/TSX grammar의 최소 definition/reference 매칭을 먼저 고정
- `identifier(` 텍스트 스캔을 대체할 `navigation.scm` PoC 작성
- 함수 본문에서 실제 call/reference만 추출
- 문자열, 주석, 정의 헤더, property access 오탐 감소

변경 위치:

| 파일 | 작업 |
|------|------|
| `queries/typescript/tags.scm` | 표준 tags-compatible definition/reference 최소 캡처 추가 |
| `queries/typescript/navigation.scm` | call/reference/local 최소 캡처 추가 |
| `src/lang/typescript.rs` | navigation query getter 추가 |
| `src/lang/mod.rs` | `LanguageSpec::navigation_query` 또는 navigation 추출 훅 추가 |
| `src/parser/types.rs` | `NavigationFile`, `CallSite`, `ReferenceSite`, `LocalBinding` 추가 |
| `src/parser/mod.rs` | concat된 symbols/navigation query를 기존 단일 pass 라우팅에 추가 |
| `src/index/engine.rs` | navigation 직렬화가 생기는 변경과 같은 커밋에서 `EXTRACTION_FORMAT_VERSION` bump |

완료 조건:

- `tags.scm`과 `navigation.scm`이 TypeScript grammar와 TSX grammar에서 모두 compile된다.
- `.ts`, `.js`, `.tsx`, `.jsx` fixture가 expected tags/navigation 결과와 일치한다.
- 기존보다 적은 오탐이 fixture로 확인된다.
- query가 실패하거나 navigation이 비어 있으면 기존 텍스트 스캔 경로를 유지할 수 있다.
- `navigation: None`과 `Some(empty)`이 구분된다.

### 단계 2: callee 방향 전환

목표:

- `callees.rs`에서 현재 함수의 `navigation.calls`를 우선 사용한다.
- navigation 미지원/parse 실패/비어 있음이면 기존 텍스트 스캔으로 폴백한다.
- navigation snapshot이 stale일 수 있으면 기존 디스크 재읽기 텍스트 스캔으로 폴백한다.

변경 위치:

| 파일 | 작업 |
|------|------|
| `src/callers/callees.rs` | `navigation.calls` 기반 callee discovery 추가 |
| `src/callers/symbols.rs` | callee 후보 lookup helper 추가 |
| `src/tools/search/mod.rs` | stale/warming 상태를 annotation runtime state로 전달 |

완료 조건:

- 기존 callee 결과와 호환된다.
- 문자열/주석/정의 헤더 오탐이 줄어든다.
- navigation 미지원 파일에서 callee가 기존보다 줄지 않는다.
- index보다 source file이 새로울 수 있는 상태에서는 기존 live-disk callee scan으로 폴백한다.

### 단계 3: 같은 파일 local definition 우선순위

목표:

- 같은 파일의 local/top-level function을 전역 후보보다 먼저 선택한다.
- local binding이 import alias보다 우선하는 shadowing은 `scope_id` 전까지 보수적으로만 처리한다.
- source 해석 없이 동명이인 과대 귀속을 줄이는 최소 ROI 단계로 둔다.

변경 위치:

| 파일 | 작업 |
|------|------|
| `src/callers/symbols.rs` | same-file lookup helper 추가 |
| `src/callers/callees.rs` | callee display 후보 축소에 same-file 우선순위 적용 |

완료 조건:

- 같은 이름 함수가 여러 파일에 있어도 같은 파일 정의가 있으면 그 후보로 좁힌다.
- 같은 파일 후보가 여러 개면 폴백한다.
- 후보 kind가 `fn` 또는 `method`가 아니면 precise 처리하지 않는다.

### 단계 4: import alias

목표:

- named/default/namespace import의 local 이름을 원래 이름과 source hint로 변환한다.
- alias 때문에 빠지던 callee 후보를 복구한다.
- 첫 구현은 source가 존재한다는 신호와 unresolved 비율 측정에 집중한다.

변경 위치:

| 파일 | 작업 |
|------|------|
| `queries/typescript/navigation.scm` | import 캡처 확장 |
| `src/parser/types.rs` | `ImportEntry`, `ImportKind` 추가 |
| `src/callers/symbols.rs` | source hint 기반 후보 lookup 추가 |

구현 메모:

- `import { fetchUser as getUser } from "./user"`는 `local_name=getUser`, `imported_name=fetchUser`, `source=./user`로 저장한다.
- `import { fetchUser } from "./user"`는 `local_name=fetchUser`, `imported_name=fetchUser`로 저장한다.
- `import api from "./api"`는 default import로 저장하되, default export 매칭을 못 하면 폴백한다.
- `import * as api from "./api"`는 namespace import로 저장한다. `api.fetchUser()`는 `source=./api`, `name=fetchUser` 후보로 좁힌다.

상대 import source 해석 규칙:

1. importing file의 디렉터리를 기준으로 source를 정규화한다.
2. source가 상대 경로가 아니면 첫 구현에서는 폴백한다.
3. 확장자가 없으면 다음 순서로 후보를 찾는다: `.ts`, `.tsx`, `.js`, `.jsx`.
4. 파일 후보가 없으면 `index.ts`, `index.tsx`, `index.js`, `index.jsx`를 같은 순서로 찾는다.
5. workspace-relative `file_path`와 정확히 매칭되는 후보가 1개면 사용한다.
6. 후보가 0개 또는 2개 이상이면 폴백한다.

완료 조건:

- alias가 1개 exported function으로 좁혀질 때만 precise 처리한다.
- source 파일 후보가 0개 또는 2개 이상이면 폴백한다.
- path alias, barrel re-export는 첫 구현에서 폴백한다.
- source 해석 실패 시 기존 텍스트 스캔 결과로 복귀한다.
- `navigation_fallback_reason=source_unresolved`를 기록한다.
- import source를 경유한 후보는 exported function/method만 허용한다.

### 단계 5: caller 역방향 귀속

목표:

- 검색 결과의 각 `fn` 정의에 대해, workspace의 call site 중 실제로 그 정의를 가리키는 caller만 표시한다.
- 동명이인 함수의 caller 목록을 name-match 하나로 섞지 않는다.
- `NavigationIndex.calls_by_name`과 budget으로 전수 순회 비용을 제한한다.

변경 위치:

| 파일 | 작업 |
|------|------|
| `src/callers/annotate.rs` | scan hit 또는 navigation call site를 `resolve_call`로 현재 정의에 매칭 |
| `src/callers/symbols.rs` | `NavigationIndex`, 정의별 후보 비교 helper 추가 |
| `src/config.rs` | `navigation_callsite_budget` 추가 |
| `src/tools/search/mod.rs` | `is_warming`, `last_error`, dead/stale 상태를 annotation에 전달 |

구현 메모:

- 초기 구현에서는 기존 `scan_workspace`를 coverage fallback으로 유지한다.
- navigation call site는 `calls_by_name`에서 budget 안에서만 꺼낸다.
- 현재 정의와 call site 해석 결과가 정확히 같은 후보일 때만 caller로 렌더링한다.
- warming/stale/error 상태에서는 caller precise를 시도하지 않는다.

완료 조건:

- `User.save`와 `File.save`가 둘 다 있을 때, `user.save()`가 `User.save` 하나로 좁혀진 경우 `File.save` caller에 섞이지 않는다.
- 후보가 좁혀지지 않으면 기존 caller 표시와 `approximate` 라벨을 유지한다.
- budget 초과 시 기존 approximate caller 표시로 폴백한다.

### 단계 6: receiver/owner hint

목표:

- `receiver.method()` 호출에서 receiver의 타입 힌트를 이용해 method owner 후보를 줄인다.
- 실제 precise 전환율이 낮을 수 있으므로 metrics로 효과를 확인한 뒤 기본 활성화한다.

지원할 첫 패턴:

```ts
const user = new User();
user.save();

const user: User = getUser();
user.save();
```

후보 축소:

```text
call.receiver = "user"
local user value_type_hint = "User"
call.name = "save"
candidate owner == "User"
```

완료 조건:

- owner hint와 method name이 모두 일치하는 후보가 1개일 때만 precise 처리한다.
- `this.save()`, optional chaining, destructuring, factory 반환 타입, interface dispatch는 폴백한다.

### 단계 7: lexical scope 정밀화

목표:

- `scope_id`를 도입해 local binding, import alias, call site의 lexical 관계를 판정한다.
- 같은 이름 local binding이 import alias를 shadowing하는 경우를 정확히 폴백 처리한다.

변경 위치:

| 파일 | 작업 |
|------|------|
| `queries/typescript/navigation.scm` | `@local.scope`, `@local.definition`, `@local.reference` 캡처 추가 |
| `src/parser/mod.rs` | scope node에 안정적인 파일 내 `scope_id` 부여 |
| `src/callers/callees.rs` | 같은 scope 또는 상위 scope local binding 우선 적용 |
| `src/callers/annotate.rs` | caller 귀속에도 scope 우선순위 적용 |

완료 조건:

- local `bar`가 import alias `bar`를 shadowing하면 import alias 귀속을 쓰지 않는다.
- scope 관계를 확정하지 못하면 폴백한다.
- 표준 locals query로 표현 가능한 범위를 먼저 지원하고, 안정적인 `scope_id` 부여가 어렵다면 line-range 근사로 유지한다.

### 단계 8: 언어 확장

TypeScript/JavaScript에서 검증한 뒤 언어를 확장한다. 각 언어는 `src/lang/<lang>.rs`와 `queries/<lang>/` 쌍으로 독립 실행된다.

**활성화 메커니즘 (Wave 6 자식 간 통일)**: navigation 활성화는 `LanguageSpec` trait의 기본 구현으로 처리한다. 단계 1에서 TypeScript를 위해 `LanguageSpec`에 다음 형태의 hook을 추가하고, 기본 구현은 `None`을 반환한다.

```rust
fn navigation_query(&self) -> Option<&'static str> { None }
```

각 Wave 6 언어는 자신의 `src/lang/<lang>.rs`(C/C++는 `src/lang/c_family/<lang>.rs`)에서 이 메서드를 override해 `Some(include_str!("<상대경로>/<lang>/navigation.scm"))`를 반환하는 것만으로 활성화된다. 따라서 **Wave 6 자식은 `src/lang/mod.rs`를 구조적으로 편집하지 않는다**. `src/lang/mod.rs`는 trait 정의(단계 1에서 hook 추가 완료)와 `spec_for_ext` 레지스트리를 담은 read-only 참조 파일이며, 언어별 등록 테이블을 따로 두지 않는다. 단계 8의 파일럿 자식(Python)은 `src/lang/mod.rs`를 새로 바꾸지 않고, 단계 1에서 확정된 trait 기본 구현 override 패턴이 정상 동작함을 확인하는 역할만 한다. 이미 모든 spec이 `spec_for_ext`/`ALL_SPECS`에 등록돼 있으므로 새 등록 항목도 필요 없다.

**ASM 처리**: ASM(`.s`, `.S`, `.asm`)은 단계 0 `symbols.scm` 분리 대상에 포함되지만, navigation 자식 대상에서 **제외**한다. 이유: tree-sitter-asm에서 `CALL`/`BL`/`JMP` 같은 분기 명령어가 일반 `instruction` 노드로 파싱되어 "함수 호출인지 브랜치인지"를 query 수준에서 구분할 수 없고, import/alias 개념이 없어 `@nav.import.*` 캡처 계약과 정합하지 않는다. ASM 파일은 §9 "navigation layer 적용 대상 아님" 폴백을 따른다.

권장 순서 및 언어별 navigation.scm 캡처 계약:

> **계약 근거**: 아래 캡처 계약은 각 언어의 `src/lang/*.rs` 인라인 query 상수와 tree-sitter grammar node shape에서 도출한 최소 매핑이다. 정확한 node 이름은 fixture 통과를 activation gate로 삼으며, 불확실한 항목은 "(fixture-confirm)" 표시로 구분한다.

#### 1. Python (`tree_sitter_python::LANGUAGE`, `.py`)

**주의사항**: `from x import y as z`, local function, method call.

`navigation.scm` 최소 캡처 계약:

```scheme
;; import: `import x`, `from x import y`, `from x import y as z`
(import_statement
  name: (dotted_name) @nav.import.name) @nav.import

(import_from_statement
  module_name: (dotted_name) @nav.import.source
  name: (dotted_name) @nav.import.name) @nav.import

(import_from_statement
  module_name: (dotted_name) @nav.import.source
  name: (aliased_import
    name: (dotted_name) @nav.import.name
    alias: (identifier) @nav.import.alias)) @nav.import

;; call: `func()`, `obj.method()`
(call
  function: (identifier) @nav.call.name) @nav.call

(call
  function: (attribute
    object: (identifier) @nav.call.receiver
    attribute: (identifier) @nav.call.name)) @nav.call

;; local binding: `x = ...`, `x: T = ...`
(assignment
  left: (identifier) @local.definition) @local.scope    ;; (fixture-confirm: scope 노드 경계)
```

완료 조건:
- `queries/python/tags.scm`과 `queries/python/navigation.scm`이 `tree_sitter_python::LANGUAGE`에서 compile된다.
- `tests/fixtures/navigation/python/` fixture가 `expected.tags.json` 및 `expected.navigation.json`과 일치한다.
- `from x import y as z` 뒤 `z()` 호출이 `y` 후보로 좁혀진다.
- navigation 미지원 파일에서 기존 텍스트 스캔 폴백이 유지된다.

#### 2. Go (`tree_sitter_go::LANGUAGE`, `.go`)

**주의사항**: package selector(`pkg.Func`)와 value receiver(`func (s *Server) Start()`) 구분.

`navigation.scm` 최소 캡처 계약:

```scheme
;; import: `import "path"`, `import alias "path"`
(import_spec
  path: (interpreted_string_literal) @nav.import.source) @nav.import

(import_spec
  name: (identifier) @nav.import.alias
  path: (interpreted_string_literal) @nav.import.source) @nav.import

;; call: `Func()`, `pkg.Func()`, `receiver.Method()`
(call_expression
  function: (identifier) @nav.call.name) @nav.call

(call_expression
  function: (selector_expression
    operand: (identifier) @nav.call.receiver
    field: (field_identifier) @nav.call.name)) @nav.call

;; local binding: `x := expr`, `var x T = expr`
(short_var_declaration
  left: (expression_list (identifier) @local.definition)) @local.scope   ;; (fixture-confirm)

(var_declaration
  (var_spec name: (identifier) @local.definition)) @local.scope          ;; (fixture-confirm)
```

완료 조건:
- `queries/go/tags.scm`과 `queries/go/navigation.scm`이 `tree_sitter_go::LANGUAGE`에서 compile된다.
- `tests/fixtures/navigation/go/` fixture가 expected JSON과 일치한다.
- `pkg.Func()` 형태의 package selector가 `nav.import.alias` → `nav.import.source`로 좁혀진다.
- value receiver `func (s *Server) Start()`에서 `method_declaration`의 receiver 타입이 `owner_hint`로 활용 가능하다. (receiver type node 매핑은 fixture-confirm)
- navigation 미지원 파일에서 기존 폴백 유지.

#### 3. Rust (`tree_sitter_rust::LANGUAGE`, `.rs`)

**주의사항**: `use ... as ...`, glob import, trait method는 보수적으로 폴백.

`navigation.scm` 최소 캡처 계약:

```scheme
;; import: `use path::name`, `use path::name as alias`, `use path::*`
(use_declaration
  argument: (scoped_identifier
    path: (_) @nav.import.source
    name: (identifier) @nav.import.name)) @nav.import

(use_declaration
  argument: (use_as_clause
    path: (scoped_identifier
      path: (_) @nav.import.source
      name: (identifier) @nav.import.name)
    alias: (identifier) @nav.import.alias)) @nav.import

(use_declaration
  argument: (use_wildcard
    path: (_) @nav.import.source)) @nav.import.glob   ;; glob은 항상 폴백

;; call: `func()`, `obj.method()`, `Struct::assoc()`
(call_expression
  function: (identifier) @nav.call.name) @nav.call

(call_expression
  function: (field_expression
    value: (identifier) @nav.call.receiver
    field: (field_identifier) @nav.call.name)) @nav.call

(call_expression
  function: (scoped_identifier
    path: (_) @nav.call.receiver
    name: (identifier) @nav.call.name)) @nav.call     ;; (fixture-confirm: path 노드 형상)

;; local binding: `let x = ...`, `let x: T = ...`
(let_declaration
  pattern: (identifier) @local.definition
  type: (_) @local.type) @local.scope                 ;; (fixture-confirm: scope 경계)

(let_declaration
  pattern: (identifier) @local.definition) @local.scope
```

완료 조건:
- `queries/rust/tags.scm`과 `queries/rust/navigation.scm`이 `tree_sitter_rust::LANGUAGE`에서 compile된다.
- `tests/fixtures/navigation/rust/` fixture가 expected JSON과 일치한다.
- `use path::name as alias` 뒤 `alias()` 호출이 `name` 후보로 좁혀진다.
- glob import(`use path::*`)는 항상 폴백; trait method 호출도 보수적 폴백.
- navigation 미지원 파일에서 기존 폴백 유지.

#### 4. Java (`tree_sitter_java::LANGUAGE`, `.java`)

**주의사항**: import와 class method owner. `modifiers`로 public 여부를 결정.

`navigation.scm` 최소 캡처 계약:

```scheme
;; import: `import com.example.Class`, `import static com.example.Class.method`
(import_declaration
  (scoped_identifier
    scope: (_) @nav.import.source
    name: (identifier) @nav.import.name)) @nav.import

;; call: `method()`, `obj.method()`, `new Class()`
(method_invocation
  name: (identifier) @nav.call.name) @nav.call

(method_invocation
  object: (identifier) @nav.call.receiver
  name: (identifier) @nav.call.name) @nav.call

(object_creation_expression
  type: (type_identifier) @nav.call.name) @nav.call   ;; (fixture-confirm: new Expr 노드)

;; local binding: `Type name = ...`
(local_variable_declaration
  type: (type_identifier) @local.type
  declarator: (variable_declarator
    name: (identifier) @local.definition)) @local.scope   ;; (fixture-confirm)
```

완료 조건:
- `queries/java/tags.scm`과 `queries/java/navigation.scm`이 `tree_sitter_java::LANGUAGE`에서 compile된다.
- `tests/fixtures/navigation/java/` fixture가 expected JSON과 일치한다.
- `import com.example.UserService` 뒤 `userService.save()` 호출에서 `UserService.save` 후보로 좁혀진다.
- navigation 미지원 파일에서 기존 폴백 유지.

#### 5. Kotlin (`tree_sitter_kotlin_ng::LANGUAGE`, `.kt`, `.kts`)

**주의사항**: grammar crate 이름이 `tree_sitter_kotlin_ng`임. `class_declaration`이 class와 interface를 공유함. top-level annotation quirk 존재.

`navigation.scm` 최소 캡처 계약:

```scheme
;; import: `import com.example.Class`, `import com.example.Class as Alias`
(import_header
  identifier: (identifier) @nav.import.name) @nav.import   ;; (fixture-confirm: full path 노드)

(import_header
  identifier: (identifier) @nav.import.name
  alias: (import_alias
    (type_alias (identifier) @nav.import.alias))) @nav.import  ;; (fixture-confirm)

;; call: `func()`, `obj.method()`, `Class()`
(call_expression
  calleeExpression: (simple_identifier) @nav.call.name) @nav.call     ;; (fixture-confirm: field name)

(call_expression
  calleeExpression: (navigation_expression
    navigationSuffix: (navigation_suffix
      (simple_identifier) @nav.call.name))
  ) @nav.call                                                           ;; (fixture-confirm)

;; local binding: `val x: T = ...`, `var x = ...`
(property_declaration
  (variable_declaration
    (simple_identifier) @local.definition
    (type_reference) @local.type)) @local.scope   ;; (fixture-confirm)
```

완료 조건:
- `queries/kotlin/tags.scm`과 `queries/kotlin/navigation.scm`이 `tree_sitter_kotlin_ng::LANGUAGE`에서 compile된다.
- `tests/fixtures/navigation/kotlin/` fixture가 expected JSON과 일치한다.
- Kotlin import alias(`import Foo as Bar`) 뒤 `Bar()` 호출이 `Foo` 후보로 좁혀진다.
- navigation 미지원 파일에서 기존 폴백 유지.

> Kotlin navigation.scm 노드 이름은 tree-sitter-kotlin-ng grammar에서 fixture-confirm이 필요한 항목이 많다. 브리프 작성 시 grammar node-types.json을 반드시 확인한다.

#### 6. C (`tree_sitter_c::LANGUAGE`, `.c`)

**주의사항**: `#include`가 유일한 import 등가 구문. free function 우선. `static` 함수는 file-local. overload 없음. `.h`는 C++ grammar 처리.

`navigation.scm` 최소 캡처 계약:

```scheme
;; import: `#include "file.h"`, `#include <file.h>`
(preproc_include
  path: (string_literal) @nav.import.source) @nav.import

(preproc_include
  path: (system_lib_string) @nav.import.source) @nav.import

;; call: `func()`, `ptr->method()`, `obj.field_fn()`
(call_expression
  function: (identifier) @nav.call.name) @nav.call

(call_expression
  function: (field_expression
    argument: (identifier) @nav.call.receiver
    field: (field_identifier) @nav.call.name)) @nav.call   ;; (fixture-confirm: arrow vs dot)

;; local binding: `Type name = ...` (function-scope)
(declaration
  type: (type_identifier) @local.type
  declarator: (identifier) @local.definition) @local.scope   ;; (fixture-confirm)
```

완료 조건:
- `queries/c/tags.scm`과 `queries/c/navigation.scm`이 `tree_sitter_c::LANGUAGE`에서 compile된다.
- `tests/fixtures/navigation/c/` fixture가 expected JSON과 일치한다.
- free function 호출이 same-file 우선으로 좁혀진다.
- `static` 선언 함수는 exported=false로 처리되어 import source hint 경유 후보에서 제외된다.
- navigation 미지원 파일에서 기존 폴백 유지.

#### 7. C++ (`tree_sitter_cpp::LANGUAGE`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx`)

**주의사항**: overload, template, operator call은 폴백. out-of-line member(`Foo::bar()`)의 owner 추출은 `cpp_outofline_owner`에 이미 구현됨. `class_specifier`의 access_specifier 기반 export 규칙 존재.

`navigation.scm` 최소 캡처 계약:

```scheme
;; import: `#include "file.h"`, `#include <file.h>`
(preproc_include
  path: (string_literal) @nav.import.source) @nav.import

(preproc_include
  path: (system_lib_string) @nav.import.source) @nav.import

;; call: `func()`, `obj.method()`, `obj->method()`, `Scope::func()`
(call_expression
  function: (identifier) @nav.call.name) @nav.call

(call_expression
  function: (field_expression
    argument: (identifier) @nav.call.receiver
    field: (field_identifier) @nav.call.name)) @nav.call   ;; (fixture-confirm)

(call_expression
  function: (qualified_identifier
    scope: (_) @nav.call.receiver
    name: (identifier) @nav.call.name)) @nav.call          ;; (fixture-confirm: scope node)

;; local binding: `Type name = ...`
(declaration
  type: (type_identifier) @local.type
  declarator: (identifier) @local.definition) @local.scope  ;; (fixture-confirm)

(init_declarator
  declarator: (identifier) @local.definition) @local.scope  ;; (fixture-confirm)
```

완료 조건:
- `queries/cpp/tags.scm`과 `queries/cpp/navigation.scm`이 `tree_sitter_cpp::LANGUAGE`에서 compile된다.
- `tests/fixtures/navigation/cpp/` fixture가 expected JSON과 일치한다.
- free function 호출이 same-file 우선으로 좁혀진다.
- overload/template/operator call은 후보 1개로 좁혀지지 않으면 폴백된다.
- navigation 미지원 파일에서 기존 폴백 유지.

---

## 9. 폴백 조건

다음 경우는 반드시 기존 name-match + `approximate`로 남긴다.

- index가 warming 상태다.
- 마지막 background refresh가 실패했거나 검색 결과가 stale일 수 있다.
- 후보 탐색이 `navigation_callsite_budget`을 초과했다.
- navigation 추출이 실패했거나 해당 파일의 navigation 관측치가 비어 있다.
- 해당 언어 또는 확장자가 navigation layer 적용 대상이 아니다. ASM(`.s`, `.S`, `.asm`)은 call site 캡처 불가 및 import 개념 없음으로 navigation 적용 대상에서 제외한다. ASM 파일은 기존 name-match + `approximate` 동작만 사용하고, `navigation_fallback_reason=unsupported_language`로 기록한다.
- 파일이 size filter로 index 대상에서 제외됐다.
- 후보가 1개로 좁혀지지 않는다.
- source module을 실제 파일 하나로 찾을 수 없다.
- local binding이 import alias를 shadowing한다.
- import가 barrel re-export를 거친다.
- TypeScript path alias가 설정 파일 해석을 요구한다.
- namespace import 안에서 실제 export 후보가 여러 개다.
- Rust glob import(`use path::*`) 또는 trait method 호출이다. glob import 폴백은 전용 사유 코드 `navigation_fallback_reason=glob_import`으로 기록한다. `unsupported_language`를 쓰지 않는다 — Rust는 navigation 지원 언어이고 `unsupported_language`는 ASM처럼 navigation layer 적용 대상이 아닌 언어/확장자 전용이다.
- Go selector가 package alias인지 value receiver인지 확정되지 않는다.
- C++ overload, template, operator call이다.
- receiver 타입 힌트가 없거나 여러 타입으로 추론된다.
- query capture는 성공했지만 Rust 소비자가 기대한 필수 캡처가 없다.
- 해당 확장자의 tags fixture가 없거나 실패한다.
- 같은 `LanguageSpec`의 일부 grammar에서 query compile이 실패한다.

navigation 폴백은 recall 회귀를 만들면 안 된다. navigation 미지원 파일, parse 실패 파일, 동적 호출, callback 전달, higher-order usage는 기존 텍스트 scan 경로와 observation caveat를 유지한다.

---

## 10. 검증 Fixture

각 단계는 작은 fixture repo로 before/after를 비교한다.

필수 fixture:

1. 기존 `symbols.scm` 추출 결과가 Rust 문자열 query 시절과 동일하다.
2. concat된 symbols/navigation runtime query가 단일 pass로 기존 symbol 결과와 navigation 결과를 모두 낸다.
3. 같은 파일 local function이 전역 동명이인보다 우선한다.
4. non-fn/non-method 동명이인은 후보 1개처럼 보여도 precise 처리하지 않는다.
5. import source 경유 후보가 non-exported면 precise 처리하지 않는다.
6. 문자열과 주석 안의 `name(`은 call로 잡히지 않는다.
7. 정의 헤더 `function name()`은 caller로 잡히지 않는다.
8. `import { foo as bar }` 뒤 `bar()`가 `foo` 후보로 좁혀진다.
9. source 해석 실패 시 기존 텍스트 scan 결과로 복귀하고 `source_unresolved`를 기록한다.
10. local `bar`가 import alias `bar`를 shadowing하면 폴백한다.
11. `api.foo()` namespace import가 source hint를 만든다.
12. `user.save()`가 `User.save` 하나로 좁혀질 때만 precise 처리된다.
13. `file.save()`와 `user.save()`가 owner hint 없이 같은 `save`에 과대 귀속되지 않는다.
14. JSX 내부 callback `onClick={() => save()}`를 navigation call로 캡처한다.
15. navigation parse 실패 파일은 기존 텍스트 scan 경로로 폴백한다.
16. callee snapshot이 stale일 수 있으면 기존 디스크 재읽기 scan으로 폴백한다.
17. warming/stale 상태에서는 후보가 1개여도 precise로 렌더링하지 않는다.
18. budget 초과 시 caller precise를 중단하고 approximate로 폴백한다.
19. `navigation: None`은 미추출, `Some(empty)`은 추출 성공 후 관측치 없음으로 구분된다.

각 fixture는 다음을 확인한다.

- symbols query의 기존 추출 결과 호환성
- 표준 tags-compatible query의 definition/reference 매칭
- `Precise`가 생기는 케이스
- 기존 `approximate` 폴백이 유지되는 케이스
- navigation query가 비어 있거나 해석 실패 시 기존 결과가 깨지지 않는 케이스
- direct call 관측 범위 caveat가 precise 경로에서도 유지되는 케이스

---

## 11. 구현 순서 요약

1. 기존 Rust 문자열 query를 `queries/<language>/symbols.scm`으로 옮기고 `include_str!`로 읽는다.
2. 기존 추출 fixture가 동일한지 확인한다.
3. TypeScript/TSX용 표준 tags-compatible query를 작성하고 compile과 fixture 매칭을 고정한다.
4. `NavigationFile` 타입 추가, navigation extraction, `EXTRACTION_FORMAT_VERSION` bump를 같은 변경 단위로 적용한다.
5. `queries/typescript/navigation.scm`을 연결하고 symbols/navigation concat 단일 pass로 call/import/local 최소 관측치를 추출한다.
6. `callees.rs`에서 navigation calls를 우선 사용하되, 없거나 stale 가능성이 있으면 기존 디스크 재읽기 텍스트 스캔으로 폴백한다.
7. 같은 파일 function 후보 우선순위를 넣고 `fn/method` kind 필터를 고정한다.
8. import alias와 상대 source path 해석을 넣고 exported 후보만 precise 허용한다.
9. 비용 metrics와 navigation on/off config를 추가한다.
10. `AnnotationRuntimeState`를 `search/mod.rs`에서 `annotate_results`로 전달한다.
11. `NavigationIndex.calls_by_name`과 caller budget을 추가한다.
12. caller 역방향 귀속에 같은 `resolve_call` 규칙을 적용한다.
13. receiver/owner hint를 metrics로 검증한 뒤 조건부로 넣는다.
14. 표준 locals-compatible capture 기반 lexical shadowing을 넣는다.
15. 각 단계마다 “1개 후보면 precise, 아니면 approximate” 규칙과 precise 억제 조건을 fixture로 고정한다.

---

## 12. 최종 멘탈 모델

```text
tree-sitter symbols query
    -> existing symbol/literal extraction
tree-sitter tags query
    -> configured-language matching gate
tree-sitter navigation query
    -> structured observations
    -> snapshot-derived call-site index
    -> budgeted query-time resolution
    -> Rust candidate narrowing
    -> exactly one candidate: precise
    -> otherwise: existing approximate fallback
```

이 층은 정밀한 의미 분석기가 아니라, 기존 name-match 위에 얹는 보수적 후보 축소기다. 정확하다고 증명된 경우에만 결과를 좁히고, 나머지는 기존 동작을 보존한다.
