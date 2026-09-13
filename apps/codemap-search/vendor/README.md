# 내장 파서 수정본

공개 저장소 검증에서 재현된 Groovy·Zsh·Kotlin 결함을 수정한 파서를 빌드에 포함한다. 원본 저장소와 고정 커밋, 생성된 C 파일의 해시는 [grammar-sources.json](grammar-sources.json)에 기록한다. `upstream.patch`는 해당 원본에 적용한 수정을 보여 준다.

| 파서 | 변경 | 근거 |
| --- | --- | --- |
| Groovy | 메서드와 최상위 함수의 문자열 이름, 세미콜론 없는 `assert`, 안전·전개 멤버 접근, 애너테이션 멤버를 인식한다. 클래스 필드·생성자 위임의 줄바꿈 종료, 메서드의 괄호 없는 호출, 직접 필드 접근과 trait도 처리한다. 정적인 이름의 이스케이프는 복원하고 동적 보간 이름은 확정하지 않는다. | [원본 grammar.js](https://github.com/amaanq/tree-sitter-groovy/blob/70efb0b9b50f95bcbd89dcfd42b275e0304e10cf/grammar.js), Gradle의 Spock 메서드 |
| Zsh | 패턴의 문자 집합 안에 있는 괄호를 일반 문자로 처리하고, 정규식 스캐너의 길이 0인 토큰과 깊이 카운터 언더플로를 막는다. `:`로 시작하는 함수·명령 이름, 마지막 세미콜론이 없는 조건 목록, `<>`·`>!`·`>>!`·`>>|`, `if [[ … ]] { … }` 형태를 인식한다. 다른 축약 조건문 형태까지 포괄하지 않는다. | [재현 사례 #37](https://github.com/georgeharker/tree-sitter-zsh/issues/37), Zinit `zinit-autoload.zsh` |
| Kotlin | 애너테이션이 붙은 이름 있는 함수 선언에 동적 우선순위를 주어 중위 표현식으로 오인하지 않게 한다. | Signal Android `ParticipantActionsSheet.kt`의 `AdminPreview` |

세 파서는 MIT로 배포된 소스를 사용한다. Zsh의 원본 라이선스와 Groovy가 기반으로 삼은 Java 문법의 라이선스를 각 디렉터리에 보존한다. Kotlin 라이선스도 고정한 원본 커밋에서 가져와 보존한다. Groovy 원본은 `grammar.js`의 저작자·MIT 표기를 유지한다.

## 빌드와 생성 파일

약 66 MiB인 생성 C 파일을 gzip으로 보관해 저장소와 소스 배포 패키지의 크기를 줄인다. `build.rs`가 `flate2`로 Cargo의 `OUT_DIR`에 풀고, 기존 문법 의존성에서도 사용하는 `cc`로 컴파일한다. `flate2`는 이 압축 해제를 위한 빌드 의존성이다. 사용자 실행 환경에서는 C 파일의 압축 해제나 문법 생성 작업이 발생하지 않는다.

Groovy의 `parser.c.gz`는 Tree-sitter CLI **0.24.4**, 언어 ABI **14**, 함께 보관한 Java 문법 **0.23.4**로 생성했다. 패키지 루트에서 다음 명령으로 원문을 재생성하고 현재 압축 파일과 대조할 수 있다. 문법을 수정했다면 대조에 실패하는 것이 정상이며, 원문 검토 후 gzip과 해시를 함께 갱신해야 한다.

```sh
cd vendor/tree-sitter-groovy
/path/to/tree-sitter-0.24.4 generate --abi 14
python3 - <<'PY'
from pathlib import Path
import gzip
assert Path('src/parser.c').read_bytes() == gzip.decompress(Path('src/parser.c.gz').read_bytes())
PY
```

Zsh와 Kotlin은 Tree-sitter CLI **0.25.6**, 언어 ABI **15**로 재생성했다. 위 명령에서 디렉터리를 `vendor/tree-sitter-zsh` 또는 `vendor/tree-sitter-kotlin`, 생성기를 `tree-sitter-0.25.6`, ABI를 `15`로 바꾸면 같은 방식으로 대조할 수 있다. 기존 5000ms 파싱 제한은 다른 비정상 입력에 대한 방어로 유지한다.

기본 빌드는 보관된 `parser.c.gz`를 사용한다. 문법을 고칠 때는 생성 C와 gzip을 함께 갱신하고, 파서 회귀 검사와 해당 공개 저장소 검증을 실행한다. 새 추출 결과가 필요한 변경에는 `EXTRACTION_FORMAT_VERSION`도 올린다.

Tree-sitter의 생성 테이블 인덱스는 16비트 범위여야 한다. 문법 확장 중 이 한도를 넘는 생성물이 실제로 관측되어 채택하지 않았다. 재생성 시 `ACTIONS(...)` 인덱스가 65535 이하인지와 기본 입력·회귀 사례의 실제 파싱을 함께 확인한다. 생성 성공만으로 정상 파서라고 판단하지 않는다.
