# WinGet 배포 가이드

Windows Package Manager(WinGet)로 `codemap-search`를 배포하는 채널이다. `microsoft/winget-pkgs`
저장소에 멀티파일 매니페스트를 제출하고 Microsoft 검토를 거쳐 머지되면 `winget install`로 설치된다.

## 현재 상태 (확인됨)

- **확인됨 / 제출 가능(valid + submittable)**: `com.livteam.codemap-search` 멀티파일 매니페스트 3종이
  스키마 1.12.0 구조 정합·YAML well-formed. **아직 `microsoft/winget-pkgs`에 제출하지 않았다**
  (submitted/merged 아님).
- **PackageIdentifier**: `com.livteam.codemap-search`
- **PackageVersion**: `0.1.0`
- **ManifestVersion**: `1.12.0`
- **첫 태그 이후 확정**: `InstallerSha256`는 placeholder(64-zeros)이므로 첫 `codemap-v0.1.0` 릴리스가
  자산을 게시한 뒤 실값으로 교체해야 한다. 실제 availability는 Microsoft 검토 머지 이후다.

## 매니페스트 위치

```
apps/codemap-search/packaging/winget/
├── com.livteam.codemap-search.yaml                  # version 매니페스트
├── com.livteam.codemap-search.locale.en-US.yaml     # defaultLocale 매니페스트
└── com.livteam.codemap-search.installer.yaml        # installer 매니페스트 (x64 + arm64)
```

- installer 매니페스트는 `InstallerType: zip` + `NestedInstallerType: portable`로, `.zip` 루트의
  `codemap-search.exe`를 PATH alias `codemap-search`로 호이스팅한다.
- x64(`codemap-search-x86_64-pc-windows-msvc.zip`)와 arm64(`codemap-search-aarch64-pc-windows-msvc.zip`)
  두 인스톨러를 모두 커버한다.
- **arm64는 build-only**: x64 러너에서 크로스 빌드되며 arm64 하드웨어 런타임은 미검증이다.

## 메인테이너 사전 준비

- **GitHub 계정**: `microsoft/winget-pkgs`를 fork하고 PR을 올릴 수 있어야 한다.
- (선택) Windows 호스트의 `winget`/`wingetcreate`: 로컬 검증·제출 자동화에 사용. macOS/Linux에는
  없으므로 매니페스트 검증(`winget validate`)은 Windows에서만 가능하다.
- 별도 토큰/시크릿은 필요 없다(WinGet 제출은 GitHub PR 기반).

## sha256 placeholder → 첫 릴리스 후 채우기

매니페스트의 `InstallerSha256`는 현재 64개의 `0`(placeholder)이다. 텍스트 sentinel은 스키마 정규식
`^[A-Fa-f0-9]{64}$`를 통과하지 못하므로 64-zeros를 사용한다. 첫 `codemap-v0.1.0` 태그가 자산과 형제
`.sha256`를 만든 뒤 실값으로 교체한다:

- x64 → `codemap-search-x86_64-pc-windows-msvc.zip.sha256`의 값(소문자 hex)을 복사.
- arm64 → `codemap-search-aarch64-pc-windows-msvc.zip.sha256`의 값을 복사.

(두 `.sha256`는 워크플로 Windows 패키지 스텝의 `Get-FileHash … .ToLower()`(utf8NoBOM)로 생성된다.)

> `winget validate`는 스키마 전용이라 자산을 다운로드/해시하지 않으므로 placeholder로도 통과한다.
> 실자산 해시 대조는 `winget install`(다운로드 + 해시 검증) 시점에 일어나며, placeholder인 동안에는
> 설치가 실패한다. 따라서 placeholder 교체는 제출 전 필수다.

## 제출 런북

1. 첫 `codemap-v0.1.0` 릴리스가 자산을 게시할 때까지 기다린다(sha256 실값 필요).
2. 두 `.sha256` 자산에서 실값을 복사해 installer 매니페스트의 placeholder를 교체한다.
3. (Windows 호스트) 매니페스트를 검증한다:
   ```powershell
   winget validate apps/codemap-search/packaging/winget
   ```
4. `microsoft/winget-pkgs`를 fork하고 매니페스트를 `manifests/c/com/livteam/codemap-search/0.1.0/`
   경로에 배치해 PR을 올린다. (또는 `wingetcreate submit`.)
5. Microsoft의 자동 검증 + 사람 검토를 거쳐 머지되면 `winget install`로 설치 가능해진다.

> 실제 `microsoft/winget-pkgs` PR 제출은 사용자가 직접 한다(외부/비가역 행위).

## 엔드유저 설치

머지 후:

```powershell
winget install com.livteam.codemap-search
```

머지 전(로컬 매니페스트):

```powershell
winget install --manifest apps/codemap-search/packaging/winget
```

두 가지 조건이 있다: (a) 첫 `codemap-v0.1.0` 릴리스가 자산을 게시하고 매니페스트의 placeholder
`sha256`를 실값으로 교체한 **이후에만** 성공한다(그 전에는 다운로드·해시 검증이 실패한다), (b) 경로가
상대경로이므로 **리포 루트에서 실행**한다. `codemap-search`가 `PATH`에 등록된다(portable alias). arm64
빌드는 build-only로 제공된다.

## 검증 천장 (정직 표기)

- 로컬에서 확정 가능: YAML well-formed, 스키마 1.12.0 필수 키·정규식·enum 구조 정합, 크로스파일 키 일치.
- 첫 태그 이후에만 확정: 실 sha256 교체, `winget install` 다운로드·해시 검증, arm64 런타임 실행.
- 외부 의존: `microsoft/winget-pkgs` 머지(Microsoft 검토).
