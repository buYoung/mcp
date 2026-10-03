# Grafana 벤치마크 비교 대상 소스 고정

codemap-search 1.0.0과 비교할 외부 도구 **5개 모두의 출처와 소스 버전**을 2026-10-03에 고정했다. 식별자는 `grafana-comparison-targets-v1`, 개정 번호는 `1`, 상태는 `source_identities_frozen`이다. 정확한 값은 [targets.lock.json](targets.lock.json)을 기준으로 한다.

| 비교 대상 | 공식 저장소 | 정식 릴리스 | 고정 커밋 |
| --- | --- | --- | --- |
| CodeGraph | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) | [1.6.1](https://github.com/colbymchenry/codegraph/releases/tag/v1.6.1) | `f4ddf508516332419ea3c95702810765936cf679` |
| zvec-grep | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) | [0.2.0](https://github.com/zvec-ai/zvec-grep/releases/tag/v0.2.0) | `38fe0f901252b3b7f61a7fe1d1858c03d63f7ba8` |
| Graphify | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) | [0.9.74](https://github.com/Graphify-Labs/graphify/releases/tag/v0.9.74) | `e10df08877f8819a625a1afa38c3297a31fda296` |
| codebase-memory-mcp | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) | [0.11.0](https://github.com/DeusData/codebase-memory-mcp/releases/tag/v0.11.0) | `8972ea69c6ad94b1ef1d4ffbf0a92d78d2db1798` |
| plain rg | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | [15.2.0](https://github.com/BurntSushi/ripgrep/releases/tag/15.2.0) | `e89fff89ac9af12e8d4ce9d5fd07beb408ca730f` |

CodeGraph는 사용자가 `colbymchenry/codegraph`를 선택했다. Graphify는 기존 `safishamsi/graphify` 주소가 이동한 공식 저장소를 사용한다. CodeGraph와 zvec-grep은 각각 `@colbymchenry/codegraph`, `@zvec/zvec-grep`의 루트 Node.js 구현을, Graphify는 `graphifyy` Python 구현을 기준으로 한다. 패키지 이름과 버전은 고정 커밋의 메타데이터와 대조했다.

## 데이터셋 연결

5개 도구와 평가 제품 codemap-search를 포함한 총 6개 비교군은 같은 [Grafana 3문항 데이터셋](../grafana-routes-v2/README.md)을 사용한다. 이 명세는 해당 데이터셋의 manifest SHA-256 `8fb24b5b8cbc8a3530c61991dda2e7c3e80d859a96354adeb76f28ec9cc51cf4`에 연결된다. 현재 데이터셋의 질문·근거 내용은 변경하지 않았으며 manifest의 문서 메타데이터 해시만 갱신했다.

plain rg는 검색 제품을 추가하지 않은 기본 코딩 에이전트 기준군이다. 이 명세의 rg 핀은 사용 가능한 실행 파일을 고정한다. 전용 rg MCP를 제공하지 않으며 명령 선택은 LLM에 맡긴다. 현재 도구 구성과 보조 도구 범위는 [실행 계약](../grafana-execution-v1.json)을 따른다.

## 고정 파일과 적용 규칙

| 파일 | 역할 |
| --- | --- |
| [targets.lock.json](targets.lock.json) | 필수 비교군, 저장소 ID·URL, 구현, 릴리스 태그, 전체 커밋·Git tree, 소스 아카이브 URL·SHA-256·크기, 데이터셋 연결과 실행 전 미확정 항목. |
| [provenance.json](provenance.json) | 공식 API에서 확인한 릴리스→태그→커밋→Git tree 연결과 소스 파일·아카이브 검증 기록. |
| [SHA256SUMS](SHA256SUMS) | 이 문서와 두 JSON 파일의 SHA-256. |

버전은 조회 시점의 각 공식 GitHub 저장소가 가리키는 최신 정식 릴리스를 선택했다. 준비·실행 때 `latest`나 기본 브랜치를 다시 따라가지 않는다. Git 체크아웃은 전체 커밋과 Git tree가 일치해야 하며, 소스 아카이브는 기록된 SHA-256과 크기를 대조해야 한다. 태그나 버전 문자열이 같아도 커밋·해시가 다르면 같은 대상으로 인정하지 않는다.

불일치나 설치 실패가 발생하면 원인을 확인한다. 다른 포크·버전으로 자동 교체하거나 필수 비교군을 제외하지 않는다. 프로젝트·구현·버전·커밋·아카이브를 바꿀 때는 이 고정본을 보존하고 변경 이유와 새 개정을 남긴다. 서로 다른 개정의 결과를 같은 조건의 측정으로 합산하지 않는다.

## 검증 범위

공식 저장소 5개에서 정식 릴리스와 태그를 확인하고 전체 커밋으로 해석했다. 고정 커밋의 소스 아카이브 5개를 내려받아 전체 바이트 SHA-256을 계산했다. 패키지 메타데이터·의존성 잠금 파일·README 등 루트 파일 총 13개는 Git blob 해시를 대조하고 아카이브 안의 내용과도 일치함을 확인했다. 불일치는 0개다.

저장소 루트에서 다음 명령으로 이 명세의 파일 무결성을 확인한다.

```sh
cd benchmark/data/grafana-comparison-targets-v1
shasum -a 256 -c SHA256SUMS
```

패키지 내부 해시 검증과 외부 소스 재검증은 구분한다. 후자는 고정 URL의 아카이브 또는 고정 커밋을 다시 확보해야 한다. 소스 아카이브와 API 전체 응답은 이 디렉터리에 포함하지 않으며, `provenance.json`은 식별에 필요한 응답 필드와 검증 기록을 보존한다. 기록의 `response_sha256`은 당시 원본 API 응답의 식별값이다. 이 절은 소스 명세를 동결할 당시의 검증 범위다. 이후의 설치·빌드·색인 검증은 현재 준비 기록에 별도로 보존한다.

## 현재 실행 준비 연결

이 잠금은 비교 도구의 소스 식별을 고정한다. 현재 하네스는 준비 단계에서 이 파일과 데이터셋 manifest를 읽고 전체 필수 비교군을 검증한다. 설치 바이너리·의존성·로컬 모델 가중치·도구 목록·색인·복원용 상태는 [최종 준비 기록](../../../docs/briefs/evidence/bench-ready/07-readiness.json)에 연결한다.

실행 조건과 준비 절차는 [실행 계약](../grafana-execution-v1.json)과 [벤치마크 안내](../../README.md)를 따른다. `targets.lock.json`의 준비 전 상태는 소스 동결 당시의 기록이다. 이 문서 갱신은 도구 버전·소스 핀을 바꾸지 않는다. 실제 풀이·채점·보정은 아직 실행하지 않았다.
