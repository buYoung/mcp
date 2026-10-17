# Text and strings 누락 단어

- 분야 id: `text-processing`
- 원본: [../text-processing.md](../text-processing.md)
- 누락 단어 27개 · 사전에 없음 21 · 뜻 미배정 0 · 다른 분야에만 있음 6
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Identifier casing (`t266.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `dot case` | phrase | 사전에 없음 | 단어를 점으로 잇는 식별자 표기 방식이다. |

### KMP matching (`t685.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `string search algorithm` | phrase | 사전에 없음 | 부분 문자열 검색 알고리즘 전반을 가리키는 검색어다. |

### Regular expressions (`t560`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `unicode property escape` | phrase | 사전에 없음 | 정규식에서 \p 문법으로 유니코드 속성을 지정하는 방식이다. |

### String operations (`t160.8`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `strcat` | identifier | 다른 분야에만 있음: algorithms-data-structures — String concatenation | C에서 문자열 뒤에 다른 문자열을 붙이는 함수 이름이며 문자열 연산에서도 핵심이다. |
| `string slicing` | phrase | 사전에 없음 | 문자열을 범위로 잘라 부분 문자열을 얻는 작업이다. |

### String template expansion (`t595.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Django template` | phrase | 사전에 없음 | Django의 템플릿 문법과 엔진을 가리키는 말이다. |

### Tabular text formatting (`t994`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `box drawing` | phrase | 사전에 없음 | 표 테두리를 선 문자로 그리는 작업을 가리킨다. |
| `columnize` | identifier | 사전에 없음 | 목록을 열로 나눠 정렬하는 함수 이름이다. |

### Text encoding conversion (`t160.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `GB2312` | proper_noun | 사전에 없음 | 간체 중국어 문자 인코딩 이름이다. |

### Text line wrapping (`t227.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `wrap width` | phrase | 사전에 없음 | 줄바꿈 기준 너비를 가리키는 말이다. |

### Text sanitization (`t795`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `DOMPurify` | proper_noun | 다른 분야에만 있음: security — Browser XSS prevention | 브라우저에서 HTML을 정화하는 라이브러리이며 텍스트 정화에서도 쓰인다. |
| `maketrans` | identifier | 사전에 없음 | Python에서 문자 변환표를 만드는 함수다. |

### Unicode text handling (`t194`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `confusable` | word | 사전에 없음 | 모양이 비슷해 혼동되는 유니코드 문자를 가리킨다. |

### Unicode text normalization (`t266.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `UAX 15` | proper_noun | 사전에 없음 | 유니코드 정규화 형식을 정의한 표준 문서 번호다. |

### Whitespace normalization (`t683.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `soft tabs` | phrase | 사전에 없음 | 탭 대신 공백을 넣는 편집기 설정을 가리킨다. |

### Word dictionaries (`t817.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `aspell` | proper_noun | 사전에 없음 | GNU 철자 검사 도구 이름이다. |
| `codespell` | proper_noun | 사전에 없음 | 소스 코드의 흔한 오타를 찾는 도구 이름이다. |
| `cspell` | proper_noun | 사전에 없음 | 코드용 철자 검사 도구 이름이다. |

## 새 주제가 필요한 단어

### Binary-to-text encodings

범위: Encoding binary data as text with hex, base32, base58, ascii85, uuencode, and quoted-printable representations.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ascii85` | proper_noun | 사전에 없음 | Base85 계열 이진 데이터 텍스트 인코딩 이름이다. |
| `hex encoding` | phrase | 다른 분야에만 있음: math-numerics — Number base conversion | 바이트를 16진 문자로 표현하는 텍스트 인코딩이다. |
| `uuencode` | identifier | 사전에 없음 | 유닉스에서 이진 데이터를 텍스트로 바꾸던 인코딩 방식이다. |

### Character classification

범위: Testing whether a character is a letter, digit, space, upper or lower case, or printable.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `isLetter` | identifier | 사전에 없음 | Java와 Kotlin에서 글자인지 검사하는 메서드다. |
| `isLetterOrDigit` | identifier | 사전에 없음 | Java와 Kotlin에서 글자 또는 숫자인지 검사하는 메서드다. |
| `str.isnumeric` | identifier | 사전에 없음 | Python에서 숫자 문자로만 이뤄졌는지 검사하는 메서드다. |

### Fuzzy string matching

범위: Measuring string similarity and finding approximate matches with edit distance and phonetic algorithms.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `edit distance` | phrase | 다른 분야에만 있음: algorithms-data-structures — Sequence diff computation | 문자열을 바꾸는 데 필요한 최소 편집 횟수를 가리킨다. |
| `fuzzy search` | phrase | 다른 분야에만 있음: algorithms-data-structures — Trie autocomplete | 오타를 허용해 찾는 검색을 가리키는 말이다. |
| `Levenshtein` | proper_noun | 다른 분야에만 있음: editors-ide — Fuzzy file finding | 편집 거리를 정의한 이름으로 유사도 라이브러리에 쓰인다. |
