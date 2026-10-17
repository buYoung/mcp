# Version control and code review 누락 단어

- 분야 id: `version-control`
- 원본: [../version-control.md](../version-control.md)
- 누락 단어 58개 · 사전에 없음 43 · 뜻 미배정 0 · 다른 분야에만 있음 15
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Branching strategies (`t229.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `long-lived branch` | phrase | 사전에 없음 | main, develop처럼 오래 유지되는 브랜치로 브랜칭 전략 문서에서 단명 브랜치와 대비해 쓰입니다. |

### Build artifact cleanup (`t957.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `untracked` | word | 다른 분야에만 있음: text-processing — Glob file filtering | Git이 추적하지 않는 파일 상태로 git status 출력과 정리 스크립트에 쓰입니다. |

### Change comparison (`t295`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `git range-diff` | phrase | 사전에 없음 | 리베이스 전후 커밋 시리즈를 비교하는 명령으로 리뷰 문서에 나옵니다. |
| `hunk` | word | 다른 분야에만 있음: algorithms-data-structures — Sequence diff computation | diff와 패치에서 연속된 변경 덩어리를 가리키는 용어로 git add -p와 리뷰 도구에 쓰입니다. |
| `name-only` | identifier | 사전에 없음 | git diff와 git log에서 변경 파일 이름만 출력하는 옵션입니다. |
| `range-diff` | identifier | 사전에 없음 | git range-diff 명령을 줄여 부르는 이름으로 패치 시리즈 비교 문서에 쓰입니다. |
| `unified diff` | phrase | 다른 분야에만 있음: algorithms-data-structures — Sequence diff computation | ---/+++와 @@ 헝크 머리글을 쓰는 표준 diff 출력 형식으로 패치 파일과 리뷰 도구가 다룹니다. |

### Code repositories (`t205`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `.gitmodules` | identifier | 다른 분야에만 있음: security — URL phishing | 서브모듈의 경로와 URL을 적는 저장소 루트 설정 파일입니다. |
| `monorepo` | word | 다른 분야에만 있음: build-release — Build caching | 여러 프로젝트를 한 저장소에서 관리하는 구조로 저장소 레이아웃과 코드 소유 문서의 핵심 용어입니다. |

### Code review submissions (`t985`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Crucible` | proper_noun | 사전에 없음 | Atlassian의 코드 리뷰 도구로 리뷰 요청 문서에 나옵니다. |
| `Reviewable` | proper_noun | 사전에 없음 | GitHub 풀 리퀘스트 위에서 동작하는 코드 리뷰 도구입니다. |
| `stacked diffs` | phrase | 사전에 없음 | 작은 변경을 순서대로 쌓아 따로 리뷰하는 워크플로로 Phabricator와 Graphite 문서에 쓰입니다. |
| `stacked pull requests` | phrase | 사전에 없음 | 서로 의존하는 풀 리퀘스트를 순서대로 쌓아 리뷰하는 방식을 가리킵니다. |

### Commit history (`t145.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `bisect` | word | 다른 분야에만 있음: observability-debugging — Regression bisection | git bisect처럼 이진 탐색으로 문제를 일으킨 커밋을 찾는 동작을 가리키며 이 분야에서는 커밋 이력 탐색의 핵심입니다. |
| `commit range` | phrase | 사전에 없음 | A..B처럼 커밋 구간을 지정하는 표기로 로그, 리뷰, 릴리스 노트 스크립트에 쓰입니다. |

### Git commit hooks (`t156`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `applypatch-msg` | identifier | 사전에 없음 | git am이 패치 메시지를 검사할 때 실행되는 훅 이름입니다. |
| `BREAKING CHANGE` | phrase | 다른 분야에만 있음: build-release — Release notes | Conventional Commits에서 호환 깨짐을 표시하는 푸터 토큰으로 릴리스 도구가 읽습니다. |
| `git commit --no-verify` | phrase | 사전에 없음 | pre-commit과 commit-msg 훅을 건너뛰는 옵션으로 훅 우회 정책 문서에 나옵니다. |
| `post-rewrite` | identifier | 사전에 없음 | amend나 rebase로 커밋이 다시 쓰인 뒤 실행되는 훅 이름입니다. |
| `pre-merge-commit` | identifier | 사전에 없음 | 병합 커밋을 만들기 전에 실행되는 Git 훅 이름입니다. |
| `pre-rebase` | identifier | 사전에 없음 | 리베이스 시작 전에 실행되는 Git 훅 이름입니다. |

### Git commits (`t140.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `git commit -a` | phrase | 사전에 없음 | 추적 중인 변경 파일을 자동으로 스테이징하며 커밋하는 옵션입니다. |

### Git forge hosting (`t627.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cgit` | proper_noun | 사전에 없음 | C로 만든 가벼운 Git 웹 인터페이스로 자체 호스팅 설정에 나옵니다. |
| `GitBucket` | proper_noun | 사전에 없음 | Scala로 만든 자체 호스팅 Git 플랫폼입니다. |
| `Gitee` | proper_noun | 사전에 없음 | 중국의 Git 코드 호스팅 서비스로 미러링 설정에 나옵니다. |
| `Gitolite` | proper_noun | 사전에 없음 | SSH 기반 Git 서버의 접근 제어 도구로 자체 호스팅 문서에 나옵니다. |
| `gitweb` | proper_noun | 사전에 없음 | Git에 포함된 기본 웹 인터페이스로 서버 설정에 나옵니다. |
| `SourceHut` | proper_noun | 사전에 없음 | 메일링 리스트 기반 워크플로를 쓰는 Git 호스팅 서비스입니다. |

### Git LFS attributes (`t627.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `textconv` | identifier | 사전에 없음 | 바이너리 파일을 텍스트로 바꿔 diff하는 gitattributes 속성입니다. |

### Git object cleanup (`t192`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `git maintenance` | phrase | 사전에 없음 | 백그라운드 저장소 최적화 작업을 예약하는 명령입니다. |

### Git reset and unstaging (`t957.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `revert` | word | 다른 분야에만 있음: software-design — Change rollback | git revert와 되돌리기 커밋을 뜻하며 이 분야에서는 되돌리기 명령과 커밋 접두사의 핵심입니다. |

### GitHub developer services (`t624`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `GITHUB_REPOSITORY_OWNER` | identifier | 사전에 없음 | GitHub Actions에서 저장소 소유자 이름을 담는 환경 변수입니다. |
| `personal access token` | phrase | 다른 분야에만 있음: auth-identity — Token credentials | GitHub에서 HTTPS 푸시와 API 호출에 비밀번호 대신 쓰는 토큰입니다. |

### History rewriting (`t140.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `rebase --abort` | phrase | 사전에 없음 | 진행 중인 리베이스를 취소하는 옵션입니다. |

### Merge commit strategies (`t682.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `subtree merge` | phrase | 사전에 없음 | 서브트리 전략으로 다른 저장소를 하위 경로에 병합하는 방식입니다. |

### Merge protection gates (`t229.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `dismiss stale review` | phrase | 사전에 없음 | 새 커밋이 푸시되면 이전 승인을 무효화하는 브랜치 보호 옵션입니다. |

### Perforce version control (`t651`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `shelveset` | word | 사전에 없음 | TFVC에서 임시 보관한 변경 묶음을 가리키는 용어입니다. |

### Pull request review (`t145.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `CodeRabbit` | proper_noun | 사전에 없음 | 풀 리퀘스트에 자동 리뷰 댓글을 남기는 AI 리뷰 서비스입니다. |

### Repository collaboration (`t140.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `fetch upstream` | phrase | 사전에 없음 | 포크 동기화에서 원본 저장소 변경을 가져오는 단계입니다. |

### VCS abstractions (`t140.7`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Breezy` | proper_noun | 사전에 없음 | Bazaar에서 갈라져 나온 버전 관리 시스템입니다. |
| `bzr` | abbreviation | 사전에 없음 | Bazaar의 명령줄 도구 이름입니다. |
| `ClearCase` | proper_noun | 사전에 없음 | IBM의 기업용 버전 관리 시스템입니다. |
| `dulwich` | proper_noun | 사전에 없음 | 순수 Python으로 구현한 Git 라이브러리입니다. |
| `gitoxide` | proper_noun | 사전에 없음 | Rust로 구현한 Git 라이브러리와 도구 모음입니다. |
| `Pijul` | proper_noun | 사전에 없음 | 패치 기반의 분산 버전 관리 시스템입니다. |
| `Plastic SCM` | proper_noun | 사전에 없음 | 게임 개발에서 쓰이는 버전 관리 시스템입니다. |
| `PyGithub` | proper_noun | 사전에 없음 | GitHub REST API의 Python 클라이언트 라이브러리입니다. |
| `Visual SourceSafe` | proper_noun | 사전에 없음 | Microsoft의 구형 버전 관리 도구입니다. |

### Version control revisions (`t972`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `named branch` | phrase | 사전에 없음 | Mercurial에서 커밋에 기록되는 브랜치 이름입니다. |

## 새 주제가 필요한 단어

### Git client tools

범위: Using graphical, terminal, and editor clients and diff viewers for Git.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `difftastic` | proper_noun | 사전에 없음 | 구문을 이해하는 구조적 diff 도구로 git diff 외부 도구로 설정합니다. |
| `SmartGit` | proper_noun | 사전에 없음 | 그래픽 Git 클라이언트입니다. |
| `TortoiseGit` | proper_noun | 사전에 없음 | Windows 탐색기용 Git 클라이언트입니다. |

### Git ignore rules

범위: Excluding files from tracking with gitignore patterns and local exclude files.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `.gitignore` | identifier | 다른 분야에만 있음: build-release — Build artifacts | 추적에서 제외할 파일 패턴을 적는 파일로 빌드 산출물과 비밀 파일 제외에 쓰입니다. |

### Git patch files

범위: Creating, mailing, and applying patch files and mbox series with Git.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `patch` | word | 다른 분야에만 있음: algorithms-data-structures — Sequence diff computation | diff를 담은 변경 묶음으로 이 분야에서는 패치 파일과 git patch 흐름의 핵심입니다. |

### Git tags and releases

범위: Marking releases and revisions with lightweight and annotated Git tags.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `annotated tag` | phrase | 다른 분야에만 있음: build-release — Release automation | 작성자와 메시지를 가진 태그 객체로 릴리스 표시에 쓰입니다. |
| `lightweight tag` | phrase | 다른 분야에만 있음: build-release — Release automation | 커밋을 바로 가리키는 단순 태그로 annotated tag와 대비됩니다. |
| `tag` | word | 다른 분야에만 있음: build-release — Release automation | 릴리스나 특정 커밋에 붙이는 이름으로 git tag 명령과 refs/tags의 핵심 용어입니다. |
| `tags` | word | 다른 분야에만 있음: build-release — Release automation | git tag가 만드는 이름표들과 git fetch --tags 같은 옵션에 쓰는 복수 표기입니다. |
