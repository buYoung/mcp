"""태그 값(tag-input.json)의 개념 단위 판정 묶음과 지침을 만든다. 지침은 unit-brief-repo.md에서 입력 설명만 바꾼다.

사용법: python3 prepare_tag_units.py <저장소 이름 ...>
출력: <작업 폴더>/batches/units-input/<저장소>/tNN.json, <작업 폴더>/batches/briefs/units-<저장소>-tNN.md
판정 결과는 <작업 폴더>/batches/units/<저장소>/tNN.json에 쓰게 한다(기존 uNN과 섞이지 않게 이름을 나눈다).
"""
import json
import sys
from pathlib import Path
from word_eval_paths import batches_dir, work_dir

from prepare_units import BATCH_SIZE, brief_for

WORK = work_dir()
BATCHES = batches_dir()
REPLACEMENTS = [
    ("저장소에 나오는 식별자 목록을 받습니다.",
     "저장소 코드의 태그 표기(속성·애너테이션·데코레이터) 안에 있는 값 목록을 받습니다. "
     "아래 규칙의 '식별자'는 이 값을 가리킵니다."),
    ("- `name`: 식별자\n", "- `name`: 태그 표기 안의 값\n"),
    ("- `where`: `defined`(이 저장소가 정의), `used_only`(쓰기만 함. 표준·외부 라이브러리 이름 등)\n",
     "- `where`: 모두 `tag_value`(속성·애너테이션·데코레이터 안의 문자열 값. 예: Rust `#[cfg(feature = \"sinks-loki\")]`의 "
     "`sinks-loki`, Java `@Path(\"/scope-mappings\")`의 `/scope-mappings`, Python `@app.route(\"/login\")`의 `/login`)\n"),
    ("- `kinds`: 정의 종류별 개수(function, method, class, variable, parameter, field, struct ...)\n",
     "- `kinds`: 태그 값에는 없음(null)\n"),
    ("- `qualifiers`: `http.StatusOK`의 `http`처럼 앞에 붙은 패키지·모듈·수신자·객체\n",
     "- `qualifiers`: 값이 들어 있는 표기와 키. 예: `cfg(feature)`, `Column(name)`, `app.route`\n"),
    ("1. 식별자를 빠짐없이 단위로 나눕니다.",
     "1. 식별자를 빠짐없이 단위로 나눕니다. 경로·URL·버전처럼 `/`, `:`, `.`, `{}`가 섞인 값도 구분 기호는 빼고 나머지를 모두 단위로 둡니다."),
]


def main(*repositories):
    for repository in repositories:
        items = json.loads((WORK / repository / "tag-input.json").read_text())
        if not items:
            print(repository, "태그 값 없음")
            continue
        brief = brief_for(repository)
        for old, new in REPLACEMENTS:
            assert brief.count(old) == 1, (repository, old)
            brief = brief.replace(old, new)
        input_directory = Path(f"{BATCHES}/units-input/{repository}")
        input_directory.mkdir(parents=True, exist_ok=True)
        Path(f"{BATCHES}/units/{repository}").mkdir(parents=True, exist_ok=True)
        batch_names = []
        for start in range(0, len(items), BATCH_SIZE):
            name = f"t{start // BATCH_SIZE + 1:02d}"
            (input_directory / f"{name}.json").write_text(json.dumps(items[start:start + BATCH_SIZE], ensure_ascii=False, indent=1))
            Path(f"{BATCHES}/briefs/units-{repository}-{name}.md").write_text(
                brief + f"\n## 배정\n입력 파일: `{input_directory / name}.json`\n결과 파일: `{BATCHES}/units/{repository}/{name}.json`\n")
            batch_names.append(name)
        print(repository, len(items), "항목", batch_names)


if __name__ == "__main__":
    main(*sys.argv[1:])
