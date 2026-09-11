# Cargo로 설치

한국어 | [English](./crates-io.md)

Rust/Cargo와 내장 파서 빌드에 필요한 C 도구 모음으로 codemap-search를 빌드·설치합니다. Cargo의 기본 실행 파일 디렉터리인 `~/.cargo/bin`을 `PATH`에 추가하세요.

## 설치와 확인

```sh
cargo install codemap-search
codemap-search --version
```

게시된 버전은 [crates.io 패키지 페이지](https://crates.io/crates/codemap-search)에서 확인합니다. 2026-09-11 문서 검토에서는 버전 정보를 확인하지 못했습니다. 특정 버전이 필요하면 게시된 버전을 Cargo의 `--version` 옵션으로 지정하세요.

로컬 소스를 설치하거나 빌드하려면 모노레포 루트에서 다음 명령을 실행합니다.

```sh
cargo install --path apps/codemap-search
# Or build without installing:
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
# Default output: apps/codemap-search/target/release/codemap-search
```

패키지나 버전을 찾지 못하면 레지스트리 페이지와 네트워크를 확인하거나 로컬 소스로 빌드하세요. 컴파일·링크 오류는 Rust와 C 도구 모음을 확인합니다. 설치 후 클라이언트가 실행 파일을 찾지 못하면 해당 클라이언트의 `PATH`를 확인하세요.

## 배포 담당자 안내

[릴리스 워크플로](../../../../.github/workflows/codemap-search-release.yml)는 `codemap-vX.Y.Z` 태그에서 실행됩니다. `publish-crate`와 `publish-release`는 모두 `build` 완료 후 각각 게시합니다. GitHub 릴리스 성공만으로 crate 게시 성공을 판단할 수 없습니다.

1. 이 crate의 게시 권한이 있는 crates.io API 토큰을 만들고 저장소 시크릿 `CARGO_REGISTRY_TOKEN`으로 등록합니다.
2. 태그 버전과 `Cargo.toml` 버전이 같은지 확인합니다.
3. 모노레포 루트에서 업로드 없이 패키징·컴파일을 검증합니다.

```sh
   cargo publish --dry-run --manifest-path apps/codemap-search/Cargo.toml
   ```

4. 게시할 버전의 태그를 푸시합니다. 게시한 crate 버전은 영구 고정되며 같은 버전을 다른 파일로 대체할 수 없습니다.
5. `publish-crate` 결과와 레지스트리의 버전 목록을 확인합니다. 작업은 이미 게시된 버전을 건너뛰고, 업로드 실패 후에는 레지스트리를 다시 확인해 실패 여부를 판단합니다.

업로드 없는 검증은 패키징·컴파일만 확인하며 토큰이나 실제 게시는 검증하지 않습니다. 사전 빌드 파일은 [설치 채널 개요](./index.ko.md)를 참고하세요.
