# 운영체제, 프로세스, 셸 누락 단어

- 분야 id: `os-shell`
- 원본: [../os-shell.md](../os-shell.md)
- 누락 단어 14개 · 사전에 없음 6 · 뜻 미배정 0 · 다른 분야에만 있음 8
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Filesystem permissions (`t17.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `chmod 755` | phrase | 사전에 없음 | 실행 파일 권한 설정 예시로 가장 흔한 명령입니다. |
| `chmod 777` | phrase | 사전에 없음 | 모든 사용자에게 전체 권한을 주는 명령으로 권한 오류 해결 글과 보안 경고에 쓰입니다. |
| `effective user id` | phrase | 사전에 없음 | 권한 검사에 쓰이는 프로세스의 유효 사용자 식별자입니다. |
| `real user id` | phrase | 사전에 없음 | 프로세스를 실제로 실행한 사용자 식별자입니다. |

### POSIX Linux runtimes (`t607.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `apt` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Debian package management | 분야 범위에 Debian/apt가 명시되어 있고 Debian 계열 패키지 설치 명령으로 쓰입니다. |
| `Debian` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Debian package management | 분야 범위에 명시된 리눅스 배포판으로 OS 호환성 문서에 쓰입니다. |
| `macOS` | proper_noun | 다른 분야에만 있음: mobile-desktop — Platform targeting | 지원 OS 목록과 uname 분기 같은 OS 판별 코드에 필수로 나오는 플랫폼입니다. |
| `NetBSD` | proper_noun | 다른 분야에만 있음: mobile-desktop — Platform targeting | 분야 범위에 BSD가 명시되어 있고 FreeBSD, OpenBSD와 함께 지원 목록에 나옵니다. |
| `Ubuntu` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Debian package management | 가장 흔한 리눅스 배포판으로 지원 OS와 CI 이미지 이름에 쓰입니다. |

### Process signal handlers (`t569.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `SIGABRT` | identifier | 다른 분야에만 있음: systems-low-level — Native heap allocation | abort 호출이 보내는 시그널로 시그널 핸들러 등록 코드에 나옵니다. |

### Standard stream piping (`t326.5`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `named pipe` | phrase | 다른 분야에만 있음: files-io — POSIX descriptor I/O | 셸에서 프로세스 사이 데이터 전달에 쓰는 FIFO 파일 개념입니다. |

### Systemd services (`t49.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `systemd-nspawn` | identifier | 사전에 없음 | systemd가 제공하는 경량 컨테이너 실행 명령입니다. |

## 새 주제가 필요한 단어

### Privilege elevation

범위: Running commands with elevated or reduced privileges through sudo, su, polkit and privilege dropping.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `polkit` | proper_noun | 사전에 없음 | 리눅스에서 비특권 프로세스의 특권 작업을 허가하는 권한 프레임워크입니다. |
| `sudo` | proper_noun | 다른 분야에만 있음: security — Privilege escalation | 분야 범위에 root/sudo가 명시되어 있고 설치 안내와 스크립트에 널리 쓰입니다. |
