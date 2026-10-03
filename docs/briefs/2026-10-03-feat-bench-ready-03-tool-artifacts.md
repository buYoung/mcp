# [feat] 비교 도구 실행 파일과 의존성 고정

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/03-artifacts.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
feat

## Current State (As-Is)
- [confirmed] `benchmark/tool_artifacts.py`가 비교 대상 잠금에서 설치 위치·실행 명령·바이너리·의존성·모델 해시를 연결한다.
- [confirmed] `benchmark/resources.py`는 현재 제품의 소스 스냅샷과 release 빌드를 준비한다. 실제 소스 바이트가 같은 고정 빌드를 새 Git HEAD로 다시 표시하지 않는다.
- [confirmed] 준비 산출물은 작업 전용 캐시에 보존하며 제품의 실제 버전과 1.0.0 출시 목표를 구분한다.

## Desired Outcome (To-Be)
- Prepare reproducible local artifacts for all six selected arms with source-to-installed-artifact provenance, actual runtime versions, and no floating dependencies or release aliases in the recorded identity.
- Freeze the actual codemap-search evaluation snapshot and binary without pretending its package version is already 1.0.0.
- Hand exact launch artifacts and local model assets to indexing, without indexing Grafana or invoking any model in this child.

## Scope
### In Scope
- Introduce per-tool artifact preparation that follows each locked project's installation/build contract rather than treating all targets as codemap crates.
- Resolve and record package dependency versions, platform/architecture, checksums, optional native backends, and code model assets used for the prepared condition.
- Keep installations in task-owned directories or virtual environments, with exact executable paths and no global PATH replacement.
- Capture acquisition/build logs and duration as preparation evidence, separate from future search performance results.
### Out of Scope
- [hard] Updating source pins to newer tags, latest npm/PyPI packages, or the machine's preinstalled unverified binaries.
- [hard] Grafana indexing, solver/grader/calibration calls, benchmark execution, or modifying upstream product code/defaults.
- [hard] Credential reads, external AI calls, global agent configuration changes, or new test files/cases.
- [deferred] Release publication, changing the subject package version, container matrices, and cross-platform support beyond the current host.

## Constraints
- Consume exact archives and hashes from the existing lock. A registry/release artifact is usable only after its correspondence to the locked source is documented; otherwise build from the pinned source.
- Lock exact dependencies actually resolved and any native packages loaded. Record hashes of binaries or installed package trees, not just version banners.
- Discover compatibility through package metadata and bounded local build/import checks. Python >=3.10 does not by itself prove every dependency works on Python 3.14; a task-local compatible interpreter is a bounded worker choice.
- Preserve native response defaults. No install-time patch may enlarge output, add tools, or change product behavior to improve benchmark quality.
- No inference calls are needed to check the requested solver/grader configuration. Record exact requested model IDs and the fact that account-level inference availability has not been exercised.

## Related Files / Entry Points
- `benchmark/tool_artifacts.py`
- `benchmark/resources.py`
- `benchmark/data/grafana-comparison-targets-v1/targets.lock.json`
- `docs/briefs/evidence/bench-ready/03-artifacts.json`

## Execution Plan
### Stage 1 — Resolve each tool's reproducible preparation path
- Starts when: `docs/briefs/evidence/bench-ready/01-contract.json` supplies the six target identities and preparation policies.
- Work: Revalidate the host and locked archive identities. Map each pinned package to its install/build command, runtime, optional extras, and native backend. Inspect zvec model files and Graphify's MCP dependency extra. Define package-tree hashing and exact source association before acquisition.
- No-op when: all six artifacts already exist with matching source, dependency, runtime, platform, and content hashes and pass bounded version/import inspection.
- No-op handoff: publish those verified artifacts at `docs/briefs/evidence/bench-ready/03-artifacts.json` for child 04 without rebuilding them.
- Deliverable: a six-row acquisition/build map including the subject snapshot recipe, task-local destinations, required runtimes, and artifact hash method.
- Verify: `Compare each acquisition map entry with its pinned upstream package metadata and the route target contract`; Inputs: all six target identities and current host metadata; Expected: one supported preparation path per arm with no floating version or unresolved source association.
- Ends when:
  - [x] CodeGraph and zvec use compatible Node runtimes and native backend requirements are explicit.
  - [x] Graphify includes the MCP serving dependencies and a verified compatible Python runtime.
  - [x] codebase-memory-mcp and rg have platform-specific acquisition/build paths and actual feature/version recording.
- Handoff: Stage 2 receives the bounded six-tool build map.
- Replan when: a pinned version cannot run on the host without source changes. Stop that arm, report the exact incompatibility, and return to the parent rather than substitute another release.

### Stage 2 — Build or acquire isolated artifacts
- Starts when: Stage 1 has established supported source-bound preparation paths.
- Work: Prepare local artifacts from the locked sources. Snapshot the subject with its actual source and working-tree identity, use locked Cargo dependencies, and hash the resulting binary. Resolve external dependency graphs in isolated environments. Download and hash zvec's selected local model revision and tokenizer without executing inference. Record effective CodeGraph native-kernel availability and any documented fallback.
- Deliverable: six task-owned launch artifacts with dependency records, runtime metadata, content hashes, build logs, and separately identified local model assets.
- Verify: `Inspect version/help or import output and recompute every artifact/model hash against its preparation record`; Inputs: exact six launch paths, package environments, and zvec model assets; Expected: all identities match the pinned source map, no unknown fallback backend, and no model inference or Grafana index has run.
- Ends when:
  - [x] No launch path accidentally resolves to a global unverified executable.
  - [x] Subject version, source snapshot, Cargo lock, toolchain, and binary hash are recorded independently of the 1.0.0 target label.
  - [x] Any unresolved dependency reproducibility or native fallback is explicit and prevents a falsely complete artifact record.
- Handoff: Stage 3 receives the prepared artifacts and provenance.
- Replan when: installation changes a locked product behavior or cannot resolve a source-bound artifact. Stop indexing for that arm and correct its preparation map before continuing.

### Stage 3 — Publish the artifact handoff
- Starts when: all six artifacts have verified provenance and local launch prerequisites.
- Work: Integrate route artifact preparation with existing resources callers, remove retired campaign paths, and write the exact indexing input contract. Include start/stop ownership hints but do not launch persistent query daemons yet.
- Deliverable: `docs/briefs/evidence/bench-ready/03-artifacts.json` with `schema_version`, `status`, `contract_hash`, `host`, and six `artifacts` containing source pin, package/binary hash, dependency lock/hash, runtime, launch path, actual version, backend, model assets, and verification evidence.
- Verify: `Read all six artifact entries and resolve each recorded launch path and content hash on the host`; Inputs: `03-artifacts.json` and task-local artifacts; Expected: six verified entries and no source-only placeholder marked ready.
- Ends when:
  - [x] Each arm has enough data for child 04 to start its documented preparation command without resolving another version.
- Handoff: child 04 consumes `docs/briefs/evidence/bench-ready/03-artifacts.json`.
- Replan when: any artifact identity changes after verification. Invalidate dependent index readiness, rebuild or reverify the artifact, and republish its hashes.

## Side Effect Checkpoints
- [x] No global installation, PATH mutation, personal daemon replacement, or credential access is required.
- [x] Native output defaults and upstream source files remain unchanged.
- [x] Source association and actual dependency/backend identity are independently auditable.
- [x] Preparation durations are not presented as search latency or benchmark measurements.

## Acceptance Criteria
- [x] `03-artifacts.json` resolves six usable launch artifacts with verified hashes and exact upstream source associations.
- [x] CodeGraph 1.6.1, zvec-grep 0.2.0, Graphify 0.9.74, codebase-memory-mcp 0.11.0, and rg 15.2.0 match the frozen lock.
- [x] The subject record reports its actual version and snapshot, and all required runtime/model dependencies are recorded.
- [x] No Grafana index, solver response, grader response, or benchmark result is produced by this child.

## Open Questions
- None — the user selected local search/index preparation, so external-AI tool modes are excluded.
