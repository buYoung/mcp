#!/usr/bin/env python3
"""Audit supplemental measurements, frozen inputs and the unchanged prior phase."""

import argparse
import ast
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys


DIRECTORY = Path(__file__).resolve().parent
CORPUS = DIRECTORY.parent
POC = CORPUS.parent
sys.path.insert(0, str(CORPUS))
from manage import evaluate
from consumers import evaluate_consumers


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def statuses(rows):
    return dict(Counter(row["status"] for row in rows))


def verify_relations(data):
    assert data["target_program_executed"] is False
    for relation in data["relations"]:
        assert relation["certainty"] == "conditional_source_relation"
        assert relation["concrete_instance_proven"] is False
        assert relation["event_classification"] == "not_inferred"
        if relation["kind"] == "stored_value_return":
            assert relation["invocation"]["kind"] == "return"
            assert "returned_value_only_not_callback_execution" in relation["conditions"]
        for kind, condition in {"stored_object_write": "object_write_only_not_callback_execution",
                                "stored_object_read": "object_read_only_not_callback_execution",
                                "stored_key_lookup": "lookup_key_only_not_callback_execution"}.items():
            if relation["kind"] == kind:
                assert condition in relation["conditions"]
        consumer = relation["storage"].get("consumer")
        if consumer:
            assert all(relation["invocation"][key] == value for key, value in consumer.items())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--dependencies", type=Path, default=Path("/tmp/codemap-language-dependencies"))
    parser.add_argument("--composites", type=Path, default=Path("/tmp/codemap-language-composite-sources"))
    args = parser.parse_args()
    subprocess.run([sys.executable, str(CORPUS / "public-gaps/verify.py"), "--sources", str(args.sources)], check=True)
    base_verification = read(CORPUS / "regressions/results/verification.json")
    baseline = DIRECTORY / "baseline"
    preserved = read(baseline / "manifest.json")["files"]
    for relative, expected in preserved.items():
        assert digest(baseline / relative) == expected, relative
    previous_stage = DIRECTORY / "history/165-cases"
    stage_files = read(previous_stage / "manifest.json")["files"]
    for relative, expected in stage_files.items():
        assert digest(previous_stage / relative) == expected, relative

    current_cases = read(CORPUS / "regressions/cases.json")
    cases_by_id = {case["id"]: case for case in current_cases}
    previous_cases = read(baseline / "language-corpus/regressions/cases.json")
    assert all(cases_by_id[case["id"]] == case for case in previous_cases)
    stage_cases = read(previous_stage / "language-corpus/regressions/cases.json")
    assert all(cases_by_id[case["id"]] == case for case in stage_cases)
    latest_baseline = DIRECTORY / "history/196-cases"
    latest_files = read(latest_baseline / "manifest.json")["files"]
    for relative, expected in latest_files.items():
        assert digest(latest_baseline / relative) == expected
    latest_cases = read(latest_baseline / "language-corpus/regressions/cases.json")
    assert all(cases_by_id[case["id"]] == case for case in latest_cases)
    current = read(CORPUS / "results/evaluation.json")["cases"]
    previous = read(baseline / "language-corpus/results/evaluation.json")["cases"]
    assert {row["id"]: row["status"] for row in current} == {row["id"]: row["status"] for row in previous}
    regression_rows = read(CORPUS / "regressions/results/evaluation.json")
    regression_by_id = {row["id"]: row for row in regression_rows}
    for row in read(baseline / "language-corpus/regressions/results/evaluation.json"):
        assert regression_by_id[row["id"]]["status"] == row["status"]

    helpers = [*DIRECTORY.glob("*.py"), CORPUS / "public-gaps/verify.py", CORPUS / "regressions/run.py"]
    for path in helpers:
        ast.parse(path.read_bytes(), filename=str(path))
    helper_hashes = {str(path.relative_to(POC)): digest(path) for path in helpers}
    lock = read(DIRECTORY / "effect-source.lock.json")
    assert lock["target_executed"] is False
    dependency_root = args.dependencies / (lock["name"] + "-" + lock["version"])
    for relative, expected in lock["source_sha256"].items():
        assert digest(dependency_root / relative) == expected, relative

    manifest = read(DIRECTORY / "inputs.json")
    labels = read(CORPUS / "cases.json")["cases"]
    variants = {}
    best = {row["id"]: row for row in current if row["repository"] != "fixtures" and row["kind"] == "positive"}
    rank = {"unresolved": 0, "argument_transfer_only": 1, "conditional_data_return": 2,
            "conditional_data_consumption": 2, "conditional_candidate": 3}
    raw = {}
    for name, spec in manifest["variants"].items():
        root = args.composites / spec["composite"] if spec.get("composite") else args.sources / spec["repository"]
        path = DIRECTORY / "results" / (name + ".json.gz")
        data = json.loads(gzip.decompress(path.read_bytes()))
        raw[name] = data
        assert data["implementation_sha256"] == base_verification["implementation_sha256"], name
        assert data["scope"]["source_sha256"] == spec["source_sha256"], name
        assert data["scope"]["max_file_bytes"] == spec.get("max_file_bytes", 524288), name
        assert data["scope"].get("module_bindings", {}) == spec.get("module_bindings", {}), name
        assert data["revision"] == spec["revision"] and data["input_variant"] == name
        revision = subprocess.check_output(["git", "-C", str(args.sources / spec["repository"]), "rev-parse", "HEAD"], text=True).strip()
        assert revision == spec["revision"]
        for relative, expected in spec["source_sha256"].items():
            source = root / relative
            assert source.is_file() and not source.is_symlink() and source.resolve().is_relative_to(root.resolve())
            assert digest(source) == expected and source.stat().st_size <= data["scope"]["max_file_bytes"]
        assert not any(notice["kind"] in {"parse_error", "missing_input", "file_size_cap", "input_file_cap", "input_byte_cap"} for notice in data["notices"]), name
        verify_relations(data)
        selected = [case for case in labels if case["repository"] == spec["repository"] and case["language"] == spec["language"]]
        measured = read(DIRECTORY / "results" / (name + ".evaluation.json"))
        assert measured["cases"] == evaluate(selected, {(spec["repository"], spec["language"]): data})["cases"]
        assert measured["input_lock_sha256"] == digest(DIRECTORY / "inputs.json")
        assert measured["max_file_bytes"] == data["scope"]["max_file_bytes"]
        variants[name] = {"files": data["metrics"]["files"], "input_bytes": data["metrics"]["input_bytes"],
                          "max_file_bytes": data["scope"]["max_file_bytes"], "statuses": statuses(measured["cases"]),
                          "cases": [{"id": row["id"], "status": row["status"]} for row in measured["cases"]],
                          "analysis_sha256": digest(path)}
        for row in measured["cases"]:
            assert row["status"] != "incorrect_connection"
            if row["id"] in best and rank[row["status"]] > rank[best[row["id"]]["status"]]:
                best[row["id"]] = row

    bevy = raw["bevy-modules"]
    crates = read(DIRECTORY / "crate-sources.lock.json")
    assert crates["revision"] == bevy["revision"]
    for crate in crates["crates"]:
        content = subprocess.check_output(["git", "-C", str(args.sources / "bevy"), "show", crates["revision"] + ":" + crate["manifest"]])
        assert hashlib.sha256(content).hexdigest() == crate["manifest_sha256"]
        assert manifest["variants"]["bevy-modules"]["module_bindings"][crate["crate"]] == crate["source"]
    returns = [relation for relation in bevy["relations"] if relation["kind"] == "stored_value_return"
               and relation["storage"]["location"]["path"].endswith("/message/messages.rs")
               and relation["storage"]["location"]["line"] in {139, 140}
               and relation["invocation"]["location"]["path"].endswith("/message/iterators.rs")
               and relation["invocation"]["location"]["line"] == 97]
    assert {relation["storage"]["target"].rsplit("[", 1)[-1] for relation in returns} == {"key:message]", "key:message_id]"}
    assert all(relation["storage"]["target"] == relation["invocation"]["target"] for relation in returns)
    assert any(fact["kind"] == "member_invoke" and fact["location"]["path"].endswith("bevy_gltf/src/loader/mod.rs")
               and fact["location"]["line"] == 1754 for fact in bevy["facts"])
    monix_names = {"monix-jvm-scala2", "monix-jvm-scala3", "monix-js-scala2", "monix-js-scala3"}
    for name in monix_names:
        assert variants[name]["statuses"] == {"conditional_candidate": 1, "source_scenario_not_executed": 2}
        subscriber_case = next(case for case in labels if case["id"] == "scala-state-subscriber")
        new_subscriber = [relation for relation in raw[name]["relations"]
                          if relation["storage"]["value"].endswith(":subscriber")
                          and all(relation[key]["location"]["path"] == subscriber_case[key][0]
                                  and subscriber_case[key][1] <= relation[key]["location"]["line"] <= subscriber_case[key][2]
                                  for key in ("storage", "invocation"))]
        required = {"source_companion_implicit_selection_required"}
        if name.endswith("2"):
            required.update({"source_macro_template_preservation_required", "compiler_macro_typing_unproven"})
        if "-jvm-" in name:
            required.update({"qualified_jdk_varhandle_access", "compare_and_set_success_required"})
        assert any(required.issubset(relation["conditions"]) for relation in new_subscriber), name
        paths = raw[name]["scope"]["source_sha256"]
        scala_version = name[-1]
        assert any("/scala-" + scala_version + "/" in path for path in paths)
        assert not any("/scala-" + ("3" if scala_version == "2" else "2") + "/" in path for path in paths)
        assert any("/atomic/" + ("jvm" if "-jvm-" in name else "js") + "/" in path for path in paths)
    effect = raw["livestore-effect"]
    stream = args.composites / "livestore-effect/__dependencies__/effect/src/Stream.ts"
    assert 524288 < stream.stat().st_size <= effect["scope"]["max_file_bytes"] == 1048576
    projections = [notice for notice in effect["notices"] if notice["kind"].startswith("typescript_type_")]
    assert len(projections) == 12
    for notice in projections:
        facts = [fact for fact in effect["facts"] if fact["location"]["path"] == notice["path"]]
        assert all("generic_type_constraints_unproven" in fact["conditions"] for fact in facts)
    for path in (CORPUS / "results/analysis").glob("*.json.gz"):
        assert json.loads(gzip.decompress(path.read_bytes()))["scope"]["max_file_bytes"] == 524288
    consumer_manifest = read(DIRECTORY / "consumers.json")
    consumer_result = read(DIRECTORY / "results/consumers.evaluation.json")
    consumer_rows = evaluate_consumers(consumer_manifest["cases"], raw)
    assert consumer_result["cases"] == consumer_rows
    assert consumer_result["cases_lock_sha256"] == digest(DIRECTORY / "consumers.json")
    assert consumer_result["target_programs_executed"] is False
    for name, expected in consumer_result["analysis_sha256"].items():
        assert digest(DIRECTORY / "results" / (name + ".json.gz")) == expected
    assert statuses(consumer_rows) == {"conditional_data_consumption": 2, "conditional_candidate": 1,
                                       "unresolved": 1, "correctly_unjoined_with_positive_control": 2}

    comparison = {"verified_on": "2026-09-16", "aggregation": "best_observed_status_per_public_positive_case_across_selected_inputs",
                  "base_case_statuses_unchanged": True, "previous_base_public": statuses(row for row in previous if row["repository"] != "fixtures"),
                  "current_base_public": statuses(row for row in current if row["repository"] != "fixtures"),
                  "supplemental_variants": variants, "combined_public_positive_statuses": statuses(best.values()),
                  "combined_public_positive_cases": [{"id": row["id"], "status": row["status"]} for row in best.values()],
                  "supplemental_consumer_statuses": statuses(consumer_rows),
                  "supplemental_consumer_cases": [{"id": row["id"], "status": row["status"]} for row in consumer_rows],
                  "target_programs_executed": False}
    summary = {"verified_on": "2026-09-16", "base_verification": "../regressions/results/verification.json",
               "implementation_sha256": base_verification["implementation_sha256"], "helper_sha256": helper_hashes,
               "input_lock_sha256": digest(DIRECTORY / "inputs.json"), "dependency_lock_sha256": digest(DIRECTORY / "effect-source.lock.json"),
               "prior_phase_files_unchanged": len(preserved), "prior_regression_cases_unchanged": len(previous_cases),
               "previous_stage_files_unchanged": len(stage_files), "previous_stage_regression_cases_unchanged": len(stage_cases),
               "latest_baseline_files_unchanged": len(latest_files), "latest_baseline_regression_cases_unchanged": len(latest_cases),
               "crate_manifest_bindings_verified": len(crates["crates"]),
               "consumer_cases_sha256": digest(DIRECTORY / "consumers.json"),
               "dependency_files_verified": len(lock["source_sha256"]), "supplemental_analysis_outputs_verified": len(variants),
               "current_analysis_outputs_verified": base_verification["current_analysis_outputs_verified"] + len(variants),
               "regression_cases": len(regression_rows), "regression_passed": sum(row["passed"] for row in regression_rows),
               "regression_callback_positives": sum(row["status"] == "conditional_candidate" for row in regression_rows),
               "regression_data_return_positives": sum(row["status"] == "conditional_data_return" for row in regression_rows),
               "regression_data_consumption_positives": sum(row["status"] == "conditional_data_consumption" for row in regression_rows),
               "regression_disconnected": sum(row["expected"] == "disconnected" for row in regression_rows),
               "regression_unresolved_guards": sum(row["expected"] == "unresolved" for row in regression_rows),
               "combined_public_positive_statuses": comparison["combined_public_positive_statuses"],
               "supplemental_consumer_statuses": statuses(consumer_rows),
               "effect_projected_type_sources": len(projections), "effect_stream_bytes": stream.stat().st_size,
               "target_programs_executed": False}
    (DIRECTORY / "comparison.json").write_text(json.dumps(comparison, ensure_ascii=False, indent=2) + "\n")
    (DIRECTORY / "verification.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: value for key, value in summary.items() if not key.endswith("sha256")}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
