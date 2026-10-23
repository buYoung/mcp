"""Score a sense dataset on the evaluation set: does a looked-up word get a sense in the field it means in the code?

Usage: python3 score_eval_set.py [--sense-dataset PATH] [--output PATH]

Each word is looked up alone with the CLI defaults (`word-eval/lookup_words.lookup_all`). For every judged word:

  hit       a sense's field is one of the expected fields
  main_hit  a sense's field is the main expected field (the first)
  top_hit   the first sense's field is an expected field
  senses, fields, candidates   number of senses, distinct fields and related words in the answer

The output keeps each word's text answer hash so two datasets can be compared word by word.
"""

import argparse
import hashlib
import json
import statistics
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "word-eval"))
from lookup_words import lookup_all  # noqa: E402
import sense_lookup as sl  # noqa: E402

GROUPS = ("target", "multi", "input")


def score_item(item, row):
    expected = item["expected_fields"]
    fields = row["sense_fields"]
    first_hit = next((rank for rank, field in enumerate(fields, 1) if field in expected), None)
    return {
        "id": item["id"], "word": item["word"], "group": item["group"], "expected_fields": expected,
        "status": row["status"], "sense_fields": fields, "hit": first_hit is not None,
        "main_hit": expected[0] in fields, "top_hit": first_hit == 1, "first_hit_rank": first_hit,
        "senses": len(fields), "fields": len(set(fields)), "candidates": row["candidate_count"],
        "answer_sha256": hashlib.sha256(row["output_text"].encode()).hexdigest(),
    }


def summarize(rows):
    count = len(rows)
    if not count:
        return {"words": 0}
    return {
        "words": count,
        "hit": round(100 * sum(r["hit"] for r in rows) / count, 1),
        "main_hit": round(100 * sum(r["main_hit"] for r in rows) / count, 1),
        "top_hit": round(100 * sum(r["top_hit"] for r in rows) / count, 1),
        "no_answer": sum(r["status"] != "ok" for r in rows),
        "median_senses": statistics.median(r["senses"] for r in rows),
        "median_fields": statistics.median(r["fields"] for r in rows),
        "median_candidates": statistics.median(r["candidates"] for r in rows),
        "mean_candidates": round(statistics.mean(r["candidates"] for r in rows), 1),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--sense-dataset", type=Path, default=sl.DEFAULT_SENSE_DATASET)
    parser.add_argument("--eval-set", type=Path, default=HERE / "eval-set.json")
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    items = [i for i in json.loads(arguments.eval_set.read_text())["items"] if i["expected_fields"]]
    rows = lookup_all([i["word"] for i in items], arguments.sense_dataset)
    scored = [score_item(item, row) for item, row in zip(items, rows)]
    summary = {"all": summarize(scored), **{g: summarize([r for r in scored if r["group"] == g]) for g in GROUPS}}
    result = {"sense_dataset": str(arguments.sense_dataset.resolve()), "summary": summary, "words": scored}
    output = arguments.output or HERE / "results" / f"{arguments.sense_dataset.resolve().parent.name}.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=1) + "\n")
    print(json.dumps(summary, ensure_ascii=False, indent=1))
    print(f"-> {output}")


if __name__ == "__main__":
    main()
