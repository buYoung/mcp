"""식별자별 개념 단위 판정(<작업 폴더>/batches/units/u*.json)을 단위별로 모으고, 관련어 데이터셋의 현재 상태와 맞춘다.

사용법: python3 aggregate_units.py <수집 결과 디렉터리> <판정 결과 디렉터리> <데이터셋 sqlite>
출력: <수집 결과 디렉터리>/units.json, units-summary.json
데이터셋 상태: head(자기 뜻 있음), input(관련어로 나온 뜻으로 분류됨), relation_only(관련어로만 있고 분류 없음), missing(없음)
분야 대조는 데이터셋 뜻의 분야에 판정 분야가 있는지만 보는 기계 대조다.
"""
import collections
import json
import re
import sqlite3
import sys
from pathlib import Path
from word_eval_paths import EXPERIMENTS

sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402
from term_canonical import word_key  # noqa: E402


def compact(text):
    return re.sub(r"[^a-z0-9]", "", text.lower())


def load_dataset(path):
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    head_fields, input_fields, persona_fields = collections.defaultdict(set), collections.defaultdict(set), {}
    for persona, key, field, *_ in sl.persona_rows(connection):
        head_fields[key].add(field)
        persona_fields[persona] = field
    for key, persona in connection.execute("SELECT word_key, persona FROM input_senses"):
        input_fields[key].add(persona_fields[persona])
    related = {key for (key,) in connection.execute("SELECT DISTINCT word_key FROM relations")}
    connection.close()
    return head_fields, input_fields, related


def main(collected_directory, judged_directory, dataset_path):
    collected_directory = Path(collected_directory)
    inputs = json.loads((collected_directory / "unit-input.json").read_text())
    judged = [record for path in sorted(Path(judged_directory).glob("u*.json")) for record in json.loads(path.read_text())]
    rows, summary = aggregate(inputs, judged, dataset_path)
    (collected_directory / "units.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1))
    (collected_directory / "units-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))
    print(json.dumps(summary, ensure_ascii=False, indent=1))


def aggregate(inputs, judged, dataset_path):
    """판정 입력 항목과 단위 판정을 단위별로 모아 (units 행 목록, 요약)을 돌려준다."""
    judged_by_name = collections.defaultdict(list)
    for record in judged:
        judged_by_name[(record["name"], record.get("where"))].append(record)

    problems = collections.Counter()
    units = {}
    for item in inputs:
        records = judged_by_name.get((item["name"], item["where"]))
        if not records:
            problems["missing_judgement"] += 1
            continue
        record = records[0]
        if compact("".join(u["text"] for u in record["units"])) != compact(item["name"]):
            problems["units_do_not_cover_name"] += 1
        for unit in record["units"]:
            text = " ".join(unit["text"].lower().split())
            entry = units.setdefault(text, {"text": text, "categorizable": 0, "not_categorizable": 0,
                                            "fields": collections.Counter(), "senses": [], "expansions": set(),
                                            "sources": [], "occurrences": 0})
            entry["categorizable" if unit.get("categorizable") else "not_categorizable"] += 1
            entry["occurrences"] += item["count"]
            if unit.get("categorizable"):
                entry["fields"][unit.get("field")] += 1
                if unit.get("sense") and len(entry["senses"]) < 4 and unit["sense"] not in entry["senses"]:
                    entry["senses"].append(unit["sense"])
            if unit.get("expansion"):
                entry["expansions"].add(unit["expansion"])
            if len(entry["sources"]) < 6:
                entry["sources"].append(item["name"])

    head_fields, input_fields, related = load_dataset(dataset_path)
    rows = []
    for entry in units.values():
        key = word_key(entry["text"])
        status = ("head" if key in head_fields else "input" if key in input_fields
                  else "relation_only" if key in related else "missing")
        dataset_fields = head_fields.get(key) or input_fields.get(key) or set()
        judged_fields = set(entry["fields"]) - {None}
        rows.append({
            "text": entry["text"], "is_included": entry["categorizable"] > 0,
            "categorizable": entry["categorizable"], "not_categorizable": entry["not_categorizable"],
            "fields": dict(entry["fields"].most_common()), "senses": entry["senses"],
            "expansions": sorted(entry["expansions"]), "occurrences": entry["occurrences"], "sources": entry["sources"],
            "is_phrase": " " in entry["text"], "dataset_status": status, "dataset_fields": sorted(dataset_fields),
            "missing_fields": sorted(judged_fields - dataset_fields) if status in ("head", "input") else sorted(judged_fields),
        })
    rows.sort(key=lambda r: (-r["occurrences"], r["text"]))

    included = [r for r in rows if r["is_included"]]
    excluded = [r for r in rows if not r["is_included"]]
    summary = {
        "input_count": len(inputs), "judged_count": len(judged), "problems": dict(problems),
        "unit_count": len(rows), "included_count": len(included), "excluded_count": len(excluded),
        "included_by_status": dict(collections.Counter(r["dataset_status"] for r in included)),
        "included_phrases_by_status": dict(collections.Counter(r["dataset_status"] for r in included if r["is_phrase"])),
        "excluded_by_status": dict(collections.Counter(r["dataset_status"] for r in excluded)),
        "included_present_without_judged_field": sum(1 for r in included if r["dataset_status"] in ("head", "input")
                                                     and r["missing_fields"] and len(r["missing_fields"]) == len(r["fields"])),
        "mixed_judgement_count": sum(1 for r in rows if r["categorizable"] and r["not_categorizable"]),
    }
    return rows, summary


if __name__ == "__main__":
    main(*sys.argv[1:4])
