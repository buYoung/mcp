# 벤치마크 재료 (기계 입력 — 서술 금지)

이 디렉터리는 2026-06 codemap-search 에이전트 캠페인의 **재현 입력물**이다 — 과제 데이터셋(ground truth 포함), MCP 설정, 셸 하니스. 벤치마크를 돌릴 때 `/tmp/benchmark-data/`로 staged되어 소비되는 기계 입력이며, **문서가 아니다.**

## 읽는 규칙 (반드시 지킬 것)

- **여기서 어떤 주장도 도출하지 말 것.** 수치·인과·결론의 단일 출처는 오직 `../docs/benchmark-workflow.md`(측정 기준·수치)와 `../docs/benchmark-evolution.md`(인과 서사)다. 이 두 문서가 말하지 않는 것은 "측정되지 않은 것"이지, 이 폴더의 파일로 보충해 추론할 대상이 아니다.
- 이 파일들은 **특정 머신·특정 스냅샷에 결속된 작업 사본**이다. `harness/config.sh`가 `/Users/...`·`/tmp/...` 절대경로를 박아두므로 다른 환경에서 그대로 돌지 않는다. 재현의 정본 명령은 workflow.md §7이고, 이 스크립트는 "당시 무엇을 어떻게 돌렸는가"의 기록으로만 유효하다.
- ground truth 줄 번호는 **캠페인 당시 코퍼스 스냅샷에만** 유효하다(코퍼스 SHA는 workflow.md §8-1). 다른 시점의 저장소에 대입하지 말 것.

## 구성

- `bench-2026-06/` — 캠페인 1·2 데이터셋: `tasks-surrealdb.md`(캠페인 1 — 셸 하니스 이전이라 JSON이 아닌 산문 기록), `tasks-ollama.json`·`tasks-clickhouse.json`(캠페인 2, C/C++), `mcp-codemap.json`.
- `bench-2026-06-django-strapi/` — 캠페인 3·4·5 데이터셋: `tasks-django.json`·`tasks-strapi.json`(hold-out 20과제, 함정 답·rubric 포함), `mcp-codemap.json`, `harness/`(실행·채점·집계 셸 8종 — `run-one-episode.sh`·`run-matrix.sh`·`extract-metrics.sh`·`write-scores.sh`·`aggregate-results.sh`·`build-scoring-batches.sh`·`ab-probe.sh`·`config.sh`).
