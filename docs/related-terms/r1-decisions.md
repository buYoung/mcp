# 관련어 시소러스: R1~R3 처리 결정 기록

[미반영 결함 목록](unresolved-defects.md)의 R1~R3 조치를 단어별로 판단해 새 데이터셋에 반영한 기록이다. 판단은 이 작업 세션이 원 비교 파일과 analyst·crosscheck 검토 원문, 데이터셋, 고정 체크아웃 코드를 직접 확인해 내렸다. 검토자 합의는 수락으로 취급하지 않았다.

| 항목 | 내용 |
| --- | --- |
| 작업일 | 2026-10-16 |
| 시작 데이터셋 | `rts3-c4a3b0fe9eb74bc2` (이전 기본값) |
| 결과 데이터셋 | `rts3-b5fb0c137c71a08f` (R1~R3 처리 시점의 기본값. 이후 [4단계](stage4-decisions.md) 결과 `rts3-9e01eaf1eadf960a`로 대체) |
| 결과 SHA-256 | `62b1122b10d6af2e4d24886e046148ae5912b6da0aa1f9504e73f6e372db3a48` |
| 작업 파일 | `~/.cache/related-terms-usage-1/word-eval/r1-integration-001/` (계획 `plans/`, 결정 `decisions/`, 확인 결과 `checks/`) |

## 결과 요약

| 범위 | 대상 | 채택 | 기각 | 보류 |
| --- | ---: | ---: | ---: | ---: |
| R1 합의 조치 (296단어) | 1,610 | 1,610 | 0 | 0 |
| R2 이견 조치 (63단어) | 116 | 60 | 25 | 31 |
| R3 합의 조치 (4단어) | 119 | 117 | 2 | 0 |

- R1 채택 1,610건 중 1건(`detectors`의 461)은 같은 뜻의 현재 위치(7310)로 재지정해 적용했다.
- R1 단어에 함께 남은 이견 조치 325건은 이번 범위에 없어 처리하지 않았다.
- 이후 [4단계](stage4-decisions.md)에서 R1 단어의 이견 325건과 아래 R2 보류 31건을 판단했다. 이 문서의 결정과 이유는 R1~R3 처리 시점의 기록으로 그대로 둔다.

## 판단 기준

- **제거:** 원천 persona의 실제 뜻과 주제 범위가 그 단어에 성립하지 않을 때 채택했다. own 제거는 그 persona의 native 관계, inbound input, input_relations가 0건인 것을 먼저 확인했다.
- **뜻 상속:** 개념마다 파생형 자체에 그 뜻이 성립하는지 확인했다. 기본형의 own은 own으로, input은 input으로만 상속했다.
- **own 추가:** 실제 코드에서 그 개념의 구성원임이 확인될 때 채택했다. 이미 input이 있는 개념에 own을 더한 4건(`ru`, `subtotals`, `typing`, `untag`)은 input 승격이 아니라 별도 직접 근거가 있는 경우다.
- **이견(R2):** 한쪽 의견만으로 채택하지 않았다. 체크아웃 코드·개념 구성원·원천 persona 관계를 직접 확인했고, 확인이 어렵거나 원칙 경계에 걸리면 보류했다. 기본형이 같은 개념에 있는데 파생형만 제거하자는 제안은 일관성 문제로 기각 또는 보류했다.

## 적용 단계

| 단계 | 데이터셋 | 내용 | 저장 확인 | 소비 확인 |
| --- | --- | --- | --- | --- |
| 1 | `rts3-a8af20d9b15ab705` | R1 own 제거 107, input 제거 113 (input 행 189) | 일치, FK 0 | 105단어 반영. `slaves`·`subtotals`는 기본형에 같은 연결이 있어 조회에 남음 |
| 2a | `rts3-554bf1373eb43b73` | R1 새 개념 5 (7331~7335) | 일치, FK 0 | 5단어 반영 |
| 2b | `rts3-5b9386bd6787963e` | R1 이동 9, own 추가 44, 상속 198항목(개념 1,332) | 일치, FK 0 | 224단어 반영. `subtotals`는 own이 생겨 기본형 확장이 멈추며 3459도 사라짐 |
| 3a | `rts3-3b085066fb651e66` | R2 새 주제 1 (x51), 새 개념 5 (7336~7340) | 일치, FK 0 | 9단어 반영 |
| 3b | `rts3-b5fb0c137c71a08f` | R2·R3 이동 2, own 제거 6, input 제거 11, own 추가 32, 상속 6항목(개념 118) | 일치, FK 0 | 42단어 반영. 아래 관찰 참고 |

3단계 소비 확인에서 관찰한 사항은 다음과 같다. 모두 조회 도구가 own이 없는 단어를 기본형으로 넓히는 동작(exact-first)에서 생긴다.

- `negations`: own이 없어 기본형 `negation`으로 넓혀지고, 기본형에 같은 2865 input이 남아 있어 조회에 계속 나온다.
- `investigations`, `preds`: own이 생겨 기본형 확장이 멈추면서 기본형에만 있던 뜻(`investigation`의 4081, `pred`의 16·1365·2222·4693)이 조회에서 빠졌다. 저장 데이터에는 원래 없던 연결이며 검토 조치에도 없어 처리하지 않았다.

## R1 단어별 결정

제거·상속·own 추가·이동·새 개념을 단어 단위로 묶었다. 상속은 모든 개념을 파생형 기준으로 확인했으며 아래 표에는 채택한 개념 수만 적었다.

| 단어 | 결정 | 내용 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `attrs` | 채택 | attr에서 상속 2개념 | 기본형 attr의 각 뜻이 attrs에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `averaged` | 채택 | input 제거 2849 | medium/mid의 중간·보통 형용사 뜻은 평균 계산 결과 averaged에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `backends` | 채택 | backend에서 상속 11개념 | 기본형 backend의 각 뜻이 backends에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `batched` | 채택 | batch에서 상속 13개념 | 기본형 batch의 각 뜻이 batched에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `batcher` | 채택 | own 제거 739 | batch file(.bat/cmd) 명칭 뜻은 묶음 처리 구성요소 batcher에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `bics` | 채택 | bic에서 상속 1개념 | 기본형 bic의 각 뜻이 bics에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `blinking` | 채택 | blink에서 상속 2개념 | 기본형 blink의 각 뜻이 blinking에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `blueprints` | 채택 | input 제거 3508; blueprint에서 상속 2개념 | prints 철자 겹침으로 logger 출력 주제에 들어온 연결. 청사진/Flask Blueprint와 무관 / 기본형 blueprint의 각 뜻이 blueprints에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `bracketed` | 채택 | bracket에서 상속 1개념 | 기본형 bracket의 각 뜻이 bracketed에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `branding` | 채택 | brand에서 상속 3개념 | 기본형 brand의 각 뜻이 branding에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `brands` | 채택 | brand에서 상속 3개념 | 기본형 brand의 각 뜻이 brands에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `bubbled` | 채택 | own 제거 456 | bubble chart 도형 명사 뜻은 이벤트 전파 과거분사 bubbled에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `bufferable` | 채택 | own 제거 4212; input 제거 4647, 5123 | Top-N 선택·Neovim LSP 탐색·공유(shared) 뜻은 버퍼 저장 가능 능력형에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `bundles` | 채택 | bundle에서 상속 14개념 | 기본형 bundle의 각 뜻이 bundles에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `cancellable` | 채택 | own 제거 3935 | OAuth 권한 철회(revocation) 뜻은 진행 중 작업의 취소 가능성과 다름 | `rts3-a8af20d9b15ab705` |
| `canonicalized` | 채택 | canonicalize에서 상속 4개념 | 기본형 canonicalize의 각 뜻이 canonicalized에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `canonicalizes` | 채택 | canonicalize에서 상속 3개념; own 추가 7204 | 기본형 canonicalize의 각 뜻이 canonicalizes에도 성립(own/input 출처 유지) / Azure Shared Key 서명용 요청 정규화 동작 = request canonicalization | `rts3-5b9386bd6787963e` |
| `cascades` | 채택 | cascade에서 상속 8개념 | 기본형 cascade의 각 뜻이 cascades에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `cells` | 채택 | cell에서 상속 8개념 | 기본형 cell의 각 뜻이 cells에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `channels` | 채택 | input 제거 2827; channel에서 상속 31개념 | AsyncAPI 메시지 경로를 inbox 읽음/안읽음 상태로 분류한 원천 오분류 / 기본형 channel의 각 뜻이 channels에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `choices` | 채택 | input 제거 5072; choice에서 상속 3개념 | possible의 허용값·대안 뜻을 nullable 처리로 잘못 연결 / 기본형 choice의 각 뜻이 choices에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `claimed` | 채택 | input 제거 3245; claim에서 상속 2개념 | reserved의 구조체 예약 비트/정렬 뜻은 확보·소유권 인수 상태 claimed와 다름 / 기본형 claim의 각 뜻이 claimed에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `cleanups` | 채택 | own 제거 3039; input 제거 2982, 4161, 536 | 필요성 표현·소켓 연결 실패·작별 인사·CSS reset 뜻은 정리 작업들에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `clearable` | 채택 | input 제거 4203 | 원천 both/float의 CSS float clear 뜻이 집합 대수 주제로 들어온 오분류 | `rts3-a8af20d9b15ab705` |
| `clicked` | 채택 | input 제거 61, 707, 790 | 원천의 click은 Python Click CLI 라이브러리. UI 클릭 과거형에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `cohorts` | 채택 | cohort에서 상속 2개념 | 기본형 cohort의 각 뜻이 cohorts에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `collaborators` | 채택 | collaborator에서 상속 1개념 | 기본형 collaborator의 각 뜻이 collaborators에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `collected` | 채택 | collect에서 상속 8개념 | 기본형 collect의 각 뜻이 collected에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `cols` | 채택 | col에서 상속 2개념 | 기본형 col의 각 뜻이 cols에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `columns` | 채택 | input 제거 2250; column에서 상속 12개념 | Bulma 레이아웃 열을 체크박스/폼 상태로 분류한 원천 오분류 / 기본형 column의 각 뜻이 columns에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `comparers` | 채택 | comparer에서 상속 4개념 | 기본형 comparer의 각 뜻이 comparers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `components` | 채택 | component에서 상속 9개념; own 추가 6573; own 추가 7246 | 기본형 component의 각 뜻이 components에도 성립(own/input 출처 유지) / Sentry grouping component 노드 = exception grouping components / CSS ValueList.getListComponents의 값 구성 요소 = rule value list 구성원 | `rts3-5b9386bd6787963e` |
| `compressed` | 채택 | compress에서 상속 2개념 | 기본형 compress의 각 뜻이 compressed에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `concatenates` | 채택 | own 제거 4509 | 문자열 이어붙이기는 서식 문자열 보간(치환) 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `configurer` | 채택 | own 제거 2179; own 추가 2079; own 추가 72 | 설정 주체 명사는 실패 상태 피드백(image pull backoff) 개념의 구성원이 아님 / ScriptBindingsConfigurer는 호출되는 설정 callback / clientConfigurer는 client 설정 권한만 가진 제한 관리 역할 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `consoles` | 채택 | own 추가 7185 | Windows console.host 확장 catalog의 console host 구현 | `rts3-5b9386bd6787963e` |
| `consts` | 채택 | input 제거 1813 | const 함수형 combinator 뜻은 constants 복수 명사에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `contractless` | 채택 | own 제거 2296, 2317, 692, 928; input 제거 236, 2387, 2944, 3572, 3980, 4071 | -less 부정 파생형에 계약 객체(bytecode·Pact·interface·precondition)와 계약 관련 input 뜻을 일괄 복제한 오상속 | `rts3-a8af20d9b15ab705` |
| `contributes` | 채택 | contribute에서 상속 4개념 | 기본형 contribute의 각 뜻이 contributes에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `coordinates` | 채택 | coordinate에서 상속 3개념 | 기본형 coordinate의 각 뜻이 coordinates에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `creat` | 채택 | own 제거 1014, 1076, 1847, 2237, 2260, 4318, 5002, 5011, 986; input 제거 1307, 1615, 3618, 4488 | POSIX creat/O_CREAT 고정 표기에 일반 create의 UI·HTTP·DDL·객체·계정 등 뜻을 일괄 상속한 오류. 파일 생성 뜻은 유지 | `rts3-a8af20d9b15ab705` |
| `creatable` | 채택 | own 제거 5002 | 생성 가능성은 캐시 만료(expiresAt) 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `custom fields` | 채택 | customfield에서 상속 1개념 | 기본형 customfield의 각 뜻이 custom fields에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `customizations` | 채택 | customization에서 상속 3개념 | 기본형 customization의 각 뜻이 customizations에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `data sources` | 채택 | datasource에서 상속 11개념 | 기본형 datasource의 각 뜻이 data sources에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `datasets` | 채택 | dataset에서 상속 10개념 | 기본형 dataset의 각 뜻이 datasets에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `definitions` | 채택 | definition에서 상속 11개념 | 기본형 definition의 각 뜻이 definitions에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `degrees` | 채택 | degree에서 상속 3개념 | 기본형 degree의 각 뜻이 degrees에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `deletes` | 채택 | delete에서 상속 32개념 | 기본형 delete의 각 뜻이 deletes에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `deletions` | 채택 | deletion에서 상속 1개념 | 기본형 deletion의 각 뜻이 deletions에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `descendent` | 채택 | input 제거 2743, 3813; own 추가 3956 | switch fall-through·compose down 뜻은 하위 노드 관계에 성립하지 않음 / 하위 노드는 tree 구조의 실제 구성 요소 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `detectors` | 채택(1건 재지정) | detector에서 상속 3개념 | 기본형 detector의 YOLO 객체 탐지 뜻이 이전 통합(281/295)에서 461→7310(YOLO vision processing)으로 재귀속됨. 같은 뜻의 현재 위치 7310을 상속 / 기본형 detector의 각 뜻이 detectors에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `devices` | 채택 | device에서 상속 11개념 | 기본형 device의 각 뜻이 devices에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `directions` | 채택 | direction에서 상속 6개념 | 기본형 direction의 각 뜻이 directions에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `dotfiles` | 채택 | input 제거 2058; dotfile에서 상속 3개념 | 파일시스템 hidden 뜻이 웹 접근성(ARIA hidden) 주제로 들어온 오분류 / 기본형 dotfile의 각 뜻이 dotfiles에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `dotted names` | 채택 | input 제거 4057; dottedname에서 상속 1개념 | dots의 산점도 점 뜻은 점 구분 한정 이름에 성립하지 않음 / 기본형 dottedname의 각 뜻이 dotted names에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `fixups` | 채택 | fixup에서 상속 3개념 | 기본형 fixup의 각 뜻이 fixups에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `flashes` | 채택 | own 추가 6346 | session['_flashes'] 목록 항목 = flash message | `rts3-5b9386bd6787963e` |
| `floating` | 채택 | own 제거 4203 | CSS float 원천이 집합 대수(union) 개념에 들어간 오상속 | `rts3-a8af20d9b15ab705` |
| `flt` | 채택 | own 제거 4203 | FLT 부동소수점 조각은 집합 대수(union) 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `folders` | 채택 | folder에서 상속 5개념 | 기본형 folder의 각 뜻이 folders에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `fragments` | 채택 | fragment에서 상속 15개념 | 기본형 fragment의 각 뜻이 fragments에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `framed` | 채택 | frame에서 상속 7개념 | 기본형 frame의 각 뜻이 framed에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `framer` | 채택 | own 제거 1077, 1783, 3660, 455; input 제거 3252, 4371 | frame의 데이터프레임·스택 프레임·fps·CSS 테두리·화면 캡처·디버거 뜻은 byte framing 수행자 framer에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `fulfillments` | 채택 | 개념 5487 이동 t213.4→t261.3 | Order fulfillment 개념이 concurrency-async 분야 주제에 있음 → business-domains 주문 이행 주제로 | `rts3-5b9386bd6787963e` |
| `gateways` | 채택 | own 제거 1838 | SMS/서비스 gateway를 wiki 문서 플랫폼에 넣은 원천 오분류의 상속 | `rts3-a8af20d9b15ab705` |
| `generators` | 채택 | generator에서 상속 16개념 | 기본형 generator의 각 뜻이 generators에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `grained` | 채택 | grain에서 상속 1개념 | 기본형 grain의 각 뜻이 grained에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `grantee` | 채택 | own 제거 3935 | 권한 수령 주체는 권한 철회 동작·철회 대상이 아님 | `rts3-a8af20d9b15ab705` |
| `grantor` | 채택 | own 제거 3935 | 권한 부여자는 권한 철회 동작·철회 대상이 아님 | `rts3-a8af20d9b15ab705` |
| `grouping` | 채택 | group에서 상속 6개념 | 기본형 group의 각 뜻이 grouping에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `guarded` | 채택 | input 제거 1433, 5133; guard에서 상속 12개념 | safely·protected의 일반 보호 뜻을 계정 검증·API 가시성 주제로 잘못 연결 / 기본형 guard의 각 뜻이 guarded에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `guards` | 채택 | guard에서 상속 15개념 | 기본형 guard의 각 뜻이 guards에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `highlighted` | 채택 | input 제거 2212; highlight에서 상속 5개념 | marked의 스트림 watermark 진행 표식 뜻은 시각적 강조 상태와 다름 / 기본형 highlight의 각 뜻이 highlighted에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `highlighter` | 채택 | 개념 5944 이동 t13.1→t572 | Text search highlighting이 Assertion matchers(testing)에 있음 → Search excerpts로 | `rts3-5b9386bd6787963e` |
| `highlights` | 채택 | highlight에서 상속 4개념 | 기본형 highlight의 각 뜻이 highlights에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `hot keys` | 채택 | input 제거 91; hotkey에서 상속 2개념 | often→hotKeys 연결이 반복 트리거 지연(debounce)으로 분류된 오류 / 기본형 hotkey의 각 뜻이 hot keys에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `identifiers` | 채택 | input 제거 3222; identifier에서 상속 12개념; own 추가 6826 (대상 identifier) | keys/names의 식별자 뜻을 자연어 단어 사전 주제로 잘못 연결 / 기본형 identifier의 각 뜻이 identifiers에도 성립(own/input 출처 유지) / 조치 대상은 기본형 identifier. forXMLEncIdentifier(String xmlEncIdentifier)가 단일 XML Encryption 알고리즘 식별자를 다룸 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `identities` | 채택 | input 제거 918 | HTTP identity 인코딩 토큰은 identities 복수형으로 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `idioms` | 채택 | idiom에서 상속 2개념 | 기본형 idiom의 각 뜻이 idioms에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `idxs` | 채택 | own 추가 2219 | depIdxs의 idxs = indices, Indexed element access의 index/indices 구성원 | `rts3-5b9386bd6787963e` |
| `images` | 채택 | input 제거 4344; image에서 상속 17개념 | 상품(product) 원천이 Hive 메타데이터 catalog 주제에 들어간 오분류 / 기본형 image의 각 뜻이 images에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `imported` | 채택 | import에서 상속 16개념 | 기본형 import의 각 뜻이 imported에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `importing` | 채택 | import에서 상속 16개념 | 기본형 import의 각 뜻이 importing에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `incl` | 채택 | 개념 5532 이동 t89.5→t310.2 | 범위 경계 포함이 web-backend 분야에 있음 → math-numerics Interval boundaries로 | `rts3-5b9386bd6787963e` |
| `individually` | 채택 | own 제거 1426, 1833, 5035 | 부사 individually는 컬렉션 항목·선택 상태·사용자 객체가 아님 | `rts3-a8af20d9b15ab705` |
| `infallible` | 채택 | own 추가 2199 | Infallible = 값이 없는 오류 타입, impossible/never 타입 | `rts3-5b9386bd6787963e` |
| `infos` | 채택 | info에서 상속 4개념 | 기본형 info의 각 뜻이 infos에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `inherited` | 채택 | input 제거 2542, 445; inherit에서 상속 10개념 | 길이 확장(extended) 표현·Markdown em 주제는 상속된 상태의 뜻이 아님 / 기본형 inherit의 각 뜻이 inherited에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `inline scripts` | 채택 | input 제거 1299; inlinescript에서 상속 1개념 | inline의 보이기/숨기기 주제는 내장 JavaScript 소스에 성립하지 않음 / 기본형 inlinescript의 각 뜻이 inline scripts에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `inlined` | 채택 | input 제거 3618; inline에서 상속 8개념 | inserted의 SQL upsert 뜻은 인라인 선언·치환에 성립하지 않음 / 기본형 inline의 각 뜻이 inlined에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `insights` | 채택 | insight에서 상속 2개념 | 기본형 insight의 각 뜻이 insights에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `installing` | 채택 | input 제거 50; install에서 상속 11개념 | service worker skipWaiting 원천이 VS Code 확장 주제로 분류된 오류 / 기본형 install의 각 뜻이 installing에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `interested` | 채택 | interest에서 상속 1개념 | 기본형 interest의 각 뜻이 interested에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `interests` | 채택 | interest에서 상속 1개념 | 기본형 interest의 각 뜻이 interests에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `invalidations` | 채택 | own 제거 1779 | 캐시 무효화는 공통 helper 추출 리팩터링 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `join handles` | 채택 | own 제거 5007 | JoinHandle은 Result 추출 연산(expect) 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `keyed` | 채택 | own 추가 1911 | .NET keyed service 등록·선택 = provider/binding IoC wiring | `rts3-5b9386bd6787963e` |
| `keystores` | 채택 | 개념 5563 이동 t706.4→t20.6 | keystore는 키·인증서 저장소. build-release 주제보다 keystores를 명시한 Secret stores가 정확 | `rts3-5b9386bd6787963e` |
| `kinesis streams` | 채택 | kinesisstream에서 상속 1개념 | 기본형 kinesisstream의 각 뜻이 kinesis streams에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `latencies` | 채택 | input 제거 3474 | 지연 측정값은 선호·개선 표현 어휘가 아님 | `rts3-a8af20d9b15ab705` |
| `linked issues` | 채택 | linkedissue에서 상속 1개념 | 기본형 linkedissue의 각 뜻이 linked issues에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `loaders` | 채택 | loader에서 상속 10개념 | 기본형 loader의 각 뜻이 loaders에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `managers` | 채택 | manager에서 상속 8개념 | 기본형 manager의 각 뜻이 managers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `margins` | 채택 | margin에서 상속 7개념 | 기본형 margin의 각 뜻이 margins에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `markers` | 채택 | marker에서 상속 8개념 | 기본형 marker의 각 뜻이 markers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `meanings` | 채택 | meaning에서 상속 2개념 | 기본형 meaning의 각 뜻이 meanings에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `measured` | 채택 | measure에서 상속 8개념 | 기본형 measure의 각 뜻이 measured에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `measurements` | 채택 | measurement에서 상속 5개념 | 기본형 measurement의 각 뜻이 measurements에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `megabytes` | 채택 | megabyte에서 상속 1개념 | 기본형 megabyte의 각 뜻이 megabytes에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `menus` | 채택 | menu에서 상속 6개념 | 기본형 menu의 각 뜻이 menus에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `messaging` | 채택 | message에서 상속 11개념 | 기본형 message의 각 뜻이 messaging에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `microtasks` | 채택 | own 제거 1453 | JS microtask는 background worker job queue 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `migrating` | 채택 | migrate에서 상속 5개념 | 기본형 migrate의 각 뜻이 migrating에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `mocked` | 채택 | own 제거 4971 | mock 대역 교체 상태는 변경 되돌리기(undo) 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `module paths` | 채택 | modulepath에서 상속 4개념 | 기본형 modulepath의 각 뜻이 module paths에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `multiple choices` | 채택 | multiplechoice에서 상속 1개념 | 기본형 multiplechoice의 각 뜻이 multiple choices에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `multivalued` | 채택 | multivalue에서 상속 1개념 | 기본형 multivalue의 각 뜻이 multivalued에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `muted` | 채택 | mute에서 상속 1개념 | 기본형 mute의 각 뜻이 muted에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `named queries` | 채택 | namedquery에서 상속 2개념 | 기본형 namedquery의 각 뜻이 named queries에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `namespaced` | 채택 | input 제거 3705; namespace에서 상속 27개념 | state store 원천이 buffered stream I/O 주제에 있어 namespace 분리 뜻과 맞지 않음 / 기본형 namespace의 각 뜻이 namespaced에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `nonspace` | 채택 | own 제거 1601, 3060, 402, 509; own 추가 5366 | 공백 아닌 문자 뜻에 디스크 공간·XML namespace·공간 복잡도·버퍼 용량 뜻을 상속한 오류 / 공백 아닌 문자 판정 = 문자 분류 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `normalized` | 채택 | normalize에서 상속 14개념; own 추가 891 | 기본형 normalize의 각 뜻이 normalized에도 성립(own/input 출처 유지) / 기대 타입으로 (int) 변환해 정규화한 값 = 명시적 타입 변환 | `rts3-5b9386bd6787963e` |
| `not allowed` | 채택 | notallow에서 상속 1개념 | 기본형 notallow의 각 뜻이 not allowed에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `notifying` | 채택 | notify에서 상속 12개념 | 기본형 notify의 각 뜻이 notifying에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `orphaned` | 채택 | input 제거 748; orphan에서 상속 3개념; own 추가 6486 | 담당자 부재(assignee) 원천이 commit 작성자 주제로 분류되어 orphaned에 성립하지 않음 / 기본형 orphan의 각 뜻이 orphaned에도 성립(own/input 출처 유지) / 동일 source 부모 프로필이 사라진 상태 = profile inheritance 부모 유효성 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `orphans` | 채택 | own 제거 3845 | 부모 잃은 대상은 이동·소유권 이전 표현 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `overflows` | 채택 | overflow에서 상속 43개념 | 기본형 overflow의 각 뜻이 overflows에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `packages` | 채택 | package에서 상속 15개념 | 기본형 package의 각 뜻이 packages에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `packets` | 채택 | packet에서 상속 5개념 | 기본형 packet의 각 뜻이 packets에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `paged` | 채택 | own 제거 2300, 3252, 486, 6981 | 페이지 나눔 상태에 오류 페이지·화면 캡처·E2E 테스트·web page 명사 뜻을 상속한 오류 | `rts3-a8af20d9b15ab705` |
| `participants` | 채택 | input 제거 983; participant에서 상속 2개념 | party의 third-party 패키지 뜻과 참여자 뜻의 동음이의 혼동 / 기본형 participant의 각 뜻이 participants에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `patches` | 채택 | input 제거 5394; patch에서 상속 28개념 | 확장 기여 지점(contribution point) 선언은 OSS 패치 기여와 다름 / 기본형 patch의 각 뜻이 patches에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `patching` | 채택 | input 제거 5245; patch에서 상속 26개념 | modifying의 읽기/쓰기 잠금 주제는 패치 적용 동작에 성립하지 않음 / 기본형 patch의 각 뜻이 patching에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `paused` | 채택 | input 제거 3219; pause에서 상속 5개념 | stopped의 완료 상태 주제는 재개 가능한 일시 중지와 다름 / 기본형 pause의 각 뜻이 paused에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `payment gateways` | 채택 | own 제거 4092 | 결제 gateway는 API key 발급·회전 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `payment intents` | 채택 | own 제거 4419 | 결제 객체는 실행 상태 값·상태 전이 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `peers` | 채택 | input 제거 1723, 4334; peer에서 상속 8개념 | ips의 문제 해결 표현·others의 spread 인자 주제는 통신 상대에 성립하지 않음 / 기본형 peer의 각 뜻이 peers에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `phases` | 채택 | input 제거 5095; phase에서 상속 5개념 | AWS CodeBuild 빌드 단계 원천이 비밀 저장소 주제에 분류된 오류 / 기본형 phase의 각 뜻이 phases에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `picks` | 채택 | pick에서 상속 5개념 | 기본형 pick의 각 뜻이 picks에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `pointer entered` | 채택 | pointerenter에서 상속 1개념 | 기본형 pointerenter의 각 뜻이 pointer entered에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `polls` | 채택 | own 추가 256 | 반복 HTTP poll 요청 = request polling | `rts3-5b9386bd6787963e` |
| `popularity` | 채택 | own 제거 4941 | 인기도 지표는 선언적 UI 렌더링 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `popups` | 채택 | popup에서 상속 3개념 | 기본형 popup의 각 뜻이 popups에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `positioner` | 채택 | input 제거 1962, 2178, 3177, 3409 | 그래프 경로 탐색·Docker 레이어·설정 우선순위·서수 복수형 주제는 배치 컴포넌트에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `prelogin` | 채택 | own 제거 1765, 4245 | 로그인 준비 단계는 비밀번호 복구·Firebase Auth 제품 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `preprocessed` | 채택 | preprocess에서 상속 9개념 | 기본형 preprocess의 각 뜻이 preprocessed에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `preprocessing` | 채택 | preprocess에서 상속 1개념 | 기본형 preprocess의 각 뜻이 preprocessing에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `presort` | 채택 | input 제거 1372 | dependent type의 sort 뜻은 사전 정렬에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `presorted` | 채택 | input 제거 1372 | dependent type의 sort 뜻은 사전 정렬 상태에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `previews` | 채택 | preview에서 상속 6개념 | 기본형 preview의 각 뜻이 previews에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `prioritized` | 채택 | prioritize에서 상속 1개념 | 기본형 prioritize의 각 뜻이 prioritized에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `processors` | 채택 | processor에서 상속 6개념 | 기본형 processor의 각 뜻이 processors에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `product categories` | 채택 | productcategory에서 상속 1개념 | 기본형 productcategory의 각 뜻이 product categories에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `product types` | 채택 | producttype에서 상속 4개념 | 기본형 producttype의 각 뜻이 product types에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `projected` | 채택 | project에서 상속 2개념; own 추가 4863 | 기본형 project의 각 뜻이 projected에도 성립(own/input 출처 유지) / projection matrix·좌표 이동 결과 = vector coordinate transforms | `rts3-5b9386bd6787963e` |
| `promoted` | 채택 | promote에서 상속 4개념 | 기본형 promote의 각 뜻이 promoted에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `proofs` | 채택 | proof에서 상속 4개념 | 기본형 proof의 각 뜻이 proofs에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `prorations` | 채택 | proration에서 상속 1개념 | 기본형 proration의 각 뜻이 prorations에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `public keys` | 채택 | publickey에서 상속 11개념 | 기본형 publickey의 각 뜻이 public keys에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `purchasable` | 채택 | own 추가 3394 | 구매 가능한 구독 플랜 구성 요소(좌석·저장공간) | `rts3-5b9386bd6787963e` |
| `purchased` | 채택 | own 추가 3394 | 구매 확보한 구독 좌석 상태·수량 | `rts3-5b9386bd6787963e` |
| `purges` | 채택 | purge에서 상속 10개념 | 기본형 purge의 각 뜻이 purges에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `qualifiers` | 채택 | qualifier에서 상속 4개념 | 기본형 qualifier의 각 뜻이 qualifiers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `quick fixes` | 채택 | quickfix에서 상속 6개념 | 기본형 quickfix의 각 뜻이 quick fixes에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `rates` | 채택 | own 추가 4664 | 세율·세율 규칙 = tax 계산 구성 요소 | `rts3-5b9386bd6787963e` |
| `rating` | 채택 | input 제거 4344 | marketplace 원천이 Hive 메타데이터 catalog 주제에 분류된 오류 | `rts3-a8af20d9b15ab705` |
| `ratings` | 채택 | rating에서 상속 3개념 | 기본형 rating의 각 뜻이 ratings에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `react testing library` | 채택 | reacttest에서 상속 1개념 | 기본형 reacttest의 각 뜻이 react testing library에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `rects` | 채택 | rect에서 상속 7개념 | 기본형 rect의 각 뜻이 rects에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `redisplay` | 채택 | own 제거 1024, 1298, 1956 | display의 CSS 속성·화면 장치 뜻은 다시 그리기 동사에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `reflectors` | 채택 | reflector에서 상속 1개념 | 기본형 reflector의 각 뜻이 reflectors에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `refreshes` | 채택 | refresh에서 상속 14개념 | 기본형 refresh의 각 뜻이 refreshes에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `registered users` | 채택 | input 제거 4171 | 서비스 등록(service registration) 뜻을 사람 계정 등록으로 잘못 연결 | `rts3-a8af20d9b15ab705` |
| `registries` | 채택 | registry에서 상속 25개념 | 기본형 registry의 각 뜻이 registries에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `remap` | 채택 | own 제거 2730, 3183, 5185 | map의 컬렉션 타입(membership·sorted map·hash map) 뜻은 재대응 동작에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `remappings` | 채택 | remapping에서 상속 1개념 | 기본형 remapping의 각 뜻이 remappings에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `remover` | 채택 | own 제거 1638 | 제거 수행자는 cascading 제어 흐름 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `replacements` | 채택 | replacement에서 상속 4개념 | 기본형 replacement의 각 뜻이 replacements에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `replays` | 채택 | replay에서 상속 7개념 | 기본형 replay의 각 뜻이 replays에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `replicated` | 채택 | replicate에서 상속 4개념 | 기본형 replicate의 각 뜻이 replicated에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `reply to addresses` | 채택 | replytoaddress에서 상속 1개념 | 기본형 replytoaddress의 각 뜻이 reply to addresses에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `representations` | 채택 | representation에서 상속 6개념 | 기본형 representation의 각 뜻이 representations에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `reprocessing` | 채택 | reprocess에서 상속 3개념 | 기본형 reprocess의 각 뜻이 reprocessing에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `resolvers` | 채택 | resolver에서 상속 12개념 | 기본형 resolver의 각 뜻이 resolvers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `resolving` | 채택 | input 제거 1662; resolve에서 상속 27개념 | SQL RETURNING 원천은 데이터 resolver 진행 상태와 다름 / 기본형 resolve의 각 뜻이 resolving에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `resource types` | 채택 | resourcetype에서 상속 2개념 | 기본형 resourcetype의 각 뜻이 resource types에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `restrictions` | 채택 | restriction에서 상속 4개념 | 기본형 restriction의 각 뜻이 restrictions에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `reviews` | 채택 | review에서 상속 3개념 | 기본형 review의 각 뜻이 reviews에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `revisioned` | 채택 | own 제거 1895, 4206, 5219 | revision 명사의 commit·sha·svn 뜻은 revision을 갖는 객체 상태에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `revisions` | 채택 | revision에서 상속 11개념 | 기본형 revision의 각 뜻이 revisions에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `rewire` | 채택 | own 제거 1781, 3572 | wire의 패킷 framing·직렬화 형식 뜻은 재연결 동사에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `rewrites` | 채택 | rewrite에서 상속 10개념 | 기본형 rewrite의 각 뜻이 rewrites에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `rewritten` | 채택 | input 제거 1806, 4863 | 완전성 표현·좌표 변환 주제는 재작성된 상태에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `rlim` | 채택 | own 제거 4667 | 프로세스 자원 한도는 재귀 제어 흐름 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `rngs` | 채택 | rng에서 상속 1개념 | 기본형 rng의 각 뜻이 rngs에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `ru` | 채택 | own 추가 3377 | struct rusage의 ru_ 필드 조각, rusage가 own인 개념. 기존 input 승격이 아닌 직접 근거 | `rts3-5b9386bd6787963e` |
| `samples` | 채택 | sample에서 상속 6개념 | 기본형 sample의 각 뜻이 samples에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `sanitizations` | 채택 | own 추가 6344 | RDB/RESTORE deep integrity 검증 횟수 = binary structure integrity | `rts3-5b9386bd6787963e` |
| `scalars` | 채택 | input 제거 3659; scalar에서 상속 7개념 | primitives의 headless UI primitive 뜻과 데이터 scalar의 혼동 / 기본형 scalar의 각 뜻이 scalars에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `schemes` | 채택 | scheme에서 상속 6개념 | 기본형 scheme의 각 뜻이 schemes에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `scrolling` | 채택 | own 추가 6770 | DECSTBM/DECSLRM 스크롤 영역 = terminal scrolling | `rts3-5b9386bd6787963e` |
| `senders` | 채택 | own 제거 1191, 2947, 4309 | 발신자는 배송 상태·Perl 선언·컴파일러 emitter 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `services` | 채택 | service에서 상속 21개념 | 기본형 service의 각 뜻이 services에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `setup intents` | 채택 | setupintent에서 상속 3개념 | 기본형 setupintent의 각 뜻이 setup intents에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `short date string` | 채택 | datestr에서 상속 3개념 | 기본형 datestr의 각 뜻이 short date string에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `sides` | 채택 | own 제거 2329; own 추가 465 | side effects 고정 문구의 앞 조각을 sides 복수형으로 상속한 오류 / 방향별 padding 가장자리 = padding 구성 요소 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `signed cookies` | 채택 | signedcookie에서 상속 4개념 | 기본형 signedcookie의 각 뜻이 signed cookies에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `sizer` | 채택 | own 제거 4275, 4456 | 크기 산정 주체는 offset 페이지 값·std::queue API 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `sizes` | 채택 | size에서 상속 17개념 | 기본형 size의 각 뜻이 sizes에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `slaves` | 채택 | input 제거 30 | whitelist 원천의 CORS 주제는 replica 서버 뜻에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `slotted` | 채택 | input 제거 4261 | connection/single의 단일성 표현 주제는 슬롯 배치 상태에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `snippets` | 채택 | snippet에서 상속 2개념 | 기본형 snippet의 각 뜻이 snippets에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `sounds` | 채택 | sound에서 상속 2개념; 개념 5964 이동 t528.2→t36.2; own 추가 5964 | 기본형 sound의 각 뜻이 sounds에도 성립(own/input 출처 유지) / MIDI notes가 event emitter 주제에 있음 → Media player controls로 / DECPS가 재생하는 MIDI 음 = MIDI notes | `rts3-5b9386bd6787963e` |
| `source files` | 채택 | sourcefile에서 상속 7개념 | 기본형 sourcefile의 각 뜻이 source files에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `sourced` | 채택 | own 제거 180, 2731, 3789, 4685, 5015, 5180, 526, 6991; input 제거 1250, 2313, 4035, 4211, 4311, 4526, 4533, 4644, 6007, 602 | 명사 source의 여러 기술 뜻(Argo CD·source map·regex·빌드·브랜치·진단·Tailwind 등)을 과거분사에 일괄 상속한 오류 | `rts3-a8af20d9b15ab705` |
| `sources` | 채택 | source에서 상속 33개념 | 기본형 source의 각 뜻이 sources에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `spaced` | 채택 | space에서 상속 1개념 | 기본형 space의 각 뜻이 spaced에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `specifiers` | 채택 | specifier에서 상속 1개념 | 기본형 specifier의 각 뜻이 specifiers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `specs` | 채택 | spec에서 상속 6개념; own 추가 1763 | 기본형 spec의 각 뜻이 specs에도 성립(own/input 출처 유지) / 원격 JSON 설정 명세 = remote config | `rts3-5b9386bd6787963e` |
| `spis` | 채택 | spi에서 상속 7개념; 개념 6871 이동 t314→t42.1 | 기본형 spi의 각 뜻이 spis에도 성립(own/input 출처 유지) / service provider interface는 provider 로딩·해결을 다루는 Plugin lifecycle이 정확 | `rts3-5b9386bd6787963e` |
| `sponsored` | 채택 | input 제거 56; sponsor에서 상속 2개념 | ad 원천에 광고 후원 뜻이 Active Directory/LDAP 주제로 섞인 오분류 / 기본형 sponsor의 각 뜻이 sponsored에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `sponsoring` | 채택 | own 추가 6196 | 구독 후원 제공 측 = subscription sponsorship 구성 역할 | `rts3-5b9386bd6787963e` |
| `stars` | 채택 | star에서 상속 2개념 | 기본형 star의 각 뜻이 stars에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `stashing` | 채택 | stash에서 상속 3개념 | 기본형 stash의 각 뜻이 stashing에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `states` | 채택 | state에서 상속 40개념; 새 개념 "Administrative address subdivisions"; own 추가 1833; own 추가 6400; own 추가 6686 | 기본형 state의 각 뜻이 states에도 성립(own/input 출처 유지) / WooCommerce STATES = 국가별 주/도. 주소 전체·배송 zone과 다른 대상 / editor selection 상태들 = selection 상태 용어 / 협업 awareness client 상태들 / TCP conn_states(close-wait 등) = TCP connection state | `rts3-5b9386bd6787963e`, `rts3-554bf1373eb43b73` |
| `statics` | 채택 | own 제거 2364, 3384, 4880, 5018, 891; 새 개념 "WinRT static-member interfaces" | static 형용사·고정 문구 뜻은 statics 명사에 성립하지 않음 / IAppExtensionCatalogStatics = WinRT 정적 멤버 interface. 기존 개념으로 표현 불가 | `rts3-a8af20d9b15ab705`, `rts3-554bf1373eb43b73` |
| `stepped` | 채택 | step에서 상속 7개념 | 기본형 step의 각 뜻이 stepped에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `stime` | 채택 | 새 개념 "system CPU time" | ru_stime = 커널 모드 CPU 누계. user CPU time(6780)과 별개 | `rts3-554bf1373eb43b73` |
| `string values` | 채택 | stringvalue에서 상속 2개념 | 기본형 stringvalue의 각 뜻이 string values에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `stubs` | 채택 | stub에서 상속 9개념 | 기본형 stub의 각 뜻이 stubs에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `subflow` | 채택 | own 제거 3628 | Flow 타입 검사 도구 뜻은 하위 흐름 subflow에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `subkeys` | 채택 | subkey에서 상속 2개념 | 기본형 subkey의 각 뜻이 subkeys에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `subsections` | 채택 | 새 개념 "form subsection" | 상품 폼의 하위 구획 객체. 기존 form control/heading과 다른 대상 | `rts3-554bf1373eb43b73` |
| `subtotals` | 채택 | input 제거 3459; own 추가 4842 | 비동기 waterfall 원천의 이벤트 큐 주제는 소계에 성립하지 않음 / 구간별 부분 집계 = total(집계 결과). 기존 sum input 승격이 아닌 직접 근거 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `subvalue` | 채택 | own 제거 3300, 5083 | 하위 값은 C++ 값 전달 방식·val 불변 선언 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `suggestions` | 채택 | input 제거 101; suggestion에서 상속 7개념; own 추가 7132 | suggested change를 권한 승인 표현 주제로 읽은 오분류 / 기본형 suggestion의 각 뜻이 suggestions에도 성립(own/input 출처 유지) / 없는 명령의 WinGet 설치 제안 = shell command correction | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `sweeps` | 채택 | sweep에서 상속 2개념 | 기본형 sweep의 각 뜻이 sweeps에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `synced` | 채택 | sync에서 상속 5개념 | 기본형 sync의 각 뜻이 synced에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `tapped` | 채택 | own 제거 3001, 4896; input 제거 962 | TAP 네트워크 인터페이스·VPN·JTAG TAP 명칭은 tap 동사 과거분사에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `targets` | 채택 | target에서 상속 24개념; own 추가 6166 | 기본형 target의 각 뜻이 targets에도 성립(own/input 출처 유지) / 매출·주문 목표값 = business performance goal | `rts3-5b9386bd6787963e` |
| `taxonomies` | 채택 | taxonomy에서 상속 5개념 | 기본형 taxonomy의 각 뜻이 taxonomies에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `terminals` | 채택 | own 제거 6774 | 종결 상태 형용사 terminal 뜻은 복수 명사 terminals에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `texts` | 채택 | own 추가 2630 | locale별 메시지 문자열 = translation catalog 내용 | `rts3-5b9386bd6787963e` |
| `tokenable` | 채택 | own 제거 1370, 1435, 4485, 4616, 4819, 570, 6277; input 제거 1597, 1658, 1957, 2420, 2679, 3152, 3287, 3341, 3464, 4487, 4520, 4528, 4690; own 추가 6110 | token의 어휘 단위·LLM·디자인 토큰·취소 토큰 등 명사 뜻을 보호 payload 능력형에 일괄 상속한 오류 / 보호 토큰으로 직렬화 가능한 payload 모델 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `tool results` | 채택 | toolresult에서 상속 1개념 | 기본형 toolresult의 각 뜻이 tool results에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `total discount amounts` | 채택 | discountamount에서 상속 2개념 | 기본형 discountamount의 각 뜻이 total discount amounts에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `trace events` | 채택 | traceevent에서 상속 1개념 | 기본형 traceevent의 각 뜻이 trace events에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `trace logging` | 채택 | tracelog에서 상속 1개념 | 기본형 tracelog의 각 뜻이 trace logging에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `traceable` | 채택 | own 제거 1014; input 제거 1669, 5412 | HTTP TRACE 메서드·차트 trace/데이터 계열 명사 뜻은 추적 가능 형용사에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `traits` | 채택 | trait에서 상속 10개념 | 기본형 trait의 각 뜻이 traits에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `transforms` | 채택 | transform에서 상속 26개념 | 기본형 transform의 각 뜻이 transforms에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `translators` | 채택 | translator에서 상속 3개념 | 기본형 translator의 각 뜻이 translators에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `transports` | 채택 | transport에서 상속 4개념 | 기본형 transport의 각 뜻이 transports에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `trapping` | 채택 | trap에서 상속 5개념 | 기본형 trap의 각 뜻이 trapping에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `trimmed` | 채택 | own 추가 3754 | 슬롯 이동 후 정리된 키/슬롯 상태 = Redis Cluster trimming | `rts3-5b9386bd6787963e` |
| `trimming` | 채택 | trim에서 상속 4개념 | 기본형 trim의 각 뜻이 trimming에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `trust managers` | 채택 | trustmanager에서 상속 2개념 | 기본형 trustmanager의 각 뜻이 trust managers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `typing` | 채택 | input 제거 2354; own 추가 3628 | entering의 텍스트 입력 뜻이 .NET 서비스 등록 주제로 분류된 오류 / Python 표준 typing 모듈 = Python typing 구성 요소. 기존 input은 유지 | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `unassigned` | 채택 | unassign에서 상속 1개념 | 기본형 unassign의 각 뜻이 unassigned에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `unblocked` | 채택 | unblock에서 상속 2개념 | 기본형 unblock의 각 뜻이 unblocked에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `uncompressed` | 채택 | input 제거 3027, 4529; uncompress에서 상속 1개념 | raw의 개정 원본·unpacked의 바이너리 packing 뜻은 압축 전 상태와 다름 / 기본형 uncompress의 각 뜻이 uncompressed에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `underlined` | 채택 | own 제거 4453 | 밑줄 장식 상태는 sequence span 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `units` | 채택 | unit에서 상속 9개념 | 기본형 unit의 각 뜻이 units에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `unlinked` | 채택 | unlink에서 상속 5개념 | 기본형 unlink의 각 뜻이 unlinked에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `unloaded` | 채택 | own 추가 6500 | XAML Unloaded 생명주기 = native control lifecycle | `rts3-5b9386bd6787963e` |
| `unparsed` | 채택 | input 제거 3027; unparse에서 상속 1개념 | raw의 개정 원본 주제는 해석 전 값과 다름 / 기본형 unparse의 각 뜻이 unparsed에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `unshared` | 채택 | input 제거 2093, 341, 3459, 3681, 5198, 821 | share의 몫·폴더·RxJS 스케줄링·LAN broadcast·ASG·template partial 원천 주제는 비공유 상태에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `unsign` | 채택 | own 제거 4245 | sign in(로그인) 뜻은 서명 해제 unsign에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `unslash` | 채택 | own 제거 341, 5018 | slash의 경로 구분자 뜻은 escape backslash 제거에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `unsubscribed` | 채택 | unsubscribe에서 상속 13개념 | 기본형 unsubscribe의 각 뜻이 unsubscribed에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `untag` | 채택 | own 추가 1226 | 태그된 JSON을 원 값으로 복원 = deserialize. 기존 input은 유지 | `rts3-5b9386bd6787963e` |
| `untrash` | 채택 | input 제거 3613 | garbage 원천의 패키지 purge 주제는 휴지통 복원과 다름 | `rts3-a8af20d9b15ab705` |
| `unwatched` | 채택 | own 제거 4944 | Apple Watch 제품 뜻은 감시 해제 상태에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `upsells` | 채택 | upsell에서 상속 1개념 | 기본형 upsell의 각 뜻이 upsells에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `utilities` | 채택 | utility에서 상속 2개념; own 추가 2048 | 기본형 utility의 각 뜻이 utilities에도 성립(own/input 출처 유지) / 테스트 요청 fixture helper = test helper | `rts3-5b9386bd6787963e` |
| `value changed` | 채택 | input 제거 16; valuechange에서 상속 1개념 | change 원천이 이전 위치(previous) 표현 주제로 분류된 오류 / 기본형 valuechange의 각 뜻이 value changed에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `valued` | 채택 | own 제거 3300, 5083 | 값을 갖는 수식형은 값 전달 방식·val 선언 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `variants` | 채택 | input 제거 3239, 4344; variant에서 상속 14개념 | various의 import 주제·product의 Hive catalog 주제는 변형들에 성립하지 않음 / 기본형 variant의 각 뜻이 variants에도 성립(own/input 출처 유지) | `rts3-a8af20d9b15ab705`, `rts3-5b9386bd6787963e` |
| `variations` | 채택 | variation에서 상속 3개념; 개념 6441 이동 t657.1→x41 | 기본형 variation의 각 뜻이 variations에도 성립(own/input 출처 유지) / block variation이 Rich text editors에 있음 → WordPress block editing으로 | `rts3-5b9386bd6787963e` |
| `vectored` | 채택 | vector에서 상속 1개념 | 기본형 vector의 각 뜻이 vectored에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `verifiable credentials` | 채택 | input 제거 2807 | vc 약어의 버전 관리(VCS) 뜻과 검증 가능한 자격증명의 혼동 | `rts3-a8af20d9b15ab705` |
| `viewers` | 채택 | viewer에서 상속 4개념 | 기본형 viewer의 각 뜻이 viewers에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `viewports` | 채택 | input 제거 2395 | template 렌더링 주제는 보이는 공간 영역 viewport에 성립하지 않음 | `rts3-a8af20d9b15ab705` |
| `virtualize` | 채택 | own 제거 486 | 가상화 동작은 브라우저 E2E 테스트 개념의 구성원이 아님 | `rts3-a8af20d9b15ab705` |
| `waitable` | 채택 | own 제거 2040, 2491; 새 개념 "waitable synchronization object" | 대기 가능한 동기화 객체는 watchdog·인증 cooldown 개념의 구성원이 아님 / GetFrameLatencyWaitableObject의 기다릴 수 있는 동기화 객체 | `rts3-a8af20d9b15ab705`, `rts3-554bf1373eb43b73` |
| `waiters` | 채택 | waiter에서 상속 1개념 | 기본형 waiter의 각 뜻이 waiters에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `walks` | 채택 | walk에서 상속 6개념 | 기본형 walk의 각 뜻이 walks에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `warns` | 채택 | own 추가 6903 | pytest.warns 경고 검사 = warning assertions | `rts3-5b9386bd6787963e` |
| `wcs` | 채택 | own 추가 4716; own 추가 6230 | wcslen/wcscmp의 wide character string / WCS&T 제품 약어 = woocommerce shipping and tax | `rts3-5b9386bd6787963e` |
| `weighted` | 채택 | weight에서 상속 7개념; 개념 6670 이동 t81.2→x25 | 기본형 weight의 각 뜻이 weighted에도 성립(own/input 출처 유지) / exponential moving average는 Distribution moments보다 Exponential smoothing이 정확 | `rts3-5b9386bd6787963e` |
| `windowed` | 채택 | window에서 상속 9개념 | 기본형 window의 각 뜻이 windowed에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `windowing` | 채택 | window에서 상속 3개념 | 기본형 window의 각 뜻이 windowing에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `woken` | 채택 | own 추가 5170 | wake 통지를 받은 future 상태 = waker 메커니즘 | `rts3-5b9386bd6787963e` |
| `zones` | 채택 | zone에서 상속 5개념 | 기본형 zone의 각 뜻이 zones에도 성립(own/input 출처 유지) | `rts3-5b9386bd6787963e` |
| `zoomed` | 채택 | 개념 6034 이동 t15.1→t275.3 | View magnification이 Access grants(auth-identity)에 있음 → Pan and zoom으로 | `rts3-5b9386bd6787963e` |

## R2 조치별 결정

| 단어 | 제안 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- | --- |
| `autosaves` | analyst | input 제거 5241 | 채택 | 원천 saved persona의 주제는 캐시 dirty 값 write-back. WordPress 자동 저장 revision은 캐시 write-behind가 아님(crosscheck도 성립 불확실로 봄) | `rts3-b5fb0c137c71a08f` |
| `avgs` | crosscheck | own 추가 4382 | 채택 | redis hnsw.c row_avgs = sum/valid_count 산술 평균. mean/average 구성 개념에 성립 | `rts3-b5fb0c137c71a08f` |
| `cacher` | crosscheck | own 제거 1530 | 보류 | 1530은 하드웨어 cache line과 read-through 등 소프트웨어 캐시가 섞인 개념. 기본형 cache도 own이라 단독 제거 근거 부족 | 미적용 |
| `catcher` | crosscheck | own 추가 3891 | 채택 | flask tests: @app.errorhandler(413) def catcher(error) — 등록된 error handler callback | `rts3-b5fb0c137c71a08f` |
| `clamped` | crosscheck | own 제거 1595 | 기각 | terminal unwrap_coord_clamped의 포화 후 narrowing 변환은 타입 변환 기법. 기본형 clamp도 같은 개념 own | 미적용 |
| `colons` | crosscheck | own 추가 5018 | 채택 | gin tree.go:81 colons := strings.Count(path, ":") — 이름 있는 라우트 파라미터 표식. slash와 같은 route 구문 구성원 | `rts3-b5fb0c137c71a08f` |
| `completed task` | analyst | completed에서 1574 상속 | 보류 | 복합 API 이름(Task.CompletedTask)에 수식어 completed의 상태 input을 상속할지는 원칙 경계. 한쪽 제안만 있음 | 미적용 |
| `completed task` | analyst | completed에서 3219 상속 | 보류 | 위와 같음 | 미적용 |
| `completed task` | analyst | completed에서 3535 상속 | 보류 | 위와 같음 | 미적용 |
| `completed task` | analyst | completed에서 3551 상속 | 보류 | 위와 같음 | 미적용 |
| `completed task` | analyst | completed에서 361 상속 | 보류 | 위와 같음 | 미적용 |
| `completed task` | analyst | completed에서 4199 상속 | 보류 | 위와 같음 | 미적용 |
| `completed task` | analyst | completed에서 4419 상속 | 보류 | 위와 같음 | 미적용 |
| `completed task` | analyst | completed에서 5168 상속 | 보류 | 위와 같음 | 미적용 |
| `corners` | analyst | own 추가 3811 | 채택 | sentry snapPreviewToCorners: 선택한 corner로 left/top 배치. top/bottom/side/anchor와 같은 배치 기준 구성원 | `rts3-b5fb0c137c71a08f` |
| `corners` | crosscheck | own 추가 460 | 채택 | terminal host/utils.cpp: 사각형의 반대편 corner를 til::point로 반환 — 기하 위치점 | `rts3-b5fb0c137c71a08f` |
| `crashing` | crosscheck | own 제거 2004 | 보류 | 기본형 crash가 같은 stall 개념 own. 파생형만 제거하면 불일치. 한쪽은 제거, 한쪽은 불확실 | 미적용 |
| `cvt` | analyst | own 추가 5254 | 채택 | redis hnsw.c _mm512_cvtepi8_epi16, _mm_cvtss_f32 — SIMD 변환 intrinsic 조각. epi16/m512i 등과 같은 구성원 | `rts3-b5fb0c137c71a08f` |
| `cvt` | analyst | own 제거 3966 | 보류 | cvt=convert 일반 약어의 decimal 변환 뜻 성립 여부를 확정할 근거 부족(한쪽 제거, 한쪽 불확실) | 미적용 |
| `cvt` | analyst | own 제거 3969 | 기각 | std::codecvt 등 cvt의 인코딩 변환 용법이 성립 | 미적용 |
| `cvt` | analyst | own 제거 4863 | 보류 | 좌표 변환 뜻 성립 여부 근거 부족(한쪽 제거, 한쪽 불확실) | 미적용 |
| `cvt` | crosscheck | 새 개념 "TrueType control value table"(t537) 단어 cvt | 채택 | gutenberg lib-font.browser.js:3237 class cvt extends SimpleTable — OpenType 공개 표 태그. 기존 폰트 개념(글리프·타이포그래피)으로 표현 불가 | `rts3-3b085066fb651e66` |
| `dbs` | crosscheck | input 제거 223 | 채택 | 원천 asm/NASM persona의 db는 define-byte 지시자(db/dw/dd). databases 약어 dbs에 지시자 복수 용법 근거 없음(analyst도 근거 미확인) | `rts3-b5fb0c137c71a08f` |
| `disconnection` | crosscheck | own 제거 5000 | 채택 | 중재(p1-arbitrate-006) 채택. 소켓 연결 끊김은 뉴스레터 수신 거부(t753)가 아님. 의존 0 | `rts3-b5fb0c137c71a08f` |
| `dismissal` | analyst | 새 개념 "payment provider incentive dismissal"(t170.2) 단어 dismissal | 채택 | 중재 채택. WooCommerce incentive의 context/timestamp 거절 기록(Incentive.php:166-209). 기존 개념으로 표현 불가 | `rts3-3b085066fb651e66` |
| `dismissal` | crosscheck | own 추가 5637 | 보류 | 뜻은 맞지만 t141.2 주제의 분야가 auth-identity로 잘못됨(A3). 중재도 분야 검토 전 보류 | 미적용 |
| `dismissal` | crosscheck | own 추가 6501 | 채택 | 중재 채택. Gutenberg iframe dismissal bridge의 메뉴 닫기 사건 = popup closing event | `rts3-b5fb0c137c71a08f` |
| `dismissal` | crosscheck | own 제거 5131 | 기각 | redis dismissMemory(server.c:7577-7594)의 페이지 반환 뜻이 명사형에 성립. 중재도 유지 | 미적용 |
| `dismissals` | analyst | own 추가 새 개념 | 채택 | 중재 채택. _dismissals 배열 = 복수 incentive dismissal 기록. 단수 신규 개념 참조 | `rts3-3b085066fb651e66` |
| `dismissals` | crosscheck | own 추가 5637 | 보류 | dismissal과 같은 A3 분야 결함으로 보류 | 미적용 |
| `dismissals` | crosscheck | own 추가 6501 | 채택 | 중재 채택. popup 닫기 사건의 복수 | `rts3-b5fb0c137c71a08f` |
| `dismissals` | crosscheck | own 제거 5131 | 기각 | 메모리 페이지 반환 뜻의 복수 명사형 성립. 중재도 유지 | 미적용 |
| `flashed` | analyst | own 추가 6346 | 채택 | Flask 공개 API get_flashed_messages/message_flashed — flashed messages. flashes(2단계 추가)와 같은 flash message 구성원 | `rts3-b5fb0c137c71a08f` |
| `fmadd` | crosscheck | own 추가 5254 | 채택 | redis hnsw.c:310 _mm512_fmadd_ps — SIMD intrinsic 조각(cvt·epi16과 동일 기준) | `rts3-b5fb0c137c71a08f` |
| `fmadd` | analyst | own 추가 6675 | 채택 | fused multiply-add는 multiply-add 연산. 개념명·주제(SIMD processing)에 성립 | `rts3-b5fb0c137c71a08f` |
| `fmadd` | analyst | own 추가 6675 (대상 fusedmultiplyadd) | 보류 | fusedmultiplyadd는 표제어가 없어 새 단어 생성이 필요. 12개 저장소 수집 단어인지 미확인 | 미적용 |
| `froze` | analyst | 새 개념 "JSON codec configuration finalization"(t289.1) 단어 froze | 기각 | jsoniter Config.Froze()는 설정 동결. 기존 889(freeze/frozen/froze 포함)로 표현 가능 | 미적용 |
| `hijacked` | crosscheck | own 제거 2867 | 기각 | 2867은 hijack·attack·MITM·spoofing·intercept 등 TLS 위협어를 포함. 기본형 hijack도 같은 개념 | 미적용 |
| `hovered` | analyst | 새 개념 "pointer hover state"(t491.6) 단어 hovered | 채택 | 양쪽 공통 제안(단어 목록만 차이). mouseenter/leave 상태·terminal hyperlink hover. 하나의 개념으로 생성 | `rts3-3b085066fb651e66` |
| `hovered` | crosscheck | 새 개념 "pointer hover state"(t491.6) 단어 hover, hovered | 채택 | 위 개념에 기본형 hover도 포함(포인터 hover 상태는 기본형 자체의 뜻) | `rts3-3b085066fb651e66` |
| `hovered` | crosscheck | own 제거 1833 | 기각 | 기본형 hover가 같은 개념 own. highlight/active 상태와 함께 다루는 개념 | 미적용 |
| `impersonated` | analyst | 새 개념 "user impersonation"(t141.1) 단어 impersonate, impersonated | 채택 | 양쪽 모두 허가된 사용자 가장(Keycloak admin impersonation) 새 개념 필요에 동의. UserResource.java:393-428 세션 생성 기능이라 Login sessions 주제 선택. 기존 2064(공격)·3524(특권 상승)와 다른 대상 | `rts3-3b085066fb651e66` |
| `impersonated` | crosscheck | 새 개념 "authorized identity impersonation"(t61) 단어 impersonated | 기각 | 같은 뜻의 대안 제안. user impersonation(t141.1) 하나로 합쳐 채택 | 미적용 |
| `installations` | crosscheck | 새 개념 "registered server installation"(new:self-hosted-installation-registration) 단어 installation, installations | 채택 | 양쪽 모두 새 개념·새 주제 필요에 동의(이름만 다름). Bitwarden Installation.cs:9-22 자가 호스팅 서버 등록 레코드. 기존 패키지 설치·앱 설치·어댑터 설정 개념과 다른 대상 | `rts3-3b085066fb651e66` |
| `installations` | crosscheck | 새 주제 "Self-hosted installation registration" | 채택 | 위 개념의 주제. 기존 주제(t39.8 Package installation, t42.1 Plugin lifecycle 등)는 대상이 다름 | `rts3-3b085066fb651e66` |
| `installations` | analyst | 새 개념 "software installation instance"(new:software-installation-instances) 단어 installations | 기각 | 같은 뜻의 대안 제안. registered server installation 하나로 합쳐 채택 | 미적용 |
| `installations` | analyst | 새 주제 "Software installation instances" | 기각 | 같은 주제의 대안. 하나로 합침 | 미적용 |
| `installations` | analyst | own 제거 6296 | 기각 | Keycloak ClientInstallationProvider/ClientInstallationSpi — 클라이언트 어댑터 설정 'installation'이 실제 뜻 | 미적용 |
| `invalidator` | analyst | own 제거 5241 (대상 invalidate) | 보류 | 기본형 invalidate persona 16847에 native 관계 3·inbound input 1이 있어 재귀속 처분 필요 | 미적용 |
| `invalidator` | analyst | own 제거 5241 | 채택 | 무효화 행위자는 dirty 값 write-behind 기록자가 아님. 의존 0. 기본형의 같은 결함은 보류 | `rts3-b5fb0c137c71a08f` |
| `invalidator` | crosscheck | 개념 5548 이동 t344.2→t48.4 | 채택 | 일반 캐시 무효화 개념이 mobile-desktop 분야 단독 주제에 있음. 같은 이름의 일반 주제 t48.4(databases-storage)로. t344.2에는 5548뿐 | `rts3-b5fb0c137c71a08f` |
| `invalidator` | crosscheck | own 추가 1594 | 채택 | WooCommerce ProductVersionStringInvalidator = 캐시 무효화 구성 요소. invalidation/invalidations와 같은 개념 | `rts3-b5fb0c137c71a08f` |
| `investigations` | analyst | own 추가 6172 | 채택 | 양쪽 모두 6172 own에 동의(한쪽은 상속 형태). sentry investigation notebook 복수 | `rts3-b5fb0c137c71a08f` |
| `investigations` | crosscheck | investigation에서 3957 상속 | 채택 | 기본형 investigation의 rca input(diagnose/rca/rootcause)이 복수형에도 성립 | `rts3-b5fb0c137c71a08f` |
| `investigations` | crosscheck | investigation에서 6172 상속 | 채택 | 위 persona 추가와 같은 결과 | `rts3-b5fb0c137c71a08f` |
| `issuing` | analyst | own 추가 6111 | 채택 | keycloak oid4vc issuance — 검증 가능한 credential 발급 행위. 6104와 같은 주제의 VCI 개념 | `rts3-b5fb0c137c71a08f` |
| `issuing` | crosscheck | own 추가 6104 (대상 issue) | 채택 | 기본형 issue의 발급 동사 뜻이 빠져 있음. credential을 issue하다는 일반 뜻 | `rts3-b5fb0c137c71a08f` |
| `killable` | crosscheck | own 추가 6803 | 채택 | redis makeThreadKillable: PTHREAD_CANCEL_ENABLE/ASYNCHRONOUS로 pthread_cancel 종료 허용 = thread termination | `rts3-b5fb0c137c71a08f` |
| `killable` | analyst | 새 개념 "Native thread cancelability"(t482.2) 단어 killable | 기각 | 같은 주제의 기존 6803 thread termination으로 표현 가능 | 미적용 |
| `killable` | analyst | own 제거 1760 | 기각 | shell job control에서 종료 가능한 작업 뜻 성립. 기본형 kill도 같은 개념 | 미적용 |
| `lagging` | analyst | own 제거 2519 | 보류 | SQL LAG 함수명 vs 시계열 lagging 연산 파생 뜻 성립 여부 미확정(한쪽 제거, 한쪽 불확실) | 미적용 |
| `lockouts` | analyst | own 제거 3920 | 보류 | 기본형 lockout이 같은 retry 개념 own. 시도 한도와 차단의 구성 관계 미확정 | 미적용 |
| `max permits` | analyst | permits에서 836 상속 | 채택 | tokio Semaphore::MAX_PERMITS = semaphore permit 수 상한. permits의 semaphore input 문맥이 성립 | `rts3-b5fb0c137c71a08f` |
| `member names` | crosscheck | membername에서 3842 상속 | 채택 | sentry memberName = member.user.name(팀 구성원 이름). 복수 member names에도 구성원 이름 뜻 성립. input 그대로 | `rts3-b5fb0c137c71a08f` |
| `member names` | crosscheck | own 추가 2774 (대상 membername) | 보류 | membername은 표제어가 없어 새 단어 생성 필요 | 미적용 |
| `member names` | crosscheck | own 추가 7151 (대상 membername) | 보류 | membername 표제어 없음(새 단어 생성 필요) | 미적용 |
| `member names` | crosscheck | own 추가 2774 | 채택 | ValidationResult.MemberNames = 객체 속성 이름, sentry enum member names. member/property가 있는 field 개념 구성원 | `rts3-b5fb0c137c71a08f` |
| `navigated` | analyst | own 추가 6500 | 채택 | terminal XAML Page OnNavigatedTo 생명주기 callback = native control lifecycle(closing/detached/unloaded와 같은 묶음) | `rts3-b5fb0c137c71a08f` |
| `navigated` | crosscheck | 새 개념 "XAML page navigation"(t335.3) 단어 navigated | 기각 | 같은 주제의 기존 6500으로 표현 가능 | 미적용 |
| `negations` | crosscheck | input 제거 2865 | 채택 | 필터 연산자의 논리 부정은 없는 엔티티 응답(not found)이 아님. non 접두 부정에서 번진 연결 | `rts3-b5fb0c137c71a08f` |
| `notifiable` | analyst | input 제거 4757 | 채택 | 원천 dsn(메일 전달 상태 알림)이 입력 변경 throttling 주제에 있음. push 알림 적격성과 무관 | `rts3-b5fb0c137c71a08f` |
| `obfuscated` | crosscheck | own 추가 3309 | 채택 | sentry tempest/serializers.py:10 _obfuscate_client_secret — clientSecret을 별표로 가림 = secret redaction | `rts3-b5fb0c137c71a08f` |
| `overindented` | crosscheck | own 제거 2548 | 기각 | 2548에 indent가 구성원(계층 표시 들여쓰기). 과도한 들여쓰기도 같은 묶음에 성립 | 미적용 |
| `paginating` | crosscheck | own 제거 2709 | 보류 | 기본형 paginate도 같은 batch 개념. pagination과 batch 구성 경계 미확정 | 미적용 |
| `pfn` | crosscheck | own 추가 6581 | 채택 | terminal DelegationConfig.cpp:91-93 get_FamilyName → pfn = Windows Package Family Name | `rts3-b5fb0c137c71a08f` |
| `pointer moved` | analyst | pointermove에서 3381 상속 | 보류 | pointermove(DOM)를 PointerMoved(XAML)의 기본형으로 볼지 경계. 기존 own 7127이 포인터 입력 뜻을 담음 | 미적용 |
| `pointer moved` | analyst | pointermove에서 5665 상속 | 보류 | 위와 같음(7127과 같은 주제 중복) | 미적용 |
| `pointer moved` | analyst | pointermove에서 708 상속 | 보류 | 위와 같음 | 미적용 |
| `polled` | crosscheck | polling에서 859 상속 | 채택 | vector PollWatcher의 설정 파일 polling 감시. file watcher의 polling own 뜻이 polled에 성립(analyst도 유효한 대안으로 인정) | `rts3-b5fb0c137c71a08f` |
| `polled` | crosscheck | input 제거 3427 | 채택 | watch/watcher persona의 파일 감시 polling 관계가 ETag/CAS optimistic revision 주제로 들어온 연결. polled는 revision 갱신이 아님 | `rts3-b5fb0c137c71a08f` |
| `preds` | analyst | own 추가 1269 | 채택 | sentry derived/framework.py:505 predecessors의 preds — DAG 위상 정렬의 선행 노드 집합 | `rts3-b5fb0c137c71a08f` |
| `preds` | analyst | own 추가 3469 | 채택 | redis t_array.c:920 arGrepPredicate *preds = predicates. pred/predicate 구성원 개념 | `rts3-b5fb0c137c71a08f` |
| `preds` | analyst | own 추가 469 | 채택 | vector LLVM backend RFC의 '; preds = %start' — CFG 선행 블록. CFG 개념에 pred 구성원 | `rts3-b5fb0c137c71a08f` |
| `preds` | analyst | own 추가 5204 | 기각 | 같은 주제의 3469 own으로 충분. 기존 input(conditions)을 own으로 두 번 연결하지 않음(승격 회피) | 미적용 |
| `progressed` | analyst | own 제거 3552 | 채택 | 진행 표시기 명사 뜻은 과거형 progressed에 성립하지 않음(명사 뜻 일괄 상속). 의존 0 | `rts3-b5fb0c137c71a08f` |
| `progressed` | analyst | own 제거 3681 | 채택 | 비율·백분율 명사 뜻은 progressed가 아님. 의존 0 | `rts3-b5fb0c137c71a08f` |
| `progressed` | crosscheck | input 제거 2179 | 채택 | onboarding/wizard 흐름 진척이 실패 상태 피드백 주제에 분류된 연결 | `rts3-b5fb0c137c71a08f` |
| `proxify` | analyst | input 제거 473 | 채택 | 원천 PhantomData/phantom type의 Proxy는 타입 표식(Haskell Proxy). JS Proxy로 감싸는 proxify와 무관 | `rts3-b5fb0c137c71a08f` |
| `reattach` | analyst | own 제거 3536 | 기각 | 3536에 attach·attaching·reattach가 함께 있는 PID 기반 debugger 재부착 묶음. crosscheck도 유효 | 미적용 |
| `recurses` | analyst | own 추가 4667 | 채택 | vector merge_into_map_at_path↔merge_values_at_path 상호 재귀 호출. recursive/recursively가 있는 recursion 개념 | `rts3-b5fb0c137c71a08f` |
| `renamer` | crosscheck | 새 개념 "title renaming UI"(t272.1) 단어 renamer | 기각 | Terminal 탭 제목 편집 UI는 프로젝트 내부 기능. 기존 rename 개념(2241 등)으로 일반 뜻 표현 | 미적용 |
| `rethrows` | analyst | own 제거 1511 | 기각 | 1511에 rethrow·error propagation·propagate가 구성원. 예외 재던지기 전파 뜻 성립 | 미적용 |
| `revokable` | analyst | own 제거 1188 | 보류 | 기본형 revoke(revokeObjectURL)가 같은 Item removal 개념. 권한 회수 가능성과 항목 제거의 경계 미확정 | 미적용 |
| `rotated` | analyst | own 추가 2465 | 채택 | Keycloak rotated client secret, Sentry rotate_token(CSRF) — 교체된 credential/token = token rotation | `rts3-b5fb0c137c71a08f` |
| `rxs` | crosscheck | 새 개념 "channel receiver"(t297.1) 단어 rxs | 보류 | 수신 endpoint 개념은 rx·receiver 등과 함께 정해야 하는 범위. 한쪽 제안 | 미적용 |
| `savings` | analyst | saving에서 5249 상속 | 기각 | saving의 쓰기·저장 동작 뜻은 절감액 명사 savings에 성립하지 않음 | 미적용 |
| `savings` | analyst | 새 개념 "execution cost savings"(t401.4) 단어 savings | 보류 | 한쪽이 제안한 신규 개념 3건. 기존 6681(앱 크기 절감) 외 뜻별 분할 필요성 미확정 | 미적용 |
| `savings` | analyst | 새 개념 "memory size savings"(t234.4) 단어 savings | 보류 | 위와 같음 | 미적용 |
| `savings` | analyst | 새 개념 "monetary cost savings"(t111.1) 단어 savings | 보류 | 위와 같음 | 미적용 |
| `selling` | analyst | 새 개념 "merchant selling activity"(x9) 단어 selling | 기각 | 온보딩 설문 항목은 프로젝트 내부 기능. 일반 판매 뜻은 기존 1406 ecommerce | 미적용 |
| `service accounts` | analyst | own 제거 60 | 기각 | 60 범위가 service-account credentials를 명시하고 service account가 구성원 | 미적용 |
| `signs` | analyst | own 추가 5749 | 채택 | vector Azure SharedKey: canonical StringToSign을 HMAC-SHA256으로 서명 = 요청 서명(signature·canonicalizes) | `rts3-b5fb0c137c71a08f` |
| `sliced` | crosscheck | input 제거 1932 | 채택 | golang/sort.Slice persona의 Go 도구 관리 주제는 sliced array 표현과 무관 | `rts3-b5fb0c137c71a08f` |
| `soft deleted` | analyst | own 제거 3197 | 기각 | 3197에 soft delete·hard delete·cascade delete 등 lifecycle 구성원. 기본형과 일관 | 미적용 |
| `std output` | analyst | own 제거 4383 | 기각 | 개념명은 standard error지만 standard output·std output·stdin을 함께 묶는 표준 스트림 개념 | 미적용 |
| `subcomponents` | analyst | own 추가 823 | 보류 | Storybook Meta subcomponents 키는 실제지만 제품 개념 소속 필요성 미확정(한쪽 제안·한쪽 기각) | 미적용 |
| `subcomponents` | crosscheck | 개념 6573 이동 t694.1→t273.3 | 채택 | Sentry exception grouping components는 issue grouping fingerprint(6704)·error tracking이 있는 t273.3이 더 정확. 분야 동일(observability-debugging) | `rts3-b5fb0c137c71a08f` |
| `suites` | crosscheck | own 추가 680 | 채택 | Keycloak HTTPS_CIPHER_SUITES 'The cipher suites to use' — cipher suite 개념 | `rts3-b5fb0c137c71a08f` |
| `tb` | analyst | own 추가 4370 | 채택 | flask cli.py:102 tb = sys.exc_info()[2], tb_next 순회 — traceback = stack trace | `rts3-b5fb0c137c71a08f` |
| `unencodable` | crosscheck | own 추가 408 | 채택 | vector 디스크 버퍼 이진 wire format 제약 — binary format(wire format·serialize) 구성원 | `rts3-b5fb0c137c71a08f` |
| `unencodable` | analyst | 새 개념 "wire-format encodability"(t201.1) 단어 unencodable | 기각 | 같은 주제의 기존 408로 표현 가능 | 미적용 |
| `unfocused` | crosscheck | focused에서 455 상속 | 보류 | 부정 상태형과 CSS 시각적 focus 장식 주제의 경계 미확정(한쪽 제안·한쪽 기각) | 미적용 |
| `wakes` | crosscheck | own 추가 5170 | 채택 | tokio wake_by_ref 재폴링 통지 = waker 개념(woken·notified와 같은 묶음) | `rts3-b5fb0c137c71a08f` |
| `wch` | crosscheck | own 추가 756 | 채택 | terminal WCHAR wide-character 값 = 문자 코드 단위(code unit) | `rts3-b5fb0c137c71a08f` |
| `wrlock` | crosscheck | own 추가 3359 | 채택 | pthread_rwlock_wrlock 쓰기 잠금 획득. 같은 개념에 rdlock·acquire·release | `rts3-b5fb0c137c71a08f` |
| `hovered` | crosscheck | own 제거 2766 | 채택 | 2766의 hover는 @media (hover: none) 장치 기능 토큰. hovered(요소 상태)는 media feature가 아님. 의존 0 | `rts3-b5fb0c137c71a08f` |

## R3 단어별 결정

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `acceptor` | 제거 4 (2037, 2087, 4900, 23) | 채택 | 부모 처분(042 arbitration031)이 4건 제거를 명시 선택했으나 적용되지 않음. HTTP Accept 헤더·HMR accept·파일 선택 accept 형식은 수락 주체 acceptor에 성립하지 않음. 의존 0 | `rts3-b5fb0c137c71a08f` |
| `attaching` | 제거 2 (160, 4295) | 기각 | 부모 처분(050)이 근거를 들어 input 160·4295를 유지. 원천 appending/connecting persona가 attaching을 synonym으로 생성했고 주제 범위(배열 덧붙이기, client connection 구성)에 input 분류가 성립 | 미적용 |
| `time windows` | 상속 4 (1085, 1835, 3954, 4783) | 채택 | 부모 기록은 검토 진행 목록일 뿐 의미 처분 없음. 기본형 time window의 시계열·간트·sliding window·기간 input이 복수형에 성립 | `rts3-b5fb0c137c71a08f` |
| `timeouts` | 상속 109 (109개념) | 채택 | 부모 기록은 검토 진행 목록일 뿐 의미 처분 없음. timeout의 각 기술 문맥(소켓·테스트·잠금·재시도 등)이 복수형 timeouts에 모두 성립 | `rts3-b5fb0c137c71a08f` |

## 남은 보류

다음 R2 조치는 근거가 부족하거나 원칙 경계에 걸려 반영하지 않았다.

> 이 보류 31건은 [4단계](stage4-decisions.md#r2-보류-재판단)에서 다시 판단했다. 채택 10, 기각 17, 보류 4(`rxs` 1, `savings` 3)다. 아래 목록은 R1~R3 처리 시점의 기록이다.

- `cacher` — own 제거 1530: 1530은 하드웨어 cache line과 read-through 등 소프트웨어 캐시가 섞인 개념. 기본형 cache도 own이라 단독 제거 근거 부족
- `completed task` — completed에서 1574 상속; completed에서 3219 상속; completed에서 3535 상속; completed에서 3551 상속; completed에서 361 상속; completed에서 4199 상속; completed에서 4419 상속; completed에서 5168 상속: 복합 API 이름(Task.CompletedTask)에 수식어 completed의 상태 input을 상속할지는 원칙 경계. 한쪽 제안만 있음
- `crashing` — own 제거 2004: 기본형 crash가 같은 stall 개념 own. 파생형만 제거하면 불일치. 한쪽은 제거, 한쪽은 불확실
- `cvt` — own 제거 3966; own 제거 4863: cvt=convert 일반 약어의 decimal 변환 뜻 성립 여부를 확정할 근거 부족(한쪽 제거, 한쪽 불확실)
- `dismissal` — own 추가 5637: 뜻은 맞지만 t141.2 주제의 분야가 auth-identity로 잘못됨(A3). 중재도 분야 검토 전 보류
- `dismissals` — own 추가 5637: dismissal과 같은 A3 분야 결함으로 보류
- `fmadd` — own 추가 6675 (대상 fusedmultiplyadd): fusedmultiplyadd는 표제어가 없어 새 단어 생성이 필요. 12개 저장소 수집 단어인지 미확인
- `invalidator` — own 제거 5241 (대상 invalidate): 기본형 invalidate persona 16847에 native 관계 3·inbound input 1이 있어 재귀속 처분 필요
- `lagging` — own 제거 2519: SQL LAG 함수명 vs 시계열 lagging 연산 파생 뜻 성립 여부 미확정(한쪽 제거, 한쪽 불확실)
- `lockouts` — own 제거 3920: 기본형 lockout이 같은 retry 개념 own. 시도 한도와 차단의 구성 관계 미확정
- `member names` — own 추가 2774 (대상 membername); own 추가 7151 (대상 membername): membername은 표제어가 없어 새 단어 생성 필요
- `paginating` — own 제거 2709: 기본형 paginate도 같은 batch 개념. pagination과 batch 구성 경계 미확정
- `pointer moved` — pointermove에서 3381 상속; pointermove에서 5665 상속; pointermove에서 708 상속: pointermove(DOM)를 PointerMoved(XAML)의 기본형으로 볼지 경계. 기존 own 7127이 포인터 입력 뜻을 담음
- `revokable` — own 제거 1188: 기본형 revoke(revokeObjectURL)가 같은 Item removal 개념. 권한 회수 가능성과 항목 제거의 경계 미확정
- `rxs` — 새 개념 "channel receiver"(t297.1) 단어 rxs: 수신 endpoint 개념은 rx·receiver 등과 함께 정해야 하는 범위. 한쪽 제안
- `savings` — 새 개념 "execution cost savings"(t401.4) 단어 savings; 새 개념 "memory size savings"(t234.4) 단어 savings; 새 개념 "monetary cost savings"(t111.1) 단어 savings: 한쪽이 제안한 신규 개념 3건. 기존 6681(앱 크기 절감) 외 뜻별 분할 필요성 미확정
- `subcomponents` — own 추가 823: Storybook Meta subcomponents 키는 실제지만 제품 개념 소속 필요성 미확정(한쪽 제안·한쪽 기각)
- `unfocused` — focused에서 455 상속: 부정 상태형과 CSS 시각적 focus 장식 주제의 경계 미확정(한쪽 제안·한쪽 기각)

