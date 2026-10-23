"""3단계: luna 판정(판정 결과 디렉터리의 *.json)을 모아 지표를 낸다.

사용법: python3 summarize_judge.py <수집 결과 디렉터리> <판정 결과 디렉터리> [회차 이름] [단위 파일 이름, 기본 units.json]
회차 이름(예: r2)을 주면 judge-input-<회차>.json을 읽고 judge-results-<회차>.jsonl, judge-summary-<회차>.json에 쓴다.
출력: judge-results.jsonl(단어별 판정 + 기계 점검), judge-summary.json
기계 점검: 판정의 helpful_words가 응답 원문의 관련어 목록에 실제로 있는지(없는 것은 helpful_count에서 뺀다),
no_result 판정과 관련어 수가 맞는지
"""
import collections
import json
import re
import statistics
import sys
from pathlib import Path

COUNT_HEADER = re.compile(r"^# 관련어 (\d+)개")


def listed_words(output_text):
    """응답 원문에 나온 관련어 전체(쉼표로 나뉜 항목)."""
    words = set()
    for line in output_text.splitlines():
        if line.startswith("## 뜻 미배정 직접 관계어"):
            words.update(w.strip() for w in line.split(": ", 1)[1].split(", "))
        elif line.startswith("  ") and ": " in line:
            words.update(w.strip() for w in line.split(": ", 1)[1].split(", "))
    return words


def describe(values):
    return {"mean": round(statistics.mean(values), 1), "median": statistics.median(values)} if values else None


def main(collected_directory, judged_directory, round_name=None, units_name="units.json"):
    collected_directory = Path(collected_directory)
    suffix = f"-{round_name}" if round_name else ""
    items = {item["id"]: item for item in json.loads((collected_directory / f"judge-input{suffix}.json").read_text())}
    judged = {record["id"]: record for path in sorted(Path(judged_directory).glob("*.json"))
              for record in json.loads(path.read_text())}
    units = {unit["text"]: unit for unit in json.loads((collected_directory / units_name).read_text())}

    rows = []
    for item_id, item in items.items():
        record = judged.get(item_id)
        if record is None:
            continue
        output = item["dictionary_output"]
        returned = int(COUNT_HEADER.match(output).group(1))
        listed = listed_words(output)
        judged_helpful = list(dict.fromkeys(record.get("helpful_words") or []))
        # 응답의 관련어 목록에 없는 단어(입력 단어 자체, 판정자가 지어낸 단어)는 세지 않는다.
        helpful = [w for w in judged_helpful if w in listed]
        rows.append({
            "id": item_id, "word": item["word"], "group": item.get("_flag") or "general",
            "is_phrase": " " in item["word"], "code_fields": (item.get("code_meaning") or item["gin_meaning"])["fields"],
            "occurrences": units[item["word"]]["occurrences"],
            "sense_result": record["sense_result"], "matched_headings": record.get("matched_headings") or [],
            "returned_count": returned, "helpful_count": len(helpful),
            "helpful_not_in_output": [w for w in judged_helpful if w not in listed],
            "is_no_result_mismatch": (record["sense_result"] == "no_result") != (returned == 0),
            "helpful_words": helpful, "note": record.get("note"),
        })
    with open(collected_directory / f"judge-results{suffix}.jsonl", "w") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")
    summary = build_summary(rows, items, judged)
    (collected_directory / f"judge-summary{suffix}.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))
    print(json.dumps({k: summary[k] for k in ("judged", "expected", "checks", "all", "general", "single_words", "phrases")},
                     ensure_ascii=False, indent=1))


def build_summary(rows, items, judged):
    """단어별 판정 행에서 지표를 낸다. items·judged는 id를 키로 하는 판정 입력·판정 결과."""
    def section(selected):
        results = collections.Counter(r["sense_result"] for r in selected)
        answered = [r for r in selected if r["returned_count"]]
        return {
            "words": len(selected),
            "sense_result": {k: results[k] for k in ("match", "partial", "none", "no_result")},
            "match_rate": round(results["match"] / len(selected), 3) if selected else None,
            "match_or_partial_rate": round((results["match"] + results["partial"]) / len(selected), 3) if selected else None,
            "helpful_count": describe([r["helpful_count"] for r in selected]),
            "returned_count": describe([r["returned_count"] for r in selected]),
            "helpful_ratio_when_answered": describe([r["helpful_count"] / r["returned_count"] for r in answered]),
        }

    by_field = collections.defaultdict(list)
    for row in rows:
        by_field[row["code_fields"][0]].append(row)
    summary = {
        "judged": len(rows), "expected": len(items), "missing_ids": sorted(set(items) - set(judged))[:20],
        "checks": {
            "helpful_words_not_in_output": sum(len(r["helpful_not_in_output"]) for r in rows),
            "helpful_words_total": sum(r["helpful_count"] for r in rows),
            "no_result_mismatch": sum(r["is_no_result_mismatch"] for r in rows),
        },
        "all": section(rows),
        "general": section([r for r in rows if r["group"] == "general"]),
        "local_abbreviation": section([r for r in rows if r["group"] == "local_abbreviation"]),
        "fixture_or_error": section([r for r in rows if r["group"] == "fixture_or_error"]),
        "single_words": section([r for r in rows if not r["is_phrase"] and r["group"] == "general"]),
        "phrases": section([r for r in rows if r["is_phrase"] and r["group"] == "general"]),
        "by_code_field": {field: section(selected) for field, selected in sorted(by_field.items(), key=lambda x: -len(x[1]))},
    }
    return summary


if __name__ == "__main__":
    main(*sys.argv[1:5])
