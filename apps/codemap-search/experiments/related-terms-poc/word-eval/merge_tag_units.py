"""태그 값 기준(사용자 확정, 2026-10-11)을 단어평가셋에 반영한 2판(v2)을 만든다. 1판 파일은 그대로 둔다.

사용법:
  python3 merge_tag_units.py prepare <저장소 ...>   단위 다시 모으기 → 새 단어 조회 → luna 판정 묶음(tNN) 만들기
  python3 merge_tag_units.py combine <저장소 ...>   1판 판정 + 새 단어 판정을 합쳐 judge-summary-v2.json 쓰기
기준
  - 태그 표기 안의 값(tag-input.json, where=tag_value)을 식별자 입력에 더한다.
  - 테스트 파일의 태그 값은 넣지 않는다. gin의 struct 태그 항목은 모두 테스트 파일에만 있어(tag-test-only.json) 뺀다.
prepare 출력: <저장소>/units-v2.json, units-summary-v2.json, tag-merge.json(새 단어·빠진 단어),
             lookups-tags.jsonl, judge-input-tags.json, <작업 폴더>/batches/repo-judge-input/<저장소>/tNN.json, 지침
combine 출력: <저장소>/judge-results-tags.jsonl, judge-summary-tags.json(새 단어만), judge-summary-v2.json(합친 것)
"""
import json
import sys
from pathlib import Path
from word_eval_paths import EXPERIMENTS, batches_dir, work_dir

sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402

import aggregate_units  # noqa: E402
import lookup_words  # noqa: E402
import prepare_judge  # noqa: E402
import summarize_judge  # noqa: E402

WORK = work_dir()
BATCHES = batches_dir()
UNIT_JUDGEMENT_ROOT = BATCHES / "units"


def read_json(path):
    return json.loads(Path(path).read_text())


def inputs_and_judgements(repository):
    directory = WORK / repository
    inputs = read_json(directory / "unit-input.json")
    if repository == "gin":
        test_only = set(read_json(directory / "tag-test-only.json")["test_only"])
        inputs = [item for item in inputs if not (item["where"] == "struct_tag" and item["name"] in test_only)]
        judged_paths = sorted(UNIT_JUDGEMENT_ROOT.glob("u[0-9].json"))
    else:
        inputs = inputs + read_json(directory / "tag-input.json")
        judged_paths = sorted((UNIT_JUDGEMENT_ROOT / repository).glob("u*.json")) + \
            sorted((UNIT_JUDGEMENT_ROOT / repository).glob("t*.json"))
    judged = [record for path in judged_paths for record in read_json(path)]
    return inputs, judged


def judge_input_name(repository):
    return "judge-input-r2.json" if repository == "gin" else "judge-input.json"


def judge_results_name(repository):
    return "judge-results-r2.jsonl" if repository == "gin" else "judge-results.jsonl"


def prepare(repository):
    directory = WORK / repository
    inputs, judged = inputs_and_judgements(repository)
    rows, summary = aggregate_units.aggregate(inputs, judged, sl.DEFAULT_SENSE_DATASET)
    (directory / "units-v2.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1))
    (directory / "units-summary-v2.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))

    previous_words = [item["word"] for item in read_json(directory / judge_input_name(repository))]
    included = [row["text"] for row in rows if row["is_included"]]
    new_words = [word for word in included if word not in set(previous_words)]
    removed_words = [word for word in previous_words if word not in set(included)]
    merge = {"repository": repository, "previous_words": len(previous_words), "v2_words": len(included),
             "new_words": new_words, "removed_words": removed_words, "unit_problems": summary["problems"]}
    (directory / "tag-merge.json").write_text(json.dumps(merge, ensure_ascii=False, indent=1))

    batches = []
    if new_words:
        lookups = lookup_words.lookup_all(new_words)
        with open(directory / "lookups-tags.jsonl", "w") as handle:
            for row in lookups:
                handle.write(json.dumps(row, ensure_ascii=False) + "\n")
        units = {row["text"]: row for row in rows}
        items = prepare_judge.build_items(units, inputs, lookups, "t")
        (directory / "judge-input-tags.json").write_text(json.dumps(items, ensure_ascii=False, indent=1))
        batches = prepare_judge.write_batches(repository, items, "t")
    print(repository, f"1판 {len(previous_words)} → 2판 {len(included)}", f"새 단어 {len(new_words)}",
          f"빠진 단어 {len(removed_words)}", "단위 문제", summary["problems"], "판정 묶음", batches)


def combine(repository):
    directory = WORK / repository
    merge = read_json(directory / "tag-merge.json")
    removed = set(merge["removed_words"])
    rows = [json.loads(line) for line in open(directory / judge_results_name(repository))]
    items = {item["id"]: item for item in read_json(directory / judge_input_name(repository))}
    if merge["new_words"]:
        summarize_judge.main(directory, f"{BATCHES}/repo-judge/{repository}", "tags", "units-v2.json")
        rows += [json.loads(line) for line in open(directory / "judge-results-tags.jsonl")]
        items.update({item["id"]: item for item in read_json(directory / "judge-input-tags.json")})
    kept = [row for row in rows if row["word"] not in removed]
    kept_items = {item_id: item for item_id, item in items.items() if item["word"] not in removed}
    summary = summarize_judge.build_summary(kept, kept_items, {row["id"]: row for row in kept})
    summary["v2"] = {"previous_words": merge["previous_words"], "new_words": len(merge["new_words"]),
                     "removed_words": len(removed)}
    (directory / "judge-summary-v2.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))
    print(repository, "2판 판정", summary["judged"], "/", summary["expected"], summary["all"]["sense_result"])


if __name__ == "__main__":
    command, *repositories = sys.argv[1:]
    for name in repositories:
        {"prepare": prepare, "combine": combine}[command](name)
