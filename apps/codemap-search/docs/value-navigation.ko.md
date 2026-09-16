# 값 관계 출력 제외와 탐색 출력 계약

`read`, 내용 조회 방식의 `grep`, `search`는 `Value relationships`를 출력하지 않는다. 값 관계 평가와 렌더링은 탐색 요청에서 실행하지 않으며, 이 분석에 딸린 `Analysis diagnostics`, 미해결 값 관계 집계, 부분 분석 안내도 제외한다.

`debug:true`로 값 관계 출력을 다시 켤 수 없다. 기존 `debug` 불리언 인자는 이전 호출과의 호환성을 위해 받아들이며 현재 출력에 영향을 주지 않는다. 기존 타입 검사와 내용 조회 전용 제약은 유지한다.

## 제공하는 정보

| 항목 | 동작 |
| --- | --- |
| 선언과 소속 멤버 | 함수·타입·impl 등의 선언별 구획과 멤버 계층을 표시한다. |
| 직접 호출 관계 | 호출자·호출 대상과 정의 위치, 직접 호출 해석의 미해결 표시를 제공한다. |
| 참조 상수 | 지원하는 소스 근거가 있는 상수 정의·값을 제공한다. |
| 구현 관계 | 지원하는 타입·메서드의 선언·구현 연결을 제공한다. |
| 이벤트 지도 | 기존 이벤트 탐색 설정과 조회 근거에 따라 제공한다. |
| 소스 경로 | 저장과 소비를 연결하는 조건부 `Source routes`를 제공한다. 콜백·인자 전달·데이터 소비를 구분하며, [별도 출력 계약](source-routes.ko.md)을 따른다. |
| 검색의 정적 컬렉션 관계 | 기존 `Related write/read paths`를 제공한다. |
| 원문 | 기존 줄 범위, 문맥 행, callable 확장과 원문 보존 규칙을 적용한다. |

값 관계의 내부 진단을 제외해도 실제 원문 잘림, 색인 제외·미지원 형식, 오래된 색인, 호출 대상 불확실성에 대한 안내는 유지한다. 소스에 들어 있는 `Value relationships` 같은 문자열은 원문의 일부이므로 그대로 읽고 검색할 수 있다.

## 파일별 출력

기본 상세 결과는 `# codemap-search` 아래에서 `## 1. src/user.ts`처럼 파일을 묶는다. 각 파일에는 `### fn active`, `### impl ImplementationIndex`처럼 선언 종류와 이름으로 제목을 만들고 마지막에 `### results`를 한 번만 배치한다. 메서드는 소속 impl/class 아래에 유지한다.

`grep` 본문은 파일 제목과 중복되는 경로 접두사를 생략한다. 원문에 들어 있는 경로 문자열과 다른 파일을 가리키는 관계 위치는 보존한다. `view=source`는 독립적으로 읽을 수 있는 기존 원문 형식이다. `view=definitions`는 선언, `view=relations`는 직접 호출·상수·구현·이벤트 관계와 소스 경로를 제공한다.

폴더 `overview`의 `## Files` 구조, 함수 시그니처, 타입 필드와 impl 메서드의 계층은 유지한다. 검색의 순위·원문 선택·나머지 순위 목록과 출력 상한도 유지한다.

## 확인

앱 디렉터리에서 실행한다.

```sh
cm search '{"query":"validate +arguments.rs","workspace_scope":"codemap-search"}'
cm read '{"file_path":"src/tools/search/arguments.rs","offset":1,"limit":44}'
cm grep '{"path":"src/tools/search/arguments.rs","pattern":"ok_or_else","-B":1,"-A":6}'
cm read '{"file_path":"src/tools/search/arguments.rs","offset":1,"limit":44,"debug":true}'
```

모든 조회에서 값 관계와 값 분석 안내가 없어야 한다. 선언·직접 호출·원문은 남아야 하며 `debug:true`의 결과도 기본 조회와 같아야 한다.

기존 도구 검증은 저장소 루트에서 실행한다.

```sh
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_value_relationships_stay_removed_across_live_views_and_restart -- --test-threads=1
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_live_diagnostics_distinguish_empty_excluded_and_stale_source -- --test-threads=1
```

## 색인 호환성

기존 `Value relationships` 출력 제외 자체에는 색인 재생성이 필요하지 않았다. 이후 추가한 `Source routes`는 색인 시 별도 분석기로 생성하며, 제거한 요청 시점의 값 관계 평가·렌더링 코드를 호출하지 않는다. 18개 언어의 소스 입력을 보존하기 위해 색인 형식은 `v34-native-source-routes`로 갱신했다. 이전 형식은 기존 형식 변경 절차에 따라 다시 생성하며, 재시작 후 저장된 소스로 같은 분석 세대를 구성한다.
