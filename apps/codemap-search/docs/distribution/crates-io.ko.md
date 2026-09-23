# Cargo로 설치

한국어 | [English](./crates-io.md)

Rust/Cargo와 내장 파서 빌드에 필요한 C 도구 모음으로 codemap-search를 빌드·설치합니다. Cargo의 기본 실행 파일 디렉터리인 `~/.cargo/bin`을 `PATH`에 추가하세요.

## 설치와 확인

```sh
cargo install codemap-search
codemap-search --version
```

게시된 버전은 [crates.io 패키지 페이지](https://crates.io/crates/codemap-search)에서 확인합니다. 특정 버전이 필요하면 게시된 버전을 Cargo의 `--version` 옵션으로 지정하세요. 제공되지 않는다면 아래의 로컬 소스로 빌드합니다.

로컬 소스를 설치하거나 빌드하려면 모노레포 루트에서 다음 명령을 실행합니다.

```sh
cargo install --path apps/codemap-search
# Or build without installing:
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
# Default output: apps/codemap-search/target/release/codemap-search
```

패키지나 버전을 찾지 못하면 레지스트리 페이지와 네트워크를 확인하거나 로컬 소스로 빌드하세요. 컴파일·링크 오류는 Rust와 C 도구 모음을 확인합니다. 설치 후 클라이언트가 실행 파일을 찾지 못하면 해당 클라이언트의 `PATH`를 확인하세요.

사전 빌드 파일은 [설치 채널 개요](./index.ko.md), 릴리스 준비·게시는 [배포 담당자 안내](./releasing.ko.md)를 참고하세요.
