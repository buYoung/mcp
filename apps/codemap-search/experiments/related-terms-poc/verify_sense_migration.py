"""Check that a migrated sense dataset answers like its base, word by word (`migrate_sense_dataset.py`).

Usage (from the repository root):
  python3 apps/codemap-search/experiments/related-terms-poc/verify_sense_migration.py \\
      --before .../datasets/rts3-48356e35e1f1ea28/dataset.sqlite3 --after .../datasets/rts4-<id>/dataset.sqlite3 \\
      --output report.json [--words N]

Every word the base classifies (own senses or `input_senses`) is looked up alone with the CLI defaults in both
datasets. Each answer is one of:
  identical       same text answer and same JSON answer
  concept_names   same text answer; the JSON differs only in the concept names of senses
  order           same senses and words, in another order
  changed         anything else
`--words N` checks a fixed-seed sample instead of every word.
"""

import argparse
import functools
import json
import random
from pathlib import Path

import category_lookup as cl
import sense_lookup as sl
from spelling_aliases import spelling_queries
from term_canonical import canonical_surface

SEED = 20261017
# Each lookup reloads the inflection evidence; it is the same for every word, so read it once.
cl.InflectionEvidence.load = staticmethod(functools.lru_cache(maxsize=None)(cl.InflectionEvidence.load))


def without_concepts(result):
    return {**result, "senses": [{k: v for k, v in s.items() if k != "concepts"} for s in result["senses"]]}


def sense_set(result):
    return sorted(json.dumps(s, sort_keys=True) for s in without_concepts(result)["senses"])


def classify(before, after):
    if sl.render_text([before]) == sl.render_text([after]):
        if before == after:
            return "identical"
        if without_concepts(before) == without_concepts(after):
            return "concept_names"
    if (before["candidate_count"] == after["candidate_count"] and before["resolution"] == after["resolution"]
            and sense_set(before) == sense_set(after) and before["unsorted_direct"] == after["unsorted_direct"]):
        return "order"
    return "changed"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--words", type=int, help="check a fixed-seed sample of this many words")
    arguments = parser.parse_args()
    before = sl.SenseIndex.load(arguments.before, is_topic_split=True)
    after = sl.SenseIndex.load(arguments.after, is_topic_split=True)
    keys = sorted(set(before.senses) | set(before.input_personas))
    if arguments.words:
        keys = sorted(random.Random(SEED).sample(keys, arguments.words))
    words = [before.surfaces[key] for key in keys]
    stored = cl.run_rtpoc3(spelling_queries(words), cl.DEFAULT_RTPOC3, cl.DEFAULT_DATASET)
    counts, examples = {}, {}
    for word in words:
        results = [sl.lookup(word, stored.get(canonical_surface(word), []), index, is_reverse_included=False,
                             is_syntax_skipped=False, is_exact_first=True, is_inflection_evidence_used=True,
                             inflection_dataset=cl.DEFAULT_DATASET, spelling_matches=stored)
                   for index in (before, after)]
        kind = classify(*results)
        counts[kind] = counts.get(kind, 0) + 1
        if kind != "identical":
            examples.setdefault(kind, []).append(word)
    report = {"before": str(arguments.before), "after": str(arguments.after), "words": len(words), "counts": counts,
              "examples": {kind: found[:50] for kind, found in examples.items()},
              "changed_words": examples.get("changed", []), "order_words": examples.get("order", [])}
    arguments.output.write_text(json.dumps(report, ensure_ascii=False, indent=1) + "\n")
    print(json.dumps({k: report[k] for k in ("words", "counts")}, ensure_ascii=False))


if __name__ == "__main__":
    main()
