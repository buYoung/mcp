# Dates, time and localization 누락 단어

- 분야 id: `time-locale`
- 원본: [../time-locale.md](../time-locale.md)
- 누락 단어 15개 · 사전에 없음 7 · 뜻 미배정 0 · 다른 분야에만 있음 8
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Clock synchronization (`t117.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `NTP server` | phrase | 사전에 없음 | `NTP server`는 시간 서버와 시계 동기화를 설정하거나 설명하는 문서와 운영 도구에서 쓰입니다. |

### Date-time libraries (`t779`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Arrow` | proper_noun | 다른 분야에만 있음: data-engineering — Columnar formats; data-engineering — Dataframe table operations; frontend-ui — Anchored popup components; functional-programming — Callable function values | `Arrow`는 날짜 시간 라이브러리나 날짜 계산 API를 가리키는 말로 코드와 문서에서 쓰입니다. |

### Locale character encodings (`t283.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `localectl` | identifier | 사전에 없음 | `localectl`는 로케일 범주 환경 변수와 로케일 설정을 다루는 셸과 C 코드에서 쓰입니다. |

### Timers/intervals (`t888`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `debounce` | word | 다른 분야에만 있음: concurrency-async — Change event throttling; concurrency-async — Trigger debouncing | `debounce`는 이 분야의 타이머 범위에 적힌 디바운스와 스로틀 구현에서 쓰입니다. |
| `throttle` | word | 다른 분야에만 있음: algorithms-data-structures — Resource throttling; concurrency-async — Browser frame scheduling; concurrency-async — Change event throttling; distributed-systems — Concurrency limits; distributed-systems — Request rate limits | `throttle`는 이 분야의 타이머 범위에 적힌 디바운스와 스로틀 구현에서 쓰입니다. |

### Translation catalog lookup (`t315.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Babel` | proper_noun | 다른 분야에만 있음: browser-platform — Browser compatibility shims; frontend-ui — Framework plugin registration; languages-compilers — Source transform configuration | `Babel`는 번역 카탈로그와 메시지 파일을 만들고 찾는 코드와 도구에서 쓰입니다. |
| `config/locales` | identifier | 사전에 없음 | `config/locales`는 번역 카탈로그와 메시지 파일을 만들고 찾는 코드와 도구에서 쓰입니다. |
| `Transloco` | proper_noun | 사전에 없음 | `Transloco`는 번역 카탈로그와 메시지 파일을 만들고 찾는 코드와 도구에서 쓰입니다. |

### Translation memory interchange (`t315.7`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Locize` | proper_noun | 사전에 없음 | `Locize`는 번역 관리 서비스와 번역 작업 도구를 가리키는 문서에서 쓰입니다. |
| `Pontoon` | proper_noun | 사전에 없음 | `Pontoon`는 번역 관리 서비스와 번역 작업 도구를 가리키는 문서에서 쓰입니다. |
| `Tolgee` | proper_noun | 사전에 없음 | `Tolgee`는 번역 관리 서비스와 번역 작업 도구를 가리키는 문서에서 쓰입니다. |

## 새 주제가 필요한 단어

### Text direction in localization

범위: Handling right-to-left and left-to-right text direction when localizing user interfaces.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `LTR` | abbreviation | 다른 분야에만 있음: text-processing — Unicode text handling | `LTR`는 현지화된 화면의 글쓰기 방향과 레이아웃 반전을 다루는 코드와 스타일에서 쓰입니다. |
| `right-to-left` | phrase | 다른 분야에만 있음: text-processing — Unicode text handling | `right-to-left`는 현지화된 화면의 글쓰기 방향과 레이아웃 반전을 다루는 코드와 스타일에서 쓰입니다. |
| `RTL` | abbreviation | 다른 분야에만 있음: algorithms-data-structures — Lookup tables; mobile-desktop — Windows binary integration; testing — Component testing queries; text-processing — Unicode text handling | `RTL`는 현지화된 화면의 글쓰기 방향과 레이아웃 반전을 다루는 코드와 스타일에서 쓰입니다. |
| `text direction` | phrase | 다른 분야에만 있음: text-processing — Unicode text handling | `text direction`는 현지화된 화면의 글쓰기 방향과 레이아웃 반전을 다루는 코드와 스타일에서 쓰입니다. |
