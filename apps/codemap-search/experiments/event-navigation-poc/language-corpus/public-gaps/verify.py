#!/usr/bin/env python3
"""Audit saved measurements against source, implementation and the checkpoint."""

import argparse
import ast
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess


POC = Path(__file__).resolve().parents[2]
CORPUS = POC / "language-corpus"
REGRESSIONS = CORPUS / "regressions"
CHECKPOINT = "01aac1515864f7066c368e0081d6fd23836a5474"


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, required=True)
    args = parser.parse_args()
    prefix = subprocess.check_output(["git", "-C", str(POC), "rev-parse", "--show-prefix"], text=True).strip()

    def checkpoint(relative):
        return subprocess.check_output(["git", "-C", str(POC), "show", f"{CHECKPOINT}:{prefix}{relative}"])

    implementation_files = [*POC.glob("*.py"), *CORPUS.glob("*.py")]
    implementation = {str(path.relative_to(POC)): digest(path) for path in implementation_files}
    parsed_files = implementation_files + [REGRESSIONS / "run.py", Path(__file__)]
    for path in parsed_files:
        ast.parse(path.read_bytes(), filename=str(path))
    for name in ("sources.lock.json", "repositories.json", "cases.json", "fixture-cases.json"):
        assert (CORPUS / name).read_bytes() == checkpoint("language-corpus/" + name), name
    history = CORPUS / "history/before-language-gap-fixes"
    preserved = read(history / "manifest.json")["files"]
    assert all(digest(history / path) == expected for path, expected in preserved.items())
    cases = read(REGRESSIONS / "cases.json")
    rows = read(REGRESSIONS / "results/evaluation.json")
    by_id = {case["id"]: case for case in cases}
    results_by_id = {row["id"]: row for row in rows}
    original_cases = json.loads(checkpoint("language-corpus/regressions/cases.json"))
    assert len(by_id) == len(cases) == len(rows)
    assert set(by_id) == set(results_by_id)
    assert all(case == by_id[case["id"]] for case in original_cases)
    assert all(row["passed"] and not row["parse_errors"] for row in rows)
    for case in cases:
        assert all(results_by_id[case["id"]][key] == value for key, value in case.items())
        assert digest(REGRESSIONS / "examples" / case["path"]) == case["source_sha256"]
        if case.get("positive_control"):
            expected = {"data_return": "conditional_data_return", "data_consumption": "conditional_data_consumption"}.get(case.get("semantic_kind"), "conditional_candidate")
            assert results_by_id[case["positive_control"]]["status"] == expected
    original_examples = read(REGRESSIONS / "baseline/evaluation.json")
    for old in original_examples:
        case = next(row for row in cases if row["language"] == old["language"] and row.get("baseline_variant") == old["kind"])
        assert case["source_sha256"] == old["source_sha256"]
        assert results_by_id[case["id"]]["status"] == "conditional_candidate"
    checked = []

    def verify_analysis(path, root, revision=None):
        data = json.loads(gzip.decompress(path.read_bytes()))
        assert data["implementation_sha256"] == implementation, str(path)
        for relative, expected in data["scope"]["source_sha256"].items():
            assert digest(root / relative) == expected, (str(path), relative)
        if revision is not None:
            assert data["revision"] == revision
        assert data["target_program_executed"] is False
        for relation in data["relations"]:
            assert relation["certainty"] == "conditional_source_relation"
            assert relation["concrete_instance_proven"] is False
            assert relation["event_classification"] == "not_inferred"
            consumer = relation["storage"].get("consumer")
            if consumer:
                assert all(relation["invocation"][key] == value for key, value in consumer.items())
            if "object_copy_registration_projection" in relation["storage"]["conditions"]:
                assert consumer is not None
        checked.append(str(path.relative_to(CORPUS)))
        return data

    repositories = read(CORPUS / "repositories.json")
    for path in sorted((CORPUS / "results/analysis").glob("*.json.gz")):
        name = path.name.split("--", 1)[0]
        verify_analysis(path, CORPUS if name == "fixtures" else args.sources / name,
                        None if name == "fixtures" else repositories[name]["revision"])
    for source_path in sorted({case["path"] for case in cases}):
        case = next(case for case in cases if case["path"] == source_path)
        output = REGRESSIONS / "results/analysis" / (source_path.replace("/", "--") + ".json.gz")
        data = verify_analysis(output, (REGRESSIONS / "examples" / source_path).parent)
        assert data["scope"].get("module_bindings", {}) == case.get("module_bindings", {})
        assert set(data["scope"]["source_sha256"]) == {Path(source_path).name, *case.get("additional_sources", {}), *case.get("support_sources", {})}
        for selected in (case for case in cases if case["path"] == source_path and case.get("relation_kinds")):
            assert any(relation["kind"] in selected["relation_kinds"]
                       and relation["storage"]["location"]["path"] == selected.get("storage_file", Path(source_path).name)
                       and relation["storage"]["location"]["line"] == selected["storage_line"]
                       and relation["invocation"]["location"]["path"] == selected.get("invocation_file", Path(source_path).name)
                       and relation["invocation"]["location"]["line"] == selected["invocation_line"]
                       and set(selected.get("required_conditions", ())).issubset(relation["conditions"])
                       for relation in data["relations"]), selected["id"]
    before = json.loads(checkpoint("language-corpus/results/evaluation.json"))["cases"]
    after = read(CORPUS / "results/evaluation.json")["cases"]
    before_by_id = {case["id"]: case for case in before}
    assert set(before_by_id) == {case["id"] for case in after}
    changes = []
    for row in after:
        old = before_by_id[row["id"]]
        if row["repository"] == "fixtures" or old["status"] == "conditional_candidate":
            assert row["status"] == old["status"], row["id"]
        if row["kind"] == "negative" and row["mode"] == "endpoint_pair":
            assert row["status"] == "correctly_unjoined_with_positive_control", row["id"]
        if old["status"] != row["status"]:
            changes.append({"id": row["id"], "before": old["status"], "after": row["status"]})
    documents = [POC / "README.ko.md", CORPUS / "README.ko.md", REGRESSIONS / "README.ko.md",
                 CORPUS / "public-gaps/README.ko.md", CORPUS / "remaining-routes/README.ko.md"]
    link_count = 0
    for path in documents:
        for target in re.findall(r"\]\(([^)]+)\)", path.read_text()):
            if "://" in target or target.startswith("#"):
                continue
            target_path = path.parent / target.split("#", 1)[0]
            if target_path.resolve() in {(REGRESSIONS / "results/verification.json").resolve(),
                                         (CORPUS / "public-gaps/comparison.json").resolve(),
                                         (CORPUS / "remaining-routes/verification.json").resolve(),
                                         (CORPUS / "remaining-routes/comparison.json").resolve()}:
                continue
            assert target_path.exists(), (str(path), target)
            link_count += 1
    summary = {
        "verified_on": "2026-09-16", "checkpoint": CHECKPOINT,
        "python_implementation_files_parsed": len(parsed_files), "implementation_sha256": implementation,
        "runner_sha256": digest(REGRESSIONS / "run.py"), "verifier_sha256": digest(Path(__file__)),
        "cases_sha256": digest(REGRESSIONS / "cases.json"), "source_lock_sha256": digest(CORPUS / "sources.lock.json"),
        "baseline_files_unchanged": len(preserved), "original_examples_unchanged": len(original_examples),
        "checkpoint_regression_cases_unchanged": len(original_cases), "current_analysis_outputs_verified": len(checked),
        "public_and_basic_case_status_changes": changes, "public_and_basic_cases": len(after),
        "regression_cases": len(rows), "regression_passed": sum(row["passed"] for row in rows),
        "regression_expected": dict(Counter(row["expected"] for row in rows)),
        "public_statuses": dict(Counter(row["status"] for row in after if row["repository"] != "fixtures")),
        "current_document_local_links_verified": link_count, "target_programs_executed": False,
        "analysis_outputs": checked,
    }
    (REGRESSIONS / "results/verification.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    targets = {case["id"] for case in read(CORPUS / "public-gaps/baseline.json")["cases"]}
    comparison = {"checkpoint": CHECKPOINT, "unchanged_public_input_lock": True,
                  "changes": changes, "current_target_results": [row for row in after if row["id"] in targets]}
    (CORPUS / "public-gaps/comparison.json").write_text(json.dumps(comparison, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: value for key, value in summary.items() if key not in {"implementation_sha256", "analysis_outputs"}}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
