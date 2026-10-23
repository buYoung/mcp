"""Copy an rt-sense/4 dataset into an rt-sense/5 dataset (`sense_mapping_schema.py`) that holds the same rows in less space.

- Relation kinds are stored once in `relation_kinds`; a relation keeps the kind's id.
- A relation's surface is kept only where it differs from its word key.
- The index on `relations.word_key`, the index on `personas.topic` and `term_legacy_categories` are left out: lookup
  reads whole tables and uses none of them. `personas_by_concept` stays, as the layout check reads through it.
Every relation row is compared with the base after the copy (word key, surface, kind, rank and persona).

Usage (from the repository root):
  python3 apps/codemap-search/experiments/related-terms-poc/compact_sense_dataset.py \\
      --base .../datasets/rts4-<id> --output-root .../datasets

Writes `rts5-<hash>/dataset.sqlite3` and `build-report.json` (sizes and counts).
"""

import argparse
import hashlib
import json
import os
import shutil
import sqlite3

from sense_mapping_schema import (COMPACT_SCHEMA, COMPACT_SQL, DATASET_ID_PREFIXES, MAPPING_SCHEMA, execute_statements,
                                  validate_mapping_layout)

# Name files of the base's folder (`revise_topics.py`, `add_topic_mappings.py`), not of this one
BASE_FILE_META = ("revisions", "mappings")


def compact(connection):
    """Rebuild the relations of a copied base and leave out what lookup does not read; the caller owns the transaction."""
    connection.execute("DROP INDEX relations_by_word")
    connection.execute("ALTER TABLE relations RENAME TO base_relations")
    execute_statements(connection, COMPACT_SQL)
    connection.execute("INSERT INTO relation_kinds (kind) SELECT DISTINCT kind FROM base_relations ORDER BY kind")
    connection.execute(
        "INSERT INTO relations SELECT b.term_id, b.rank, b.word_key, nullif(b.surface, b.word_key), k.kind_id, b.persona "
        "FROM base_relations AS b JOIN relation_kinds AS k USING (kind) ORDER BY b.term_id, b.word_key")
    base_count = connection.execute("SELECT count(*) FROM base_relations").fetchone()[0]
    count = connection.execute("SELECT count(*) FROM relations").fetchone()[0]
    changed = connection.execute(
        "SELECT count(*) FROM (SELECT term_id, rank, word_key, surface, kind, persona FROM base_relations "
        "EXCEPT SELECT term_id, rank, word_key, surface, kind, persona FROM relation_texts)").fetchone()[0]
    if count != base_count or changed:
        raise SystemExit(f"compacted relations differ from the base: {count} of {base_count} rows, {changed} changed")
    connection.execute("DROP TABLE base_relations")
    connection.execute("DROP INDEX personas_by_topic")
    connection.execute("DROP TABLE term_legacy_categories")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", required=True, help="rt-sense/4 dataset directory")
    parser.add_argument("--output-root", required=True)
    arguments = parser.parse_args()
    base_path = os.path.join(arguments.base, "dataset.sqlite3")
    base = sqlite3.connect(f"file:{base_path}?mode=ro", uri=True)
    try:
        meta = dict(base.execute("SELECT key, value FROM sense_meta"))
        if meta.get("schema_version") != MAPPING_SCHEMA:
            raise SystemExit(f"base must be {MAPPING_SCHEMA}, not {meta.get('schema_version')!r}")
        validate_mapping_layout(base)
    finally:
        base.close()
    digest = hashlib.sha256(f"{COMPACT_SCHEMA}\0{meta['dataset_id']}\0compact".encode()).hexdigest()
    dataset_id = f"{DATASET_ID_PREFIXES[COMPACT_SCHEMA]}-{digest[:16]}"
    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=False)
    pending_path = os.path.join(output_dir, ".dataset.sqlite3.pending")
    shutil.copyfile(base_path, pending_path)
    connection = sqlite3.connect(pending_path)
    try:
        compact(connection)
        connection.executemany("INSERT OR REPLACE INTO sense_meta VALUES (?, ?)", [
            ("schema_version", COMPACT_SCHEMA), ("dataset_id", dataset_id), ("base_dataset", meta["dataset_id"]),
            ("compaction", "compact_sense_dataset.py: relation kinds stored once, relation surfaces kept only where they "
                           "differ from the word key, indexes and table lookup does not read left out"),
        ])
        connection.executemany("DELETE FROM sense_meta WHERE key = ?", [(key,) for key in BASE_FILE_META])
        connection.commit()
        validate_mapping_layout(connection)
        kinds = [kind for (kind,) in connection.execute("SELECT kind FROM relation_kinds ORDER BY kind_id")]
        relations, overrides = connection.execute("SELECT count(*), count(surface_override) FROM relations").fetchone()
        connection.execute("VACUUM")
    finally:
        connection.close()
    report = {"dataset_id": dataset_id, "schema_version": COMPACT_SCHEMA, "base_dataset": meta["dataset_id"],
              "bytes": {"before": os.path.getsize(base_path), "after": os.path.getsize(pending_path)},
              "relations": relations, "surface_overrides": overrides, "relation_kinds": kinds}
    with open(os.path.join(output_dir, "build-report.json"), "x") as handle:
        json.dump(report, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    os.rename(pending_path, os.path.join(output_dir, "dataset.sqlite3"))
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
