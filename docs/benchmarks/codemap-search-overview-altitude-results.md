# codemap-search overview 고도 보정 — fd before/after 실측 (2026-06-09)

> 브리프 `docs/briefs/2026-06-09-fix-codemap-overview-altitude.md` 구현 후 fd(@`25461e5`, 23파일/6,813 LOC) 실측.
> 대상: overview 세 고도(root/folder/file)의 응답 크기와 역할정합. 측정 바이너리: `--release`, CLI `codemap`(MCP overview와 동일 렌더러).

## 1. root overview — 핵심 지표

| 항목 | before | after | 변화 |
|---|--:|--:|--:|
| 응답 크기 | 20,688 B | **1,265 B** | **−93.9%** |
| 라인 수 | 597 L | **38 L** | −93.6% |
| per-symbol `[L..-..]` 라인 | 580 (97%) | **0** | 제거 |
| Total Symbols(표시) | 559 (raw) | 555 (significant) | 카운트 의미 변경 |

- before 원자료: `artifacts/cms-trace.log:1` (`overview {} -> 20688B 597L`, 본 문서 기준 상대 경로).
- **역할정합 AC PASS**: root 출력에 per-symbol 라인 위치(`[L..-..]`)가 **0개** — 완전한 심볼 인덱스로 동작 불가. 구조적으로 심볼 위치 탐색은 `search`로 강제됨.
- root는 이제 디렉터리 목록 + 파일당 1줄(`- File: {path} ({lines} lines, {N} symbols)`). 출력은 O(files), 심볼 수와 무관.

## 2. folder overview (`src`)

- after: 10,180 B. 라인범위 유지(설계대로 — folder/file 고도에만 `[L..]`).
- significant 필터 적용: exported + top-level 정의 유지, fn-local 드롭. 타입 멤버(struct 필드·메서드)는 유지 — 타입에 포함되며 fn에 포함되지 않으므로(`tests (mod)` 하위 fn도 mod 스코프라 유지).
- before 정밀 byte 미기록(사전 스냅샷 부재). 구조 변화: before는 root와 동일한 전체 per-symbol 덤프(로컬 포함) → after는 significant만.

## 3. file overview (`src/walk.rs`)

- after: 1,665 B / 58 L. 아웃라인(name/kind/range)만. before의 per-symbol flags/docstrings + `## Literals` 덤프 제거 → `read`/`grep` 담당.
- struct 필드(`items (field)`)·`const`·`type`·`enum` 모두 유지 확인.

## 4. significant 필터 — drop/keep 검증 + fd 한계 ⚠️

### 합성 repo 검증 (drop 경로 직접 확인)
fd는 drop 후보가 4개뿐이라 필터의 DROP 경로를 거의 안 때림 → 합성 Rust repo로 직접 검증:

| 분류 | 심볼 | 결과 |
|---|---|---|
| keep | `exported_fn`, `private_top_fn`(비-export top-level fn), `EXPORTED_CONST`, `PRIVATE_CONST`(비-export top-level const), `Thing`(struct), `field_a`(struct 필드), `method_kept`(메서드) | 전부 유지 ✓ |
| drop | `local_inside`(`let`), `accumulator`/`inner_assign`(assignment LHS), `method_local`(메서드 내부 `let`) | 전부 드롭 ✓ |

- 과잉 드롭 없음(top-level fn/const, struct 필드, 메서드 보존), 정확한 드롭(fn/메서드 내부 캡처 심볼 제거). range-containment 로직 정상.
- 메서드(`method_kept`, kind=`fn`)는 `impl`이 심볼이 아니라 어떤 fn에도 포함되지 않아 유지되고, 그 내부 `method_local`은 메서드(fn) 범위에 포함되어 드롭 — 설계 의도 그대로.

### fd 효과/한계
- fd 전체: raw 559 → significant 555 (**−4개**). Rust fd엔 캡처되는 fn-local이 거의 없어 **필터 효과는 미미**. fd root의 94% 감소는 거의 전부 per-symbol 열거 제거에서 나옴.
- **필터의 실제 시험대는 TS arrow-function 로컬**: parser의 TS 쿼리(`parser.rs:129-183`)는 `arrow_function`/function expression을 `symbol.fn`으로 캡처하지 않음. `const foo = () => { const x = ... }`에서 `foo`·`x`가 모두 `variable`로만 추출되고, `x`를 감싸는 fn-kind 부모가 없어 **range-containment 필터가 `x`를 드롭하지 못함**(누수).
- 결과적으로 vue(TS, 로컬 59%) folder 가중치는 명세대로의 필터만으로는 추정 ~41% 유지보다 **약하게 줄어들 가능성**. 이것이 브리프의 "필터 단독 vs folder-level cap" measure-then-decide 판정의 핵심 입력.
- parser.rs는 브리프상 `[hard]` out-of-scope이므로 본 변경에서 TS 쿼리는 손대지 않음. vue/scrapy 정밀 측정은 사용자 greenlight 시(deferred).

## 5. side-effect 검증

- `cargo test -p codemap-search`: 유닛 35 + e2e 89 = **전부 green** (0 실패). 영향받은 스냅샷 3건(`test_codemap_details_view`, `test_codemap_hierarchical_navigation`, `test_cross_extraction_codemaps`)은 새 아웃라인 형태로 재생성하되 의도(동적 재추출) 보존.
- search 출력 분기(`mcp.rs:400-457`)는 자체 텍스트로 generate_*_view를 호출하지 않음 → 미변경. e2e search 테스트 green이 이를 보증.
- CLI `codemap`과 MCP overview는 동일 렌더러 공유 — 일관성 유지.

## 6. fd behavioral 재관찰 (informational AC)

과제 `fd-feat-1`(`--total-count` 옵션 추가)을 에이전트에게 주고, fd 코드베이스를 **오직 codemap-search(cms.py 래퍼)로만** 탐색하도록 제약해 도구 사용을 관찰. anchor/기대값/baseline은 비공개(순환 편향 방지). 트레이스: `artifacts/cms-trace.log`(after) vs `artifacts/cms-trace-baseline.log`(before).

| | overview 호출 | read | search/grep/find | overview 총 바이트 |
|---|--|--:|--:|--:|
| before | 1회 (root, 20,688B — 전 심볼 라인범위 덤프) | 5 | 0 | 20,688 B |
| after | 2회 (root **1,264B** + folder `src` **10,179B**) | 5 | 0 | 11,443 B |

**실측이 지지하는 결론(딱 셋, 그 이상 주장 금지):**
- (a) **root 역할정합 (결정적)**: root가 더 이상 완전 심볼 인덱스로 동작 불가(1,264B, per-symbol `[L..]` 0개).
- (b) **overview 컨텍스트 −45%**: 20,688B → 11,443B.
- (c) **탐색의 2-고도화**: 단일 root 덤프 → root(구조) + folder(범위)로 분리 하강.

**behavioral 목표는 fd로 증명 불가(inconclusive), 해소 아님:**
- search 사용은 **0 → 0 (불변)**. 에이전트는 "덤프 1개 신뢰 → 덤프 2개(root+folder) 신뢰"로 바뀌었을 뿐, 여전히 search 교차검증 없이 overview만 신뢰한다. fd가 작다는 건 성공의 근거가 아니라 **측정 불가의 근거**다 — behavioral search-usage 게이트는 대형 티어(scrapy/vue)에서만 의미.
- **위치 정확도는 보존**: 슬림 overview에도 essential anchor 전부 정확히 특정(`src/cli.rs:574-596`, `src/walk.rs:282-293` `stop()`, `src/config.rs:135`·`src/main.rs:377`, 카운터 원천 `src/walk.rs:148/220`). 슬림화가 탐색 품질을 해치지 않음.

**folder 라인범위 제거 결정 → R2 재관찰로 검증 (사용자 결정 반영):**
- R1에서 과신이 root→folder로 이동한 것을 확인 후, 사용자가 **folder도 라인범위 제거(search 강제)**를 선택. folder는 significant 심볼 목록(이름/kind)까지만, 정확 위치는 search/grep으로 강제. fd `src` folder: 10,180B → **7,294B**(−28%), folder `[L..]` 0개. file view(단일 파일 명시)는 라인범위 유지.

### 3-라운드 도구 사용 비교 (실측, decisive)

| 라운드 | 환경 | overview | read | grep | search | 해석 |
|---|---|--:|--:|--:|--:|---|
| before | 변경 전 | 1 (20,688B 전량덤프) | 5 | 0 | 0 | root 덤프 1개를 전적 신뢰 |
| after R1 | root만 슬림 | 2 | 5 | 0 | 0 | 과신이 folder로 **이동**(folder가 여전히 locator) |
| after R2 | folder도 범위제거 | 2 | 6 | **5** | **1** | overview=orient, **search/grep=locate** |

- **R2 트레이스**: `overview {}`(구조) → `overview src/walk.rs`(단일 파일 구조) → `grep quiet`/`grep num_results`/`search "quiet flag arg cli option"`로 정확 위치 탐색 → 타깃 read. `cms-trace.log` 기록.
- **결론**: folder 라인범위 제거로 "overview가 search를 대체"하는 패턴이 root·folder 양쪽에서 사라짐. 에이전트가 비로소 search/grep을 locator로 사용. 사용자의 개념적 불만(overview ≠ search) 해소.
- **위치 정확도 보존**: 슬림+범위제거에도 essential anchor 전부 정확 특정(`cli.rs:585-624`, `walk.rs:282` `stop()`, `config.rs`, `main.rs` construct_config, 카운터 `walk.rs:148/220`). 탐색 품질 무손상.
- **별도 트랙(deferred)**: folder "무게"(대형 deep 폴더에서 significant 심볼 목록 자체가 큰지)는 vue 티어 measure-then-decide로 여전히 분리 — 이번 변경은 locator 성격을 없앤 것이지 무게 cap이 아님.

## 7. 메트릭 의미 변경 주의 (bench 하베스 정합)

- root overview의 headline `**Total Symbols**`가 **raw 추출 총량 → significant(필터 후) 합**으로 의미가 바뀜. 인덱스는 여전히 모든 심볼(로컬 포함)을 보유하며 `search`는 전부 찾는다 — 줄어든 건 overview 표시값뿐.
- 파급: `harness/bench_axisA.py:52`의 `codemap_counts()`가 `Total Symbols`를 regex 파싱한다. 재실행 시 이 값은 이제 significant를 집계하므로, `codemap-search-axisA-results.md`의 심볼 수(fd 559 / ripgrep 4,094 / scrapy 15,398 / vue 19,536 = **raw/indexed**)와 의미가 달라진다.
- 조치: axisA 문서의 심볼 수는 raw/indexed로 그대로 유효(인덱싱 산출물 기준). 두 지표를 혼동하지 않도록 axisA 문서에 주석을 추가했고, bench를 재실행해 overview 기반 심볼 수를 다시 쓸 경우 컬럼을 "significant"로 라벨링할 것.
</content>
</invoke>
