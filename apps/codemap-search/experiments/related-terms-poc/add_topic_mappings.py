"""Copy an rt-sense/4 dataset into a new one that maps existing words to more topics (`sense_mapping_schema.py`).

Usage (from the repository root):
  python3 apps/codemap-search/experiments/related-terms-poc/add_topic_mappings.py \\
      --base .../datasets/rts4-<id> --mappings mappings.json --output-root .../datasets

mappings.json:
  {"base_dataset": "rts4-<id>", "mappings": [{"word": "logo", "topic": "t123", ...},
                                             {"word": "requireNonNull", "topic": "t635.2", "via": "data validation"}, ...]}
Each mapping adds one persona: the word, the topic, no concept and persona count 0 (mapped by review, not generated),
so the new sense follows the word's existing senses. Further keys of a mapping (fit, reason) are kept in the copy
of the file but not read. The word must already be in `terms`, the topic must exist, and the word must have no persona
in that topic yet. Nothing in the base is removed or changed.
A mapping with `via` is for an input-only word (not in `terms`): it binds the word in `input_senses` to the lowest
persona of the `via` word in the topic, so the word reads that topic as a sense through the `via` word. The word must
not be bound to a persona of that topic yet.
"""

import argparse
import hashlib
import json
import os
import shutil
import sqlite3

from sense_mapping_schema import MAPPING_SCHEMA, validate_mapping_layout
from term_canonical import word_key


def via_persona(connection, mapping):
    """The persona an input-only word is bound to: the lowest persona of the `via` word in the topic, or None."""
    row = connection.execute("SELECT min(p.persona) FROM personas AS p JOIN terms AS t USING (term_id) "
                             "WHERE t.word_key = ? AND p.topic = ?", (word_key(mapping["via"]), mapping["topic"])).fetchone()
    return row[0]


def mapping_errors(connection, mappings):
    terms = dict(connection.execute("SELECT word_key, term_id FROM terms"))
    topics = {topic for (topic,) in connection.execute("SELECT topic FROM topics")}
    mapped = set(connection.execute("SELECT term_id, topic FROM personas"))
    bound = set(connection.execute("SELECT s.word_key, p.topic FROM input_senses AS s JOIN personas AS p USING (persona)"))
    errors, seen = [], set()
    for number, mapping in enumerate(mappings):
        key, topic = word_key(mapping.get("word", "")), mapping.get("topic")
        term = terms.get(key)
        if "via" in mapping:
            if term is not None:
                errors.append(f"{number}: {mapping['word']!r} is in terms; map it without via")
            elif topic not in topics or via_persona(connection, mapping) is None:
                errors.append(f"{number}: {mapping['via']!r} has no persona in {topic!r}")
            elif (key, topic) in bound or (key, topic) in seen:
                errors.append(f"{number}: {mapping['word']!r} is already bound to {topic}")
            seen.add((key, topic))
            continue
        if term is None:
            errors.append(f"{number}: word not in terms: {mapping.get('word')!r}")
        elif topic not in topics:
            errors.append(f"{number}: unknown topic {topic!r}")
        elif (term, topic) in mapped or (term, topic) in seen:
            errors.append(f"{number}: {mapping['word']!r} already maps to {topic}")
        seen.add((term, topic))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", required=True, help="rt-sense/4 dataset directory")
    parser.add_argument("--mappings", required=True)
    parser.add_argument("--output-root", required=True)
    arguments = parser.parse_args()
    document = json.load(open(arguments.mappings))
    base_path = os.path.join(arguments.base, "dataset.sqlite3")
    base = sqlite3.connect(f"file:{base_path}?mode=ro", uri=True)
    try:
        meta = dict(base.execute("SELECT key, value FROM sense_meta"))
        if meta.get("schema_version") != MAPPING_SCHEMA:
            raise SystemExit(f"base must be {MAPPING_SCHEMA}, not {meta.get('schema_version')!r}")
        if document.get("base_dataset") != meta["dataset_id"]:
            raise SystemExit(f"mappings were reviewed on {document.get('base_dataset')!r}, base is {meta['dataset_id']!r}")
        validate_mapping_layout(base)
        errors = mapping_errors(base, document["mappings"])
    finally:
        base.close()
    if errors:
        raise SystemExit("mappings rejected:\n" + "\n".join(errors[:50]))
    mappings_json = json.dumps(document, ensure_ascii=False, sort_keys=True)
    digest = hashlib.sha256(f"{MAPPING_SCHEMA}\0{meta['dataset_id']}\0mappings\0{mappings_json}".encode()).hexdigest()
    dataset_id = f"rts4-{digest[:16]}"
    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=False)
    pending_path = os.path.join(output_dir, ".dataset.sqlite3.pending")
    shutil.copyfile(base_path, pending_path)
    connection = sqlite3.connect(pending_path)
    try:
        terms = dict(connection.execute("SELECT word_key, term_id FROM terms"))
        personas = [m for m in document["mappings"] if "via" not in m]
        bindings = [m for m in document["mappings"] if "via" in m]
        first = connection.execute("SELECT max(persona) FROM personas").fetchone()[0] + 1
        connection.executemany("INSERT INTO personas VALUES (?, ?, ?, NULL, 0)", [
            (first + number, terms[word_key(m["word"])], m["topic"]) for number, m in enumerate(personas)
        ])
        connection.executemany("INSERT INTO input_senses VALUES (?, ?)", [
            (word_key(m["word"]), via_persona(connection, m)) for m in bindings
        ])
        connection.executemany("INSERT OR REPLACE INTO sense_meta VALUES (?, ?)", [
            ("dataset_id", dataset_id), ("base_dataset", meta["dataset_id"]), ("mappings", "mappings.json"),
        ])
        # Names a file of the base's folder (`revise_topics.py`), not of this one.
        connection.execute("DELETE FROM sense_meta WHERE key = 'revisions'")
        connection.commit()
        validate_mapping_layout(connection)
        connection.execute("VACUUM")
    finally:
        connection.close()
    shutil.copyfile(arguments.mappings, os.path.join(output_dir, "mappings.json"))
    report = {"dataset_id": dataset_id, "schema_version": MAPPING_SCHEMA, "base_dataset": meta["dataset_id"],
              "mappings": len(document["mappings"]), "words": len({word_key(m["word"]) for m in document["mappings"]}),
              "input_bindings": sum("via" in m for m in document["mappings"])}
    with open(os.path.join(output_dir, "build-report.json"), "x") as handle:
        json.dump(report, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    os.rename(pending_path, os.path.join(output_dir, "dataset.sqlite3"))
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
