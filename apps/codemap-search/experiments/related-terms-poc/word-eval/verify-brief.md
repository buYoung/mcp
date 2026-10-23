# Go 코드에서 정의된 이름 목록 만들기

배정된 Go 파일을 직접 읽고, 그 파일 안에서 **새 이름이 선언(정의)되는 위치**를 빠짐없이 나열해 주세요.
이 목록은 다른 방법으로 만든 목록과 대조할 기준으로 씁니다. 정확성과 빠짐없음이 가장 중요합니다.

## 작업 규칙
- 배정된 파일만 읽습니다. 다른 파일이나 디렉터리는 열지 않습니다.
- 원문을 직접 읽고 목록을 작성합니다. 코드를 파싱하거나 이름을 자동 추출하는 프로그램·스크립트(go/ast, gopls, ctags, tree-sitter, 정규식 추출 스크립트 등)는 쓰지 않습니다.
  파일 보기 명령(cat, sed -n, nl)이나 위치를 찾는 검색(grep -n)은 써도 되지만, 목록의 각 항목은 원문에서 직접 확인해야 합니다.
- 파일을 수정하지 않습니다. 결과 파일 하나만 씁니다.

## 셀 것 (kind 값)
- `package`: `package xxx`의 이름
- `import_alias`: import에서 붙인 별칭(`alias "net/http"`의 alias). `_`와 `.`은 제외
- `function`: 최상위 함수 `func Name(`
- `method`: 수신자가 있는 함수 `func (r Recv) Name(`
- `interface_method`: interface 타입 안에 나열된 메서드 이름
- `struct`: `type X struct {...}`
- `interface`: `type X interface {...}`
- `type`: 그 밖의 타입 정의 `type X int`, `type X func(...)`, `type X map[...]...` 등
- `type_alias`: `type X = Y`
- `type_parameter`: 제네릭 타입 매개변수 `[T any]`. 수신자의 `func (s *S[T])`에 쓰인 T도 포함
- `constant`: `const` 선언의 이름(최상위·함수 안 모두)
- `variable`: `var` 선언의 이름(최상위·함수 안 모두), 그리고 `:=` 왼쪽의 모든 이름
  (`x, err := f()`, `for i, v := range xs`, `if v, ok := m[k]; ok`, `switch t := x.(type)`, `case msg := <-ch` 포함).
  이미 있던 변수를 `:=`로 다시 쓰는 경우도 `:=` 왼쪽에 있으면 셉니다.
- `field`: 이름이 있는 struct 필드(익명 struct 안의 필드 포함). 이름 없이 타입만 있는 임베디드 필드(`*Embedded`, `io.Reader`)는 제외
- `parameter`: 함수·메서드 매개변수, 수신자 이름, 이름 붙은 반환값, 함수 리터럴(`func(x int) {...}`)의 매개변수,
  함수 타입(`type F func(c *Context)`)과 interface 메서드 시그니처에 적힌 매개변수 이름
- `label`: 문장 레이블(`Loop:`)

## 세지 않을 것
- 이미 선언된 이름을 쓰는 곳(호출, 참조, `=` 대입, 필드 접근 `a.B`)
- struct 리터럴의 키(`Foo{Name: x}`의 Name)
- 빈 식별자 `_`
- 별칭 없는 import 경로, 문자열 내용, 주석, 빌드 태그

## 결과 형식
JSON 배열 하나를 결과 파일에 씁니다. 항목마다:
`{"file": "<배정 목록에 적힌 상대 경로>", "line": <이름이 있는 줄 번호, 1부터>, "name": "<이름>", "kind": "<위 kind 값>"}`
한 줄에 이름이 여럿이면(`a, b := ...`) 항목도 여럿입니다. 판단이 애매했던 항목은 `"note"`에 이유를 적습니다.
마지막 답변에는 파일별 항목 수와 애매했던 경우만 짧게 적습니다.
