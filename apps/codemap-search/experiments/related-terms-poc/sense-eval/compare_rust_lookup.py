"""Stream the pinned PoC reference and compare ordered lookup fields (evaluation, not tests).

Raw outputs must be written outside the repository. The reference uses the CLI defaults
except similarity ordering, and retains full answers for independent review.
"""
import argparse
import json
from pathlib import Path
import sys
import subprocess
import hashlib
import shutil
import sqlite3
import tempfile
from collections import Counter
from itertools import zip_longest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import category_lookup as cl
import sense_lookup as sl
from spelling_aliases import spelling_queries
from term_canonical import canonical_surface, word_key


def comparison_fields(answer):
    return {
        "resolution": {"mode": answer["resolution"]["mode"], "seeds": [
            {"span": seed["span"], "terms": seed["terms"]} for seed in answer["resolution"]["seeds"]]}, 
        "senses": [{key: sense[key] for key in
                    ("span", "field", "topics", "direct", "near", "same_topic")}
                   for sense in answer["senses"]],
        "unsorted_direct": answer["unsorted_direct"],
    }


def reference(output):
    output.mkdir(parents=True, exist_ok=True)
    index = sl.SenseIndex.load(sl.DEFAULT_SENSE_DATASET, is_topic_split=True)
    keys = sorted(set(index.senses) | set(index.input_personas))
    words = [index.surfaces[key] for key in keys]
    if len(words) != 176351:
        raise ValueError(f"unexpected classified population: {len(words)}")
    (output / "words.json").write_text(json.dumps(words, ensure_ascii=False))
    with (output / "reference.jsonl").open("w") as destination:
        for start in range(0, len(words), 2000):
            batch = words[start:start + 2000]
            stored = cl.run_rtpoc3(spelling_queries(batch), cl.DEFAULT_RTPOC3, cl.DEFAULT_DATASET)
            for word in batch:
                answer = sl.lookup(word, stored.get(canonical_surface(word), []), index,
                                   is_exact_first=True, spelling_matches=stored,
                                   is_similarity_ordered=False)
                destination.write(json.dumps(answer, ensure_ascii=False) + "\n")
            destination.flush()
            print(f"reference {min(start + len(batch), len(words))}/{len(words)}", flush=True)
    (output / "reference-summary.json").write_text(json.dumps({
        "words": len(words), "similarity_order": False,
        "fields": ["resolution", "ordered senses: span/field/topics/direct/near/same_topic",
                   "ordered unsorted_direct"],
    }, indent=2) + "\n")


def rust_answers(binary, dataset, output, words, name, extra=()):
    inputs = output / f"{name}-inputs.jsonl"
    inputs.write_text("".join(json.dumps(word, ensure_ascii=False) + "\n" for word in words))
    destination = output / f"{name}.jsonl"
    with inputs.open() as source, destination.open("w") as target, (output / f"{name}.stderr").open("w") as errors:
        subprocess.run([str(binary), str(dataset), "--global-dir", str(output / "home"), *extra],
                       stdin=source, stdout=target, stderr=errors, check=True)
    return destination


def replay_resolution(answer, index):
    """Counterfactual only: keep the unchanged PoC collector, substitute Rust's seeds.

    This is never the baseline. A resolution difference is accepted only if this replay
    reproduces every ordered comparison field; otherwise it remains 'other'.
    """
    seeds = [cl.seed(seed["span"], [word_key(term) for term in seed["terms"]], [], index.vocabulary)
             for seed in answer["resolution"]["seeds"]]
    resolution = cl.Resolution(answer["resolution"]["mode"], seeds, set())
    with patch.object(cl, "resolve", return_value=resolution):
        return sl.lookup(answer["input"], [], index, is_exact_first=True,
                         is_inflection_evidence_used=False, is_similarity_ordered=False)


def compare(args):
    words = json.loads((args.output / "words.json").read_text())
    if len(words) != 176351:
        raise ValueError("full comparison requires 176351 words")
    rust = args.rust_answers or rust_answers(args.binary, args.dataset, args.output, words, "rust")
    index = sl.SenseIndex.load(args.dataset, is_topic_split=True)
    counts = {"identical": 0, "resolution-differs": 0, "other": 0}
    with (args.output / "reference.jsonl").open() as before, rust.open() as after, \
            (args.output / "differences.jsonl").open("w") as differences:
        for position, (word, left, right) in enumerate(zip_longest(words, before, after), 1):
            if word is None or left is None or right is None:
                raise ValueError(f"answer population mismatch at {position}")
            left, right = json.loads(left), json.loads(right)
            if left["input"] != word or right["input"] != word:
                raise ValueError(f"input ordering mismatch at {position}")
            a, b = comparison_fields(left), comparison_fields(right)
            if a == b:
                kind = "identical"
            elif a["resolution"] != b["resolution"] and comparison_fields(replay_resolution(right, index)) == b:
                kind = "resolution-differs"
            else:
                kind = "other"
            counts[kind] += 1
            if kind != "identical":
                differences.write(json.dumps({"input": word, "class": kind,
                    "poc_resolution": a["resolution"], "rust_resolution": b["resolution"],
                    "changed_fields": [key for key in a if a[key] != b[key]],
                    "trace": "matched input/base words differ; unchanged PoC collection replay matches" if kind == "resolution-differs" else "unexplained"}, ensure_ascii=False) + "\n")
            if position % 10000 == 0:
                print(json.dumps({"compared": position, "counts": counts}), flush=True)
    report = {"words": len(words), "counts": counts, "differences": str(args.output / "differences.jsonl"),
              "comparison_fields": ["resolved mode and ordered span/terms (not matcher provenance)",
                                    "ordered span/field/topics/direct/near/same_topic", "ordered unsorted_direct"]}
    (args.output / "comparison-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report), flush=True)
    if counts["other"]:
        raise SystemExit(1)


def trace_resolutions(args):
    """Require a concrete matcher/source-spelling explanation, not merely different seeds."""
    differences = [json.loads(line) for line in (args.output / "differences.jsonl").read_text().splitlines()]
    index = sl.SenseIndex.load(args.dataset, is_topic_split=True)
    words = [row["input"] for row in differences if row["class"] == "resolution-differs"]
    stored = cl.run_rtpoc3(spelling_queries(words), cl.DEFAULT_RTPOC3, cl.DEFAULT_DATASET) if words else {}
    (args.output / "difference-matcher.json").write_text(json.dumps(stored, ensure_ascii=False) + "\n")
    counts, unknown = Counter(), []
    with (args.output / "resolution-traces.jsonl").open("w") as destination:
        for row in differences:
            if row["class"] != "resolution-differs":
                continue
            before = {word_key(term) for seed in row["poc_resolution"]["seeds"] for term in seed["terms"]}
            after = {word_key(term) for seed in row["rust_resolution"]["seeds"] for term in seed["terms"]}
            traces = []
            for key in sorted(before ^ after):
                matches = [match for match in stored[canonical_surface(row["input"])] if word_key(match["term"]) == key and match["scope"] == "whole"]
                eligible = [match for match in matches if match["match"] in cl.INFLECTION_MATCHES and cl.PLAIN_WORD.fullmatch(canonical_surface(match["term"]))]
                surface = index.surfaces.get(key)
                sense_plain = surface is not None and bool(cl.PLAIN_WORD.fullmatch(canonical_surface(surface)))
                if key in before and eligible and not sense_plain:
                    cause = "poc_plain_alias_missing_from_sense_surface"
                elif key in after and sense_plain and not matches:
                    cause = "sense_key_absent_from_poc_whole_matches"
                elif key in after and sense_plain and matches and not eligible:
                    cause = "poc_match_symbolic_but_sense_surface_plain"
                else:
                    cause = "unexplained"
                traces.append({"key": key, "selected_by": "poc" if key in before else "rust", "cause": cause,
                               "sense_surface": surface, "poc_matches": matches})
            if not traces or any(trace["cause"] == "unexplained" for trace in traces):
                unknown.append(row["input"])
            for trace in traces:
                counts[trace["cause"]] += 1
            destination.write(json.dumps({**row, "source_traces": traces}, ensure_ascii=False) + "\n")
    report = {"words": len(words), "source_trace_counts": counts, "unexplained_words": unknown,
              "base_form_evidence_changes": 0 if not unknown else None}
    (args.output / "resolution-trace-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report), flush=True)
    if unknown:
        raise SystemExit(1)


def evaluate(args):
    items = json.loads((Path(__file__).parent / "eval-set.json").read_text())["items"]
    index = sl.SenseIndex.load(args.dataset, is_topic_split=True)
    words = [item["word"] for item in items]
    stored = cl.run_rtpoc3(spelling_queries(words), cl.DEFAULT_RTPOC3, cl.DEFAULT_DATASET)
    reference_rows = [sl.lookup(word, stored.get(canonical_surface(word), []), index,
                               is_exact_first=True, spelling_matches=stored, is_similarity_ordered=False) for word in words]
    (args.output / "eval-reference.jsonl").write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in reference_rows))
    path = rust_answers(args.binary, args.dataset, args.output, words, "eval-rust")
    actual_rows = [json.loads(line) for line in path.read_text().splitlines()]
    if len(actual_rows) != len(items):
        raise ValueError("eval population mismatch")
    rows, summary = [], {side: Counter() for side in ("poc", "rust")}
    for item, left, right in zip(items, reference_rows, actual_rows):
        if not item["expected_fields"]:
            continue
        row = {"id": item["id"], "word": item["word"], "expected_fields": item["expected_fields"]}
        for side, answer in (("poc", left), ("rust", right)):
            fields = [sense["field"] for sense in answer["senses"]]
            hits = {"hit": any(f in item["expected_fields"] for f in fields),
                    "main_hit": item["expected_fields"][0] in fields,
                    "top_hit": bool(fields) and fields[0] in item["expected_fields"]}
            row[side] = {"fields": fields, **hits}
            summary[side].update({"words": 1, **hits})
        rows.append(row)
    changed = [r["word"] for r in rows if r["poc"]["hit"] != r["rust"]["hit"]]
    result = {"summary": summary, "changed_expected_field_hits": changed, "words": rows}
    (args.output / "eval-report.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"summary": summary, "changed_expected_field_hits": changed}), flush=True)
    if len(rows) != 224 or changed:
        raise SystemExit(2)


def preparation_checks(args):
    """Exercise the real preparation example on scratch copies; never edit the input."""
    root = Path(tempfile.mkdtemp(prefix="preparation-", dir=args.output))
    original_hash = hashlib.sha256(args.dataset.read_bytes()).hexdigest()
    home = root / "home"
    command = [str(args.binary), str(args.dataset), "--global-dir", str(home), "--prepare-only"]
    first = subprocess.check_output(command, text=True).strip()
    prepared = Path(first)
    assert prepared.is_relative_to(home / "related-terms" / "local")
    def schema(path):
        with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as connection:
            return list(connection.execute("SELECT name,sql FROM sqlite_master WHERE type='index' ORDER BY name"))
    added = sorted(set(schema(prepared)) - set(schema(args.dataset)))
    assert len(added) == 3, added
    assert {sql.rsplit(" ON ", 1)[1] for _, sql in added} == {"relations(word_key)", "relations(persona)", "personas(topic)"}
    before = (prepared.read_bytes(), (prepared.parent / "ready.json").read_bytes(), prepared.stat().st_mtime_ns)
    assert subprocess.check_output(command, text=True).strip() == first
    assert before == (prepared.read_bytes(), (prepared.parent / "ready.json").read_bytes(), prepared.stat().st_mtime_ns)
    concurrent_home = root / "concurrent"
    concurrent_command = [str(args.binary), str(args.dataset), "--global-dir", str(concurrent_home), "--prepare-only"]
    processes = [subprocess.Popen(concurrent_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(2)]
    paths = []
    for process in processes:
        out, err = process.communicate()
        if process.returncode:
            raise RuntimeError(err)
        paths.append(out.strip())
    assert paths[0] == paths[1]
    path = Path(paths[0])
    assert sorted(p.name for p in path.parent.iterdir()) == ["dataset.sqlite3", "prepare.lock", "ready.json"]
    with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as connection:
        quick_check = connection.execute("PRAGMA quick_check").fetchone()[0]
    assert quick_check == "ok"
    # A version change rebuilds the existing copy, not the user source or any download.
    marker_path = path.parent / "ready.json"
    marker = json.loads(marker_path.read_text())
    marker["index_version"] = 0
    marker_path.write_text(json.dumps(marker))
    with sqlite3.connect(path) as connection:
        connection.execute("DROP INDEX related_terms_by_word")
        connection.execute("CREATE INDEX related_terms_by_word ON relations(rank)")
    subprocess.run(concurrent_command, check=True, stdout=subprocess.DEVNULL)
    assert json.loads(marker_path.read_text())["index_version"] == 1
    assert dict(schema(path))["related_terms_by_word"].endswith("ON relations(word_key)")
    changed_source = root / "changed-source.sqlite3"
    shutil.copyfile(args.dataset, changed_source)
    changed_home = root / "changed-home"
    changed_command = [str(args.binary), str(changed_source), "--global-dir", str(changed_home), "--prepare-only"]
    changed_path = Path(subprocess.check_output(changed_command, text=True).strip())
    with sqlite3.connect(changed_source) as connection:
        connection.execute("INSERT INTO sense_meta(key,value) VALUES('preparation_probe','changed')")
    source_hash = hashlib.sha256(changed_source.read_bytes()).hexdigest()
    subprocess.run(changed_command, check=True, stdout=subprocess.DEVNULL)
    assert hashlib.sha256(changed_source.read_bytes()).hexdigest() == source_hash
    with sqlite3.connect(changed_path) as connection:
        assert connection.execute("SELECT value FROM sense_meta WHERE key='preparation_probe'").fetchone()[0] == "changed"
    with sqlite3.connect(changed_source) as connection:
        connection.execute("UPDATE sense_meta SET value='rt-sense/999' WHERE key='schema_version'")
    invalid = subprocess.run(changed_command, capture_output=True, text=True)
    assert invalid.returncode != 0 and "rt-sense/999" in invalid.stderr
    assert hashlib.sha256(args.dataset.read_bytes()).hexdigest() == original_hash
    report = {"source_sha256_before_and_after": original_hash, "local_path": first,
              "added_indexes": added, "second_preparation_unchanged": True,
              "concurrent_processes": 2, "quick_check": quick_check,
              "index_version_rebuild": True, "changed_source_recopied": True,
              "unsupported_schema_error": invalid.stderr.strip()}
    (args.output / "preparation-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report), flush=True)


def semantics_checks(args):
    items = json.loads((Path(__file__).parent / "eval-set.json").read_text())["items"]
    syntax = ["pub fn build", "class HeroDisplay", "*_test.go", "pending[Mm]easure", "struct ",
              "async [a-z]+\\(", "pub fn", "key string", "return true", "export default",
              "class_declaration", "UseRoutes", ".where(", "list|locationApi", "*.{ts,tsx}", "new or existing"]
    normalization = ["$state", "@Component", "todo!", "useState()", "__init__", ".json", "AbortSignal",
                     "abort signal", "abort_signal", "$?", "$0", ".c", ".h", "std::vector", "C#", "C++", ".NET"]
    aliases = ["backhannel", "polices", "acquring", "cirds", "mata", "asterick", "certficate", "comitted", "conut", "replicy", "uncontented"]
    supplemental_inputs = ["blocked", "retried", "appVersion", "getUserName", "cacheEntries", "abortSignals",
                         "running", "stopped", "parsed", "policies", "retries", "loadUserProfile", "http2Server",
                         "unknownRtermsProbe", "user_session_count", "nonexistent rterms probe"]
    words = list(dict.fromkeys([item["word"] for item in items] + syntax + normalization + aliases + supplemental_inputs))
    index = sl.SenseIndex.load(args.dataset, is_topic_split=True)
    stored = cl.run_rtpoc3(spelling_queries(words), cl.DEFAULT_RTPOC3, cl.DEFAULT_DATASET)
    summaries = {}
    for skip in (False, True):
        name = f"semantics-syntax-{str(skip).lower()}"
        actual = [json.loads(line) for line in rust_answers(args.binary, args.dataset, args.output, words, name,
                  ["--skip-syntax-parts"] if skip else []).read_text().splitlines()]
        counts = Counter()
        differences = []
        for word, right in zip(words, actual):
            left = sl.lookup(word, stored.get(canonical_surface(word), []), index, is_exact_first=True,
                             spelling_matches=stored, is_similarity_ordered=False, is_syntax_skipped=skip)
            a, b = comparison_fields(left), comparison_fields(right)
            kind = "identical" if a == b else "resolution-differs" if a["resolution"] != b["resolution"] and comparison_fields(replay_resolution(right, index)) == b else "other"
            counts[kind] += 1
            if kind != "identical":
                differences.append({"input": word, "class": kind, "poc": a["resolution"], "rust": b["resolution"]})
        if len(actual) != len(words):
            raise ValueError("semantics population mismatch")
        summaries[name] = {"counts": counts, "differences": differences}
    smoke_path = rust_answers(args.binary, args.dataset, args.output, ["token", "appVersion", "abort signal"], "smoke")
    smoke = [json.loads(line) for line in smoke_path.read_text().splitlines()]
    assert all(answer["senses"] for answer in smoke)
    assert {"appVersion", "app", "version"} <= set(smoke[1]["looked_up_inputs"])
    token = smoke[0]
    displayed = [word_key(word) for sense in token["senses"] for ring in ("direct", "near", "same_topic") for word in sense[ring]]
    displayed += [word_key(word) for group in token["unsorted_direct"] for word in group["direct"]]
    assert len(displayed) == len(set(displayed))
    full_counts = Counter(word for sense in token["senses"] for word in sense["full_word_keys"])
    shared = next(word for word, count in full_counts.items() if count > 1 and word in displayed)
    report = {"words_per_mode": len(words), "modes": summaries, "supplemental_inputs": supplemental_inputs,
              "supplemental_scope": "four recorded stage-13 examples plus twelve explicitly selected checks; not the unavailable original sixteen", 
              "shared_word": shared, "full_sense_occurrences": full_counts[shared],
              "display_occurrences": displayed.count(shared), "identifier_decomposition": smoke[1]["looked_up_inputs"]}
    (args.output / "semantics-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"words_per_mode": len(words), "counts": {key: value["counts"] for key, value in summaries.items()},
                      "shared_word": shared, "full_sense_occurrences": full_counts[shared], "display_occurrences": displayed.count(shared)}))
    if any(value["counts"].get("other", 0) for value in summaries.values()):
        raise SystemExit(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--reference", action="store_true")
    mode.add_argument("--eval", action="store_true")
    mode.add_argument("--prepare-check", action="store_true")
    mode.add_argument("--semantics-check", action="store_true")
    mode.add_argument("--trace-resolutions", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--dataset", type=Path, default=sl.DEFAULT_SENSE_DATASET)
    parser.add_argument("--rust-answers", type=Path, help="reuse an already completed full Rust output")
    args = parser.parse_args()
    if args.output.resolve().is_relative_to(Path(__file__).resolve().parents[5]):
        parser.error("evaluation output must be outside the repository")
    if args.reference:
        reference(args.output)
    elif args.trace_resolutions:
        trace_resolutions(args)
    elif args.semantics_check:
        if not args.binary:
            parser.error("--semantics-check needs --binary")
        semantics_checks(args)
    elif args.prepare_check:
        if not args.binary:
            parser.error("--prepare-check needs --binary")
        preparation_checks(args)
    elif args.eval:
        if not args.binary:
            parser.error("--eval needs --binary")
        evaluate(args)
    else:
        if not args.binary and not args.rust_answers:
            parser.error("comparison needs --binary or --rust-answers")
        compare(args)


if __name__ == "__main__":
    main()
