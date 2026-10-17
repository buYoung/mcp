"""Copy an rt-sense/3 dataset into a new rt-sense/4 dataset (`sense_mapping_schema.py`) without changing what it says.

What rt-sense/3 repeats is stored once:
- A concept whose only word is the word it is named after (`nvidia-smi` in a concept `nvidia-smi`) is dropped; that
  word's persona maps it to the topic with no concept.
- Personas that repeat a word in the same topic and concept become one, the lowest persona id, which records how
  many it stands for (`source_persona_count`, the sense order weight). Relations, input bindings and input relations
  of the others move to it.
- A persona's field is no longer stored; it is the primary field of its topic, which rt-sense/3 already required.
- A concept no persona uses is dropped.

Usage (from the repository root):
  python3 apps/codemap-search/experiments/related-terms-poc/migrate_sense_dataset.py \\
      --base apps/codemap-search/experiments/related-terms-poc/datasets/rts3-48356e35e1f1ea28 \\
      --output-root apps/codemap-search/experiments/related-terms-poc/datasets

Writes `rts4-<hash>/dataset.sqlite3`, `build-report.json` (counts) and `migration.json` (dropped concepts and merged
personas). `verify_sense_migration.py` checks that lookups answer the same.
"""

import argparse
from collections import Counter
import hashlib
import json
import os
import sqlite3

from reviewed_relations import INPUT_RELATIONS_SCHEMA, validate_dataset_layout
from sense_mapping_schema import MAPPING_SCHEMA, create_schema, validate_mapping_layout
from term_canonical import word_key

# Meta keys that describe the content and stay true after migration; the base's build keys stay in the base.
KEPT_META = ("graph", "key_rule", "input_senses", "input_sense_rule")
COPIED_TABLES = ("terms", "fields", "topics", "topic_fields", "term_legacy_categories")


def plan_migration(base):
    """Which concepts to drop and which persona each persona becomes, read from the base."""
    names = dict(base.execute("SELECT concept, name FROM concepts"))
    topic_of_concept = dict(base.execute("SELECT concept, topic FROM concepts"))
    word_keys = dict(base.execute("SELECT term_id, word_key FROM terms"))
    rows = base.execute("SELECT persona, term_id, concept FROM personas ORDER BY persona").fetchall()
    members = {}
    for _, term, concept in rows:
        members.setdefault(concept, set()).add(term)
    word_named = {c for c, terms in members.items()
                  if len(terms) == 1 and word_key(names[c]) == word_keys[next(iter(terms))]}
    unused = set(names) - set(members)
    kept, target, weight = {}, {}, Counter()
    for persona, term, concept in rows:
        mapping = (term, topic_of_concept[concept], None if concept in word_named else concept)
        target[persona] = kept.setdefault(mapping, persona)
        weight[target[persona]] += 1
    personas = [(persona, term, topic, concept, weight[persona]) for (term, topic, concept), persona in kept.items()]
    return {"word_named": word_named, "unused": unused, "names": names, "target": target, "personas": sorted(personas)}


def migrate(base_path, connection, plan):
    """Fill an empty rt-sense/4 connection from the base; the caller owns the transaction."""
    connection.execute("ATTACH DATABASE ? AS base", (f"file:{base_path}?mode=ro",))
    for table in COPIED_TABLES:
        connection.execute(f"INSERT INTO {table} SELECT * FROM base.{table}")
    dropped = plan["word_named"] | plan["unused"]
    connection.executemany("INSERT INTO concepts VALUES (?, ?, ?)", [
        row for row in connection.execute("SELECT concept, name, topic FROM base.concepts ORDER BY concept")
        if row[0] not in dropped
    ])
    connection.executemany("INSERT INTO personas VALUES (?, ?, ?, ?, ?)", plan["personas"])
    target = plan["target"]
    connection.executemany("INSERT INTO relations VALUES (?, ?, ?, ?, ?, ?)", [
        (term, rank, key, surface, kind, None if persona is None else target[persona])
        for term, rank, key, surface, kind, persona in connection.execute(
            "SELECT term_id, rank, word_key, surface, kind, persona FROM base.relations")
    ])
    bindings = {(key, target[persona]) for key, persona in connection.execute("SELECT word_key, persona FROM base.input_senses")}
    connection.executemany("INSERT INTO input_senses VALUES (?, ?)", sorted(bindings))
    input_relations = Counter()
    for key, persona, related, surface, kind, rank in connection.execute("SELECT * FROM base.input_relations"):
        input_relations[(key, target[persona], related, surface, kind, rank)] += 1
    connection.executemany("INSERT INTO input_relations VALUES (?, ?, ?, ?, ?, ?)", sorted(input_relations))
    connection.commit()
    connection.execute("DETACH DATABASE base")


def table_counts(connection, tables):
    return {table: connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0] for table in tables}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", required=True, help="rt-sense/3 dataset directory")
    parser.add_argument("--output-root", required=True)
    arguments = parser.parse_args()
    base_path = os.path.abspath(os.path.join(arguments.base, "dataset.sqlite3"))
    base = sqlite3.connect(f"file:{base_path}?mode=ro", uri=True)
    try:
        meta = dict(base.execute("SELECT key, value FROM sense_meta"))
        if meta.get("schema_version") != INPUT_RELATIONS_SCHEMA:
            raise SystemExit(f"base must be {INPUT_RELATIONS_SCHEMA}, not {meta.get('schema_version')!r}")
        validate_dataset_layout(base, INPUT_RELATIONS_SCHEMA)
        plan = plan_migration(base)
        before = table_counts(base, ("terms", "concepts", "senses", "personas", "relations", "input_senses", "input_relations"))
    finally:
        base.close()
    digest = hashlib.sha256(f"{MAPPING_SCHEMA}\0{meta['dataset_id']}\0migrate".encode()).hexdigest()
    dataset_id = f"rts4-{digest[:16]}"
    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=False)
    database_path = os.path.join(output_dir, "dataset.sqlite3")
    pending_path = os.path.join(output_dir, ".dataset.sqlite3.pending")
    # URI mode so that ATTACH opens the base read-only.
    connection = sqlite3.connect(f"file:{os.path.abspath(pending_path)}", uri=True)
    try:
        create_schema(connection)
        connection.executemany("INSERT INTO sense_meta VALUES (?, ?)", sorted({
            **{key: meta[key] for key in KEPT_META if key in meta},
            "schema_version": MAPPING_SCHEMA, "dataset_id": dataset_id, "base_dataset": meta["dataset_id"],
            "migration": "migrate_sense_dataset.py: word-named single-word concepts dropped, repeated personas merged, "
                         "persona field read from the topic's primary field",
        }.items()))
        migrate(base_path, connection, plan)
        validate_mapping_layout(connection)
        after = table_counts(connection, ("terms", "concepts", "personas", "relations", "input_senses", "input_relations"))
        connection.execute("VACUUM")
    finally:
        connection.close()
    merged = {persona: kept for persona, kept in plan["target"].items() if persona != kept}
    word_named_personas = sum(1 for _, _, _, concept, _ in plan["personas"] if concept is None)
    report = {
        "dataset_id": dataset_id, "path": database_path, "schema_version": MAPPING_SCHEMA, "base_dataset": meta["dataset_id"],
        "before": before, "after": after, "dropped_word_named_concepts": len(plan["word_named"]),
        "dropped_unused_concepts": len(plan["unused"]), "topic_only_personas": word_named_personas,
        "merged_personas": len(merged),
    }
    with open(os.path.join(output_dir, "migration.json"), "x") as handle:
        json.dump({
            "dropped_concepts": [{"concept": c, "name": plan["names"][c], "reason": "word-named" if c in plan["word_named"] else "unused"}
                                 for c in sorted(plan["word_named"] | plan["unused"])],
            "merged_personas": [{"persona": p, "into": k} for p, k in sorted(merged.items())],
        }, handle, ensure_ascii=False, indent=1)
        handle.write("\n")
    with open(os.path.join(output_dir, "build-report.json"), "x") as handle:
        json.dump(report, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    os.rename(pending_path, database_path)
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
