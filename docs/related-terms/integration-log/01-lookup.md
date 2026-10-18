# 01 — 뜻 데이터셋만 사용하는 Rust 조회

작업일은 2026-10-17이다. 시작 HEAD는 `6f5a85d7a8558998582c8f9414beafb7ea18f0fd`이며 브랜치는 `feat/related-terms-lookup`이다. 후속 04 담당에게 조회 API와 검증 근거를 넘긴다.

**176,351개 전수 비교에서 identical은 176,189개, resolution-differs는 162개, other는 0개다.** 해석 차이 162개 모두 입력 매칭 자료의 차이로 추적했고, 224개 공식 평가 입력의 expected-field 적중은 양쪽 모두 193개다. 후속 MCP의 호출당 term cap은 **16**으로 정한다. 이는 측정한 batch 최대 시간을 바탕으로 정한 입력 개수 제한이며, 모든 입력의 실행시간을 보장하는 값은 아니다.

## 1. 구현 범위와 인계 API

구현은 `apps/codemap-search/src/related_terms/`에 있다. `lib.rs`에 모듈 한 줄만 등록했다. fetch, config, MCP handler, evidence는 변경하지 않았다. `main.rs`와 `src/mcp/`에는 이 모듈을 호출하는 참조가 없으므로 이 단계만으로 서버 시작 시 데이터셋을 열지 않는다. 새 의존성, 테스트, lint·formatter 설정은 추가하지 않았다.

| 경로 | 역할 |
| --- | --- |
| `src/related_terms/prepare.rs` | 복사·잠금·인덱스·ready marker를 관리한다 |
| `src/related_terms/normalize.rs` | `canonical_surface`, `exact_key`, `word_key`, 기본형 후보, 확인된 별칭 11개를 구현한다 |
| `src/related_terms/syntax.rs` | fallback 입력 부분의 syntax 역할을 판별한다 |
| `src/related_terms/lookup.rs` | lazy connection, 입력 해석, 뜻·관련어 수집, 정렬, 중복 제거, 캐시를 구현한다 |
| `examples/related_terms_dump.rs` | 입력당 JSON 한 줄을 쓰는 개발용 Cargo example이다. Cargo의 example 자동 탐색을 사용한다 |
| `experiments/related-terms-poc/sense-eval/compare_rust_lookup.py` | reference 생성, 전수 비교, 차이 추적, 공식 평가, 준비·의미 검증을 실행한다 |
| `experiments/related-terms-poc/sense-eval/measure_rust_lookup.py` | cold·단건·batch 시간을 측정한다 |

위 경로는 모두 `apps/codemap-search/` 아래의 경로다. 공개 CLI subcommand나 MCP 도구를 추가하지 않았다.

```rust
prepare_dataset(
    source: &Path,
    global_dir: &Path,
    is_user_owned: bool,
) -> Result<PreparedDataset, String>

PreparedDataset::path(&self) -> &Path
PreparedDataset::dataset_id(&self) -> &str

RelatedTermsLookup::new(prepared: PreparedDataset) -> Self
RelatedTermsLookup::with_options(prepared: PreparedDataset, options: LookupOptions) -> Self
RelatedTermsLookup::lookup(&mut self, input: &str) -> Result<LookupResult, String>
RelatedTermsLookup::lookup_batch(&mut self, terms: &[String]) -> Result<Vec<LookupResult>, String>
```

`prepare_dataset`은 동기 작업이므로 startup worker에서 호출해야 한다. `RelatedTermsLookup` 생성자는 I/O를 하지 않는다. 첫 조회가 준비 잠금과 marker를 확인하고 read-only SQLite 연결을 연다. SQL은 필요한 행만 읽으며 `prepare_cached`로 문장을 재사용한다. 시작 시 데이터셋 전체를 메모리에 올리지 않는다. 문장 캐시는 32개, 결과 캐시는 128개 FIFO다. 단어 정보는 16,384개, 주제 정보는 256개까지 보관한 뒤 해당 정보 캐시를 비운다.

첫 조회도 파일 잠금을 기다릴 수 있으므로 후속 MCP 담당은 준비·최초 open을 sequential MCP loop에서 직접 실행해서는 안 된다. 오류는 하위 계층의 `Result<_, String>`으로 전달하며, 다운로드·준비 중 상태를 MCP에 매핑하는 일은 03·04 범위다.

`LookupOptions.skip_syntax_parts`의 기본값은 `false`다. 기준 PoC의 `sense_lookup.py`는 `--skip-syntax-parts`를 `store_true`로 선언하며, `category_lookup.resolve`도 명시적으로 켠 경우에만 부분을 건너뛴다. 부모 확인에 따라 기본값을 보존하고 명시적 `true` 옵션을 구현했다. 옵션은 조회 인스턴스에 고정되므로 모드 사이에 결과 캐시가 섞이지 않는다. 이 옵션에 대응하는 새 설정 키나 MCP 인자는 만들지 않았다.

### 결과와 상태

- `LookupResult.resolution`은 `mode`(`whole`, `parts`, `none`)와 순서 있는 `seeds`(`span`, `terms`)를 담는다.
- `senses`의 각 `SenseResult`는 분야·주제·개념, 표시용 `direct`·`near`·`same_topic`, `full_word_keys`를 담는다. `full_word_keys`는 해석된 입력 자신을 제외하고 **뜻 사이 표시 중복을 제거하기 전**의 canonical word 집합이다.
- `unsorted_direct`는 뜻에 배정되지 않은 직접 관계어를 담는다. `candidate_count`는 입력별 중복을 제거한 관련어 수다.
- `looked_up_inputs`에는 전체 표현과 시도한 구성 표현이 남는다. `appVersion`은 `appVersion`, `app`, `version`을 기록하지만 이미 분류된 전체 표현의 뜻을 구성 단어 뜻으로 넓히지는 않는다.
- `status=ok`는 관련어가 하나 이상 있다는 뜻이다. `no_sense`는 선택된 seed는 있으나 관련어가 없다는 뜻이고, `no_match`는 선택된 seed와 관련어가 없다는 뜻이다. 이것은 rtr1의 원시 matcher 상태가 아니라 rts5 조회의 상태다.

`token`은 26개 뜻, `appVersion`은 2개 뜻, `abort signal`은 17개 뜻을 반환했다. `token`의 `context`는 두 뜻의 `full_word_keys`에 남고 표시 그룹에는 한 번만 나온다. `lookup_batch`는 비교용 하위 API로 개수 제한을 강제하지 않는다. **04 handler는 호출당 16개 제한을 입력 검증에 적용해야 한다.** 출력의 뜻 수·그룹별 단어 수·전체 byte cap은 04 범위다.

## 2. 준비 계약과 검증

데이터셋 id는 `sense_meta.dataset_id`에서 읽는다. `sense_meta.schema_version`이 `rt-sense/5`가 아니면 발견한 버전을 포함해 거부한다. id는 경로 구성 전에 ASCII 영숫자·`-`·`_`만 허용한다.

```text
<global_dir>/related-terms/
  <dataset_id>/                  # is_user_owned=false
  local/<dataset_id>/            # is_user_owned=true
    dataset.sqlite3
    prepare.lock
    ready.json
```

`prepare.lock`은 `File::try_lock`으로 잡으며, `WouldBlock`이면 10ms씩 약 30초까지 재시도한다. 복사부터 marker 게시까지 같은 잠금을 유지한다. reader도 최초 open 시 같은 잠금을 잡는다. SQLite busy timeout은 30초다.

user-owned source는 read-only로만 연다. 같은 대상 폴더에 임시 복사본을 만들고 다음 세 인덱스를 transaction에서 생성한 뒤 완성된 파일만 rename한다. 원본이 read-only여도 쓰기 권한은 소유한 임시 복사본에만 부여한다.

```sql
CREATE INDEX IF NOT EXISTS related_terms_by_word ON relations(word_key);
CREATE INDEX IF NOT EXISTS related_terms_by_persona ON relations(persona);
CREATE INDEX IF NOT EXISTS related_terms_personas_by_topic ON personas(topic);
```

`ready.json`은 `dataset_id`, `index_version`, `source`를 기록한다. 현재 `INDEX_VERSION`은 `1`이다. user-owned source의 `source`에는 canonical path, byte 크기, 수정 시각의 nanoseconds가 들어가고 managed source는 `null`이다. 같은 source와 현재 version의 marker가 있으면 재색인하지 않는다. source가 바뀌면 다시 복사하며, 복사 도중 source의 크기·수정 시각이 바뀌어도 거부한다. version만 바뀌면 기존 복사본에서 세 인덱스를 다시 만들고 marker를 교체한다. 다운로드는 요청하지 않는다.

갱신 전에는 marker를 제거한다. SQLite 파일을 sync·rename한 다음 별도의 임시 marker를 sync·rename한다. Unix에서는 각 rename 뒤 폴더도 sync한다. reader는 marker가 없거나 id/version이 맞지 않으면 열지 않는다. 임시 파일과 잠금의 수명은 소유 객체가 관리한다.

`preparation-report.json`으로 다음을 확인했다.

| 검증 | 실제 결과 |
| --- | --- |
| source SHA-256 전후 | `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`로 같다 |
| user-owned 위치 | scratch의 `home/related-terms/local/rts5-75e02862daa29308/dataset.sqlite3`다 |
| 두 번째 준비 | SQLite bytes·mtime·marker bytes가 바뀌지 않았다 |
| 인덱스 | 기존 것에 정확히 위 세 개만 추가됐다 |
| 새 폴더에 두 프로세스 동시 준비 | 복사본·잠금·marker 각각 하나만 남았고 `PRAGMA quick_check`는 `ok`다 |
| index version 변경 | version을 0으로 내리고 인덱스 열을 일부러 바꾼 scratch에서 올바른 정의로 재생성됐다 |
| source 변경 | scratch source 수정이 복사본에 반영됐고 수정된 source 자체는 다시 바뀌지 않았다 |
| 잘못된 schema | `unsupported related-terms schema "rt-sense/999"; expected rt-sense/5`로 거부했다 |

## 3. 비교 방법과 결과

로컬 rtr1은 451,076,096 bytes이며 `lookup-v3/target/release/rtpoc3`도 존재함을 먼저 확인했다. `sense_lookup.py token --no-similarity-order --format json`은 성공했다. Rust 런타임은 이 두 입력을 읽거나 실행하지 않는다. Python reference·평가 스크립트만 이를 사용한다.

reference 모집단은 기존 migration 검증과 같은 `own senses ∪ input_senses`의 정렬된 key이며, 이를 표시 표기로 바꾼 176,351개 전부다. PoC의 다른 CLI 기본값은 유지하고 similarity ordering만 껐다. reference와 Rust의 전체 JSONL을 저장소 밖에 남겼다.

비교 필드는 다음과 같다.

1. 해석 `mode`와 순서 있는 seed의 `span`, `terms`다. matcher 내부 provenance인 `match=exact/lemma/joined...`는 비교 필드가 아니며 PoC 원시 답안에는 보존했다.
2. 순서 있는 뜻의 `span`, `field`, `topics`와 각 뜻의 순서 있는 `direct`, `near`, `same_topic` 단어다.
3. 순서 있는 `unsorted_direct`와 그 단어다.

해석이 다르다는 이유만으로 통과시키지 않았다. Rust의 seed를 변경하지 않은 PoC 수집기에 주입한 별도 counterfactual 결과가 모든 비교 필드를 재현해야 `resolution-differs`로 분류했다. 기준 답안은 바꾸지 않았다. 그 뒤 해당 입력의 실제 rtpoc3 matcher 후보와 rts5 표기를 다시 읽어 각 차이의 출처를 확인했다.

| 분류 | 입력 수 |
| --- | ---: |
| identical | 176,189 |
| resolution-differs | 162 |
| other | 0 |
| 합계 | 176,351 |

| 162개 해석 차이의 추적 원인 | 입력 수 | 예시 |
| --- | ---: | --- |
| PoC에는 plain 별칭이 있지만 rts5 대표 표기에는 없다 | 124 | `AddOptions`의 PoC seed는 `add_option`, `AddOptions`이고 Rust는 `AddOptions`다 |
| rts5 key는 있지만 PoC의 whole matcher 후보에는 없다 | 35 | `RISING`의 PoC seed는 `rise`, `RISING`이고 Rust는 `ris`, `rise`, `RISING`이다 |
| PoC matcher에는 기호 표기만 있고 rts5 표기는 plain이다 | 3 | `at times`에 Rust는 `atTime`도 선택한다. PoC의 해당 후보 표기는 `at_time`이다 |

원인 미확인 입력은 0개다. 이 전수 비교에서 기본형 근거 관계의 출처 차이로 남은 입력은 0개다. 기본형 tie 자체는 rts5의 양방향 `form`과 incoming relation으로 판정한다. 각 입력의 양쪽 seed, 선택이 달라진 key, rts5 표기, 실제 PoC matcher 후보는 `resolution-traces.jsonl`에 있다. 부록에는 162개 입력을 원인별로 모두 열거한다.

### 공식 평가와 보충 검증

`eval-set.json`의 240개 중 `expected_fields`가 있는 224개를 같은 no-similarity 기준으로 채점했다.

| 지표 | PoC | Rust |
| --- | ---: | ---: |
| expected-field hit | 193/224 | 193/224 |
| main expected-field hit | 177/224 | 177/224 |
| first-sense expected-field hit | 155/224 | 155/224 |

hit 여부가 바뀐 입력은 0개다. 기본 false·명시적 true의 syntax 모드는 각각 300개 입력에서 동일 모드 PoC와 모두 일치했다. 이 300개는 공식 240개와 syntax 예시, 정규화 예시, 확인된 별칭 11개, 보충 입력을 합쳐 중복을 제거한 수다.

stage-13 기록에는 원래 사전 밖 입력 16개의 전체 목록이 없고 네 예시만 있다. 따라서 원래 16개를 재현했다고 주장하지 않는다. 기록된 `blocked`, `retried`, `appVersion`, `getUserName`에 굴절형·식별자·미등록 표현을 점검하는 다음 12개를 더했다: `cacheEntries`, `abortSignals`, `running`, `stopped`, `parsed`, `policies`, `retries`, `loadUserProfile`, `http2Server`, `unknownRtermsProbe`, `user_session_count`, `nonexistent rterms probe`다. 이 보충 16개도 위 두 모드 비교에 포함됐고 일치했다. 원래 전체 목록 미확인은 남은 한계이며 공식 전수·224개 평가를 대체하지 않는다.

## 4. 성능과 term cap

측정 환경은 Darwin arm64, `rustc 1.98.1 (48a229cea 2026-09-01)`의 release example이다. cold는 새 프로세스·새 연결의 첫 조회이며 open과 최초 SQL 준비를 포함한다. 데이터셋 준비와 JSON 직렬화는 제외했고 OS file cache는 비우지 않았다. 단건 측정은 `token`으로 연결을 연 뒤 공식 240개와 seed `20261017`의 무작위 1,000개를 각각 조회했다. p95는 nearest-rank로 계산했다.

| 구분 | 호출 수 | p50 (ms) | p95 (ms) | max (ms) |
| --- | ---: | ---: | ---: | ---: |
| cold `token` | 10 | 8.953 | 53.460 | 53.460 |
| 공식 240개 단건 | 240 | 0.976 | 6.531 | 11.754 |
| 고정 표본 1,000개 단건 | 1,000 | 0.221 | 1.189 | 5.501 |
| batch 1개 | 1,240 | 0.203 | 1.752 | 10.208 |
| batch 2개 | 620 | 0.435 | 3.485 | 12.528 |
| batch 4개 | 310 | 0.836 | 7.881 | 22.712 |
| batch 8개 | 155 | 1.687 | 14.391 | 35.782 |
| batch 16개 | 78 | 3.255 | 31.363 | 64.090 |
| batch 32개 | 39 | 7.209 | 62.211 | 110.728 |

batch는 단건에서 느렸던 서로 다른 입력부터 배치해 한 호출 전체의 시간을 쟀다. batch 크기별로 새 프로세스를 사용했고 마지막 batch는 남은 입력만 담았다. 결과 캐시 hit만 반복하는 부하를 피했다. 관측 batch 예산 100ms 안에 든 가장 큰 측정 크기가 16이므로 **호출당 cap은 16**이다. 32개는 최대 110.728ms로 예산을 넘었다. 임의 길이·임의 표현에 대한 시간 상한은 아니므로 후속 MCP는 출력 cap과 비차단 작업 경계도 지켜야 한다.

부모가 같은 머신에서 실제 codemap-search MCP `search`를 10회 순차 호출했다. 조건은 `query=token`, `workspace_scope=apps/codemap-search`, `include_seen=true`, 나머지 기본값이다. p50은 6,307ms, p95/max는 6,550ms이며 모두 `isError=false`였다. 부분 source analysis와 vendor event input unavailable 안내는 있었다. lookup 단건 p95는 이 참고 시간보다 낮다. **search 수치는 MCP transport를 포함한 부모 측 벽시계이고 lookup 수치는 엔진 단독 시간이므로, 이를 같은 측정 경로의 속도 배수로 해석해서는 안 된다.** 부모 증거는 `/Users/buyong/tmp/rterms-search-token-latency-20261017.json`에 있다.

## 5. 검증 명령과 산출물

모든 명령의 cwd는 `/Users/buyong/workspace/private/buyong-mcp`다. 아래 변수는 실제 사용한 경로다.

```bash
S=/Users/buyong/tmp/rterms-01-5_i16l71
D=apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3
E=apps/codemap-search/experiments/related-terms-poc/sense-eval
B=$S/target/release/examples/related_terms_dump

python3 "$E/compare_rust_lookup.py" --reference --output "$S"
CARGO_TARGET_DIR="$S/target" cargo check --manifest-path apps/codemap-search/Cargo.toml
CARGO_TARGET_DIR="$S/target" cargo build --release --manifest-path apps/codemap-search/Cargo.toml --example related_terms_dump
python3 "$E/compare_rust_lookup.py" --output "$S" --binary "$B"
python3 "$E/compare_rust_lookup.py" --trace-resolutions --output "$S"
python3 "$E/compare_rust_lookup.py" --eval --output "$S" --binary "$B"
python3 "$E/compare_rust_lookup.py" --prepare-check --output "$S" --binary "$B"
python3 "$E/compare_rust_lookup.py" --semantics-check --output "$S" --binary "$B"
python3 "$E/measure_rust_lookup.py" --output "$S" --binary "$B" --dataset "$D" --search-p95-ms 6550 --batch-budget-ms 100
CARGO_TARGET_DIR="$S/target" cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests
cargo package --list --allow-dirty --manifest-path apps/codemap-search/Cargo.toml
```

위 실행은 모두 성공했다. 마지막 Rust 수정 뒤 `cargo check`는 exit 0이며 기존 e2e는 **235 passed, 0 failed, 1 ignored**다. package 목록은 476개이며 `.sqlite3`·`.tar.zst`는 없다. 새 테스트는 추가하지 않았다. 재개 시 Rust 파일이 마지막 성공 check와 e2e 이후 바뀌지 않았음을 확인해 완료된 대규모 검증을 반복하지 않았다.

실패했던 명령은 성공 기록과 구분한다. 최초 공유 target의 기본 `cargo check`는 통과했으나 뒤의 `--example related_terms_dump` check는 `E0432: unresolved import codemap_search::related_terms`로 실패했다. 실제 `lib.rs` 등록과 Cargo lib 경로는 일치했다. 새 외부 `CARGO_TARGET_DIR`의 `--example ... -vv` check는 통과했고, example의 `--extern codemap_search`가 그 target의 새 `.rmeta`를 가리킴을 확인했다. 공유 target 오류 원인은 확정하지 않았으며 다른 writer/build 경쟁으로 단정하지 않는다. 후속 검증과 측정은 모두 독립 target을 사용했다.

또한 최초 `cargo package --list`는 미커밋 변경 7개 때문에 exit 101로 거부됐다. 커밋이나 정리를 하지 않고 `--allow-dirty --list`로 현재 작업 트리의 목록만 확인해 성공했다.

| scratch 산출물 | 내용 |
| --- | --- |
| `words.json`, `reference.jsonl`, `reference-summary.json` | 176,351개 모집단과 변경하지 않은 PoC 답안이다 |
| `rust.jsonl`, `comparison-report.json`, `differences.jsonl` | 최종 전수 Rust 답안과 분류다 |
| `difference-matcher.json`, `resolution-traces.jsonl`, `resolution-trace-report.json` | 162개 각각의 선택 차이와 실제 출처 근거다 |
| `eval-reference.jsonl`, `eval-rust.jsonl`, `eval-report.json` | 공식 224개 채점 및 240개 원시 응답이다 |
| `semantics-report.json`, `semantics-syntax-*.jsonl`, `smoke.jsonl` | 두 syntax 모드, 보충 입력, full word set 검증이다 |
| `preparation-report.json`, `preparation-hb_3guwf/` | 새 폴더의 원본 불변·동시 준비·version·schema 검증이다 |
| `latency-report.json`, `latency-sample.json`, `cold-*.stderr`, `eval240.stderr`, `sample1000.stderr`, `batch-*.stderr` | 개별 실측과 집계다 |
| `check-base-rule.log`, `build-base-rule.log`, `e2e.log` | 최종 Rust 변경에 대한 check·release build·기존 e2e 결과다 |
| `example-check.log`, `isolated-check.log`, `package-list.stderr`, `package-list-allow-dirty.log` | 실패 원문과 독립 재검증·패키지 목록이다 |
| `round1/` | 공백을 포함하는 기본형 후보 규칙을 수정하기 전 결과다. 최종 기준으로 사용하지 않는다 |

최종 rts5 크기는 31,715,328 bytes이고 SHA-256은 고정값과 같다. 시작 HEAD를 기준으로 모든 tracked dataset 및 금지된 네 PoC 조회 파일의 `git diff --stat`는 비었다. staged 파일은 0개다. 사용자 기존 변경과 untracked dataset·`__pycache__`는 정리하지 않았다. 게시·tag push·외부 변경은 하지 않았다.

## 6. 후속 담당이 지킬 경계와 한계

1. 03은 이 모듈의 기존 등록과 API를 보존하고 fetch submodule만 추가해야 한다. 내려받은 파일은 `is_user_owned=false`, 사용자가 준 파일은 반드시 `true`로 준비해야 한다.
2. 04는 background 준비와 최초 open, 호출당 16개 제한, 출력 cap, 준비 실패의 MCP 상태 매핑을 담당한다. 기본 syntax 옵션은 false다.
3. 05는 뜻의 지원 여부를 표시 그룹이 아니라 `full_word_keys`로 판단해야 한다. 사용자 승인 전 removal 기본값을 켜서는 안 된다.
4. 동시 준비와 실행 검증은 이 Darwin arm64 환경에서 했다. Windows에서 열린 SQLite 파일 교체 동작은 실측하지 않았다.
5. 원래 stage-13의 16개 전체 목록은 미확인이다. 기록된 네 입력과 추가 12개의 보충 검증으로 명확히 구분한다.
6. search MCP와 lookup latency의 측정 경로가 다르다. 100ms는 관측 batch 예산이며 실행 deadline이 아니다. 독립 target은 성공했으나 최초 공유 target E0432의 근본 원인은 미확정이다.

자기 검토에서는 준비 잠금·marker 게시 순서, user-owned source의 read-only 경계, syntax 옵션의 캐시 격리, full word set과 표시 중복의 분리, 비교 필드·출처 추적·실측 분모를 확인했다. 이 01 인계 범위에서 추가로 확인된 blocker는 없다. 독립 reviewer 검토는 별도 acceptance gate로 남긴다.

## 부록 — 해석 차이 162개 전체

아래 각 목록의 모든 입력에는 해당 소제목의 원인이 적용된다. 양쪽에서 선택한 base/matched word와 직접 근거는 scratch의 `resolution-traces.jsonl`에서 같은 `input`으로 조회한다.

### PoC plain 별칭이 rts5 대표 표기에 없는 124개

`--baseline-results`, `--rebase-merges`, `AddOptions`, `addTests`, `algebraic types`, `allow_failures`, `all pairs shortest paths`, `anti-patterns`, `asString`, `auth.users`, `authorizeRequests`, `baseline_results`, `bind_params`, `buildTypes`, `built-in types`, `c.String`, `cache clearing`, `cache nulls`, `chat messages`, `CI/CD pipelines`, `col_names`, `conn string`, `CoT prompting`, `C string`, `data files`, `date string`, `decode_tokens`, `density-independent pixels`, `download artifacts`, `duplicate files`, `exchange binding`, `execution counts`, `failed logins`, `fileOpened`, `findElements`, `finetuned`, `finish_reasons`, `from_string`, `f-strings`, `getKeys`, `get_params`, `getUsages`, `gotests`, `GPG signed`, `GPG signing`, `grid-rows`, `half-closed`, `has_perms`, `hex decoding`, `hex-encoded`, `in-app notifications`, `indegrees`, `isEnded`, `isListed`, `JsonUtils`, `keyNames`, `key naming`, `klinecharts`, `left-handed`, `live updates`, `locale-aware sorting`, `logged-in users`, `low level designs`, `micro-optimizations`, `month names`, `multiprocessing`, `namespace names`, `n-gram indexing`, `no-args`, `null-terminated strings`, `oldVals`, `on_deleted`, `outbox_events`, `outbox_messages`, `partial sums`, `perf counters`, `plotLines`, `port forwards`, `prior boxes`, `projectNames`, `python versions`, `queue binding`, `remote-tracking branches`, `remove_stopwords`, `right-handed`, `row numbers`, `rule groups`, `run commands`, `scoped locking`, `self-referencing`, `SendMessages`, `set_params`, `shell quoting`, `show usages`, `sourceDirs`, `spec/helpers`, `SQS consumers`, `ssh keys`, `sub-routes`, `systemInstructions`, `tab contents`, `table driven tests`, `tbl_users`, `timeout_secs`, `tokenTypes`, `tpl_string`, `trailing spaces`, `two passes`, `type-level functions`, `unnecessary re-renders`, `upload artifacts`, `user-defined exceptions`, `user defined functions`, `user-perceived characters`, `Viterbi decoding`, `well-known ports`, `write-ahead logging`, `write-behind caching`, `write-through caching`, `zero cost abstractions`, `zero_grads`, `zero-initialized`, `zero padding`, `zero-shot prompting`이다.

### rts5 key가 PoC whole matcher 후보에 없는 35개

`...props`, `.git/hooks`, `_.values`, `attribute setting`, `auto waiting`, `backlinks`, `C# naming`, `C++ templates`, `dismiss stale reviews`, `Django templates`, `fs_paths`, `gles`, `hive partitioning`, `hkeys`, `HttpUtils`, `hypothesis testing`, `hypothesis tests`, `ip masking`, `lifecycle scripts`, `named branches`, `ntp servers`, `OTA updates`, `reverse mapping`, `RISING`, `secret versioning`, `secret versions`, `split tunneling`, `spread arguments`, `STDOUT.puts`, `stimes`, `stop bits`, `tmpfs`, `triggerCharacters`, `unicode property escapes`, `unprotected`다.

### PoC는 기호 표기, rts5는 plain 표기인 3개

`at times`, `formatNamed`, `new_items`다. PoC matcher의 해당 표기는 각각 `at_time`, `format_name`, `New-Item`이고 rts5 표기는 각각 `atTime`, `format name`, `new item`이다.
