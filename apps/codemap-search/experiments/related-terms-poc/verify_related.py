"""End-to-end verification of `rtpoc3 related` over a built `rt-related/1` dataset.

Checks, each recorded in the report:
1. keys parity: `rtpoc3 related-check` (Rust `rtk-1` keys == Python builder keys);
2. variant probe: systematic spellings of sampled dataset terms (case, separators, attached
   ASCII symbols, wrapping, plurals, compounds) must still match the term;
3. attached-symbol invariance: a symbol-decorated spelling returns exactly the related list
   of the bare spelling, unless the decoration forms an existing protected term (`.net`);
4. fixed cases: protected names, numeronyms, digits, non-ASCII words, joined spellings;
5. output invariants: related lists sorted by similarity, every candidate scored;
6. regression: `rtpoc3 lookup` == frozen `rtpoc2 lookup` on the held-out terms;
7. method freeze: every frozen artifact's sha256 is unchanged.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import random
import sqlite3
import subprocess
import sys
from pathlib import Path

SEED = 20261008
SYMBOL_PREFIXES = ["$", "@", "#", "&", "*", "%", "~", "\\", "!", "^", "+", "=", "_", "__", ":", "::", "-", "--", ".", "/", "?", "<", ">"]
SYMBOL_SUFFIXES = ["?", "!", "()", "[]", ";", ",", "*", "+", "#", "...", "=", ":", "_", "__", "-", ".", "/", "<T>", "()?", "!!", "{}", "&", "%"]
WRAPPERS = ['""', "``", "<>", "()", "[]", "{}", "''", "**", "__"]


def is_protected(key: str) -> bool:
    """Same rule as `rtk-1` (`build_related_dataset.is_protected`)."""
    has_alphanumeric = any(c.isascii() and c.isalnum() for c in key)
    return has_alphanumeric and (key.startswith(".") or "+" in key or "#" in key)


def camel(words: list[str]) -> str:
    return words[0] + "".join(word.capitalize() for word in words[1:])


def pascal(words: list[str]) -> str:
    return "".join(word.capitalize() for word in words)


def plural(word: str) -> str:
    if word.endswith("y") and len(word) > 1 and word[-2] not in "aeiou":
        return word[:-1] + "ies"
    return word + ("es" if word.endswith(("s", "x", "ch", "sh", "z")) else "s")


def run_related(binary: Path, dataset: Path, terms: list[str], work: Path, name: str) -> list[dict]:
    terms_file = work / f"{name}-terms.txt"
    output_file = work / f"{name}-related.jsonl"
    terms_file.write_text("".join(term + "\n" for term in terms))
    subprocess.run(
        [str(binary), "related", "--dataset", str(dataset), "--terms-file", str(terms_file), "--output", str(output_file)],
        check=True,
    )
    outputs = [json.loads(line) for line in output_file.read_text().splitlines() if line.strip()]
    assert len(outputs) == len(terms), (name, len(outputs), len(terms))
    return outputs


def sample_base_terms(dataset: Path, count: int) -> list[tuple[str, str]]:
    """`(exact_key, match_key)` of terms with relations, 1-3 alphabetic words, not protected."""
    connection = sqlite3.connect(f"file:{dataset}?mode=ro", uri=True)
    rows = connection.execute(
        """SELECT t.exact_key, t.match_key FROM terms t
           WHERE t.match_key <> '' AND t.is_atomic = 0
             AND t.exact_key NOT GLOB '*[^a-z ]*'
             AND length(t.match_key) - length(replace(t.match_key, ' ', '')) < 3
             AND EXISTS (SELECT 1 FROM relations r WHERE r.from_term = t.term_id AND r.source = 'llm-thesaurus-v1')"""
    ).fetchall()
    rows = [row for row in rows if all(len(word) >= 2 for word in row[1].split())]
    return random.Random(SEED).sample(rows, min(count, len(rows)))


def build_variants(base: list[tuple[str, str]]) -> list[tuple[str, str, str, str]]:
    """`(category, variant, base_exact_key, expected scope)`; scope `whole` or `part`."""
    variants = []
    for exact, match_key in base:
        words = match_key.split()
        add = lambda category, variant, scope="whole": variants.append((category, variant, exact, scope))
        add("00 as stored", exact)
        add("01 UPPER", exact.upper())
        add("01 Title", exact.title())
        if len(words) > 1:
            add("02 camelCase", camel(words))
            add("02 PascalCase", pascal(words))
            add("02 snake_case", "_".join(words))
            add("02 SCREAMING_SNAKE", "_".join(words).upper())
            add("02 kebab-case", "-".join(words))
            add("02 dot.sep", ".".join(words))
            add("02 a::b", "::".join(words))
            add("02 a/b", "/".join(words))
            add("02 concatenated lower", "".join(words))
            add("02 concatenated UPPER", "".join(words).upper())
        bare = camel(words)
        for symbol in SYMBOL_PREFIXES:
            add(f"03 prefix {symbol}", symbol + bare)
        for symbol in SYMBOL_SUFFIXES:
            # `<T>` adds the word `t`, so the base is a part of `eventLoop<T>`, not the whole.
            add(f"04 suffix {symbol}", bare + symbol, "part" if symbol == "<T>" else "whole")
        for wrapper in WRAPPERS:
            half = len(wrapper) // 2
            add(f"05 wrapped {wrapper}", wrapper[:half] + bare + wrapper[half:])
        add("06 surrounding spaces", f"  {exact}  ")
        if len(words) > 1:
            add("06 double inner space", "  ".join(words))
        last = words[-1]
        if len(last) >= 3:
            plural_words = words[:-1] + [plural(last)]
            add("07 plural natural", " ".join(plural_words))
            add("07 plural camelCase", camel(plural_words))
            add("07 plural snake_case", "_".join(plural_words))
            add("07 plural concatenated", "".join(plural_words))
        add("08 compound prefix camel", camel(["run"] + words), "part")
        add("08 compound suffix camel", camel(words + ["handler"]), "part")
        add("08 compound snake", "_".join(["get"] + words + ["config"]), "part")
        add("08 compound natural", " ".join(["update"] + words), "part")
    return variants


FIXED_CASES = [
    # (input, must match exact keys, must not match exact keys, must not relate, description)
    (".net", [".net"], ["net"], ["network", "internet"], ".net is a proper noun, not net"),
    (".NET", [".net"], ["net"], ["network", "internet"], "case-insensitive .net"),
    ("`.net`", [".net"], ["net"], [], "wrapped .net"),
    ("c++", ["c++"], ["c"], [], "c++ is not c"),
    ("C++17", ["c++"], ["c"], [], "c++ inside a versioned name"),
    ("c#", ["c#"], ["c"], [], "c# is not c"),
    ("F#", ["f#"], ["f"], [], "f# is not f"),
    ("gtk+", ["gtk+"], [], [], "gtk+ matches itself"),
    (".csv", [".csv"], [], [], "extension is protected"),
    ("read .csv file", [".csv", "file"], [], [], "extension inside a phrase"),
    ("k8s", ["k8s"], ["s", "k8"], [], "numeronym is one word"),
    ("i18n", ["i18n"], ["n", "i18"], [], "numeronym is one word"),
    ("HTTP/2", ["http"], ["2"], [], "digits are not looked up alone"),
    ("café", [], ["caf"], [], "non-ASCII word is not stripped to ASCII"),
    ("café menu", ["menu"], ["caf"], [], "non-ASCII word skipped, the rest matches"),
    ("backup", ["backup", "back up"], [], [], "joined spelling variants are merged (exact first)"),
    ("back up", ["back up", "backup"], [], [], "joined spelling variants are merged (exact first)"),
    ("eventLoop", ["event loop", "event", "loop"], [], [], "whole plus parts"),
    ("$eventLoop", ["event loop", "event", "loop"], [], [], "attached symbol"),
    ("eventloop", ["event loop", "event", "loop"], [], [], "concatenated spelling gets the term's words"),
    ("JavaScript", ["javascript"], ["java", "script"], [], "atomic proper noun is not split"),
    ("GitHub", ["github"], ["git", "hub"], [], "atomic proper noun is not split"),
    ("addEventListener", ["add", "event", "listener"], [], [], "identifier parts"),
    ("eventListeners", ["event listener"], [], [], "plural identifier reaches the lemma"),
]


def check_fixed_cases(outputs: list[dict]) -> list[dict]:
    results = []
    for (text, must_match, must_not_match, must_not_relate, description), output in zip(FIXED_CASES, outputs):
        matched = {item["term"].lower() for item in output["matches"]}
        related = {item["term"].lower() for item in output["related"]}
        problems = []
        problems += [f"missing match {key!r}" for key in must_match if key not in matched]
        problems += [f"unexpected match {key!r}" for key in must_not_match if key in matched]
        problems += [f"unexpected related {key!r}" for key in must_not_relate if key in related]
        results.append({"input": text, "description": description, "pass": not problems, "problems": problems,
                        "matches": sorted(matched), "skipped": output["skipped"]})
    return results


def check_invariants(outputs: list[dict]) -> dict:
    unsorted = unscored = 0
    for output in outputs:
        scores = [item["similarity_pct"] for item in output["related"]]
        if any(score is None for score in scores):
            unscored += 1
            continue
        if any(left < right for left, right in zip(scores, scores[1:])):
            unsorted += 1
    return {"outputs": len(outputs), "unsorted_outputs": unsorted, "outputs_with_unscored_candidates": unscored}


def check_regression(arguments: argparse.Namespace, work: Path) -> dict:
    def run(binary: Path, name: str) -> list[dict]:
        output = work / f"regression-{name}.jsonl"
        subprocess.run([str(binary), "lookup", "--dataset", str(arguments.v2_dataset), "--policy", str(arguments.policy),
                        "--terms-file", str(arguments.heldout_terms), "--output", str(output)], check=True, stdout=subprocess.DEVNULL)
        return [json.loads(line) for line in output.read_text().splitlines() if line.strip()]

    def strip_timing(value):
        if isinstance(value, dict):
            return {key: strip_timing(item) for key, item in value.items() if key != "lookup_us"}
        if isinstance(value, list):
            return [strip_timing(item) for item in value]
        return value

    frozen = run(arguments.frozen_binary, "rtpoc2")
    current = run(arguments.binary, "rtpoc3")
    return {"terms": len(frozen), "identical": strip_timing(frozen) == strip_timing(current)}


def check_freeze(freeze_path: Path, repository: Path) -> dict:
    freeze = json.loads(freeze_path.read_text())
    entries = []

    def walk(node):
        if isinstance(node, dict):
            if "sha256" in node and "path" in node:
                entries.append((node["path"], node["sha256"]))
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)

    walk(freeze)
    changed = []
    for path, expected in entries:
        file_path = repository / path
        actual = hashlib.sha256(file_path.read_bytes()).hexdigest() if file_path.exists() else None
        if actual != expected:
            changed.append(path)
    return {"artifacts": len(entries), "changed": changed}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--frozen-binary", type=Path, required=True)
    parser.add_argument("--v2-dataset", type=Path, required=True)
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--heldout-terms", type=Path, required=True)
    parser.add_argument("--freeze", type=Path, required=True)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--base-terms", type=int, default=400)
    arguments = parser.parse_args()
    arguments.work.mkdir(parents=True, exist_ok=True)
    report: dict = {}

    keys = subprocess.run([str(arguments.binary), "related-check", "--dataset", str(arguments.dataset)], capture_output=True, text=True)
    report["keys_parity"] = json.loads(keys.stdout) if keys.stdout else {"status": "error", "stderr": keys.stderr}

    base = sample_base_terms(arguments.dataset, arguments.base_terms)
    variants = build_variants(base)
    outputs = run_related(arguments.binary, arguments.dataset, [variant for _, variant, _, _ in variants], arguments.work, "probe")
    table: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    misses: dict[str, list] = collections.defaultdict(list)
    protected_examples: list[dict] = []
    bare_related: dict[str, list] = {}
    for (category, variant, exact, scope), output in zip(variants, outputs):
        found = any(item["term"].lower() == exact and (scope == "part" or item["scope"] == "whole") for item in output["matches"])
        # `.log` / `.key` are extensions: a protected term matches instead of the bare word.
        is_protected_spelling = any(item["scope"] in ("whole", "protected") and is_protected(item["term"].lower()) for item in output["matches"])
        if not found and is_protected_spelling:
            table[category]["protected"] += 1
            if len(protected_examples) < 20:
                protected_examples.append({"variant": variant, "base": exact, "matched": [item["term"] for item in output["matches"]]})
            continue
        table[category]["hit" if found else "miss"] += 1
        if not found and len(misses[category]) < 5:
            misses[category].append({"variant": variant, "want": exact, "matches": [(item["term"], item["scope"], item["match"]) for item in output["matches"]]})
        if category == "00 as stored":
            bare_related[exact] = output["related"]
    invariance = collections.Counter()
    invariance_failures = []
    camel_related: dict[str, list] = {}
    for (category, variant, exact, scope), output in zip(variants, outputs):
        if category in ("02 camelCase",) or (category == "00 as stored" and " " not in exact):
            camel_related[exact] = output["related"]
    for (category, variant, exact, scope), output in zip(variants, outputs):
        if not category.startswith(("03", "04", "05")) or category == "04 suffix <T>":
            continue
        has_protected_match = any(item["scope"] == "protected" for item in output["matches"])
        if has_protected_match:
            invariance["protected_collision"] += 1
            continue
        # The decorated spelling is itself a dataset term (`@EventHandler`, `__init__`).
        # A raw exact hit means the decorated text itself is a term, so its own relations apply.
        if any(item["match"] == "exact" for item in output["matches"]):
            invariance["decorated_spelling_is_a_term"] += 1
            continue
        if output["related"] == camel_related.get(exact):
            invariance["identical"] += 1
        else:
            invariance["different"] += 1
            if len(invariance_failures) < 20:
                invariance_failures.append({"variant": variant, "base": exact, "matches": output["matches"]})
    report["probe"] = {
        "base_terms": len(base),
        "variants": len(variants),
        "categories": {
            category: {"total": counter["hit"] + counter["miss"] + counter["protected"], "hit": counter["hit"],
                       "protected_spelling": counter["protected"],
                       "rate": round(counter["hit"] / max(1, counter["hit"] + counter["miss"]), 4), "miss_examples": misses[category]}
            for category, counter in sorted(table.items())
        },
        "protected_spelling_examples": protected_examples,
        "symbol_invariance": dict(invariance),
        "symbol_invariance_failures": invariance_failures,
    }

    fixed_outputs = run_related(arguments.binary, arguments.dataset, [case[0] for case in FIXED_CASES], arguments.work, "fixed")
    report["fixed_cases"] = check_fixed_cases(fixed_outputs)
    report["invariants"] = check_invariants(outputs + fixed_outputs)
    report["regression"] = check_regression(arguments, arguments.work)
    report["freeze"] = check_freeze(arguments.freeze, arguments.repository)

    failed_fixed = [case["input"] for case in report["fixed_cases"] if not case["pass"]]
    probe_failures = {category: stats["total"] - stats["hit"] - stats["protected_spelling"]
                      for category, stats in report["probe"]["categories"].items()
                      if stats["hit"] + stats["protected_spelling"] < stats["total"]}
    report["summary"] = {
        "keys_parity": report["keys_parity"].get("status"),
        "probe_variants": len(variants),
        "probe_misses_by_category": probe_failures,
        "probe_protected_spellings": sum(stats["protected_spelling"] for stats in report["probe"]["categories"].values()),
        "symbol_invariance": dict(invariance),
        "fixed_cases_failed": failed_fixed,
        "invariants": report["invariants"],
        "regression_identical": report["regression"]["identical"],
        "freeze_changed": report["freeze"]["changed"],
    }
    (arguments.work / "verification.json").write_text(json.dumps(report, indent=1, ensure_ascii=False))
    print(json.dumps(report["summary"], indent=1, ensure_ascii=False))
    has_failures = (
        report["keys_parity"].get("status") != "pass"
        or bool(probe_failures)
        or invariance.get("different", 0) > 0
        or bool(failed_fixed)
        or report["invariants"]["unsorted_outputs"] > 0
        or report["invariants"]["outputs_with_unscored_candidates"] > 0
        or not report["regression"]["identical"]
        or bool(report["freeze"]["changed"])
    )
    return int(has_failures)


if __name__ == "__main__":
    sys.exit(main())
