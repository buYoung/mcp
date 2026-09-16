#!/usr/bin/env python3
"""Compare saved analysis with independently source-reviewed endpoint labels."""

import argparse
import gzip
import json
from pathlib import Path
from model import CALL_RELATIONS, DATA_RELATIONS


def load_result(directory, name):
    plain = directory / (name + ".json")
    compressed = directory / (name + ".json.gz")
    return json.loads(plain.read_bytes() if plain.exists() else gzip.decompress(compressed.read_bytes()))


def endpoint_matches(endpoint, location, files):
    return location["path"] == files[endpoint[0]] and endpoint[1] <= location["line"] <= endpoint[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args()
    labels = json.loads((Path(__file__).parent / "cases.json").read_text())
    files = labels["files"]
    results = {name: load_result(options.results, name) for name in {case["repository"] for case in labels["cases"]}}
    rows = []
    for case in labels["cases"]:
        source_result = results[case["repository"]]
        loaded = source_result["scope"]["source_sha256"]
        is_input_complete = all(files[case[key][0]] in loaded for key in ("storage", "invocation"))
        matches = [relation for relation in source_result["relations"]
                   if endpoint_matches(case["storage"], relation["storage"]["location"], files)
                   and endpoint_matches(case["invocation"], relation["invocation"]["location"], files)]
        invocation_matches = [relation for relation in matches if relation["kind"] in CALL_RELATIONS]
        data_matches = [relation for relation in matches if relation["kind"] in DATA_RELATIONS]
        argument_matches = [relation for relation in matches if relation["kind"] == "stored_value_argument"]
        if not is_input_complete:
            status = "input_unavailable"
        elif case["kind"] == "negative":
            status = "incorrect_connection" if matches else "correctly_unjoined"
        else:
            status = "conditional_candidate" if invocation_matches else ("conditional_data_return" if data_matches else
                     ("argument_transfer_only" if argument_matches else "unresolved"))
        rows.append({**case, "status": status, "matching_relations": len(matches),
                     "evidence": [{"storage": relation["storage"]["location"],
                                   "invocation": relation["invocation"]["location"],
                                   "conditions": relation["conditions"], "kind": relation["kind"]}
                                  for relation in matches[:3]]})
    summary = {}
    for name in sorted(results):
        summary[name] = {}
        for phase in ("development", "evaluation"):
            selected = [row for row in rows if row["repository"] == name and row["phase"] == phase and row["kind"] != "negative"]
            summary[name][phase] = {"reference_pairs": len(selected),
                                    "conditional_candidates": sum(row["status"] == "conditional_candidate" for row in selected),
                                    "argument_transfer_only": sum(row["status"] == "argument_transfer_only" for row in selected),
                                    "conditional_data_return": sum(row["status"] == "conditional_data_return" for row in selected),
                                    "unresolved": sum(row["status"] == "unresolved" for row in selected),
                                    "input_unavailable": sum(row["status"] == "input_unavailable" for row in selected)}
        negatives = [row for row in rows if row["repository"] == name and row["kind"] == "negative"]
        summary[name]["negative_controls"] = {"pairs": len(negatives), "incorrect_connections": sum(row["status"] == "incorrect_connection" for row in negatives)}
    language_summary = {}
    for row in rows:
        extension = Path(files[row["storage"][0]]).suffix
        language = {".ts": "TypeScript", ".tsx": "TypeScript", ".js": "JavaScript", ".go": "Go", ".rs": "Rust"}[extension]
        row["language"] = language
        counts = language_summary.setdefault(language, {})
        counts[row["status"]] = counts.get(row["status"], 0) + 1
    output = {"unit": "source-reviewed endpoint pair", "scope": "selected examples; not repository-wide recall or precision",
              "runtime_delivery_verified": False, "summary": summary, "cases": rows}
    output["languages"] = language_summary
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
