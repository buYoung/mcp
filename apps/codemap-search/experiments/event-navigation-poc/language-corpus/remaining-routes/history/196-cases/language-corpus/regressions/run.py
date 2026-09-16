"""Check exact storage/call pairs without executing any target program."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys


DIRECTORY = Path(__file__).resolve().parent
IMPLEMENTATION = DIRECTORY.parent
sys.path.insert(0, str(IMPLEMENTATION.parent))
from model import CALL_RELATIONS, DATA_RELATIONS, CONSUMPTION_RELATIONS


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("languages", nargs="*")
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--output", type=Path, default=DIRECTORY / "results")
    args = parser.parse_args()
    cases = json.loads((DIRECTORY / "cases.json").read_text())
    selected = [case for case in cases
                if (not args.languages or case["language"] in args.languages)
                and (not args.case or case["id"] in args.case)]
    if not selected:
        parser.error("no cases selected")
    args.output.mkdir(parents=True, exist_ok=True)
    analyses = {}
    rows = []
    for case in selected:
        path = DIRECTORY / "examples" / case["path"]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != case["source_sha256"]:
            raise RuntimeError(f"source changed without updating the case: {case['path']}")
        for relative, expected in case.get("additional_sources", {}).items():
            target = path.parent / relative
            if not target.resolve().is_relative_to(path.parent.resolve()) or hashlib.sha256(target.read_bytes()).hexdigest() != expected:
                raise RuntimeError(f"additional source changed or escaped the case: {relative}")
        for relative, support in case.get("support_sources", {}).items():
            target = path.parent / relative
            if not target.resolve().is_relative_to(path.parent.resolve()) or hashlib.sha256(target.read_bytes()).hexdigest() != support["sha256"]:
                raise RuntimeError(f"support source changed or escaped the case: {relative}")
        if case["path"] not in analyses:
            output = args.output / "analysis" / (case["path"].replace("/", "--") + ".json.gz")
            output.parent.mkdir(parents=True, exist_ok=True)
            command = [sys.executable, "-X", "faulthandler", str(IMPLEMENTATION / "analyze_languages.py"),
                       "--root", str(path.parent), "--path", path.name,
                       "--language", case["language"], "--output", str(output)]
            for relative in case.get("additional_sources", {}):
                command.extend(["--path", relative])
            for relative, support in case.get("support_sources", {}).items():
                command.extend(["--support-path", support["language"] + ":" + relative])
            process = subprocess.run(command, capture_output=True, text=True)
            if process.returncode:
                raise RuntimeError(f"{case['id']}: analyzer failed: {process.stderr}")
            analyses[case["path"]] = json.loads(gzip.decompress(output.read_bytes()))
        result = analyses[case["path"]]
        matches = [relation for relation in result["relations"]
                   if relation["storage"]["location"]["path"] == case.get("storage_file", path.name)
                   and relation["storage"]["location"]["line"] == case["storage_line"]
                   and relation["storage"]["value"].endswith(case.get("storage_value_suffix", ""))
                   and relation["invocation"]["location"]["path"] == case.get("invocation_file", path.name)
                   and relation["invocation"]["location"]["line"] == case["invocation_line"]]
        calls = [relation for relation in matches if relation["kind"] in CALL_RELATIONS]
        returned = [relation for relation in matches if relation["kind"] in DATA_RELATIONS]
        consumed = [relation for relation in matches if relation["kind"] in CONSUMPTION_RELATIONS]
        arguments = [relation for relation in matches if relation["kind"] == "stored_value_argument"]
        parse_errors = [notice for notice in result["notices"] if notice["kind"] == "parse_error"]
        has_contract_error = any(relation.get("certainty") != "conditional_source_relation"
                                 or relation.get("concrete_instance_proven") is not False
                                 or relation.get("event_classification") != "not_inferred"
                                 for relation in result["relations"])
        positives = (consumed if case.get("semantic_kind") == "data_consumption" else
                     returned if case.get("semantic_kind") == "data_return" else calls)
        is_expected = bool(positives) if case["expected"] == "connected" else not matches
        if case.get("required_conditions"):
            is_expected = is_expected and any(set(case["required_conditions"]).issubset(relation["conditions"]) for relation in positives)
        row = {**case, "status": "conditional_candidate" if calls else
               ("conditional_data_return" if returned else ("conditional_data_consumption" if consumed else ("argument_transfer_only" if arguments else "unresolved"))),
               "call_matches": len(calls), "argument_matches": len(arguments), "data_return_matches": len(returned), "data_consumption_matches": len(consumed),
               "parse_errors": parse_errors, "has_contract_error": has_contract_error,
               "passed": is_expected and not parse_errors and not has_contract_error}
        rows.append(row)
        print(f"{'PASS' if row['passed'] else 'FAIL'} {case['id']}: {row['status']} "
              f"(calls={len(calls)}, arguments={len(arguments)}, returns={len(returned)}, parse_errors={len(parse_errors)})",
              flush=True)
    evaluation_path = args.output / "evaluation.json"
    if evaluation_path.exists() and len(selected) != len(cases):
        selected_ids = {case["id"] for case in selected}
        previous = json.loads(evaluation_path.read_text())
        rows = [row for row in previous if row["id"] not in selected_ids] + rows
    rows.sort(key=lambda row: row["id"])
    by_id = {row["id"]: row for row in rows}
    for row in rows:
        if row.get("positive_control"):
            control = by_id.get(row["positive_control"])
            row["has_positive_control"] = bool(control and control["passed"] and control["status"] in {"conditional_candidate", "conditional_data_return", "conditional_data_consumption"})
            row["passed"] = row["passed"] and row["has_positive_control"]
    evaluation_path.write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")
    failures = [row["id"] for row in rows if not row["passed"]]
    print(json.dumps({"measured_this_run": len(selected), "recorded_cases": len(rows),
                      "passed": len(rows) - len(failures), "failures": failures}, ensure_ascii=False))
    return int(any(not row["passed"] for row in rows if row["id"] in {case["id"] for case in selected}))


if __name__ == "__main__":
    raise SystemExit(main())
