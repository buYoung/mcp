# WinGet 설치

한국어 | [English](./winget.md)

Windows Package Manager(WinGet) 또는 릴리스 압축 파일로 설치합니다. 패키지 식별자는 `com.livteam.codemap-search`입니다.

## 제공 상태와 설치

[공식 WinGet 저장소](https://github.com/microsoft/winget-pkgs/tree/master/manifests/c/com/livteam/codemap-search)에서 제공 여부를 확인하세요. 패키지가 등록되어 있다면 다음 명령으로 설치합니다.

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

저장소의 [로컬 매니페스트](../../packaging/winget/)에는 임시 체크섬이 있어 그대로 설치할 수 없습니다. 갱신·검증·제출 방법은 [배포 담당자 안내](./releasing.ko.md#winget)를 따릅니다.

다운로드 오류는 선택한 릴리스의 파일을 확인하고, 해시 오류는 검증을 우회하지 말고 해당 `.sha256`와 대조하세요. 다른 방법은 [설치 채널 개요](./index.ko.md)를 참고하세요.
