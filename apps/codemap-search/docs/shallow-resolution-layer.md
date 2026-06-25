# 제안: tree-sitter 위 얕은 해석 층 (import / scope / owner)

> 상태: 제안(Proposal) · 대상 독자: code-agent · 무게: 가벼움(증분 도입)
> 한 줄 요약: caller/callee의 name-match 근사 오류를, **빌드 환경 없이** tree-sitter 위에 얕은 의미 해석 층을 얹어 점진적으로 줄인다.

---

## 1. 배경 (왜 하는가)

현재 caller/callee 귀속은 순수 이름 매칭(name matching)이다. 타입 해석이 전혀 없어 구조적으로 두 방향의 오류가 난다.

- **False positive (과대 귀속)**: 같은 이름의 동명이인을 구분 못 함. `user.save()`와 `file.save()`가 같은 `save` 정의에 함께 묶임. → `src/callers/annotate.rs`가 정의 2개 이상일 때 `approximate` 라벨을 붙이는 바로 그 케이스.
- **False negative (누락)**: `import { foo as bar }` 후 `bar()` 호출은 이름이 `bar`라 `foo`의 호출자로 안 잡힘.

LSP / SCIP는 이를 정밀하게 풀지만 **빌드 환경을 전제**하고 **언어 커버리지가 파편화**된다. 현재 도구의 핵심 가치(9개 언어 균일 처리 · 빌드 불요 · 실시간 증분 · never-fails graceful degrade)와 충돌한다.

따라서 외부 정밀 인덱서를 도입하는 대신, **이미 가진 tree-sitter 자산 위에 얕은 해석 층을 직접 더한다.** 목표는 "항상 정답"이 아니라 **"모호함을 줄일 수 있을 때만 줄이고, 안 되면 기존 동작으로 안전하게 폴백"**이다.

---

## 2. 설계 원칙 (반드시 지킬 것)

1. **graceful degrade 유지**: 해석이 1개 후보로 못 좁히면 기존 name-match + `approximate` 라벨로 떨어진다. 절대 응답을 깨지 않는다(`src/callers/mod.rs`의 never-fails 속성 보존).
2. **빌드 환경 불요**: 외부 도구·컴파일러·의존성 resolve를 호출하지 않는다. 파일 텍스트 + tree-sitter + 기존 인덱스 데이터만 사용.
3. **9개 언어 균일성 유지**: 언어별 차이는 `LanguageSpec` 트레이트(`src/lang/mod.rs`)에만 둔다. 코어 로직은 언어 중립.
4. **증분 친화**: 파일 단위로 계산 가능해야 한다(현재 watcher 250ms debounce 증분 모델과 호환).
5. **비용 최소**: import 문은 파일 상단 몇 줄, owner는 호출 토큰 앞 1개, scope는 이미 인덱스에 있는 정의 위치 비교로 끝낸다.

---

## 3. 세 개의 층 (동작 예시)

### 3.1 owner 층 — 동명이인 메서드 구분
```ts
const user = new User();   // user : User
const file = new File();   // file : File
user.save();   // → User.save 에만 귀속
file.save();   // → File.save 에만 귀속
```
호출 토큰 앞의 receiver(`user` / `file`)를 읽고, 지역 변수의 타입 힌트(`new T()` / `: T`)로 owner 후보를 좁힌다. `ExtractedSymbol.owner`(`src/parser/types.rs`)가 이미 추출돼 있으니 호출 측에서도 receiver 한 토큰만 더 읽으면 매칭 가능.

### 3.2 import 층 — alias 추적 (ROI 최고, 먼저 도입 권장)
```ts
import { fetchUser as getUser } from "./user";
getUser();   // → ./user 의 fetchUser 호출자로 귀속
```
파일 상단 import 문을 파싱해 `{ alias → (realName, sourceModule) }` 테이블을 만든다. 호출 토큰이 alias면 진짜 이름·출처로 치환 후 귀속. 누락(false negative)과 오귀속을 동시에 줄인다.

### 3.3 scope 층 — 로컬 정의 우선
```ts
function save() { ... }        // 파일 로컬 헬퍼
class File {
  flush() { save(); }          // → 같은 파일의 로컬 save 에 귀속
}
```
귀속 우선순위: **① 같은 파일/모듈 로컬 정의 > ② import된 정의 > ③ 전역 name-match(폴백)**.

---

## 4. 귀속 알고리즘 (의사코드)

```
resolveCall(callName, callSite, fileContext):
    # 1. owner 층
    receiver = readReceiverBefore(callSite)          # `x.callName()` 의 x
    ownerHint = inferLocalVarType(receiver, fileContext)  # new T() / : T

    # 2. import 층
    if callName in fileContext.importAliases:
        (callName, sourceModule) = fileContext.importAliases[callName]

    # 3. scope 층 (우선순위 탐색)
    candidates = lookupDefs(callName, ownerHint, sourceModule,
                            order = [SAME_FILE, IMPORTED, GLOBAL])

    # 4. 결정
    if candidates.len == 1:
        return Precise(candidates[0])                # approximate 라벨 제거
    else:
        return NameMatchFallback(callName)           # 기존 동작 + approximate 라벨
```

핵심은 **4번**: 후보가 1개로 좁혀질 때만 정밀 귀속하고, 아니면 현행 동작으로 폴백한다.

---

## 5. 구현 위치 (어디를 건드리나)

| 영역 | 파일 | 변경 내용 |
|------|------|----------|
| 언어별 규칙 | `src/lang/mod.rs` + `src/lang/{rust,python,typescript,...}.rs` | `LanguageSpec`에 import 문 파싱 / receiver 문법 / 타입 힌트 추출 훅 추가 |
| 심볼 메타 | `src/parser/types.rs` | 필요 시 `ExtractedFile`에 `imports: Vec<ImportEntry>` 추가 (alias → realName/source) |
| caller 스캔 | `src/callers/scan.rs` | 히트의 receiver/scope를 함께 수집(현재는 `(` 여부만 봄) |
| callee 발견 | `src/callers/callees.rs` | body 스캔 시 import alias 치환 + 로컬 정의 우선 적용 |
| 심볼 인덱스 | `src/callers/symbols.rs` | scope 우선순위 lookup 지원(같은 파일 > import > 전역) |
| 라벨링 | `src/callers/annotate.rs` | 1개로 좁혀지면 `approximate` 라벨 제거, 아니면 유지 |

> 인덱싱 파이프라인(`src/index/{engine,indexer,watcher}.rs`)의 갱신 모델은 **변경하지 않는다.** 파일 단위 증분과 호환되도록 해석 층도 파일 단위로 계산한다.

---

## 6. 단계별 롤아웃 (증분 도입)

각 단계는 독립적으로 머지 가능하며, 끝날 때마다 graceful degrade가 깨지지 않는지 확인한다.

1. **단계 1 — import 층 (TypeScript 1개 언어 PoC)**
   - `LanguageSpec`에 import 파싱 훅 + `ImportEntry` 추출
   - `callees.rs` / `scan.rs`에서 alias 치환만 적용
   - 가장 적은 변경으로 false negative를 눈에 띄게 줄임 → ROI 검증
2. **단계 2 — scope 우선순위**
   - `symbols.rs` lookup에 같은 파일 > import > 전역 순서 적용
3. **단계 3 — owner 층**
   - receiver + 지역 변수 타입 힌트로 동명이인 메서드 분리
4. **단계 4 — 언어 확장**
   - 단계 1~3이 검증되면 Python → Go → 나머지 언어 `LanguageSpec`에 규칙 추가

---

## 7. 비범위 (Out of Scope)

- 전체 타입 추론 / 제네릭 해석 / 흐름 분석 — 하지 않는다.
- LSP·SCIP·외부 indexer 연동 — 이 문서 범위 아님.
- 빌드/의존성 resolve, `node_modules` 내부 해석 — 하지 않는다(현행 제외 정책 유지).
- 동적 호출(`obj[name]()`), 리플렉션 — 폴백으로 남긴다.

---

## 8. 검증 아이디어 (가벼움)

- 동명이인·alias·로컬-shadow 케이스를 담은 작은 fixture 레포로 before/after 귀속 비교.
- 회귀 안전망: 해석 층이 1개로 못 좁히는 입력에서 **출력이 기존과 동일**한지 확인(폴백 보존 증명).
- 선택: 단일 언어에서 LSP/grep 결과와 대조해 정밀도(precision)·재현율(recall) 대략 측정.

---

## 부록: 한 줄 멘탈 모델

> "stack-graphs의 축소판을 이 도구에 맞게 직접 얹는다 — 단, 빌드 없이, tree-sitter와 기존 인덱스 데이터만으로, 모호하면 즉시 폴백."
