# Docker 검증 도구

한국어 | [English](./README.md)

로컬 x86_64 GNU·musl 바이너리를 빌드하고 여러 Linux 컨테이너에서 시작·작은 Rust 파일 파싱을 확인합니다. 로컬 빌드를 검증하며 게시된 릴리스를 내려받거나 검증하는 도구는 아닙니다.

## 실행

Bash, BuildKit의 로컬 출력 기능을 지원하는 실행 중인 Docker 엔진, 이미지·빌드 의존성을 받을 네트워크가 필요합니다. arm64 호스트에서는 Docker의 `linux/amd64` 에뮬레이션이 필요합니다. `timeout` 또는 `gtimeout`이 있으면 시간 제한을 적용합니다.

`apps/codemap-search`에서 실행합니다.

```bash
# From the apps/codemap-search directory:
bash docker/verify.sh

# Options:
bash docker/verify.sh --skip-gnu    # only build + test musl
bash docker/verify.sh --skip-musl   # only build + test gnu
bash docker/verify.sh --no-cleanup  # skip `docker image prune` at end
```

| 옵션 | 동작 |
|---|---|
| `--skip-gnu` | musl만 빌드·확인 |
| `--skip-musl` | GNU만 빌드·확인 |
| `--no-cleanup` | 마지막 `docker image prune -f` 생략 |

기본 정리는 다른 작업의 이미지를 포함해 이름표 없는 Docker 이미지를 삭제합니다. 유지하려면 `--no-cleanup`을 사용하세요.

로그는 `docker/verify-run.log`에 이어 쓰고 바이너리는 `docker/out/`에 저장합니다. 이전 결과가 남을 수 있으므로 이번 실행의 시작·완료 표시와 빌드 상태를 함께 확인하세요.

## 검사 항목과 이미지

빌드에 성공한 바이너리마다 아래 이미지에서 `--version`을 실행하고 생성한 `/tmp/a.rs` 파일을 파싱합니다. 종료 코드와 `GLIBC_x.xx not found`, `ld-linux` 같은 로더 오류를 기록합니다.

| 이미지 | libc 계열 |
|---|---|
| `ubuntu:20.04` | glibc |
| `ubuntu:22.04` | glibc |
| `ubuntu:24.04` | glibc |
| `debian:12` | glibc |
| `rockylinux:9` | glibc |
| `alpine:3.20` | musl |

[Dockerfile.build-gnu](./Dockerfile.build-gnu)는 Ubuntu 24.04에서, [Dockerfile.build-musl](./Dockerfile.build-musl)은 `rust:alpine`에서 빌드합니다. GNU 환경은 릴리스 워크플로의 Ubuntu 22.04와 다르므로 게시된 바이너리의 최소 glibc 버전을 확정할 수 없습니다. musl은 glibc에 의존하지 않습니다.

## 결과 해석

두 검사에 성공하면 다음 형식으로 표시합니다. 버전 번호는 예시입니다.

```
  --version exit=0  output: codemap-search 0.1.0
  smoke    exit=0   output: ...
  STATUS: PASS
```

로더 오류가 있으면 원인을 함께 표시합니다.

```
  STATUS: FAIL  detail: /lib/x86_64-linux-gnu/libc.so.6: version 'GLIBC_2.38' not found
```

기계가 읽을 수 있는 결과 줄은 `RESULT|`로 시작합니다.

```
RESULT|ubuntu:20.04|gnu|FAIL|version_exit=1|smoke_exit=1|GLIBC_2.38 not found
RESULT|ubuntu:20.04|musl|PASS|version_exit=0|smoke_exit=0|
```

이미지·바이너리별 `PASS`/`FAIL`과 빌드 요약을 함께 확인하세요. 스크립트의 최종 종료 코드만으로 모든 실패를 판단할 수 없습니다. 빌드 실패·생략으로 결과가 없는 항목은 통과가 아닙니다.

`timeout`/`gtimeout`이 있으면 빌드마다 `BUILD_CAP_SECONDS=1200`(20분), 파싱 검사는 30초로 제한합니다. 둘 다 없으면 경고하고 해당 시간 제한 없이 실행합니다. 빌드가 실패하거나 시간을 초과하면 해당 바이너리의 이미지 검사를 건너뛰며 다른 빌드의 부분 결과는 얻을 수 있습니다.

## 검증 범위

모든 빌드·실행 대상은 `linux/amd64`입니다. Apple Silicon에서는 에뮬레이션하므로 느리거나 실제 x86_64와 동작이 다를 수 있습니다. arm64 바이너리, 실제 하드웨어 동작, 성능, 전체 색인 기능, MCP 세션은 검증하지 않습니다. 호환성 결과를 공유할 때는 검사한 빌드와 환경을 함께 기록하세요.

릴리스 파일은 [설치 채널 개요](../docs/distribution/index.ko.md)를 참고하세요.
