"""Rebuild the prospectively authored contracts from their pinned source windows."""
from pathlib import Path
from benchmark.v2.core import read_json, write_json, digest
from benchmark.v2.dataset import source_text
from benchmark.v2.formal import PROTOCOL, validate

ROOT = Path('benchmark/v2/artifacts/formal-r1').resolve()
QUESTIONS = []


def question(repo, number, language, difficulty, tags, context, items):
    qid = f'{repo}-{number:02}'
    evidence = []
    facts = []
    for index, (requirement, correct, wrong, windows) in enumerate(items, 1):
        ids = []
        for path, start, end in windows:
            existing = next((e for e in evidence if (e['path'], e['start_line'], e['end_line']) == (path, start, end)), None)
            if existing is None:
                text = source_text(ROOT / 'repositories' / repo, path, start, end)
                existing = {'id': f'e{len(evidence)+1}', 'path': path, 'start_line': start, 'end_line': end,
                            'text': text, 'text_sha256': digest(text)}
                evidence.append(existing)
            ids.append(existing['id'])
        facts.append({'id': f'f{index}', 'requirement': requirement, 'correct': correct,
                      'conditions': [], 'wrong_claims': [wrong], 'confusions': [], 'evidence_sets': [ids]})
    refs = {e['id']: f"{e['path']}:{e['start_line']}-{e['end_line']}" for e in evidence}
    examples = []
    for kind in ['minimal', 'alternative', 'partial', 'wrong']:
        selected = facts if kind in {'minimal', 'alternative'} else facts[:1]
        if kind == 'wrong':
            answer = selected[0]['wrong_claims'][0] + '\n근거: ' + ', '.join(refs[e] for e in selected[0]['evidence_sets'][0])
        elif kind == 'alternative':
            answer = '확인한 근거: ' + '; '.join(refs.values()) + '\n' + '\n'.join(f['correct'] for f in reversed(selected))
        else:
            answer = '\n'.join(f['correct'] + ' (' + ', '.join(refs[e] for e in f['evidence_sets'][0]) + ')' for f in selected)
        judgments = []
        for fact in facts:
            is_correct = kind != 'wrong' and fact in selected
            judgments.append({'fact_id': fact['id'], 'correct': is_correct, 'supported': is_correct,
                              'evidence_ids': fact['evidence_sets'][0] if is_correct else [],
                              'answer_quote': fact['correct'] if is_correct else '',
                              'explanation': '정답과 근거를 명시함' if is_correct else '요구 사실을 올바르게 설명하지 않음'})
        expected = {'status': 'judged', 'has_final_answer': True, 'reason': '', 'facts': judgments,
                    'major_errors': [{'answer_quote': selected[0]['wrong_claims'][0], 'explanation': '질문에서 요구한 동작과 반대되는 주장'}] if kind == 'wrong' else []}
        examples.append({'id': kind, 'kind': kind, 'answer': answer, 'expected': expected})
    QUESTIONS.append({'id': qid, 'repository_id': repo, 'language': language, 'phase': 'main', 'difficulty': difficulty,
                      'difficulty_rationale': {'simple': '명시한 대상의 직접 구현 또는 자원 값 두 가지를 확인한다.',
                          'medium': '조건별 우선순위나 오류 분기를 구분하여 설명한다.',
                          'complex': '둘 이상의 구현 위치를 연결하여 전달·소비·상태 변화를 설명한다.'}[difficulty],
                      'tags': tags, 'source_reviewed': True, 'prompt': context + '\n' + '\n'.join(f'{i}. {f["requirement"]}' for i, f in enumerate(facts, 1)),
                      'facts': facts, 'evidence': evidence, 'examples': examples})


question('django', 1, 'python', 'simple', ['exact-symbol', 'headers'], 'Django의 patch_vary_headers 동작을 확인하라.', [
    ('기존 Vary 값과 대소문자가 다른 새 헤더를 어떻게 합치는가?', '기존 헤더 순서를 유지하고, 기존 값과 대소문자 구분 없이 같은 새 헤더는 추가하지 않는다.', '기존 Vary 값은 버리고 새 헤더만 사용한다.', [('django/utils/cache.py', 313, 325)]),
    ('합친 값에 별표가 있으면 최종 Vary 값은 무엇인가?', '합친 값에 *가 있으면 최종 Vary 값은 단일 *이다.', '별표가 있어도 다른 헤더와 쉼표로 이어 붙인다.', [('django/utils/cache.py', 325, 329)]),
])
question('django', 2, 'python', 'simple', ['resource-primary', 'locale'], '한국어 로캘의 날짜·숫자 형식 정의를 확인하라.', [
    ('DATE_FORMAT과 SHORT_DATE_FORMAT의 정확한 값은 무엇인가?', '한국어 DATE_FORMAT은 Y년 n월 j일이고 SHORT_DATE_FORMAT은 Y-n-j이다.', '한국어 DATE_FORMAT은 m/d/Y이다.', [('django/conf/locale/ko/formats.py', 5, 10)]),
    ('소수 구분자, 천 단위 구분자, 묶음 크기는 무엇인가?', '소수 구분자는 ., 천 단위 구분자는 ,이고 NUMBER_GROUPING은 3이다.', 'NUMBER_GROUPING은 4이다.', [('django/conf/locale/ko/formats.py', 52, 54)]),
])
question('django', 3, 'python', 'medium', ['precedence', 'locale'], '번역 구현에서 요청의 언어를 결정하는 흐름을 설명하라.', [
    ('check_path가 켜졌을 때 경로·언어 쿠키·Accept-Language의 우선순위는 무엇인가?', 'check_path가 켜져 경로에서 언어를 얻으면 먼저 반환하며, 그 다음 쿠키의 언어 또는 지원 변형을 시도하고, 그 다음 Accept-Language를 확인한다.', 'Accept-Language가 경로와 쿠키보다 먼저 적용된다.', [('django/utils/translation/trans_real.py', 585, 614)]),
    ('Accept-Language의 별표와 최종 기본 언어 처리는 어떻게 되는가?', 'Accept-Language의 *를 만나면 그 반복을 중단하고, 최종적으로 LANGUAGE_CODE의 지원 변형을 시도한 뒤 실패하면 LANGUAGE_CODE 자체를 반환한다.', 'Accept-Language의 *를 만나면 언어 코드 *를 반환한다.', [('django/utils/translation/trans_real.py', 603, 619)]),
])
question('django', 4, 'python', 'medium', ['fallback', 'validation'], 'CSRF 토큰 검사에서 면제되지 않은 요청의 제출 토큰 선택을 설명하라.', [
    ('POST 폼 토큰과 헤더 토큰 중 무엇을 선택하며, 빈 폼 값은 어떻게 처리하는가?', 'POST에서는 csrfmiddlewaretoken 값을 먼저 읽고, 그 값이 빈 문자열이면 CSRF_HEADER_NAME 헤더로 대체하며, 비어 있지 않은 폼 토큰은 헤더보다 우선한다.', 'CSRF 헤더가 항상 비어 있지 않은 POST 폼 토큰을 덮어쓴다.', [('django/middleware/csrf.py', 365, 389)]),
    ('secret이 없거나 선택한 토큰이 일치하지 않으면 어떻게 되는가?', 'CSRF secret이 없으면 REASON_NO_CSRF_COOKIE로 거부하고, 선택한 토큰이 secret과 일치하지 않으면 incorrect 사유로 거부한다.', 'secret이 없어도 헤더 토큰이 있으면 토큰 검사를 통과한다.', [('django/middleware/csrf.py', 353, 362), ('django/middleware/csrf.py', 391, 399)]),
])
question('django', 5, 'python', 'complex', ['cross-file', 'transactions'], 'transaction.on_commit의 등록부터 실행까지, 연결의 저장점과 robust 설정을 추적하라.', [
    ('공개 on_commit은 어느 연결 메서드로 전달되고 atomic 블록 안에서는 무엇을 저장하는가?', '공개 on_commit은 get_connection(using).on_commit(func, robust)로 전달하고, atomic 블록 안에서는 현재 savepoint_ids의 집합과 func, robust를 run_on_commit에 저장한다.', '공개 on_commit은 atomic 블록 안에서도 콜백을 즉시 실행한다.', [('django/db/transaction.py', 142, 147), ('django/db/backends/base/base.py', 727, 732)]),
    ('저장점으로 롤백할 때 어떤 콜백이 제거되는가?', '지원되는 저장점으로 롤백하면 등록 당시 저장점 집합에 해당 sid가 들어 있는 콜백을 제거한다.', '저장점 롤백은 등록된 콜백을 전혀 제거하지 않는다.', [('django/db/backends/base/base.py', 405, 420)]),
    ('커밋 훅 실행에서 robust=True의 예외 처리와 계속 실행 여부는 무엇인가?', '커밋 훅은 등록 순서로 꺼내 실행하고 robust=True 콜백의 Exception은 로그에 기록한 뒤 다음 콜백 실행을 계속한다.', 'robust=True 콜백의 Exception은 잡지 않고 즉시 바깥으로 전파한다.', [('django/db/backends/base/base.py', 749, 766)]),
])
question('axum', 1, 'rust', 'simple', ['exact-symbol', 'response'], 'Redirect의 세 생성자가 선택하는 상태 코드를 비교하라.', [
    ('Redirect::to가 선택하는 상태 코드는 무엇인가?', 'Redirect::to는 SEE_OTHER, 즉 303을 선택한다.', 'Redirect::to는 301을 선택한다.', [('axum/src/response/redirect.rs', 37, 39)]),
    ('temporary와 permanent가 선택하는 상태 코드는 각각 무엇인가?', 'temporary는 TEMPORARY_REDIRECT 307이고 permanent는 PERMANENT_REDIRECT 308이다.', 'temporary는 302이고 permanent는 301이다.', [('axum/src/response/redirect.rs', 41, 56)]),
])
question('axum', 2, 'rust', 'medium', ['validation', 'optional-extractor'], 'JSON 요청 추출의 Content-Type 조건을 확인하라.', [
    ('application/json과 application/problem+json, text/json 중 어떤 타입을 허용하는가?', 'application/json과 application/problem+json은 허용하지만 text/json은 허용하지 않는다. 주 타입이 application이고 하위 타입이나 접미사가 json이어야 한다.', 'text/json도 JSON 접미사만 있으면 허용한다.', [('axum/src/json.rs', 138, 147)]),
    ('선택적 Json 추출에서 Content-Type이 아예 없는 경우와 존재하지만 맞지 않는 경우는 어떻게 다른가?', '선택적 Json 추출은 Content-Type이 없으면 Ok(None)을 반환하고, 헤더가 있지만 JSON 타입이 아니면 MissingJsonContentType 오류를 반환한다.', '선택적 Json 추출은 잘못된 Content-Type도 모두 Ok(None)으로 처리한다.', [('axum/src/json.rs', 123, 135)]),
])
question('axum', 3, 'rust', 'medium', ['optional-extractor', 'errors'], '요청 확장 값 Extension<T>의 필수·선택적 추출을 비교하라.', [
    ('필수 Extension<T>를 찾지 못하면 어떤 오류와 응답 상태가 되는가?', '필수 Extension<T>가 없으면 MissingExtension 오류이고 응답 상태는 INTERNAL_SERVER_ERROR 500이다.', '필수 Extension<T>가 없으면 NOT_FOUND 404가 된다.', [('axum/src/extension.rs', 90, 97), ('axum/src/extract/rejection.rs', 42, 48)]),
    ('선택적 추출에서 값이 있거나 없을 때 무엇을 반환하고 기존 확장 값을 제거하는가?', '선택적 추출은 타입 T의 값을 clone하여 Some(Extension)을 반환하고 없으면 None이며, get을 사용하므로 원래 확장 값을 제거하지 않는다.', '선택적 추출은 값을 가져오면서 요청 확장에서 제거한다.', [('axum/src/extension.rs', 74, 80), ('axum/src/extension.rs', 100, 112)]),
])
question('axum', 4, 'rust', 'complex', ['cross-file', 'body-limit', 'option-propagation'], 'DefaultBodyLimit 설정이 Json 본문 추출에 적용되는 흐름을 설명하라.', [
    ('DefaultBodyLimit 레이어는 요청에 무엇을 기록하며 실제 바이트 제한은 어디서 적용하는가?', '레이어 서비스는 요청 extensions에 DefaultBodyLimitKind를 넣고, RequestExt::with_limited_body가 그 값을 읽어 Limited 본문을 구성한다.', 'DefaultBodyLimit 레이어가 요청을 받자마자 모든 본문을 직접 읽어 제한한다.', [('axum-core/src/extract/default_body_limit.rs', 224, 227), ('axum-core/src/ext_traits/request.rs', 316, 327)]),
    ('설정이 없을 때 기본 제한과 Disable이 있을 때의 처리는 무엇인가?', '설정이 없으면 2,097,152바이트 제한을 적용하고 Disable이면 요청을 그대로 반환해 이 기본 제한을 적용하지 않는다.', '설정이 없으면 제한이 없고 Disable이면 0바이트만 허용한다.', [('axum-core/src/ext_traits/request.rs', 319, 327)]),
    ('Json 추출은 어떤 본문 추출기를 거쳐 이 제한을 소비하는가?', 'Json::from_request는 Bytes::from_request를 호출하며 Bytes 추출기는 into_limited_body를 collect한다. into_limited_body는 with_limited_body를 거쳐 본문을 반환한다.', 'Json 추출은 Bytes를 거치지 않고 원본 본문을 직접 읽어 DefaultBodyLimit를 무시한다.', [('axum/src/json.rs', 106, 113), ('axum-core/src/extract/request_parts.rs', 101, 113), ('axum-core/src/ext_traits/request.rs', 330, 332)]),
])
question('axum', 5, 'rust', 'complex', ['cross-file', 'json', 'errors'], 'Json::from_bytes의 역직렬화 오류가 거절 응답으로 바뀌는 과정을 확인하라.', [
    ('serde_json의 Data 오류는 어떤 거절 타입과 상태 코드로 연결되는가?', 'Data 오류는 JsonDataError로 변환되고 UNPROCESSABLE_ENTITY 422 응답으로 연결된다.', 'Data 오류는 JsonSyntaxError로 변환되어 400이 된다.', [('axum/src/json.rs', 164, 170), ('axum/src/extract/rejection.rs', 9, 19)]),
    ('Syntax 또는 Eof 오류는 어떤 거절 타입과 상태 코드로 연결되는가?', 'Syntax와 Eof 오류는 JsonSyntaxError로 변환되고 BAD_REQUEST 400 응답으로 연결된다.', 'Syntax와 Eof 오류는 내부 서버 오류 500이 된다.', [('axum/src/json.rs', 167, 171), ('axum/src/extract/rejection.rs', 21, 30)]),
    ('값 하나를 성공적으로 읽은 후 남은 입력도 검사하는가?', '역직렬화 성공 후 deserializer.end()를 호출하며 남은 입력 검사 오류도 JsonSyntaxError로 처리한다.', '첫 JSON 값만 읽으면 뒤에 다른 값이 남아도 성공한다.', [('axum/src/json.rs', 184, 193)]),
])
question('vite', 1, 'typescript', 'simple', ['exact-symbol', 'configuration'], 'resolveEnvPrefix의 설정 검증을 확인하라.', [
    ('기본 접두사는 무엇이며 빈 문자열을 포함하면 어떻게 되는가?', '기본 접두사는 VITE_이며 접두사 목록에 빈 문자열이 있으면 Error를 던진다.', '빈 envPrefix는 모든 변수를 노출하도록 허용한다.', [('packages/vite/src/node/env.ts', 109, 117)]),
    ('공백이 포함된 접두사는 오류로 중단하는가?', '공백 문자가 포함된 접두사는 console.warn으로 경고하고 접두사 목록을 반환한다.', '공백이 포함된 접두사는 반드시 Error를 던져 처리를 중단한다.', [('packages/vite/src/node/env.ts', 118, 126)]),
])
question('vite', 2, 'typescript', 'medium', ['precedence', 'environment'], 'loadEnv의 파일 선택과 환경 변수 우선순위를 확인하라.', [
    ('envDir가 false이면 파일 목록은 어떻게 되고, 일반 mode에서는 어떤 순서인가?', 'envDir가 false이면 빈 목록이며, 그렇지 않으면 .env, .env.local, .env.<mode>, .env.<mode>.local 순서로 파일 경로를 만든다.', 'envDir가 false여도 현재 디렉터리의 .env를 읽는다.', [('packages/vite/src/node/env.ts', 13, 27)]),
    ('반환할 변수의 접두사 필터와 실제 process.env와의 최종 우선순위는 무엇인가?', 'parsed 값 중 설정한 접두사로 시작하는 키만 넣고, 마지막에 접두사가 맞는 process.env 값을 대입하므로 실제 process.env가 같은 키를 덮어쓴다.', '동일한 키가 있으면 .env 값이 실제 process.env보다 우선한다.', [('packages/vite/src/node/env.ts', 83, 102)]),
])
question('vite', 3, 'typescript', 'medium', ['assets', 'precedence'], '빌드 자원 인라인 여부를 정하는 shouldInline의 우선순위와 크기 조건을 설명하라.', [
    ('no-inline과 inline 쿼리가 모두 있으면 무엇이 우선하는가?', 'no-inline 검사가 먼저 false를 반환하므로 두 쿼리가 모두 있으면 인라인하지 않는다.', '두 쿼리가 모두 있으면 inline이 우선해 인라인한다.', [('packages/vite/src/node/plugins/asset.ts', 730, 740)]),
    ('명시적 우선 조건에 걸리지 않을 때 assetsInlineLimit 함수가 nullish를 반환하면 어떤 한도와 최종 조건을 쓰는가?', 'assetsInlineLimit 함수가 null 또는 undefined를 반환하면 DEFAULT_ASSETS_INLINE_LIMIT를 쓰고, content.length가 한도보다 작으며 Git LFS placeholder가 아닐 때만 인라인한다.', '한도와 파일 크기가 같아도 인라인하고 Git LFS placeholder도 허용한다.', [('packages/vite/src/node/plugins/asset.ts', 751, 759)]),
])
question('vite', 4, 'typescript', 'complex', ['cross-file', 'assets', 'public-files'], '자원 플러그인의 ?raw 요청이 public 디렉터리 파일을 처리하는 과정을 확인하라.', [
    ('raw 분기는 실제로 읽을 파일을 어떤 우선순위로 선택하고 무엇을 반환하는가?', 'checkPublicFile(id, config)가 찾은 파일을 우선 사용하고 없으면 cleanUrl(id)를 사용한다. UTF-8로 읽은 내용을 JSON.stringify하여 기본 내보내기 문자열 모듈을 반환한다.', 'raw 요청은 원문 대신 public URL 문자열만 내보낸다.', [('packages/vite/src/node/plugins/asset.ts', 244, 254)]),
    ('public 파일 확인은 publicDir와 URL 시작 문자에 어떤 조건을 두는가?', 'publicDir가 없거나 URL이 /로 시작하지 않으면 checkPublicFile은 undefined를 반환한다.', '상대 URL도 publicDir가 있으면 항상 public 파일로 처리한다.', [('packages/vite/src/node/publicDir.ts', 36, 45)]),
    ('public 파일 목록 캐시가 있을 때는 디스크 상태를 다시 검사하는가?', 'publicFiles 캐시가 있으면 cleanUrl로 얻은 파일명이 그 집합에 있는지로 즉시 반환하며, 캐시에 없다고 디스크 검사로 내려가지 않는다.', 'publicFiles 캐시에 없으면 항상 디스크를 다시 검사한다.', [('packages/vite/src/node/publicDir.ts', 47, 55)]),
])
question('vite', 5, 'typescript', 'complex', ['cross-file', 'routing', 'fallback'], '개발 서버의 HTML fallback 등록과 실제 URL 변경 조건을 추적하라.', [
    ('spa와 mpa에서 미들웨어를 등록하는가, 최종 SPA fallback은 두 경우 모두 켜지는가?', 'spa와 mpa 모두 htmlFallbackMiddleware를 등록하지만 spaFallback 인자는 appType이 spa일 때만 true다.', 'mpa에서도 spaFallback 인자를 true로 전달한다.', [('packages/vite/src/node/server/index.ts', 1062, 1071)]),
    ('어떤 HTTP 메서드와 Accept 조건을 허용하며 favicon 요청은 어떻게 되는가?', 'GET 또는 HEAD만 허용하고 /favicon.ico는 제외한다. Accept가 없거나 비었거나 text/html 또는 */*를 포함해야 다음 HTML 처리로 진행한다.', 'POST 요청도 Accept가 text/html이면 HTML fallback으로 처리한다.', [('packages/vite/src/node/server/middlewares/htmlFallback.ts', 26, 41)]),
    ('실제 HTML 파일 후보를 찾지 못했을 때 spaFallback이 참이면 URL을 무엇으로 바꾸는가?', '기존 HTML·디렉터리 index.html·.html 후보로 처리되지 않은 요청은 spaFallback이 참이면 req.url을 /index.html로 바꾼다.', 'spaFallback이 참이어도 파일 후보가 없으면 요청 URL을 바꾸지 않는다.', [('packages/vite/src/node/server/middlewares/htmlFallback.ts', 52, 83)]),
])
G = 'gson/src/main/java/com/google/gson/'
question('gson', 1, 'java', 'simple', ['exact-symbol', 'null-handling'], 'TypeAdapter.nullSafe()의 래퍼 동작을 확인하라.', [
    ('null 값을 쓰거나 JSON null을 읽을 때 원래 어댑터를 호출하는가?', 'null 쓰기는 out.nullValue()로 처리하고 JSON null 읽기는 nextNull()을 소비한 뒤 null을 반환하며 두 경우 원래 어댑터를 호출하지 않는다.', 'null 값도 원래 어댑터의 read 또는 write에 그대로 전달한다.', [(G+'TypeAdapter.java', 299, 315)]),
    ('이미 nullSafe 래퍼인 어댑터에 nullSafe()를 다시 호출하면 새 래퍼를 만드는가?', '이미 NullSafeTypeAdapter이면 this를 반환하여 새 래퍼를 만들지 않는다.', 'nullSafe()는 호출할 때마다 항상 새 래퍼를 중첩한다.', [(G+'TypeAdapter.java', 292, 297)]),
])
question('gson', 2, 'java', 'simple', ['exact-symbol', 'naming'], 'FieldNamingPolicy.LOWER_CASE_WITH_DOTS의 문자열 변환을 확인하라.', [
    ('대문자 구분과 소문자 변환에 어떤 구분자와 로캘을 쓰는가?', 'separateCamelCase에 . 구분자를 주고 Locale.ENGLISH로 소문자 변환한다.', '시스템 기본 로캘로만 소문자 변환한다.', [(G+'FieldNamingPolicy.java', 168, 172)]),
    ('aURL이라는 필드 이름은 어떻게 바뀌며 연속 대문자는 한 단어로 묶는가?', 'aURL은 a.u.r.l이 된다. 첫 문자가 아닌 각 대문자 앞에 구분자를 넣으므로 연속 대문자를 한 단어로 묶지 않는다.', 'aURL은 a.url이 되어 연속 대문자가 한 단어로 유지된다.', [(G+'FieldNamingPolicy.java', 168, 188)]),
])
question('gson', 3, 'java', 'medium', ['serialization', 'branching'], 'MapTypeAdapterFactory의 맵 직렬화 표현을 비교하라.', [
    ('complexMapKeySerialization이 꺼져 있을 때 키와 컨테이너를 어떻게 출력하는가?', 'JSON 객체를 만들고 각 키를 String.valueOf로 변환하여 이름으로 쓴다.', '옵션이 꺼져 있어도 모든 맵을 키·값 쌍의 배열로 쓴다.', [(G+'internal/bind/MapTypeAdapterFactory.java', 223, 231)]),
    ('옵션이 켜졌을 때 어떤 경우에 쌍의 배열을 쓰고 어떤 경우에 객체를 쓰는가?', '키 어댑터로 JSON 트리를 만든 뒤 키 중 하나라도 배열이나 객체이면 모든 항목을 [키, 값] 쌍의 배열로 쓰고, 그렇지 않으면 객체로 쓴다.', '옵션이 켜지면 키 종류와 무관하게 항상 객체로 쓴다.', [(G+'internal/bind/MapTypeAdapterFactory.java', 233, 260)]),
])
question('gson', 4, 'java', 'complex', ['cross-file', 'option-propagation', 'annotations'], 'excludeFieldsWithoutExposeAnnotation 설정이 반사 기반 필드 선택에 적용되는 흐름을 추적하라.', [
    ('GsonBuilder 설정은 Excluder의 어떤 상태를 바꾸는가?', 'GsonBuilder는 excluder.excludeFieldsWithoutExposeAnnotation()의 반환값으로 excluder를 바꾸고, 그 메서드는 복제본의 requireExpose를 true로 설정한다.', '설정 메서드는 Excluder의 상태를 바꾸지 않는다.', [(G+'GsonBuilder.java', 259, 262), (G+'internal/Excluder.java', 90, 94)]),
    ('설정이 켜졌을 때 @Expose가 없거나 방향별 플래그가 false이면 어떻게 되는가?', 'requireExpose가 켜졌으면 @Expose가 없는 필드를 제외하고, 직렬화 시 serialize=false 또는 역직렬화 시 deserialize=false인 필드도 해당 방향에서 제외한다.', '@Expose가 없어도 이 설정과 관계없이 모든 필드를 포함한다.', [(G+'internal/Excluder.java', 176, 182)]),
    ('반사 어댑터는 직렬화와 역직렬화를 따로 판정하며 양쪽 모두 제외되면 어떻게 하는가?', 'includeField는 excluder.excludeField의 부정값이며, 필드마다 true와 false 방향으로 각각 호출하고 양쪽 모두 false이면 그 필드를 건너뛴다.', '반사 어댑터는 직렬화 판정 하나를 역직렬화에도 그대로 사용한다.', [(G+'internal/bind/ReflectiveTypeAdapterFactory.java', 80, 82), (G+'internal/bind/ReflectiveTypeAdapterFactory.java', 350, 355)]),
])
question('gson', 5, 'java', 'complex', ['cross-file', 'runtime-types', 'precedence'], '반사 기반 필드 직렬화에서 선언 타입과 런타임 타입 어댑터 선택을 추적하라.', [
    ('필드 @JsonAdapter에서 어댑터를 얻은 경우에도 런타임 타입 래퍼를 씌우는가?', '필드 @JsonAdapter에서 어댑터를 얻었다면 그것을 writeTypeAdapter로 직접 사용하고, 그렇지 않은 직렬화 필드는 TypeAdapterRuntimeTypeWrapper로 감싼다.', '필드 @JsonAdapter 어댑터도 항상 런타임 타입 래퍼로 감싼다.', [(G+'internal/bind/ReflectiveTypeAdapterFactory.java', 195, 218)]),
    ('더 구체적인 런타임 타입의 어댑터가 반사 기반이고 선언 타입의 어댑터가 비반사 기반이면 무엇을 선택하는가?', '런타임 타입 어댑터가 반사 기반이고 기존 delegate가 비반사 기반이면 선언 타입의 delegate를 우선한다.', '이 경우에도 런타임 타입 반사 어댑터가 무조건 우선한다.', [(G+'internal/bind/TypeAdapterRuntimeTypeWrapper.java', 57, 76)]),
    ('래퍼의 read도 런타임 타입 선택을 수행하는가?', 'read는 런타임 타입 선택 없이 delegate.read(in)을 그대로 호출한다.', 'read도 값의 런타임 클래스를 먼저 조사해 어댑터를 바꾼다.', [(G+'internal/bind/TypeAdapterRuntimeTypeWrapper.java', 38, 41)]),
])
question('fmt', 1, 'cpp', 'simple', ['exact-symbol', 'strings'], 'fmt::format_int의 문자열 접근 메서드를 비교하라.', [
    ('data()와 c_str()는 종료 null 문자 처리에서 어떻게 다른가?', 'data()는 str_만 반환하고 종료 null을 붙이지 않으며, c_str()는 내부 버퍼 마지막 칸에 null 문자를 쓴 뒤 str_를 반환한다.', 'data()도 호출할 때마다 종료 null 문자를 쓴다.', [('include/fmt/format.h', 4461, 4470)]),
    ('str()는 문자열 길이를 어떻게 정하며 종료 null까지 포함하는가?', 'str()는 str_와 size()로 std::string을 만들며 size() 계산은 마지막 종료 null용 칸을 제외하므로 종료 null을 포함하지 않는다.', 'str()는 종료 null까지 문자열 길이에 포함한다.', [('include/fmt/format.h', 4456, 4459), ('include/fmt/format.h', 4472, 4473)]),
])
question('fmt', 2, 'cpp', 'simple', ['exact-symbol', 'memory'], 'basic_memory_buffer::grow의 용량 증가와 저장소 교체를 확인하라.', [
    ('할당기 최대 크기에 걸리지 않을 때 기존 용량 100, 요청 크기 120이면 새 용량은 얼마이고 요청이 200이면 얼마인가?', '기존 용량에 그 절반을 더하므로 요청 120에는 150을 사용하고, 요청 200은 계산된 150보다 크므로 200을 사용한다.', '기존 용량 100과 요청 120이면 무조건 200으로 두 배 확장한다.', [('include/fmt/format.h', 950, 960)]),
    ('기존 데이터는 어느 길이만큼 복사하며 내장 저장소도 해제하는가?', 'buf.size() * sizeof(T) 바이트를 복사하고 새 저장소를 설정하며, 이전 데이터가 내장 store_가 아닐 때만 할당기로 해제한다.', '이전 저장소가 내장 store_여도 항상 할당기로 해제한다.', [('include/fmt/format.h', 961, 971)]),
])
question('fmt', 3, 'cpp', 'medium', ['parser', 'state'], 'parse_context의 숫자 인자 인덱스 지정 방식을 확인하라.', [
    ('수동 인덱스 지정 후 next_arg_id()로 자동 인덱스를 요청하면 어떻게 되는가?', 'next_arg_id_가 음수이면 manual에서 automatic으로 전환할 수 없다는 오류를 보고한다.', '수동 인덱스 뒤의 자동 인덱스는 항상 0부터 정상 재시작한다.', [('include/fmt/base.h', 871, 880)]),
    ('자동 인덱스를 이미 사용한 뒤 check_arg_id(int)로 수동 인덱스를 요청하면 어떻게 되며 정상 수동 전환은 어떤 상태를 저장하는가?', 'next_arg_id_가 양수이면 automatic에서 manual로 전환할 수 없다는 오류를 보고하고, 정상 수동 전환은 next_arg_id_를 -1로 설정한다.', '자동 인덱스를 이미 사용해도 수동 인덱스로 자유롭게 전환할 수 있다.', [('include/fmt/base.h', 883, 892)]),
])
question('fmt', 4, 'cpp', 'complex', ['cross-file', 'truncation', 'compiled-format'], 'FMT_COMPILE 문자열을 받는 format_to_n의 출력 제한과 반환 개수를 추적하라. 출력 반복자는 std::back_inserter(std::string)인 경우로 설명하라.', [
    ('컴파일 문자열 오버로드는 어떤 버퍼 정책으로 n을 전달하고 어떤 함수로 포맷하는가?', 'fixed_buffer_traits를 쓰는 iterator_buffer에 out과 n을 전달하고 appender(buf)를 출력 대상으로 fmt::format_to를 호출한다.', '컴파일 문자열 경로는 n을 무시하고 원래 출력 반복자에 직접 전부 쓴다.', [('include/fmt/compile.h', 536, 544)]),
    ('fixed_buffer_traits는 한도를 넘어선 데이터도 count에 포함하는가?', 'limit(size)는 남은 허용량과 size 중 작은 값을 반환하지만 count_에는 size 전체를 더하므로 잘린 분량도 전체 개수에 포함한다.', 'count_에는 실제 복사한 문자 수만 더하므로 반환 크기는 n을 넘지 않는다.', [('include/fmt/base.h', 1871, 1884)]),
    ('최종 반환값의 out과 size는 무엇을 의미하는가?', 'buf.out()은 flush 후의 출력 반복자이고 buf.count()는 정책의 count와 아직 버퍼에 남은 크기를 더한다. 따라서 반환 size는 실제로 쓴 최대 n개가 아니라 잘리지 않았을 전체 출력 개수다.', '반환 size는 항상 실제로 출력한 문자 수이므로 잘린 전체 길이를 알 수 없다.', [('include/fmt/compile.h', 540, 544), ('include/fmt/base.h', 1967, 1971)]),
])
question('fmt', 5, 'cpp', 'complex', ['cross-file', 'io', 'partial-write'], 'fmt::ostream이 버퍼를 비울 때 파일 쓰기 결과를 소비하는 흐름을 확인하라.', [
    ('ostream::grow는 버퍼가 가득 찼을 때 용량을 재할당하는가?', 'ostream::grow는 size와 capacity가 같으면 flush를 호출하며 여기서 용량을 재할당하지 않는다.', 'ostream::grow는 가득 찬 버퍼를 두 배로 재할당한다.', [('src/os.cc', 380, 382)]),
    ('flush는 file::write의 반환 길이를 확인해 부분 쓰기의 나머지를 반복해서 쓰는가?', 'flush는 file_.write를 한 번 호출하고 반환 길이를 확인하지 않은 채 clear한다. 따라서 flush 자체에는 부분 쓰기의 나머지를 반복하는 로직이 없다.', 'flush는 실제 쓰인 길이를 누적하면서 버퍼 전체가 쓰일 때까지 반복한다.', [('include/fmt/os.h', 378, 382), ('src/os.cc', 280, 286)]),
    ('file::write의 최종 시스템 호출 결과가 음수이거나 비음수이면 각각 어떻게 처리하는가?', '최종 결과가 음수이면 cannot write to file이라는 system_error를 던지고, 비음수이면 실제 결과를 부호 없는 크기로 반환한다.', '최종 시스템 호출이 실패해도 요청한 count를 성공한 길이로 반환한다.', [('src/os.cc', 280, 286)]),
])

question('kubernetes-api', 1, 'go', 'simple', ['exact-symbol', 'matching'], 'core/v1.Taint의 비교와 문자열 표현을 확인하라.', [
    ('MatchTaint는 어떤 필드를 비교하며 Value가 달라도 일치할 수 있는가?', 'MatchTaint는 Key와 Effect만 비교하므로 두 값이 같으면 Value가 달라도 일치한다.', 'MatchTaint는 Key, Effect, Value가 모두 같아야 일치한다.', [('core/v1/taint.go', 21, 25)]),
    ('Effect는 비어 있고 Value는 있는 경우 ToString 형식은 무엇인가?', 'Effect가 비어 있고 Value가 있으면 key=value: 형식으로 마지막 콜론을 포함한다.', 'Effect가 없으면 key=value 형식으로 콜론을 붙이지 않는다.', [('core/v1/taint.go', 28, 34)]),
])
question('kubernetes-api', 2, 'go', 'simple', ['generated-primary', 'auxiliary-path-primary', 'protobuf'], 'core/v1.Toleration의 생성된 protobuf Size 계산을 확인하라.', [
    ('수신 포인터 자체가 nil이면 어떤 크기를 반환하는가?', 'Toleration 포인터가 nil이면 Size는 0을 반환한다.', 'nil Toleration의 Size는 포인터를 역참조해서 panic한다.', [('core/v1/generated.pb.go', 20608, 20611)]),
    ('TolerationSeconds가 nil인 경우와 0을 가리키는 비nil 포인터인 경우를 같은 방식으로 생략하는가?', 'TolerationSeconds는 포인터가 nil일 때만 생략한다. 0을 가리켜도 비nil이면 1 + sovGenerated(uint64(*m.TolerationSeconds))를 크기에 더한다.', 'TolerationSeconds가 0을 가리키면 nil 포인터와 똑같이 생략한다.', [('core/v1/generated.pb.go', 20622, 20625)]),
])
question('kubernetes-api', 3, 'go', 'medium', ['matching', 'feature-flag'], 'core/v1.Toleration이 taint를 허용하는 조건을 확인하라.', [
    ('Effect와 Key가 비어 있지 않을 때의 불일치와, 빈 Operator 및 Exists의 값 비교는 어떻게 처리하는가?', '비어 있지 않은 Effect 또는 Key가 taint와 다르면 false다. 그 검사를 통과하면 빈 Operator는 Equal처럼 Value를 비교하고 Exists는 Value와 무관하게 true다.', 'Exists이면 Effect나 Key가 불일치해도 항상 true다.', [('core/v1/toleration.go', 52, 67)]),
    ('Lt 또는 Gt인데 enableComparisonOperators가 false이면 숫자 비교를 하는가?', 'enableComparisonOperators가 false이면 Lt와 Gt는 숫자 비교 없이 false를 반환한다.', 'Lt와 Gt는 enableComparisonOperators가 false여도 숫자 비교를 수행한다.', [('core/v1/toleration.go', 68, 75)]),
])
question('kubernetes-api', 4, 'go', 'medium', ['generated-primary', 'auxiliary-path-primary', 'protobuf', 'determinism'], 'ConfigMap의 생성된 protobuf MarshalToSizedBuffer에서 맵 순서와 선택적 boolean 처리를 설명하라.', [
    ('Data와 BinaryData의 키를 Go 맵 순회 순서 그대로 쓰는가?', '두 맵 모두 키 목록을 모아 sort.Strings로 정렬하고 그 목록을 뒤에서 앞으로 순회하며 역방향 버퍼에 기록한다. Go 맵의 임의 순회 순서를 그대로 쓰지 않는다.', 'Data와 BinaryData는 Go 맵을 range한 순서를 그대로 protobuf에 기록한다.', [('core/v1/generated.pb.go', 1693, 1701), ('core/v1/generated.pb.go', 1719, 1727)]),
    ('Immutable이 nil일 때와 false를 가리킬 때는 어떻게 다른가?', 'Immutable이 nil이면 그 필드를 생략하지만 false를 가리키는 비nil 포인터이면 값 바이트 0과 필드 태그 0x20을 기록한다.', 'Immutable이 false를 가리키면 nil일 때처럼 필드를 생략한다.', [('core/v1/generated.pb.go', 1683, 1692)]),
])
question('kubernetes-api', 5, 'go', 'complex', ['cross-file', 'generated-primary', 'deep-copy'], 'ConfigMap의 BinaryData 타입에서 DeepCopyObject까지 복사 계약을 추적하라.', [
    ('BinaryData의 타입은 무엇이고 DeepCopyInto가 바깥 맵과 각 바이트 배열을 공유하는가?', 'BinaryData는 map[string][]byte이며 DeepCopyInto는 새 맵을 만들고 비nil 바이트 슬라이스마다 새 배열을 할당해 복사하므로 원본과 공유하지 않는다.', 'DeepCopyInto는 BinaryData의 바깥 맵만 새로 만들고 바이트 배열은 원본과 공유한다.', [('core/v1/types.go', 8466, 8467), ('core/v1/zz_generated.deepcopy.go', 602, 615)]),
    ('BinaryData 항목의 값이 nil이면 복사본에서 키를 삭제하는가?', 'nil 값의 항목도 복사본에 같은 키와 nil 값으로 남으며 삭제하지 않는다.', 'nil 값을 가진 BinaryData 항목은 복사하면서 삭제한다.', [('core/v1/zz_generated.deepcopy.go', 605, 615)]),
    ('수신자가 nil일 때 DeepCopy와 DeepCopyObject는 무엇을 반환하는가?', 'DeepCopy는 nil 수신자이면 nil을 반환하고 DeepCopyObject도 이 결과를 확인하여 nil을 반환한다.', 'nil 수신자에도 DeepCopyObject는 빈 ConfigMap을 담은 비nil runtime.Object를 반환한다.', [('core/v1/zz_generated.deepcopy.go', 621, 636)]),
])

if __name__ == '__main__':
    repos = {key: {k: v for k, v in value.items() if k != 'checkout'} for key, value in read_json(ROOT / 'repositories.json').items()}
    dataset = {'protocol': PROTOCOL, 'repositories': repos, 'questions': QUESTIONS,
               'selection_notes': ['Grafana preparation questions and results are excluded.',
                   'These repositories were not used for the observed product tuning in this session; model pretraining exposure is unknown.',
                   'Questions were purposively authored from fixed code rather than sampled from PRs.',
                   'Before formal execution, client-go was replaced by kubernetes/api because no initial repository matched the auxiliary-path heuristic.',
                   'Generated/resource tags describe answer sources; auxiliary-path-primary specifically matches the frozen path rule.']}
    validate(dataset, ROOT)
    write_json(ROOT / 'dataset.json', dataset)
    write_json(Path(__file__).with_name('dataset.json'), dataset)
    print(f'{len(QUESTIONS)} questions, {sum(len(q["facts"]) for q in QUESTIONS)} facts')
