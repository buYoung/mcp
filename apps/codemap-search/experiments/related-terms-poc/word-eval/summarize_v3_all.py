"""3판(v3) 다시 판정 결과를 저장소 12개에 걸쳐 모은다(판정 없음, 수만 센다). prepare_v3.py combine 뒤에 돌린다.

사용법: python3 summarize_v3_all.py <저장소 ...>
비교 기준: 2판 요약(judge-summary-v2.json)이 있으면 2판, 없으면 1판(judge-summary.json). prepare_v3.v2_state와 같은 기준이다.
세는 것
  - 저장소별 판정 분포 전후(before: 2판/1판 전체 단어, after: 3판 전체 단어. 단어 집합이 다르다)
  - 다시 판정한 단어 가운데 이전 판정이 있던 단어의 판정 전후(v3-compare.json의 transitions)
  - 나눈 문구(평가셋에서 빠짐)의 이전 판정 분포와 새 구성 단어의 3판 판정 분포
  - 이전 판정을 그대로 쓴 단어 가운데 이전에 도움 된다고 한 관련어가 새 응답에서 빠진 단어 수
  - 기존 데이터셋 응답에는 관련어가 있었는데 새 데이터셋 응답에서 0개가 된 단어(그 반대도)
출력: v3-all-summary.json
"""
import collections
import json
import sys
from word_eval_paths import work_dir

import prepare_v3
import summarize_judge

WORK = work_dir()
RESULTS = ["match", "partial", "none", "no_result"]


def related_count(output_text):
    return len(summarize_judge.listed_words(output_text))


def before_summary(directory):
    path = directory / "judge-summary-v2.json"
    if not path.exists():
        path = directory / "judge-summary.json"
    return path, json.loads(path.read_text())


def repository_summary(repository):
    directory = WORK / repository
    before_path, before = before_summary(directory)
    after = json.loads((directory / "judge-summary-v3.json").read_text())
    compare = json.loads((directory / "v3-compare.json").read_text())
    merge = json.loads((directory / "v3-merge.json").read_text())
    _, v2_rows, _ = prepare_v3.v2_state(repository)
    v2_by_word = {row["word"]: row for row in v2_rows}
    new_rows = {row["word"]: row for row in prepare_v3.read_jsonl(directory / "judge-results-v3new.jsonl")}

    removed = merge["split_phrases_removed"]
    components = merge["components_added"]
    # 두 판 모두에 있는 단어만: 이전 판정 대 3판 판정(다시 판정했으면 새 판정, 아니면 이전 판정 그대로)
    v3_words = [unit["text"] for unit in prepare_v3.read_json(directory / "units-v3.json")]
    common = [w for w in v3_words if w in v2_by_word]
    common_before = collections.Counter(v2_by_word[w]["sense_result"] for w in common)
    common_after = collections.Counter((new_rows.get(w) or v2_by_word[w])["sense_result"] for w in common)
    # 응답 원문이 그대로인데 다시 판정한 단어: 판정 전후 차이는 판정 흔들림으로 본다
    unchanged = [w for w in merge["rejudged_output_unchanged"] if w in v2_by_word and w in new_rows]
    unchanged_transitions = collections.Counter(
        f"{v2_by_word[w]['sense_result']}->{new_rows[w]['sense_result']}" for w in unchanged)
    old = prepare_v3.read_jsonl(directory / "lookups-v3-old.jsonl")
    new = prepare_v3.read_jsonl(directory / "lookups-v3-new.jsonl")
    dropped_to_zero, rose_from_zero = [], []
    for o, n in zip(old, new):
        old_count, new_count = related_count(o["output_text"]), related_count(n["output_text"])
        if old_count and not new_count:
            judged = new_rows.get(o["word"]) or v2_by_word.get(o["word"])
            dropped_to_zero.append({"word": o["word"], "old_related": old_count,
                                    "v3_sense_result": judged["sense_result"] if judged else None,
                                    "rejudged": o["word"] in new_rows})
        elif new_count and not old_count:
            rose_from_zero.append(o["word"])

    return {
        "repository": repository, "before_file": before_path.name,
        "before": before["all"], "after": after["all"],
        "v3": after["v3"],
        "transitions": compare["transitions"], "rejudged_with_previous": compare["rejudged_with_v2"],
        "common_words": len(common), "common_before": dict(common_before), "common_after": dict(common_after),
        "output_unchanged_rejudged": len(unchanged),
        "output_unchanged_transitions": dict(unchanged_transitions.most_common()),
        "split_phrases_before": dict(collections.Counter(
            v2_by_word[t]["sense_result"] for t in removed if t in v2_by_word)),
        "components_after": dict(collections.Counter(
            new_rows[t]["sense_result"] for t in components if t in new_rows)),
        "components_not_judged": [t for t in components if t not in new_rows],
        "dropped_to_zero": dropped_to_zero, "rose_from_zero": len(rose_from_zero),
    }


def add_counts(total, counts):
    for key, value in counts.items():
        total[key] += value


def main(repositories):
    rows = [repository_summary(name) for name in repositories]
    totals = {name: collections.Counter() for name in
              ["before", "after", "transitions", "split_phrases_before", "components_after",
               "common_before", "common_after", "output_unchanged_transitions"]}
    for row in rows:
        add_counts(totals["before"], row["before"]["sense_result"])
        add_counts(totals["after"], row["after"]["sense_result"])
        add_counts(totals["transitions"], row["transitions"])
        add_counts(totals["common_before"], row["common_before"])
        add_counts(totals["common_after"], row["common_after"])
        add_counts(totals["output_unchanged_transitions"], row["output_unchanged_transitions"])
        add_counts(totals["split_phrases_before"], row["split_phrases_before"])
        add_counts(totals["components_after"], row["components_after"])
    v3_keys = ["v2_words", "v3_words", "split_phrases_removed", "components_added", "output_changed", "rejudged",
               "rejudged_new_components", "rejudged_touched", "rejudged_heading_changed_only", "changed_not_rejudged",
               "kept_v2_judgements", "kept_words_with_helpful_lost", "kept_helpful_words_lost"]
    summary = {
        "repositories": rows,
        "total": {
            "before": dict(totals["before"]), "after": dict(totals["after"]),
            "before_words": sum(totals["before"].values()), "after_words": sum(totals["after"].values()),
            "v3": {key: sum(row["v3"].get(key, 0) for row in rows) for key in v3_keys},
            "transitions": dict(totals["transitions"].most_common()),
            "rejudged_with_previous": sum(row["rejudged_with_previous"] for row in rows),
            "common_words": sum(row["common_words"] for row in rows),
            "common_before": dict(totals["common_before"]), "common_after": dict(totals["common_after"]),
            "output_unchanged_rejudged": sum(row["output_unchanged_rejudged"] for row in rows),
            "output_unchanged_transitions": dict(totals["output_unchanged_transitions"].most_common()),
            "components_not_judged": sum(len(row["components_not_judged"]) for row in rows),
            "dropped_to_zero_words": [dict(item, repository=row["repository"])
                                      for row in rows for item in row["dropped_to_zero"]],
            "split_phrases_before": dict(totals["split_phrases_before"]),
            "components_after": dict(totals["components_after"]),
            "dropped_to_zero": sum(len(row["dropped_to_zero"]) for row in rows),
            "rose_from_zero": sum(row["rose_from_zero"] for row in rows),
        },
    }
    (WORK / "v3-all-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))

    def rates(counts):
        words = sum(counts.values())
        return (f"{words}단어 " + " ".join(f"{k} {counts.get(k, 0)}" for k in RESULTS)
                + f" | match {counts.get('match', 0) / words:.3f} match+partial "
                f"{(counts.get('match', 0) + counts.get('partial', 0)) / words:.3f}")
    for row in rows:
        print(row["repository"], f"[{row['before_file']}]")
        print("  전:", rates(row["before"]["sense_result"]))
        print("  후:", rates(row["after"]["sense_result"]))
        print("  다시 판정 전후:", row["transitions"])
        print("  0개로 줄어듦:", len(row["dropped_to_zero"]), "0개에서 생김:", row["rose_from_zero"],
              "도움어 빠진 유지 단어:", row["v3"]["kept_words_with_helpful_lost"])
    print("합계 전:", rates(totals["before"]))
    print("합계 후:", rates(totals["after"]))
    print("합계 다시 판정 전후:", summary["total"]["transitions"])
    print("공통 단어 전:", rates(totals["common_before"]))
    print("공통 단어 후:", rates(totals["common_after"]))
    print("응답 그대로인데 다시 판정:", summary["total"]["output_unchanged_rejudged"],
          summary["total"]["output_unchanged_transitions"])
    print("판정 안 된 구성 단어:", summary["total"]["components_not_judged"])
    print("0개로 줄어든 단어:", summary["total"]["dropped_to_zero_words"])
    print("나눈 문구 이전 판정:", summary["total"]["split_phrases_before"], "구성 단어 3판:", summary["total"]["components_after"])
    print("0개로 줄어듦:", summary["total"]["dropped_to_zero"], "0개에서 생김:", summary["total"]["rose_from_zero"])
    print("3판 수:", summary["total"]["v3"])


if __name__ == "__main__":
    main(sys.argv[1:])
