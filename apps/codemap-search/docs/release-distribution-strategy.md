# codemap-search 배포 채널 전략

이 문서는 `codemap-search`를 어떤 채널로 배포할지에 대한 **전략과 목표**를 정리한다.
아직 대부분의 채널은 준비되어 있지 않다. 따라서 각 항목은 "지금 되는 것"과
"앞으로 하려는 것"을 분명히 구분해서 적는다. 모든 판단은 `확인됨`, `추론됨`,
`미준비`로 표기한다.

## 한 줄 요약

확인됨: 지금 실제로 동작하는 배포 경로는 **GitHub Release 사전 빌드 바이너리 하나뿐**이다.
Linux `musl` 빌드는 빌드 매트릭스에 추가되어 확인됨 상태다.
crates.io 자동 게시, `curl` installer, `mcp config` 명령,
Homebrew, WinGet은 **전부 미준비** 상태이며 이 문서의 목표일 뿐 현재 기능이 아니다.

## 현재 상태 (확인됨)

근거 파일: `.github/workflows/codemap-search-release.yml`, `apps/codemap-search/Cargo.toml`,
`apps/codemap-search/src/main.rs`, `apps/codemap-search/src/mcp/mod.rs`, `apps/codemap-search/README.md`.

- 확인됨: 릴리즈는 `codemap-v[0-9]+.[0-9]+.[0-9]+` 태그(예: `codemap-v0.0.0`) 푸시에서만
  GitHub Release 자산을 만든다. PR·브랜치 push로는 릴리즈가 동작하지 않으며,
  `codemap-v1`·`-rc1` 접미사 같은 형태는 매칭되지 않는다.
- 확인됨: 현재 빌드 대상은 다섯 가지이고, 각 자산에 `.sha256`를 함께 만든다.

```text
codemap-search-x86_64-unknown-linux-gnu.tar.gz
codemap-search-x86_64-unknown-linux-musl.tar.gz
codemap-search-aarch64-apple-darwin.tar.gz
codemap-search-x86_64-apple-darwin.tar.gz
codemap-search-x86_64-pc-windows-msvc.zip
```

- 확인됨: 워크플로 주석과 README 모두 Windows를 "best-effort(최선 노력)"로 명시한다.
- 확인됨(Docker): Linux x86_64 지원 범위 — musl 바이너리는 Ubuntu 22.04, 24.04, 26.04에서
  Docker-verified (exit 0, `--version` + `parse` 스모크); gnu 바이너리는 24.04, 26.04에서
  Docker-verified (exit 0), 22.04 이하는 GLIBC_2.39 누락으로 실패 확인됨.
- 지원 기준(실기기 확인, Docker 불가): macOS Sequoia (15)+, Windows 11+.
- 확인됨: `Cargo.toml`에는 `name`, `version`(`0.1.0`), `description`, `license`,
  `repository`, `readme`, `keywords`, `categories`가 있고 `publish = false`는 없다.
  즉 crates.io 게시에 필요한 메타데이터는 있으나, 워크플로는 `cargo publish`를 하지 않는다.
- 확인됨: `--version`과 `--help`는 `clap`의 `#[command(version, ...)]`로 지원된다.
- 확인됨: MCP 서버는 `codemap-search mcp`로 기동된다.
- 확인됨: README에 MCP 등록 안내가 있고, 설치 안내는 "소스 빌드"와 "사전 빌드 바이너리" 두 가지다.

## 아직 안 되는 것 (미준비)

이 항목들은 현재 코드/워크플로에 존재하지 않는다. 문서에서 "확정"이라고 적지 않는다.

- 확인됨: Linux `x86_64-unknown-linux-musl` 빌드. 빌드 매트릭스에 musl 타깃이 추가되어 정적 바이너리로 빌드되고 ubuntu:22.04, ubuntu:24.04, ubuntu:26.04 에서 Docker-verified 됨. gnu 빌드는 ubuntu:24.04, ubuntu:26.04 에서 Docker-verified 됨.
- 미준비: crates.io 자동 게시. 워크플로에 `cargo publish` 단계가 없다.
- 미준비: `curl` installer 스크립트(`install.sh`). 저장소에 없다.
- 미준비: `mcp config` 설정 출력 명령. `main.rs`의 `Commands` enum에 해당 서브커맨드가 없다.
- 미준비: Homebrew, WinGet 채널. 매니페스트/포뮬러가 없다.
- 확인됨: `serverInfo.version`이 `mcp/mod.rs`에서 `env!("CARGO_PKG_VERSION")`로 읽어
  `Cargo.toml` 버전과 자동 동기화된다. 별도 관리 불필요.

## 목표 채널과 우선순위

현재 준비 상태를 반영한 권장 순서다. 위로 갈수록 가깝고 비용이 낮다.

| 우선순위 | 채널 | 현재 상태 | 목표 |
|---|---|---|---|
| 1 | GitHub Release 바이너리 | 확인됨, 동작 | 원천 배포 자산으로 유지 |
| 2 | crates.io | 미준비 | 같은 버전으로 수동 게시부터 시작 |
| 3 | Linux `musl` 자산 | 확인됨, 동작 | gnu와 함께 제공해 glibc 차이 완화; 정적 바이너리로 Ubuntu 22.04~26.04 Docker-verified |
| 4 | `curl` installer | 미준비 | Release 자산 다운로드 + 체크섬 검증 보조 경로 |
| 5 | WinGet | 미준비 | Windows 지원이 안정화된 뒤 검토 |
| 6 | Homebrew core | 미준비 | 사용량/인지도 확보 후 검토 |

## 채널별 메모

### GitHub Release 바이너리 (지금 유일하게 동작)

확인됨: 이미 동작하는 단 하나의 경로다. 당분간 이것을 표준 배포 자산으로 둔다.
설치는 "플랫폼별 archive 다운로드 → 압축 해제 → `PATH`에 `codemap-search` 배치"다.

### crates.io (미준비, 다음 목표)

메타데이터는 있으나 자동 게시는 없다. 처음에는 자동화하지 말고 수동으로 시작한다.
crates.io는 같은 버전을 다시 못 올리므로, Release 자산·태그·버전이 맞는지 확인한 뒤 게시한다.

```bash
cargo publish --dry-run   # 먼저 검증
cargo publish             # 확인 후 수동 실행
```

### Linux musl (확인됨)

확인됨: `x86_64-unknown-linux-musl` 타깃이 빌드 매트릭스에 추가되어 정적 바이너리로 제공된다.
glibc 의존성이 없어 가장 넓은 Linux 배포판 지원이 가능하며 Ubuntu 22.04, 24.04, 26.04 에서 Docker-verified 됨.
gnu 바이너리(`x86_64-unknown-linux-gnu`)는 glibc 2.39+(Ubuntu 24.04+) 환경에서 동작하며, 24.04와 26.04에서 Docker-verified 됨.

### curl installer (미준비, 보조)

기본 경로가 아니라 빠른 설치용 보조 경로다. 스크립트는 특정 버전 Release 자산을 받고
SHA-256을 검증한 뒤 설치하는 얇은 래퍼여야 한다. 현재 그런 스크립트는 없다.

### Homebrew / WinGet (미준비, 보류)

둘 다 매니페스트·체크섬·버전 업데이트 등 추가 운영 부담이 있다.
Homebrew core는 사용 신호가 쌓인 뒤, WinGet은 Windows 지원이 best-effort를 벗어난 뒤 검토한다.

### npm 래퍼 (채택 안 함)

Rust 단일 바이너리이므로 핵심 설치 경로가 아니다. 현재 범위에서 다루지 않는다.

## 단계별 로드맵

```text
지금 (확인됨)
- GitHub Release 바이너리 (linux gnu x64, linux musl x64, macOS arm64/x64, Windows x64 best-effort)
- serverInfo.version 이 env!("CARGO_PKG_VERSION") 로 Cargo.toml 과 자동 동기화
- Linux musl: Ubuntu 22.04, 24.04, 26.04 Docker-verified (exit 0)
- Linux gnu: Ubuntu 24.04, 26.04 Docker-verified (exit 0); 22.04 이하 GLIBC_2.39 누락 확인됨
- macOS Sequoia (15)+: 지원 기준 (실기기 확인, Docker 불가)
- Windows 11+: 지원 기준, best-effort (실기기 확인, Docker 불가)

다음 (가까운 목표)
- crates.io 수동 게시

이후 (검토)
- curl installer
- mcp config 명령
- WinGet / Homebrew core
- Linux/Windows arm64 자산
```

## 배포 전 체크리스트 (목표 기준)

아래는 "되어 있어야 하는" 조건이다. 현재 다수는 미준비이며, 채널을 열기 전에 채워야 한다.

```text
[ ] Cargo.toml version 과 릴리즈 태그가 일치한다
[x] mcp serverInfo.version 이 Cargo.toml version 과 일치한다  (확인됨: env! 동기화)
[ ] Cargo.lock 이 커밋되어 있다
[ ] cargo publish --dry-run 이 통과한다                       (crates.io: 미준비)
[ ] codemap-search --version / --help 가 정상 출력된다         (확인됨)
[ ] codemap-search mcp 가 stdio 서버로 기동된다                (확인됨)
[ ] Linux x86_64 gnu 자산이 생성된다                          (확인됨)
[ ] Linux x86_64 musl 자산이 생성된다                         (확인됨, Docker-verified ubuntu:22.04, 24.04, 26.04)
[ ] Linux x86_64 gnu 자산 glibc 범위가 문서화된다              (확인됨: glibc 2.39+; Docker-verified 24.04, 26.04; 22.04 이하 FAIL)
[ ] macOS arm64 / x86_64 자산이 생성된다                       (확인됨)
[ ] Windows x86_64 자산이 생성된다                            (확인됨, best-effort)
[ ] 각 자산의 SHA-256 이 생성된다                             (확인됨)
[ ] README 설치 문구가 실제 자산 이름과 일치한다
[ ] Windows 지원 수준이 문서와 워크플로에서 일치한다            (확인됨)
```

## 외부 문서 안내 문구 (현 상태 기준)

```text
지금은 GitHub Release 사전 빌드 바이너리로만 설치할 수 있다.
플랫폼별 archive 를 내려받아 압축을 풀고 PATH 에 codemap-search 를 두면 된다.
Linux 는 x86_64 gnu 와 x86_64 musl 을 제공한다 (musl 권장: glibc 불필요, Ubuntu 22.04~26.04 Docker-verified).
macOS 는 arm64/x86_64 를 제공하며 Windows 는 best-effort 다.
crates.io, curl installer, Homebrew, WinGet 은 아직 제공하지 않는다.
MCP 등록은 codemap-search mcp 를 기준으로 한다.
```
