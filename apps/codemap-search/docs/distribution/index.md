# codemap-search 설치 채널 개요 (per-OS)

이 문서는 OS별 설치 경로를 한눈에 정리하고, 각 채널 가이드로 연결한다. 모든 채널은 **우열 없이
동등(co-equal)** 하다. 각 채널의 메인테이너 사전 준비·게시/제출 런북은 채널별 가이드를 참조한다.

- [crates.io 가이드](./crates-io.md)
- [WinGet 가이드](./winget.md)
- [Homebrew 가이드](./homebrew.md)
- [curl installer 가이드](./curl-installer.md)

> 로드베어링 전제: 저장소에는 아직 `codemap-v*` 태그가 하나도 없다. 따라서 아래 라이브 설치
> 명령은 **첫 `codemap-v0.1.0` 태그가 자산을 게시하고(그리고 WinGet/Homebrew는 외부 머지가 끝난)
> 뒤에** 실제로 동작한다. 그 전까지는 소스 빌드 또는 in-repo 매니페스트/포뮬러 로컬 설치를 쓴다.

## OS별 권장 경로

### Linux

**권장: `cargo install codemap-search`** (crates.io).

```sh
cargo install codemap-search
```

대안:

- `install.sh` 원라이너 (사전 빌드 바이너리, `.sha256` 검증 후 `~/.local/bin` 설치):
  ```sh
  curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
  ```
- GitHub Release 자산 직접 다운로드 + 압축 해제 + `PATH` 배치.

Linux 자산은 **musl 정적 빌드**가 기본이다(glibc 불필요). 아키텍처별 자산명:

| 아키텍처 | 자산명 | 비고 |
|---|---|---|
| x86_64 | `codemap-search-x86_64-unknown-linux-musl.tar.gz` | 기본(musl 선호); Ubuntu 22.04~26.04 Docker-verified |
| x86_64 (gnu) | `codemap-search-x86_64-unknown-linux-gnu.tar.gz` | glibc 2.34+ 필요; `CODEMAP_LINUX_LIBC=gnu` 명시 시에만 |
| arm64 (aarch64) | `codemap-search-aarch64-unknown-linux-musl.tar.gz` | 크로스 빌드(cross-rs); arm64 하드웨어 런타임 미검증 |

> arm64 Linux는 **musl만** 제공한다(gnu arm64 자산 없음). `install.sh`에서 arm64 + gnu 요청은
> 자산 부재로 명시적 에러 처리된다.

### macOS

라이브 경로: **`brew install codemap-search`** (Homebrew).

```sh
brew install codemap-search
```

단 homebrew-core 수용(인지도 심사)이 끝나기 전까지 `brew install`은 동작하지 않는다.
그 전까지 **macOS 폴백**:

- `cargo install codemap-search` (crates.io)
- `install.sh` 원라이너 (위 Linux와 동일 명령; macOS/arm64·x86_64 자동 감지)
- GitHub Release 자산 직접 다운로드.

macOS 자산명:

| 아키텍처 | 자산명 |
|---|---|
| arm64 (Apple Silicon) | `codemap-search-aarch64-apple-darwin.tar.gz` |
| x86_64 (Intel) | `codemap-search-x86_64-apple-darwin.tar.gz` |

### Windows

라이브 경로: **`winget install com.livteam.codemap-search`** (WinGet).

```powershell
winget install com.livteam.codemap-search
```

단 `microsoft/winget-pkgs` 머지(Microsoft 검토)가 끝나기 전까지는 in-repo 매니페스트로 로컬 설치:

```powershell
winget install --manifest apps/codemap-search/packaging/winget
```

두 가지 조건이 있다: (a) 첫 `codemap-v0.1.0` 릴리스가 자산을 게시하고 매니페스트의 placeholder
`sha256`를 실값으로 교체한 **이후에만** 다운로드·해시 검증을 통과한다, (b) 경로가 상대경로이므로
**리포 루트에서 실행**한다.

> **WinGet 머지 전·placeholder 기간의 확실한 폴백** (툴체인 불필요): GitHub Release에서
> `codemap-search-x86_64-pc-windows-msvc.zip`을 직접 다운로드 → 압축 해제 → `codemap-search.exe`를
> `PATH` 디렉터리에 배치한다. macOS의 `install.sh` 폴백과 대칭이며, WinGet 머지/실 sha256 교체를 기다리지
> 않고 지금 바로 설치하는 경로다. (arm64는 `codemap-search-aarch64-pc-windows-msvc.zip`, build-only.)

Windows 자산명:

| 아키텍처 | 자산명 | 비고 |
|---|---|---|
| x86_64 | `codemap-search-x86_64-pc-windows-msvc.zip` | 실기기 지원 기준 |
| arm64 (aarch64) | `codemap-search-aarch64-pc-windows-msvc.zip` | **build-only** — x64 러너 크로스 빌드, arm64 실행 미검증 |

`install.sh`는 macOS + Linux 전용이다. Windows는 WinGet을 쓴다.

## install.sh 동작 요약

- 기본 설치 경로: `~/.local/bin` (sudo 불필요). `INSTALL_DIR` 환경변수로 override
  (예: `INSTALL_DIR=/usr/local/bin …`).
- `uname -s`/`uname -m`으로 OS·arch 감지 → 타깃 트리플·자산명 결정. arm 정규화(`arm64`/`aarch64`→`aarch64`,
  `x86_64`/`amd64`→`x86_64`).
- **Linux는 musl 선호**: 기본 musl 정적 빌드. x86_64에서 `CODEMAP_LINUX_LIBC=gnu` 명시 시에만 gnu.
- **추출 이전에 `.sha256` 검증**: archive 다운로드 → `.sha256` 다운로드 → 검증 → (성공 시에만) `tar -xzf`.
  체크섬 불일치 시 non-zero exit로 중단하며 아무것도 설치하지 않는다.
- 버전 핀: `--version codemap-vX.Y.Z` 또는 `CODEMAP_VERSION` (기본은 `releases/latest`).
- 설치 디렉터리가 `PATH`에 없으면 `export PATH=…` 안내를 출력한다.

자세한 내용은 [curl installer 가이드](./curl-installer.md) 참조.

## 추후작업 (future work)

다음 Linux 채널은 현재 미구현이며 향후 추가를 계획한다 — Snap(`snapcraft` / Snap Store),
AUR(`PKGBUILD`). 산출물이 들어오면 동등 채널로 승격한다.
