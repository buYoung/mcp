# codemap-search 배포 채널 전략

이 문서는 `codemap-search`를 어떤 채널로 배포할지에 대한 **전략과 현재 상태**를 정리한다.
배포 채널은 **우열 없이 동등(co-equal)** 하게 다룬다. 특정 채널을 "1순위", 다른 채널을
"나중에 검토"로 미루지 않는다. 각 채널은 산출물(워크플로 잡, 스크립트, 매니페스트, 포뮬러)이
저장소에 들어와 있고 검증 가능한 상태에서 출시 준비를 마쳤다. 모든 판단은 `확인됨`, `추론됨`으로
표기하며, 각 채널이 **지금 실제로 검증된 것**과 **첫 릴리스 태그 이후에 확정될 것**을
정직하게 구분한다.

## 한 줄 요약

확인됨: 다섯 개 배포 채널 — **GitHub Release 사전 빌드 바이너리 / crates.io / `curl` installer(`install.sh`) /
WinGet / Homebrew** — 의 산출물이 모두 저장소에 들어와 검증을 마쳤다. npm 래퍼는 **채택하지 않는다**(거부 결정,
보류 아님).

확인됨(로드베어링 전제): 저장소에는 아직 `codemap-v*` 태그가 하나도 없다(`git tag --list` 빈 출력).
따라서 **첫 `codemap-v0.1.0` 태그가 모든 채널의 공통 해제 조건(shared unlock)** 이다 — 이 태그가
릴리스 자산을 생성하고, 형제 `.sha256` 파일을 만들어 WinGet/Homebrew의 placeholder 체크섬을 채우게 하며,
crates.io 실제 게시를 트리거하고, 각 빌드 leg(Windows·arm64 포함)의 첫 실제 CI 검증이 된다.
지금 단계의 검증 천장은 "valid + 검증됨(로컬)"이지 "submitted/merged/published"가 아니다.

## 배포 채널 (동등, 우선순위 없음)

| 채널 | 상태 | 지금 검증된 것 | 첫 `codemap-v0.1.0` 태그 이후 확정될 것 |
|---|---|---|---|
| GitHub Release 바이너리 | 확인됨 | 7개 타깃 빌드 매트릭스·자산명 규칙·`.sha256` 생성이 워크플로에 정의됨 | 첫 태그가 자산을 실제로 게시 (현재 태그 0개) |
| crates.io | 확인됨 / CI 자동화 | `cargo publish --dry-run` exit 0(진짜 green); `publish-crate` 잡이 `codemap-v*` 태그에서 멱등 게시 | 실제 게시 — `CARGO_REGISTRY_TOKEN` 시크릿 + 실태그 필요. 첫 게시가 `0.1.0`을 영구 고정 |
| `curl` installer (`install.sh`) | 확인됨 | 구문·OS/arch 매핑·체크섬 검증·오프라인 전체 흐름이 로컬 green(`file://` 픽스처) | 라이브 GitHub 다운로드 성공 경로 — 실태그가 자산을 게시한 뒤 |
| WinGet | 확인됨 / 제출 가능 | 매니페스트 3종이 스키마 1.12.0 구조 정합·YAML well-formed | `microsoft/winget-pkgs` 머지(Microsoft 검토); placeholder sha256 실값 교체 |
| Homebrew (macOS 전용) | 확인됨 / 제출 가능 | 포뮬러가 `brew style` exit 0 + 오프라인 `FormulaAudit` cop 통과 | `homebrew/homebrew-core` 수용(인지도 심사); placeholder sha256 실값 교체 |

세 채널(WinGet / Homebrew / Release 다운로드)은 모두 첫 릴리스 자산에 의존한다. 자산이 아직 없으므로
WinGet `InstallerSha256`·Homebrew `sha256`는 **명시적 placeholder(64개의 `0`)** 로 두었고,
첫 `codemap-v0.1.0` 태그가 자산과 형제 `.sha256`를 만든 뒤 실값으로 교체한다. 이 placeholder는
work-failure가 아니라 예상된 보류 전제조건이다.

## 현재 상태 (확인됨)

근거 파일: `.github/workflows/codemap-search-release.yml`, `apps/codemap-search/Cargo.toml`,
`apps/codemap-search/src/main.rs`, `apps/codemap-search/src/mcp/mod.rs`, `apps/codemap-search/README.md`,
`apps/codemap-search/install.sh`, `apps/codemap-search/packaging/`.

- 확인됨: 릴리즈는 `codemap-v[0-9]+.[0-9]+.[0-9]+` 태그(예: `codemap-v0.1.0`) 푸시에서만
  GitHub Release 자산을 만든다. PR·브랜치 push로는 릴리즈가 동작하지 않으며,
  `codemap-v1`·`-rc1` 접미사 같은 형태는 매칭되지 않는다.
- 확인됨: 빌드 매트릭스는 일곱 가지 타깃이고, 각 자산에 형제 `.sha256`를 함께 만든다.

```text
codemap-search-x86_64-unknown-linux-gnu.tar.gz
codemap-search-x86_64-unknown-linux-musl.tar.gz
codemap-search-aarch64-unknown-linux-musl.tar.gz
codemap-search-aarch64-apple-darwin.tar.gz
codemap-search-x86_64-apple-darwin.tar.gz
codemap-search-x86_64-pc-windows-msvc.zip
codemap-search-aarch64-pc-windows-msvc.zip
```

- 확인됨(빌드): Linux arm64는 `aarch64-unknown-linux-musl` 정적 바이너리로 cross-rs를 통해
  크로스 빌드된다(실제 `cross build` 성공, `ELF 64-bit ... ARM aarch64, statically linked` 확인).
  단 arm64 하드웨어 **런타임 실행은 미검증**(빌드만 확인됨).
- 확인됨(빌드 도구 분기): Windows arm64는 `aarch64-pc-windows-msvc`로 x64 러너에서 **build-only**
  로 빌드된다. arm64 Windows에서의 실행은 미검증.
- 확인됨: 공유 `Build` 스텝에 `shell: bash`가 지정되어 있어 Windows 러너에서도 `--manifest-path`
  인수가 정상 전개된다(이전에는 `shell:` 키 부재로 pwsh가 POSIX식 변수를 빈 문자열로 확장하는
  버그가 있었으나 수정됨). 따라서 두 Windows leg는 실제 러너에서 빌드 가능하다. 다만 아직 태그가
  실행된 적이 없으므로 **첫 태그가 Windows 빌드의 첫 실제 CI 검증**이 된다.
- 확인됨(Docker): Linux x86_64 지원 범위 — musl 바이너리는 Ubuntu 22.04, 24.04, 26.04에서
  Docker-verified (exit 0, `--version` + `parse` 스모크); gnu 바이너리는 22.04, 24.04, 26.04에서
  Docker-verified (exit 0); 20.04 이하는 GLIBC_2.32/2.33/2.34 누락으로 실패 확인됨.
- 지원 기준(실기기 확인, Docker 불가): macOS Sequoia (15)+, Windows 11+.
- 확인됨: `Cargo.toml`에는 `name`, `version`(`0.1.0`), `description`, `license`,
  `repository`, `readme`, `keywords`, `categories`가 있고 `publish = false`는 없다.
  crates.io 게시에 필요한 메타데이터가 갖춰져 있다.
- 확인됨: 워크플로에 `publish-crate` 잡이 있어 `codemap-v*` 태그에서 crates.io에 게시한다.
  이 잡은 바이너리 `release` 잡을 게이트하지 않고(`needs:` 없음) 병렬로 돈다.
- 확인됨: `--version`과 `--help`는 `clap`의 `#[command(version, ...)]`로 지원된다.
- 확인됨: MCP 서버는 `codemap-search mcp`로 기동된다.
- 확인됨: `serverInfo.version`이 `mcp/mod.rs`에서 `env!("CARGO_PKG_VERSION")`로 읽어
  `Cargo.toml` 버전과 자동 동기화된다. 별도 관리 불필요.
- 확인됨: README에 MCP 등록 안내가 있고, 설치 안내는 crates.io / WinGet / Homebrew /
  소스 빌드 / 사전 빌드 바이너리(+`install.sh`)를 모두 다룬다.

## 채널별 메모

각 채널의 상세 운영 가이드(메인테이너 사전 준비 + 게시/제출 런북 + 엔드유저 설치)는
[`distribution/`](./distribution/) 디렉터리에 있다.

### GitHub Release 바이너리

확인됨: 워크플로가 일곱 타깃의 archive와 형제 `.sha256`를 생성한다. 설치는 "플랫폼별 archive
다운로드 → 압축 해제 → `PATH`에 `codemap-search` 배치"다. `install.sh`가 이 과정을 자동화한다.
가이드: [`distribution/curl-installer.md`](./distribution/curl-installer.md),
[`distribution/index.md`](./distribution/index.md).

### crates.io

확인됨 / CI 자동화: `publish-crate` 잡이 `codemap-v*` 태그에서 crates.io에 게시한다.
멱등 가드(crates.io 인덱스 사전조회)로 같은 버전 재게시 시도를 skip한다. `cargo publish --dry-run`은
exit 0(진짜 green). 실제 게시는 `CARGO_REGISTRY_TOKEN` 시크릿과 실태그가 있어야만 CI에서 발생하며,
첫 게시가 `0.1.0`을 crates.io에 **영구 고정**한다(재게시 불가). 가이드:
[`distribution/crates-io.md`](./distribution/crates-io.md).

```bash
cargo publish --dry-run   # 로컬 검증 (exit 0 확인됨)
# 실제 게시는 codemap-v0.1.0 태그 푸시 시 CI의 publish-crate 잡이 수행
```

### Linux musl / arm64

확인됨: `x86_64-unknown-linux-musl`·`aarch64-unknown-linux-musl` 타깃이 정적 바이너리로 제공된다.
glibc 의존성이 없어 가장 넓은 Linux 배포판을 지원하며 x86_64 musl은 Ubuntu 22.04, 24.04, 26.04에서
Docker-verified 됨. gnu 바이너리(`x86_64-unknown-linux-gnu`)는 glibc 2.34+(Ubuntu 22.04+) 환경에서
동작하며 22.04, 24.04, 26.04에서 Docker-verified 됨; 20.04 이하는 실패. arm64 musl은 크로스 빌드
확인됨(arm64 하드웨어 런타임 미검증). Linux 권장 설치 경로는 `cargo install codemap-search`다.

### curl installer (`install.sh`)

확인됨: macOS + Linux용 POSIX `sh` 설치기. 특정/최신 Release 자산을 받아 **추출 이전에 SHA-256을
검증**한 뒤 `~/.local/bin`(기본, `INSTALL_DIR`로 override)에 설치한다. 범용 Linux는 musl 정적 빌드를
기본 선호한다. 구문·매핑·체크섬·오프라인 전체 흐름이 로컬 green이며, 라이브 다운로드 성공 경로만
첫 실태그 이후 확정된다. 가이드: [`distribution/curl-installer.md`](./distribution/curl-installer.md).

### WinGet

확인됨 / 제출 가능: `com.livteam.codemap-search` 멀티파일 매니페스트(version/defaultLocale/installer)가
스키마 1.12.0 구조 정합·YAML well-formed. x64·arm64 두 인스톨러를 커버하며 arm64는 build-only.
`InstallerSha256`는 placeholder(64-zeros)로, 첫 릴리스 후 형제 `.sha256`에서 실값을 채운다.
실제 availability는 `microsoft/winget-pkgs` 머지(Microsoft 검토) 이후다. 가이드:
[`distribution/winget.md`](./distribution/winget.md).

### Homebrew (macOS 전용)

확인됨 / 제출 가능: `homebrew/homebrew-core`용 포뮬러(Option B — 자체 tap 없음). macOS arm64·x86_64
darwin tarball을 설치한다. `brew style` exit 0 + 오프라인 `FormulaAudit` cop 통과. `sha256`는
placeholder로, 첫 릴리스 후 형제 `.sha256`에서 실값을 채운다. 실제 `brew install codemap-search`
availability는 homebrew-core 수용(인지도 심사) 이후다. homebrew-core가 일반적으로 소스 빌드를
선호하는 데 비해 본 포뮬러는 사전 빌드 바이너리를 쓰므로, 첫 실제 `brew audit --new` 시 이 차이가
지적될 수 있다(가이드에 정직 표기). 그 전까지 macOS 폴백은 `install.sh` / `cargo install` / Release
직접 다운로드다. 가이드: [`distribution/homebrew.md`](./distribution/homebrew.md).

### npm 래퍼 (채택 안 함)

Rust 단일 바이너리이므로 npm 래퍼는 핵심 설치 경로가 아니다. **채택하지 않는다** — 이는 거부 결정이지
보류가 아니다. 현재·향후 범위에서 다루지 않는다.

## 추후작업 (future work)

아래는 현재 구현하지 않았으나 향후 추가를 계획하는 Linux 채널이다. 지금은 산출물이 없으므로
위 동등 채널 목록에 포함하지 않고 명시적으로 미래 작업으로 분리한다.

- **Snap** (`snapcraft` / Snap Store): 우분투 계열 광범위 배포용. `snapcraft.yaml` 작성 + Snap Store
  등록이 필요하다. 미구현.
- **AUR** (Arch User Repository): Arch Linux용 `PKGBUILD` + AUR 게시. 미구현.

두 채널 모두 산출물·검증이 없으며, 추가 시 본 문서의 동등 채널 목록으로 승격한다.

## 단계별 로드맵

```text
지금 (확인됨, 산출물 저장소 반영)
- GitHub Release 바이너리: 7개 타깃 매트릭스
  (linux gnu x64, linux musl x64, linux musl arm64,
   macOS arm64/x64, Windows x64, Windows arm64 build-only)
- crates.io: publish-crate 잡 (codemap-v* 태그 트리거, dry-run exit 0)
- curl installer (install.sh): macOS + Linux, 로컬 green
- WinGet: com.livteam.codemap-search 매니페스트 (제출 가능)
- Homebrew: codemap-search.rb 포뮬러 (제출 가능, macOS 전용)
- serverInfo.version 이 env!("CARGO_PKG_VERSION") 로 Cargo.toml 과 자동 동기화
- Linux musl/gnu x64: Ubuntu 22.04, 24.04, 26.04 Docker-verified (exit 0)
- macOS Sequoia (15)+: 지원 기준 (실기기 확인, Docker 불가)
- Windows 11+: 지원 기준 (실기기 확인, Docker 불가)

공통 해제 조건 (첫 codemap-v0.1.0 태그)
- 7개 자산 + 형제 .sha256 실제 게시
- crates.io 실제 게시 (CARGO_REGISTRY_TOKEN 시크릿 필요, 0.1.0 영구 고정)
- WinGet/Homebrew placeholder sha256 → 실값 교체
- Windows·arm64 빌드 leg의 첫 실제 CI 검증
- install.sh 라이브 다운로드 성공 경로 확정

태그 이후 외부 절차 (사용자 직접)
- WinGet: microsoft/winget-pkgs 제출 → Microsoft 검토 머지
- Homebrew: homebrew/homebrew-core 제출 → 인지도 심사 수용

추후작업
- Snap (snapcraft / Snap Store)
- AUR (PKGBUILD)
```

## 배포 전 체크리스트

아래는 첫 `codemap-v0.1.0` 릴리스 전에 채워야 하는 조건이다.

```text
[ ] Cargo.toml version 과 릴리즈 태그(codemap-v0.1.0)가 일치한다
[x] mcp serverInfo.version 이 Cargo.toml version 과 일치한다  (확인됨: env! 동기화)
[x] Cargo.lock 이 커밋되어 있다                               (확인됨: git ls-files 추적됨)
[x] cargo publish --dry-run 이 통과한다                       (확인됨: exit 0)
[x] codemap-search --version / --help 가 정상 출력된다         (확인됨)
[x] codemap-search mcp 가 stdio 서버로 기동된다                (확인됨)
[x] Linux x86_64 gnu 자산이 매트릭스에 정의된다                (확인됨)
[x] Linux x86_64 musl 자산이 매트릭스에 정의된다               (확인됨, Docker-verified ubuntu:22.04, 24.04, 26.04)
[x] Linux arm64 musl 자산이 매트릭스에 정의된다                (확인됨: cross 빌드; arm64 런타임 미검증)
[x] Linux x86_64 gnu 자산 glibc 범위가 문서화된다              (확인됨: glibc 2.34+; Docker-verified 22.04, 24.04, 26.04; 20.04 이하 FAIL)
[x] macOS arm64 / x86_64 자산이 매트릭스에 정의된다            (확인됨)
[x] Windows x86_64 / arm64 자산이 매트릭스에 정의된다          (확인됨; arm64 build-only)
[x] 각 자산의 SHA-256 이 생성된다                             (확인됨: 워크플로 Package 스텝)
[x] install.sh 가 자산명·.sha256 포맷과 일치한다              (확인됨: 오프라인 전체 흐름 green)
[x] WinGet/Homebrew 매니페스트·포뮬러가 valid 하다            (확인됨: 스키마 정합 / brew style exit 0 — 단 valid≠설치가능: 아래 sha256 교체가 끝나야 install 통과)
[ ] 첫 태그 후 WinGet/Homebrew placeholder sha256 을 실값으로 교체한다
[ ] README 설치 문구가 실제 자산 이름과 일치한다              (확인됨: 7개 자산 반영)
[ ] CARGO_REGISTRY_TOKEN 시크릿이 등록되어 있다               (메인테이너 수동 — distribution/crates-io.md 참조)
```

## 외부 문서 안내 문구 (현 상태 기준)

```text
codemap-search 는 다섯 채널로 설치할 수 있다:
- cargo install codemap-search          (crates.io; Linux 권장)
- winget install com.livteam.codemap-search   (Windows; Microsoft 검토 후 머지)
- brew install codemap-search           (macOS; homebrew-core 수용 후)
- curl … install.sh | sh                (macOS + Linux; .sha256 검증 후 설치)
- GitHub Release 사전 빌드 바이너리       (7개 타깃 archive + .sha256 직접 다운로드)
모든 채널의 라이브 availability 는 첫 codemap-v0.1.0 태그(자산 생성 + 외부 머지)에 달려 있다.
MCP 등록은 codemap-search mcp 를 기준으로 한다.
```
