# P0 — codemap-search 개선 #1~#7 구현 (실행 보고서)

- 브랜치: `perf/cms-improve-20260615` (HEAD `9f1bb8ada`에서 분기)
- 빌드: `cargo build --release` (apps/codemap-search) — **clean, 경고 0**
- 테스트: `cargo test --release` — **전부 통과** (e2e 102 + extraction-snapshot 14 + 단위/doc 0; 0 failed)
- 스모크: CLI `--help` + CLI `search` + MCP `mcp` 세션(합성 exec+legacy 픽스처, 실제 `src` 인덱스) — panic 없음, 신규 기능 동작 확인
- 범위: #1~#7만. **#8(매크로 dispatch 심볼 추출/인덱스 포맷 변경)은 의도적으로 미구현.**
- 변경 파일(HEAD 대비, 5개): `index/engine.rs`, `index/ranking.rs`, `tools/search/mod.rs`, `tools/search/render.rs`, `callers/annotate.rs`. parser 등 다른 파일은 미변경.

## 항목별 (changed file + function + 1줄 what)

| # | 파일 | done | 함수/위치 | 한 일 |
|---|---|---|---|---|
| 1 | `index/ranking.rs` (+`index/engine.rs`) | yes | `qualified_literal_exact_hit`, `is_qualified_word`, post-rank 블록, `SearchResult.qualified_literal_hit` 필드 | qualified name(`::`/`.`)이 literal에만 정확히 일치하면 `QUALIFIED_LITERAL_SCORE_BOOST=1.6` 가점. **symbol-exact 히트가 없을 때만** 적용해 `EXACT_NAME_SCORE_BOOST=3.0`과 곱해지지 않음 → co-exposure(legacy가 exec와 함께 detail/tail 진입), exec를 밀어내지 않음 |
| 2 | `tools/search/mod.rs` | yes | `match_reason`(non-fallback 분기) + tail 렌더 루프(양 sub-branch) | 매칭된 qualified literal을 `match_reason`("matched literal: \`...\`")과 tail row 양쪽에 노출. tail은 `qualified_literal_hit`이 Some이면 symbol-notes/ bare-path 두 경로 모두에서 literal을 맨 앞에 표기 → `encoding [L20]` 오라벨 해소 |
| 3 | `tools/search/render.rs` | yes | `render_anchored_symbols` 데모트 분기 | `is_match = is_full_anchor && !is_overcap_anchor`로 `!any_match` short-circuit 제거 → 심볼이 query와 무매칭인(=`!any_match`, forward-decl/대형 비-anchor) 파일에서 full snippet 대신 3줄 시그니처. `any_match`일 땐 바이트 동일(공통 경로 비회귀), `symbol_fallback` 경로는 무매칭 심볼이 도달 안 함(순수 path/docstring 가시성 유지) |
| 4 | `tools/search/mod.rs` | yes | `CrossPathPresence`(신규), `ambiguity_note`(시그니처 변경) | 결과 전체를 1회 스캔해 동일 qualified name이 구현 심볼 파일 + dispatch literal 파일 양쪽에 있으면 "이 이름은 N개 경로에 존재 (구현 심볼 X + dispatch 리터럴 Y) — 둘 다 확인"으로 교체. 기존 동명-개수 노이즈는 cross-path가 없을 때만 fallback |
| 5 | `tools/search/mod.rs` | yes | `match_reason`, `ambiguity_note`, `format_read_suggestion`, 힌트 조립부 | match_reason 라벨을 짧은 enum형(`exact`/`owner-exact`/`token N/M`)으로; read_suggestion을 `read <path>:<offset> (<limit> lines)` 단축형(JSON 제거, 단축이지 제거 아님); 힌트를 `- match: <reason>; <ambiguity>` 1줄 + `- <read>` 1줄로 압축 |
| 6 | `tools/search/mod.rs` | yes | `diversified_order`(신규), `parent_dir`, `run()` detail-head 적용 | top-5 detail이 한 디렉토리에 독식되지 않도록 부모 디렉토리당 `DETAIL_DIR_CAP=3` 소프트 캡. 점수-비율 가드(`DIVERSITY_MIN_SCORE_RATIO=0.6`) 추가 → 훨씬 약한 타-디렉토리 파일이 강한 동일-디렉토리 파일을 밀어내지 않음. 밀린 파일은 tail로(드롭 없음) |
| 7 | `tools/search/render.rs` + `tools/search/mod.rs` + `callers/annotate.rs` | yes | `render_anchored_symbols`(파라미터 추가), `run()` `'files` 루프 위 dedup 생성, callee 렌더 | (a) `CallerBlockDedup`을 파일 루프 **밖**에서 1개 생성해 `&mut`로 주입 → cross-file caller-block dedup("same as `name` above")이 파일 경계 넘어 동작. (b) target-ambiguous callee 줄(`X (N defs, target ambiguous)`) 억제 → 액션 불가 라인 대신 "(N ambiguous callee(s) suppressed)" 카운트 1줄 |

## 가드레일 준수
- **`read_output_byte_cap` 미변경** (config.rs 손대지 않음). codex −74%의 원천 보존. ✅
- **co-exposure, re-rank 아님**: #1은 symbol-exact가 있으면 발동 안 함(else-if). 스모크에서 exec `BuiltinFunctionExec.decode`가 rank 1, legacy `fnc/mod.rs`가 rank 2로 **함께** 노출 — exec 위로 올리지 않음. ✅
- **인덱싱/추출 포맷 불변**: parser/lang 미변경, `EXTRACTION_FORMAT_VERSION` 영향 없음(#8 보류). ✅
- **#3 비회귀**: `any_match=true`(심볼이 매칭된 일반 케이스)에서 데모트 식이 변경 전과 바이트 동일. 순수 path/docstring(`symbol_fallback`) 가시성 유지. ✅

## 스모크 테스트 결과 (요지)
합성 픽스처(`exec/builtin.rs`에 심볼 `decode`, `fnc/mod.rs`에 `"encoding::base64::decode"` literal만)로 MCP `search "encoding::base64::decode builtin function"` 실행:
```
### File: exec/builtin.rs (8 lines)
- match: owner-exact `BuiltinFunctionExec.decode`        ← exec, rank 1 (symbol-exact 우선)
...
### File: fnc/mod.rs (14 lines)                            ← legacy, rank 2 (#1 co-exposure)
- match: matched literal: `encoding::base64::decode`;     ← #2 오라벨 해소
  `encoding::base64::decode` 이름은 2개 경로에 존재
  (구현 심볼 1 + dispatch 리터럴 1) — 둘 다 확인          ← #4 cross-path
- read fnc/mod.rs:1 (1 lines)                              ← #5 단축 read
```
실제 `src` 인덱스 broad query에서 detail+tail 정상, `same as ... above`(#7 cross-file dedup) 및 `ambiguous callee(s) suppressed`(#7 callee 억제) 모두 출력 확인, panic 없음.

**#3 전용 데모(중요)**: 위 두 쿼리는 모두 심볼이 query와 매칭되는 `any_match=true` 경로(변경 전후 바이트 동일)라 #3의 신규 `!any_match` 분기를 발동하지 않는다. 그래서 별도 픽스처로 직접 발동시켜 확인: `assemble_gizmo`(8줄 본문)의 **docstring**에만 "frobnicate"를 넣고(심볼 이름/owner엔 없음) MCP `search "frobnicate"` 실행 → 파일은 docstring으로 non-fallback 진입하나 이름-매칭 심볼이 없어 `any_match=false` → 본문이 **3줄 시그니처 + `… (5 more lines)`로 강등**되고 caller/callee 주석 미부착. 변경 전이면 8줄 full snippet이 떴을 것. → #3는 inspection + 기존 스위트 비회귀 + **신규 분기 실측 데모**까지 완료.

## 부수 변경 / deviation (정직 보고)
- **테스트 1건 수정(불가피)**: `callers/annotate.rs`의 `test_callee_and_caller_ambiguity_labels_for_common_name`이 억제된 `make (2 defs, target ambiguous)` 줄을 단언하고 있었음. #7(ambiguous callee 억제)이 의도적으로 그 줄을 없애므로, 단언을 "억제됨 + suppressed 카운트 노트 존재"로 갱신. 새 테스트 추가는 없음(기존 테스트의 기대치만 신규 동작에 맞춰 정정).
- **#6 점수-비율 가드는 계획 보강**: 원안(개선안 4)은 "점수 격차 작을 때만 양보"를 명시했으나 초기 구현은 디렉토리 카운트만 봤음. advisor 지적을 반영해 `DIVERSITY_MIN_SCORE_RATIO=0.6` 가드를 추가(약한 타-디렉토리 후보가 강한 동일-디렉토리 후보를 밀어내지 못하게). 보수적 동작.
- **MCP tool description 미세 stale(미수정, scope 보류)**: `tools/mod.rs`의 search description이 "compact match_reason/ambiguity/read_suggestion hints"로 개념을 서술. 힌트 prefix를 `- match:`로 바꾸고 read_suggestion을 단축형으로 바꿨으나 description은 정확한 prefix 문자열을 인용하진 않아 사실관계 오류는 아님(개념 유지). scope 확장 회피 위해 미변경 — 후속에서 정밀화 가능. **오케스트레이터 판단 필요 시 1줄 수정으로 정합 가능.**
- **#4 휴리스틱 한계 2건(허용 가능, 기록만)**: (1) cross-path 노트는 literal을 **보유한 결과(legacy 파일)**에만 붙는다 — exec 파일 쪽엔 "이 이름은 다른 곳에 dispatch literal로도 존재" 노트가 안 붙음(개선안 3의 "양 site 모두 annotate"를 부분 충족). 단 노트가 같은 search 응답에 렌더되므로 모델은 본다. (2) `qualified_name_leaf`→`defines_leaf`가 leaf 토큰(예 `decode`)으로 매칭하므로 무관한 `decode` 메서드가 있으면 "구현 심볼 N"이 과대 집계될 수 있음. 둘 다 보수적 휴리스틱 — 재벤치에서 노이즈로 보이면 leaf-매칭을 owner-qualified exact로 좁히는 후속 가능.
- **상수 값**: `QUALIFIED_LITERAL_SCORE_BOOST=1.6`(계획 1.5~1.8 범위 중앙, 3.0 미만), `DETAIL_DIR_CAP=3`(threshold 5의 과반), `DIVERSITY_MIN_SCORE_RATIO=0.6`. 모두 보수적 선택; 재벤치로 튜닝 가능.

## 검증 권장(오케스트레이터)
- 랭킹(#1): 실제 surrealdb 인덱스에서 `search "encoding::base64::decode builtin function" --limit 100` → `fnc/mod.rs`가 detail(5)/tail 상단 진입 + exec가 top5 잔존(co-exposure) 확인. 역방향 가드: `search "fnc run synchronous dispatch"`에서 exec가 top5에서 사라지지 않는지.
- 출력(#3/#5/#6/#7): claude context 재측정 + LOSS 4종 부호 + codex 비회귀.
