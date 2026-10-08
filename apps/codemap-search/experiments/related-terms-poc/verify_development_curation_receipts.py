"""Durable, completed-workflow publication evidence; frozen v2 inputs remain unchanged."""
from __future__ import annotations

import argparse
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

import batch_development_curation as producer

ROOT = producer.preparation.CURATION / "durable-verification-v3"
MANIFEST = ROOT / "manifest.json"
CONTRACT = producer.preparation.EXPERIMENT_DIR / "contracts" / "development-curation-durable-publication.md"
VERSION = "development-curation-durable-verification/3"
REVIEWER_TOOLS = {"read", "grep", "find", "ls", "watchdog_diff", "contact_supervisor"}
SOURCE_SEAMS = {
    "runs/shared/structured-output.js": "createStructuredOutputToolParameters permits value plus separate acceptanceReport; collector saves report separately without changing value",
    "runs/shared/single-output.js": "persistSingleOutput/resolveSingleOutput: savedPath follows successful write or changed-file read, not a requested path",
    "runs/background/subagent-runner.js": "exit0 structured JSON persistence; savedOutputPath; status projection of completion/error/timeout",
    "runs/background/chain-root-attachment.js": "imported publication carries savedOutputPath; status-only fallback has success=false and no saved path",
    "runs/foreground/subagent-executor.js": "importWorkflowAwaitedChildResult consumes publication; workflowChildResult derives outputReference from savedOutputPath; completed receipt writer",
    "runs/background/await-async-run.js": "claimWorkflowAwaitedResult renames transient result",
    "runs/background/result-files.js": "removeResultIndex removes transient pending/index records",
    "workflows/workflow-receipt.js": "buildWorkflowReceipt persists child outputReference and latestRunId; writeWorkflowReceipt",
    "workflows/workflow-child-summary.js": "completed child summary includes exact child key, run and official model",
    "api/preflight.js": "external CLI/job plus outputSchema is rejected; native schema-bound events provide positive execution evidence",
    "runs/shared/external-cli-contract.js": "external stdout is untrusted and cannot supply native toolEvents/structuredOutput",
}
FLAGS = ("error", "outputPartial", "structuredOutputFailed", "outputSaveError", "artifactOutputSaveFailed",
         "stopped", "timedOut", "interrupted", "externalProcess", "externalJob", "runner", "toolBudgetBlocked", "contextOverflow")
require = producer.require
load = producer.load
sha256_file = producer.sha256_file
json_bytes = producer.json_bytes


def exact_path(path: Path) -> Path:
    require(path.is_absolute() and path.resolve() == path, f"absolute non-symlink path required: {path}")
    return path


def run_id(value: str) -> str:
    require(type(value) is str and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", value) is not None, "invalid exact run ID")
    return value


def publish(path: Path, data: bytes) -> None:
    exact_path(ROOT)
    exact_path(path)
    require(path.is_relative_to(ROOT), "write outside verification revision")
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        require(path.read_bytes() == data, f"refuse overwrite: {path}")
    else:
        with path.open("xb") as handle:
            handle.write(data)


def prepare(source_root: Path) -> dict:
    producer.plan(False)
    original = load(producer.PLAN)
    exact_path(source_root)
    sources = [{"path": str(source_root / relative), "sha256": sha256_file(exact_path(source_root / relative)), "basis": basis}
               for relative, basis in SOURCE_SEAMS.items()]
    # Include already stored results and historical failures, not only the 150 fixed inputs.
    previous = list(producer.preparation.CURATION.glob("readonly-batches*"))
    previous += [p for p in producer.preparation.CURATION.glob("durable-verification-*") if p != ROOT]
    preserved = sorted(p for folder in previous for p in folder.rglob("*") if p.is_file())
    manifest = {"schema_version": VERSION, "verifier_path": str(Path(__file__).resolve()),
                "verifier_sha256": sha256_file(Path(__file__)), "contract_path": str(CONTRACT),
                "contract_sha256": sha256_file(CONTRACT), "input_manifest_path": str(producer.PLAN),
                "input_manifest_sha256": sha256_file(producer.PLAN), "producer_path": original["helper_path"],
                "producer_sha256": original["helper_sha256"], "input_contract_path": original["contract_path"],
                "input_contract_sha256": original["contract_sha256"], "runtime_source_proof": sources,
                "preserved_files": [{"path": str(p), "sha256": sha256_file(exact_path(p))} for p in preserved],
                "approved_launch": {"agent": "reviewer", "registered_tools": sorted(REVIEWER_TOOLS),
                                    "basis": "Supervisor-confirmed builtin reviewer capabilities; exact inventory corroborated by official recovery descriptor. Invocation policy remains read/structured_output only."},
                "prefix_concepts": 1839, "pending_concepts": 5308, "batch_count": 142,
                "scope": "Verification/writer revision only; producer, assignments, schema, prefix and classifications are not regenerated."}
    publish(MANIFEST, json_bytes(manifest))
    return {"status": "prepared", "manifest_path": str(MANIFEST), "manifest_sha256": sha256_file(MANIFEST), "preserved_files": len(preserved)}


def verify_revision() -> dict:
    manifest = load(exact_path(MANIFEST))
    require(manifest["schema_version"] == VERSION, "verification revision mismatch")
    expected = {"verifier_path": str(Path(__file__).resolve()), "contract_path": str(CONTRACT),
                "input_manifest_path": str(producer.PLAN), "producer_path": str(Path(producer.__file__).resolve()),
                "input_contract_path": str(producer.CONTRACT)}
    for key, path in expected.items():
        require(manifest[key] == path, f"revision path mismatch: {key}")
        digest_key = {"input_manifest_path": "input_manifest_sha256", "producer_path": "producer_sha256", "input_contract_path": "input_contract_sha256"}.get(key, key.replace("_path", "_sha256"))
        require(sha256_file(exact_path(Path(path))) == manifest[digest_key], f"revision hash mismatch: {key}")
    for item in manifest["runtime_source_proof"] + manifest["preserved_files"]:
        require(sha256_file(exact_path(Path(item["path"]))) == item["sha256"], f"preserved source/file changed: {item['path']}")
    producer.plan(False)
    return manifest


def reject_recorded_failure(record: dict, label: str) -> None:
    require(type(record) is dict, f"{label}: metadata object required")
    require(not any(record.get(key) for key in FLAGS), f"{label}: recorded error/incomplete/external execution")
    require(not record.get("terminalOutcome") and not record.get("acceptanceRecovery") and not record.get("externalAdapter"), f"{label}: partial/recovery/external outcome")
    if record.get("execution") is not None:
        execution = record["execution"]
        require(execution.get("status") in ("complete", "completed") and execution.get("success") is True and execution.get("exitCode") == 0, f"{label}: execution incomplete")


def publication(shard: dict, assignment: dict, binding: dict, trace_root: Path, output: dict) -> dict:
    rid, parent_id = run_id(binding["run_id"]), run_id(binding["workflow_run_id"])
    key = binding["workflow_key"]
    require(type(key) is str and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", key) is not None, "invalid workflow key")
    require(rid != parent_id, "child cannot be its own completed workflow")
    directory, parent = exact_path(trace_root / rid), exact_path(trace_root / parent_id)
    receipt_path = exact_path(Path(binding["receipt_path"]))
    require(receipt_path == parent / "workflow-receipt.json", "receipt outside exact official workflow directory")
    paths = {"child_status": directory / "status.json", "parent_status": parent / "status.json",
             "receipt": receipt_path, "process_terminal": directory / "process-terminal.json",
             "recovery_descriptor": directory / "recovery-descriptor.json", "output": Path(shard["output_path"])}
    status, parent_status, receipt, terminal, descriptor = [load(exact_path(paths[name])) for name in ("child_status", "parent_status", "receipt", "process_terminal", "recovery_descriptor")]
    require(status.get("runId") == rid and status.get("state") == "complete" and status.get("mode") == "single", "child not complete native single run")
    require(status.get("cwd") == str(producer.preparation.REPO_ROOT), "child cwd mismatch")
    require((status.get("parentWorkflowRunId"), status.get("workflowKey")) == (parent_id, key), "child parent/key binding mismatch")
    steps = status.get("steps")
    require(type(steps) is list and len(steps) == 1, "single child step required")
    step = steps[0]
    require(step.get("status") == "complete" and type(step.get("exitCode")) is int and step["exitCode"] == 0, "child step not complete exit0")
    model = step.get("model")
    require(type(model) is str and re.fullmatch(r"(?:[^/:]+/)?gpt-6\.1-sol(?::(?:minimal|low|medium|high|xhigh))?", model) is not None, "official model mismatch")
    require(parent_status.get("runId") == parent_id and parent_status.get("state") == "complete" and parent_status.get("mode") == "workflow", "parent workflow is not complete")
    require(parent_status.get("cwd") == status["cwd"] and parent_status.get("workflowReceiptPath") == str(receipt_path), "parent receipt/cwd binding mismatch")
    require(receipt.get("version") == 1 and receipt.get("workflowRunId") == parent_id and receipt.get("state") == "complete", "receipt not completed official workflow")
    summary = receipt.get("workflowChildren")
    require(type(summary) is dict and summary.get("version") == 1 and summary.get("workflowRunId") == parent_id and summary.get("inventoryComplete") is True and summary.get("workflowState") == "completed", "receipt child inventory incomplete")
    children = summary.get("children")
    require(type(children) is list and all(type(c) is dict for c in children), "invalid receipt child inventory")
    require(len({c.get("childId") for c in children}) == len(children), "duplicate receipt child key")
    matches = [c for c in children if c.get("childId") == key]
    require(len(matches) == 1, "exact receipt child key missing")
    child = matches[0]
    require((child.get("runId"), child.get("state"), child.get("model")) == (rid, "completed", model), "receipt child run/state/model mismatch")
    entries = receipt.get("entries")
    require(type(entries) is dict and key in entries, "receipt entry missing")
    entry = entries[key]
    require(entry.get("key") == key and entry.get("latestRunId") == rid and entry.get("outputReference") == shard["output_path"], "completed publication run/key/output mismatch")
    chain = entry.get("continuation", {}).get("runIds")
    require(type(chain) is list and bool(chain) and chain[-1] == rid and len(set(chain)) == len(chain), "receipt continuation latest run mismatch")
    require(child.get("agent") == step.get("agent") == entry.get("agent") == descriptor.get("agent"), "official agent binding mismatch")
    for label, record in (("child status", status), ("step", step), ("parent status", parent_status), ("receipt", receipt), ("entry", entry), ("receipt child", child), ("launch", descriptor)):
        reject_recorded_failure(record, label)
    require(terminal.get("version") == 1 and terminal.get("state") == "observed" and terminal.get("runId") == rid, "terminal observation missing")
    require(status.get("processTerminal") == terminal, "status/terminal proof mismatch")
    instances = terminal.get("instances")
    require(type(instances) is list and len(instances) == 1, "single normal runner terminal required")
    runner = instances[0]
    require(runner.get("kind") == "runner" and runner.get("processInstanceId") == terminal.get("runnerProcessInstanceId"), "terminal runner identity mismatch")
    require(type(runner.get("exitCode")) is int and runner["exitCode"] == 0 and "signal" in runner and runner["signal"] is None and type(runner.get("closeObservedAt")) is int and runner["closeObservedAt"] > 0, "runner did not exit normally")
    require(descriptor.get("version") == 1 and descriptor.get("cwd") == status["cwd"] and descriptor.get("model") == model, "schema-bound launch cwd/model mismatch")
    require(descriptor.get("outputPath") == shard["output_path"] and descriptor.get("structuredOutputSchema") == producer.RESULT_SCHEMA, "launch output/schema mismatch")
    tools = descriptor.get("tools")
    require(descriptor.get("agent") == "reviewer" and type(tools) is list and all(type(t) is str for t in tools)
            and set(tools) == REVIEWER_TOOLS and len(tools) == len(set(tools)), "approved reviewer launch inventory mismatch")
    require(descriptor.get("launchContractDigest") == step.get("launchContractDigest") == status.get("launchContractDigest") and type(descriptor.get("launchContractDigest")) is str and re.fullmatch(r"[0-9a-f]{64}", descriptor["launchContractDigest"]) is not None, "launch contract digest binding mismatch")
    for name, field in (("captured", "structuredOutputPath"), ("captured_schema", "structuredOutputSchemaPath")):
        path = exact_path(Path(step[field]))
        require(path.is_relative_to(directory / "structured-output"), "captured output/schema outside exact child")
        paths[name] = path
    require(load(paths["captured"]) == step.get("structuredOutput") == output, "captured/status/designated JSON mismatch")
    require(load(paths["captured_schema"]) == producer.RESULT_SCHEMA, "official structured schema mismatch")
    require(load(exact_path(paths["output"])) == output, "output changed during publication validation")
    return {"kind": "completed-workflow-receipt/1", "workflow_run_id": parent_id, "workflow_key": key,
            "run_id": rid, "model": model, "agent": child["agent"], "output_reference": entry["outputReference"],
            "source_files": {name: {"path": str(path), "sha256": sha256_file(path)} for name, path in paths.items()},
            "runner_process_instance_id": terminal["runnerProcessInstanceId"], "runner_exit_code": 0, "runner_signal": None,
            "launch_registered_tools": tools, "invocation_policy": ["read", "structured_output"],
            "launch_contract_digest": descriptor["launchContractDigest"], "schema_bound_native_launch": True,
            "structured_json_complete_and_published": True,
            "recorded_flags": {label: {k: record[k] for k in FLAGS if k in record} for label, record in (("status", status), ("step", step), ("receipt_entry", entry))},
            "unretained_raw_result_flags": "not-recorded; never synthesized as false",
            "limitations": "Raw prose outputPartial and unrelated debug-artifact save flags are not retained. Consumed complete structured JSON is checked instead; semantic correctness is not proven."}


def read_evidence(shard: dict, assignment: dict, rid: str, trace_root: Path, output: dict, publication_proof: dict) -> dict:
    """Same native event state machine as frozen extract_run; reuse its confirmed_read."""
    events_path = exact_path(trace_root / rid / "events.jsonl")
    allowed = {shard["assignment_path"]: shard["assignment_sha256"], str(producer.CONTRACT): assignment["contract_sha256"], assignment["result_schema_path"]: assignment["result_schema_sha256"]}
    allowed.update({p["path"]: p["sha256"] for p in assignment["source_pages"]})
    for path, digest in allowed.items():
        require(sha256_file(exact_path(Path(path))) == digest, "fixed required input hash changed")
    starts, ended, calls = {}, set(), []
    coverage = collections.defaultdict(set)
    output_start_line = None
    parameter_keys, acceptance_wrapper = None, None
    with events_path.open(encoding="utf-8") as handle:
        for event_line, line in enumerate(handle, 1):
            if '"tool_execution_start"' not in line and '"tool_execution_end"' not in line:
                continue
            event = json.loads(line)
            kind = event.get("type")
            if kind not in ("tool_execution_start", "tool_execution_end"):
                continue
            require(event.get("subagentRunId") == rid, "native tool event run binding mismatch")
            call_id, tool = event.get("toolCallId"), event.get("toolName")
            require(type(call_id) is str and type(tool) is str, "native tool call identity missing")
            require(tool in ("read", "structured_output"), "classifier used a non-read/non-output tool")
            if kind == "tool_execution_start":
                require(output_start_line is None, "tool start after structured_output submission")
                require(call_id not in starts, "duplicate tool start")
                args = event.get("args", {})
                require(type(args) is dict, "tool args object missing")
                if tool == "read":
                    requested = args.get("path")
                    require(type(requested) is str, "read path missing")
                    path = Path(requested)
                    path = path if path.is_absolute() else producer.preparation.REPO_ROOT / path
                    require(str(path) in allowed and path.resolve() == path, "classification read outside assigned inputs")
                    require(sha256_file(path) == allowed[str(path)], "classification read input hash changed")
                    starts[call_id] = (args, path, event_line)
                else:
                    require(set(starts) == ended, "structured_output submitted before prior reads completed")
                    missing = {path: len(set(range(1, len(Path(path).read_text(encoding="utf-8").splitlines()) + 1)) - coverage[path]) for path in allowed}
                    require(not any(missing.values()), f"missing successful read coverage before structured_output: { {p: n for p, n in missing.items() if n} }")
                    require(set(args) in ({"value"}, {"value", "acceptanceReport"}) and args.get("value") == output, "unsupported structured_output wrapper / value differs from bound result")
                    parameter_keys = sorted(args)
                    if "acceptanceReport" in args:
                        require(type(args["acceptanceReport"]) is dict, "runtime acceptanceReport wrapper must be an object")
                        report_path = exact_path(Path(publication_proof["source_files"]["captured_schema"]["path"]).parent / "acceptance-report.json")
                        require(load(report_path) == args["acceptanceReport"], "official captured acceptance wrapper mismatch")
                        acceptance_wrapper = {"path": str(report_path), "sha256": sha256_file(report_path), "basis": "Separate runtime wrapper, not classification JSON or semantic-correctness proof."}
                    starts[call_id] = None
                    output_start_line = event_line
            else:
                require(call_id in starts, "tool end without a preceding registered start")
                require(call_id not in ended, "duplicate tool end")
                ended.add(call_id)
                require(tool == ("structured_output" if starts[call_id] is None else "read"), "tool start/end mismatch")
                require(event.get("isError") is False, "tool execution failed")
                if starts[call_id] is not None:
                    args, path, start_line = starts[call_id]
                    call = {"call_id": call_id, "tool_name": "read", "start_event_line": start_line, "end_event_line": event_line,
                            **producer.confirmed_read(args, event.get("result"), path)}
                    calls.append(call)
                    lo, hi = call["returned_line_range"]
                    coverage[call["path"]].update(range(lo, hi + 1))
    require(set(starts) == ended and sum(v is None for v in starts.values()) == 1, "unfinished tool call / exactly one structured_output required")
    if acceptance_wrapper is not None:
        require(sha256_file(Path(acceptance_wrapper["path"])) == acceptance_wrapper["sha256"], "runtime acceptance wrapper changed during read verification")
    return {"events_path": str(events_path), "read_calls": calls, "structured_output_start_event_line": output_start_line,
            "structured_output_parameter_keys": parameter_keys, "acceptance_report_wrapper": acceptance_wrapper,
            "coverage_complete_before_output": True, "coverage_complete": True}


def bindings(path: Path, batch_id: str | None, manifest: dict) -> tuple[Path, list[dict]]:
    exact_path(path)
    require(path.is_relative_to(ROOT), "bindings must be in new verification revision")
    value = load(path)
    require(type(value) is dict and set(value) == {"trace_root", "runs"} and type(value["runs"]) is list, "bindings need trace_root/runs")
    trace_root = exact_path(Path(value["trace_root"]))
    items = value["runs"]
    fields = {"batch_id", "run_id", "workflow_run_id", "workflow_key", "receipt_path"}
    require(all(type(x) is dict and set(x) == fields for x in items), "receipt binding fields mismatch")
    ids = [x["batch_id"] for x in items]
    require(len(ids) == len(set(ids)) and len({x["run_id"] for x in items}) == len(items), "duplicate batch/run binding")
    require(len({(x["workflow_run_id"], x["workflow_key"]) for x in items}) == len(items), "workflow child reused")
    expected = {s["id"] for s in manifest["shards"]}
    require(set(ids).issubset(expected), "unknown batch")
    if batch_id is None:
        require(set(ids) == expected, "final validation requires all 142 batches")
    else:
        require(batch_id in expected and ids == [batch_id], "partial validation requires exactly the named batch")
    return trace_root, items


def validate(bindings_path: Path, batch_id: str | None, merge: bool) -> dict:
    revision = verify_revision()
    manifest = load(producer.PLAN)
    pm, rm, records, prefixes, _, _, _, _ = producer.inputs()
    trace_root, items = bindings(bindings_path, batch_id, manifest)
    shards = {s["id"]: s for s in manifest["shards"]}
    converted, ledgers = {}, []
    for binding in items:
        shard = shards[binding["batch_id"]]
        assignment = load(Path(shard["assignment_path"]))
        output = load(exact_path(Path(shard["output_path"])))
        rows = producer.convert_result(shard, assignment, output, pm, rm, records)
        require(output["status"] == "complete", "incomplete structured batch")
        pub = publication(shard, assignment, binding, trace_root, output)
        reads = read_evidence(shard, assignment, binding["run_id"], trace_root, output, pub)
        ledger = {"schema_version": VERSION, "batch_id": shard["id"], "run_id": binding["run_id"],
                  "assignment_sha256": shard["assignment_sha256"], "concept_count": len(rows),
                  "lexicalization_count": sum(len(r["lexicalization_rationales"]) for r in rows), "page_count": len(assignment["source_pages"]),
                  "publication": pub, **reads, "limitations": "Delivery/publication/structure evidence, not semantic correctness."}
        # Recheck durable sources and fixed inputs after the stream, before accepting the ledger.
        for source in pub["source_files"].values():
            require(sha256_file(Path(source["path"])) == source["sha256"], "publication source changed during verification")
        for row in rows:
            require(row["concept_id"] not in converted and row["concept_id"] not in prefixes, "duplicate/prefix result")
            converted[row["concept_id"]] = row
        ledger_path = ROOT / "evidence" / f"{shard['id']}-{binding['run_id']}.json"
        publish(ledger_path, json_bytes(ledger))
        ledgers.append(str(ledger_path))
    if batch_id is None:
        require(set(converted) == set(records) - prefixes and len(converted) == 5308, "incomplete final coverage")
    require(not merge or batch_id is None, "partial merge forbidden")
    report = {"status": "ready", "scope": "single actual batch" if batch_id else "all 142 batches", "merged": False,
              "verification_manifest_path": str(MANIFEST), "verification_manifest_sha256": sha256_file(MANIFEST),
              "input_manifest_path": str(producer.PLAN), "input_manifest_sha256": revision["input_manifest_sha256"],
              "bindings_path": str(bindings_path), "bindings_sha256": sha256_file(bindings_path), "ledgers": ledgers,
              "validated_concepts": len(converted), "validated_lexicalizations": sum(len(r["lexicalization_rationales"]) for r in converted.values()),
              "prefix_concepts": len(prefixes), "preserved_files": len(revision["preserved_files"]),
              "semantic_correctness_guaranteed": False, "command": ["python3", "-B", str(Path(__file__).resolve()), *sys.argv[1:]],
              "cwd": str(producer.preparation.REPO_ROOT)}
    if merge:
        report.update(merge_canonical(converted))
    verify_revision()
    report["preservation"] = producer.preparation.verify_preservation()
    report_path = ROOT / "reports" / ((batch_id + "-" + items[0]["run_id"]) if batch_id else ("merge" if merge else "all-results"))
    report["report_path"] = str(report_path.with_suffix(".json"))
    publish(Path(report["report_path"]), json_bytes(report))
    return report


def merge_canonical(converted: dict) -> dict:
    """Same lossless canonical construction and official validators as frozen producer."""
    paths, validations = [], []
    for name in producer.PREFIXES:
        snapshot = producer.ROOT / "prefix" / f"{name}.jsonl"
        tail = [converted[cid] for cid in sorted(converted) if converted[cid]["packet_name"] == name]
        prefix = snapshot.read_bytes()
        data = prefix + b"".join((producer.canonical_json(row) + "\n").encode("utf-8") for row in tail)
        path = ROOT / "canonical" / f"{name}.jsonl"
        publish(path, data)
        require(path.read_bytes().startswith(prefix), "canonical prefix bytes changed")
        paths.append(path)
        command = [sys.executable, "-B", str(producer.preparation.EXPERIMENT_DIR / "project_development_curation.py"), "validate-decisions", str(path), "--packet", name]
        print(json.dumps({"validation_command": command, "cwd": str(producer.preparation.REPO_ROOT)}), file=sys.stderr, flush=True)
        completed = subprocess.run(command, cwd=producer.preparation.REPO_ROOT, capture_output=True, text=True)
        validations.append({"command": command, "exit_code": completed.returncode, "stdout": completed.stdout, "stderr": completed.stderr})
        if completed.returncode != 0:
            publish(ROOT / "canonical" / "validation-failure.json", json_bytes({"complete": False, "validations": validations}))
            raise ValueError("official validate-decisions failed; canonical files are incomplete")
    merged = ROOT / "canonical" / "decisions.jsonl"
    publish(merged, b"".join(p.read_bytes() for p in paths))
    require(sum(1 for _ in producer.preparation.jsonl(merged)) == 7147, "canonical denominator mismatch")
    evidence = {"merged": True, "canonical_path": str(merged), "canonical_sha256": sha256_file(merged), "official_validations": validations}
    publish(ROOT / "canonical" / "validation.json", json_bytes(evidence))
    return evidence


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("prepare")
    command.add_argument("--runtime-source-root", type=Path, required=True)
    for name in ("verify-batch", "validate-results", "merge"):
        command = commands.add_parser(name)
        command.add_argument("--bindings", type=Path, required=True)
        if name == "verify-batch":
            command.add_argument("--batch-id", required=True)
    args = parser.parse_args()
    require(Path.cwd() == producer.preparation.REPO_ROOT, "writer cwd must be repository root")
    try:
        if args.command == "prepare":
            result = prepare(args.runtime_source_root)
        else:
            result = validate(args.bindings, getattr(args, "batch_id", None), args.command == "merge")
    except Exception as error:
        failure = {"status": "blocked", "command": ["python3", "-B", str(Path(__file__).resolve()), *sys.argv[1:]],
                   "cwd": str(Path.cwd()), "error_type": type(error).__name__, "error": str(error), "merged": False}
        suffix = getattr(args, "batch_id", "all")
        publish(ROOT / "reports" / f"failure-{args.command}-{suffix}.json", json_bytes(failure))
        print(json.dumps(failure, ensure_ascii=False), file=sys.stderr)
        raise SystemExit(1)
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
