"""3단계 준비: 사전 응답(lookups.jsonl)과 코드에서의 뜻(units.json)을 묶어 luna 판정 입력과 지침을 만든다.

사용법: python3 prepare_judge.py <저장소 이름>
입력: <저장소>/units.json, <저장소>/unit-input.json, <저장소>/lookups.jsonl, judge-brief-repo.md
출력: <저장소>/judge-input.json(전체), <작업 폴더>/batches/repo-judge-input/<저장소>/jNN.json(묶음),
      <작업 폴더>/batches/briefs/judge-<저장소>-jNN.md(지침). 판정 결과는 <작업 폴더>/batches/repo-judge/<저장소>/jNN.json
gin에서 만든 판정 입력과 같은 내용이며, 키 이름만 code_meaning·code_usage로 바꿨다.
묶음은 큰 응답부터 가장 가벼운 묶음에 넣어 크기를 고르게 한다(묶음 하나 약 220KB, gin 2회차와 같은 크기).
"""
import json
import math
import sys
from pathlib import Path
from word_eval_paths import SCRIPTS, batches_dir, work_dir

from prepare_units import DESCRIPTIONS

WORK = work_dir()
BATCHES = batches_dir()
BATCH_BYTES = 220 * 1024


def main(repository):
    collected_directory = WORK / repository
    units = {unit["text"]: unit for unit in json.loads((collected_directory / "units.json").read_text())}
    input_items = json.loads((collected_directory / "unit-input.json").read_text())
    lookups = [json.loads(line) for line in open(collected_directory / "lookups.jsonl")]
    items = build_items(units, input_items, lookups, "w")
    (collected_directory / "judge-input.json").write_text(json.dumps(items, ensure_ascii=False, indent=1))
    write_batches(repository, items, "j")


def build_items(units, input_items, lookups, id_prefix):
    """사전 응답과 코드에서의 뜻을 묶은 판정 입력. id는 <id_prefix>0001부터."""
    examples = {}
    for item in input_items:
        examples.setdefault(item["name"], item["examples"])
    items = []
    for lookup in lookups:
        unit = units[lookup["word"]]
        usage = [examples[name][0] for name in unit["sources"][:3] if examples.get(name)]
        items.append({"id": f"{id_prefix}{len(items) + 1:04d}", "word": lookup["word"],
                      "code_meaning": {"fields": list(unit["fields"]), "senses": unit["senses"], "expansions": unit["expansions"]},
                      "code_usage": {"identifiers": unit["sources"][:6], "code": usage},
                      "dictionary_output": lookup["output_text"], "_flag": None})
    return items


def write_batches(repository, items, batch_prefix):
    """판정 입력을 크기가 고른 묶음으로 나눠 묶음 파일과 지침을 쓴다. 묶음 이름은 <batch_prefix>01부터."""
    sizes = {item["id"]: len(json.dumps(item, ensure_ascii=False).encode()) for item in items}
    batch_count = max(1, math.ceil(sum(sizes.values()) / BATCH_BYTES))
    batches, loads = [[] for _ in range(batch_count)], [0] * batch_count
    for item in sorted(items, key=lambda x: -sizes[x["id"]]):
        index = loads.index(min(loads))
        batches[index].append(item)
        loads[index] += sizes[item["id"]]
    input_directory = Path(f"{BATCHES}/repo-judge-input/{repository}")
    input_directory.mkdir(parents=True, exist_ok=True)
    Path(f"{BATCHES}/repo-judge/{repository}").mkdir(parents=True, exist_ok=True)
    brief = ((SCRIPTS / "judge-brief-repo.md").read_text().replace("{repo}", repository)
             .replace("{description}", DESCRIPTIONS[repository]))
    for number, batch in enumerate(batches, 1):
        batch.sort(key=lambda x: x["id"])
        name = f"{batch_prefix}{number:02d}"
        (input_directory / f"{name}.json").write_text(
            json.dumps([{k: v for k, v in item.items() if k != "_flag"} for item in batch], ensure_ascii=False, indent=1))
        Path(f"{BATCHES}/briefs/judge-{repository}-{name}.md").write_text(
            brief + f"\n## 배정\n입력 파일: `{input_directory / name}.json`\n"
                    f"결과 파일: `{BATCHES}/repo-judge/{repository}/{name}.json`\n")
    print(repository, len(items), "단어", batch_count, "묶음", "묶음 크기(KB)", [round(load / 1024) for load in loads])
    return [f"{batch_prefix}{number:02d}" for number in range(1, batch_count + 1)]


if __name__ == "__main__":
    main(sys.argv[1])
