# 설치 채널 개요

한국어 | [English](./index.md)

운영체제에 맞춰 소스 빌드 또는 사전 빌드 파일을 선택합니다. 설치 후 `codemap-search --version`을 실행하고 [MCP 클라이언트 등록](../../README.ko.md#mcp-클라이언트-등록)을 진행하세요.

| 운영체제 | 소스 빌드 | 사전 빌드 설치 |
|---|---|---|
| Linux | [Cargo](./crates-io.ko.md) | [설치 스크립트](./curl-installer.ko.md) 또는 GitHub 릴리스 압축 파일 |
| macOS | [Cargo](./crates-io.ko.md) | [설치 스크립트](./curl-installer.ko.md), [Homebrew](./homebrew.ko.md) 또는 GitHub 릴리스 압축 파일 |
| Windows | [Cargo](./crates-io.ko.md) | [WinGet 또는 압축 파일 수동 설치](./winget.ko.md) |

Cargo는 Rust와 C 도구 모음이 필요합니다. macOS/Linux 스크립트는 표준 POSIX 도구, `curl` 또는 `wget`, `tar`, SHA-256 도구가 필요하며 기본 설치 위치는 `~/.local/bin`입니다. Homebrew·WinGet은 자체 설치 위치를 관리합니다. 채널별 안내에서 버전 선택, `PATH`, 문제 해결, 배포 담당자의 게시 절차를 확인할 수 있습니다.

## 채널 제공 상태

2026-09-11 문서 검토에서 공식 페이지로 게시 버전과 패키지 수용 여부를 확인하지 못했습니다. 각 안내의 명령은 선택한 릴리스·패키지가 제공되어야 동작합니다. 공개 채널을 선택하기 전에 [crates.io 패키지](https://crates.io/crates/codemap-search), [GitHub Releases](https://github.com/buYoung/mcp/releases), [Homebrew 포뮬러 목록](https://formulae.brew.sh/formula/codemap-search), [WinGet 저장소](https://github.com/microsoft/winget-pkgs/tree/master/manifests/c/com/livteam/codemap-search)를 확인하세요.

저장소의 Homebrew 포뮬러와 WinGet 매니페스트에는 아직 임시 체크섬이 있습니다. 로컬 설치 전 릴리스 정보에 맞게 갱신해야 합니다. 공개 설치를 사용할 수 없으면 [로컬 소스 빌드 명령](./crates-io.ko.md#설치와-확인)을 사용하세요.

## 릴리스 파일 이름

저장소의 [릴리스 워크플로](../../../../.github/workflows/codemap-search-release.yml)에 설정된 대상입니다. 선택한 릴리스에 실제 파일이 있는지 확인하세요.

| 운영체제 | 아키텍처 | 설치 파일 |
|---|---|---|
| Linux | x86_64, 기본 musl | `codemap-search-x86_64-unknown-linux-musl.tar.gz` |
| Linux | x86_64, GNU | `codemap-search-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | arm64, musl | `codemap-search-aarch64-unknown-linux-musl.tar.gz` |
| macOS | Apple Silicon | `codemap-search-aarch64-apple-darwin.tar.gz` |
| macOS | Intel | `codemap-search-x86_64-apple-darwin.tar.gz` |
| Windows | x64 | `codemap-search-x86_64-pc-windows-msvc.zip` |
| Windows | arm64 | `codemap-search-aarch64-pc-windows-msvc.zip` |

Linux는 기본 musl을 사용하며 glibc가 필요하지 않습니다. GNU는 x86_64에서만 명시적으로 선택하며 Linux arm64 GNU는 설정되어 있지 않습니다. Windows 빌드는 실패하면 해당 압축 파일만 빠지고 다른 대상은 게시될 수 있습니다. Linux arm64는 크로스 빌드하며 Windows arm64도 크로스 빌드 후 해당 하드웨어에서 실행하지 않습니다. 빌드 파일만으로 실행 호환성을 확인할 수는 없습니다. Linux 검증 방법과 범위는 [Docker 안내](../../docker/README.ko.md)에 있습니다.
