"""Measure the dictionary overlay on pinned codebases without running model agents.

Requires a branch binary built with --features related-terms-eval. The private
related_terms/evaluate method switches measurement modes without reindexing.
Shared checkouts are read-only: config/index/reports live under --output-dir.
--clone creates fresh pinned checkouts in an owned directory (Docker uses /work).
Only the separate branch-check clone fetches an earlier tag or changes revision.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import platform
import random
import re
import subprocess
import sys
import tempfile
import time
import traceback

from mcp_tool_eval import McpSession

HERE = Path(__file__).resolve().parent
POC = HERE.parent
REPO = HERE.parents[4]
sys.path.insert(0, str(POC))
from term_canonical import word_key

SCOPES = ("symbols", "docstrings", "literals")
MODES = ("off", "demote", "remove")
SEED = 20261017


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def git(path, *args):
    try:
        return subprocess.check_output(["git", "-C", str(path), *args], text=True,
                                       stderr=subprocess.PIPE, timeout=300).strip()
    except subprocess.CalledProcessError as error:
        raise RuntimeError(f"git {' '.join(args)}: {error.stderr.strip()}") from error


def read_pins():
    text = (REPO / "docs/related-terms/sources.md").read_text()
    rows = re.findall(r"^\| ([a-z][a-z0-9-]+) \| (https://[^ ]+) \| `([^`]+)` \| `([a-f0-9]{40})`", text, re.MULTILINE)
    if len(rows) != 12:
        raise ValueError(f"expected 12 pinned repositories, found {len(rows)}")
    pins = {name: {"url": url, "tag": tag, "commit": commit} for name, url, tag, commit in rows}
    if len(pins) != 12:
        raise ValueError("repository names must be unique")
    return pins


def clone_repositories(arguments, pins):
    shared = (REPO / ".codemap/poc/related-terms/workspaces/checkouts").resolve()
    if arguments.checkouts == shared:
        raise ValueError("--clone must not write to the shared PoC checkouts")
    arguments.checkouts.mkdir(parents=True, exist_ok=True)
    logs = arguments.output_dir / "clone-logs"
    logs.mkdir(exist_ok=True)
    for name, pin in pins.items():
        checkout = arguments.checkouts / name
        if checkout.exists():
            if not arguments.should_resume:
                raise ValueError(f"{checkout} already exists; use a fresh directory or --resume")
            continue
        print(f"clone {name}: {pin['tag']} ({pin['commit']})", flush=True)
        with tempfile.TemporaryDirectory(prefix=f".{name}-", dir=arguments.checkouts) as temporary:
            staged = Path(temporary) / "checkout"
            with (logs / f"{name}.log").open("w") as log:
                subprocess.run(["git", "clone", "--depth=1", "--single-branch", "--branch", pin["tag"],
                                pin["url"], str(staged)], check=True, stdout=log, stderr=log, timeout=600)
            actual = git(staged, "rev-parse", "HEAD")
            if actual != pin["commit"]:
                raise ValueError(f"{name}: tag resolved to {actual}, expected {pin['commit']}")
            staged.rename(checkout)


def preflight(checkouts, pins):
    for name, pin in pins.items():
        repo = checkouts / name
        if (git(repo, "rev-parse", "HEAD") != pin["commit"]
                or git(repo, "rev-parse", f"refs/tags/{pin['tag']}^{{commit}}") != pin["commit"]
                or git(repo, "diff", "--name-only", "HEAD")):
            raise ValueError(f"{name}: pinned commit or tracked working tree differs")
        if (repo / ".codemap/config.toml").exists():
            raise ValueError(f"{name}: repo config overrides measurement settings; use a scratch checkout")


def output_words(text):
    words = set()
    for line in text.splitlines():
        if not line.startswith(("Direct (", "Near (", "Same topic (", "Direct without a sense (")):
            continue
        match = re.search(r"\(showing [0-9]+ of [0-9]+(?:; [^)]*)?\): (.*)$", line)
        if match:
            words.update(word_key(word) for word in json.loads("[" + match.group(1) + "]"))
    return sorted(words)


def measure(client, item):
    result = client.lookup_result([item["word"]], should_wait_for_evidence=True)
    text = "\n".join(part["text"] for part in result["content"] if part["type"] == "text")
    metadata = result["_meta"]["related_terms_eval"]
    measured = metadata["answers"][0]
    if measured["input"] != item["word"]:
        raise ValueError("MCP input/result mismatch")
    before = measured["before"]
    after = measured["after"]
    if Counter(after["fields"]) - Counter(before["fields"]):
        raise ValueError("overlay added a sense field")
    if set(after["word_keys"]) - set(before["word_keys"]):
        raise ValueError("overlay added a dictionary word")
    if metadata["is_overlay_enabled"] and not metadata["should_remove"]:
        if Counter(before["fields"]) != Counter(after["fields"]):
            raise ValueError("demotion removed a sense")
    capped_fields = re.findall(r"^### .* \[([^\]]+)\] — ", text, re.MULTILINE)
    expected = set(item["expected_fields"] or [])
    return {"id": item["id"], "word": item["word"], "group": item["group"],
            "repository": item["repository"], "expected_fields": item["expected_fields"],
            "before_fields": before["fields"], "after_fields": after["fields"], "capped_fields": capped_fields,
            "before_hit": bool(expected.intersection(before["fields"])),
            "after_hit": bool(expected.intersection(after["fields"])),
            "capped_hit": bool(expected.intersection(capped_fields)),
            "before_word_keys": before["word_keys"], "after_word_keys": after["word_keys"],
            "capped_word_keys": output_words(text), "decision": measured["decision"],
            "generation": metadata["evidence_generation"], "output_bytes": len(text.encode()), "text": text}


def evidence_report(path, scope, generation):
    evidence = json.loads(path.read_text())
    if evidence["scope"] != scope or evidence["generation"] != generation:
        raise ValueError("evidence artifact and MCP response refer to different snapshots")
    return evidence


def ratio(numerator, denominator):
    return numerator / denominator if denominator else None


def metrics(rows, evidence):
    judged = [row for row in rows if row["expected_fields"]]
    keys = evidence["words"]
    returned = sum(len(row["capped_word_keys"]) for row in rows)
    found = sum(sum(key in keys for key in row["capped_word_keys"]) for row in rows)
    uncapped_returned = sum(len(row["after_word_keys"]) for row in rows)
    uncapped_found = sum(sum(key in keys for key in row["after_word_keys"]) for row in rows)
    return {"inputs": len(rows), "expected_inputs": len(judged),
            "lookup_hits": sum(row["before_hit"] for row in judged),
            "uncapped_hits": sum(row["after_hit"] for row in judged),
            "capped_hits": sum(row["capped_hit"] for row in judged),
            "lost_uncapped": [row["word"] for row in judged if row["before_hit"] and not row["after_hit"]],
            "lost_after_caps": [row["word"] for row in judged if row["before_hit"] and not row["capped_hit"]],
            "found_words": found, "returned_words": returned, "found_share": ratio(found, returned),
            "uncapped_found_share": ratio(uncapped_found, uncapped_returned),
            "output_bytes": sum(row["output_bytes"] for row in rows),
            "removed_senses": sum(row["decision"]["removed_sense_count"] for row in rows)}


def removal_sample(rows, items):
    context = {item["id"]: item for item in items}
    strata = defaultdict(list)
    for row in rows:
        for removed in row["decision"]["removed_senses"]:
            item = context[row["id"]]
            strata[(removed["field"], item["group"])].append({
                "input_id": item["id"], "word": item["word"], "repository": item["repository"],
                "file": item["file"], "line": item["line"], "code": item["code"],
                "expected_fields": item["expected_fields"], **removed,
                "judgment_a": None, "judgment_b": None, "tie_breaker": None,
            })
    randomizer = random.Random(SEED)
    for bucket in strata.values():
        randomizer.shuffle(bucket)
    strata_keys = sorted(strata)
    randomizer.shuffle(strata_keys)
    sample = []
    while len(sample) < 20 and any(strata.values()):
        for key in strata_keys:
            if strata[key] and len(sample) < 20:
                sample.append(strata[key].pop())
    return sample


def evaluate_repository(name, pin, items, control_items, arguments, fingerprint):
    output = arguments.output_dir / "repositories" / name
    report_path = output / "report.json"
    if arguments.should_resume and report_path.exists():
        previous = json.loads(report_path.read_text())
        if previous.get("fingerprint") == fingerprint and previous.get("commit") == pin["commit"]:
            print(f"reuse {name}: 3 scopes", flush=True)
            return previous
    output.mkdir(parents=True, exist_ok=True)
    latest = output / "evidence-latest.json"
    print(f"index {name}: waiting for a published snapshot and current evidence", flush=True)
    client = McpSession(arguments.binary, arguments.dataset, output, cwd=arguments.checkouts / name,
                        home=arguments.output_dir / "shared-home",
                        evidence_timeout_seconds=arguments.evidence_timeout_seconds,
                        env_overrides={"CODEMAP_RELATED_TERMS_EVAL_REPORT": str(latest)})
    scopes = {}
    started = time.monotonic()
    try:
        client.initialize()
        client.configure("symbols", "off")
        initial = capture_evidence(client, latest)
        print(f"indexed {name}: files={initial['file_count']} index={initial['index_pass_ms']:.1f}ms", flush=True)
        for scope in SCOPES:
            scope_output = output / scope
            scope_output.mkdir(exist_ok=True)
            modes = {}
            for mode in MODES:
                client.configure(scope, mode)
                rows = [measure(client, item) for item in items]
                evidence = evidence_report(latest, scope, rows[-1]["generation"])
                (scope_output / f"{mode}.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1))
                modes[mode] = metrics(rows, evidence)
                if mode == "remove":
                    controls = [measure(client, item) for item in control_items]
                    (scope_output / "control.json").write_text(json.dumps(controls, ensure_ascii=False, indent=1))
                    sample = removal_sample(rows, items)
                    (scope_output / "removal-sample.json").write_text(json.dumps(sample, ensure_ascii=False, indent=1))
            (scope_output / "evidence.json").write_text(json.dumps(evidence, ensure_ascii=False))
            scopes[scope] = {"modes": modes, "file_count": evidence["file_count"],
                             "evidence_keys": len(evidence["words"]), "evidence_ms": evidence["elapsed_ms"],
                             "index_pass_ms": evidence["index_pass_ms"],
                             "removal_sample_size": len(sample), "resolved_judgments": 0,
                             "wrong_removal_rate": None, "wrong_removal_upper_bound": None,
                             "model_evaluation": "deferred by user"}
            print(f"{name} {scope}: expected={modes['off']['expected_inputs']} "
                  f"caps off/demote/remove={modes['off']['capped_hits']}/{modes['demote']['capped_hits']}/{modes['remove']['capped_hits']} "
                  f"evidence={evidence['elapsed_ms']:.1f}ms index={evidence['index_pass_ms']:.1f}ms", flush=True)
    finally:
        client.close()
    report = {"repository": name, "commit": pin["commit"], "fingerprint": fingerprint,
              "elapsed_seconds": time.monotonic() - started, "scopes": scopes}
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    return report


def cross_repository_controls(arguments, names):
    result = {}
    for scope in SCOPES:
        applicable = skipped = expected_count = own_hits = foreign_hits = 0
        own_found = foreign_found = own_words = foreign_words = 0
        for index, name in enumerate(names):
            foreign = names[(index + 1) % len(names)]
            own_dir = arguments.output_dir / "repositories" / name / scope
            foreign_dir = arguments.output_dir / "repositories" / foreign / scope
            if not own_dir.exists() or not foreign_dir.exists():
                continue
            own_rows = json.loads((own_dir / "remove.json").read_text())
            controls = {row["id"]: row for row in json.loads((foreign_dir / "control.json").read_text())}
            evidence = json.loads((own_dir / "evidence.json").read_text())["words"]
            for own in own_rows:
                other = controls[own["id"]]
                if not all(row["decision"]["is_input_present"] and row["decision"]["is_applied"] for row in [own, other]):
                    skipped += 1
                    continue
                applicable += 1
                if own["expected_fields"]:
                    expected_count += 1
                    own_hits += own["capped_hit"]
                    foreign_hits += other["capped_hit"]
                own_words += len(own["capped_word_keys"])
                foreign_words += len(other["capped_word_keys"])
                own_found += sum(key in evidence for key in own["capped_word_keys"])
                foreign_found += sum(key in evidence for key in other["capped_word_keys"])
        result[scope] = {"applicable": applicable, "skipped": skipped, "expected_cases": expected_count,
                         "own_capped_hits": own_hits, "foreign_capped_hits": foreign_hits,
                         "own_words_found_in_source_share": ratio(own_found, own_words),
                         "foreign_words_found_in_source_share": ratio(foreign_found, foreign_words),
                         "pairing": "next repository in sorted pinned list"}
    return result


def capture_evidence(client, path):
    response = client.lookup_result(["token"], should_wait_for_evidence=True)
    metadata = response["_meta"]["related_terms_eval"]
    return evidence_report(path, "symbols", metadata["evidence_generation"])


def verify_branch(arguments, pins, fingerprint):
    output = arguments.output_dir / "branch"
    if arguments.should_resume and (output / "report.json").exists():
        report = json.loads((output / "report.json").read_text())
        if report.get("fingerprint") == fingerprint:
            return report
    checkout = output / "checkout"
    output.mkdir(parents=True, exist_ok=True)
    pin = pins["express"]
    older = "4.21.2"
    older_commit = "1faf228935aa0a13111f92c28ee795be64ce3f0f"
    if not checkout.exists():
        subprocess.run(["git", "clone", "--shared", "--no-hardlinks", str(arguments.checkouts / "express"), str(checkout)],
                       check=True, capture_output=True)
    # Fetch only into this owned scratch clone, never into the shared pinned checkout.
    git(checkout, "fetch", "--depth=1", pin["url"], f"refs/tags/{older}:refs/tags/{older}")
    if git(checkout, "rev-parse", f"refs/tags/{older}^{{commit}}") != older_commit:
        raise ValueError("earlier Express tag no longer matches its pinned commit")
    git(checkout, "checkout", "--detach", pin["commit"])
    live = output / "incremental"
    latest = live / "evidence-latest.json"
    client = McpSession(arguments.binary, arguments.dataset, live, cwd=checkout,
                        home=arguments.output_dir / "shared-home", is_watch_enabled=True,
                        evidence_timeout_seconds=arguments.evidence_timeout_seconds,
                        env_overrides={"CODEMAP_RELATED_TERMS_EVAL_REPORT": str(latest)})
    try:
        client.initialize(); client.configure("symbols", "demote")
        before = capture_evidence(client, latest)
        git(checkout, "checkout", "--detach", older)
        fresh_dir = output / "fresh"
        fresh_path = fresh_dir / "evidence-latest.json"
        # Separate CODEMAP_HOME: changing shared config would redirect the live indexer.
        fresh_client = McpSession(arguments.binary, arguments.dataset, fresh_dir, cwd=checkout,
                                  evidence_timeout_seconds=arguments.evidence_timeout_seconds,
                                  env_overrides={"CODEMAP_RELATED_TERMS_EVAL_REPORT": str(fresh_path)})
        try:
            fresh_client.initialize(); fresh_client.configure("symbols", "demote")
            fresh = capture_evidence(fresh_client, fresh_path)
        finally:
            fresh_client.close()
        if before["words"] == fresh["words"]:
            raise ValueError("branch revisions did not produce different dictionary evidence")
        deadline = time.monotonic() + 120
        while True:
            after = capture_evidence(client, latest)
            if (after["generation"] > before["generation"]
                    and all(after[key] == fresh[key] for key in ["words", "field_word_counts", "file_count"])):
                break
            if time.monotonic() > deadline:
                raise ValueError("incremental branch evidence did not converge to the fresh map")
            time.sleep(0.25)
    finally:
        client.close()
    (output / "before.json").write_text(json.dumps(before))
    (output / "incremental.json").write_text(json.dumps(after))
    (output / "fresh.json").write_text(json.dumps(fresh))
    report = {"fingerprint": fingerprint, "repository": "express", "first_tag": pin["tag"], "first_commit": pin["commit"],
              "second_tag": older, "second_commit": git(checkout, "rev-parse", "HEAD"),
              "maps_differ": before["words"] != after["words"], "incremental_equals_fresh": True,
              "first_key_count": len(before["words"]), "second_key_count": len(after["words"]),
              "changed_file_count": after["changed_file_count"], "removed_file_count": after["removed_file_count"]}
    (output / "report.json").write_text(json.dumps(report, indent=2))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--dataset", type=Path, default=POC / "datasets/rts5-75e02862daa29308/dataset.sqlite3")
    parser.add_argument("--checkouts", type=Path, default=REPO / ".codemap/poc/related-terms/workspaces/checkouts")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--clone", dest="should_clone", action="store_true", help="Clone all 12 pins into an owned checkout directory")
    parser.add_argument("--evidence-timeout-seconds", type=int, default=900)
    parser.add_argument("--repositories", nargs="*", help="Optional bounded smoke subset, not global acceptance")
    parser.add_argument("--resume", dest="should_resume", action="store_true", help="Reuse only matching completed repository reports")
    parser.add_argument("--skip-branch", dest="should_skip_branch", action="store_true", help="Leave the branch gate explicitly unverified")
    arguments = parser.parse_args()
    for field in ["binary", "dataset", "checkouts", "output_dir"]:
        setattr(arguments, field, getattr(arguments, field).resolve())
    arguments.output_dir.mkdir(parents=True, exist_ok=True)
    pins = read_pins(); names = sorted(pins)
    selected = arguments.repositories or names
    if set(selected) - set(names):
        raise ValueError("unknown repository selection")
    if arguments.evidence_timeout_seconds <= 0:
        raise ValueError("evidence timeout must be positive")
    if arguments.should_clone:
        clone_repositories(arguments, pins)
    preflight(arguments.checkouts, pins)
    items = json.loads((HERE / "eval-set.json").read_text())["items"]
    grouped = {name: [item for item in items if item["repository"] == name] for name in names}
    if any(len(grouped[name]) != 20 for name in names):
        raise ValueError("expected 20 evaluation inputs per repository")
    dataset_sha256 = digest(arguments.dataset)
    identity = {"binary": digest(arguments.binary), "dataset": dataset_sha256,
                "eval_set": digest(HERE / "eval-set.json"), "script": digest(Path(__file__)),
                "client": digest(HERE / "mcp_tool_eval.py"), "seed": SEED, "pins": pins,
                "evidence_timeout_seconds": arguments.evidence_timeout_seconds}
    fingerprint = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    reports = []
    failures = {}
    for name in selected:
        previous = names[(names.index(name) - 1) % len(names)]
        try:
            reports.append(evaluate_repository(name, pins[name], grouped[name], grouped[previous], arguments, fingerprint))
        except Exception as error:
            failures[name] = {"type": type(error).__name__, "message": str(error)}
            (arguments.output_dir / "failures.json").write_text(json.dumps(failures, ensure_ascii=False, indent=2))
            traceback.print_exc()
    controls = cross_repository_controls(arguments, names) if not failures and set(selected) == set(names) else None
    branch = None
    if not arguments.should_skip_branch:
        try:
            branch = verify_branch(arguments, pins, fingerprint)
        except Exception as error:
            failures["branch"] = {"type": type(error).__name__, "message": str(error)}
            traceback.print_exc()
    preflight(arguments.checkouts, pins)
    if digest(arguments.dataset) != dataset_sha256:
        raise ValueError("tracked source dataset changed")
    result = {"fingerprint": fingerprint, "identity": identity, "repositories": reports,
              "scope_rows": sum(len(report["scopes"]) for report in reports),
              "cross_repository": controls, "branch": branch, "source_unchanged": True,
              "shared_checkouts_unchanged": True, "independent_judgments": "deferred by user",
              "shipped_removal": False, "failures": failures,
              "environment": {"docker": Path("/.dockerenv").exists(), "system": platform.system(),
                              "architecture": platform.machine(), "python": platform.python_version(),
                              "event_navigation": "default (enabled)"}}
    (arguments.output_dir / "report.json").write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps({"repositories": len(reports), "scope_rows": result["scope_rows"],
                      "branch": branch, "failures": failures,
                      "report": str(arguments.output_dir / "report.json")}, ensure_ascii=False))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
