"""Measure whether synonyms surfaces a codebase's own names for words the code does not use.

Pairs come from the pinned codebases, not from the dictionary: a doc-comment summary word that the
documented declaration's name does not use, paired with that name. The dictionary only decides
which pairs it can bridge at all. No model runs.

  build: read documented symbols from each repository's published index and write the pair set.
  run:   call synonyms for every pair under each ordering configuration and score the responses.

Requires a binary built with --features related-terms-eval. Shared checkouts stay read-only;
indexes, configuration and reports live under --output-dir, so later runs reuse the indexes.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import random
import re
import sqlite3
import time
import traceback

from mcp_tool_eval import McpSession
import overlay_eval as oe
from syntax_parts import subparts
from term_canonical import word_key

HERE = Path(__file__).resolve().parent
SEED = 20261020
MAX_PAIRS = 60
DOC_FREQUENCY_LIMIT = 0.05
EXCLUDED_DIRECTORIES = {"test", "tests", "__tests__", "testing", "spec", "specs", "fixtures", "mocks", "vendor",
                        "vendored", "third_party", "node_modules", "dist", "build"}
EXCLUDED_NAME_PARTS = (".min.", ".test.", ".spec.", "_test.")
EXCLUDED_KIND_PARTS = ("variable", "const", "field", "property", "variant", "static", "test")
# Plain words only: an internal capital, digit or underscore marks a code reference instead.
SUMMARY_WORD = re.compile(r"\b[A-Za-z][a-z]{2,}\b")
GROUP_LINE = re.compile(r"^(?:Direct|Near|Same topic) \(showing [0-9]+ of [0-9]+(?:; [^)]*)?\): (.*)$")


def dictionary_keys(dataset):
    connection = sqlite3.connect(f"file:{dataset}?mode=ro", uri=True)
    try:
        keys = {row[0] for row in connection.execute(
            "SELECT word_key FROM terms UNION SELECT word_key FROM relations UNION SELECT word_key FROM input_senses "
            "UNION SELECT input_word_key FROM input_relations UNION SELECT related_word_key FROM input_relations")}
        inputs = {row[0] for row in connection.execute("SELECT DISTINCT word_key FROM input_senses")}
    finally:
        connection.close()
    return keys, inputs


def is_eligible(symbol):
    path = Path(symbol["path"])
    kind = symbol["kind"].lower()
    return (not symbol["is_test"] and path.suffix in oe.CODE_EXTENSIONS
            and not EXCLUDED_DIRECTORIES.intersection(path.parts[:-1])
            and not any(part in path.name for part in EXCLUDED_NAME_PARTS)
            and not any(part in kind for part in EXCLUDED_KIND_PARTS))


def summary(docstring):
    lines = [line.strip().lstrip("/*#!\"'").strip() for line in docstring.splitlines()]
    paragraph = re.split(r"\n\s*\n", "\n".join(lines).strip(), maxsplit=1)[0]
    sentence = re.split(r"(?<=\.)\s", " ".join(paragraph.split()), maxsplit=1)[0]
    return sentence[:300]


def is_input_form(token, word):
    """The server's overlay rule: the same word, an s/es plural, or a shared 4+ character prefix."""
    def is_plural(longer, shorter):
        return longer.startswith(shorter) and longer[len(shorter):] in ("s", "es")
    return (token == word or is_plural(token, word) or is_plural(word, token)
            or (len(word) >= 4 and len(token) >= 4 and (token.startswith(word) or word.startswith(token))))


def target_keys(name, keys):
    parts = subparts(name)
    targets = set()
    for start in range(len(parts)):
        for end in range(start + 1, min(len(parts), start + 4) + 1):
            key = "".join(parts[start:end])
            if key in keys and (len(key) >= 4 or end - start >= 2):
                targets.add(key)
    return sorted(targets)


def session(arguments, name):
    output = arguments.output_dir / "repositories" / name
    latest = output / "evidence-latest.json"
    client = McpSession(arguments.binary, arguments.dataset, output, cwd=arguments.checkouts / name,
                        home=arguments.output_dir / "shared-home",
                        evidence_timeout_seconds=arguments.evidence_timeout_seconds,
                        env_overrides={"CODEMAP_RELATED_TERMS_EVAL_REPORT": str(latest)})
    return client, latest


def build_repository(name, arguments, keys, inputs):
    client, latest = session(arguments, name)
    try:
        client.initialize()
        client.configure("symbols", "demote")
        evidence = oe.capture_evidence(client, latest)
        documented = client.call("related_terms/documented_symbols", {})
    finally:
        client.close()
    symbols = sorted((symbol for symbol in documented["symbols"] if is_eligible(symbol)),
                     key=lambda symbol: (symbol["path"], symbol["name"], symbol["kind"], symbol["docstring"]))
    summaries = [summary(symbol["docstring"]) for symbol in symbols]
    frequency = Counter(word for text in summaries for word in {word.lower() for word in SUMMARY_WORD.findall(text)})
    limit = DOC_FREQUENCY_LIMIT * len(summaries)
    by_word = defaultdict(list)
    for symbol, text in zip(symbols, summaries):
        targets = target_keys(symbol["name"], keys)
        if not targets:
            continue
        name_tokens = subparts(symbol["name"]) + (subparts(symbol["owner"]) if symbol.get("owner") else [])
        for word in sorted({word.lower() for word in SUMMARY_WORD.findall(text)}):
            if (word_key(word) in inputs and frequency[word] < limit
                    and not any(is_input_form(token, word) for token in name_tokens)):
                by_word[word].append((symbol, text, targets))
    randomizer = random.Random(f"{SEED}-{name}")
    pairs = [(word, randomizer.choice(by_word[word])) for word in sorted(by_word)]
    sampled = randomizer.sample(pairs, min(MAX_PAIRS, len(pairs)))
    items = []
    for index, (word, (symbol, text, targets)) in enumerate(sorted(sampled, key=lambda pair: pair[0])):
        items.append({"id": f"{name}-{index:02d}", "repository": name, "word": word,
                      "is_present": word_key(word) in evidence["words"], "targets": targets, "summary": text,
                      "answer": {key: symbol[key] for key in ["path", "name", "kind", "owner"]}})
    print(f"build {name}: documented={len(documented['symbols'])} eligible={len(symbols)} words={len(by_word)} "
          f"pairs={len(items)} present={sum(item['is_present'] for item in items)}", flush=True)
    return items, {"documented": len(documented["symbols"]), "eligible": len(symbols), "candidate_words": len(by_word)}


def displayed_senses(text):
    """Word keys per displayed sense block; words outside a sense (`Direct without a sense`) are skipped."""
    blocks = []
    for line in text.splitlines():
        if line.startswith("### "):
            blocks.append([])
            continue
        if line.startswith("Direct without a sense"):
            blocks.append(None)
            continue
        match = GROUP_LINE.match(line)
        if match and blocks and blocks[-1] is not None:
            blocks[-1].extend(word_key(word) for word in json.loads("[" + match.group(1) + "]"))
    return [block for block in blocks if block is not None]


def evaluate_item(client, item):
    result = client.lookup_result([item["word"]], should_wait_for_evidence=True)
    text = "\n".join(part["text"] for part in result["content"] if part["type"] == "text")
    measured = result["_meta"]["related_terms_eval"]["answers"][0]
    if measured["input"] != item["word"]:
        raise ValueError("MCP input/result mismatch")
    targets = set(item["targets"])

    def matches(key):
        return any(key in (target, target + "s", target + "es") or target in (key + "s", key + "es")
                   for target in targets)

    bridgeable = any(matches(key) for sense in measured["before"]["senses"]
                     for key in sense["direct"] + sense["near"] + sense["same_topic"])
    blocks = displayed_senses(text)
    rank = next((index + 1 for index, block in enumerate(blocks) if any(matches(key) for key in block)), None)
    decision = measured["decision"] or {}
    return {"id": item["id"], "is_applied": bool(decision.get("is_applied")), "bridgeable": bridgeable,
            "rank": rank, "displayed_senses": len(blocks), "output_bytes": len(text.encode())}


def summarize(rows, items):
    present = {item["id"] for item in items if item["is_present"]}
    judged = [row for row in rows if row["is_applied"] and row["bridgeable"]]
    absent = [row for row in rows if row["id"] not in present]
    absent_bridgeable = [row for row in absent if row["bridgeable"]]
    return {"pairs": len(rows), "applied": sum(row["is_applied"] for row in rows),
            "judged": len(judged), "judged_ids": sorted(row["id"] for row in judged),
            "first": sum(row["rank"] == 1 for row in judged),
            "shown": sum(row["rank"] is not None for row in judged),
            "reciprocal_rank_sum": sum(1 / row["rank"] for row in judged if row["rank"]),
            "absent": len(absent), "absent_bridgeable": len(absent_bridgeable),
            "absent_shown": sum(row["rank"] is not None for row in absent_bridgeable),
            "absent_first": sum(row["rank"] == 1 for row in absent_bridgeable),
            "output_bytes": sum(row["output_bytes"] for row in rows)}


def run_repository(name, items, configs, arguments):
    client, _ = session(arguments, name)
    results = {}
    started = time.monotonic()
    try:
        client.initialize()
        for config in configs:
            client.configure("symbols", "demote", config["score"])
            rows = [evaluate_item(client, item) for item in items]
            results[config["name"]] = {"summary": summarize(rows, items), "rows": rows}
    finally:
        client.close()
    report = {"repository": name, "elapsed_seconds": time.monotonic() - started, "configurations": results}
    (arguments.output_dir / "repositories" / name / "mismatch-report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=1))
    first = results[configs[0]["name"]]["summary"]
    print(f"run {name}: pairs={first['pairs']} judged={first['judged']} {configs[0]['name']} first={first['first']} "
          f"shown={first['shown']} elapsed={report['elapsed_seconds']:.1f}s", flush=True)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["build", "run"])
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--dataset", type=Path, default=oe.POC / "datasets/rts5-75e02862daa29308/dataset.sqlite3")
    parser.add_argument("--checkouts", type=Path, default=oe.REPO / ".codemap/poc/related-terms/workspaces/checkouts")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--set", dest="pair_set", type=Path, default=HERE / "mismatch-set.json")
    parser.add_argument("--configs", type=Path, help="run: JSON list of {name, score}; the first is the baseline")
    parser.add_argument("--repositories", nargs="*", help="Repositories to use (default: all pins)")
    parser.add_argument("--evidence-timeout-seconds", type=int, default=900)
    arguments = parser.parse_args()
    for field in ["binary", "dataset", "checkouts", "output_dir", "pair_set"]:
        setattr(arguments, field, getattr(arguments, field).resolve())
    arguments.output_dir.mkdir(parents=True, exist_ok=True)
    pins = oe.read_pins()
    selected = arguments.repositories or sorted(pins)
    if set(selected) - set(pins):
        raise ValueError("unknown repository selection")
    oe.preflight(arguments.checkouts, pins)
    dataset_sha256 = oe.digest(arguments.dataset)
    identity = {"binary": oe.digest(arguments.binary), "dataset": dataset_sha256, "script": oe.digest(Path(__file__)),
                "pins": {name: pins[name]["commit"] for name in selected}}
    failures = {}
    if arguments.command == "build":
        keys, inputs = dictionary_keys(arguments.dataset)
        items, sources = [], {}
        for name in selected:
            try:
                repository_items, sources[name] = build_repository(name, arguments, keys, inputs)
                items.extend(repository_items)
            except Exception as error:
                failures[name] = {"type": type(error).__name__, "message": str(error)}
                traceback.print_exc()
        if not failures:
            arguments.pair_set.write_text(json.dumps({"identity": identity, "seed": SEED, "sources": sources,
                                                      "items": items}, ensure_ascii=False, indent=1) + "\n")
        result = {"pairs": len(items), "set": str(arguments.pair_set)}
    else:
        configs = json.loads(arguments.configs.read_text())
        pair_set = json.loads(arguments.pair_set.read_text())
        identity |= {"configs": oe.digest(arguments.configs), "set": oe.digest(arguments.pair_set)}
        grouped = defaultdict(list)
        for item in pair_set["items"]:
            grouped[item["repository"]].append(item)
        reports = []
        for name in selected:
            try:
                reports.append(run_repository(name, grouped[name], configs, arguments))
            except Exception as error:
                failures[name] = {"type": type(error).__name__, "message": str(error)}
                traceback.print_exc()
        summary_path = arguments.output_dir / "mismatch-report.json"
        summary_path.write_text(json.dumps({
            "fingerprint": hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest(),
            "identity": identity, "configurations": configs, "failures": failures,
            "repositories": [{"repository": report["repository"],
                              "summaries": {config: result["summary"] for config, result in report["configurations"].items()}}
                             for report in reports]}, ensure_ascii=False, indent=1))
        result = {"repositories": len(reports), "report": str(summary_path)}
    oe.preflight(arguments.checkouts, pins)
    if oe.digest(arguments.dataset) != dataset_sha256:
        raise ValueError("tracked source dataset changed")
    print(json.dumps(result | {"failures": failures}, ensure_ascii=False))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
