# 구조화된 검색 필터 검증

2026-09-23, 필터 단위 테스트 7개와 기존 search e2e 24개가 통과했다. 실제 source를 읽고 선택하는 기존 경로를 유지하고, 선택 결과의 선언·본문 구간을 평가기와 최종 조립기에 전달한다.

## 준비·평가·렌더링 계약

`search::run_with_metadata`의 `SearchOutput`은 기본 `text`와 `source_files` 외에 요청 소유 `prepared: Option<PreparedEvidence>`를 제공한다. `FileOutput::start_symbol`/`push_body`가 선언 식별자와 출력 구간을 기록한다. `PreparedEvidence::capture`는 렌더러가 기록한 구간과 관계 삽입 위치로 출력 조각을 구성한다. Markdown을 분석해 선언이나 fence를 재발견하지 않으며 추가 파일을 읽지 않는다. 이벤트 전용·준비 중·정지·오류 경로는 평가 입력을 제공하지 않는다.

`search::jev::evaluate(&PreparedEvidence, task_query, search_arguments, threshold, &dyn Evaluator, EvaluationOptions)`는 완전한 본문별 Noul 질문을 보낸다. task_query와 검색 인자를 별도 공유 상태에 담고, 질문에는 파일/kind/name/owner/range, 이미 마스킹된 표시 본문과 실제 표시된 호출 문맥만 담는다. 원문을 지시문으로 취급하지 않는다.

완전성은 선택 창이 전체 선언을 포함하고, 바이트·줄 제한에 잘리지 않았고, 원본 digest가 인덱스 증거와 일치하며, 본문이 12,000바이트 이하일 때만 인정한다. partial/missing/oversized와 알 수 없거나 구조적인 선언은 유지한다. 숨겨진 중첩 선언뿐 아니라 같은 물리적 줄에 겹쳐 표시된 다른 선언도 보호한다. source digest는 파일당 한 번 확인하며 선언 식별에는 owner도 포함한다.

질문은 “이 표시된 선언 본문이 task_query의 동작과 무관한가”이다. true는 무관함, false는 직접/간접 구현, 설정·계약, 순서, 실패 처리, 질문 전제를 반박하는 근거를 포함한다. 단어 불일치만으로 무관하다고 판단하지 않는다.

`retain(prepared, raw_answers, threshold)`는 순수 호스트 정책이다. 유한한 `0.5 < threshold <= 1.0`만 허용하며 기본값은 잠정 0.70이다. `Noul >= threshold`인 완전하고 보호되지 않은 본문만 생략한다. 표시된 본문·문맥의 식별자 일치와 선언 포함 관계로 보수적인 연결을 만들고, 유지된 근거의 연결 폐쇄를 보존한다. 동명 후보는 모호한 연결로 모두 보존하며 해결된 호출이라고 주장하지 않는다.

`FilterResult`에 raw 평가, keep mask, 선언별 이유, 실제 threshold, metrics, `search-unrelated-v1`/`search-filter-experimental-v1`을 보존한다. threshold만 변경할 때 재추론하지 않는다. 모든 질문의 반환 ID와 Noul 범위가 맞아야 하며 실패하면 기본 결과 전체를 유지한다.

`render(&SearchOutput, &Retention, byte_cap)`는 선언 이름·범위·파일 헤더·꼬리·관계·안내를 유지하고, 선택된 본문 조각만 생략 표식으로 바꾼다. 모두 유지하면 기본 문자열과 관측값이 동일하다. 필터링한 경우 최종 전달한 source/literal 구간만 파일별 집계하며 경로만 보이는 파일은 읽은 것으로 기록하지 않는다. 바이트 제한을 넘으면 호출자가 기본 결과로 복구한다.

## 실제 검증

저장소 루트에서 모두 종료 코드 0.

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::search`: 7개 통과. 0.00/0.50/0.69/0.70/0.71/1.00 경계, 0.80의 0.70/0.90 재판정, 오류·누락·비유한 값, 부분/누락/과대 본문, Rust impl/struct/constant, TypeScript 중첩 메서드, 세 함수 연결, 모호한 동명 선언, 파일별 source와 fence 보존, post-filter 관측값.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::search`: 기존 24개 통과.

정책 fixture가 준 완전성 상태는 단위 검증용이다. 실제 source digest와 MCP 활성화의 조합은 04 통합 및 05의 실제 parser/index/MCP 사례에서 확인했다. 05에서 같은 줄의 두 함수 재현이 실패해 03의 다른 선언 보존 규칙을 보완했다. 수정 후 검색 단위 7개와 Jev e2e 5개가 통과했고, 실제 Rust/TypeScript 보호 사례를 더한 최종 Jev e2e 6개와 전체 e2e도 통과했다. Noul 품질 보정과 유료 호출은 미실행이며 Python Choice 측정과 동등하다고 주장하지 않는다.
