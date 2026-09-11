# WinGet 설치

한국어 | [English](./winget.md)

Windows Package Manager(WinGet) 또는 릴리스 압축 파일로 설치합니다. 패키지 식별자는 `com.livteam.codemap-search`입니다.

## 제공 상태와 설치

2026-09-11 문서 검토에서 공개 제공 여부를 확인하지 못했습니다. [공식 WinGet 저장소](https://github.com/microsoft/winget-pkgs/tree/master/manifests/c/com/livteam/codemap-search)를 확인하세요. 패키지가 제공되면 다음 명령으로 설치합니다.

```powershell
winget install com.livteam.codemap-search
```

설치 위치는 WinGet이 관리하며 `codemap-search` 명령 별칭을 등록합니다. 새 터미널에서 `codemap-search --version`을 실행하고 MCP 클라이언트도 `PATH`에서 명령을 찾을 수 있는지 확인하세요.

패키지가 없으면 [GitHub Releases](https://github.com/buYoung/mcp/releases)에서 태그를 선택하고 아키텍처에 맞는 압축 파일·체크섬을 받습니다. 체크섬을 확인한 뒤 `codemap-search.exe`를 추출해 `PATH`에 있는 디렉터리에 넣으세요. 릴리스 태그로 버전을 선택합니다.

| 아키텍처 | 설치 파일 |
|---|---|
| x64 | `codemap-search-x86_64-pc-windows-msvc.zip` |
| arm64 | `codemap-search-aarch64-pc-windows-msvc.zip` |

릴리스 워크플로는 Windows 빌드가 실패해도 다른 대상의 게시를 막지 않으며 해당 파일만 빠질 수 있습니다. 선택한 릴리스에 필요한 압축 파일이 있는지 확인하세요. arm64는 x64 실행기에서 크로스 빌드하며 이 워크플로에서 arm64 하드웨어로 실행하지 않습니다. [설치 스크립트](./curl-installer.ko.md)는 macOS와 Linux 전용입니다.

## 로컬 매니페스트로 설치

[매니페스트 디렉터리](../../packaging/winget/)에는 x64·arm64용 버전·기본 언어·설치 매니페스트가 있습니다. 현재 파일의 버전은 `0.1.0`, 스키마는 `1.12.0`이며 `InstallerSha256`는 0으로 채운 임시값입니다. 설치 전 제공되는 릴리스에 맞게 URL·버전·체크섬을 갱신해야 합니다.

Windows에서 WinGet의 로컬 매니페스트 설치를 활성화한 뒤 모노레포 루트에서 실행합니다.

```powershell
winget install --manifest apps/codemap-search/packaging/winget
```

임시 체크섬으로는 설치에 실패합니다. 스키마 검사만으로 압축 파일을 다운로드하거나 해시를 확인하지는 않습니다. 다운로드 오류는 릴리스 파일을, 해시 오류는 매니페스트와 릴리스의 `.sha256` 값을 확인하세요.

## 배포 담당자 제출 절차

1. 세 매니페스트의 버전을 맞추고 설치 매니페스트에 실제 체크섬을 넣습니다.
2. Windows의 모노레포 루트에서 검증합니다.

```powershell
   winget validate apps/codemap-search/packaging/winget
   ```

3. 로컬 매니페스트로 설치와 CLI 시작을 확인합니다.
4. `microsoft/winget-pkgs`에 해당 버전의 매니페스트를 제출합니다. 기존 `manifests/c/com/livteam/codemap-search/0.1.0/` 구조에서 버전 부분을 게시할 값으로 맞춥니다.
5. 수용을 확인한 뒤 공개 WinGet 설치를 안내합니다.

다른 방법은 [설치 채널 개요](./index.ko.md)를 참고하세요.
