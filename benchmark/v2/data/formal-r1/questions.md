# 정식 평가 질문 30개

정답 계약과 코드 근거는 dataset.json, 방법과 제한은 README.md를 따른다.

## tokio-rs/axum

고정 커밋: `715e9a8950b910dd6bb71aac48ce3633c9359ed0`

### axum-01 · 단순

Redirect의 세 생성자가 선택하는 상태 코드를 비교하라.
1. Redirect::to가 선택하는 상태 코드는 무엇인가?
2. temporary와 permanent가 선택하는 상태 코드는 각각 무엇인가?

### axum-02 · 중간

JSON 요청 추출의 Content-Type 조건을 확인하라.
1. application/json과 application/problem+json, text/json 중 어떤 타입을 허용하는가?
2. 선택적 Json 추출에서 Content-Type이 아예 없는 경우와 존재하지만 맞지 않는 경우는 어떻게 다른가?

### axum-03 · 중간

요청 확장 값 Extension<T>의 필수·선택적 추출을 비교하라.
1. 필수 Extension<T>를 찾지 못하면 어떤 오류와 응답 상태가 되는가?
2. 선택적 추출에서 값이 있거나 없을 때 무엇을 반환하고 기존 확장 값을 제거하는가?

### axum-04 · 복잡

DefaultBodyLimit 설정이 Json 본문 추출에 적용되는 흐름을 설명하라.
1. DefaultBodyLimit 레이어는 요청에 무엇을 기록하며 실제 바이트 제한은 어디서 적용하는가?
2. 설정이 없을 때 기본 제한과 Disable이 있을 때의 처리는 무엇인가?
3. Json 추출은 어떤 본문 추출기를 거쳐 이 제한을 소비하는가?

### axum-05 · 복잡

Json::from_bytes의 역직렬화 오류가 거절 응답으로 바뀌는 과정을 확인하라.
1. serde_json의 Data 오류는 어떤 거절 타입과 상태 코드로 연결되는가?
2. Syntax 또는 Eof 오류는 어떤 거절 타입과 상태 코드로 연결되는가?
3. 값 하나를 성공적으로 읽은 후 남은 입력도 검사하는가?

## django/django

고정 커밋: `e2a3da142687c3b3dfa9cbc63c1a1a4433a8f842`

### django-01 · 단순

Django의 patch_vary_headers 동작을 확인하라.
1. 기존 Vary 값과 대소문자가 다른 새 헤더를 어떻게 합치는가?
2. 합친 값에 별표가 있으면 최종 Vary 값은 무엇인가?

### django-02 · 단순

한국어 로캘의 날짜·숫자 형식 정의를 확인하라.
1. DATE_FORMAT과 SHORT_DATE_FORMAT의 정확한 값은 무엇인가?
2. 소수 구분자, 천 단위 구분자, 묶음 크기는 무엇인가?

### django-03 · 중간

번역 구현에서 요청의 언어를 결정하는 흐름을 설명하라.
1. check_path가 켜졌을 때 경로·언어 쿠키·Accept-Language의 우선순위는 무엇인가?
2. Accept-Language의 별표와 최종 기본 언어 처리는 어떻게 되는가?

### django-04 · 중간

CSRF 토큰 검사에서 면제되지 않은 요청의 제출 토큰 선택을 설명하라.
1. POST 폼 토큰과 헤더 토큰 중 무엇을 선택하며, 빈 폼 값은 어떻게 처리하는가?
2. secret이 없거나 선택한 토큰이 일치하지 않으면 어떻게 되는가?

### django-05 · 복잡

transaction.on_commit의 등록부터 실행까지, 연결의 저장점과 robust 설정을 추적하라.
1. 공개 on_commit은 어느 연결 메서드로 전달되고 atomic 블록 안에서는 무엇을 저장하는가?
2. 저장점으로 롤백할 때 어떤 콜백이 제거되는가?
3. 커밋 훅 실행에서 robust=True의 예외 처리와 계속 실행 여부는 무엇인가?

## fmtlib/fmt

고정 커밋: `d3b2fac5c997510c2dd646268ae7a5762335637b`

### fmt-01 · 단순

fmt::format_int의 문자열 접근 메서드를 비교하라.
1. data()와 c_str()는 종료 null 문자 처리에서 어떻게 다른가?
2. str()는 문자열 길이를 어떻게 정하며 종료 null까지 포함하는가?

### fmt-02 · 단순

basic_memory_buffer::grow의 용량 증가와 저장소 교체를 확인하라.
1. 할당기 최대 크기에 걸리지 않을 때 기존 용량 100, 요청 크기 120이면 새 용량은 얼마이고 요청이 200이면 얼마인가?
2. 기존 데이터는 어느 길이만큼 복사하며 내장 저장소도 해제하는가?

### fmt-03 · 중간

parse_context의 숫자 인자 인덱스 지정 방식을 확인하라.
1. 수동 인덱스 지정 후 next_arg_id()로 자동 인덱스를 요청하면 어떻게 되는가?
2. 자동 인덱스를 이미 사용한 뒤 check_arg_id(int)로 수동 인덱스를 요청하면 어떻게 되며 정상 수동 전환은 어떤 상태를 저장하는가?

### fmt-04 · 복잡

FMT_COMPILE 문자열을 받는 format_to_n의 출력 제한과 반환 개수를 추적하라. 출력 반복자는 std::back_inserter(std::string)인 경우로 설명하라.
1. 컴파일 문자열 오버로드는 어떤 버퍼 정책으로 n을 전달하고 어떤 함수로 포맷하는가?
2. fixed_buffer_traits는 한도를 넘어선 데이터도 count에 포함하는가?
3. 최종 반환값의 out과 size는 무엇을 의미하는가?

### fmt-05 · 복잡

fmt::ostream이 버퍼를 비울 때 파일 쓰기 결과를 소비하는 흐름을 확인하라.
1. ostream::grow는 버퍼가 가득 찼을 때 용량을 재할당하는가?
2. flush는 file::write의 반환 길이를 확인해 부분 쓰기의 나머지를 반복해서 쓰는가?
3. file::write의 최종 시스템 호출 결과가 음수이거나 비음수이면 각각 어떻게 처리하는가?

## google/gson

고정 커밋: `b3f4ca20087f9066de4c340522ff84e0558e1ad1`

### gson-01 · 단순

TypeAdapter.nullSafe()의 래퍼 동작을 확인하라.
1. null 값을 쓰거나 JSON null을 읽을 때 원래 어댑터를 호출하는가?
2. 이미 nullSafe 래퍼인 어댑터에 nullSafe()를 다시 호출하면 새 래퍼를 만드는가?

### gson-02 · 단순

FieldNamingPolicy.LOWER_CASE_WITH_DOTS의 문자열 변환을 확인하라.
1. 대문자 구분과 소문자 변환에 어떤 구분자와 로캘을 쓰는가?
2. aURL이라는 필드 이름은 어떻게 바뀌며 연속 대문자는 한 단어로 묶는가?

### gson-03 · 중간

MapTypeAdapterFactory의 맵 직렬화 표현을 비교하라.
1. complexMapKeySerialization이 꺼져 있을 때 키와 컨테이너를 어떻게 출력하는가?
2. 옵션이 켜졌을 때 어떤 경우에 쌍의 배열을 쓰고 어떤 경우에 객체를 쓰는가?

### gson-04 · 복잡

excludeFieldsWithoutExposeAnnotation 설정이 반사 기반 필드 선택에 적용되는 흐름을 추적하라.
1. GsonBuilder 설정은 Excluder의 어떤 상태를 바꾸는가?
2. 설정이 켜졌을 때 @Expose가 없거나 방향별 플래그가 false이면 어떻게 되는가?
3. 반사 어댑터는 직렬화와 역직렬화를 따로 판정하며 양쪽 모두 제외되면 어떻게 하는가?

### gson-05 · 복잡

반사 기반 필드 직렬화에서 선언 타입과 런타임 타입 어댑터 선택을 추적하라.
1. 필드 @JsonAdapter에서 어댑터를 얻은 경우에도 런타임 타입 래퍼를 씌우는가?
2. 더 구체적인 런타임 타입의 어댑터가 반사 기반이고 선언 타입의 어댑터가 비반사 기반이면 무엇을 선택하는가?
3. 래퍼의 read도 런타임 타입 선택을 수행하는가?

## kubernetes/api

고정 커밋: `696b79c644cf23cc92b470ce570a0a37857ecdf1`

### kubernetes-api-01 · 단순

core/v1.Taint의 비교와 문자열 표현을 확인하라.
1. MatchTaint는 어떤 필드를 비교하며 Value가 달라도 일치할 수 있는가?
2. Effect는 비어 있고 Value는 있는 경우 ToString 형식은 무엇인가?

### kubernetes-api-02 · 단순

core/v1.Toleration의 생성된 protobuf Size 계산을 확인하라.
1. 수신 포인터 자체가 nil이면 어떤 크기를 반환하는가?
2. TolerationSeconds가 nil인 경우와 0을 가리키는 비nil 포인터인 경우를 같은 방식으로 생략하는가?

### kubernetes-api-03 · 중간

core/v1.Toleration이 taint를 허용하는 조건을 확인하라.
1. Effect와 Key가 비어 있지 않을 때의 불일치와, 빈 Operator 및 Exists의 값 비교는 어떻게 처리하는가?
2. Lt 또는 Gt인데 enableComparisonOperators가 false이면 숫자 비교를 하는가?

### kubernetes-api-04 · 중간

ConfigMap의 생성된 protobuf MarshalToSizedBuffer에서 맵 순서와 선택적 boolean 처리를 설명하라.
1. Data와 BinaryData의 키를 Go 맵 순회 순서 그대로 쓰는가?
2. Immutable이 nil일 때와 false를 가리킬 때는 어떻게 다른가?

### kubernetes-api-05 · 복잡

ConfigMap의 BinaryData 타입에서 DeepCopyObject까지 복사 계약을 추적하라.
1. BinaryData의 타입은 무엇이고 DeepCopyInto가 바깥 맵과 각 바이트 배열을 공유하는가?
2. BinaryData 항목의 값이 nil이면 복사본에서 키를 삭제하는가?
3. 수신자가 nil일 때 DeepCopy와 DeepCopyObject는 무엇을 반환하는가?

## vitejs/vite

고정 커밋: `5bce8ca0f19bfdd5206c98bfabc1f8453deb32c7`

### vite-01 · 단순

resolveEnvPrefix의 설정 검증을 확인하라.
1. 기본 접두사는 무엇이며 빈 문자열을 포함하면 어떻게 되는가?
2. 공백이 포함된 접두사는 오류로 중단하는가?

### vite-02 · 중간

loadEnv의 파일 선택과 환경 변수 우선순위를 확인하라.
1. envDir가 false이면 파일 목록은 어떻게 되고, 일반 mode에서는 어떤 순서인가?
2. 반환할 변수의 접두사 필터와 실제 process.env와의 최종 우선순위는 무엇인가?

### vite-03 · 중간

빌드 자원 인라인 여부를 정하는 shouldInline의 우선순위와 크기 조건을 설명하라.
1. no-inline과 inline 쿼리가 모두 있으면 무엇이 우선하는가?
2. 명시적 우선 조건에 걸리지 않을 때 assetsInlineLimit 함수가 nullish를 반환하면 어떤 한도와 최종 조건을 쓰는가?

### vite-04 · 복잡

자원 플러그인의 ?raw 요청이 public 디렉터리 파일을 처리하는 과정을 확인하라.
1. raw 분기는 실제로 읽을 파일을 어떤 우선순위로 선택하고 무엇을 반환하는가?
2. public 파일 확인은 publicDir와 URL 시작 문자에 어떤 조건을 두는가?
3. public 파일 목록 캐시가 있을 때는 디스크 상태를 다시 검사하는가?

### vite-05 · 복잡

개발 서버의 HTML fallback 등록과 실제 URL 변경 조건을 추적하라.
1. spa와 mpa에서 미들웨어를 등록하는가, 최종 SPA fallback은 두 경우 모두 켜지는가?
2. 어떤 HTTP 메서드와 Accept 조건을 허용하며 favicon 요청은 어떻게 되는가?
3. 실제 HTML 파일 후보를 찾지 못했을 때 spaFallback이 참이면 URL을 무엇으로 바꾸는가?
