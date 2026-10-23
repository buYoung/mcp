"""Measure the real release example; raw timings and answers stay outside the repository.

SQLite/index preparation and JSON serialization are outside the timed lookup interval.
Cold means a new lookup instance/connection, not an evicted operating-system file cache.
"""
import argparse
import json
import math
from pathlib import Path
import random
import statistics

from compare_rust_lookup import rust_answers


def distribution(values):
    ordered = sorted(values)
    return {"calls": len(values), "p50_ms": statistics.median(values),
            "p95_ms": ordered[math.ceil(len(values) * .95) - 1], "max_ms": max(values)}


def timings(args, words, name, batch_size=1):
    rust_answers(args.binary, args.dataset, args.output, words, name,
                 ["--timings", "--batch-size", str(batch_size)])
    return [json.loads(line) for line in (args.output / f"{name}.stderr").read_text().splitlines()]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--search-p95-ms", type=float, required=True)
    parser.add_argument("--batch-budget-ms", type=float, default=100)
    args = parser.parse_args()
    if args.output.resolve().is_relative_to(Path(__file__).resolve().parents[5]):
        parser.error("evaluation output must be outside the repository")
    report_path = args.output / "comparison-report.json"
    comparison = json.loads(report_path.read_text())
    if comparison["words"] != 176351 or comparison["counts"]["other"]:
        parser.error("latency handoff requires a completed full comparison with other=0")
    items = json.loads((Path(__file__).parent / "eval-set.json").read_text())["items"]
    eval_words = [item["word"] for item in items]
    if len(eval_words) != 240:
        raise ValueError("expected 240 eval identifiers")
    population = json.loads((args.output / "words.json").read_text())
    sample = random.Random(20261017).sample(population, 1000)
    (args.output / "latency-sample.json").write_text(json.dumps(sample, ensure_ascii=False) + "\n")
    cold = [timings(args, ["token"], f"cold-{i}")[0]["milliseconds"] for i in range(10)]
    singles, word_times = {}, {}
    for name, words in (("eval240", eval_words), ("sample1000", sample)):
        rows = timings(args, ["token", *words], name)[1:]
        singles[name] = distribution([row["milliseconds"] for row in rows])
        for row in rows:
            word_times[row["terms"][0]] = row["milliseconds"]
    worst_first = sorted(word_times, key=lambda word: (-word_times[word], word))
    batches = {}
    for size in (1, 2, 4, 8, 16, 32):
        # Distinct words avoid turning a worst-case batch into repeated answer-cache hits.
        rows = timings(args, worst_first, f"batch-{size}", size)
        batches[size] = {**distribution([row["milliseconds"] for row in rows]),
                         "worst_terms": max(rows, key=lambda row: row["milliseconds"])["terms"]}
    candidates = [size for size, result in batches.items() if result["max_ms"] <= args.batch_budget_ms]
    cap = max(candidates, default=1)
    report = {"units": "milliseconds", "seed": 20261017, "cold_token": distribution(cold),
              "singles": singles, "batches": batches, "batch_budget_ms": args.batch_budget_ms,
              "recommended_term_cap": cap, "search_p95_ms": args.search_p95_ms,
              "single_p95_below_search": all(row["p95_ms"] <= args.search_p95_ms for row in singles.values()),
              "cold_definition": "new process and connection; OS file cache not flushed; excludes preparation and serialization"}
    (args.output / "latency-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False))
    if not report["single_p95_below_search"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
