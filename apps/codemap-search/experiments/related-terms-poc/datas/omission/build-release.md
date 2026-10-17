# Build, packaging and release 누락 단어

- 분야 id: `build-release`
- 원본: [../build-release.md](../build-release.md)
- 누락 단어 17개 · 사전에 없음 8 · 뜻 미배정 1 · 다른 분야에만 있음 8
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Build project configuration (`t39.6`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `build.rs` | identifier | 다른 분야에만 있음: languages-compilers — Rust build scripts | Cargo 빌드 스크립트 파일 이름으로 범위 설명에도 나오는 핵심 식별자입니다. |
| `jsr.json` | identifier | 사전에 없음 | JSR 패키지 메타데이터 파일 이름입니다. |

### Build task runners (`t638`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `lifecycle script` | phrase | 사전에 없음 | npm 설치와 배포 단계에 연결된 스크립트 하나를 가리킵니다. |
| `postpack` | identifier | 사전에 없음 | npm pack 후에 실행되는 라이프사이클 스크립트 이름입니다. |

### CI build pipelines (`t28.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Buildkite` | proper_noun | 다른 분야에만 있음: language-constructs — Buildkite pipelines | 자체 에이전트로 파이프라인을 실행하는 CI 서비스입니다. |
| `TeamCity` | proper_noun | 다른 분야에만 있음: version-control — Merge protection gates | JetBrains의 CI 서버입니다. |

### Incremental build graphs (`t181.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `MSBuild` | proper_noun | 다른 분야에만 있음: languages-compilers — C# development ecosystem | .NET과 Visual Studio 프로젝트를 빌드하는 시스템입니다. |
| `SCons` | proper_noun | 사전에 없음 | Python으로 빌드 규칙을 쓰는 빌드 도구입니다. |

### JavaScript bundler optimization (`t249.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `asset hashing` | phrase | 사전에 없음 | 번들 산출물 이름에 내용 해시를 붙이는 설정입니다. |
| `Babel` | proper_noun | 다른 분야에만 있음: languages-compilers — Source transform configuration | JS 트랜스파일 단계를 담당하는 빌드 도구입니다. |
| `Snowpack` | proper_noun | 사전에 없음 | ESM 기반 개발 서버를 쓰던 번들 도구입니다. |
| `unbuild` | proper_noun | 사전에 없음 | 라이브러리용 통합 번들 도구입니다. |

### Npm package publishing (`t706.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `dist-tags` | identifier | 사전에 없음 | npm 패키지의 배포 태그 목록을 가리킵니다. |

### Package installation (`t39.8`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `uv` | proper_noun | 다른 분야에만 있음: languages-compilers — Python package configuration | 빠른 Python 패키지 관리자입니다. |

## 새 주제가 필요한 단어

### Monorepo workspace tooling

범위: Managing multiple packages in one repository with monorepo task runners and workspace definitions.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Nx` | proper_noun | 다른 분야에만 있음: databases-storage — Build artifact caching | 모노레포 빌드와 영향 범위 실행을 맡는 도구입니다. |
| `Turborepo` | proper_noun | 다른 분야에만 있음: databases-storage — Build artifact caching | JS 모노레포의 작업 실행과 캐시 도구입니다. |

### Release planning and cadence

범위: Planning release schedules, ownership, checklists and cadence for shipping versions.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `release manager` | phrase | 뜻 미배정 | 릴리스를 책임지는 담당자를 가리키는 말입니다. |
