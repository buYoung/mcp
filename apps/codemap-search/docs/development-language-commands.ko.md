# 개발 언어 품질 확인 명령

소스 저장소의 검증 실행기로 현재 바이너리의 언어별 탐색을 확인합니다. 아래 명령은 **모노레포 루트**에서 실행합니다. 개인 설치 경로나 `cm` 같은 별도 단축 명령은 필요하지 않습니다.

## 준비

- Python 3.10 이상과 Git이 필요합니다. 기본 자동 빌드와 `test`에는 Rust/Cargo 및 네이티브 C 도구 모음이 필요합니다.
- `quick`은 공개 저장소나 별도 언어 파서를 내려받지 않습니다. 자동 Cargo 빌드는 프로젝트 의존성을 준비할 수 있습니다.
- `public`은 공개 저장소 이력·파서 의존성을 내려받고 별도 작업 트리·색인·결과를 만듭니다. 네트워크와 디스크 공간을 확보하세요. 선택 언어에 따라 `tokei`, Go, `rust-analyzer`, Universal Ctags, Node/TypeScript, Swift, Java 또는 Dart 도구가 필요합니다.
- 설치 바이너리를 비교하려면 `--binary`에 실행 파일 경로나 PATH의 명령 이름을 지정합니다. 자동 빌드를 생략하므로 비교 대상 버전을 먼저 확인하세요.

실행할 작업만 확인하려면 다음 명령을 사용합니다. 빌드·다운로드·결과 파일 생성은 하지 않습니다.

```sh
apps/codemap-search/verify --help
apps/codemap-search/verify public --dry-run
```

## 작은 검사와 기존 Cargo 검증

```sh
# 현재 코드 빌드와 25개 개발 언어의 작은 검증
apps/codemap-search/verify

# 필요한 언어와 설정만 선택
apps/codemap-search/verify --language rust --language typescript --profile structural

# 기존 cargo check와 cargo test
apps/codemap-search/verify test

# PATH의 설치 바이너리 비교
apps/codemap-search/verify --binary codemap-search
```

`quick`은 호출·제외·상수 문맥 등의 작은 사례를 검사합니다. 통과 개수는 실행 결과에서 확인하며, 작은 검사의 통과를 모든 문법이나 실제 프로젝트의 정확도로 해석하지 않습니다. `test`의 테스트 개수는 Cargo 로그에서 확인합니다.

## 공개 저장소 검증

```sh
apps/codemap-search/verify public --language python --repository django/django
apps/codemap-search/verify public --language rust --language go --jobs 2
apps/codemap-search/verify public --language dart
```

대상은 [언어별 고정 후보](../validation/development-languages.json)에서 선택합니다. 선택 언어에 속하지 않는 저장소나 오타는 실행 전에 거절합니다. Dart·Scala·Groovy의 파서 도우미는 함께 준비하며, 고정 lockfile과 맞지 않는 의존성은 실패로 처리합니다.

앞선 실행에서 준비가 완료된 캐시를 재사용할 때만 `--skip-prepare`를 지정하세요. 검증 자체를 생략하는 옵션은 아닙니다.

```sh
apps/codemap-search/verify public --language python --repository django/django \
  --cache "$HOME/.cache/codemap-public-validation" --skip-prepare
```

| 선택 사항 | 적용 |
| --- | --- |
| `--language` | `quick`·`public` 대상 언어; 반복 지정 가능 |
| `--profile default` / `structural` | `quick`·`public` 설정; 생략하면 두 설정 모두 |
| `--repository owner/repository` | `public`의 고정 후보 선택; 반복 지정 가능 |
| `--cache` | 결과·공개 저장소 캐시; 기본 `~/.cache/codemap-public-validation`, `CODEMAP_VALIDATION_CACHE`로도 지정 가능 |
| `--binary` | `quick`·`public`에서 자동 빌드 대신 사용할 실행 파일 |
| `--jobs 1` … `4` | `public` 저장소별 동시 작업 수; 기본 2 |
| `--skip-prepare` | `public`에서 준비된 저장소·파서 캐시 재사용 |
| `--dry-run` | 실행할 명령만 표시 |

병렬 단위는 저장소이며 각 저장소는 별도 작업 트리·색인·MCP 프로세스를 사용합니다. 한 저장소 안의 설정별 검사와 파일 생성·수정·삭제 검사는 순차로 실행합니다. 실행 중인 작업 트리와 색인을 다른 MCP 프로세스에서 동시에 사용하지 마세요.

## 결과 확인

각 실행은 `<cache>/runs/verify-<실행 ID>/summary.json`과 단계별 로그·하위 결과 경로를 남깁니다. `quick`·`public`은 검사 바이너리 사본과 SHA-256도 보존하므로 실행 도중 다른 빌드로 바뀌지 않습니다.

1. 종료 코드와 전체 완료 여부를 확인합니다. 실패·판정 보류·실행 오류는 성공으로 처리하지 않습니다.
2. `summary.json`에서 단계별 `pass`, `fail`, `unverified`, 오류와 검사 대상 바이너리를 확인합니다.
3. 실패 사례의 원시 MCP 응답과 독립 대조 자료를 확인합니다. 색인 준비 중 응답, 미지원 문법, 잘못된 연결을 구분하세요.

이전 실행은 덮어쓰지 않습니다. 중단·실패한 실행과 재실행 결과도 별도로 해석하며, 다른 바이너리의 통과 개수를 합쳐 현재 전체 검증 결과로 표시하지 않습니다.

## MCP와 CLI로 직접 확인

MCP 클라이언트의 작업공간을 이 앱 또는 확인할 저장소로 지정합니다. `initial_instructions`로 안내를 읽고, Jev를 켰다면 [작업 등록·루트 개요 순서](configuration.ko.md#작업-등록과-도구-순서)를 따릅니다. 다음은 이 앱에서 현재 선언 위치를 찾는 MCP 요청입니다.

```json
{"name":"grep","arguments":{"path":"src/config.rs","pattern":"pub fn load","view":"source","head_limit":1}}
```

반환된 파일·행으로 `read`를 호출합니다. 원문만 필요하면 `view=source`, 선언만 필요하면 `definitions`, 관계만 필요하면 `relations`를 사용합니다. `expand=callable`은 함수를 통째로 읽으며 큰 함수는 `expand=none`과 줄 범위로 나눕니다. 상세 계약은 [탐색 출력](value-navigation.ko.md)에 있습니다.

CLI의 직접 파일 파싱은 다음처럼 확인합니다.

```sh
codemap-search parse apps/codemap-search/src/config.rs
codemap-search codemap --path apps/codemap-search/src/config.rs
```

CLI `search`의 파일 목록과 MCP `search`의 상세 응답은 다릅니다. MCP 출력 검증을 CLI 검색 결과만으로 대신하지 마세요.

### 매크로 생성 선언

`[output.macro_expansion].is_enabled`의 **기본값은 `true`**입니다. C/C++·ASM의 생성 선언을 확인하려면 설치된 Clang/NASM과 대상 프로젝트의 빌드 문맥이 필요합니다. 실행 파일 경로·컴파일 데이터베이스·플래그는 [매크로 설정](configuration.ko.md#매크로-확장)을 따릅니다. 전처리 결과의 선언 위치와 실제 원문을 구분하고 미해결 관계를 실행 증거로 해석하지 않습니다.

### 추상 선언과 구현 후보

[추상 선언·구현 탐색](implementation-navigation.ko.md)의 지원 범위에 해당하는 변경은 다음 기존 검증으로 좁혀 확인할 수 있습니다.

```sh
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --lib implementations::tests::
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_implementation_
```

## 과거 검증 기록

[전체 언어 검증](development-language-validation.ko.md), [Rust·Go 검증](public-repository-validation.ko.md), [당시 개별 확인 명령](history/development-language-commands.ko.md)은 고정된 이전 실행의 기록입니다. 개인 경로·줄 번호·측정 수치를 현재 실행의 전제나 결과로 사용하지 않습니다.
