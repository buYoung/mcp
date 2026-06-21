# Homebrew 배포 가이드

macOS용 Homebrew 채널이다. `homebrew/homebrew-core`에 포뮬러를 제출하고(Option B — 자체 tap 없음)
수용되면 `brew install codemap-search`로 설치된다. **macOS 전용**이며 Linux/linuxbrew 포뮬러는 없다.

## 현재 상태 (확인됨)

- **확인됨 / 제출 가능(valid + submittable)**: 포뮬러가 `brew style` exit 0(진짜 green) + 오프라인
  `FormulaAudit` cop(desc, homepage, license, components order, test) 통과. **아직
  `homebrew/homebrew-core`에 제출하지 않았다**(submitted/merged 아님).
- **Option B**: 공식 homebrew-core 경로만 사용. 자체 tap 없음.
- **macOS 전용**: `on_macos` 블록 안에 `on_arm`/`on_intel`만 둔다. Linux 설치는 `cargo install` /
  `install.sh` / Release 직접 다운로드를 쓴다.
- **첫 태그 이후 확정**: `sha256`는 placeholder(64-zeros)이므로 첫 `codemap-v0.1.0` 릴리스가 darwin
  tarball을 게시한 뒤 실값으로 교체해야 한다. 실제 `brew install codemap-search` availability는
  homebrew-core 수용(인지도 심사) 이후다.

## 포뮬러 위치

```
apps/codemap-search/packaging/homebrew/codemap-search.rb
```

- `version "0.1.0"`, `homepage "https://github.com/buYoung/mcp"`, `license "MIT"` — 모두
  `Cargo.toml`에서 인출(invent 아님).
- `desc`는 `FormulaAudit/Desc`(선행 관사 금지, 길이 캡)를 충족하도록 Cargo `description`에서 적응시킨
  문구를 쓴다.
- `on_arm` → `codemap-search-aarch64-apple-darwin.tar.gz`,
  `on_intel` → `codemap-search-x86_64-apple-darwin.tar.gz`.
- `install`은 tarball 루트의 맨 `codemap-search` 바이너리를 `bin.install`한다.
- `test do`는 `codemap-search --version` 출력에 `version` 문자열이 포함되는지 검사한다(오프라인, 실제).

## 메인테이너 사전 준비

- **GitHub 계정**: `homebrew/homebrew-core`를 fork하고 PR을 올릴 수 있어야 한다.
- 로컬 `brew`(macOS): `brew style`/`brew audit` 실행에 사용.
- 별도 토큰/시크릿은 필요 없다(Homebrew 제출은 GitHub PR 기반).

## sha256 placeholder → 첫 릴리스 후 채우기

포뮬러의 각 `sha256`는 현재 64개의 `0`(placeholder)이다. Homebrew는 64-hex 문자열을 요구하므로
64-zeros를 사용한다. 첫 `codemap-v0.1.0` 태그가 darwin tarball과 형제 `.sha256`를 만든 뒤 실값으로 교체한다:

- `on_arm`  ← `codemap-search-aarch64-apple-darwin.tar.gz.sha256`
- `on_intel` ← `codemap-search-x86_64-apple-darwin.tar.gz.sha256`

## 로컬 검증

```sh
brew style apps/codemap-search/packaging/homebrew/codemap-search.rb
# → 1 file inspected, no offenses detected (exit 0)
```

> `brew audit --strict --new`의 온라인 체크(`url` 도달성, `sha256` 매칭)는 첫 릴리스 전에는 통과할 수
> 없다 — 자산이 아직 없어 URL이 404이고 placeholder 해시가 매칭되지 않기 때문이다. 이는 work-failure가
> 아니라 예상된 보류 전제조건이다. by-name `brew audit`는 포뮬러를 Homebrew tap 트리에 스테이징해야
> 하므로 로컬에서 자유롭게 돌리기 어렵다(첫 태그 이후, tap 환경에서 실행).
>
> 참고: 현 포뮬러는 standalone RuboCop 실행 산물로 `# typed: strict` sigil을 쓰는데, homebrew-core는
> `Formula/**`에 대해 `Sorbet/StrictSigil`을 끄는 자체 `.rubocop.yml`을 써서 관례상 `# typed: false`
> (또는 sigil 생략)를 쓴다. homebrew-core에 제출할 때 sigil을 그쪽 관례에 맞출 수 있다.
>
> 참고(`depends_on :macos`): 현 포뮬러에는 `depends_on :macos`가 없다(오프라인 `FormulaAudit` cop이
> 부재를 지적하지 않음). 단 macOS 전용 포뮬러에 대한 실제 by-name `brew audit --new`(tap 환경, post-tag)는
> 이를 요구할 수 있으므로, 그 시점에 추가가 필요할 수 있다.

## binary-vs-source-build 텐션 (정직 표기 — review 항목)

homebrew-core는 일반적으로 **소스 빌드**를 선호하고 신규 포뮬러에 **인지도(notability) 바**를 적용한다.
본 포뮬러는 (브리프 지시대로) 사전 빌드 darwin tarball을 쓰는 **바이너리 포뮬러**다. 따라서:

- 첫 실제 `brew audit --new`(post-tag, tap 환경)에서 binary-only 형태나 인지도에 대한 지적이 나올 수
  있다. 그 경우 verbatim 메시지를 캡처해 검토하고, **임의로 소스 빌드로 전환하지 않는다** — 이는
  사용자 결정 사항이다.
- 신규 프로젝트의 new-formula PR은 인지도가 확보될 때까지 보류될 수 있다.

## 제출 런북

1. 첫 `codemap-v0.1.0` 릴리스가 darwin tarball을 게시할 때까지 기다린다(sha256 실값 필요).
2. 두 `.sha256` 자산에서 실값을 복사해 포뮬러의 placeholder를 교체한다.
3. `brew style` / (tap 환경에서) `brew audit --strict --new`로 검증한다.
4. `homebrew/homebrew-core`를 fork하고 `Formula/c/codemap-search.rb`로 PR을 올린다.
5. 메인테이너 검토 + 인지도 심사를 거쳐 수용되면 `brew install`로 설치 가능해진다.

> 실제 `homebrew/homebrew-core` PR 제출은 사용자가 직접 한다(외부/비가역 행위).

## 엔드유저 설치

수용 후:

```sh
brew install codemap-search
```

수용 전 macOS 폴백:

- `cargo install codemap-search` (crates.io)
- `install.sh` 원라이너 (`.sha256` 검증 후 설치)
- GitHub Release 자산 직접 다운로드.

## 검증 천장 (정직 표기)

- 로컬에서 확정 가능: `brew style` exit 0, 오프라인 `FormulaAudit` cop 통과, `test do` `--version` 단언.
- 첫 태그 이후에만 확정: 실 sha256 교체, `brew audit --online` URL/sha256 매칭.
- 외부 의존: homebrew-core 수용(인지도 심사 + binary-vs-source 검토).
