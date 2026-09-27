# 배포 담당자 안내

한국어 | [English](./releasing.md)

codemap-search 릴리스를 준비하고 게시 결과를 확인하는 절차입니다. 사용자 설치는 [설치 채널](./index.ko.md)을 참고하세요. 빌드·게시 작업의 기준은 [릴리스 워크플로](../../../../.github/workflows/codemap-search-release.yml)이며, 저장소의 매니페스트나 Git 태그만으로 공개 제공을 판단하지 않습니다.

## 1. 릴리스 준비

게시할 커밋을 선택한 뒤 모노레포 루트에서 확인합니다.

- `apps/codemap-search/Cargo.toml` 버전이 게시할 `codemap-vX.Y.Z` 태그와 같고, 해당 태그·버전이 이미 사용 중이지 않아야 합니다.
- 릴리스 커밋에 게시할 소스·lockfile·문서·설치기가 포함되어 있어야 합니다.
- GitHub Actions 실행기와 필요한 시크릿을 준비합니다. crates.io 게시는 `CARGO_REGISTRY_TOKEN`을 사용하며, macOS 작업에는 워크플로에 설정된 서명·공증 환경도 필요합니다. 인증정보는 파일이나 명령 인수에 넣지 않습니다.

업로드 없이 패키징·컴파일을 검증합니다.

```sh
cargo publish --dry-run --manifest-path apps/codemap-search/Cargo.toml
```

이 명령의 성공은 인증정보·실제 업로드·다른 플랫폼 실행·패키지 관리자 수용 여부를 확인한 결과가 아닙니다.

## 2. 게시 시작과 두 결과 확인

**릴리스 태그를 푸시하면 외부 게시를 시작합니다. 게시한 crate 버전은 다른 파일로 덮어쓸 수 없습니다.** 게시할 커밋과 버전을 승인한 뒤 사용 중인 릴리스 절차로 `codemap-vX.Y.Z` 태그를 푸시합니다. 과거 첫 릴리스 태그나 개발 브랜치를 그대로 재사용하지 않습니다.

브랜치 push나 pull request는 이 게시 워크플로를 실행하지 않습니다.

1. `build`가 대상별 압축 파일과 `.sha256`를 만듭니다.
2. `publish-release`와 `publish-crate`는 각각 `build`를 기다린 뒤 독립적으로 게시합니다.
3. 두 작업 결과를 따로 확인합니다. GitHub 릴리스 성공이 crates.io 성공을 뜻하지 않으며 반대도 같습니다.

Linux·macOS 빌드 실패는 게시를 막습니다. Windows 작업은 실패해도 다른 대상의 게시를 허용하므로 파일이 빠질 수 있습니다. 크로스 빌드만으로 해당 하드웨어 실행을 확인한 것은 아닙니다. 제공을 안내할 대상마다 [압축 파일 목록](./index.ko.md#릴리스-파일-이름)과 실제 게시 파일을 대조하세요.

`publish-crate`는 매니페스트의 정확한 버전이 이미 게시되어 있으면 건너뜁니다. 업로드 실패 후에도 다시 조회해 실제 게시 완료와 실패를 구분합니다. 실패한 릴리스를 재시도하기 전에 레지스트리와 릴리스 파일을 확인하고, 어느 쪽에도 변경이 없었다고 가정하지 마세요.

## 3. 압축 파일과 설치기 확인

제공을 안내하는 모든 대상에 대해 선택한 태그에 압축 파일과 체크섬이 함께 있는지 확인합니다. `.sha256` 파일은 `<hash>  <basename>` 형식이며 내려받은 압축 파일과 일치해야 합니다.

[설치기](../../install.sh)는 API 토큰 없이 공개 릴리스 파일을 읽습니다. 워크플로와 파일 이름을 맞추고 [태그를 지정한 설치](./curl-installer.ko.md#설치-위치와-버전)를 확인하세요. 태그를 생략하면 저장소의 최신 릴리스가 다른 제품일 수도 있습니다.

실제로 검증한 각 플랫폼에서 `codemap-search --version`과 대상 저장소의 MCP 연결을 확인합니다. 빌드만 하고 실행하지 않은 플랫폼은 별도로 기록합니다. Linux 검증 범위는 [Docker 안내](../../docker/README.ko.md)를 참고하세요.

## 4. 패키지 관리자 제출 자료 갱신

### Homebrew

[로컬 macOS 포뮬러](../../packaging/homebrew/codemap-search.rb)는 버전 `0.1.0`과 0으로 채운 체크섬을 사용하므로 현재 파일 그대로는 설치할 수 없습니다.

1. 게시할 버전과 두 Darwin 압축 파일의 URL·체크섬을 실제 릴리스에 맞춥니다.
2. 적절한 Homebrew tap 환경에서 포뮬러를 검증합니다. 내장 검사는 `--version`과 `tokenize`를 실행하며, 실제 다운로드와 체크섬도 확인합니다.
3. `homebrew/homebrew-core`의 `Formula/c/codemap-search.rb`로 제출하고 검토에 대응합니다. 현재 포뮬러는 사전 빌드 파일을 사용하며 이 배포 방식의 수용을 보장하지 않습니다.
4. 공개 수용을 확인한 뒤 `brew install codemap-search` 제공을 안내합니다.

이 저장소에는 Linux 포뮬러나 별도 tap이 없습니다. 로컬 포뮬러 검사만으로 공개 수용 여부를 확인할 수는 없습니다.

### WinGet

[매니페스트 디렉터리](../../packaging/winget/)에는 버전·기본 언어·설치 매니페스트가 있으며 현재 버전은 `0.1.0`, 스키마는 `1.12.0`, 설치 체크섬은 0으로 채운 임시값입니다.

1. 세 매니페스트의 버전과 x64·arm64 URL·실제 릴리스 체크섬을 갱신합니다. 식별자는 `com.livteam.codemap-search`를 유지합니다.
2. Windows의 모노레포 루트에서 검증합니다.

```powershell
winget validate apps/codemap-search/packaging/winget
```

3. WinGet 로컬 매니페스트 설치를 활성화한 환경에서 설치와 CLI 시작을 확인합니다.

```powershell
winget install --manifest apps/codemap-search/packaging/winget
codemap-search --version
```

4. `microsoft/winget-pkgs`의 `manifests/c/com/livteam/codemap-search/<version>/`에 제출하고 수용 여부를 확인한 뒤 공개 설치를 안내합니다.

스키마 검사만으로 압축 파일을 내려받거나 체크섬을 확인하지는 않습니다. 해시 오류를 우회하지 마세요.

## 완료 확인

릴리스 커밋·태그, 두 게시 작업의 결과, 실제 압축 파일·체크섬, 확인한 설치 경로, 패키지 관리자 수용 여부를 각각 기록합니다. 없는 대상이나 제출 대기는 그대로 보고하며 다른 채널의 성공으로 대신하지 않습니다.

[초기 배포 채널 설계](../release-distribution-strategy.md)와 [첫 릴리스 체크리스트](./first-release-checklist.md)는 과거 기록이며 현재 배포 명령으로 사용하지 않습니다.
