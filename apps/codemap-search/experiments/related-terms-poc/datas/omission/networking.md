# Networking and protocols 누락 단어

- 분야 id: `networking`
- 원본: [../networking.md](../networking.md)
- 누락 단어 19개 · 사전에 없음 11 · 뜻 미배정 0 · 다른 분야에만 있음 8
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### DNS record management (`t489.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ccTLD` | abbreviation | 사전에 없음 | 도메인 등록과 public suffix 문서에서 국가 코드 최상위 도메인을 가리킨다. |
| `split horizon` | phrase | 사전에 없음 | 내부와 외부에서 같은 도메인에 다른 DNS 응답을 주는 구성을 설명하는 문서에서 쓰인다. |
| `ttl` | abbreviation | 다른 분야에만 있음: databases-storage — Lifetime durations | DNS 레코드의 캐시 유효 시간 설정 키로 존 파일과 IaC 코드에 항상 등장하며 다른 분야의 TTL과 구분되는 뜻이 있다. |
| `txt` | abbreviation | 다른 분야에만 있음: business-domains — Sender authentication | SPF, DKIM, 도메인 소유 확인에 쓰이는 DNS TXT 레코드로 존 파일과 IaC 코드에 등장한다. |

### Firewall/NAT rules (`t703`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `hairpin nat` | phrase | 사전에 없음 | 내부 클라이언트가 외부 주소로 내부 서버에 접속할 때 필요한 NAT 설정을 설명하는 문서에서 쓰인다. |

### Hostname resolution (`t489.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `nip.io` | proper_noun | 사전에 없음 | IP를 넣어 로컬 개발용 와일드카드 호스트 이름을 만드는 서비스로 문서에 쓰인다. |
| `sslip.io` | proper_noun | 사전에 없음 | IP를 넣어 와일드카드 호스트 이름을 만드는 서비스로 개발 환경 문서에 쓰인다. |

### Packet framing (`t605`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `mss clamping` | phrase | 사전에 없음 | VPN과 터널에서 MSS를 낮춰 단편화를 피하는 방화벽 설정이다. |

### Socket connection failures (`t236.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ENETDOWN` | identifier | 사전에 없음 | 네트워크 인터페이스가 내려갔을 때 나오는 소켓 오류 코드다. |
| `ENOBUFS` | identifier | 사전에 없음 | 커널 버퍼 부족으로 소켓 작업이 실패할 때 나오는 오류 코드다. |

### VPN tunnels (`t1015`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `split tunnel` | phrase | 사전에 없음 | VPN으로 보낼 트래픽만 선별하는 구성을 가리킨다. |

## 새 주제가 필요한 단어

### Socket options

범위: Setting kernel-level socket options for buffering, keep-alive, reuse, and listener queues.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `getsockopt` | identifier | 다른 분야에만 있음: databases-storage — Socket connection options | 소켓 옵션을 읽는 API로 소켓 코드에서 쓰이며 다른 분야에서는 연결 옵션 뜻으로 등록돼 있다. |
| `setsockopt` | identifier | 다른 분야에만 있음: databases-storage — Socket connection options | 소켓 옵션을 설정하는 API로 소켓 코드에서 쓰이며 다른 분야에서는 연결 옵션 뜻으로 등록돼 있다. |
| `SO_KEEPALIVE` | identifier | 다른 분야에만 있음: databases-storage — Socket connection options | TCP 연결 유지 점검 소켓 옵션으로 이 분야의 소켓 코드에서도 핵심이다. |
| `TCP_NODELAY` | identifier | 다른 분야에만 있음: databases-storage — Socket connection options | Nagle 알고리즘을 끄는 옵션으로 이 분야의 소켓 코드에서도 핵심이다. |

### TCP congestion and throughput

범위: Controlling TCP congestion windows, retransmission, and traffic shaping that affect throughput.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `bandwidth` | word | 다른 분야에만 있음: distributed-systems — Network traffic QoS | 네트워크 처리 용량을 가리키며 이 분야에서는 링크 대역폭 뜻으로 쓰인다. |
| `congestion control` | phrase | 다른 분야에만 있음: concurrency-async — Transport flow control | TCP 혼잡 제어 알고리즘 설정과 커널 옵션에 쓰인다. |

### Tunneling and port exposure

범위: Exposing local services through reverse tunnels, SSH tunnels, and jump hosts.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `autossh` | identifier | 사전에 없음 | SSH 터널을 자동 재연결하는 도구다. |
| `sshuttle` | identifier | 사전에 없음 | SSH 기반 VPN 같은 터널 도구다. |
