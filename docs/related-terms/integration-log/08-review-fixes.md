# 08 — 준비본 원본 일치와 표시 상한 오류 수정

심층 검토에서 찾은 두 경계조건을 테스트로 재현한 뒤 수정했다. 같은 ID의 준비본이 교체돼도 조회와 근거 사전이 다른 원본을 읽지 않게 했으며, 표시 상한으로 숨긴 단어를 이미 표시했다고 안내하는 오류를 고쳤다. 새 회귀 테스트 6건이 수정 전 실패했고 수정 후 통과했다.

2026-10-18(UTC)에 `8c1e8c08e08e55ad27c945e06bfcbb2664f5fa91` 이후 작업 트리에서 실행했다. 사용자는 두 오류의 개선을 요청하고 회귀 테스트 추가를 별도로 승인했다. 메인 에이전트가 구현과 검증을 맡았으며 새 모델 세션은 실행하지 않았다.

## 준비 핸들과 조회 연결의 원본을 맞췄다

수정 전에는 `PreparedDataset`이 원본 지문을 보관하지 않았다. 서로 다른 로컬 파일이 같은 `dataset_id`와 전역 준비 폴더를 공유하면, 이전 핸들의 지연 조회가 교체된 파일을 열었다. 이미 열린 조회 연결은 이전 파일을 유지하지만 근거 사전은 경로를 다시 열어 다른 판을 읽기도 했다.

테스트에서는 다음 순서를 고정해 재현했다. 프로세스를 동시에 실행하거나 임의의 대기 시간에 의존하지 않고, 경합에서 가능한 파일 교체 순서를 직접 만들었다.

1. `alphaword`가 들어 있는 원본을 준비하고 조회 핸들을 만들었다.
2. 같은 ID를 쓰는 `betaword` 원본으로 공용 준비본을 교체했다.
3. 아직 열지 않은 이전 핸들이 `betaword`를 반환하는 것을 확인했다. 같은 경로의 원본을 갱신한 경우에도 재현했다.
4. 이미 열린 조회는 `alphaword`를 반환하는데 근거 사전은 다른 판을 읽는 불일치도 확인했다. 열린 파일을 유지한 채 경로를 교체하는 이 테스트는 Unix 조건으로 실행한다.

수정한 `prepare.rs`는 기존 마커의 원본 지문인 경로·크기·수정 시각을 핸들에도 보관한다. 파일을 처음 열 때 준비 잠금 안에서 ID·인덱스 버전과 원본 지문을 확인하며, 원본이 달라졌으면 오류를 반환한다. 준비 실패는 기존 런타임의 다음 호출 재시도 경로로 전달한다.

`lookup.rs`는 지연 초기화한 엔진의 연결을 내부 소비자에게 빌려준다. `evidence.rs`와 `runtime.rs`는 그 연결로 근거 사전을 읽으므로, 경로가 교체돼도 조회와 근거가 같은 열린 파일을 사용한다. 공용 캐시 경로, `ready.json` 형식, `INDEX_VERSION`과 사용자 원본은 바꾸지 않았다. 별도의 세대 번호나 콘텐츠 해시 방식은 추가하지 않았다.

## 실제 표시와 관계 우선순위를 분리했다

수정 전에는 중복 제거 집합에 모든 후보를 먼저 넣고 출력 단계에서 상한을 적용했다. 먼저 나온 그룹에서 잘린 공유 단어도 뒤 그룹에서 제거돼, 실제 어디에도 없는데 `Related words are already shown in another group.`으로 안내했다. 실제 MCP 응답에서도 `direct1x24`로 이 현상을 재현했다.

`presentation.rs`는 같은 관계 단계에서 실제 표시하는 단어만 중복 제거 대상으로 등록한다. 앞 그룹의 상한 밖 단어는 뒤 그룹에 여유가 있으면 표시한다. 합쳐진 그룹 안의 중복은 별도로 제거하고, 전체 후보 수는 생략 안내에 남긴다.

관계 우선순위는 `direct → unclassified direct → near → same-topic`으로 유지했다. 강한 관계에서 상한 때문에 생략된 단어를 더 약한 관계로 다시 표시하지 않는다. 이런 단어만 남은 근거 있는 그룹에는 이미 표시됐다는 문구 대신 상한으로 생략됐다고 안내한다. 그룹 6개와 그룹별 20/10/10개 상한은 그대로다.

## 검증 결과

작업 디렉터리는 `/Users/buyong/workspace/private/buyong-mcp`이며 Rust/Cargo는 1.98.1이다.

| 검증 | 결과 |
| --- | --- |
| 수정 전 새 단위 테스트 | 의도한 원인으로 5개 실패 |
| 수정 전 새 MCP e2e | 숨긴 공유 단어를 이미 표시됐다고 안내해 1개 실패 |
| 수정 후 일반 빌드 단위 테스트 | 5개 통과 |
| 수정 후 일반 빌드 관련어 e2e | 기존 20개와 새 1개, 총 21개 통과 |
| `related-terms-eval` 단위 테스트·관련어 e2e | 각각 5개·21개 통과 |
| `cargo check`, 기존 Clippy 검사 | 통과 |
| 마지막 일반 바이너리 빌드 | 통과 |

실행한 Rust 검증 명령은 다음과 같다.

```sh
cargo fmt --manifest-path apps/codemap-search/Cargo.toml
cargo check --manifest-path apps/codemap-search/Cargo.toml
cargo test --manifest-path apps/codemap-search/Cargo.toml --lib related_terms
cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms
cargo test --manifest-path apps/codemap-search/Cargo.toml --features related-terms-eval --lib related_terms
cargo test --manifest-path apps/codemap-search/Cargo.toml --features related-terms-eval --test e2e_tests related_terms
cargo clippy --manifest-path apps/codemap-search/Cargo.toml --all-targets -- -D warnings
cargo build --manifest-path apps/codemap-search/Cargo.toml --bin codemap-search
```

모델 없이 기존 `mcp_tool_eval.py`로 224개 입력을 다시 조회했다. 상한 전 기대 분야 적중은 기존 비제한 결과의 **193/224**, 새 MCP 응답의 상한 후 적중은 **191/224**다. 기존 상한 손실인 `base`, `editor` 외에 새 손실은 없었다. 출력 합계는 384,906 bytes, 최대는 3,833 bytes다. 앞선 표시 개선 기록의 384,002 bytes보다 904 bytes 늘었으며 응답 크기를 줄였다는 의미는 아니다.

```sh
python3 apps/codemap-search/experiments/related-terms-poc/sense-eval/mcp_tool_eval.py \
  --binary apps/codemap-search/target/debug/codemap-search \
  --dataset apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3 \
  --uncapped-jsonl /Users/buyong/tmp/rterms-01-5_i16l71/eval240.jsonl \
  --output-dir /Users/buyong/tmp/rterms-review-fixes-K9rLmK/cap-eval
```

실제 MCP 도구 설명의 SHA-256은 `0dbf5d229a9668dc319865f1a2f9164cf6285ecf849a3cc38cb6556007c66729`로 앞선 입력 선별 관측과 같다. 입력은 여전히 `terms`만 받는 1–16개 문자열이며 새 길이 제한은 없다. 프롬프트, 입력 정규화·분리, 무제한 조회 결과의 구성, 제품의 뜻 제거 비활성 정책은 유지했다.

## 보존한 근거와 남은 범위

원시 로그는 `/Users/buyong/tmp/rterms-review-fixes-K9rLmK/`에 보관했다. `red-unit.log`, `red-mcp.log`는 수정 전 실패를 보존하며, `green-unit.log`, `green-e2e.log`, `eval-unit.log`, `eval-e2e.log`는 수정 후 결과다. `cap-eval/report.json`과 `cap-eval/responses.jsonl`에는 실제 MCP 조회 결과를 기록했다.

마지막 일반 바이너리의 SHA-256은 `3f781f851a47b82480d013c85349d48bfd1af22b2cd1082d906da3dfa056e2a6`이다. 원본 rts5 데이터셋의 SHA-256은 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`로 유지됐다. 선행 사용자 변경과 보호한 PoC 파일도 변경 전 지문과 일치했다.

이번에는 관련어 검증만 실행했다. 전체 e2e·전체 단위 테스트, 12개 저장소 Docker 평가, 새 Luna 관측, 실제 브라우저 검증, Windows 실행은 다시 수행하거나 새로 수행하지 않았다. 과거 리플레이와 모델 관측 기록은 덮어쓰지 않았다. 릴리스 게시·실사용 MCP 재시작·커밋·푸시는 하지 않았으며, 실행 중인 MCP에 적용하려면 새 일반 바이너리로 다시 시작해야 한다.
