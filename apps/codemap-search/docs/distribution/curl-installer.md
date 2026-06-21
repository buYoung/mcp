# curl installer (`install.sh`) 가이드

macOS + Linux용 POSIX `sh` 설치 스크립트다. OS/arch를 감지해 매칭되는 GitHub Release 자산을 받고,
**추출 이전에 `.sha256`를 검증**한 뒤 `~/.local/bin`에 설치한다. Windows는 다루지 않는다(WinGet 사용).

## 현재 상태 (확인됨)

- **확인됨(동작함)**: `apps/codemap-search/install.sh` 작성 완료. POSIX `sh`(`set -eu`, no bashisms),
  실행 가능. 구문(`sh -n`/`dash -n`)·OS/arch 매핑·체크섬 검증·오프라인 전체 흐름이 로컬 green
  (`file://` 픽스처).
- **첫 태그 이후 확정**: 라이브 GitHub 다운로드 **성공 경로**만 보류다. 실태그가 없는 지금, 실제
  `releases/latest/download/...` URL은 404를 반환하며 `curl -f`가 이를 잡아 non-zero로 실패하고 설치
  디렉터리를 생성조차 하지 않음(부분 설치 0)을 라이브로 확인했다. 즉 로드베어링 `-f` 동작·URL 형식·
  미존재 자산의 무-부분설치는 확인됨이고, 성공 경로만 첫 `codemap-v0.1.0` 자산 게시 이후 확정된다.

## 스크립트 위치

```
apps/codemap-search/install.sh
```

## 동작

### OS/arch → 자산 매핑

| OS (`uname -s`) | arch (`uname -m`) | 타깃 트리플 | 자산명 |
|---|---|---|---|
| Darwin | aarch64/arm64 | `aarch64-apple-darwin` | `codemap-search-aarch64-apple-darwin.tar.gz` |
| Darwin | x86_64/amd64 | `x86_64-apple-darwin` | `codemap-search-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64/amd64 | `x86_64-unknown-linux-musl` (기본, musl 선호) | `codemap-search-x86_64-unknown-linux-musl.tar.gz` |
| Linux | aarch64/arm64 | `aarch64-unknown-linux-musl` (musl만) | `codemap-search-aarch64-unknown-linux-musl.tar.gz` |

- arm 정규화: `arm64`/`aarch64` → `aarch64`, `x86_64`/`amd64` → `x86_64`.
- **Linux는 musl 정적 빌드를 기본 선호**(glibc 불필요). x86_64에서 `CODEMAP_LINUX_LIBC=gnu`를
  명시할 때만 `x86_64-unknown-linux-gnu`로 분기한다.
- **arm64 Linux + gnu 요청은 명시적 에러**: `aarch64-unknown-linux-gnu` 자산이 없으므로 404 URL을
  조용히 내보내지 않고 non-zero로 거부한다.
- 미지원 OS(Windows/MINGW 등)·미지원 arch(i686 등)는 명확한 에러 + non-zero.

### 설치 경로 + PATH

- 기본: `${INSTALL_DIR:-$HOME/.local/bin}` — `~/.local/bin`. sudo 불필요(없으면 `mkdir -p`).
- override: `INSTALL_DIR` 환경변수 (예: `INSTALL_DIR=/usr/local/bin`). 해당 디렉터리가 필요로 할 때만
  sudo.
- 설치 디렉터리가 `PATH`에 없으면 현재 세션용 `export PATH="<dir>:$PATH"` 안내를 stderr로 출력한다.
  영속화하려면 그 줄을 셸 프로파일(zsh는 `~/.zshrc`, bash는 `~/.bashrc`)에 추가한 뒤 셸을 재시작한다.

### sha256 검증 (추출 이전)

`download archive` → `download .sha256` → `verify_sha256` → (검증 성공 시에만) `tar -xzf`.
`.sha256`(`"<hash>  <basename>"` 포맷)에서 기대 해시를 추출하고, 로컬에서 `sha256sum` 또는
`shasum -a 256`으로 계산해 비교한다. 불일치 시 에러 + non-zero로 중단하며 **아무것도 설치하지 않는다**
(설치 디렉터리도 생성 안 함).

### 버전 핀

- 기본: `releases/latest/download/<asset>` (latest).
- 핀: `--version codemap-vX.Y.Z` 또는 `CODEMAP_VERSION` 환경변수 →
  `releases/download/codemap-vX.Y.Z/<asset>`.

> latest는 저장소 전역 redirect다. 현재 릴리스 워크플로는 `codemap-search-release.yml` 하나뿐이라
> 안전하지만, 향후 다른 제품이 릴리스를 cut하면 latest가 codemap에서 벗어날 수 있다. 재현 가능한
> 설치는 `--version`으로 태그를 핀한다.

## 메인테이너 준비

- 별도 토큰/시크릿 없음. 스크립트는 공개 GitHub Release 자산만 소비한다.
- 자산명·`.sha256` 포맷은 릴리스 워크플로(`codemap-search-release.yml`)가 생성하는 규칙과 일치해야
  한다 — 현재 일치 확인됨.

## 엔드유저 설치

기본(최신):

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
```

설치 디렉터리 변경:

```sh
INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
```

버전 핀:

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh -s -- --version codemap-v0.1.0
```

필요 도구: `curl`(또는 `wget`), `tar`, `sha256sum`/`shasum`. 추가 런타임 없음. macOS + Linux 전용.

## 검증 천장 (정직 표기)

- 로컬에서 확정 가능: 구문, OS/arch 매핑(4조합 + gnu opt-in + arm64+gnu 거부 + 미지원 거부),
  체크섬 검증(일치 통과 / 변조 중단), 오프라인 전체 흐름(`file://` 픽스처), 트랩 정리.
- 첫 태그 이후에만 확정: 라이브 다운로드 성공 경로(전송 계층). 코드 경로는 file:// 등가 픽스처로
  이미 green.
