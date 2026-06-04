# [feat] Add file-based logging to ~/.scout/logs/

## Work Type
feat

## Current State (As-Is)
- scout 는 진단 출력을 **stderr 로만** 내보낸다. 영속 로그 파일이 전혀 없다.
  - `apps/scout/src/index.ts:66` — 시작 시 누락 바이너리 설치 안내(`buildInstallationGuidance`)를 `process.stderr.write` 로 출력(저하 모드 부팅).
  - `apps/scout/src/providers/text-search/zoekt-webserver-lifecycle.ts:80` — zoekt-webserver 자식 프로세스의 stderr 청크를 `[scout][zoekt-webserver]` 접두로 그대로 forward.
  - `apps/scout/src/config/scout-config.ts:459` — never-exit 철학의 중앙 경고 헬퍼(`[scout] <message>\n`). 설정 파싱 실패·타입 오류·알 수 없는 키 경고가 전부 여기로 모인다.
  - `apps/scout/src/startup/git-exclude.ts:72` — `.git/info/exclude` 등록 실패 경고.
- scout 는 stdio MCP 서버다. stdout 은 JSON-RPC 전용이므로 진단은 반드시 stderr 로 가야 한다(그래서 위 사이트가 전부 stderr).
- 문제: **host(MCP 클라이언트)가 자식 프로세스 stderr 를 삼키거나 버리는 경우** 위 진단이 어디에도 남지 않는다. 깨진 설정, 누락 바이너리, zoekt-webserver 크래시 같은 부팅·런타임 실패를 사후에 추적할 수단이 없다.
- 영속 저장 위치 선례는 이미 있다: 관리형 바이너리는 `~/.scout/bin/<tag>`, 전역 설정은 `~/.scout/config.toml`(둘 다 `os.homedir()` + `SCOUT_DIRECTORY_NAME=".scout"` 기준, defaults.ts:95). 로그도 같은 home root 하위가 자연스럽다.

## Desired Outcome (To-Be)
- scout 의 모든 진단이 **stderr 와 더불어 `~/.scout/logs/` 아래 파일에도** 기록되어, host 가 stderr 를 삼켜도 사후 디버깅이 가능하다.
- 로그 파일은 **코드 레포(`<repo>/.scout/`) 가 아니라 home root(`~/.scout/logs/`)** 에 둔다 — 레포 오염·다중 레포 색인 디렉터리 혼선을 피하고 바이너리/전역 설정과 같은 전역 위치를 공유한다.
- 회전/크기 상한이 있어 로그가 디스크를 무한정 점유하지 않는다.
- 파일 로깅 실패(권한·디스크풀 등)는 **never-exit 철학을 따라 절대 프로세스를 죽이지 않고**, 최악의 경우 stderr-only 로 조용히 저하된다.

## Scope
### In Scope
- `~/.scout/logs/` 디렉터리 보장(`mkdir recursive`) 후 그 아래 로그 파일에 append 하는 파일 로거 추가.
- 기존 4개 stderr 사이트를 단일 로깅 경로로 통일: 호출부는 한 헬퍼를 호출하고 그 헬퍼가 stderr **와** 파일에 동시 기록(tee).
- 각 로그 라인에 **타임스탬프 + pid + repo 태그** 부착(아래 결정 사항 참고).
- 회전/크기 상한 정책(아래 결정 사항 참고).
- 모든 로깅 상수는 `apps/scout/src/config/defaults.ts` 에 집중(리터럴 인라인 금지). 신규 상수 후보(이름·값은 구현 시 확정): 로그 디렉터리 이름, 파일명 패턴, 파일당 바이트 상한, 보관 파일 개수.
### Out of Scope
- [hard] 로그 디렉터리를 설정 가능 키로 노출하는 것 — 경로는 SPEC §1.4 처럼 고정(`~/.scout/logs/`). env 로 경로 주입 금지(SPEC §1.3, `SCOUT_*` env 사용 금지 원칙).
- [hard] sibling 앱(`apps/mcp-server`) 수정/import — 필요한 패턴은 복사한다(§0).
- [deferred] 외부 로깅 라이브러리(pino/winston 등) 도입 — 신규 의존성은 정당화 전까지 보류. 의존성 없이 `fs.appendFile`/`createWriteStream` 으로 시작.
- [deferred] 로그 레벨(debug/info/warn/error) 체계와 레벨별 필터 — v1 은 단일 스트림으로 충분, 레벨은 후속 과제.
- [deferred] 구조화(JSON) 로그 — v1 은 사람이 읽는 텍스트 라인.

## Constraints
- **never-exit**: 로깅 경로의 어떤 fs 실패도 throw/`process.exit` 금지. catch 후 stderr-only 로 저하(SPEC §1.6 의 설정 never-exit 철학을 로깅에도 적용). 로거가 죽어서 서버가 죽으면 안 된다.
- **stdout 오염 금지**: stdout 은 JSON-RPC 전용. 진단은 stderr + 파일에만. stdout 으로의 로그 절대 금지.
- **stderr 유지**: 파일 로깅은 stderr 를 **대체하지 않고 추가**한다(tee). host 가 stderr 를 보여주는 환경에서의 기존 동작을 보존한다.
- 파일/디렉터리 작성은 `~/.scout/logs/` 소유 하위 경로로 한정. rm-safety 관점에서 home root 밖을 건드리지 않는다.
- 로그 라인은 **반드시 pid + repo 태그** 를 포함해 여러 세션이 같은 파일에 interleave 될 때 출처를 구분할 수 있어야 한다(단일 파일 결정을 택할 경우 필수).
- 신규/수정 로컬 import 는 `.js` 확장자(§0, NodeNext). 파일명 kebab-case `.ts`. 상수는 `UPPER_SNAKE_CASE` 로 `defaults.ts` 에 집중. 주석/JSDoc 한국어.
- repo 태그 소스는 `index.ts` 의 `repositoryRoot = process.cwd()` (이미 존재). 로거는 부팅 시 repo 태그를 주입받는 형태가 자연스럽다.

## Related Files / Entry Points
- `apps/scout/src/config/defaults.ts` — 로깅 상수(디렉터리 이름, 파일명 패턴, 크기 상한, 보관 개수) 추가. `SCOUT_DIRECTORY_NAME`(`.scout`)·`os.homedir()` 선례가 여기/근처에 있다.
- `apps/scout/src/config/scout-config.ts:457-459` — 중앙 경고 헬퍼(`emitWarning` 계열). 파일 로깅을 태울 가장 자연스러운 단일 합류점. tee 로직을 여기에 둘지, 별도 logger 모듈로 분리할지 구현 시 결정.
- `apps/scout/src/index.ts:66` — 설치 안내 stderr 출력 사이트(부팅).
- `apps/scout/src/providers/text-search/zoekt-webserver-lifecycle.ts:80` — zoekt-webserver 자식 stderr forward 사이트.
- `apps/scout/src/startup/git-exclude.ts:72` — exclude 등록 실패 경고 사이트.
- `apps/scout/src/startup/managed-bin-storage.ts` — `~/.scout/bin/<tag>` 경로 해석 패턴(home root 하위 디렉터리 보장의 참고 구현).
- `apps/scout/src/logging/scout-logger.ts` (제안) — 파일 로거 + tee + 회전. (정확한 위치/이름은 구현 시 확정.)

## Side Effect Checkpoints
- [ ] stdout 은 여전히 JSON-RPC 전용(로그 한 줄도 stdout 으로 새지 않음).
- [ ] stderr 출력은 기존과 동일하게 유지(파일은 추가일 뿐 대체 아님).
- [ ] `~/.scout/logs/` 생성/쓰기 실패가 서버 부팅·검색을 죽이지 않음(저하 모드 확인).
- [ ] zoekt-webserver 가 stderr 를 폭발적으로 쏟아도(루프 크래시 등) 회전/크기 상한이 디스크를 보호.
- [ ] 다중 scout 세션이 동시에 같은 로그 파일을 쓸 때 라인이 깨지지 않고 pid/repo 로 출처 구분 가능(단일 파일 결정 시).
- [ ] `pnpm --filter @buyong-mcp/scout check-types` 와 루트 `pnpm lint`(biome) 둘 다 통과.

## Acceptance Criteria
- [ ] 깨진 설정/누락 바이너리 시나리오에서 stderr 에 나오던 경고가 `~/.scout/logs/` 아래 파일에도 동일하게 기록된다.
- [ ] host 가 stderr 를 버리는 환경을 모사해도(자식 stderr 미수집), 같은 진단이 로그 파일에서 발견된다.
- [ ] 각 로그 라인이 타임스탬프 + pid + repo 태그를 포함한다.
- [ ] 로그 파일이 설정된 크기 상한을 넘으면 회전되고, 보관 파일 개수 상한이 지켜진다.
- [ ] 로그 디렉터리를 읽기 전용으로 만들어 쓰기를 실패시켜도 서버는 정상 부팅·검색하고 stderr-only 로 저하된다.

## Open Questions
- **per-repo 분리 vs 단일 파일** (핵심 결정, 미정 — 구현 전 확정 필요):
  - 단일 파일(`~/.scout/logs/scout.log`): 단순하고 한 곳만 보면 됨. 단점은 여러 레포에서 동시에 띄운 세션이 interleave → **라인마다 pid + repo 태그 필수**.
  - per-repo 분리(예: repo 경로 SHA 또는 basename 키로 `~/.scout/logs/<key>.log`): interleave 없음·레포별 추적 쉬움. 단점은 파일 난립과 어떤 파일이 어느 레포인지 매핑 부담. (참고: SPEC §1.4 는 index 의 repo-path SHA 해시 디렉터리를 **폐기**했다 — 같은 해시 디렉터리 패턴을 로그에 다시 들이는 것은 그 결정과 상충하므로 신중히.)
  - 권장 출발점(미확정): **단일 파일 + pid·repo 태그** 로 시작하고, 운영상 필요하면 per-repo 로 확장. 최종 결정은 사용자/구현자가 내린다.
- **회전 메커니즘**: 자체 크기-기반 회전(append 전 stat → 상한 초과 시 rename + N개 보관)으로 충분한지, 아니면 시간 기반(일자별 파일)이 나은지. 자체 구현 vs 경량 라이브러리. 크기 상한·보관 개수의 구체 값.
- **동시 쓰기 안전성**: 다중 프로세스가 같은 파일에 append 할 때 `fs.appendFile`(원자적 작은 write)로 충분한지, lock 이 필요한지. 단일 파일 결정과 직접 연결되는 질문.
- **zoekt-webserver 자식 stderr 의 처리 입자**: 청크 단위 forward(현행)를 라인 단위로 정규화해 태깅할지, 원시 청크 그대로 파일에 넣을지.
