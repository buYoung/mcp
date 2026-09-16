#!/usr/bin/env python3
"""Evaluate supplemental consumer endpoints without changing the original cases."""

import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path

DIRECTORY = Path(__file__).resolve().parent


def evaluate_consumers(cases, analyses):
    rows = []
    for case in cases:
        analysis = analyses[case["input_variant"]]
        for path, digest in case["source_sha256"].items():
            assert analysis["scope"]["source_sha256"][path] == digest, case["id"]
        matches = [relation for relation in analysis["relations"]
                   if all(relation[key]["location"]["path"] == case[key][0]
                          and case[key][1] <= relation[key]["location"]["line"] <= case[key][2]
                          for key in ("storage", "invocation"))]
        selected = [relation for relation in matches if relation["kind"] in case["relation_kinds"]
                    and set(case.get("required_conditions", ())).issubset(relation["conditions"])]
        if case["kind"] == "negative":
            status = "incorrect_connection" if matches else "negative_pending_positive_control"
        elif selected:
            status = "conditional_candidate" if selected[0]["kind"] == "storage_to_invocation" else "conditional_data_consumption"
        else:
            status = "unresolved"
        rows.append({**case, "status": status, "matching_relations": len(matches), "selected_relations": len(selected),
                     "evidence": selected[:3]})
    by_id = {row["id"]: row for row in rows}
    for row in rows:
        if row["status"] == "negative_pending_positive_control":
            control = by_id[row["positive_control"]]
            row["status"] = ("correctly_unjoined_with_positive_control" if control["selected_relations"]
                             else "negative_unverified_without_positive_control")
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", type=Path, default=DIRECTORY / "results")
    args = parser.parse_args()
    manifest = json.loads((DIRECTORY / "consumers.json").read_text())
    paths = {case["input_variant"]: args.results / (case["input_variant"] + ".json.gz") for case in manifest["cases"]}
    analyses = {name: json.loads(gzip.decompress(path.read_bytes())) for name, path in paths.items()}
    rows = evaluate_consumers(manifest["cases"], analyses)
    result = {"unit": manifest["unit"], "cases_lock_sha256": hashlib.sha256((DIRECTORY / "consumers.json").read_bytes()).hexdigest(),
              "analysis_sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in paths.items()},
              "target_programs_executed": False, "statuses": dict(Counter(row["status"] for row in rows)), "cases": rows}
    (args.results / "consumers.evaluation.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"statuses": result["statuses"], "cases": [{"id": row["id"], "status": row["status"]} for row in rows]}, ensure_ascii=False))
    return int(any(row["status"] in {"incorrect_connection", "negative_unverified_without_positive_control"} for row in rows))


if __name__ == "__main__":
    raise SystemExit(main())
