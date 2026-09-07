# 이전 벤치마크 폐기 기록

2026-09-07, 승인된 V2 교체 계획에 따라 기존 `harness/`, `analysis-tools/`, `benchmark/` 질문·정답과 `corpus/directus/`, `RUNBOOK.md`를 제거했다. 이전 OpenCode/Directus 실행·채점·집계 코드를 V2에 재사용하지 않는다.

폐기한 Directus 스냅샷은 `9f2f73aee7d8647d3f187dac43f724fe617763f5`(트리 `0beb7bd5187e9131aba4a582effb3630d378eb4c`)였다. 더 이전 ClickHouse·Deno·Angular 결과 역시 현재 비교의 근거가 아니다. 추적된 폐기 파일의 원문은 Git 이력에 남아 있다.

`experiments/codex-codemap/candidate-patches/`의 제품 패치와 `manifest.json`은 보존했다. 제품 작업 트리, 설치된 MCP, IDE 변경은 V2 교체 범위에 포함하지 않았다. 과거 실험 결과를 새 측정 결과로 제시하지 않는다.
