"""Split the named topics of a sense graph into narrower topics, named by agents (internal experiment).

A lookup's same-topic group holds every word of the input's topics, so the size of a topic sets the size of that
group. Named topics were cut from automatic topics by subject; this asks agents to cut every named topic of two or
more concepts again, keeping together only concepts whose words the same code uses. Personas, concepts and their
members do not change, no concept leaves its named topic, and every new topic keeps its named topic's fields.

Re-clustering the concept graph instead does not follow the named topics: at the inducer's own topic resolution it
already splits 233 of the 490 named topics of two or more concepts in `sense-graph-2` (295 concepts have no edge to
another concept of their named topic), so that split would undo the subject cuts rather than refine them.

Subcommands, run from the repository root:
  prepare  write --output: personas.jsonl; concepts.jsonl whose `topic` is the named topic's number; names/ with
           field-list.json and topics-carried.jsonl for one-concept topics; naming/named-topics.json (number -> named
           topic id, label, scope, fields) and naming/split-topics-<n>.jsonl for the agents
  check    validate names/topics-<n>.jsonl against naming/split-topics-<n>.jsonl (all shards, or --shard)
  fields   write names/field-assign-split.json: every final topic gets its named topic's fields
"""

from __future__ import annotations

import argparse
from collections import defaultdict
import glob
import json
import os
import shutil

SPLIT_SHARDS = 5
WORDS_PER_CONCEPT = 12


def read_jsonl(path):
    with open(path) as handle:
        return [json.loads(line) for line in handle if line.strip()]


def write_jsonl(path, rows):
    with open(path, "w") as handle:
        handle.writelines(json.dumps(row, ensure_ascii=False) + "\n" for row in rows)


def named_topics(graph):
    """Named topic id (`t10`, `t10.2`) -> label, scope, fields and concept ids, from a named graph's names/."""
    fields = {}
    for path in sorted(glob.glob(os.path.join(graph, "names", "field-assign-*.json"))):
        fields.update(json.load(open(path)))
    topics = {}
    for path in sorted(glob.glob(os.path.join(graph, "names", "topics-*.jsonl"))):
        for row in read_jsonl(path):
            for index, group in enumerate(row["groups"]):
                topic_id = f"t{row['topic']}" if len(row["groups"]) == 1 else f"t{row['topic']}.{index + 1}"
                topics[topic_id] = {"label": group["label"], "scope": group["scope"], "fields": fields[topic_id],
                                    "concepts": sorted(group["concepts"])}
    return topics


def prepare(arguments):
    topics = named_topics(arguments.graph)
    concepts = {c["concept"]: c for c in read_jsonl(os.path.join(arguments.graph, "concepts.jsonl"))}
    numbered = {topic_id: number for number, topic_id in enumerate(sorted(topics, key=lambda t: topics[t]["concepts"][0]))}
    os.makedirs(os.path.join(arguments.output, "names"), exist_ok=True)
    os.makedirs(os.path.join(arguments.output, "naming"), exist_ok=True)
    shutil.copyfile(os.path.join(arguments.graph, "personas.jsonl"), os.path.join(arguments.output, "personas.jsonl"))
    shutil.copyfile(os.path.join(arguments.graph, "names", "field-list.json"), os.path.join(arguments.output, "names", "field-list.json"))
    topic_of = {concept: numbered[topic_id] for topic_id, topic in topics.items() for concept in topic["concepts"]}
    write_jsonl(os.path.join(arguments.output, "concepts.jsonl"), [{**c, "topic": topic_of[c["concept"]]} for c in concepts.values()])
    json.dump({str(numbered[t]): {"named_topic": t, **{k: v for k, v in topic.items() if k != "concepts"}} for t, topic in topics.items()},
              open(os.path.join(arguments.output, "naming", "named-topics.json"), "w"), ensure_ascii=False, indent=0)

    carried, to_split = [], []
    for topic_id, topic in sorted(topics.items(), key=lambda item: numbered[item[0]]):
        number = numbered[topic_id]
        if len(topic["concepts"]) == 1:
            carried.append({"topic": number, "groups": [{"label": topic["label"], "scope": topic["scope"], "concepts": topic["concepts"]}]})
        else:
            to_split.append({"topic": number, "label": topic["label"], "scope": topic["scope"], "concepts": [
                {"concept": c, "name": concepts[c]["name"], "words": concepts[c]["words"][:WORDS_PER_CONCEPT]} for c in topic["concepts"]]})
    write_jsonl(os.path.join(arguments.output, "names", "topics-carried.jsonl"), carried)
    shards = [[] for _ in range(SPLIT_SHARDS)]
    for row in sorted(to_split, key=lambda r: -len(r["concepts"])):
        min(shards, key=lambda rows: sum(len(r["concepts"]) for r in rows)).append(row)
    for number, rows in enumerate(shards, 1):
        write_jsonl(os.path.join(arguments.output, "naming", f"split-topics-{number}.jsonl"), sorted(rows, key=lambda r: r["topic"]))
    print(json.dumps({"named_topics": len(topics), "carried": len(carried), "to_split": len(to_split),
                      "shards": [len(rows) for rows in shards]}))


def check(arguments):
    shards = [arguments.shard] if arguments.shard else range(1, SPLIT_SHARDS + 1)
    errors = []
    for shard in shards:
        expected = {row["topic"]: sorted(c["concept"] for c in row["concepts"])
                    for row in read_jsonl(os.path.join(arguments.graph, "naming", f"split-topics-{shard}.jsonl"))}
        path = os.path.join(arguments.graph, "names", f"topics-{shard}.jsonl")
        if not os.path.exists(path):
            errors.append(f"shard {shard}: {path} missing")
            continue
        seen = set()
        for line, row in enumerate(read_jsonl(path), 1):
            topic = row.get("topic")
            if topic not in expected:
                errors.append(f"shard {shard} line {line}: topic {topic!r} is not in split-topics-{shard}.jsonl")
                continue
            if topic in seen:
                errors.append(f"shard {shard}: topic {topic} written twice")
            seen.add(topic)
            placed = []
            for group in row.get("groups", []):
                if not str(group.get("label", "")).strip() or not str(group.get("scope", "")).strip():
                    errors.append(f"topic {topic}: every group needs a label and a scope")
                placed += group.get("concepts", [])
            if sorted(placed) != expected[topic]:
                errors.append(f"topic {topic}: groups hold {sorted(placed)}, expected each of {expected[topic]} exactly once")
        missing = sorted(set(expected) - seen)
        if missing:
            errors.append(f"shard {shard}: {len(missing)} topics missing, e.g. {missing[:10]}")
    print("\n".join(errors[:40]) if errors else "ok")


def fields(arguments):
    named = json.load(open(os.path.join(arguments.graph, "naming", "named-topics.json")))
    assigned = {}
    for path in sorted(glob.glob(os.path.join(arguments.graph, "names", "topics-*.jsonl"))):
        for row in read_jsonl(path):
            groups = row["groups"]
            for index in range(len(groups)):
                topic_id = f"t{row['topic']}" if len(groups) == 1 else f"t{row['topic']}.{index + 1}"
                assigned[topic_id] = named[str(row["topic"])]["fields"]
    with open(os.path.join(arguments.graph, "names", "field-assign-split.json"), "w") as handle:
        json.dump(assigned, handle, ensure_ascii=False, indent=0)
    print(json.dumps({"topics": len(assigned)}))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=("prepare", "check", "fields"))
    parser.add_argument("--graph", required=True, help="prepare: the named source graph; check/fields: the new graph")
    parser.add_argument("--output", help="prepare: new graph directory")
    parser.add_argument("--shard", type=int, help="check: only that shard")
    arguments = parser.parse_args()
    if arguments.command == "prepare":
        if not arguments.output:
            parser.error("prepare needs --output")
        prepare(arguments)
    elif arguments.command == "check":
        check(arguments)
    else:
        fields(arguments)


if __name__ == "__main__":
    main()
