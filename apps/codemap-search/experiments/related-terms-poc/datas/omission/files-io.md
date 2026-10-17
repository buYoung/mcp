# Files and I/O 누락 단어

- 분야 id: `files-io`
- 원본: [../files-io.md](../files-io.md)
- 누락 단어 17개 · 사전에 없음 8 · 뜻 미배정 0 · 다른 분야에만 있음 9
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Disk filesystem formats (`t645`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `F2FS` | proper_noun | 사전에 없음 | 디스크 파일 시스템 형식 이름으로 포맷, 마운트 문서에 나온다. |
| `ZFS` | proper_noun | 다른 분야에만 있음: systems-low-level — Copy-on-write snapshots | ZFS는 디스크 파일 시스템 형식으로 마운트와 포맷 문서에서 NTFS, APFS와 나란히 쓰인다. |

### File copies (`t393.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `rsync` | proper_noun | 다른 분야에만 있음: os-shell — Operation dry runs | rsync는 파일과 디렉터리를 복사하고 동기화하는 명령으로 파일 복사 스크립트와 문서에 쓰인다. |

### File pointer seeking (`t935.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `os.pread` | identifier | 사전에 없음 | 위치를 옮기지 않고 오프셋을 지정해 읽고 쓰는 호출이다. |
| `preadv` | identifier | 사전에 없음 | 위치를 옮기지 않고 오프셋을 지정해 읽고 쓰는 호출이다. |
| `std::io::SeekFrom` | identifier | 사전에 없음 | Rust와 Go에서 읽기 위치를 옮기거나 임의 위치를 읽는 인터페이스다. |

### File stream handles (`t397.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ungetc` | identifier | 사전에 없음 | C 표준 입출력의 읽기, 쓰기, 오류 확인 함수다. |

### File watching/hot reload (`t362`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `kqueue` | proper_noun | 다른 분야에만 있음: concurrency-async — Event loop readiness | kqueue는 macOS와 BSD에서 파일 변경 감시 도구가 쓰는 이벤트 인터페이스다. |
| `watchdog` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Liveness health checks | watchdog은 Python에서 파일 변경 이벤트를 감시하는 라이브러리로 파일 감시 코드에서 쓰인다. |

### Filesystem internals (`t570`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `NFS` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Kubernetes persistent storage | NFS는 네트워크로 마운트하는 파일 시스템 이름으로 경로와 파일 시스템 문서에 나온다. |

### Item removal (`t453.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `rm` | identifier | 다른 분야에만 있음: os-shell — ANSI terminal escapes | rm은 파일과 디렉터리를 삭제하는 셸 명령으로 삭제 스크립트와 문서에서 쓰인다. |

### Native file streams (`t615.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ios::app` | identifier | 사전에 없음 | C++ 파일 스트림 클래스와 열기 모드 상수다. |
| `ios::binary` | identifier | 사전에 없음 | C++ 파일 스트림 클래스와 열기 모드 상수다. |
| `ios::trunc` | identifier | 사전에 없음 | C++ 파일 스트림 클래스와 열기 모드 상수다. |

### Standard I/O streams (`t521`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cout` | identifier | 다른 분야에만 있음: os-shell — Standard stream piping | cout은 C++ 표준 출력 스트림 객체로 콘솔 출력 코드에서 쓰인다. |
| `standard error` | phrase | 다른 분야에만 있음: math-numerics — Confidence interval estimation | standard error는 표준 출력, 표준 입력과 함께 표준 스트림 문서에서 오류 출력 대상을 가리킨다. |
| `stderr` | word | 다른 분야에만 있음: observability-debugging — Error diagnostic output | stderr는 stdout, stdin과 함께 파일 입출력과 표준 스트림 코드에서 오류 출력 대상을 가리킨다. |
