# 관련어 시소러스: 데이터셋에 반영되지 않은 결함

이 문서는 관련어 시소러스 데이터셋에서 **문제로 확인되거나 제안됐지만 최종 데이터셋에 반영되지 않은 결함**과, 결함이 숨어 있을 수 있는 조회 도구·검증의 공백을 모은 목록이다. 다음 작업자가 항목 이름이나 단어로 바로 찾아 처리 여부를 판단할 수 있게 정리했다.

| 항목 | 내용 |
| --- | --- |
| 조사 대상 데이터셋 | `rts3-c4a3b0fe9eb74bc2` (SHA-256 `88edc1c3f25bc4218dc51fbc4dd4e1707cd83a02088ac8cb45517c71d9441f77`) |
| 현재 기본 데이터셋 | `rts3-9e01eaf1eadf960a` (4단계 처리 결과, SHA-256 `ab8f1aea8a60625fb4f4795e02ffdfa9b3947ce61cc7120c84a03dc61646d192`). `sense_lookup.py`의 기본값. 같은 날 먼저 게시한 `rts3-d10ac73c3aee85cb`는 표기 결함(D4와 같은 원인)으로 대체했고 쓰지 않는다. 직전 기본값은 R1~R3 처리 결과 `rts3-b5fb0c137c71a08f`(SHA-256 `62b1122b10d6af2e4d24886e046148ae5912b6da0aa1f9504e73f6e372db3a48`) |
| 데이터셋 위치 | `.codemap/poc/related-terms/thesaurus-v1/datasets/<데이터셋 ID>/dataset.sqlite3` |
| 기준선 | `rts2-309e9bbaa19f7eb5` |
| 확인일·방법 | 2026-10-16. 최종 DB와 기준선 DB를 읽기 전용으로 직접 조회해 현재 상태를 확인했다 |
| 근거 경로 기준 | 아래 상대 경로는 `~/.cache/related-terms-usage-1/word-eval/followup-20261011/` 기준이다 |

## 이전 완료 판단의 정정

2026-10-16 최종 검토393은 "P1 556단어가 모두 부모 처분 또는 통합 기록에 연결되어 있고, 남은 보류는 P1 약 20단어와 P2 4건"이라고 보고했다. **이 판단은 틀렸다.** 당시 검사는 단어 문자열이 전체 단어 인벤토리 파일에 나타나기만 해도 연결로 셌기 때문에, 처분이 없는 단어도 통과했다.

구조화된 부모 처분·적용 계획 기록으로 다시 확인한 결과는 다음과 같다.

- 그런 기록이 있는 P1 단어는 84개다.
- 472개에는 그런 기록이 없다. 그중 296단어에는 두 검토자가 합의한 조치 1,610건이 반영되지 않은 채 남아 있다([R1](#r1-처분반영되지-않은-p1-합의-조치)).

사용자 결정393("남은 보류는 현행 유지로 일괄 확정")과 그 뒤의 완료 기록397은 이 잘못된 범위를 근거로 했다. 따라서 "현재 범위 완료"는 이 공백을 포함하지 않은 판단이다.

이 공백(R1~R3)은 2026-10-16 별도 작업으로 단어별로 판단해 반영했고, 그 결정이 결정393을 대체한다. 결정과 이유는 [R1~R3 처리 결정 기록](r1-decisions.md)에 있다.

그 뒤에 남은 R1 단어의 이견 325건, R2 보류 31건, R1~R3 처리로 생긴 조회 변화, A·B·C·D·E는 2026-10-16 4단계 작업으로 처리했다. 결정과 이유는 [4단계 결정 기록](stage4-decisions.md)에 있다.

## 한눈에 보기

| ID | 구분 | 규모 | 현재 상태 |
| --- | --- | --- | --- |
| [R1](#r1-처분반영되지-않은-p1-합의-조치) | 처분·반영되지 않은 P1 합의 조치 | 296단어, 1,610건 | 처리함: 1,610건 채택(1건 재지정). 같은 단어의 이견 325건은 4단계에서 채택 226, 기각 72, **보류 27** |
| [R2](#r2-이견만-남은-p1-단어) | 이견만 남은 P1 단어 | 63단어, 이견 조치 116건 | 처리함: 채택 60, 기각 25, 보류 31. 보류는 4단계에서 채택 10, 기각 17, **보류 4** |
| [R3](#r3-부모-기록은-있으나-합의-조치-일부가-빠진-단어) | 부모 기록은 있으나 합의 조치 일부가 빠진 단어 | 4단어, 119건 | 처리함: 채택 117, 기각 2 |
| [A](#a-잘못된-연결로-판단됐으나-고치지-않은-항목) | 잘못된 연결로 판단됐으나 고치지 않은 항목 | 3건 | 처리함(4단계): 3건 모두 수정 |
| [B](#b-뜻이-확정되지-않은-채-남은-연결) | 뜻이 확정되지 않은 채 남은 연결 | 23건 | 처리함(4단계): 9건에서 잘못된 연결 정리, 나머지는 유지 결정, **보류 2**(B16, B23 관계 혼합 확인) |
| [C](#c-추가하지-않은-후보) | 추가하지 않은 후보 | 8건 | 처리함(4단계): 추가 4(C1·C3·C4·C5), 추가 안 함 3, **보류 1**(C7) |
| [D](#d-조회도구-결함) | 조회·도구 결함 | 3건 | 처리함(4단계): D1 의도된 동작 확인, D2 제약 유지, D3 실행 확인. 새로 확인한 D4는 기록만 함. 코드 미수정 |
| [E](#e-검증-공백) | 검증 공백 | 5건 | E1~E3 재확인 완료, E4 부분 보완, E5 범위 밖 |
| [F](#f-최종본에서-다시-확인하지-않은-이전-관찰) | 최종본에서 다시 확인하지 않은 이전 관찰 | 4건 | 범위 밖(최종 전후 비교에 해당). 최종본 영향 미측정 |
| [G](#g-4단계에서-새로-확인한-관찰) | 4단계에서 새로 확인한 관찰 | 3건 | 기록만 함 |

## 용어

- **own:** 단어가 그 개념의 구성원으로 직접 등록된 연결이다.
- **input:** 단어를 다른 단어의 뜻(source persona)을 통해 분류하는 연결이다. 동의어일 필요는 없다.
- **합의 조치:** analyst와 crosscheck 두 독립 검토자가 같은 조치를 낸 것이다. **부모 수락이 아니며**, 반영하려면 부모가 개별로 골라야 한다.
- **뜻 상속:** 파생형(예: `timeouts`)이 기본형(`timeout`)의 특정 개념을 이어받는 조치다. 개념마다 따로 골라야 하며 일괄 상속하지 않는다.

## R. 처분되지 않은 P1 검토 결과

### R1. 처분·반영되지 않은 P1 합의 조치

부모 처분 기록이 없는 P1 296단어에서, 두 검토자가 합의했지만 조사 대상 데이터셋에 반영되지 않았던 조치다. 2026-10-16에 단어별로 판단해 모두 반영했다.

| 조치 종류 | 건수 | 처리 결과 |
| --- | ---: | --- |
| 잘못된 own 제거 | 107 | 107 채택. 대상 persona의 native 관계·inbound input·input_relations는 모두 0건이었다 |
| 잘못된 input 제거 | 113 | 113 채택(input 행 189개) |
| 뜻 상속 | 1,332 | 1,332 채택. `detectors`의 461은 기본형 연결이 이전 통합에서 7310으로 재귀속돼 7310으로 재지정 |
| own 추가 | 44 | 44 채택. 1건(`identifiers` 행)의 대상은 기본형 `identifier` |
| 개념 이동 | 9 | 9 채택 |
| 새 개념 | 5 | 5 채택(개념 7331~7335) |
| 합계 | 1,610 | 채택 1,610, 기각 0, 보류 0 |

- 반영 데이터셋: 제거 `rts3-a8af20d9b15ab705`, 새 개념 `rts3-554bf1373eb43b73`, 나머지 `rts3-5b9386bd6787963e`. 단어별 결정은 [결정 기록](r1-decisions.md#r1-단어별-결정)에 있다.
- 처리 전 목록은 [부록 §1](unreflected-review-actions.md#1-두-검토자가-잘못된-연결이라고-합의했으나-남아-있는-연결)·[§3](unreflected-review-actions.md#3-단어별-미반영-합의-조치-수)과 [JSON 부록](unreflected-review-actions.json)에 그대로 두었다.
- **이견 조치 325건:** R1~R3 처리 범위에는 없었고, 4단계에서 채택 226, 기각 72, 보류 27로 처리했다([결정](stage4-decisions.md#r1-이견-조치별-결정)). 남은 보류 27건은 대부분 한쪽만 제안한 새 개념이고, 목록은 [남은 보류](stage4-decisions.md#남은-보류)에 있다.
- **조회에서 남던 기본형 결함:** 조회 도구는 own이 없는 단어를 기본형으로 넓힌다. `slaves`는 기본형 `slave`의 잘못된 input(30, `whitelist` 경유) 때문에 제거한 연결이 조회에 계속 나왔다. 4단계에서 기본형의 같은 연결을 지웠다.

### R2. 이견만 남은 P1 단어

부모 처분 기록이 없고 합의 조치는 없지만, 한쪽 검토자만 낸 조치 116건이 반영되지 않았던 63단어다. 처리 전 목록은 [부록 §4](unreflected-review-actions.md#4-합의는-없고-두-검토자의-의견이-갈린-채-남은-단어)에 있다.

2026-10-16에 한쪽 의견만으로 채택하지 않고, 양쪽 원문과 고정 체크아웃 코드·개념 구성원·원천 persona 관계를 직접 확인해 판단했다. 별도 중재 결과가 있던 3단어(`disconnection`, `dismissal`, `dismissals`, `p1-arbitration-04/p1-arbitrate-006.json`)는 그 결과도 대조했다.

| 결과 | 건수 |
| --- | ---: |
| 채택 | 60 |
| 기각 | 25 |
| 보류 | 31 |

- 반영 데이터셋: 새 주제·개념 `rts3-3b085066fb651e66`(주제 x51, 개념 7336~7340), 나머지 `rts3-b5fb0c137c71a08f`. 조치별 결정은 [결정 기록](r1-decisions.md#r2-조치별-결정)에 있다.
- **예: `dismissal`, `dismissals`.** 중재 결과와 같이 6501(popup closing event) own과 새 개념 `payment provider incentive dismissal`(7337)을 채택했다. 5131(virtual memory) 제거는 Redis `dismissMemory`의 페이지 반환 뜻이 성립해 기각했다. 5637(대화상자 닫기) 추가는 그 주제의 분야 결함([A3](#a-잘못된-연결로-판단됐으나-고치지-않은-항목)) 때문에 보류했다.
- **4단계 재판단:** 아래 보류 31건은 4단계에서 채택 10, 기각 17, 보류 4(`rxs` 1, `savings` 3)로 다시 판단했다([결정](stage4-decisions.md#r2-보류-재판단)). `dismissal`·`dismissals`는 A3를 고친 뒤 채택했다.
- **R1~R3 처리 시점의 보류 31건(18단어):** `cacher`, `completed task`(8), `crashing`, `cvt`(2), `dismissal`, `dismissals`, `fmadd`, `invalidator`, `lagging`, `lockouts`, `member names`(2), `paginating`, `pointer moved`(3), `revokable`, `rxs`, `savings`(3), `subcomponents`, `unfocused`. 주된 이유는 다음과 같다. 항목별 이유는 [남은 보류](r1-decisions.md#남은-보류)에 있다.
  - 기본형이 같은 개념에 있는데 파생형만 제거하자는 제안이고, 양쪽 의견도 갈린다(`cacher`, `crashing`, `lockouts`, `paginating`, `revokable` 등).
  - 표제어가 없는 단어를 새로 만들어야 한다(`fusedmultiplyadd`, `membername`).
  - 복합 API 이름이나 다른 표기를 기본형으로 볼지가 원칙 경계다(`completed task`, `pointer moved`).
  - 기본형 persona에 native 관계가 있어 재귀속 처분이 필요하다(`invalidator` 행의 `invalidate`/5241).
- **조회 관찰:** `negations`는 own이 없어 기본형 `negation`으로 넓혀지고, 기본형에 같은 2865 input이 남아 있어 제거한 연결이 조회에 계속 나온다. `investigations`·`preds`는 own이 생겨 기본형 확장이 멈추면서, 기본형에만 있던 뜻(`investigation`의 4081, `pred`의 16·1365·2222·4693)이 조회에서 빠졌다. 4단계에서 `negation`의 2865를 지우고, 빠진 뜻 다섯 개를 파생형에 상속했다([결정](stage4-decisions.md#조회-변화-보완)).

### R3. 부모 기록은 있으나 합의 조치 일부가 빠진 단어

`acceptor` 4건, `attaching` 2건, `time windows` 4건, `timeouts` 109건이다. 2026-10-16에 부모 기록을 다시 읽고 다음과 같이 처리했다.

| 단어 | 부모 기록의 실제 | 결정 |
| --- | --- | --- |
| `acceptor` | 부모(`semantic-comparison-042/parent-disposition-001-arbitration031.json`)가 4건 제거를 명시 선택했으나 적용되지 않았다 | 채택 4 |
| `attaching` | 부모(`semantic-comparison-050/parent-disposition-002-first22-ten.json`)가 근거를 들어 input 160·4295를 유지했다(의도적 미선택) | 기각 2. 원천 persona 관계와 주제 범위를 다시 확인해 부모 판단을 유지 |
| `time windows`, `timeouts` | 검토 진행 목록에 이름만 있고 의미 처분은 없다 | 채택 4·109. 복수형에 각 뜻이 성립 |

반영 데이터셋은 `rts3-b5fb0c137c71a08f`이다.

### R의 산정 방법과 한계

- **기준:** 단어마다 가장 최근 비교 파일(`semantic-comparison-*/p1-*-comparison.json`)의 `common_actions`·`analyst_only`·`crosscheck_only`를 기준선과 최종 DB에 대조했다.
- **부모 기록 판정:** 136개 부모 처분 파일과 통합 적용 계획에서 `word`류 필드가 그 단어인지로 판정했다.
- **한계:**
  - 키 구조가 다른 처분 문서는 놓칠 수 있다. 실제로 P2의 `explorer`·`replicy`·`scene`은 처분이 있었지만 이 방식으로는 기록 없음으로 나왔다. 따라서 R1·R2의 단어 수는 실제보다 조금 많을 수 있다.
  - P2 76단어는 모두 부모 처분이 있다. 반영되지 않은 P2 합의 조치(`acquring`, `common`, `polices`)는 부모가 의도적으로 고르지 않은 것이라 이 목록에서 뺐다.
  - 이 대조는 검토 합의가 옳다는 판정이 아니다.

> A~F는 R1~R3 처리에서 다루지 않았고, 2026-10-16 4단계에서 처리했다. 아래 표는 처리 전 상태를 담은 근거로 그대로 두고, 각 절 머리에 처리 결과를 적었다. 항목별 결정은 [4단계 결정 기록](stage4-decisions.md)에 있다.

## A. 잘못된 연결로 판단됐으나 고치지 않은 항목

**4단계 처리:** 세 건 모두 고쳤다.
- A1·A2는 관계를 같은 단어의 맞는 뜻 persona로 옮기고 input을 해제·재연결한 뒤 own을 제거했다. A1은 `execution`의 persona 11311(개념 1036 invocation)로, A2는 `disconnect`의 연결 수명주기 persona 9185로 옮겼다. A1의 관계는 조회에서 web-backend 뜻 아래 표시되는 한계가 있다([G2](#g-4단계에서-새로-확인한-관찰)).
- A3는 개념 5637을 `t773` Popup UI widgets(frontend-ui)로 옮겼다.

| ID | 단어 | 현재 연결 | 문제 | 고치지 않은 이유 | 근거 |
| --- | --- | --- | --- | --- | --- |
| A1 | `execution` | own 812 `log`(t347.1, 로거 출력 설정), persona 11312 | 실행 사건이 로거 설정 개념에 소속돼 있다 | 부모가 제거 취지는 지지했으나 의존 관계(native 4, input 3)를 처분하지 않았다 | `semantic-comparison-030/parent-disposition-001-arbitrations014016.json` `/selections/6/blocked_scopes/0` |
| A2 | `disconnect` | own 5000 `unsubscribe`(t753), persona 9188 | 관계는 소켓 연결 끊김인데, 주제는 뉴스레터 구독 해지 문맥이다 | 제거가 제안됐으나 의존 관계(native 4, input 3) 검토 전이라 보류됐다 | `p1-006-1-parent-extract-001.json` |
| A3 | `onclose` | own 5637 `대화상자 닫기`, persona 22852 | 주제 t141.2(Dialog closing)의 분야가 `auth-identity` 하나뿐이다. UI 닫기 뜻이 인증 분야로 분류된다 | 주제의 분야를 고치는 처분이 없다. 작업 기록에는 분야 불일치 보류로만 남아 있다 | 최종 DB의 `topic_fields` |

## B. 뜻이 확정되지 않은 채 남은 연결

**4단계 처리:** 각 연결을 개념 범위와 코드로 다시 판단했다.
- **정리한 연결(9건):** B3 `encodable`(own 2253, input 1442), B8 `exporters`(own 1167·1308·3592, input 3591·2296), B10 `favorite`(input 3673), B12 `filled`(input 110), B13 `finalized`(input 5923), B14 `fingerprinter`(own 1716), B15 `mixed`(input 4412), B21 `lead`(CRM persona의 `leadtime` 관계·input), B22 `scene`(input 1298).
- **유지 결정:** B1·B2·B4·B5·B6·B7·B9·B11·B19·B20과 B8·B13·B14의 나머지 연결, B23의 `common`·`multiple` 소속. B17·B18은 이전 사용자 결정(기존 상태만 보존, `followup107-user-policy-001`)을 따랐다. 아래 표의 B18 "질문하지 않았다"는 틀린 서술이었다. 사용자는 이미 "기존 상태만 보존"을 골랐다.
- **보류:** B16 `multinode`. 코드 용례는 vector의 `MULTINODE` 하나뿐이라 구조 노드 뜻의 성립 여부를 확정하지 못했다. B23은 두 단어 persona에 남은 관계가 섞여 있는지 전수 확인을 하지 않아 그 확인을 보류로 남겼다.

부모 처분에서 "의미 미확정·현행 보존"으로 남긴 연결이다. 모두 기준선부터 있던 연결이며 최종본에도 그대로 있다. 맞는 연결이라는 승인도, 틀렸다는 판정도 아니다.

| ID | 단어 | 현재 연결 (개념 이름, 주제) | 미확정 내용 |
| --- | --- | --- | --- |
| B1 | `execution` | own 3908 `runtime`(t433.1, 실행 시간 측정), persona 11310 | 실행 사건과 소요 시간의 소속 적합성, native 12·input 4 처분. 후속389의 제거 권고는 수락 실패로 반영되지 않았다 |
| B2 | `dsig` | own 1265 `cms`(t221.3) persona 43578, own 2438 `Requests`(t136) persona 43580 | `dsig` 표기가 CMS·JWS 범위에 속한다는 근거 부족 |
| B3 | `encodable` | own 2253 `sanitize`(t410.2) persona 43119; input 1442 `Vega-Lite`(t535.1), source 867·14187·37225 | 인코딩 능력 형용사 뜻, 시각화 인코딩과 바이트 직렬화의 구별 |
| B4 | `engines` | input 2083 `host`(t662.1), source 29398 | 런타임 버전 요구와 실행 호스트 분류의 경계 |
| B5 | `ent` | own 8개(JPA, ddd, OneToMany, record, model, subject, payload, component; persona 42740–42747), input 10개(dao 4, hexagonal architecture 3, orm 1, ssrf 2) | 공개 ORM `ent`와 다른 뜻의 범위 |
| B6 | `exchange` | own 3935 `cancel`(t912), persona 11208 | 일반 교환 뜻의 lifecycle 범위 |
| B7 | `exchanged` | input 1084 `Parquet`(t74.1) source 25502·35567; input 3652 `RabbitListener`(t24.4) source 3001·26661·26670 | source별 의미 경계 |
| B8 | `exporters` | own 1167 `export`·1308 `dll`·3592 `expose`(persona 43083–43085); input 3591 `use`(263·21027), 1377 `backup`(2541·9906), 2296 `interface`(28492) | 모듈·DLL·API 내보내기 구성요소라는 근거 부족 |
| B9 | `expression` | own 555 `calculate`(t260), persona 11523 | source의 분야·의존 처분 |
| B10 | `favorite` | input 3673 `range`(t334.1), source 3241 | 연결의 의미 |
| B11 | `file type` | own 2953 `named pipe`(t236.2), persona 12181 | 소속 적합성 |
| B12 | `filled` | input 110 `initialized`(t66), source 25127 | 연결의 의미 |
| B13 | `finalized` | input 5923 `Eiffel contracts`(t836.1) source 10197; input 2194 `available`(t610.1) source 12306 | 연결의 의미 |
| B14 | `fingerprint`, `fingerprinter` | `fingerprint` own 2105 `HTMLCanvasElement`(t108.1) persona 39047; `fingerprinter` own 1716 `gpg`(t80.1) persona 42749 | 기본형·파생형 소속 적합성 |
| B15 | `mixed` | input 4412 `gof`(t100.2), source 5598 | 패턴 조합 문맥이 이 source에 맞는지 |
| B16 | `multinode` | own 1962 `vertex`·2246 `dom`·3956 `tree`·4890 `SyntaxKind`(persona 42961–42966), input 12개 개념 | 구조 노드와 여러 실행 노드의 구별 |
| B17 | `sections` | input 1932 `golang`(t382.1, source Hugo 15403), 4230 `shopping cart`(t261.2, source Shopify 30965), 4988 `tag`(t460.3, source 21457) | 제품 문맥과 주제의 연결 |
| B18 | `updaters` | own 3427 `updated at`(t556.1) persona 43659; input 354 `createHash`(7159), 2020 `hash`(source 7개), 2260 `bulk insert`(source 3개) | 처분이 "사용자 선택 필요"로 남았으나 질문하지 않았다 |
| B19 | `drifted`, `editable` | `drifted` own 1237 `heuristic`(t463) persona 42911; `editable` own 625 `onChange`(t292) persona 43340 | 소속 범위 |
| B20 | `downloadable` | input 4798 `translation memory`(t315.7), source POEditor 24991·Smartling 31435 | 내려받는 대상이 번역 메모리(TMX)인지 일반 번역 파일인지 |
| B21 | `lead` | native `lead_time`(persona 18395, rank 28, identifier)과 input `leadtime`→18395, CRM 901(t378) | `lead_time`이 측정하는 업무 사건 |
| B22 | `scene` | input `scene`→29899 `screen`(1298, t483.1 화면 픽셀 밀도) | 장면과 화면 치수 검사 주제의 연결 |
| B23 | `common`, `multiple` | `common` own 5730(3877 `mjs`, t405.1)·5731(3973 `Next.js`, t915); `multiple` own 21415(2838 `mro`)·21416(1360 `duplicate`)·21421(1804 `tuple`) | 일부 관계만 정리했고 남은 혼합 payload는 전수 확인하지 않았다. `common`의 own 제거 제안은 선택하지 않았다 |

근거는 다음 처분 문서의 `named_pending_scopes`·`pending`·`holds` 항목이다.

- `semantic-comparison-028`~`030`·`041`의 `parent-disposition-*.json`
- `semantic-comparison-026/parent-disposition-002-downloadable.json`
- `held375-parent-selection-001/decision.json`
- 단어별 위치: `final396-closure-check-001/hold-closure.json`

**처리 여부를 확인하지 못한 보류:** `searchable`·`searched`의 input 1706·5034·3633, `alignments`의 3105·3443, `Bookmarks` 5342(공유 주제-분야)는 보류로 기록된 뒤 이후 처분에서 해소됐을 수 있었다. 4단계에서 확인해 모두 처리했다.
- `searchable`·`searched`의 input 6개와 `alignments`의 3443을 지웠다. `alignments`의 3105는 이미 없었다.
- Bookmarks 주제 `t766.2`의 분야가 `language-constructs`뿐이라, 새 주제 x52 Bookmarks and favorites(frontend-ui, browser-platform)를 만들고 5342를 옮겼다.

## C. 추가하지 않은 후보

**4단계 처리:**
- **추가:** C1 `exited`(469·1231·2499의 input 7개, 후속389 권고대로 1717 제외), C3 `missed`(2865 own), C4 `download`(3237 own), C5 `rax`(3956 own, `redis/src/rax.c`의 공개 radix tree 라이브러리).
- **추가하지 않음:** C2 `embedded`(파생 뜻 근거 없음), C6 `scenes`·C8 `favorited`(기본형의 같은 연결을 제거함).
- **보류:** C7 `committed`.

검토나 후속 분석에서 추가 후보로 나왔지만 최종본에 없는 연결이다. 빠진 뜻일 수 있다.

| ID | 단어 | 후보 | 추가하지 않은 이유 |
| --- | --- | --- | --- |
| C1 | `exited` | input 9개: 469 `control flow graph`(source 4486·6747), 1231 `destructor`(8478·8832·9864), 1717 `close`(5150·12300), 2499 `lsp`(18231·19546) | 상속 보류. 후속389가 1717을 뺀 7개 추가를 권고했으나 수락 실패로 반영하지 않았다 |
| C2 | `embedded` | input 391 `bidirectional`(t193.1, source 2889), 2070 `hoc`(t35.6, source 38367) | 파생 의미 근거 부족 |
| C3 | `missed` | `miss`의 2865 `not found`(t209.3, source 20891) | 놓친 관측과 엔티티 부재 응답의 구별 미해결 |
| C4 | `download` | own 3237(package) | 기본형의 패키지 획득 뜻 근거가 따로 필요하다 |
| C5 | `rax` | own·input 3956 `tree`(t496.4) | Rax(radix tree)의 공개 라이브러리 자격 미확정 |
| C6 | `scenes` | input 29899 `screen` | `scene`의 같은 연결(B22)이 미확정이라 복수형에 넓히지 않았다 |
| C7 | `committed` | `CommitInfo` 관련어 | 정확한 외부 제공자를 확인하지 못했다 |
| C8 | `favorited` | 3673 연결 | 추가 여부 보류 |

## D. 조회·도구 결함

**4단계 처리:** 코드는 고치지 않았다.
- **D1:** 결함이 아니라 의도된 표기 통합이다. 조회 도구는 같은 `word_key`에 표기를 하나만 쓰고 표제어(`terms`) 표기를 우선한다(`sense_lookup.py`의 `surfaces`). `checkin`과 `check-in`은 같은 `word_key`다.
- **D2:** 고정된 거부 조건(fail-closed)으로 유지한다. 4단계의 정밀 편집에는 해당하지 않았다.
- **D3:** `rts2-309e9bbaa19f7eb5`에서 실제로 실행해 호환성을 확인했다. 스키마는 `rt-sense/2`로 유지됐고 FK는 0이며 기준선 해시는 바뀌지 않았다.

| ID | 위치 | 결함 | 확인 내용 |
| --- | --- | --- | --- |
| D1 | `sense_lookup.py` 출력 | 저장된 관련어 표기와 다른 표기를 보여 준다 | `committed`를 조회하면 저장 literal `TransactionManager`·`checkin` 대신 `transaction manager`·`check-in`이 나온다. 저장 값은 보존돼 있다. 원인 위치는 확인하지 않았다 |
| D2 | `correct_sense_dataset.py` exact 편집 | 한 계획에서 donor persona가 다른 행의 recipient이면 계획 전체를 거부한다 (`reviewed_graph_edits.py:236-239`) | 출력 생성 전에 실패하므로 데이터 손상은 없다. 그런 편집은 계획을 둘로 나눠 순서대로 적용해야 한다 |
| D3 | `build_sense_dataset.py` `add_entries` | `rt-sense/2` 데이터셋에 대한 실제 실행 호환성을 확인하지 않았다 | `/3` 지원 확장 뒤 `/2` 경로는 정적 비교만 했다 |
| D4(4단계에서 새로 확인) | `correct_sense_dataset.py` `input_bindings` | 넘겨받은 `word` 표기를 그대로 표제어 표기로 저장한다. `word_key` 형태(예: `mxauto`)를 넘기면 그 단어의 조회 표시가 원래 표기(`mx-auto`)에서 바뀐다 | 시작 전 기본값 `rts3-c4a3b0fe9eb74bc2`에 이미 표시가 바뀐 corrected 표제어 24개가 있다(예: `COCO`→`coco`, `look and feel`→`lookandfeel`, `maintenance mode`→`maintenancemode`). 4단계도 처음엔 같은 결함을 만들었다가 고쳤다. 남은 24개는 표기만 고치는 조작이 없어 고치지 않았다 |

## E. 검증 공백

**4단계 처리:**
- **E1:** 281~305 단계 폴더의 `corrections.json`이 모두 출력 DB의 `correction_plan_sha256`과 일치한다. 권한 파일 경로는 여전히 일부 적용 스크립트에만 있다.
- **E2:** 281·285·291·292·293·295·296·304·305를 더 강한 저장 확인(정확한 집합, sense·분야 일치, 고아 sense, 나머지 테이블 불변)으로 다시 확인했다. 모두 문제 0건이다.
- **E3:** E235를 같은 입력으로 다시 실행해 모든 테이블과 `dataset_id`가 같음을 확인했다.
- **E4:** 4단계 소비 확인은 대상 단어의 다른 개념 변화도 표시한다. 대상 밖 단어의 전수 의미·순위는 보지 않았다.
- **E5:** 사용자가 범위에서 제외했다.

결함이 있어도 지금까지의 확인으로는 검출되지 않았을 수 있는 부분이다.

| ID | 공백 | 영향 |
| --- | --- | --- |
| E1 | 281~305 단계의 `invocation.json`에 부모 권한 파일 경로가 없고, 적용 스크립트의 상수에만 있다. 285 이후에는 계획 파일 해시도 없다 | 실행 기록만으로 승인 근거를 추적하기 어렵다. 독립 검토395에서 대조한 범위에서는 불일치가 없었다 |
| E2 | 일부 저장 확인 검사가 약하다. 286은 sense ID 존재만 비교했다. 293의 `all(...)`은 빈 삭제 집합에서도 참이다. 296·304는 지워야 할 고아 sense가 모두 지워졌는지 확인하지 않았다 | 같은 ID의 내용 변경이나 누락된 정리를 놓칠 수 있다. 최종 점검에서 고아 sense 0건은 확인했다 |
| E3 | E235 단계의 실행 소스 해시가 기록되지 않았고, 그 확인의 원시 질의 17개는 독립 검토하지 않았다 | E 단계 FK·행 보존은 요약 수준까지만 독립 확인했다. 최종본 전체 FK 0건은 확인했다 |
| E4 | 소비자 확인은 선택한 값이 조회 결과에 들어오는지만 본다. 전수 의미·순위·선택 밖 결과는 보지 않았다 | 잘못된 다른 뜻이 함께 나오는 결함은 검출하지 못한다 |
| E5 | 회귀검증·평가셋4판·최종 전후 비교는 사용자가 범위에서 제외했다 | 최종본의 품질 개선이나 회귀 안전성은 측정되지 않았다 |

## F. 최종본에서 다시 확인하지 않은 이전 관찰

**4단계 처리:** 하지 않았다. 재측정은 사용자가 범위에서 제외한 최종 전후 비교에 해당한다.

이전 부분 보정판에서 관찰된 문제다. 그 부분판들은 최종본과 다른 순서로 만들어졌고, 최종본은 309부터 다시 쌓았다. 최종본에서 같은 문제가 있는지는 측정하지 않았다.

| ID | 부분판 | 관찰 |
| --- | --- | --- |
| F1 | `rts2-e855765a665e4664` | Terminal `menu` 문맥에서 `deselect` 도움어 1개 소실 |
| F2 | `rts2-df1561d7967db4c3` | Keycloak `comitted` 문맥에서 도움어 3개 소실. 이후 관계 보완판 `rts3-0a7992fceda3e026`에서 단일 문맥 재평가는 match, 도움어 51→73 |
| F3 | `rts3-60f343d53e5d0d41` | `debounced` 관련어 키 328→309(19개 감소) |
| F4 | `rts3-696350a67945afdd` (preview010) | 실제 유용 후보 소실 2회. Redis `multiple` 41→40, Vector `bulk` 25→24, Sentry `tuple` 48→14(대부분 후보에는 있으나 선택되지 않음) |

## G. 4단계에서 새로 확인한 관찰

4단계 작업과 독립 검토에서 새로 확인했지만 고치지 않은 사항이다.

| ID | 대상 | 관찰 | 처리 |
| --- | --- | --- | --- |
| G1 | 개념 3060 `ns`(t508.1 XML parsing) | 구성원에 XML namespace 용어와 `isolate`·`tenant isolation`·`namespace` 같은 격리 용어가 섞여 있다. 그래서 Linux `unshare`가 `isolate`·`namespace` persona를 거쳐 XML parsing 개념의 input이 된다 | 검토 조치가 없고 개념 분리가 필요해 고치지 않음 |
| G2 | `execution`의 persona 11311(개념 1036 `invocation`, t89.1 Server route handlers) | A1에서 옮긴 관계(`executionorder`·`executionflow`·`executiontrace`·`trace`)가 조회에서 web-backend 뜻 아래 표시된다. execution에는 일반 프로그램 실행 개념이 없다 | 네 persona 가운데 가장 가까운 수신자로 둠. 개념 신설은 검토 제안이 없어 하지 않음 |
| G3 | 빈 주제 | 개념이 없는 주제가 시작 데이터셋 14개에서 17개로 늘었다(t141.2·t766.2·t896.4) | 조회에 쓰이지 않아 그대로 둠 |

## 처리할 때 지켜야 할 조건

- 검토 합의는 부모 수락이 아니다. 반영하려면 단어별로 부모가 고른다. 파생형 뜻 상속은 개념마다 따로 고른다.
- own을 지우기 전에 그 persona의 native 관계·input·input 관계를 먼저 처리한다. 의존이 남아 있으면 `remove_own`이 거부한다.
- exact 편집에서 donor와 recipient persona가 겹치면 계획을 나눈다(D2).
- 의존이 있는 own 제거는 exact 계획으로 한다. 관계는 `reattribute_same_head`나 `remove`로, inbound input은 `input_detachments`로 처리하고 필요하면 `input_bindings`로 다시 연결한다. exact 계획은 이동·상속 같은 일반 조작과 한 계획에 섞을 수 없다.
- `input_bindings`는 표제어가 없는 input 단어를 표제어(`terms`, kinds `["corrected"]`)로 만든다. 저장 확인에서 이 표제어 생성을 기대값에 넣는다.
- 데이터셋을 새로 만들면 바로 앞 단계의 실제 출력 ID를 base로 쓰고, 끝나면 `sense_lookup.py`의 `DEFAULT_SENSE_DATASET`을 갱신한다.

## 근거 위치

| 내용 | 위치 |
| --- | --- |
| 미반영 검토 조치 세부(처리 전) | [unreflected-review-actions.md](unreflected-review-actions.md), [unreflected-review-actions.json](unreflected-review-actions.json) |
| R1~R3 처리 결정 | [r1-decisions.md](r1-decisions.md) |
| R1~R3 작업 파일 | `~/.cache/related-terms-usage-1/word-eval/r1-integration-001/` (계획 `plans/`, 결정 `decisions/`, 확인 결과 `checks/`, 단계별 데이터셋 `datasets/`) |
| 4단계 처리 결정 | [stage4-decisions.md](stage4-decisions.md) |
| 4단계 작업 파일 | `~/.cache/related-terms-usage-1/word-eval/remaining-001/` (결정 `r1d_decisions.py`·`other_decisions.py`, 계획 `plans/`, 확인 결과 `checks/`, E 재확인 `checks/e-reverify/`, 단계별 데이터셋 `datasets/`) |
| 원 비교 파일 | `semantic-comparison-*/p1-*-comparison.json`, `semantic-comparison-*/p2-*-comparison.json` |
| 보류 항목 원문 위치 | `final396-closure-check-001/hold-closure.json` |
| 단계별 적용 기록 | `integration*/` (각 `corrections.json`, `actual-output.json`, `confirmation.json`) |
| 독립 적용 검토 | `application-review330/`, `application-review349/`, `application-review369/`, `application-review383/`, `application-review395/` |
| 후속389 보고서 | `P1-followup389-001/downloadable/report.md`, `P1-followup389-001/execution-exited/report.md` |
| 최종 정합성 점검 | `final396-closure-check-001/integrity-check.json` |
| 근거 저장소 12개 | 데이터셋 폴더의 `sources.json` |
