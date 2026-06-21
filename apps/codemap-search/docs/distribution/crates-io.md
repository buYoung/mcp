# crates.io 배포 가이드

crate `codemap-search` `0.1.0`을 crates.io에 게시하는 채널이다. 게시는 `codemap-v*` 태그에서
CI 워크플로의 `publish-crate` 잡이 **자동**으로 수행한다. 메인테이너가 한 번 토큰을 셋업하면 이후
태그 푸시만으로 게시된다.

## 현재 상태 (확인됨)

- **확인됨 / CI 자동화**: `.github/workflows/codemap-search-release.yml`의 `publish-crate` 잡이
  `codemap-v*` 태그에서 crates.io에 게시한다. `release`(바이너리) 잡을 게이트하지 않고(`needs:` 없음)
  병렬로 돈다.
- **확인됨(dry-run)**: `cargo publish --dry-run --allow-dirty --manifest-path apps/codemap-search/Cargo.toml`
  → exit 0(진짜 green). 70개 파일 패키징 + verify-compile 성공 + dry-run upload abort 로그.
- **확인됨**: crate 이름 `codemap-search`는 crates.io에서 사용 가능(미존재, 404 확인). 이름 충돌 없음.
- **첫 태그 이후 확정**: 실제 게시는 `CARGO_REGISTRY_TOKEN` 시크릿 + 실제 `codemap-v0.1.0` 태그가
  있어야만 CI에서 발생한다. **첫 게시가 `0.1.0`을 crates.io에 영구 고정**한다(같은 버전 재게시 불가).

> crates.io는 `Cargo.toml`의 `repository` 필드를 게시자의 git remote와 대조 검증하지 **않는다**.
> 현재 메타데이터(`repository = "https://github.com/buYoung/mcp"`)로 게시 가능하며, remote URL과
> 문자열이 정확히 같을 필요는 없다.

## 메인테이너 사전 준비

### 1. crates.io API 토큰 발급

1. <https://crates.io>에 GitHub 계정으로 로그인한다.
2. Account Settings → **API Tokens** → **New Token**.
3. 토큰 범위는 publish(`publish-new`, `publish-update`)를 포함하도록 한다. 가능하면 crate를
   `codemap-search`로 스코프 제한한다(첫 게시 후 crate가 존재하게 되면 스코프 지정 가능).
4. 생성된 토큰 문자열을 안전하게 복사한다(한 번만 표시됨).

### 2. GitHub Secret 등록

발급한 토큰을 저장소 시크릿 `CARGO_REGISTRY_TOKEN`으로 등록한다. 워크플로가 이 이름으로 읽는다.

```sh
gh secret set CARGO_REGISTRY_TOKEN --repo buYoung/mcp
# 프롬프트에 토큰 문자열 붙여넣기
```

또는 GitHub 웹 UI에서 **Settings → Secrets and variables → Actions → New repository secret**,
Name `CARGO_REGISTRY_TOKEN`, Value에 토큰.

> 호스트에서 맨 `cargo publish`를 절대 실행하지 않는다 — 자격증명을 픽업해 의도치 않은 실제 게시가
> 일어날 수 있다. 로컬 검증은 `cargo publish --dry-run`까지만 한다.

## 게시 런북

1. `Cargo.toml`의 `version`이 게시하려는 버전(`0.1.0`)과 일치하는지 확인한다.
2. 로컬에서 dry-run으로 패키징·컴파일을 검증한다.
   ```sh
   cargo publish --dry-run --manifest-path apps/codemap-search/Cargo.toml
   ```
3. `codemap-v0.1.0` 태그를 푸시한다. (실제 태그 푸시는 사용자가 직접 — 이 가이드의 검증 범위 밖.)
   ```sh
   git tag codemap-v0.1.0
   git push origin codemap-v0.1.0
   ```
4. CI의 `publish-crate` 잡이 자동 실행된다. 잡은 crates.io 인덱스를 사전조회해 `0.1.0`이 이미 있으면
   skip(멱등 가드), 없으면 `cargo publish`로 게시한다. 같은 태그를 재푸시/재실행해도 두 번째부터는
   200을 받아 skip하므로 red가 되지 않는다.

> **영구성 경고**: 첫 게시가 성공하면 `0.1.0`은 crates.io에 영구히 고정된다. 같은 버전으로 다시
> 게시할 수 없으므로, 게시 전 dry-run green과 버전 번호를 반드시 확인한다.

## 엔드유저 설치

```sh
cargo install codemap-search
```

`~/.cargo/bin`에 빌드·설치된다(해당 디렉터리가 `PATH`에 있어야 한다). 사전 빌드 archive와 동일한
바이너리다. Linux 권장 설치 경로다.

## 검증 천장 (정직 표기)

- 로컬에서 확정 가능: `cargo publish --dry-run` exit 0, crate 이름 가용성, 멱등 가드 200/404 분기.
- 첫 태그 이후에만 확정: 실제 게시(시크릿 + 실태그 필요), `0.1.0` 영구 고정.
