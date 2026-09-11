# Homebrew 설치

한국어 | [English](./homebrew.md)

저장소에는 Apple Silicon·Intel Mac용 포뮬러가 있으며 사전 빌드 릴리스 파일을 설치합니다. Linux에서는 [Cargo](./crates-io.ko.md)나 [설치 스크립트](./curl-installer.ko.md)를 사용하세요.

## 제공 상태와 설치

2026-09-11 문서 검토에서 homebrew-core의 공개 제공 여부를 확인하지 못했습니다. [공식 포뮬러 목록](https://formulae.brew.sh/formula/codemap-search)을 확인하세요. 포뮬러가 제공되면 Homebrew로 설치합니다.

```sh
brew install codemap-search
```

설치 위치는 Homebrew가 관리합니다. `codemap-search --version`으로 확인하고 클라이언트의 `PATH`에 Homebrew 실행 파일 디렉터리가 있는지 확인하세요. 포뮬러가 없으면 Cargo, 설치 스크립트 또는 [GitHub Releases](https://github.com/buYoung/mcp/releases)의 해당 환경용 압축 파일을 사용합니다. 정확한 태그나 별도 설치 위치가 필요하면 설치 스크립트를 사용하세요.

로컬 [포뮬러](../../packaging/homebrew/codemap-search.rb)는 아직 버전 `0.1.0`과 0으로 채운 임시 체크섬을 사용하므로 현재 파일 그대로는 설치 준비가 되지 않았습니다. 이것만으로 별도의 공개 포뮬러가 수용되었는지는 판단할 수 없습니다.

## 배포 담당자 제출 절차

1. 릴리스를 선택하고 포뮬러의 버전·URL·두 체크섬을 해당 `.sha256` 파일에 맞게 갱신합니다.
2. 적절한 tap 환경에서 Homebrew로 검증합니다. 포뮬러의 검증은 `--version`과 `tokenize`를 실행합니다.
3. `homebrew/homebrew-core`의 `Formula/c/codemap-search.rb`로 제출하고 검토를 완료한 뒤 `brew install` 제공을 안내합니다.

설치 파일은 `codemap-search-aarch64-apple-darwin.tar.gz`와 `codemap-search-x86_64-apple-darwin.tar.gz`입니다. 이 저장소에는 Linux 포뮬러나 별도 tap이 없습니다. 로컬 포뮬러 검사만으로 공개 수용 여부나 다운로드 URL·체크섬의 실제 동작을 확인할 수는 없습니다.

다른 방법은 [설치 채널 개요](./index.ko.md)를 참고하세요.
