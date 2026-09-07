# codemap-search 제품 패치 보관

기존 하니스로 수행한 이번 작업의 측정·평가 결과는 사용자 요청에 따라 폐기했다. 점수 JSON, 답변·도구·토큰 기록, 결과 보고서와 분석, 채점 계약, 실행기, 원자료 아카이브와 확인된 임시 실험 사본을 모두 삭제했다. 기존 결과를 새 검증에 재사용하지 않는다.

새 하네스와 실행 안내는 [V2 안내](../../README.md)에 둔다. 이전 측정의 성공·실패 판정을 제품 품질의 검증 근거로 사용하지 않는다. 아래 패치는 미채택 상태의 제품 변경 보관본이다.

제품 작업 트리는 0.7.0 기준 커밋 `146d779a7d186327e765c2837637ad0543f802d1`으로 원복했다. 설치된 MCP 교체·릴리스·커밋·푸시는 하지 않았다.

| 패치 | 포함한 제품 변경 |
| --- | --- |
| [0.7.0→C](candidate-patches/0.7.0-to-C.patch) | grep 페이지·문맥·출력 한도, 검색 범위 필터, overview 상한, 읽기 안내와 관련 설정·문서·테스트 |
| [C→A](candidate-patches/C-to-A.patch) | 근거 완결성 안내와 검색 심볼의 소유 타입 표시 |
| [C→AB](candidate-patches/C-to-AB.patch) | A와 named import 관측 후보 안내 |
| [C→후속](candidate-patches/followup/C-to-followup.patch) | 안내와 후보 선별·예산 배분 보완의 전체 변경 |
| [AB→후속](candidate-patches/followup/AB-to-followup.patch) | AB 이후 후보 선별·예산 배분·역할 구분 안내의 추가 변경 |

패치는 각각 표기한 기준에 적용하는 별도 변경이며 모두 순서대로 적용하는 묶음이 아니다. 실험 하니스·측정 결과·IDE 변경은 제품 패치에 포함하지 않았다. [패치 명세](candidate-patches/manifest.json)는 기준과 파일 크기·SHA256만 기록한다.
