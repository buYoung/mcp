"""데이터셋 추가(2026-10-11) 뒤 단어평가셋 3판(v3). 1·2판 파일은 그대로 둔다.

사용법:
  python3 prepare_v3.py prepare <새 데이터셋 sqlite> <저장소 ...>
  python3 prepare_v3.py combine <저장소 ...>
기준(사용자 확정 15번): 1단계에서 나눈(split) 문구는 평가셋에서 빼고, 그 구성 단어를 평가셋에 넣는다.
prepare
  - 2판 단어(2판이 없는 저장소는 1판) 가운데 나눈 문구를 빼고, 그 저장소 평가셋에 없던 구성 단어를 더한다.
    구성 단어의 코드 뜻·식별자는 원래 문구의 것을 쓰고, 뜻 앞에 "part of '<문구>':"를 붙인다.
  - 3판 단어 전부를 기존 기본 데이터셋과 새 데이터셋으로 조회해 응답이 바뀐 단어를 센다.
  - 다시 판정할 단어: 새 구성 단어 + 새 데이터셋에서 뜻이 생기거나 늘어난 단어(gap/entries.json의 personas·inherit 단어)
    + 뜻 묶음(## 제목)이 바뀐 단어. 관련어만 늘거나 준 나머지 단어는 다시 판정하지 않고 수만 센다
    (v3-merge.json의 changed_not_rejudged, combine에서 2판 도움 단어가 새 응답에서 빠진 수).
  - 조회 결과 파일이 이미 있고 단어·데이터셋이 같으면 다시 조회하지 않는다.
  출력: <저장소>/units-v3.json, lookups-v3-old.jsonl, lookups-v3-new.jsonl, judge-input-v3new.json, v3-merge.json,
       <작업 폴더>/batches/repo-judge-input/<저장소>/vNN.json, <작업 폴더>/batches/briefs/judge-<저장소>-vNN.md
       (판정 결과는 <작업 폴더>/batches/repo-judge/<저장소>/vNN.json)
combine
  - 다시 판정한 단어는 새 판정, 나머지 3판 단어는 2판 판정을 써서 judge-summary-v3.json을 쓴다.
  - 다시 판정한 단어 가운데 2판 판정이 있던 단어의 판정 전후를 v3-compare.json에 쓴다.
"""
import collections
import json
import shutil
import sys
from pathlib import Path
from word_eval_paths import EXPERIMENTS, batches_dir, work_dir

sys.path.insert(0, str(EXPERIMENTS))
from term_canonical import word_key  # noqa: E402

import lookup_words  # noqa: E402
import merge_tag_units  # noqa: E402
import prepare_judge  # noqa: E402
import summarize_judge  # noqa: E402

WORK = work_dir()
BATCHES = batches_dir()
GAP = WORK / "gap"
# gin은 첫 저장소라 판정 지침(judge-brief.md)이 따로였다. 저장소 지침(judge-brief-repo.md)에 넣을 설명을 같은 말로 채운다.
prepare_judge.DESCRIPTIONS.setdefault("gin", "Go 웹 프레임워크")


def read_json(path):
    return json.loads(Path(path).read_text())


def read_jsonl(path):
    return [json.loads(line) for line in open(path)]


def v2_state(repository):
    """2판(없으면 1판)의 단위, 판정 행, 식별자 입력."""
    directory = WORK / repository
    if (directory / "units-v2.json").exists():
        units = read_json(directory / "units-v2.json")
        removed = set(read_json(directory / "tag-merge.json")["removed_words"])
        rows = read_jsonl(directory / merge_tag_units.judge_results_name(repository))
        if (directory / "judge-results-tags.jsonl").exists():
            rows += read_jsonl(directory / "judge-results-tags.jsonl")
        rows = [row for row in rows if row["word"] not in removed]
        inputs, _ = merge_tag_units.inputs_and_judgements(repository)
    else:
        units = read_json(directory / "units.json")
        rows = read_jsonl(directory / "judge-results.jsonl")
        inputs = read_json(directory / "unit-input.json")
    return units, rows, inputs


def touched_word_keys():
    """새 데이터셋에서 뜻이 생기거나 늘어난 단어(페르소나를 새로 받았거나 기본형 뜻을 이어받은 단어)."""
    entries = read_json(GAP / "entries.json")
    return {word_key(p["word"]) for p in entries["personas"]} | {word_key(i["word"]) for i in entries["inherit"]}


def headings(output_text):
    return [line for line in output_text.splitlines() if line.startswith("## ")]


def cached_lookups(path, words, sense_dataset):
    """같은 단어·같은 데이터셋으로 이미 조회한 파일이 있으면 그대로 쓰고, 없으면 조회해서 쓴다."""
    if path.exists():
        rows = read_jsonl(path)
        expected = str(Path(sense_dataset)) if sense_dataset else str(lookup_words.sl.DEFAULT_SENSE_DATASET)
        if [row["word"] for row in rows] == words and all(row["sense_dataset"] == expected for row in rows):
            return rows
    rows = lookup_words.lookup_all(words, sense_dataset)
    with open(path, "w") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")
    return rows


def component_units(phrase_units, split):
    """나눈 문구의 구성 단어 단위. 코드 뜻·식별자는 원래 문구의 것을 쓴다."""
    components = {}
    for unit in phrase_units:
        for raw in split[unit["text"]]:
            text = raw.strip().lower()
            entry = components.setdefault(text, {
                "text": text, "is_included": True, "categorizable": 0, "not_categorizable": 0,
                "fields": collections.Counter(), "senses": [], "expansions": [], "occurrences": 0, "sources": [],
                "is_phrase": " " in text, "from_phrases": []})
            entry["fields"].update(unit["fields"])
            entry["senses"] += [f"part of '{unit['text']}': {sense}" for sense in unit["senses"]]
            entry["expansions"] += unit.get("expansions", [])
            entry["occurrences"] += unit.get("occurrences", 0)
            entry["sources"] = list(dict.fromkeys(entry["sources"] + unit["sources"]))
            entry["from_phrases"].append(unit["text"])
    for entry in components.values():
        entry["fields"] = dict(entry["fields"].most_common())
        entry["senses"] = list(dict.fromkeys(entry["senses"]))[:6]
        entry["expansions"] = list(dict.fromkeys(entry["expansions"]))
    return components


def prepare(new_dataset, repository):
    directory = WORK / repository
    units, _, inputs = v2_state(repository)
    split = {r["text"]: r["components"] for r in read_json(GAP / "phrase-results.json") if r["decision"] == "split"}
    included = {unit["text"]: unit for unit in units if unit["is_included"]}
    removed_phrases = [text for text in included if text in split]
    kept = {text: unit for text, unit in included.items() if text not in split}
    components = {text: unit for text, unit in component_units([included[t] for t in removed_phrases], split).items()
                  if text not in kept}
    v3_units = list(kept.values()) + list(components.values())
    (directory / "units-v3.json").write_text(json.dumps(v3_units, ensure_ascii=False, indent=1))

    words = [unit["text"] for unit in v3_units]
    old = cached_lookups(directory / "lookups-v3-old.jsonl", words, None)
    new = cached_lookups(directory / "lookups-v3-new.jsonl", words, new_dataset)
    changed = {o["word"] for o, n in zip(old, new) if o["output_text"] != n["output_text"]}
    # 뜻 묶음(## 제목)이 바뀐 단어는 2판 판정을 그대로 쓸 수 없어 다시 판정한다.
    heading_changed = {o["word"] for o, n in zip(old, new) if headings(o["output_text"]) != headings(n["output_text"])}
    touched = touched_word_keys()
    targets = [w for w in words if w in components or word_key(w) in touched or w in heading_changed]
    target_set = set(targets)

    items = prepare_judge.build_items({u["text"]: u for u in v3_units}, inputs,
                                      [row for row in new if row["word"] in target_set], "v")
    (directory / "judge-input-v3new.json").write_text(json.dumps(items, ensure_ascii=False, indent=1))
    # 다시 만들 때 이전에 만든(아직 판정하지 않은) vNN 묶음이 남지 않게 한다.
    if any(Path(f"{BATCHES}/repo-judge/{repository}").glob("v*.json")):
        raise SystemExit(f"{repository}: 이미 vNN 판정 결과가 있어 묶음을 다시 만들지 않는다")
    for stale in Path(f"{BATCHES}/repo-judge-input/{repository}").glob("v*.json"):
        stale.unlink()
    for stale in (BATCHES / "briefs").glob(f"judge-{repository}-v*.md"):
        stale.unlink()
    batches = prepare_judge.write_batches(repository, items, "v") if items else []
    merge = {
        "repository": repository, "new_dataset": str(new_dataset), "v2_words": len(included),
        "split_phrases_removed": removed_phrases, "components_added": sorted(components),
        "v3_words": len(words), "output_changed": len(changed), "rejudged": len(targets),
        "rejudged_new_components": sum(1 for w in targets if w in components),
        "rejudged_touched": sum(1 for w in targets if w not in components and word_key(w) in touched),
        "rejudged_heading_changed_only": sum(1 for w in targets if w not in components and word_key(w) not in touched),
        "rejudged_output_unchanged": sorted(w for w in targets if w not in changed and w not in components),
        "changed_not_rejudged": len(changed - target_set), "batches": batches,
    }
    (directory / "v3-merge.json").write_text(json.dumps(merge, ensure_ascii=False, indent=1))
    print(repository, f"2판 {len(included)} → 3판 {len(words)}", f"나눈 문구 -{len(removed_phrases)}",
          f"구성 단어 +{len(components)}", f"응답 바뀜 {len(changed)}", f"다시 판정 {len(targets)}",
          f"바뀌었지만 판정 안 함 {merge['changed_not_rejudged']}", "묶음", batches)


def combine(repository):
    directory = WORK / repository
    merge = read_json(directory / "v3-merge.json")
    # summarize_judge.main은 판정 디렉터리의 *.json을 모두 읽으므로 vNN만 따로 모은다.
    judged_directory = Path(f"{BATCHES}/repo-judge-v3/{repository}")
    judged_directory.mkdir(parents=True, exist_ok=True)
    for path in sorted(Path(f"{BATCHES}/repo-judge/{repository}").glob("v*.json")):
        shutil.copy(path, judged_directory / path.name)
    summarize_judge.main(directory, judged_directory, "v3new", "units-v3.json")
    new_rows = read_jsonl(directory / "judge-results-v3new.jsonl")
    new_items = {item["id"]: item for item in read_json(directory / "judge-input-v3new.json")}

    _, v2_rows, _ = v2_state(repository)
    v3_words = {unit["text"] for unit in read_json(directory / "units-v3.json")}
    rejudged = {row["word"] for row in new_rows} | {item["word"] for item in new_items.values()}
    kept_rows = [row for row in v2_rows if row["word"] in v3_words and row["word"] not in rejudged]
    rows = kept_rows + new_rows
    items = {row["id"]: row for row in kept_rows} | new_items
    summary = summarize_judge.build_summary(rows, items, {row["id"]: row for row in rows})
    summary["v3"] = {k: (len(v) if isinstance(v, list) else v) for k, v in merge.items() if k != "new_dataset"}
    summary["v3"]["kept_v2_judgements"] = len(kept_rows)
    # 2판 판정을 그대로 쓴 단어 가운데 응답이 바뀐 단어: 2판에서 도움 된다고 한 관련어가 새 응답에서 빠졌는지 센다.
    new_outputs = {row["word"]: row["output_text"] for row in read_jsonl(directory / "lookups-v3-new.jsonl")}
    lost = {row["word"]: [w for w in row["helpful_words"] if w not in set(summarize_judge.listed_words(new_outputs[row["word"]]))]
            for row in kept_rows}
    summary["v3"]["kept_words_with_helpful_lost"] = sum(1 for words in lost.values() if words)
    summary["v3"]["kept_helpful_words_lost"] = sum(len(words) for words in lost.values())
    (directory / "judge-summary-v3.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))

    before = {row["word"]: row for row in v2_rows}
    pairs = [(before[row["word"]], row) for row in new_rows if row["word"] in before]
    compare = {
        "repository": repository, "rejudged_with_v2": len(pairs),
        "transitions": dict(collections.Counter(f"{b['sense_result']}->{a['sense_result']}" for b, a in pairs).most_common()),
        "helpful_count_before": summarize_judge.describe([b["helpful_count"] for b, _ in pairs]),
        "helpful_count_after": summarize_judge.describe([a["helpful_count"] for _, a in pairs]),
        "returned_count_before": summarize_judge.describe([b["returned_count"] for b, _ in pairs]),
        "returned_count_after": summarize_judge.describe([a["returned_count"] for _, a in pairs]),
        "words": [{"word": a["word"], "before": b["sense_result"], "after": a["sense_result"],
                   "helpful_before": b["helpful_count"], "helpful_after": a["helpful_count"]} for b, a in pairs],
    }
    (directory / "v3-compare.json").write_text(json.dumps(compare, ensure_ascii=False, indent=1))
    print(repository, "3판 판정", summary["judged"], "/", len(v3_words), summary["all"]["sense_result"],
          "다시 판정 전후", compare["transitions"])


if __name__ == "__main__":
    command, *arguments = sys.argv[1:]
    if command == "prepare":
        dataset, *repositories = arguments
        for name in repositories:
            prepare(dataset, name)
    else:
        for name in arguments:
            combine(name)
