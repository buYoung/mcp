"""Copy an rt-sense/4 dataset into a new one whose topics are revised by a reviewed plan (`sense_mapping_schema.py`).

Usage (from the repository root):
  python3 apps/codemap-search/experiments/related-terms-poc/revise_topics.py \\
      --base .../datasets/rts4-<id> --plan revisions.json --output-root .../datasets

revisions.json, applied in this order (further keys such as "reason" are kept in the copy but not read):
  {"base_dataset": "rts4-<id>",
   "new_topics": [{"topic": "x307", "label": "...", "scope": "...", "fields": ["cloud-infrastructure"]}],
   "labels": [{"topic": "t399", "label": "...", "scope": "..."}],
   "fields": [{"topic": "t58.2", "fields": ["algorithms-data-structures"]}],
   "moves": [{"word": "Manacher", "topic": "t123.4", "into": "t456"}],
   "merges": [{"topic": "t1009", "into": "t948"}],
   "drops": [{"topic": "t89.5"}]}
- new_topics: a topic with no words yet; `add_topic_mappings.py` maps words to it afterwards.
- labels: the topic's new label and scope.
- fields: the topic's new fields, primary first. A persona's field is its topic's primary field, so the topic's
  words change field with it.
- moves: a word's personas in one topic move to another without their concept (the concept stays with its other
  words; a concept left without words is dropped). The topic may be merged afterwards; the target may not.
- merges: the topic's concepts and personas move into the other topic and the topic is deleted.
- drops: a topic with no personas and no concepts in the base is deleted. No other entry may name it.
When a word ends up with two personas in one topic and no concept, the lower persona id stays, carrying both weights
(`source_persona_count`), and the relations, input bindings and input relations of the other move to it.
"""

import argparse
import hashlib
import json
import os
import shutil
import sqlite3

from sense_mapping_schema import MAPPING_SCHEMA, validate_mapping_layout
from term_canonical import word_key

PLAN_KEYS = ("new_topics", "labels", "fields", "moves", "merges", "drops")


def plan_errors(connection, plan):
    topics = {topic for (topic,) in connection.execute("SELECT topic FROM topics")}
    fields = {field for (field,) in connection.execute("SELECT field FROM fields")}
    terms = dict(connection.execute("SELECT word_key, term_id FROM terms"))
    mapped = set(connection.execute("SELECT term_id, topic FROM personas"))
    errors = []
    for item in plan.get("new_topics", []):
        if item["topic"] in topics:
            errors.append(f"new topic exists: {item['topic']}")
        if not item.get("label") or not item.get("scope"):
            errors.append(f"new topic without label or scope: {item['topic']}")
        topics.add(item["topic"])
    for item in plan.get("new_topics", []) + plan.get("fields", []):
        if not item["fields"] or len(set(item["fields"])) != len(item["fields"]) or not set(item["fields"]) <= fields:
            errors.append(f"bad fields for {item['topic']}: {item['fields']}")
    for item in plan.get("labels", []) + plan.get("fields", []):
        if item["topic"] not in topics:
            errors.append(f"unknown topic: {item['topic']}")
    merged = {item["topic"] for item in plan.get("merges", [])}
    for item in plan.get("merges", []):
        if item["topic"] not in topics or item["into"] not in topics or item["topic"] == item["into"]:
            errors.append(f"bad merge: {item['topic']} -> {item['into']}")
        if item["into"] in merged:
            errors.append(f"merge into a merged topic: {item['topic']} -> {item['into']}")
    if len(merged) != len(plan.get("merges", [])):
        errors.append("a topic is merged twice")
    for item in plan.get("labels", []) + plan.get("fields", []):
        if item["topic"] in merged:
            errors.append(f"revising a merged topic: {item['topic']}")
    for item in plan.get("moves", []):
        term = terms.get(word_key(item["word"]))
        if term is None or (term, item["topic"]) not in mapped:
            errors.append(f"{item['word']!r} has no persona in {item['topic']}")
        if item["into"] not in topics or item["into"] in merged or item["topic"] == item["into"]:
            errors.append(f"bad move target for {item['word']!r}: {item['topic']} -> {item['into']}")
    used = {topic for (topic,) in connection.execute("SELECT topic FROM personas UNION SELECT topic FROM concepts")}
    named = ({item["topic"] for key in ("new_topics", "labels", "fields", "moves", "merges") for item in plan.get(key, [])}
             | {item["into"] for key in ("moves", "merges") for item in plan.get(key, [])})
    dropped = [item["topic"] for item in plan.get("drops", [])]
    for topic in dropped:
        if topic not in topics or topic in used or topic in named:
            errors.append(f"bad drop: {topic}")
    if len(set(dropped)) != len(dropped):
        errors.append("a topic is dropped twice")
    return errors


def merge_persona(connection, persona, into):
    """Fold one persona into another of the same word, topic and no concept."""
    weight = connection.execute("SELECT source_persona_count FROM personas WHERE persona = ?", (persona,)).fetchone()[0]
    connection.execute("UPDATE personas SET source_persona_count = source_persona_count + ? WHERE persona = ?", (weight, into))
    connection.execute("UPDATE relations SET persona = ? WHERE persona = ?", (into, persona))
    connection.execute("INSERT OR IGNORE INTO input_senses SELECT word_key, ? FROM input_senses WHERE persona = ?", (into, persona))
    connection.execute("INSERT OR IGNORE INTO input_relations SELECT input_word_key, ?, related_word_key, surface, kind, rank "
                       "FROM input_relations WHERE source_persona = ?", (into, persona))
    connection.execute("DELETE FROM input_relations WHERE source_persona = ?", (persona,))
    connection.execute("DELETE FROM input_senses WHERE persona = ?", (persona,))
    connection.execute("DELETE FROM personas WHERE persona = ?", (persona,))


def place_persona(connection, persona, term, topic, concept, merged):
    """Put a persona in a topic, folding it into the word's topic-only persona there when both have no concept."""
    if concept is None:
        existing = connection.execute("SELECT persona FROM personas WHERE term_id = ? AND topic = ? AND concept IS NULL "
                                      "AND persona <> ?", (term, topic, persona)).fetchone()
        if existing:
            keep, fold = sorted((persona, existing[0]))
            merge_persona(connection, fold, keep)
            if keep == persona:
                connection.execute("UPDATE personas SET topic = ?, concept = NULL WHERE persona = ?", (topic, persona))
            merged.append({"persona": fold, "into": keep})
            return
    connection.execute("UPDATE personas SET topic = ?, concept = ? WHERE persona = ?", (topic, concept, persona))


def apply_plan(connection, plan):
    """Apply the plan in order; the caller owns the transaction. Returns what merged and what was dropped."""
    merged, moved = [], []
    for item in plan.get("new_topics", []):
        connection.execute("INSERT INTO topics VALUES (?, ?, ?, -1)", (item["topic"], item["label"], item["scope"]))
        connection.executemany("INSERT INTO topic_fields VALUES (?, ?, ?)",
                               [(item["topic"], field, rank) for rank, field in enumerate(item["fields"])])
    for item in plan.get("labels", []):
        connection.execute("UPDATE topics SET label = ?, scope = ? WHERE topic = ?", (item["label"], item["scope"], item["topic"]))
    for item in plan.get("fields", []):
        connection.execute("DELETE FROM topic_fields WHERE topic = ?", (item["topic"],))
        connection.executemany("INSERT INTO topic_fields VALUES (?, ?, ?)",
                               [(item["topic"], field, rank) for rank, field in enumerate(item["fields"])])
    terms = dict(connection.execute("SELECT word_key, term_id FROM terms"))
    for item in plan.get("moves", []):
        term = terms[word_key(item["word"])]
        for (persona,) in connection.execute("SELECT persona FROM personas WHERE term_id = ? AND topic = ? ORDER BY persona",
                                             (term, item["topic"])).fetchall():
            place_persona(connection, persona, term, item["into"], None, merged)
            moved.append(persona)
    for item in plan.get("merges", []):
        connection.execute("UPDATE concepts SET topic = ? WHERE topic = ?", (item["into"], item["topic"]))
        for persona, term, concept in connection.execute(
                "SELECT persona, term_id, concept FROM personas WHERE topic = ? ORDER BY persona", (item["topic"],)).fetchall():
            place_persona(connection, persona, term, item["into"], concept, merged)
        connection.execute("DELETE FROM topic_fields WHERE topic = ?", (item["topic"],))
        connection.execute("DELETE FROM topics WHERE topic = ?", (item["topic"],))
    for item in plan.get("drops", []):
        connection.execute("DELETE FROM topic_fields WHERE topic = ?", (item["topic"],))
        connection.execute("DELETE FROM topics WHERE topic = ?", (item["topic"],))
    dropped =[{"concept": concept, "name": name} for concept, name in connection.execute(
        "SELECT concept, name FROM concepts c WHERE NOT EXISTS (SELECT 1 FROM personas p WHERE p.concept = c.concept)")]
    connection.execute("DELETE FROM concepts WHERE NOT EXISTS (SELECT 1 FROM personas p WHERE p.concept = concepts.concept)")
    return {"merged_personas": merged, "moved_personas": moved, "dropped_concepts": dropped}


def table_counts(connection):
    return {table: connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
            for table in ("topics", "concepts", "personas", "relations", "input_senses", "input_relations")}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", required=True, help="rt-sense/4 dataset directory")
    parser.add_argument("--plan", required=True)
    parser.add_argument("--output-root", required=True)
    arguments = parser.parse_args()
    plan = json.load(open(arguments.plan))
    base_path = os.path.join(arguments.base, "dataset.sqlite3")
    base = sqlite3.connect(f"file:{base_path}?mode=ro", uri=True)
    try:
        meta = dict(base.execute("SELECT key, value FROM sense_meta"))
        if meta.get("schema_version") != MAPPING_SCHEMA:
            raise SystemExit(f"base must be {MAPPING_SCHEMA}, not {meta.get('schema_version')!r}")
        if plan.get("base_dataset") != meta["dataset_id"]:
            raise SystemExit(f"plan was reviewed on {plan.get('base_dataset')!r}, base is {meta['dataset_id']!r}")
        validate_mapping_layout(base)
        errors = plan_errors(base, plan)
        before = table_counts(base)
    finally:
        base.close()
    if errors:
        raise SystemExit("plan rejected:\n" + "\n".join(errors[:50]))
    plan_json = json.dumps(plan, ensure_ascii=False, sort_keys=True)
    digest = hashlib.sha256(f"{MAPPING_SCHEMA}\0{meta['dataset_id']}\0revisions\0{plan_json}".encode()).hexdigest()
    dataset_id = f"rts4-{digest[:16]}"
    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=False)
    pending_path = os.path.join(output_dir, ".dataset.sqlite3.pending")
    shutil.copyfile(base_path, pending_path)
    connection = sqlite3.connect(pending_path)
    try:
        outcome = apply_plan(connection, plan)
        connection.executemany("INSERT OR REPLACE INTO sense_meta VALUES (?, ?)", [
            ("dataset_id", dataset_id), ("base_dataset", meta["dataset_id"]), ("revisions", "revisions.json"),
        ])
        # Names a file of the base's folder (`add_topic_mappings.py`), not of this one.
        connection.execute("DELETE FROM sense_meta WHERE key = 'mappings'")
        connection.commit()
        validate_mapping_layout(connection)
        after = table_counts(connection)
        connection.execute("VACUUM")
    finally:
        connection.close()
    shutil.copyfile(arguments.plan, os.path.join(output_dir, "revisions.json"))
    report = {"dataset_id": dataset_id, "schema_version": MAPPING_SCHEMA, "base_dataset": meta["dataset_id"],
              **{key: len(plan.get(key, [])) for key in PLAN_KEYS}, "before": before, "after": after,
              "merged_personas": len(outcome["merged_personas"]), "moved_personas": len(outcome["moved_personas"]),
              "dropped_concepts": len(outcome["dropped_concepts"])}
    with open(os.path.join(output_dir, "revision-outcome.json"), "x") as handle:
        json.dump(outcome, handle, ensure_ascii=False, indent=1)
        handle.write("\n")
    with open(os.path.join(output_dir, "build-report.json"), "x") as handle:
        json.dump(report, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    os.rename(pending_path, os.path.join(output_dir, "dataset.sqlite3"))
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
