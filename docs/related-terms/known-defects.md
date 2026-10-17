# 관련어 PoC 데이터셋의 알려진 결함

현재 기본 데이터셋에서 직접 확인한 결함, 빠졌을 수 있는 뜻, 아직 확인하지 못한 점을 항목별로 모은 목록이다. 이미 해결된 결함과 작업 경과는 담지 않는다.

| 항목 | 내용 |
| --- | --- |
| 뜻 데이터셋 | `rts4-e5083ab344e1336d`(`rt-sense/4`). `sense_lookup.py`의 기본값이다. 위치는 `apps/codemap-search/experiments/related-terms-poc/datasets/rts4-e5083ab344e1336d/dataset.sqlite3`, SHA-256은 `6888bf291e20df573b831f539224d42b9a4331c0234ab509c48f745d017c4b0f`이다. 아래 결함은 `rts3-9e01eaf1eadf960a`(SHA-256 `ab8f1aea8a60625fb4f4795e02ffdfa9b3947ce61cc7120c84a03dc61646d192`)에서 확인했다. 그 뒤 판은 분야별 누락 단어(`rts3-25df1cf41d0d8247`), 관련어로만 있던 단어의 분야 연결(`rts3-08b1780cc2880c26`), 연결하지 못한 단어의 새 개념(`rts3-5fba9ba33d7c7c7f`), 빠진 단어 재검토(`rts3-48356e35e1f1ea28`)를 더하고, 구조를 정리한 뒤(`rts4-3b9284a3b42c2917`) 분야가 하나뿐인 단어에 주제를 연결하고(`rts4-86c5620af1f181ce`), 주제를 재분류한 것(`rts4-0fb3d5bc20c215ba`, `rts4-e5083ab344e1336d`)이다. 결함 항목 단어는 `unshare`에 뜻 하나가 더해졌고([K1](#k1-개념-하나에-다른-뜻이-섞임)), 나머지는 관련어가 늘어난 것 말고는 `rts3-25df1cf41d0d8247`과 같다. `rts4` 판에서 결함 항목 단어의 뜻과 그 순서가 그대로인 것은 2026-10-17에 확인했다([작업 이력 4단계](sense-mapping-log/04-verification.md#6-결함-단어와-예시-단어)). 재분류 판과 새 뜻 순서 규칙에서도 K1·K3~K6의 증상은 그대로이고, K2는 고쳐졌다([작업 이력 8단계](sense-mapping-log/08-default-and-sense-order.md#5-함께-확인한-것)). 10단계의 뜻 순서 규칙(A)에서도 K1·K3~K5의 증상은 그대로다([작업 이력 10단계](sense-mapping-log/10-sense-order-rule.md#5-a-전환)) |
| 관련어 데이터셋 | `rtr1-930791c83bea6132`(`rt-related/1`). `sense_lookup.py`와 `category_lookup.py`가 입력 단어 해석에 쓴다. 저장소에 없고 로컬 `.codemap/poc/related-terms/thesaurus-v1/datasets/`에만 있다 |
| 확인일 | 2026-10-16 |
| 확인 방법 | 두 데이터셋을 읽기 전용으로 조회하고, `sense_lookup.py`와 `rtpoc3 related`로 실제 조회 결과를 확인했다. 코드 용례는 데이터셋 근거로 쓴 12개 저장소의 고정 체크아웃(`rts3-9e01eaf1eadf960a` 폴더의 `sources.json`)에서 찾았다 |

## 한눈에 보기

| ID | 구분 | 대상 | 조회에서 보이는 현상 |
| --- | --- | --- | --- |
| [K1](#k1-개념-하나에-다른-뜻이-섞임) | 개념에 다른 뜻이 섞임 | 개념 3060 `ns` | `unshare`에 XML parsing 뜻이 함께 나온다 |
| [K2](#k2-주제의-분야가-틀림) | 주제의 분야가 틀림 | 주제 `t56.12` | 팀 협업 뜻이 `databases-storage` 분야로 분류된다. 현재 기본값에서는 고쳐졌다 |
| [K3](#k3-관련어가-다른-뜻-아래-표시됨) | 관련어가 다른 뜻 아래 표시됨 | `execution` | 일반 실행 관련어가 `web-backend` 뜻 아래 나온다 |
| [K4](#k4-의심스러운-소속-idx-idxs) | 의심스러운 소속 | `idx`, `idxs` | 색인 관련어가 복수형 규칙(`time-locale`) 뜻 아래 나온다 |
| [K5](#k5-의심스러운-소속-multinode) | 의심스러운 소속 | `multinode` | 클러스터 노드 뜻보다 그래프·트리 구조 뜻이 더 많이 나온다 |
| [K6](#k6-판정하지-못한-소속-sourced-specs) | 판정하지 못한 소속 | `sourced`, `specs` | 맞는 연결인지 판정하지 못했다 |
| [K7](#k7-표제어-표기가-바뀜) | 표제어 표기가 바뀜 | 보정으로 추가된 표제어 28개 | `maintenance mode`가 `maintenancemode`로 표시된다 |
| [K8](#k8-관련어-데이터셋의-경쟁-제품-관계) | 경쟁 제품 관계 | 관련어 데이터셋 `rtr1` | `svelte`에 `Blazor`·`React`·`Vue`·`Angular`가 나온다 |
| [K9](#k9-뜻-순서-규칙의-한계) | 뜻 순서 | `sense_lookup.py`의 뜻 정렬 | `role`의 문서 마크업 뜻이 권한 역할 뜻보다, `tokens`의 어휘 토큰 뜻이 인증 토큰 뜻보다 앞선다 |
| [M](#m-빠졌을-수-있는-뜻) | 빠졌을 수 있는 뜻 | 단어 23개 | 제안된 뜻이 데이터셋에 없다 |
| [U1~U6](#u-확인하지-못한-것) | 확인하지 못한 것 | | 결함인지 모른다 |

## 용어

- **뜻의 구조:** 단어의 뜻(persona)은 개념(concept)에 속하고, 개념은 주제(topic)에, 주제는 1~3개의 분야(field)에 속한다. 조회 결과의 뜻은 기본적으로 주제 단위로 나뉘고, 그 뜻의 분야는 주제의 첫 분야다.
- **own:** 단어가 개념의 구성원으로 직접 등록된 연결이다.
- **input:** 단어를 다른 단어의 뜻을 통해 그 개념으로 분류하는 연결이다. 동의어일 필요는 없다.
- **관련어:** 뜻마다 저장된 관련 단어 목록이다. 조회 결과의 `direct`에 나온다.

## K. 확인된 결함

### K1. 개념 하나에 다른 뜻이 섞임

| 항목 | 내용 |
| --- | --- |
| 대상 | 개념 3060 `ns`, 주제 `t508.1` XML parsing(분야 `data-formats`) |
| 현재 상태 | 구성원 14개에 XML 이름공간 용어(`xmlnamespace`, `qnames`, `prefix` 등)와 격리 뜻 단어(`isolate`, `tenantisolation`)가 함께 있다. `namespace`·`namespaced`도 이 개념에 있다 |
| 조회 결과 | Linux 격리 명령 `unshare`는 이 개념의 persona 17162·21601을 거쳐 input으로도 분류된다. 새 개념 추가([파이프라인](pipeline.md) 12단계)로 `os-shell`의 Namespace and root isolation 뜻이 먼저 나오지만, 그 아래에 XML parsing 뜻과 관련어 `isolate`, `namespace`가 그대로 나온다 |
| 영향 | 격리·이름공간 관련 단어의 조회에 XML 뜻이 섞인다 |

### K2. 주제의 분야가 틀림

| 항목 | 내용 |
| --- | --- |
| 대상 | 주제 `t56.12` Team collaboration |
| 현재 상태 | 분야가 `databases-storage` 하나뿐이다. 이 주제의 개념은 5793 `팀 협업 서비스`, 6418 `Slack message attachment`, 6421 `chat bot`, 6424 `direct message`, 6425 `user mention`, 6434 `link preview`다 |
| 영향 | 이 개념에 속한 단어를 조회하면 팀 협업 뜻이 `databases-storage` 분야로 표시된다 |
| 관련 항목 | [M](#m-빠졌을-수-있는-뜻)의 `messaging` 제안은 이 주제를 대상으로 한다 |
| 고친 판 | 재분류 판 `rts4-0fb3d5bc20c215ba`부터 이 주제의 분야는 `docs-collaboration`이다. 현재 기본값 `rts4-e5083ab344e1336d`에도 들어 있다. 위의 현재 상태는 `rts4-86c5620af1f181ce`까지의 상태다([작업 이력 6단계](sense-mapping-log/06-reclassification.md)) |

### K3. 관련어가 다른 뜻 아래 표시됨

| 항목 | 내용 |
| --- | --- |
| 대상 | `execution`의 persona 11311. 개념 1036 `invocation`, 주제 `t89.1` Server route handlers(분야 `web-backend`)에 속한다 |
| 현재 상태 | 일반 실행 관련어 16개가 이 persona에 붙어 있다: `exec`, `run`, `runs`, `invocation`, `executioncontext`, `executionorder`, `executionflow`, `executionresult`, `executiontrace`, `exitstatus`, `jobrun`, `testrun`, `sandbox`, `entrypoint`, `permissions`, `trace`. `execution`에는 일반 프로그램 실행을 뜻하는 개념이 없다 |
| 조회 결과 | `execution`을 조회하면 위 관련어가 `web-backend` 뜻 아래 나온다 |

### K4. 의심스러운 소속: `idx`, `idxs`

| 항목 | 내용 |
| --- | --- |
| 대상 | 개념 3409 `Intl.PluralRules`, 주제 `t315.5` Plural form selection(분야 `time-locale`) |
| 현재 상태 | `idx`와 `idxs`가 이 개념의 구성원이다. 같은 개념에 `nth`, `ordinal`, `day`, `pluralrules` 등이 있다 |
| 조회 결과 | `idx`를 조회하면 Plural form selection 뜻 아래 `subscript`, `ordinal`, `array index`, `database index`, `getIndex`가 나온다 |
| 판정 상태 | 의미 검토에서 한 명만 문제를 지적해 고치지 않았다 |

### K5. 의심스러운 소속: `multinode`

| 항목 | 내용 |
| --- | --- |
| 대상 | own 4개: 1962 `vertex`(`t354.1` Graph path search), 2246 `dom`(`t321.1` DOM element manipulation), 3956 `tree`(`t496.4` Tree structure traversal), 4890 `SyntaxKind`(`t496.5` TypeScript syntax trees) |
| 코드 용례 | 12개 저장소의 용례 5곳은 모두 여러 서버·클러스터 노드라는 뜻이다. vector Kubernetes 테스트의 `MULTINODE` 환경 변수, vector 릴리스 기록과 RFC, keycloak 캐싱 문서, redis 클러스터 테스트다. 그래프·트리·DOM 노드 뜻의 용례는 찾지 못했다 |
| 조회 결과 | 뜻 19개가 나온다. 클러스터 뜻(Kubernetes pod·manifest, host, Redis Cluster)은 4개이고, 나머지는 그래프·트리·AST·연결 리스트·리치 텍스트 편집기 같은 구조 뜻이다 |
| 판정 상태 | 구조 노드 뜻(예: 여러 노드 선택)이 성립하는지 판정하지 못해 고치지 않았다 |

### K6. 판정하지 못한 소속: `sourced`, `specs`

| 단어 | 현재 연결 | 판정하지 못한 이유 |
| --- | --- | --- |
| `sourced` | own 809 `interpreter`(`t254.1` Bytecode interpretation) | 셸 `source` 명령으로 실행된 스크립트라는 뜻이면 성립할 수 있다. 소스 코드라는 뜻과 구별할 근거가 부족하다 |
| `specs` | own 1079 `filter`(`t286.1` Log ingestion pipelines) | 이 소속의 근거가 불명확하지만, 대신 넣을 개념이 없어 지우면 뜻이 사라진다 |

### K7. 표제어 표기가 바뀜

| 항목 | 내용 |
| --- | --- |
| 원인 | `correct_sense_dataset.py`는 데이터셋에 없는 단어를 추가할 때, 넘겨받은 표기를 그대로 표제어 표기로 저장한다(`term`, 161~169행). 띄어쓰기·대소문자가 없는 키 형태를 넘기면 그 형태가 저장된다 |
| 조회 결과 | 조회 도구는 같은 단어에 표제어 표기를 우선 표시한다. `bypass`를 조회하면 관련어가 `maintenance mode`가 아니라 `maintenancemode`로 나온다 |
| 대상 | 보정으로 추가된 표제어 114개 중 28개. 표제어 표기가 키와 같고, 다른 단어의 관련어로는 다른 표기로 저장된 것이다 |
| 참고 | `cutoff`, `backchannel`처럼 붙여 쓰는 표기도 쓰이는 단어와, `committed`, `retired`처럼 대소문자만 다른 단어는 표시 문제가 작다 |

대상 28개(저장된 표제어 → 다른 곳의 표기): `ssd` → `SSD`, `coco` → `COCO`, `iou` → `IoU`, `nms` → `NMS`, `getlookandfeel` → `getLookAndFeel`, `lookandfeel` → `look and feel`, `elementat` → `elementAt`, `elemindex` → `elemIndex`, `elemsize` → `elem_size`, `elemtype` → `elem_type`, `cutoff` → `cut off`, `endofservice` → `end of service`, `eos` → `EOS`, `maintenancemode` → `maintenance mode`, `retired` → `Retired`, `releasedate` → `release date`, `releasemanager` → `release manager`, `committed` → `COMMITTED`, `rax` → `RAX`, `ziplist` → `ZipList`, `bio` → `BIO`, `backchannel` → `back channel`, `nextrow` → `next row`, `followingrow` → `following row`, `nextvalue` → `next value`, `window.orderby` → `Window.orderBy`, `f.lead` → `F.lead`, `leadgeneration` → `lead generation`

### K8. 관련어 데이터셋의 경쟁 제품 관계

| 항목 | 내용 |
| --- | --- |
| 대상 | 관련어 데이터셋 `rtr1-930791c83bea6132` |
| 현재 상태 | GitHub Explore(`github-explore-c8d7bc5`)에서 온 `related_topic` 관계가 경쟁 제품·프레임워크를 서로 잇는다 |
| 조회 결과 | `rtpoc3 related`로 `svelte`를 조회하면 `Blazor` 73.9%(`reverse:related_topic`), `React` 57.9%, `Vue` 55.1%, `Vue.js` 51.6%, `Angular` 50.9%가 나온다 |
| 확인하지 못한 것 | 이 관계가 `sense_lookup.py`의 입력 해석이나 결과에 영향을 주는지는 확인하지 않았다 |

### K9. 뜻 순서 규칙의 한계

| 항목 | 내용 |
| --- | --- |
| 대상 | `sense_lookup.py`의 뜻 순서. 검토로만 연결한 뜻은 맨 뒤, 단어 자신의 뜻은 다른 단어를 거쳐 읽는 뜻보다 앞, 그다음 생성 횟수 순이고, 생성 횟수가 같으면 입력의 임베딩과 각 뜻의 주제 단어 평균 임베딩의 코사인 순이다([작업 이력 10단계](sense-mapping-log/10-sense-order-rule.md)) |
| 현재 상태 | 생성 횟수는 코드에서의 빈도가 아니어서, 횟수가 많은 엉뚱한 뜻이 앞설 수 있다. 검토로만 연결한 뜻은 맞는 뜻이어도 맨 뒤다. 생성 횟수가 같은 뜻 사이에서는 질의 표기만 보므로 문맥을 구분하지 못하고, 같은 글자가 든 주제가 앞서기 쉽다. 평가 세트 224단어에서 첫 뜻 적중은 70.5%이고, 정렬만으로 닿을 수 있는 상한은 85.3%다 |
| 조회 결과 | `role`은 4번 생성된 Documentation markup이 Role assignments(1번)보다 앞선다. `tokens`는 Lexical token streams와 Design token tooling이 OAuth access tokens보다 앞선다(모두 1번). `jsonp`는 검토로만 연결한 Browser cross-origin policies가 뒤다 |
| 판정 상태 | 유사도만 쓰는 정렬, 생성 횟수와 유사도를 섞은 점수, 여백 규칙, 주제 단어 코사인 평균 등은 이 규칙보다 낫다는 근거를 얻지 못했다(10단계 3절, 6절) |

## M. 빠졌을 수 있는 뜻

의미 검토에서 검토자 한 명만 추가를 제안해 반영하지 않은 뜻이다. 결함으로 확정된 것은 아니다. 2026-10-16 기준으로 제안된 개념은 데이터셋에 하나도 없다. 대부분 복수형만 담는 개념이다.

| 단어 | 제안된 뜻 | 반영하지 않은 이유 |
| --- | --- | --- |
| `consoles` | 계정 관리 콘솔(account management console) | 복수형만 담는 개념이 되어 기본형 `console`과 어긋난다 |
| `datasets` | 텔레메트리 질의 대상 데이터 집합(telemetry query dataset) | 복수형만 담는 개념이 되어 기본형 `dataset`과 어긋난다 |
| `grantee` | 권한을 받는 주체(access grant recipient) | 같은 개념에 `grantor`, `grantees`가 있어, `grantee`만 옮기면 역할어가 두 개념으로 갈린다 |
| `inline`, `inlined` | 문서 안에 넣은 웹 자원 본문(inline web resource content) | 제안된 주제(`t586`, 리소스 파일 조직)와 맞지 않고, 기존 개념 6124·6295와의 관계가 정해지지 않았다 |
| `insights` | 애플리케이션 성능 분석(application performance analytics) | 기존 개념 2281 `observability`와의 관계가 정해지지 않았다 |
| `kinesis streams` | Amazon Kinesis Data Streams | 같은 개념 1392에 `Kinesis`, `Amazon Kinesis`가 있어 새 개념과 겹친다 |
| `messaging` | 팀 메시징(team messaging) | 대상 주제 `t56.12`의 분야가 틀려 있어([K2](#k2-주제의-분야가-틀림)), 이 주제에 추가하면 팀 메시징 뜻이 `databases-storage` 분야로 분류된다 |
| `packages` | Windows 앱 패키지(Windows application package) | 복수형만 담는 개념이 되고, 같은 주제의 `appx`(573)와 겹친다 |
| `prelogin` | SQL Server 연결의 prelogin 단계 | 근거가 keycloak 테스트의 오류 문자열 하나뿐이다 |
| `previews` | 청구서 미리보기(invoice preview) | 복수형만 담는 개념이 되어 기본형 `preview`와 어긋난다 |
| `processors` | 입력 처리 구성요소, 규칙 평가 처리기 | 두 제안의 범위가 다르고, 맞는 기존 주제가 없다 |
| `rates` | 배송 요금(shipping rate) | 복수형만 담는 개념이 되어 기본형 `rate`와 어긋난다 |
| `services` | 비즈니스 서비스 상품(business service offerings) | 근거가 marketplace UI 문자열 하나뿐이고, 복수형만 담는 개념이 된다 |
| `sizer` | 배치 항목 크기 추정기(batch item size estimator) | 근거가 vector의 `StatsdBatchSizer` 하나뿐이다 |
| `sizes` | 이미지 크기 변형(image size variants) | 복수형만 담는 개념이 되어 기본형 `size`와 어긋난다 |
| `subkeys` | Redis 해시 하위 키(Redis hash subkeys) | 복수형만 담는 개념이 되어 기본형 `subkey`와 어긋난다 |
| `unsubscribed` | 이슈 알림 구독 상태 | `subscribe`, `subscribed`와 개념이 갈리고, 근거가 sentry 하나뿐이다 |
| `valued` | 속성 값의 개수(single-valued, multi-valued) | `valued` 단독에 이 뜻이 성립하는지 정해지지 않았다 |
| `viewers` | 콘텐츠 뷰어(content viewer) | 복수형만 담는 개념이 되어 기본형 `viewer`와 어긋난다 |
| `rxs` | 채널 수신 끝점(channel receiver) | `rx`, `receiver`와 개념이 갈린다 |
| `savings` | 실행 비용 절감, 메모리 크기 절감, 금전 비용 절감 | 기존 개념 6681(앱 크기 절감) 외에 뜻을 나눌 필요가 있는지 정해지지 않았다 |
| `short date string` | `datestr`의 개념 1746 뜻 | 형태 변화 관계가 아니고, 원래 뜻이 서식 지정자인지 일반 문자열 이름인지 확정되지 않았다 |
| `committed` | `CommitInfo` 관련어 | 이 이름을 제공하는 외부 라이브러리를 확인하지 못했다 |

분야 연결([파이프라인](pipeline.md) 11단계)에서 맞는 개념이 없어 연결하지 않은 1,060쌍은 새 개념 추가(12단계)에서 다시 판정해 1,033쌍을 넣었다. 넣지 않은 27쌍은 빠진 단어 재검토(13단계)에서 설명이 닿는 모든 분야를 다시 보고 25단어를 연결했다. 연결할 분야가 남지 않은 `requireNonNull`, `negative testing`과 단어별 판단은 `rts3-48356e35e1f1ea28` 폴더의 `rereview.json`에 있다.

## U. 확인하지 못한 것

| ID | 내용 | 상태 |
| --- | --- | --- |
| U1 | 품질 개선·회귀 | [단어 평가](word-evaluation.md)의 평가 단어는 최신 데이터셋으로 다시 판정하지 않았다. 조회 품질이 나아졌는지, 다른 단어가 나빠졌는지 그 단어들로는 모른다. `rts4` 판들만 새 평가 세트 224단어와 이전 판과의 조회 비교로 확인했다([작업 이력](sense-mapping-log/README.md)) |
| U2 | 이전 중간판에서 본 도움 단어 소실 | 이전 중간판에서 일부 평가 문맥의 도움 단어가 줄었다. Terminal `menu`의 `deselect`, Keycloak `comitted`, `debounced`의 관련어 수, Redis `multiple`·Vector `bulk`·Sentry `tuple`이 그 사례다. 최신 데이터셋에서는 다시 측정하지 않았다 |
| U3 | `common`, `multiple`의 관련어 혼합 | 소속 개념(각각 8개, 7개)은 확인했다. 각 뜻의 관련어에 다른 뜻의 단어가 섞였는지는 전수 확인하지 않았다 |
| U4 | 같은 단어의 뜻 여러 개가 같은 개념에 속한 경우 | 1,804쌍(persona 3,741개)이 있다. 조회 품질에 영향을 주는지 판정하지 않았다 |
| U5 | 분야 연결로 입력 해석이 바뀐 단어 | 분야를 연결한 2,953단어는 이제 단어 전체로 해석된다. 이 중 610단어는 전에 부분으로 나뉘어 나오던 다른 분야 뜻이 빠졌다. 대부분은 `AppVeyor` → `app`, `OpenWhisk` → `open` 같은 부분 잡음이다. 그러나 `std::multimap`의 C++ STL 뜻(`std`)처럼 쓸모 있던 뜻도 빠진다. 전체 품질이 나아졌는지 판정하지 않았다 |
| U6 | 새 개념 추가·재검토로 입력 해석이 바뀐 단어 | 새 개념에 넣은 1,019단어도 단어 전체로 해석된다. 이 중 104단어는 전에 부분으로 나뉘어 나오던 뜻이 빠졌다. 대부분은 `user-select` → `user`·`select` 같은 부분 잡음이다. 그러나 `ANSI escape code`가 `ansi escape`를 거쳐 받던 `os-shell` 뜻처럼 쓸모 있던 뜻도 빠진다. 넣은 단어를 포함하는 다른 입력은 나뉘는 방식이 바뀐다(`avro-tools.jar` → `avro tools` + `jar`). 자기 뜻이 생긴 단어는 기본형으로 넓혀 찾지 않으므로 기본형의 뜻이 빠진다(`Event Hubs`가 `Event Hub`의 AMQP publishing 뜻을 잃음). 빠진 단어 재검토(13단계)에서도 49단어 가운데 8단어가 같은 이유로 뜻이 빠졌다. `spelling` → `SpEL` 같은 잡음이 대부분이다. 그러나 `cgroups`는 기본형 `cgroup`을 거쳐 받던 CPU resource usage 뜻을 잃었다. 전체 품질이 나아졌는지 판정하지 않았다 |

## 고칠 때 지킬 점

- own을 지우기 전에 그 persona에 붙은 관련어·input 의존을 먼저 옮기거나 지운다. 의존이 남아 있으면 `correct_sense_dataset.py`가 거부한다.
- 한 보정 계획에서 관련어를 보내는 persona가 다른 행에서 받는 persona이면 계획 전체가 거부된다. 계획을 둘로 나눠 순서대로 적용한다.
- 새 단어는 원래 표기로 넘긴다. 키 형태로 넘기면 [K7](#k7-표제어-표기가-바뀜)이 생긴다.
- 첫째·둘째 항목은 `correct_sense_dataset.py`의 규칙이다. 이 도구는 `rt-sense/3` 판만 받으므로, 현재 기본값(`rt-sense/4`)은 `add_topic_mappings.py`, `revise_topics.py` 같은 `rt-sense/4`용 도구로 고친다.
- 새 데이터셋을 기본값으로 쓰려면 `sense_lookup.py`의 `DEFAULT_SENSE_DATASET`을 바꾼다.
