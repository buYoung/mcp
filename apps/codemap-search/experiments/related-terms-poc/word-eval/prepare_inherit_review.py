"""데이터셋 추가 2단계 보정: 파생어·약어가 기본형의 사전 뜻을 이어받아도 되는지 다시 판정할 묶음을 만든다.

w01~w43은 "기본형의 사전 뜻이 코드 쓰임과 다르면 이어받지 않는다"(disarm → ARM)와
"일부만 맞으면 inherit: true + 나머지 쓰임만 senses"(trimmed) 규칙이 생기기 전에 판정했다.
그 묶음의 derived·abbreviation_common add 가운데 기본형(원래 말)이 데이터셋에 뜻이 있는 항목을 모두 다시 본다.

사용법: python3 prepare_inherit_review.py
출력: gap/inherit-input.json, <작업 폴더>/batches/gap/inherit-input/iNN.json, <작업 폴더>/batches/briefs/gap-inherit-iNN.md
판정 결과는 <작업 폴더>/batches/gap/inherit/iNN.json에 쓰게 한다. collect_gap_results.py words가 (묶음, text)로 맞춰 덮어쓴다.
"""
import collections
import json
import sys
from pathlib import Path
from word_eval_paths import CHECKOUTS, EXPERIMENTS, SCRIPTS, batches_dir, work_dir

sys.path.insert(0, str(EXPERIMENTS))
from term_canonical import word_key  # noqa: E402

from prepare_gap_words import Dataset  # noqa: E402

WORK = work_dir()
BATCHES = batches_dir()
TMP = BATCHES / "gap"
REVIEWED_BATCHES = [f"w{n:02d}" for n in range(1, 44)]
BATCH_SIZE = 60


def main():
    dataset = Dataset()
    with_senses = set(dataset.head_fields) | set(dataset.input_fields)
    items = []
    for batch in REVIEWED_BATCHES:
        inputs = {item["text"]: item for item in json.loads((TMP / "word-input" / f"{batch}.json").read_text())}
        for result in json.loads((TMP / "word" / f"{batch}.json").read_text()):
            if result.get("decision") != "add" or result.get("type") not in ("derived", "abbreviation_common"):
                continue
            source = result.get("root") if result["type"] == "derived" else result.get("expansion")
            if not source or word_key(source) not in with_senses:
                continue
            item = inputs[result["text"]]
            items.append({"text": result["text"], "batch": batch, "type": result["type"], "source": source,
                          "source_senses": dataset.current_senses(source),
                          "repositories": list(item["repositories"]), "identifiers": item.get("identifiers", []),
                          "code": item.get("code", []), "analyst_senses": result.get("senses") or [],
                          "analyst_note": result.get("note", "")})
    (WORK / "gap" / "inherit-input.json").write_text(json.dumps(items, ensure_ascii=False, indent=1))
    input_directory, result_directory = TMP / "inherit-input", TMP / "inherit"
    input_directory.mkdir(parents=True, exist_ok=True)
    result_directory.mkdir(parents=True, exist_ok=True)
    brief = ((SCRIPTS / "gap-inherit-brief.md").read_text().replace("{catalog}", str(TMP / "catalog"))
             .replace("{checkouts}", str(CHECKOUTS)))
    names = []
    for start in range(0, len(items), BATCH_SIZE):
        name = f"i{start // BATCH_SIZE + 1:02d}"
        (input_directory / f"{name}.json").write_text(json.dumps(items[start:start + BATCH_SIZE], ensure_ascii=False, indent=1))
        Path(f"{BATCHES}/briefs/gap-inherit-{name}.md").write_text(
            brief + f"\n## 배정\n입력 파일: `{input_directory / name}.json`\n결과 파일: `{result_directory / name}.json`\n")
        names.append(name)
    kinds = collections.Counter((item["type"], "analyst_placed" if item["analyst_senses"] else "analyst_inherit") for item in items)
    print(len(items), dict(kinds), len(names), "묶음", names[0], "~", names[-1])


if __name__ == "__main__":
    main()
