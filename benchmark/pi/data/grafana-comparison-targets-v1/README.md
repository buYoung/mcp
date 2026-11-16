# Grafana 벤치마크 비교 대상 소스 고정

2026-11-15 **개정 10**이다. Serena 2.1.0(`oraios/serena`, `5e4f5cd173c7ff3eed37ee3f1de6c266b937c5dc`)을 비교 대상에서 제외했다(사용자 결정). 사유: 색인이 너무 오래 걸린다. Grafana 전체의 Go·TypeScript LSP 색인이 1회에 약 34~70분 걸렸다. 남은 외부 비교군 5개의 버전·소스와 toolchain 고정은 개정 9와 같다. `previous_lock_sha256`은 Git 이력에 보존된 개정 9를 가리키고, 제외 기록은 lock의 `excluded_comparisons`에 있다.

2026-10-28 개정 9(이전 개정)다. 외부 비교군 6개의 버전·소스는 개정 8과 같다. 풀이를 pi direct/codemode(모델당 84슬롯)로 되돌리고, 평가 제품을 codemap-search 0.14.2로 고정한다. `previous_lock_sha256`은 Git 이력에 보존된 개정 8을 가리킨다. 새 `@buyong/pi-mcp-adapter`와 child-env API를 넣은 `@buyong/pi-claude-bridge`는 `pi_lock pin`으로 npm 버전·integrity를 고정하며 prepare는 이전 설치물이나 scratch tarball로 우회할 수 없다. 제품 설정은 기본값이다.

| 비교 대상 | 공식 저장소 | 고정 버전 | 고정 커밋 |
| --- | --- | --- | --- |
| CodeGraph | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) | [1.6.2](https://github.com/colbymchenry/codegraph/releases/tag/v1.6.2) | `6560052a6f856855d3f71eee838fd66ccfa4285d` |
| zvec-grep | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) | [0.2.2 tag](https://github.com/zvec-ai/zvec-grep/tree/v0.2.2) · npm stable | `b1a9148e26a7bc9bd4a52229ffb7532d3063793d` |
| Graphify | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) | [0.9.95](https://github.com/Graphify-Labs/graphify/releases/tag/v0.9.95) | `92741d162b2094ffc2fed58cfbd8ee823e9da62c` |
| codebase-memory-mcp | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) | [0.12.1](https://github.com/DeusData/codebase-memory-mcp/releases/tag/v0.12.1) | `a7158ac2f48ca646b4527341b76f930f2aeff87e` |
| plain rg | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | [15.2.0](https://github.com/BurntSushi/ripgrep/releases/tag/15.2.0) · 유지 | `e89fff89ac9af12e8d4ce9d5fd07beb408ca730f` |

- CodeGraph는 사용자가 선택한 공식 `@colbymchenry/codegraph` Node 구현이다. zvec-grep은 `@zvec/zvec-grep` Node 구현이다. zvec 0.2.2에는 GitHub Release가 없으므로 release URL/ID/시각은 `null`이다. GitHub latest의 오래된 0.2.0 메타데이터를 빌려 쓰지 않는다.
- Graphify는 공식 `Graphify-Labs/graphify`의 `graphifyy` Python 구현이다. `v1.0.0`은 2026-04-05의 과거 release이며 PyPI에는 없다. 현재 upstream latest와 PyPI가 일치하는 0.9.95를 선택했다.
- 평가 제품 codemap-search는 작업 트리가 아니라 `codemap-v0.14.2` (`9b29f397edb7a43f8f2ff0982492a09f7f50d4e5`)의 `git_ref` 스냅샷을 빌드한다. 실제 버전은 **0.14.2**이며 출시 목표 1.0.0으로 바꿔 표시하지 않는다.

## 데이터셋 연결

총 7개 비교군은 같은 [Grafana 2문항 데이터셋](../grafana-routes-v4/README.md)을 사용한다. manifest SHA-256은 `b292a2a3979c063d65d6df2c4ba86fef420604f0f617104d6d80399257fef8bf`이며 이번 개정은 데이터셋 README와 그 결합 hash만 갱신했으며 질문·정답·Grafana 커밋은 그대로다. rg는 검색 제품 MCP 없이 pi bash를 사용하는 기준군의 실행 파일 핀이며, 제품 비교군 6개에도 두 모드 모두 같은 bash를 보조로 제공한다. 제품군은 MCP 도구를 최대한 사용하고 그 도구로 할 수 없는 작업에만 bash를 사용한다. direct는 제품·resource 도구와 bash를, codemode는 같은 도구들을 스크립트 안에서 제공한다.

## 설치 조건과 고정 파일

| 파일 | 역할 |
| --- | --- |
| [targets.lock.json](targets.lock.json) | 저장소·태그·전체 커밋·Git tree, source archive URL·SHA-256·크기, source file blob, 데이터셋/평가 소스 연결. |
| [provenance.json](provenance.json) | Phase A의 공식 API·태그·커밋·root blob·archive 검증 기록과 Graphify wheel 설치 조건. |
| [graphifyy-0.9.95-py312-darwin-arm64.requirements.txt](graphifyy-0.9.95-py312-darwin-arm64.requirements.txt) | Python 3.12/macOS arm64에서 한 번 해석한 Graphify wheel 및 59개 runtime package의 hash lock. |
| [pi-package.json](pi-package.json) / [pi-package-lock.json](pi-package-lock.json) | pi 1.4.0, SDK 0.3.311 (Claude Code 2.1.311)의 기반 npm integrity lock. 두 미게시 패키지는 targets.lock.json의 publications에서 대기하며 게시 후 전체 의존성을 다시 고정한다. |
| [SHA256SUMS](SHA256SUMS) | 이 bundle의 모든 고정 파일 SHA-256. |

Graphify upstream `uv.lock`은 루트 버전 0.9.84와 누락된 필수 PyYAML 때문에 0.9.95의 `uv sync --locked`가 실패한다. `--frozen`으로 오류를 숨기지 않고 공식 wheel을 위 requirements로 `uv pip sync --require-hashes --only-binary :all:` 설치한다. 준비 시 의존성을 다시 해석하지 않는다. wheel package 파일을 고정 tag source와 대조하고 Graphify는 `uv pip check`를 수행한다. 원본 `uv.lock`은 source provenance로 보존하며 실제 runtime lock과 구분한다.

CBM은 공식 darwin-arm64 asset 0.12.1을 사용한다.

## 적용 규칙과 검증 범위

준비·실행 때 `latest`나 기본 브랜치를 다시 따라가지 않는다. 고정 archive SHA-256과 source pin을 검증하고 빌드 전후 원문을 대조한다. 태그·버전 문자열만 같고 commit/hash가 다르면 다른 조건이다. 설치 실패 시 다른 버전으로 대체하거나 필수 비교군을 제외하지 않는다. 구버전 index/baseline/ready 기록은 재인증하지 않고 새로 준비한다.

Phase A에서 공식 저장소 6개, GitHub Release 5개와 npm/tag-only 1개, source archive 6개 및 root source file 16개의 Git blob/바이트를 확인했다. `response_sha256`은 당시 API 원문의 식별값이며 응답 전체나 archive는 이 bundle에 포함하지 않는다. 전체 Git tree 재구성·binary 재현 빌드·전체 Grafana readiness를 인증하는 기록이 아니다.

```sh
cd benchmark/data/grafana-comparison-targets-v1
shasum -a 256 -c SHA256SUMS
```

하네스는 bundle과 manifest를 검증하고 설치·도구 목록·색인·중립 조회의 해시를 별도 준비 기록에 결합한다. 이 잠금 자체는 실행 승인이 아니다. 현재 절차는 [벤치마크 안내](../../README.md)와 [실행 계약](../grafana-execution-v1.json)을 따른다.

pi toolchain은 macOS arm64에서 `settings.CACHE/pi-toolchain`에 `npm ci --ignore-scripts --no-audit --no-fund`로 설치한다. tools lock에 npm lock·manifest·전체 node_modules tree SHA-256을 기록하며 재사용 때 전체 파일/모드/symlink와 host Node hash를 검사한다. drift나 미완료 설치는 자동 재설치로 숨기지 않고 중단한다. operator pi 설치와 auth 파일은 설치 검증 대상이 아니다.

게시 후 값 채우기는 `python3 -m benchmark.pi_lock pin` 한 명령으로 한다. 정확한 버전·integrity 인자와 해시 갱신·재준비 순서는 [README](../../README.md#npm-게시-후-고정)를 따른다. 하위/optional 의존성도 npm lock의 registry URL·정확한 버전·sha512 integrity가 있어야 하고, Agent SDK는 override로 고정한다. `file:`·Git·link 패키지는 운영 lock에서 거부한다.
