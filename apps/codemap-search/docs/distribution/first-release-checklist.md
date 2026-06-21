# codemap-search 첫 릴리스 체크리스트 (`codemap-v0.1.0`)

메인테이너가 직접 수행해야 하는 항목을 순서·의존관계대로 정리한 런북이다. 각 채널의 상세 절차는 같은 디렉터리의 채널별 가이드(`crates-io.md`, `winget.md`, `homebrew.md`, `curl-installer.md`, `index.md`)를 참고한다.

> **핵심 원칙**: 첫 `codemap-v0.1.0` 태그가 모든 채널의 **공통 해제 조건(shared unlock)** 이다. 태그가 7개 자산 + 형제 `.sha256`를 만들고, crates.io 실제 게시를 트리거하며, Windows·arm64 빌드의 첫 실제 CI 검증이 된다. 그래서 "태그 전 준비 → 태그 → 태그 후 채우기" 순서가 강제된다.

> **표기**: ⚠️ = **비가역**(태그/crates.io 게시) 또는 **외부 통제 밖**(winget/homebrew 머지).

---

## Phase 0 — 코드 반영 (지금 바로)

- [ ] **브랜치를 `main`에 반영** (푸시 + PR 또는 머지).
  - `install.sh` 원라이너와 가이드가 `raw.githubusercontent.com/buYoung/mcp/main/...` 와 `releases/download/...` 를 가리키므로 코드가 **`main`에 있어야** curl 설치 경로가 동작한다.

```bash
git push -u origin feat/codemap-search-dist-channels
gh pr create --fill
# 또는: git switch main && git merge feat/codemap-search-dist-channels
```

---

## Phase 1 — crates.io 사전 준비 (태그 전 필수)

- [ ] **crates.io API 토큰 발급**: <https://crates.io> 로그인(GitHub OAuth) → *Account Settings → API Tokens → New Token*. scope에 **publish-new**(첫 게시) + publish-update 포함, crate를 `codemap-search`로 제한.
- [ ] **GitHub Secret 등록**: repo *Settings → Secrets and variables → Actions → New repository secret*.
  - Name: `CARGO_REGISTRY_TOKEN` / Value: 위 토큰.
- [ ] **버전 일치 확인**: `apps/codemap-search/Cargo.toml`의 `version`이 `0.1.0`인지(푸시할 태그 `codemap-v0.1.0`과 일치해야 함).

상세: `crates-io.md`

---

## Phase 2 — 첫 릴리스 태그 (공통 해제 조건) ⚠️ 비가역

- [ ] **`main` 최신 커밋에서 태그 푸시**:

```bash
git switch main && git pull
git tag codemap-v0.1.0
git push origin codemap-v0.1.0      # 이 태그만 릴리스를 트리거함 (PR/브랜치 push는 릴리스 안 함)
```

- [ ] **GitHub Actions 워크플로 green 확인** (*Actions* 탭): `release` 매트릭스 7개 타깃 + `publish-crate` 잡.
  - ⚠️ **Windows x86_64/arm64 빌드의 첫 실제 검증 지점**이다(셸 버그를 수정했으나 태그가 0개라 실 CI 미실행 상태였다). 빨간불이면 Windows leg부터 확인.
- [ ] **GitHub Release 자산 확인**: 아카이브 7개 + `.sha256` 7개 업로드.

```text
codemap-search-x86_64-unknown-linux-gnu.tar.gz(.sha256)
codemap-search-x86_64-unknown-linux-musl.tar.gz(.sha256)
codemap-search-aarch64-unknown-linux-musl.tar.gz(.sha256)
codemap-search-aarch64-apple-darwin.tar.gz(.sha256)
codemap-search-x86_64-apple-darwin.tar.gz(.sha256)
codemap-search-x86_64-pc-windows-msvc.zip(.sha256)
codemap-search-aarch64-pc-windows-msvc.zip(.sha256)
```

- [ ] **crates.io 게시 확인**: <https://crates.io/crates/codemap-search> 에 `0.1.0` 노출.
  - ⚠️ **`0.1.0`은 영구 고정** — 게시 후 동일 버전 재게시 불가(수정하려면 `0.1.1`로 올려야 함).

---

## Phase 3 — placeholder sha256 실값 교체 (태그 직후 필수)

태그가 만든 실제 `.sha256` 값으로 WinGet/Homebrew의 `0000...0000`(64-zeros)을 교체한다.

- [ ] **체크섬 확보**:

```bash
gh release download codemap-v0.1.0 --repo buYoung/mcp --pattern '*.sha256' --dir /tmp/cm-sha
cat /tmp/cm-sha/*.sha256
```

- [ ] **WinGet** `apps/codemap-search/packaging/winget/com.livteam.codemap-search.installer.yaml`:
  - `Architecture: x64` → `codemap-search-x86_64-pc-windows-msvc.zip.sha256` 값
  - `Architecture: arm64` → `codemap-search-aarch64-pc-windows-msvc.zip.sha256` 값
- [ ] **Homebrew** `apps/codemap-search/packaging/homebrew/codemap-search.rb`:
  - `on_arm` → `codemap-search-aarch64-apple-darwin.tar.gz.sha256` 값
  - `on_intel` → `codemap-search-x86_64-apple-darwin.tar.gz.sha256` 값
- [ ] **교체분 커밋** (예: `chore(codemap-search): pin 0.1.0 release checksums`).

---

## Phase 4 — WinGet 제출 (Windows 호스트 필요)

- [ ] **Windows에서 검증** (real sha256 채운 뒤):

```powershell
winget validate apps\codemap-search\packaging\winget
```

- [ ] **`microsoft/winget-pkgs` 제출**: `wingetcreate submit` 또는 수동 PR. `PackageIdentifier = com.livteam.codemap-search`.
  - ⚠️ **머지는 Microsoft 검토(외부, 통제 밖)**. 제출=완료, 머지=대기.

상세: `winget.md`

---

## Phase 5 — Homebrew 제출 (macOS, Option B)

- [ ] **typed sigil flip**: `codemap-search.rb` 1행 `# typed: strict` → `# typed: false`.
  - (homebrew-core의 rubocop은 `Sorbet/StrictSigil`을 끄므로 그쪽 관례. 로컬 `brew style` green 유지를 위해 지금까진 `strict`로 둠.)
- [ ] **온라인 감사** (real sha256 + 실 URL이라 이제 통과 가능):

```bash
brew style apps/codemap-search/packaging/homebrew/codemap-search.rb
brew audit --strict --new --online apps/codemap-search/packaging/homebrew/codemap-search.rb
```

- [ ] ⚠️ **binary-vs-source 인지**: homebrew-core는 소스 빌드를 선호 → 사전빌드 바이너리 포뮬러가 지적/반려될 수 있음. 그 경우 소스 빌드 포뮬러(`depends_on "rust" => :build` + `cargo install`)로 전환 검토.
- [ ] **`homebrew/homebrew-core` PR 제출**.
  - ⚠️ **인지도 심사(외부)로 홀드/반려 가능**(Option B 수용). 그동안 macOS는 `install.sh` / `cargo install` / Release 직접 다운로드가 폴백.

상세: `homebrew.md`

---

## Phase 6 — 설치 경로 스모크 테스트 (머지/게시 후)

- [ ] `cargo install codemap-search` (crates.io, Linux 권장 경로)
- [ ] `curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh` (macOS/Linux)
- [ ] `winget install com.livteam.codemap-search` (winget-pkgs 머지 후)
- [ ] `brew install codemap-search` (homebrew-core 수용 후)
- [ ] 각각 `codemap-search --version`이 `0.1.0`을 출력하는지.

---

## 선택 / 추후작업 (지금 안 해도 됨)

- [ ] **Snap / AUR** Linux 채널 추가 — `release-distribution-strategy.md`에 *추후작업(future work)* 으로 문서화됨.
- [ ] **WinGet/Homebrew auto-bump** 워크플로 스텝(`wingetcreate update` / `brew bump-formula-pr`) — 이번엔 "manual first"로 결정해 미구현. 버전 올릴 때 자동화하려면 추가.

---

## 한눈에 보는 의존 순서

```text
Phase 0 (머지)
  → Phase 1 (토큰/시크릿/버전)
    → Phase 2 (태그 = 해제) ⚠️ 비가역
      → Phase 3 (sha256 실값 채우기)
        → Phase 4 (WinGet 제출) ─┐  외부 머지 대기 ⚠️
        → Phase 5 (Homebrew 제출) ┘  (병렬 가능)
          → Phase 6 (스모크 테스트)
```

가장 주의할 것: `codemap-v0.1.0` 태그는 crates.io `0.1.0`을 **영구 고정**한다. 태그 전 Phase 1(토큰/버전 확인)을 반드시 마칠 것.
