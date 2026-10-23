"""gin 1회차와 2회차 판정을 비교하고, 같은 입력을 다시 판정한 흔들림 기준선과 나란히 놓는다.

사용법: python3 compare_rounds.py
입력: gin/judge-results.jsonl(1회차), gin/judge-results-r2.jsonl(2회차), gin/judge-results-noise.jsonl(1회차 입력 재판정),
      gin/judge-input.json, gin/judge-input-r2.json
출력: gin/round-compare.json
- 응답이 바뀐 단어와 그대로인 단어를 나눠 판정 이동(올라감/내려감)과 도움 단어 수 변화를 센다.
- 기준선: 같은 1회차 입력을 다시 판정했을 때 판정이 바뀐 비율과 도움 단어 수 변화.
"""
import collections
import json
import statistics
from word_eval_paths import work_dir

GIN = work_dir() / "gin"
ORDER = {"no_result": 0, "none": 1, "partial": 2, "match": 3}


def load(name):
    return {row["id"]: row for row in map(json.loads, open(GIN / name))}


def moves(pairs):
    counter = collections.Counter((before["sense_result"], after["sense_result"]) for before, after in pairs)
    up = sum(n for (a, b), n in counter.items() if ORDER[b] > ORDER[a])
    down = sum(n for (a, b), n in counter.items() if ORDER[b] < ORDER[a])
    deltas = [after["helpful_count"] - before["helpful_count"] for before, after in pairs]
    return {
        "words": len(pairs), "up": up, "down": down, "same": len(pairs) - up - down,
        "flip_rate": round((up + down) / len(pairs), 3) if pairs else None,
        "transitions": {f"{a}->{b}": n for (a, b), n in counter.most_common() if a != b},
        "before": dict(collections.Counter(before["sense_result"] for before, _ in pairs)),
        "after": dict(collections.Counter(after["sense_result"] for _, after in pairs)),
        "helpful_before": statistics.median([before["helpful_count"] for before, _ in pairs]) if pairs else None,
        "helpful_after": statistics.median([after["helpful_count"] for _, after in pairs]) if pairs else None,
        "helpful_delta_median": statistics.median(deltas) if deltas else None,
        "helpful_delta_mean": round(statistics.mean(deltas), 1) if deltas else None,
        "returned_before": statistics.median([before["returned_count"] for before, _ in pairs]) if pairs else None,
        "returned_after": statistics.median([after["returned_count"] for _, after in pairs]) if pairs else None,
    }


def main():
    first, second, noise = load("judge-results.jsonl"), load("judge-results-r2.jsonl"), load("judge-results-noise.jsonl")
    first_input = {item["id"]: item for item in json.loads((GIN / "judge-input.json").read_text())}
    second_input = {item["id"]: item for item in json.loads((GIN / "judge-input-r2.json").read_text())}
    changed = [i for i in second if first_input[i]["dictionary_output"] != second_input[i]["dictionary_output"]]
    unchanged = [i for i in second if i not in set(changed)]
    noise_ids = sorted(noise)
    report = {
        "first_round": moves([(first[i], first[i]) for i in first])["before"],
        "second_round": moves([(second[i], second[i]) for i in second])["before"],
        "output_changed": moves([(first[i], second[i]) for i in changed]),
        "output_unchanged_reused": moves([(first[i], second[i]) for i in unchanged]),
        # 기준선 단어(응답이 바뀐 단어 중 표본)만 따로: 같은 단어에서 응답 변경 효과와 흔들림을 나란히 본다.
        "noise_same_input": moves([(first[i], noise[i]) for i in noise_ids]),
        "noise_sample_with_new_output": moves([(first[i], second[i]) for i in noise_ids]),
        "noise_vs_new_output": moves([(noise[i], second[i]) for i in noise_ids]),
    }
    groups = collections.defaultdict(list)
    for i in changed:
        groups[first[i]["group"]].append((first[i], second[i]))
    report["output_changed_by_group"] = {group: moves(pairs) for group, pairs in groups.items()}
    report["flips_same_input"] = [
        {"word": first[i]["word"], "first": first[i]["sense_result"], "again": noise[i]["sense_result"],
         "first_headings": first[i]["matched_headings"], "again_headings": noise[i]["matched_headings"]}
        for i in noise_ids if first[i]["sense_result"] != noise[i]["sense_result"]]
    report["moves_new_output"] = [
        {"word": first[i]["word"], "first": first[i]["sense_result"], "second": second[i]["sense_result"],
         "first_headings": first[i]["matched_headings"], "second_headings": second[i]["matched_headings"],
         "second_note": second[i]["note"]}
        for i in changed if first[i]["sense_result"] != second[i]["sense_result"]]
    (GIN / "round-compare.json").write_text(json.dumps(report, ensure_ascii=False, indent=1))
    for key in ("first_round", "second_round"):
        print(key, report[key])
    for key in ("output_changed", "output_unchanged_reused", "noise_same_input", "noise_sample_with_new_output", "noise_vs_new_output"):
        print(key, {k: v for k, v in report[key].items() if k not in ("before", "after")})


if __name__ == "__main__":
    main()
