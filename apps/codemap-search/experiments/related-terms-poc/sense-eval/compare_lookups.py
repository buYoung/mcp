"""Compare how two sense datasets answer the same words, for a dataset step that only adds or revises topics.

Usage: python3 compare_lookups.py --before A.sqlite3 --after B.sqlite3 --output report.json [--revisions revisions.json]
           (--words-file FILE | --mapped-by mappings.json [--affected] | --sample N)

Words: a file (one per line), the words of a mappings.json, the words of other mappings' topics (`--affected`: every
word with a sense in a mapped topic, which now lists the mapped word under "같은 주제"), or a fixed-seed sample of
the words the before dataset classifies. With --revisions, `--mapped-by revisions.json --affected` takes every word
with a sense in a topic the plan merges, moves words out of, or gives new fields. Each word is looked up alone with
the CLI defaults in both datasets.

Report per word set: identical answers, answers that only gained (no sense and no related word lost), answers that
lost a sense (field and topics) or a related word, the change in related-word counts, and examples. It also reports
answers whose shared senses come in another order or start with another sense, and related words kept in the answer
but shown under another sense or ring.
With --revisions (`revise_topics.py`), a before sense is compared as the sense it becomes: a merged topic as the
topic it merged into, and the field as that topic's primary field in the after dataset. A label change alone still
makes the answer not identical.
"""

import argparse
import copy
import functools
import json
import random
import sqlite3
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import category_lookup as cl  # noqa: E402
import sense_lookup as sl  # noqa: E402
from spelling_aliases import spelling_queries  # noqa: E402
from term_canonical import canonical_surface, word_key  # noqa: E402

SEED = 20261017
cl.InflectionEvidence.load = staticmethod(functools.lru_cache(maxsize=None)(cl.InflectionEvidence.load))


def related_words(result):
    found = {w for s in result["senses"] for ring in ("direct", "near", "same_topic") for w in s[ring]}
    return found | {w for part in result["unsorted_direct"] for w in part["direct"]}


def sense_key(sense):
    return sense["span"], sense["field"], tuple(t["topic"] for t in sense["topics"])


def sense_keys(result):
    return {sense_key(s) for s in result["senses"]}


def order_change(old, new):
    """Whether the senses both answers have come in another order, and whether the first sense changed."""
    old_order = [sense_key(s) for s in old["senses"]]
    new_keys = [sense_key(s) for s in new["senses"]]
    shared = set(old_order) & set(new_keys)
    is_first_changed = bool(old_order) and old_order[0] != next(iter(new_keys), None)
    return [k for k in old_order if k in shared] != [k for k in new_keys if k in shared], is_first_changed


def moved_words(old, new):
    """Related words in both answers shown under another sense or ring."""
    def places(result):
        return {w: (sense_key(s), ring) for s in result["senses"] for ring in ("direct", "near", "same_topic") for w in s[ring]}
    old_places, new_places = places(old), places(new)
    return sorted(w for w, place in old_places.items() if w in new_places and new_places[w] != place)


def revision_translator(revisions, after_path):
    """Rewrite a before answer's senses as the senses they become after `revise_topics.py` applied the plan."""
    merged = {item["topic"]: item["into"] for item in revisions.get("merges", [])}
    connection = sqlite3.connect(f"file:{after_path}?mode=ro", uri=True)
    try:
        primary = dict(connection.execute("SELECT topic, field FROM topic_fields WHERE rank = 0"))
    finally:
        connection.close()

    def translate(result):
        result = copy.deepcopy(result)
        for sense in result["senses"]:
            topics = list(dict.fromkeys(merged.get(t["topic"], t["topic"]) for t in sense["topics"]))
            sense["topics"] = [{"topic": t} for t in topics]
            if topics and topics[0] in primary:
                sense["field"] = primary[topics[0]]
        return result
    return translate


def revised_topics(revisions):
    """Topics whose words' answers the plan changes: merged, moved out of, or given new fields."""
    return ({item["topic"] for item in revisions.get("merges", []) + revisions.get("moves", []) + revisions.get("fields", [])}
            | {item["into"] for item in revisions.get("merges", []) + revisions.get("moves", [])})


def compare(words, before, after, stored, translate=None):
    rows = []
    for word in words:
        raw_old, new = (sl.lookup(word, stored.get(canonical_surface(word), []), index, is_reverse_included=False,
                                  is_syntax_skipped=False, is_exact_first=True, is_inflection_evidence_used=True,
                                  inflection_dataset=cl.DEFAULT_DATASET, spelling_matches=stored) for index in (before, after))
        old = translate(raw_old) if translate else raw_old
        is_order_changed, is_first_changed = order_change(old, new)
        rows.append({"word": word, "identical": raw_old == new, "lost_senses": sorted(map(list, sense_keys(old) - sense_keys(new))),
                     "gained_senses": sorted(map(list, sense_keys(new) - sense_keys(old))),
                     "lost_words": len(related_words(old) - related_words(new)),
                     "delta": new["candidate_count"] - old["candidate_count"],
                     "status": [old["status"], new["status"]],
                     "is_order_changed": is_order_changed, "is_first_changed": is_first_changed,
                     "moved_words": moved_words(old, new)})
    changed = [r["delta"] for r in rows if r["delta"]]
    return {
        "words": len(rows), "identical": sum(r["identical"] for r in rows),
        "only_gained": sum(not r["identical"] and not r["lost_senses"] and not r["lost_words"] for r in rows),
        "lost_a_sense": sum(bool(r["lost_senses"]) for r in rows), "lost_words": sum(bool(r["lost_words"]) for r in rows),
        "gained_a_sense": sum(bool(r["gained_senses"]) for r in rows),
        "status_changes": sum(r["status"][0] != r["status"][1] for r in rows),
        "order_changed": sum(r["is_order_changed"] for r in rows), "first_changed": sum(r["is_first_changed"] for r in rows),
        "with_moved_words": sum(bool(r["moved_words"]) for r in rows), "moved_words": sum(len(r["moved_words"]) for r in rows),
        "delta_median": statistics.median(changed) if changed else 0, "delta_max": max(changed, default=0),
        "delta_min": min(changed, default=0),
        "lost_examples": [r for r in rows if r["lost_senses"] or r["lost_words"]][:40],
    }, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--words-file", type=Path)
    parser.add_argument("--mapped-by", type=Path)
    parser.add_argument("--affected", action="store_true")
    parser.add_argument("--sample", type=int)
    parser.add_argument("--revisions", type=Path)
    arguments = parser.parse_args()
    before = sl.SenseIndex.load(arguments.before, is_topic_split=True)
    after = sl.SenseIndex.load(arguments.after, is_topic_split=True)
    revisions = json.loads(arguments.revisions.read_text()) if arguments.revisions else None
    translate = revision_translator(revisions, arguments.after) if revisions else None
    if arguments.words_file:
        words = [line.strip() for line in arguments.words_file.read_text().splitlines() if line.strip()]
    elif arguments.mapped_by:
        document = json.loads(arguments.mapped_by.read_text())
        mappings = document.get("mappings", [])
        mapped = {word_key(m["word"]): m["word"] for m in mappings}
        if arguments.affected:
            topics = revised_topics(document) if "mappings" not in document else {m["topic"] for m in mappings}
            keys = {k for k in set(before.senses) | set(before.input_personas)
                    if any(topics & s.topics for s in before.senses_of(k))} - set(mapped)
            words = sorted(before.surfaces[k] for k in keys)
        else:
            words = sorted(mapped.values())
    else:
        keys = sorted(set(before.senses) | set(before.input_personas))
        words = [before.surfaces[k] for k in sorted(random.Random(SEED).sample(keys, arguments.sample))]
    stored = cl.run_rtpoc3(spelling_queries(words), cl.DEFAULT_RTPOC3, cl.DEFAULT_DATASET)
    summary, rows = compare(words, before, after, stored, translate)
    arguments.output.write_text(json.dumps({"before": str(arguments.before), "after": str(arguments.after),
                                            "summary": summary, "rows": rows}, ensure_ascii=False, indent=1) + "\n")
    print(json.dumps({k: v for k, v in summary.items() if k != "lost_examples"}, ensure_ascii=False))


if __name__ == "__main__":
    main()
