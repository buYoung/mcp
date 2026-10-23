# 03 — 고정 데이터셋 다운로드와 설정

2026-10-17에 설정·다운로드 코드를 확인하고 검증 결과를 정리했다. 설정은 기본 비활성이며 이 03단계에서는 다운로드 API를 시작 경로에 연결하지 않았다. 이후 MCP 연결 결과는 [04 기록](04-mcp-tool.md)에 있다. 다운로드 무결성 e2e 10개와 기존 설정·Jev 검증이 통과했다. 공개 릴리스의 실제 내려받기는 02의 게시 이후에 확인해야 한다.

## 설정과 후속 연결

| 설정 | 기본값 | 동작 |
| --- | --- | --- |
| `related_terms.enabled` | `false` | 04가 시작 작업과 도구 노출을 제어할 값이다 |
| `related_terms.dataset_path` | 없음 | 사용자 파일을 사용하며 상대 경로는 서버 작업 디렉터리를 기준으로 한다 |

`ResolvedConfig.related_terms`에 두 값이 전달된다. 저장소 설정, 전역 설정, 기본값 순으로 합치며 잘못된 타입은 경고하고 키별 상속값·기본값을 사용한다. schema version은 29에서 30으로 올렸다. 정규화·구역 순서는 `config/layout.rs`, 양언어 템플릿은 `config_template.toml`·`config_template.ko.toml`, 추가 마이그레이션은 `config.rs`가 관리한다. 기존 locale 선택을 재사용하므로 `config_locale.rs`의 로직은 바꾸지 않았다. 영어·한국어 설정 문서와 README의 다운로드 조건도 맞췄다.

실제 MCP 세션에서 버전 29의 사용자 주석과 `detail_file_limit = 7`의 값·행 끝 주석이 보존되고 버전 30 및 두 새 설정의 주석이 추가됨을 확인했다. 04의 전역 설정 검증에서 자동 생성 템플릿의 활성 `enabled = false`가 전역 true를 덮어쓰는 결함을 발견했다. 양언어 템플릿의 이 값을 주석으로 바꾸고 `cargo check`, 설정 필터 16개, 새 작업공간의 실제 MCP 평가를 통과했다. 기본 false는 코드에서 유지한다. `dataset_path = 42`는 `must be a string` 경고를 내며 서버를 종료하지 않는다.

## 다운로드 API

구현은 `apps/codemap-search/src/related_terms/fetch.rs`다.

```rust
fetch_dataset(
    global_dir: &Path,
    dataset_path: Option<&Path>,
    spec: &DatasetSpec<'_>,
) -> FetchStatus

DatasetSpec { id, url, size, sha256 }
FetchStatus::Ready { path, user_owned }
FetchStatus::InProgress
FetchStatus::Failed { reason }
```

이 함수는 동기 함수다. 04는 기능이 켜진 경우에만 별도 작업 스레드에서 호출해야 한다. `Ready`의 `user_owned`를 01의 `prepare_dataset`에 그대로 전달해야 한다. `InProgress`는 다른 프로세스가 다운로드 잠금을 잡고 있다는 뜻이며 나중에 재시도한다. 실패 이유는 `Failed`로 반환한다.

`dataset_path`가 있으면 파일·폴더에 쓰거나 HTTP client를 만들지 않고 `user_owned = true`로 반환한다. 존재 여부와 SQLite 형식 확인은 후속 준비가 맡는다. 다운로드 완료 파일이 이미 있으면 네트워크와 해시 검사 없이 재사용한다. 준비 인덱스가 그 파일의 바이트를 바꿀 수 있기 때문이다.

### 고정 자료

- id: `rts5-75e02862daa29308`
- schema: `rt-sense/5`이며 형식 검증은 01의 준비 단계가 맡는다.
- URL: `https://github.com/buYoung/mcp/releases/download/related-terms-data-rts5-75e02862daa29308/rts5-75e02862daa29308.tar.zst`
- 추출 크기: 31,715,328 bytes다.
- 추출 SHA-256: `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`다.
- 최종 경로: `<global_dir>/related-terms/<dataset_id>/dataset.sqlite3`다.

### 무결성과 자원 제한

`download.lock`을 `File::try_lock`으로 잡은 프로세스만 임시 파일을 정리하고 다운로드한다. 잠금은 최종 rename과 임시 파일 정리가 끝날 때까지 유지한다. 다른 프로세스는 즉시 `InProgress`를 받는다. 임시 파일은 최종 파일과 같은 폴더의 `download.tar.zst.part`·`dataset.sqlite3.part`이며 실패하면 제거한다. 비정상 종료가 남긴 파일도 다음 잠금 소유자가 정리한다.

다운로드 상한은 67,108,864 bytes(64 MiB)다. `Content-Length`와 실제 스트림 양쪽을 제한한다. 연결 제한은 10초, 전체 HTTP 요청 제한은 120초, redirect 상한은 5회이며 client 자체 재시도는 끈다. zstd window 상한은 64 MiB, 해제된 tar 전체 상한은 데이터셋 크기 + 1 MiB다.

tar는 raw entry를 검사하며 정확히 하나의 일반 파일 `dataset.sqlite3`만 허용한다. 다른 이름, 절대·상대 탈출 경로, 링크, 디렉터리, 확장 metadata와 중복 entry를 거부한다. tar 종료 padding 뒤 entry도 검사하고 zstd frame 끝까지 읽는다. 추출 크기와 SHA-256이 맞고 파일 sync가 끝난 경우에만 rename한다. 이 단계는 SQLite를 열거나 인덱스를 만들지 않는다.

## 의존성

- `tar` 0.4.46을 추가했다. 외부 실행 파일 없이 tar entry의 종류·이름을 검사하기 위해 필요하며 기본 확장 기능은 껐다.
- 이미 Tantivy가 사용하던 `zstd` 0.13.3을 직접 의존성으로 명시했다. 의존성 트리에는 한 버전만 있다.
- 기존 `reqwest`에 `blocking` 기능을 추가했다. 별도 시작 스레드에서 제한된 스트림을 읽기 위해 사용한다. Jev client는 인증·redirect 정책이 달라 재사용하지 않고 ring provider 설정 방식만 공유한다. Jev 동작은 바꾸지 않았다.
- `flate2`는 기존 `[build-dependencies]`에만 남아 있다.

## 검증 결과

모든 명령의 작업 디렉터리는 `/Users/buyong/workspace/private/buyong-mcp`다. 중단 전 성공한 검사 로그를 확인했으며 이후 Rust 코드는 바뀌지 않아 같은 검사를 반복하지 않았다.

| 명령 | 결과 |
| --- | --- |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml` | exit 0이다 |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_fetch` | 10 passed, 0 failed다 |
| 같은 test 명령의 `config` 필터 | 16 passed, 0 failed다 |
| 같은 test 명령의 `jev` 필터 | 25 passed, 0 failed다 |
| `cargo tree --manifest-path apps/codemap-search/Cargo.toml -i zstd -e normal` | zstd 0.13.3 한 버전이다 |
| `python3 /Users/buyong/tmp/rterms-03-verify.py` | 기본 설정·버전 전환·잘못된 경로 타입의 MCP 세션 3개 모두 exit 0이다 |

다운로드 e2e 함수명은 모두 `test_related_terms_fetch_`로 시작한다.

1. `cut_connection`: 본문 중간 연결 종료를 거부한다.
2. `wrong_hash`: 틀린 SHA-256을 거부한다.
3. `rejects_extra_traversal_absolute_links_and_metadata`: 허용하지 않은 entry를 거부한다.
4. `expansion_past_size_bound`: 추출 크기 초과를 거부한다.
5. `download_byte_bound`: HTTP header와 스트림의 크기 상한을 확인한다.
6. `retry_cleans_stale_files`: 실패·남은 임시 파일 이후 재시도가 성공한다.
7. `local_and_cached_sources_do_not_download_or_rehash`: 사용자 파일·준비된 파일을 그대로 재사용한다.
8. `truncated_zstd_and_trailing_archive`: 잘린 zstd frame과 tar 종료 뒤 entry를 거부한다.
9. `concurrent_processes_download_once`: 두 프로세스 중 한 개만 내려받고 최종 hash가 일치한다.
10. `process_worker`: 위 다중 프로세스 검증에 쓰는 자식 테스트 진입점이다. 단독 호출은 환경변수가 없으면 바로 반환한다.

따라서 10개에는 실행 보조 진입점 1개가 포함되며 독립 무결성 검증은 9개다. 모든 실패 검증은 최종 파일과 두 임시 파일이 남지 않는지 함께 확인한다. 기본 MCP 세션에는 `synonyms`가 없고 새 `CODEMAP_HOME`에 `related-terms` 폴더도 생기지 않았다.

기존 검사 로그는 `/tmp/rterms-03-fetch-config/`와 중단된 담당의 실행 기록에 있다. 추가 확인의 report·stderr·동기화 설정은 `/Users/buyong/tmp/rterms-03-final-f6d_2tpe/`에 보존했다. 최초 sha2 출력 형식 컴파일 오류는 수정한 뒤 위 check와 테스트를 통과했다. 공개 URL 다운로드와 Windows/Linux의 실행 검증은 아직 수행하지 않았다.
