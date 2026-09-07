"""V2-specific contract checks; no external model is used by the offline suite."""
from __future__ import annotations

import copy
import io
import json
import tempfile
import unittest
from pathlib import Path

from .core import (SPEC, ContractError, change, digest, metric, percentile, question_schedule,
                   ratio, read_jsonl, require, safe_path, write_json)
from .dataset import validate_dataset
from .grading import (classify, make_bundles, no_answer_judgment, validate_batch,
                      validate_judgment)
from .metrics import (bootstrap, compare, evidence_exposure, exploration_metrics, normalize_run,
                      summarize, usage_metrics, verdict)
from .transport import Relay, source_lines, text_result, trim_result


def example_question() -> dict:
    q = {"id": "fixture", "phase": "preparation", "difficulty": "simple", "language": "typescript",
         "entrypoint": "demo.ts", "prompt": "Explain the default and the consumer.", "difficulty_rationale": "exact key",
         "source": {"pr": 1, "change_key": "one", "issue_keys": [], "code_reviewed": True,
                    "ancestor_verified": True, "url": "https://example.invalid/fixture"},
         "evidence": [{"id": "e1", "path": "demo.ts", "start_line": 1, "end_line": 1, "text": "const delay = 5000;", "text_sha256": digest("const delay = 5000;")},
                      {"id": "e2", "path": "demo.ts", "start_line": 2, "end_line": 2, "text": "setTimeout(work, delay);", "text_sha256": digest("setTimeout(work, delay);")}],
         "facts": [{"id": "f1", "requirement": "default", "correct": "5000ms", "conditions": [], "evidence_sets": [["e1"]], "wrong_claims": ["5ms"], "confusions": []},
                   {"id": "f2", "requirement": "consumer", "correct": "setTimeout", "conditions": [], "evidence_sets": [["e2"]], "wrong_claims": ["setInterval"], "confusions": ["import does not prove use"]}], "examples": []}
    def make(kind, answer, facts, errors):
        expected = {"status": "judged", "has_final_answer": True, "reason": "fixture", "major_errors": errors,
                    "facts": [{"fact_id": f"f{i + 1}", "correct": c, "supported": s, "evidence_ids": [f"e{i + 1}"] if s else [],
                               "answer_quote": quote, "explanation": "fixture"} for i, (c, s, quote) in enumerate(facts)]}
        return {"id": kind, "kind": kind, "answer": answer, "expected": expected}
    q["examples"] = [make("minimal", "5000ms and setTimeout (demo.ts:1-2).", [(True, True, "5000ms"), (True, True, "setTimeout")], []),
                     make("alternative", "Five seconds; passed to the timeout scheduler (demo.ts:1-2).", [(True, True, "Five seconds"), (True, True, "timeout scheduler")], []),
                     make("partial", "5000ms (demo.ts:1).", [(True, True, "5000ms"), (False, False, "")], []),
                     make("wrong", "It uses setInterval.", [(False, False, ""), (False, False, "setInterval")],
                          [{"answer_quote": "setInterval", "explanation": "different consumer"}])]
    return q


def fixture_usage(input_tokens=100, output_tokens=20, cached_input_tokens=30):
    usage = {"input_tokens": input_tokens, "output_tokens": output_tokens, "cached_input_tokens": cached_input_tokens,
             "total_tokens": input_tokens + output_tokens}
    return [{"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage}},
            {"type": "event_msg", "payload": {"type": "token_count", "info": {"total_token_usage": usage}}}]


def fixture_call(request_id="1", tool="read"):
    return {"id": request_id, "tool": tool, "arguments": {"file_path": "demo.ts"}, "status": "success", "effective_scope": None,
            "raw_bytes": 100, "delivered_bytes": 100, "product_truncated": False, "host_truncated": False, "result_count": None,
            "observation_step": 1, "delivered_at_seconds": 2,
            "delivered_lines": [{"path": "demo.ts", "line": 1, "text": "const delay = 5000;"},
                                {"path": "demo.ts", "line": 2, "text": "setTimeout(work, delay);"}]}


def fixture_run(q, group="A", repeat=1, status="completed"):
    calls = [fixture_call()]
    return {"id": f"{q['id']}-{group}-{repeat}", "question_id": q["id"], "group": group, "repeat": repeat, "phase": q["phase"],
            "answer": q["examples"][0]["answer"], "status": status, "conditions_valid": True, "elapsed_seconds": 2,
            "host_calls": 1, "artifact": "fixture", "calls": calls,
            "usage": usage_metrics(fixture_usage(), terminal_complete=True, evidence="fixture"),
            "exploration": exploration_metrics(calls, complete=True, evidence="fixture")}


def normalized_fixture(group="A", repeat=1, question=None):
    q = question or example_question()
    run = fixture_run(q, group, repeat)
    judgment = dict(q["examples"][0]["expected"], answer_id=run["id"], question_id=q["id"])
    return normalize_run(q, run, judgment)


class ContractChecks(unittest.TestCase):
    def test_dataset_requirements_and_source(self):
        q = example_question()
        data = {"spec_sha256": digest(SPEC), "source_commit": SPEC["source_commit"], "questions": [q]}
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "demo.ts").write_text("const delay = 5000;\nsetTimeout(work, delay);\n")
            validate_dataset(data, root, complete=False)
            q["facts"][0]["requirement"] = "outside question"
            with self.assertRaises(ContractError):
                validate_dataset(data, root, complete=False)

    def test_controls_and_references(self):
        q = example_question()
        expected = ["correct", "correct", "partial", "incorrect"]
        for e, category in zip(q["examples"], expected):
            j = dict(e["expected"], answer_id=e["id"], question_id=q["id"])
            validate_judgment(q, j, e["id"], e["answer"])
            self.assertEqual(classify(q, j), category)
        j = dict(copy.deepcopy(q["examples"][0]["expected"]), answer_id="a", question_id=q["id"])
        j["facts"][0]["evidence_ids"] = ["other-question:e1"]
        with self.assertRaises(ContractError):
            validate_judgment(q, j, "a", q["examples"][0]["answer"])

    def test_alternative_evidence_and_absent_final(self):
        q = example_question()
        q["evidence"].append({**q["evidence"][0], "id": "alternate", "path": "equivalent.ts"})
        q["facts"][0]["evidence_sets"].append(["alternate"])
        example = q["examples"][1]
        j = dict(copy.deepcopy(example["expected"]), answer_id="alternate-answer", question_id=q["id"])
        j["facts"][0]["evidence_ids"] = ["alternate"]
        validate_judgment(q, j, "alternate-answer", example["answer"])
        self.assertEqual(classify(q, j), "correct")
        absent = no_answer_judgment(q, "empty")
        validate_judgment(q, absent, "empty", "")
        self.assertEqual(classify(q, absent), "no_answer")
        j["status"] = "indeterminate"; j["reason"] = "alternative source cannot be checked"
        self.assertEqual(classify(q, j), "indeterminate")
        j["facts"][0]["evidence_ids"] = ["outside-question"]
        with self.assertRaises(ContractError):
            validate_judgment(q, j, "alternate-answer", example["answer"])

    def test_real_wrapper_delivery_and_no_print(self):
        from .runner import normalize_calls
        from .core import append_jsonl
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            write_json(folder / "runtime.json", {"started_unix_seconds": 100, "started_monotonic": 0, "elapsed_seconds": 10})
            for i in [1, 2]:
                append_jsonl(folder / "relay.jsonl", {"event": "request", "id": str(i), "tool": "read", "arguments": {"file_path": "demo.ts"},
                    "metadata": {"itemId": "outer"}, "effective_scope": None})
                append_jsonl(folder / "relay.jsonl", {"event": "result", "id": str(i), "status": "success", "raw_bytes": 100,
                    "relay_result": text_result("demo.ts:1:const delay = 5000;"), "host_truncated": False, "product_truncated": False, "result_count": None})
            append_jsonl(folder / "rollout.jsonl", {"type": "response_item", "payload": {"type": "custom_tool_call", "id": "outer", "call_id": "outer-call", "name": "exec"}})
            append_jsonl(folder / "rollout.jsonl", {"type": "response_item", "payload": {"type": "custom_tool_call_output", "call_id": "outer-call",
                "output": [{"type": "input_text", "text": json.dumps(text_result("demo.ts:1:const delay = 5000;"))}],
                "internal_chat_message_metadata_passthrough": {"create_time": 102}}})
            calls, complete, host_calls, violations = normalize_calls(folder)
            self.assertTrue(complete); self.assertEqual(host_calls, 1); self.assertFalse(violations)
            self.assertEqual(len(calls), 2)
            self.assertEqual(sum(c["delivered_bytes"] is not None for c in calls), 1)
            self.assertEqual(calls[0]["delivered_at_seconds"], 2)

    def test_model_content_field_is_not_mcp_content_array(self):
        from .runner import delivered_leaves, execution_digest
        self.assertEqual(delivered_leaves('{"file":"demo.ts","content":"demo.ts:1:answer"}'), ["demo.ts", "demo.ts:1:answer"])
        self.assertEqual(delivered_leaves('{"content":["one", "two"]}'), ["one", "two"])
        self.assertEqual(execution_digest(), execution_digest())

    def test_truncated_serialized_source_prefix(self):
        from .runner import delivered_leaves
        q = example_question()
        raw = json.dumps(text_result("demo.ts:1:const delay = 5000;\ndemo.ts:2:setTimeout(work, delay);"))
        clipped = raw[:raw.index("setTimeout") + 3]
        leaves = delivered_leaves(clipped)
        prefix = next(leaf for leaf in leaves if leaf.startswith("demo.ts:1:"))
        call = fixture_call(); call["delivered_lines"] = source_lines("read", {"file_path": "demo.ts"}, prefix)
        self.assertEqual(evidence_exposure(q, [call])["required_evidence_ratio"]["value"], .5)

    def test_read_repeat_is_independent_of_overview_scope(self):
        calls = [fixture_call("1"), fixture_call("2")]
        calls[0]["effective_scope"] = "pkg/a.go"; calls[1]["effective_scope"] = "public"
        self.assertEqual(exploration_metrics(calls, complete=True, evidence="fixture")["identical_request_repeat_ratio"]["value"], .5)

    def test_post_exit_completion_preserves_original_journal(self):
        from .runner import seal_run, verify_run_seal
        from .core import append_jsonl, file_digest
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            write_json(folder / "run.json", {"status": "token_limit"})
            write_json(folder / "runtime.json", {"started_monotonic": 100, "elapsed_seconds": 10})
            append_jsonl(folder / "relay.jsonl", {"event": "request", "id": "one"})
            seal_run(folder); original = file_digest(folder / "seal.json")
            append_jsonl(folder / "relay.jsonl", {"event": "result", "id": "one", "finished_monotonic": 110.03})
            late = verify_run_seal(folder)
            self.assertEqual(late[0]["model_delivery"], False)
            self.assertEqual(file_digest(folder / "seal.json"), original)
            self.assertEqual(verify_run_seal(folder), late)
            append_jsonl(folder / "relay.jsonl", {"event": "request", "id": "extra"})
            with self.assertRaises(ContractError):
                verify_run_seal(folder)

    def test_seal_detects_tampering(self):
        from .runner import seal_run, load_experiment_runs
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            q = example_question(); run = fixture_run(q)
            entry = {k: run[k] for k in ["id", "question_id", "group", "repeat", "phase"]}
            write_json(folder / "schedule.json", [entry])
            path = folder / "runs" / run["id"]
            write_json(path / "run.json", run); seal_run(path)
            self.assertEqual(load_experiment_runs(folder)[0]["answer"], run["answer"])
            run["answer"] = "changed after sealing"; write_json(path / "run.json", run)
            with self.assertRaises(ContractError):
                load_experiment_runs(folder)

    def test_duplicate_usage_cache_and_cumulative(self):
        rows = fixture_usage()
        result = usage_metrics(rows + rows, terminal_complete=True, evidence="fixture")
        self.assertEqual(result["total_tokens"]["value"], 120)
        self.assertEqual(result["cached_input_ratio"]["value"], .3)
        self.assertEqual(result["model_responses"]["value"], 1)
        conflict = copy.deepcopy(rows[0]); conflict["payload"]["usage"]["input_tokens"] = 101
        self.assertIsNone(usage_metrics(rows + [conflict], terminal_complete=True, evidence="fixture")["total_tokens"]["value"])
        self.assertIsNone(usage_metrics(rows[1:], terminal_complete=True, evidence="fixture")["total_tokens"]["value"])

    def test_zero_null_percentile(self):
        self.assertIsNone(ratio(0, 0)["value"])
        self.assertEqual(ratio(0, 2)["value"], 0)
        self.assertIsNone(change(0, 2)["value"])
        self.assertEqual(change(0, 2)["absolute_difference"], 2)
        self.assertAlmostEqual(percentile([0, 10], .95), 9.5)
        with self.assertRaises(ContractError):
            metric(None, "tokens")

    def test_actual_calls_loop_parallel_and_errors(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); (root / "demo.ts").write_text("const delay = 5000;\n")
            relay = Relay({"source": str(root), "group": "A", "log": str(root / "relay.jsonl")})
            for i in range(6):
                relay.call(i, {"name": "read", "arguments": {"file_path": "demo.ts"}})
            import concurrent.futures
            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
                list(executor.map(lambda i: relay.call(i, {"name": "read", "arguments": {"file_path": "absent.ts"}}), [6, 7]))
            rows = read_jsonl(root / "relay.jsonl")
            self.assertEqual(sum(r["event"] == "request" for r in rows), 8)
            self.assertEqual(sum(r.get("status") == "error" for r in rows), 2)

    def test_boundary_and_symlink(self):
        with tempfile.TemporaryDirectory() as folder, tempfile.TemporaryDirectory() as outside:
            root = Path(folder); (root / "escape").symlink_to(outside)
            for path in ["../secret", "/etc/passwd", "escape/secret", ".git/config"]:
                with self.assertRaises(ContractError):
                    safe_path(root, path, must_exist=False)
            relay = Relay({"source": str(root), "group": "A", "log": str(root / "relay.jsonl")})
            self.assertTrue(relay.call(1, {"name": "exec_command", "arguments": {"cmd": "cat secret"}})["isError"])
            relay.count = 80
            self.assertTrue(relay.call(2, {"name": "read", "arguments": {"file_path": "demo.ts"}})["isError"])
            self.assertEqual(relay.count, 80)

    def test_output_delivery_not_raw_exposure(self):
        q = example_question()
        call = fixture_call()
        result = evidence_exposure(q, [call])
        self.assertEqual(result["required_evidence_ratio"]["value"], 1)
        call["delivered_lines"] = call["delivered_lines"][:1]
        self.assertEqual(evidence_exposure(q, [call])["required_evidence_ratio"]["value"], .5)
        call["delivered_lines"] = None
        self.assertIsNone(evidence_exposure(q, [call])["required_evidence_ratio"]["value"])
        raw = text_result("가" * 300)
        delivered, truncated = trim_result(raw, 80)
        self.assertTrue(truncated)
        self.assertLessEqual(len(delivered["content"][0]["text"].encode()), 80)
        self.assertIsNone(source_lines("search", {}, "demo.ts"))

    def test_repeat_read_and_error_metrics(self):
        calls = [fixture_call("1"), fixture_call("2")]
        result = exploration_metrics(calls, complete=True, evidence="fixture")
        self.assertEqual(result["duplicate_read_ratio"]["value"], .5)
        self.assertEqual(result["identical_request_repeat_ratio"]["value"], .5)
        self.assertEqual(result["unique_read_files"]["value"], 1)
        self.assertIsNone(exploration_metrics([calls[0], calls[0]], complete=True, evidence="fixture")["exploration_calls"]["value"])

    def test_limits_no_answer_and_missing_denominator(self):
        q = example_question()
        run = fixture_run(q, status="timeout"); run["answer"] = ""
        row = normalize_run(q, run, None)
        self.assertEqual(row["category"], "no_answer")
        self.assertEqual(row["cost"]["total_tokens"]["value"], 120)
        self.assertEqual(row["quality"]["full_correct"]["value"], 0)
        other = copy.deepcopy(row); other["repeat"] = 2; other["cost"]["total_tokens"] = metric(None, "tokens", reason="missing")
        result = summarize([row, other], 2)
        self.assertEqual(result["scheduled_runs"], 2)
        self.assertIsNone(result["metrics"]["total_tokens"]["sum"]["value"])
        self.assertEqual(result["metrics"]["total_tokens"]["observed_partial_sum"], 120)
        run["status"] = "environment_error"
        self.assertIsNone(normalize_run(q, run, None)["quality"]["full_correct"]["value"])

    def test_ratio_of_means_and_cluster_bootstrap(self):
        rows = [normalized_fixture(group, repeat) for group in ["A", "B"] for repeat in [1, 2]]
        values = [10, 1000, 20, 1100]
        for row, value in zip(rows, values):
            row["cost"]["total_tokens"] = metric(value, "tokens")
        result = compare(rows, 2, with_bootstrap=False)
        self.assertAlmostEqual(result["differences"]["total_tokens"]["value"], 100 * (560 / 505 - 1))
        first = bootstrap(rows, 100)
        self.assertEqual(first, bootstrap(rows, 100))
        self.assertEqual(first["intervals"]["full_correct"]["low"], 0)

    def test_both_correct_auxiliary_keeps_question_weights(self):
        rows = []
        for question_id, costs in [("one", (10, 20)), ("two", (100, 100))]:
            q = example_question(); q["id"] = question_id
            for group, cost in zip(["A", "B"], costs):
                for repeat in [1, 2]:
                    row = normalized_fixture(group, repeat, q)
                    row["cost"]["total_tokens"] = metric(cost, "tokens")
                    if question_id == "two" and group == "B" and repeat == 2:
                        row["quality"]["full_correct"] = metric(0, "ratio")
                    rows.append(row)
        auxiliary = compare(rows, 2, with_bootstrap=False)["both_correct_cost"]
        self.assertEqual((auxiliary["pairs"], auxiliary["questions"]), (3, 2))
        self.assertEqual(auxiliary["metrics"]["total_tokens"]["A_mean"], 55)
        self.assertEqual(auxiliary["metrics"]["total_tokens"]["B_mean"], 60)

    def test_cost_bootstrap_does_not_depend_on_quality_availability(self):
        rows = []
        for i in range(1, 11):
            q = example_question(); q["id"] = f"question-{i}"
            for group in ["A", "B"]:
                row = normalized_fixture(group, 1, q)
                row["cost"]["total_tokens"] = metric(i if group == "A" else i * i, "tokens")
                rows.append(row)
        expected = bootstrap(rows, 100)["intervals"]["total_tokens"]
        rows[0]["quality"]["full_correct"] = metric(None, "ratio", reason="missing grade")
        self.assertEqual(bootstrap(rows, 100)["intervals"]["total_tokens"], expected)

    def test_25_percent_boundary_before_rounding(self):
        comparison = {"differences": {"full_correct": metric(5, "pp"), "total_tokens": metric(25, "percent"),
                                       "exploration_calls": metric(-10, "percent")},
                      "bootstrap": {"intervals": {"full_correct": {"low": 1, "high": 9}}}}
        self.assertIn("비허용", verdict(comparison, True)["decision"])
        comparison["differences"]["total_tokens"]["value"] = 24.9999999
        self.assertNotIn("비허용", verdict(comparison, True)["decision"])
        self.assertIn("보류", verdict(comparison, False)["decision"])

    def test_schedule_reverse_and_blind_batches(self):
        q = example_question(); q["phase"] = "main"
        data = {"questions": [q]}
        schedule = question_schedule(data, "main")
        self.assertEqual([r["group"] for r in schedule[:2]], [r["group"] for r in schedule[2:]][::-1])
        runs = [fixture_run(q, r["group"], r["repeat"]) for r in schedule]
        bundles, registry = make_bundles(data, runs)
        rendered = json.dumps(bundles)
        self.assertNotIn('"group"', rendered); self.assertNotIn('"total_tokens"', rendered)
        self.assertEqual(sum(not r["control"] for r in registry.values()), 4)
        self.assertEqual(make_bundles(data, runs), (bundles, registry))
        with self.assertRaises(ContractError):
            validate_batch(data, bundles[0], registry, {"judgments": []})

    def test_five_tables_end_to_end_and_reaggregation(self):
        from .reporting import create_summary, render_report
        questions, runs, judgments = [], [], {}
        for difficulty in ["simple", "medium", "complex"]:
            for i in range(2):
                q = copy.deepcopy(example_question()); q["id"] = f"{difficulty}-{i}"; q["difficulty"] = difficulty
                questions.append(q)
                for group in ["A", "B"]:
                    run = fixture_run(q, group); runs.append(run)
                    judgments[run["id"]] = dict(q["examples"][0]["expected"], answer_id=run["id"], question_id=q["id"])
        grading = {"judgments": judgments, "batches": [{"status": "complete"}]}
        summary = create_summary({"questions": questions}, runs, grading, "preparation", frozen={"preparation": {}})
        self.assertTrue(summary["harness"]["ready_for_main"])
        self.assertEqual(summary, create_summary({"questions": questions}, runs, grading, "preparation", frozen={"preparation": {}}))
        report = render_report(summary)
        for i in range(1, 6):
            self.assertIn(f"## {i}.", report)

    def test_unknown_question_cost_does_not_become_total_share(self):
        from .reporting import create_summary
        questions, runs, judgments = [], [], {}
        for i in range(2):
            q = example_question(); q["id"] = f"question-{i}"; questions.append(q)
            for group in ["A", "B"]:
                run = fixture_run(q, group)
                if group == "B":
                    run["usage"] = usage_metrics(fixture_usage(200, 20), terminal_complete=True, evidence="fixture")
                if i == 1 and group == "A":
                    run["usage"]["total_tokens"] = metric(None, "tokens", reason="unfinished request")
                runs.append(run)
                judgments[run["id"]] = dict(q["examples"][0]["expected"], answer_id=run["id"], question_id=q["id"])
        summary = create_summary({"questions": questions}, runs, {"judgments": judgments, "batches": []}, "preparation", frozen={"preparation": {}})
        increase = summary["cost_increases"][0]
        self.assertIsNone(increase["positive_additional_share"])
        self.assertEqual(increase["observed_positive_additional_share"], 1)
        self.assertEqual(summary["cost_increase_coverage"]["known_questions"], 1)


def verify_offline() -> dict:
    output = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ContractChecks)
    result = unittest.TextTestRunner(stream=output, verbosity=2).run(suite)
    return {"passed": result.wasSuccessful(), "checks": result.testsRun,
            "failures": len(result.failures), "errors": len(result.errors), "output": output.getvalue()}


def verify_runtime(output: Path, binary: Path) -> dict:
    from .runner import execute_codex, normalize_calls, output_blocks
    output.mkdir(parents=True, exist_ok=False)
    results = []
    for group in ["A", "B"]:
        folder = output / group
        source = folder / "fixture"; source.mkdir(parents=True)
        (source / ".git").mkdir()
        (source / "demo.ts").write_text("const delay = 5000;\nsetTimeout(work, delay);\n")
        settings = {"group": group, "source": str(source.resolve()), "log": str((folder / "relay.jsonl").resolve()),
                    "product_binary": str(binary.resolve()), "product_home": str((folder / "product-home").resolve())}
        prompt = ("Runtime verification: using exec, print ALL_TOOLS.map(x=>x.name). "
                  "Print exactly this object: text({fetch:typeof fetch,require:typeof require,process:typeof process}); "
                  "Use navigation's read tool to read demo.ts and print the full result. "
                  "Try its read with ../outside-secret and /etc/passwd; both must return errors. "
                  "Do not use any other code access path. Then answer the exact delay and consumer with source lines.")
        execution = execute_codex(folder, prompt, settings)
        results.append(inspect_runtime_group(folder, group))
    return {"passed": all(r["passed"] for r in results), "groups": results}


def inspect_runtime_group(folder: Path, group: str) -> dict:
    from .runner import normalize_calls, output_blocks, codex_options
    from .core import read_json
    execution = read_json(folder / "execution.json")
    calls, complete, host_calls, violations = normalize_calls(folder)
    inventories = []
    globals_safe = False
    for row in read_jsonl(folder / "rollout.jsonl"):
        payload = row.get("payload", {})
        if payload.get("type") not in {"custom_tool_call_output", "function_call_output"}:
            continue
        for block in output_blocks(payload):
            try:
                data = json.loads(block)
            except ValueError:
                continue
            # The requested ALL_TOOLS.map emits strings. A later filtered metadata
            # array is a lookup, not a second (incomplete) global inventory.
            if isinstance(data, list) and data and all(isinstance(name, str) for name in data):
                if any(name.startswith("mcp__") for name in data):
                    inventories.append(set(data))
            if data == {"fetch": "undefined", "require": "undefined", "process": "undefined"}:
                globals_safe = True
    expected_inventory = {"mcp__navigation__" + name for name in SPEC["groups"][group]}
    inventory_valid = bool(inventories) and all(inventory == expected_inventory for inventory in inventories)
    args = read_json(folder / "command.json")["args"]
    expected_options = codex_options()
    options_valid = all(value in args for value in expected_options[1::2])
    result = {"group": group, "execution": execution, "calls_complete": complete, "host_calls": host_calls,
              "violations": violations, "calls": calls, "inventory_valid": inventory_valid, "globals_safe": globals_safe,
              "options_valid": options_valid}
    result["passed"] = execution["status"] == "completed" and execution["conditions_valid"] and execution["usage"]["complete"] and complete and not violations and inventory_valid and globals_safe and options_valid and any(c["status"] == "success" for c in calls) and sum(c["status"] == "error" for c in calls) >= 2
    return result


def reinspect_runtime(root: Path) -> dict:
    groups = [inspect_runtime_group(root / group, group) for group in ["A", "B"]]
    return {"passed": all(group["passed"] for group in groups), "groups": groups, "reinspected": True}
