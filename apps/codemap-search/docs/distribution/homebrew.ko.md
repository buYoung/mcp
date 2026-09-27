# Homebrew 설치

한국어 | [English](./homebrew.md)

저장소에는 Apple Silicon·Intel Mac용 포뮬러가 있으며 사전 빌드 릴리스 파일을 설치합니다. Linux에서는 [Cargo](./crates-io.ko.md)나 [설치 스크립트](./curl-installer.ko.md)를 사용하세요.

## 제공 상태와 설치

[공식 포뮬러 목록](https://formulae.brew.sh/formula/codemap-search)에서 제공 여부를 확인하세요. 포뮬러가 등록되어 있다면 Homebrew로 설치합니다.

```sh
brew install codemap-search
```

설치 위치는 Homebrew가 관리합니다. `codemap-search --version`으로 확인하고 클라이언트의 `PATH`에 Homebrew 실행 파일 디렉터리가 있는지 확인하세요. 포뮬러가 없으면 Cargo, 설치 스크립트 또는 [GitHub Releases](https://github.com/buYoung/mcp/releases)의 해당 환경용 압축 파일을 사용합니다. 정확한 태그나 별도 설치 위치가 필요하면 설치 스크립트를 사용하세요.

로컬 [포뮬러](../../packaging/homebrew/codemap-search.rb)는 아직 버전 `0.1.0`과 0으로 채운 임시 체크섬을 사용하므로 현재 파일 그대로는 설치 준비가 되지 않았습니다. 이것만으로 별도의 공개 포뮬러가 수용되었는지는 판단할 수 없습니다.

다른 방법은 [설치 채널 개요](./index.ko.md), 포뮬러 갱신·검증·제출은 [배포 담당자 안내](./releasing.ko.md#homebrew)를 참고하세요.
