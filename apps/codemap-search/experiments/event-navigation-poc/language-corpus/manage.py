#!/usr/bin/env python3
"""Prepare, freeze, verify, measure and evaluate the programming-language corpus."""

import argparse
from collections import Counter, defaultdict
import gzip
import hashlib
import importlib.metadata
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

DIRECTORY = Path(__file__).resolve().parent
PACKAGES = ["tree-sitter", "tree-sitter-typescript", "tree-sitter-rust", "tree-sitter-go",
            "tree-sitter-language-pack", "tree-sitter-c-sharp", "tree-sitter-embedded-template", "tree-sitter-yaml"]


def read(name):
    return json.loads((DIRECTORY / name).read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def git(root, *arguments):
    return subprocess.check_output(["git", "-C", str(root), *arguments], text=True).strip()


def prepare(sources, selected):
    sources.mkdir(parents=True, exist_ok=True)
    for name, spec in read("repositories.json").items():
        if selected and name not in selected:
            continue
        target = sources / name
        if target.exists():
            if git(target, "rev-parse", "HEAD") != spec["revision"]:
                raise ValueError("Refusing to replace existing checkout: " + str(target))
        else:
            with tempfile.TemporaryDirectory(prefix="event-language-fetch-", dir=sources) as directory:
                checkout = Path(directory) / "checkout"
                subprocess.run(["git", "init", "--quiet", str(checkout)], check=True)
                git(checkout, "remote", "add", "origin", spec["url"])
                git(checkout, "fetch", "--filter=blob:none", "--depth=1", "--quiet", "origin", spec["revision"])
                # Exact source files, including evidence, make preparation
                # independent of the original exploratory sparse checkout.
                git(checkout, "sparse-checkout", "init", "--no-cone")
                git(checkout, "sparse-checkout", "set", "--no-cone", *["/" + path for path in spec["source_paths"]])
                git(checkout, "checkout", "--quiet", "--detach", "FETCH_HEAD")
                checkout.rename(target)
        print(name + ": " + spec["revision"], flush=True)


def inventory(sources, is_fixtures_only=False):
    repositories = read("repositories.json")
    fixtures = read("fixture-cases.json")["cases"]
    cases = fixtures if is_fixtures_only else read("cases.json")["cases"] + fixtures
    languages = read("languages.json")["languages"]
    result = {"repositories": {}, "fixtures": {}, "cases": {}, "languages": {}}
    seen = set()
    for case in cases:
        if case["id"] in seen:
            raise ValueError("Duplicate case id: " + case["id"])
        seen.add(case["id"])
        if case["language"] not in languages:
            raise ValueError("Out-of-scope language: " + case["language"])
        if case["mode"] == "endpoint_pair" and case["kind"] == "boundary":
            raise ValueError("A scenario cannot be scored as an endpoint pair")
        name = case["repository"]
        root = DIRECTORY if name == "fixtures" else sources / name
        if name != "fixtures" and name not in result["repositories"]:
            spec = repositories[name]
            if git(root, "rev-parse", "HEAD") != spec["revision"]:
                raise ValueError(name + ": revision mismatch")
            if git(root, "status", "--porcelain", "--untracked-files=no"):
                raise ValueError(name + ": tracked sources have changes")
            result["repositories"][name] = {"revision": spec["revision"], "source_sha256": {}}
            for relative in spec["source_paths"]:
                path = root / relative
                if not path.resolve().is_relative_to(root.resolve()) or path.is_symlink() or not path.is_file():
                    raise ValueError(name + ": invalid source path " + relative)
                result["repositories"][name]["source_sha256"][relative] = digest(path)
        endpoints = []
        for role, endpoint in [("storage", case["storage"]), ("invocation", case["invocation"]), *[("evidence", item) for item in case.get("evidence", [])]]:
            relative, start, end = endpoint
            path = root / relative
            if not path.resolve().is_relative_to(root.resolve()) or path.is_symlink():
                raise ValueError("Unsafe endpoint: " + relative)
            data = path.read_bytes()
            lines = data.splitlines(keepends=True)
            if not 1 <= start <= end <= len(lines):
                raise ValueError(f"{case['id']}: invalid lines {start}-{end} in {relative}")
            value = {"role": role, "path": relative, "start": start, "end": end,
                     "sha256": hashlib.sha256(b"".join(lines[start - 1:end])).hexdigest()}
            if name != "fixtures":
                spec = repositories[name]
                value["url"] = spec["url"].removesuffix(".git") + "/blob/" + spec["revision"] + "/" + relative + f"#L{start}-L{end}"
            else:
                result["fixtures"][relative] = hashlib.sha256(data).hexdigest()
            endpoints.append(value)
        result["cases"][case["id"]] = endpoints
    for language in languages:
        selected = [case for case in cases if case["language"] == language]
        counts = Counter(case["kind"] for case in selected)
        if counts["positive"] < 1 or counts["negative"] < 1:
            raise ValueError(language + ": positive or negative coverage missing")
        result["languages"][language] = dict(counts)
    result["manifest_sha256"] = {name: digest(DIRECTORY / name) for name in ["languages.json", "repositories.json", "cases.json", "fixture-cases.json", "requirements.txt"]}
    return result


def matches(endpoint, location):
    return endpoint[0] == location["path"] and endpoint[1] <= location["line"] <= endpoint[2]


def evaluate(cases, outputs):
    rows = []
    for case in cases:
        result = outputs[(case["repository"], case["language"])]
        source_hashes = result["scope"]["source_sha256"]
        is_complete = all(case[key][0] in source_hashes for key in ("storage", "invocation"))
        relations = [relation for relation in result["relations"]
                     if matches(case["storage"], relation["storage"]["location"])
                     and matches(case["invocation"], relation["invocation"]["location"])]
        calls = [relation for relation in relations if relation["kind"] != "stored_value_argument"]
        if not is_complete:
            status = "input_unavailable"
        elif case["mode"] != "endpoint_pair":
            status = "source_scenario_not_executed"
        elif case["kind"] == "negative":
            status = "incorrect_connection" if relations else "negative_pending_positive_control"
        else:
            status = "conditional_candidate" if calls else ("argument_transfer_only" if relations else "unresolved")
        rows.append({**case, "status": status, "matching_relations": len(relations),
                     "evidence_matches": [{"kind": row["kind"], "storage": row["storage"]["location"],
                                           "invocation": row["invocation"]["location"], "conditions": row["conditions"]}
                                          for row in relations[:3]]})
    for row in rows:
        if row["status"] == "negative_pending_positive_control":
            controls = [control for control in rows if control["language"] == row["language"]
                        and control["repository"] == row["repository"] and control["kind"] == "positive"
                        and control["status"] == "conditional_candidate"]
            if controls:
                row["status"] = "correctly_unjoined_with_positive_control"
                row["observed_positive_controls"] = [control["id"] for control in controls]
            else:
                row["status"] = "negative_unverified_without_positive_control"
    counts = defaultdict(lambda: {"fixtures": Counter(), "public": Counter()})
    for row in rows:
        counts[row["language"]]["fixtures" if row["repository"] == "fixtures" else "public"][row["status"]] += 1
    return {"unit": "source endpoint pair or explicitly unexecuted source scenario", "target_program_executed": False,
            "scope": "selected examples; not repository-wide precision or recall", "languages": counts, "cases": rows}


def measure(options):
    observed = inventory(options.sources, options.fixtures)
    if (DIRECTORY / "sources.lock.json").exists():
        locked = read("sources.lock.json")
        if (observed["fixtures"] != locked["fixtures"] if options.fixtures else observed != locked):
            raise ValueError("Corpus or source bytes differ from sources.lock.json")
    fixture_cases = read("fixture-cases.json")["cases"]
    cases = fixture_cases if options.fixtures else read("cases.json")["cases"] + fixture_cases
    if options.repo:
        cases = [case for case in cases if case["repository"] in options.repo]
    if options.language:
        cases = [case for case in cases if case["language"] in options.language]
    specs = read("repositories.json")
    outputs = {}
    metrics = []
    for name, language in sorted({(case["repository"], case["language"]) for case in cases}):
        root = DIRECTORY if name == "fixtures" else options.sources / name
        paths = sorted({case["storage"][0] for case in cases if case["repository"] == name and case["language"] == language}) if name == "fixtures" else specs[name]["source_paths"]
        output = options.output / "analysis" / (name + "--" + language + ".json.gz")
        command = [sys.executable, "-X", "faulthandler", str(DIRECTORY / "analyze_languages.py"),
                   "--root", str(root), "--language", language, "--output", str(output)]
        for path in paths:
            command.extend(["--path", path])
        started = time.perf_counter()
        process = subprocess.run(command, text=True, capture_output=True)
        if process.returncode:
            raise RuntimeError(name + "/" + language + ":\n" + process.stderr)
        result = json.loads(gzip.decompress(output.read_bytes()))
        result["revision"] = specs[name]["revision"] if name != "fixtures" else None
        output.write_bytes(gzip.compress((json.dumps(result, ensure_ascii=False, separators=(",", ":")) + "\n").encode(), mtime=0))
        outputs[(name, language)] = result
        metric = {"repository": name, "language": language, **result["metrics"], "wall_seconds": time.perf_counter() - started,
                  "parse_errors": sum(item.get("count", 1) for item in result["notices"] if item["kind"] == "parse_error"),
                  "notices": result["notices"]}
        metrics.append(metric)
        print(json.dumps({key: metric[key] for key in ("repository", "language", "files", "functions", "facts", "relations", "parse_errors")}), flush=True)
    evaluation = evaluate(cases, outputs)
    evaluation["dependencies"] = {name: importlib.metadata.version(name) for name in PACKAGES}
    write(options.output / "evaluation.json", evaluation)
    write(options.output / "metrics.json", metrics)
    failures = [row for row in evaluation["cases"] if row["repository"] == "fixtures" and row["status"] not in {"conditional_candidate", "correctly_unjoined_with_positive_control"}]
    incorrect = [row for row in evaluation["cases"] if row["repository"] != "fixtures" and row["status"] == "incorrect_connection"]
    parse_failures = sum(1 for metric in metrics if metric["repository"] == "fixtures" for notice in metric["notices"] if notice["kind"] == "parse_error")
    print(json.dumps({"cases": len(cases), "fixture_failures": len(failures), "fixture_parse_error_files": parse_failures,
                      "public_incorrect_connections": len(incorrect), "output": str(options.output)}, ensure_ascii=False))
    return 1 if failures or incorrect or parse_failures else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "freeze", "check", "measure"])
    parser.add_argument("--sources", type=Path, default=Path("/tmp/codemap-event-poc-sources"))
    parser.add_argument("--output", type=Path, default=DIRECTORY / "results")
    parser.add_argument("--fixtures", action="store_true")
    parser.add_argument("--repo", action="append", choices=["fixtures", *read("repositories.json")])
    parser.add_argument("--language", action="append", choices=list(read("languages.json")["languages"]))
    options = parser.parse_args()
    options.sources = options.sources.resolve()
    options.output = options.output.resolve()
    if options.command == "prepare":
        prepare(options.sources, options.repo)
    elif options.command == "measure":
        raise SystemExit(measure(options))
    else:
        observed = inventory(options.sources, options.fixtures)
        lock = DIRECTORY / "sources.lock.json"
        if options.command == "freeze":
            if options.fixtures:
                raise ValueError("Freeze requires the complete public and fixture corpus")
            if lock.exists():
                raise ValueError("The source lock already exists; do not silently overwrite a frozen corpus")
            write(lock, observed)
            print(lock)
        elif lock.exists() and not options.fixtures:
            if read("sources.lock.json") != observed:
                raise ValueError("Corpus or source bytes differ from sources.lock.json")
            print("Source lock, revisions, endpoint ranges and language coverage match")
        else:
            print(json.dumps({"languages": len(observed["languages"]), "cases": len(observed["cases"]), "source_lock_present": lock.exists()}))


if __name__ == "__main__":
    main()
