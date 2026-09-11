# 설치 스크립트 (`install.sh`)

한국어 | [English](./curl-installer.md)

macOS와 Linux에서 운영체제·아키텍처에 맞는 codemap-search 설치 파일을 받아 설치합니다. 압축을 풀기 전에 `.sha256` 체크섬을 검증하고 기본적으로 `~/.local/bin`에 설치합니다. Windows는 [WinGet·수동 설치 안내](./winget.ko.md)를 참고하세요.

## 설치

POSIX 셸, 표준 시스템 도구, `curl` 또는 `wget`, `tar`, `sha256sum` 또는 `shasum`이 필요합니다. 별도 런타임은 필요하지 않습니다. 다음 명령은 `curl`을 사용합니다.

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
```

설치 후 `codemap-search --version`을 실행하세요. 명령을 찾지 못하면 설치 디렉터리를 `PATH`에 추가합니다. 스크립트가 출력하는 `export PATH` 줄을 `~/.zshrc` 또는 `~/.bashrc`에 추가하면 이후 셸에서도 유지됩니다.

선택한 릴리스에 압축 파일과 `.sha256` 파일이 모두 있어야 합니다. 2026-09-11 문서 검토에서 실제 다운로드 제공 여부를 확인하지 못했습니다. 사용할 태그와 파일을 [GitHub Releases](https://github.com/buYoung/mcp/releases)에서 확인하세요.

## 설치 위치와 버전

설치 위치를 바꾸려면 스크립트를 실행하는 셸에 `INSTALL_DIR`을 전달합니다. 쓰기 가능한 디렉터리여야 하며 기본 사용자 디렉터리는 `sudo`가 필요하지 않습니다.

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | INSTALL_DIR=/usr/local/bin sh
```

릴리스 태그를 지정하려면:

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh -s -- --version codemap-v0.1.6
```

`codemap-v0.1.6`은 버전 지정 예시입니다. 필요한 파일이 있는 태그를 사용하세요.

| 옵션 | 의미 |
|---|---|
| `INSTALL_DIR` | 설치 디렉터리. 기본값 `$HOME/.local/bin` |
| `--version` / `CODEMAP_VERSION` | 정확한 릴리스 태그. `--version`이 우선 |
| `CODEMAP_LINUX_LIBC` | 기본값 `musl`. Linux x86_64에서만 `gnu` 선택 가능 |
| `--print-target` | 다운로드 없이 대상·다운로드 URL·설치 경로 표시 |

태그를 생략하면 `releases/latest/download/<asset>`를 사용합니다. 저장소의 최신 릴리스를 가리키므로 다른 제품 릴리스일 수 있습니다. 같은 버전으로 반복 설치하려면 태그를 지정하세요. 설치용 변수는 실행 중인 MCP 서버를 설정하지 않습니다.

## 지원 환경

| 운영체제 | 아키텍처 | 설치 파일 |
|---|---|---|
| macOS | arm64 / aarch64 | `codemap-search-aarch64-apple-darwin.tar.gz` |
| macOS | x86_64 / amd64 | `codemap-search-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64 / amd64, 기본 musl | `codemap-search-x86_64-unknown-linux-musl.tar.gz` |
| Linux | x86_64 / amd64, `CODEMAP_LINUX_LIBC=gnu` | `codemap-search-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | arm64 / aarch64, musl만 | `codemap-search-aarch64-unknown-linux-musl.tar.gz` |

musl 빌드는 glibc에 의존하지 않습니다. x86_64 GNU 빌드를 선택하려면 `INSTALL_DIR`과 같은 방식으로 `sh`에 `CODEMAP_LINUX_LIBC=gnu`를 전달하세요. Linux arm64의 `gnu`, 지원하지 않는 아키텍처·운영체제는 오류로 거부합니다.

## 문제 해결

- 다운로드 오류: 릴리스 태그, 압축 파일 이름, 체크섬 파일, 네트워크 연결을 확인하세요. 파일이 없으면 설치를 중단합니다.
- 체크섬 불일치: 압축 해제 전에 중단하며 설치 디렉터리도 만들지 않습니다. 다시 내려받아 릴리스 체크섬과 비교하고 검증을 생략하지 마세요.
- 쓰기 오류: 쓰기 가능한 `INSTALL_DIR`을 지정하세요.
- 명령을 찾지 못함: 터미널뿐 아니라 클라이언트의 `PATH`도 확인하세요.

## 배포 담당자 안내

스크립트는 [install.sh](../../install.sh)에 있습니다. 공개 릴리스 파일을 사용하므로 API 토큰이 필요하지 않습니다. 설치 파일 이름과 체크섬은 [릴리스 워크플로](../../../../.github/workflows/codemap-search-release.yml)와 일치해야 합니다. `.sha256` 파일 형식은 `<hash>  <basename>`이며 대상별로 압축 파일과 체크섬을 함께 게시합니다.

소스 빌드와 다른 패키지 관리자는 [설치 채널 개요](./index.ko.md)를 참고하세요.
