"""단어평가셋(2판) 단어가 데이터셋에 있고 분류되어 있는지 모은다(판정 없음, 기계 대조).

사용법: python3 dataset_coverage.py
분류됨: 표제어(head, 자기 뜻) 또는 입력 뜻(input, 다른 표제어 뜻 안의 관련어로 분류).
분류 없음: 관계 파일에만 있음(relation_only), 데이터셋에 없음(missing).
분야 대조: 분석가가 코드 쓰임으로 고른 분야가 데이터셋 분야(자기 뜻 + 입력 뜻)에 있는지 보는 이름 대조다. 뜻이 맞는지는 luna 판정을 본다.
출력: dataset-coverage-v2.json(단어별), dataset-coverage-v2.md(요약)
"""
import collections
import json
import sys
from word_eval_paths import EXPERIMENTS, work_dir

sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402
from term_canonical import word_key  # noqa: E402

import aggregate_units  # noqa: E402
from cross_repo_summary import REPOSITORIES  # noqa: E402

WORK = work_dir()
STATUSES = ("head", "input", "relation_only", "missing")


def dataset_fields():
    head_fields, input_fields, _ = aggregate_units.load_dataset(sl.DEFAULT_SENSE_DATASET)
    return {key: head_fields.get(key, set()) | input_fields.get(key, set()) for key in head_fields.keys() | input_fields.keys()}


def main():
    fields_by_key = dataset_fields()
    words = {}
    for repository in REPOSITORIES:
        directory = WORK / repository
        units_file = "units-v2.json" if (directory / "judge-summary-v2.json").exists() else "units.json"
        results_files = ["judge-results-r2.jsonl" if repository == "gin" else "judge-results.jsonl", "judge-results-tags.jsonl"]
        removed = set(json.loads((directory / "tag-merge.json").read_text())["removed_words"]) \
            if (directory / "tag-merge.json").exists() else set()
        results = {}
        for name in results_files:
            if (directory / name).exists():
                results.update({row["word"]: row["sense_result"] for row in map(json.loads, open(directory / name))})
        for unit in json.loads((directory / units_file).read_text()):
            if not unit["is_included"] or unit["text"] in removed:
                continue
            word = words.setdefault(unit["text"], {"word": unit["text"], "is_phrase": unit["is_phrase"],
                                                   "dataset_status": unit["dataset_status"], "code_fields": set(),
                                                   "repositories": {}, "occurrences": 0})
            word["code_fields"] |= set(unit["fields"]) - {None}
            word["repositories"][repository] = results.get(unit["text"])
            word["occurrences"] += unit["occurrences"]
    for word in words.values():
        data = fields_by_key.get(word_key(word["word"]), set())
        missing = word["code_fields"] - data
        word["field_coverage"] = None if word["dataset_status"] not in ("head", "input") else \
            "all" if not missing else "some" if len(missing) < len(word["code_fields"]) else "none"
        word["code_fields"], word["dataset_fields"] = sorted(word["code_fields"]), sorted(data)
    rows = sorted(words.values(), key=lambda w: (STATUSES.index(w["dataset_status"]), -w["occurrences"], w["word"]))
    (WORK / "dataset-coverage-v2.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1))

    total = len(rows)
    by_status = collections.Counter((w["dataset_status"], "문구" if w["is_phrase"] else "한 단어") for w in rows)
    coverage = collections.Counter((w["dataset_status"], w["field_coverage"]) for w in rows if w["field_coverage"])
    percent = lambda n: f"{n / total * 100:.1f}%"
    lines = ["# 단어평가셋(2판) 단어의 데이터셋 수록·분류 현황 (판정 없음, 기계 대조)", "",
             f"- 데이터셋: {sl.DEFAULT_SENSE_DATASET.parent.name}. 12개 저장소에서 겹치는 단어는 하나로 셈(서로 다른 단어 {total}개)",
             "- 분야 대조는 분야 이름만 비교한다. 뜻이 맞는지는 luna 판정(cross-repo-summary-v2.md)을 본다", "",
             "| 상태 | 뜻 | 한 단어 | 문구 | 합계 | 비율 |", "|---|---|---|---|---|---|"]
    meaning = {"head": "분류됨(자기 뜻)", "input": "분류됨(다른 표제어 뜻의 관련어)", "relation_only": "수록, 분류 없음", "missing": "없음"}
    for status in STATUSES:
        single, phrase = by_status[(status, "한 단어")], by_status[(status, "문구")]
        lines.append(f"| {status} | {meaning[status]} | {single} | {phrase} | {single + phrase} | {percent(single + phrase)} |")
    lines += ["", "## 분류된 단어의 분야 대조(코드 쓰임 분야가 데이터셋 분야에 있는가)", "",
              "| 상태 | 모두 있음 | 일부 있음 | 없음 |", "|---|---|---|---|"]
    for status in ("head", "input"):
        lines.append(f"| {status} | " + " | ".join(str(coverage[(status, k)]) for k in ("all", "some", "none")) + " |")
    for status in ("relation_only", "missing"):
        selected = [w for w in rows if w["dataset_status"] == status]
        lines += ["", f"## {status} 상위 60개 (등장 횟수 순, 저장소: 판정)", ""]
        lines += [f"- `{w['word']}` — " + ", ".join(f"{r}: {v}" for r, v in w["repositories"].items()) for w in selected[:60]]
    (WORK / "dataset-coverage-v2.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines[:20]))


if __name__ == "__main__":
    main()
