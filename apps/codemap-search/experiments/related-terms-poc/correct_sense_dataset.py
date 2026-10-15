"""Apply reviewed, explicit corrections to a new immutable rt-sense/2 or /3 SQLite version.

This does not infer meanings or change lookup/default selection. Use add-entries first
when new concepts/topics are needed; correction plans refer to concrete existing IDs.

Usage:
  python3 correct_sense_dataset.py --base DATASET_DIR --corrections PLAN.json --output-root DATASETS

Plan schema rt-sense-corrections/1:
  base_dataset: exact source sense_meta.dataset_id
  moves: [{concept, from_topic, to_topic}]
  remove_input: [{word, concept}]       remove this input's links to personas in that concept
  remove_own: [{word, concept}]         remove own personas only if no dependent links remain
  personas: [{word, concept}]
  input_bindings: [{word, source_persona, source_head, concept}]  reviewed classification, not own membership
  inherit: [{word, from, concepts: [id, ...]}]  explicit subset of source own/input concepts
  relation_additions: [{source, target, kind, rank}]  exact existing donor row and own/input recipient;
                      see reviewed_relations.py for the strict identity contract. Input targets require /3.
  relation_edits: exact native row removal/reattribution/consolidation/transfer, including opt-in
                  transfer_to_existing_input_binding (requires /3); see reviewed_graph_edits.py
  input_detachments: [{word_key, source: complete_persona_identity}]  detach only one source binding
  Exact edge plans combine only with input_bindings/remove_own; other legacy plans are unchanged.
  provenance: optional review/evidence metadata, included in the version fingerprint

All original versions remain intact. Existing output directories, stale preconditions,
unknown IDs and dangling references fail closed. No relation words are synthesized.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import sqlite3

from build_sense_dataset import SCHEMA_VERSION
from term_canonical import canonical_surface, word_key
from reviewed_relations import (INPUT_RELATIONS_SCHEMA, has_input_relations, create_input_relations,
                                validate_dataset_layout, validate_relation_shape, relation_identity,
                                validate_relation_preconditions, add_reviewed_relation)

from reviewed_graph_edits import (EXACT_OPERATIONS, has_exact_edits, has_input_transfers, validate_exact_shapes,
                                  validate_exact_preconditions, apply_relation_edits, apply_input_detachments)

CORRECTION_SCHEMA = "rt-sense-corrections/1"
TABLES = ("fields", "topics", "topic_fields", "concepts", "terms", "senses", "personas", "input_senses", "relations")
OPERATIONS = {"moves", "remove_input", "remove_own", "personas", "input_bindings", "inherit", "relation_additions"} | EXACT_OPERATIONS


def file_digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def table_counts(connection: sqlite3.Connection, is_input_relation_included=None) -> dict:
    counts = {table: connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0] for table in TABLES}
    present = has_input_relations(connection)
    should_include = present if is_input_relation_included is None else is_input_relation_included
    if should_include:
        counts["input_relations"] = connection.execute("SELECT count(*) FROM input_relations").fetchone()[0] if present else 0
    return counts


def validate_plan_shape(plan: dict) -> None:
    if not isinstance(plan, dict) or plan.get("schema_version") != CORRECTION_SCHEMA:
        raise ValueError(f"expected correction schema {CORRECTION_SCHEMA}")
    unknown = set(plan) - OPERATIONS - {"schema_version", "base_dataset", "provenance"}
    if unknown:
        raise ValueError(f"unknown correction keys: {sorted(unknown)}")
    if not isinstance(plan.get("base_dataset"), str) or not plan["base_dataset"]:
        raise ValueError("base_dataset is required")
    validate_exact_shapes(plan)
    required = {
        "moves": {"concept", "from_topic", "to_topic"},
        "remove_input": {"word", "concept"}, "remove_own": {"word", "concept"},
        "personas": {"word", "concept"}, "inherit": {"word", "from", "concepts"},
        "input_bindings": {"word", "source_persona", "source_head", "concept"},
    }
    for operation in sorted(OPERATIONS):
        if operation in EXACT_OPERATIONS:
            continue
        entries = plan.get(operation, [])
        if not isinstance(entries, list):
            raise ValueError(f"{operation} must be an array")
        seen = set()
        for entry in entries:
            if operation == "relation_additions":
                validate_relation_shape(entry)
                identity = relation_identity(entry)
                if identity in seen:
                    raise ValueError(f"duplicate relation_additions operation: {identity}")
                seen.add(identity)
                continue
            if not isinstance(entry, dict) or set(entry) != required[operation]:
                raise ValueError(f"{operation} entry must have keys {sorted(required[operation])}")
            for key in ("word", "from", "source_head", "from_topic", "to_topic"):
                if key in entry and (not isinstance(entry[key], str) or not entry[key].strip()):
                    raise ValueError(f"{operation}: {key} must be a nonempty string")
            if "concept" in entry and type(entry["concept"]) is not int:
                raise ValueError(f"{operation}: concept must be an integer")
            if operation == "input_bindings":
                if type(entry["source_persona"]) is not int:
                    raise ValueError("input_bindings: source_persona must be an integer")
                if not word_key(entry["word"]) or not word_key(entry["source_head"]):
                    raise ValueError("input_bindings requires nonempty canonical word keys")
                if word_key(entry["word"]) == word_key(entry["source_head"]):
                    raise ValueError("self input binding is not a correction")
            if operation == "inherit":
                concepts = entry["concepts"]
                if not isinstance(concepts, list) or not concepts or any(type(c) is not int for c in concepts):
                    raise ValueError("inherit requires a nonempty explicit concepts list")
                if len(set(concepts)) != len(concepts):
                    raise ValueError("duplicate inherited concept")
                if word_key(entry["word"]) == word_key(entry["from"]):
                    raise ValueError("self inheritance is not a correction")
            identity = (entry["concept"] if operation == "moves" else
                        (word_key(entry["word"]), entry["source_persona"]) if operation == "input_bindings" else
                        (word_key(entry["word"]), word_key(entry["from"])) if operation == "inherit" else
                        (word_key(entry["word"]), entry["concept"]))
            if identity in seen:
                raise ValueError(f"duplicate {operation} operation: {identity}")
            seen.add(identity)
    removed = {(word_key(r["word"]), r["concept"]) for r in plan.get("remove_own", [])}
    added = {(word_key(r["word"]), r["concept"]) for r in plan.get("personas", [])}
    added |= {(word_key(r["word"]), c) for r in plan.get("inherit", []) for c in r["concepts"]}
    if removed & added:
        raise ValueError(f"conflicting own removal/addition: {sorted(removed & added)}")


def validate_input_source(connection: sqlite3.Connection, entry: dict) -> None:
    """Check the exact reviewed source identity, not merely that an ID exists."""
    actual = connection.execute(
        "SELECT t.word_key,p.concept FROM personas p JOIN terms t USING(term_id) WHERE p.persona=?",
        (entry["source_persona"],),
    ).fetchone()
    expected = (word_key(entry["source_head"]), entry["concept"])
    if actual != expected:
        raise ValueError(f"stale/unknown input source persona {entry['source_persona']}: expected {expected}, actual {actual}")


class CorrectionEditor:
    """Apply graph row operations while maintaining topic-primary-field ownership."""

    def __init__(self, connection: sqlite3.Connection):
        self.connection = connection
        self.counts = Counter()
        self.old_senses = set()
        self.affected_concepts = set()

    def next_id(self, table: str, column: str) -> int:
        return (self.connection.execute(f"SELECT max({column}) FROM {table}").fetchone()[0] or 0) + 1

    def concept_field(self, concept: int) -> str:
        row = self.connection.execute(
            "SELECT tf.field FROM concepts c JOIN topic_fields tf USING(topic) WHERE c.concept=? AND tf.rank=0", (concept,)
        ).fetchone()
        if row is None:
            raise ValueError(f"unknown concept or missing primary field: {concept}")
        return row[0]

    def term(self, word: str) -> int:
        key = word_key(word)
        row = self.connection.execute("SELECT term_id FROM terms WHERE word_key=?", (key,)).fetchone()
        if row:
            return row[0]
        term = self.next_id("terms", "term_id")
        self.connection.execute("INSERT INTO terms VALUES (?, ?, ?, ?)", (term, canonical_surface(word), key, '["corrected"]'))
        self.counts["terms_added"] += 1
        return term

    def sense(self, term: int, field: str) -> int:
        row = self.connection.execute("SELECT sense FROM senses WHERE term_id=? AND field=?", (term, field)).fetchone()
        if row:
            return row[0]
        sense = self.next_id("senses", "sense")
        self.connection.execute("INSERT INTO senses VALUES (?, ?, ?)", (sense, term, field))
        self.counts["senses_added"] += 1
        return sense

    def add_persona(self, word: str, concept: int) -> None:
        field = self.concept_field(concept)
        term = self.term(word)
        if self.connection.execute("SELECT 1 FROM personas WHERE term_id=? AND concept=?", (term, concept)).fetchone():
            self.counts["personas_already_present"] += 1
            return
        self.connection.execute("INSERT INTO personas VALUES (?, ?, ?, ?)",
                                (self.next_id("personas", "persona"), term, concept, self.sense(term, field)))
        self.counts["personas_added"] += 1
        self.affected_concepts.add(concept)

    def move_concept(self, entry: dict) -> None:
        concept, source, target = entry["concept"], entry["from_topic"], entry["to_topic"]
        current = self.connection.execute("SELECT topic FROM concepts WHERE concept=?", (concept,)).fetchone()
        if current != (source,) or source == target:
            raise ValueError(f"stale/empty move for concept {concept}: expected {source}, actual {current}, target {target}")
        target_field = self.connection.execute("SELECT field FROM topic_fields WHERE topic=? AND rank=0", (target,)).fetchone()
        if target_field is None:
            raise ValueError(f"unknown target topic or missing primary field: {target}")
        self.connection.execute("UPDATE concepts SET topic=? WHERE concept=?", (target, concept))
        for persona, term, old_sense in self.connection.execute(
            "SELECT persona,term_id,sense FROM personas WHERE concept=? ORDER BY persona", (concept,)
        ).fetchall():
            new_sense = self.sense(term, target_field[0])
            if new_sense != old_sense:
                self.connection.execute("UPDATE personas SET sense=? WHERE persona=?", (new_sense, persona))
                self.old_senses.add(old_sense)
                self.counts["personas_reassigned_field"] += 1
        self.affected_concepts.add(concept)
        self.counts["concepts_moved"] += 1

    def remove_input(self, word: str, concept: int) -> None:
        self.concept_field(concept)
        if has_input_relations(self.connection) and self.connection.execute(
            "SELECT 1 FROM input_relations r JOIN personas p ON p.persona=r.source_persona "
            "WHERE r.input_word_key=? AND p.concept=? LIMIT 1", (word_key(word), concept)
        ).fetchone():
            raise ValueError(f"input removal has unreviewed dependent input_relations: {word}/{concept}")
        cursor = self.connection.execute(
            "DELETE FROM input_senses WHERE word_key=? AND persona IN (SELECT persona FROM personas WHERE concept=?)",
            (word_key(word), concept),
        )
        if not cursor.rowcount:
            raise ValueError(f"missing input binding to remove: {word}/{concept}")
        self.counts["input_senses_removed"] += cursor.rowcount

    def remove_own(self, word: str, concept: int) -> None:
        records = self.connection.execute(
            "SELECT p.persona,p.sense FROM personas p JOIN terms t USING(term_id) WHERE t.word_key=? AND p.concept=?",
            (word_key(word), concept),
        ).fetchall()
        if not records:
            raise ValueError(f"missing own binding to remove: {word}/{concept}")
        for persona, sense in records:
            for table in ("relations", "input_senses"):
                if self.connection.execute(f"SELECT 1 FROM {table} WHERE persona=? LIMIT 1", (persona,)).fetchone():
                    raise ValueError(f"removal has unreviewed dependent {table}: {word}/{concept}/{persona}")
            self.connection.execute("DELETE FROM personas WHERE persona=?", (persona,))
            self.old_senses.add(sense)
            self.counts["personas_removed"] += 1
        self.affected_concepts.add(concept)

    def add_input_binding(self, entry: dict) -> None:
        """Classify a word through one reviewed existing persona, without joining its concept."""
        persona, concept = entry["source_persona"], entry["concept"]
        validate_input_source(self.connection, entry)
        self.concept_field(concept)
        self.term(entry["word"])
        cursor = self.connection.execute("INSERT OR IGNORE INTO input_senses VALUES (?, ?)",
                                         (word_key(entry["word"]), persona))
        self.counts["input_senses_added" if cursor.rowcount else "input_senses_already_present"] += 1
        self.affected_concepts.add(concept)

    def inherit_selected(self, entry: dict) -> None:
        source = word_key(entry["from"])
        own = {r[0] for r in self.connection.execute(
            "SELECT DISTINCT p.concept FROM personas p JOIN terms t USING(term_id) WHERE t.word_key=?", (source,))}
        inputs = self.connection.execute(
            "SELECT i.persona,p.concept FROM input_senses i JOIN personas p USING(persona) WHERE i.word_key=?", (source,)
        ).fetchall()
        selected = set(entry["concepts"])
        missing = selected - own - {c for _, c in inputs}
        if missing:
            raise ValueError(f"source {entry['from']} does not have selected concepts: {sorted(missing)}")
        for concept in sorted(selected & own):
            self.add_persona(entry["word"], concept)
        for persona, concept in inputs:
            if concept in selected:
                self.term(entry["word"])
                cursor = self.connection.execute("INSERT OR IGNORE INTO input_senses VALUES (?, ?)", (word_key(entry["word"]), persona))
                self.counts["input_senses_added"] += cursor.rowcount
        self.counts["inheritance_choices_applied"] += 1

    def add_relation(self, entry: dict) -> None:
        table, is_added = add_reviewed_relation(self.connection, entry)
        self.counts[table + ("_added" if is_added else "_already_present")] += 1
        self.affected_concepts.add(entry["target"]["concept"])

    def finish(self) -> None:
        for sense in sorted(self.old_senses):
            cursor = self.connection.execute("DELETE FROM senses WHERE sense=? AND NOT EXISTS (SELECT 1 FROM personas WHERE sense=?)", (sense, sense))
            self.counts["orphan_senses_removed"] += cursor.rowcount
        for concept in self.affected_concepts:
            invalid = self.connection.execute(
                "SELECT p.persona FROM personas p JOIN senses s USING(sense) JOIN concepts c USING(concept) "
                "JOIN topic_fields tf ON tf.topic=c.topic AND tf.rank=0 WHERE p.concept=? AND (s.field<>tf.field OR s.term_id<>p.term_id)",
                (concept,),
            ).fetchall()
            if invalid:
                raise ValueError(f"invalid persona field/term after correction: {invalid}")


def apply_corrections(base: Path, plan: dict, output_root: Path) -> dict:
    validate_plan_shape(plan)
    base = base / "dataset.sqlite3" if base.is_dir() else base
    if not base.is_file():
        raise FileNotFoundError(base)
    original_digest = file_digest(base)
    canonical_plan = json.dumps(plan, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    plan_digest = hashlib.sha256(canonical_plan.encode()).hexdigest()
    source = sqlite3.connect(base.resolve().as_uri() + "?mode=ro", uri=True)
    directory = None
    try:
        try:
            meta = dict(source.execute("SELECT key,value FROM sense_meta"))
            if meta.get("dataset_id") != plan["base_dataset"]:
                raise ValueError("source dataset does not match the reviewed correction plan")
            validate_dataset_layout(source, meta.get("schema_version"))
            output_schema = (INPUT_RELATIONS_SCHEMA if meta["schema_version"] == INPUT_RELATIONS_SCHEMA
                             or any(e["target"]["origin"] == "input" for e in plan.get("relation_additions", []))
                             or has_input_transfers(plan) else SCHEMA_VERSION)
            validate_relation_preconditions(source, plan)
            validate_exact_preconditions(source, plan)
            removed_sources = {(word_key(entry["word"]), entry["concept"]) for entry in plan.get("remove_own", [])}
            for entry in plan.get("input_bindings", []):
                # Validate against the unedited base before additions can create or recycle IDs.
                validate_input_source(source, entry)
                if (word_key(entry["source_head"]), entry["concept"]) in removed_sources:
                    raise ValueError(f"input source scheduled for own removal: {entry['source_persona']}")
            is_extended = output_schema == INPUT_RELATIONS_SCHEMA
            before = table_counts(source, is_extended)
            fingerprint_schema = CORRECTION_SCHEMA + ("\0" + output_schema if is_extended else "")
            digest = hashlib.sha256((fingerprint_schema + "\0" + meta["dataset_id"] + "\0" + canonical_plan).encode()).hexdigest()
            dataset_id = ("rts3-" if is_extended else "rts2-") + digest[:16]
            destination = output_root / dataset_id
            destination.mkdir(parents=True, exist_ok=False)
            directory = destination  # Only this newly reserved directory may receive failure artifacts.
            database = directory / "dataset.sqlite3"
            pending_database = directory / ".dataset.sqlite3.pending"
            connection = sqlite3.connect(pending_database)
            try:
                source.backup(connection)
                # Publish one closed file, including when the source uses WAL journaling.
                connection.execute("PRAGMA journal_mode=DELETE")
                connection.execute("PRAGMA foreign_keys=ON")
                connection.execute("BEGIN IMMEDIATE")
                if is_extended and not has_input_relations(connection):
                    create_input_relations(connection)
                editor = CorrectionEditor(connection)
                if has_exact_edits(plan):
                    validate_exact_preconditions(connection, plan)
                    editor.counts.update(apply_relation_edits(connection, plan))
                    for entry in plan.get("input_bindings", []): editor.add_input_binding(entry)
                    editor.counts.update(apply_input_detachments(connection, plan))
                    for entry in plan.get("remove_own", []): editor.remove_own(entry["word"], entry["concept"])
                    for entry in plan.get("relation_edits", []):
                        editor.affected_concepts.add(entry["source"]["concept"])
                        if "target" in entry: editor.affected_concepts.add(entry["target"]["concept"])
                else:
                    for entry in plan.get("moves", []): editor.move_concept(entry)
                    for entry in plan.get("remove_input", []): editor.remove_input(entry["word"], entry["concept"])
                    for entry in plan.get("remove_own", []): editor.remove_own(entry["word"], entry["concept"])
                    for entry in plan.get("personas", []): editor.add_persona(entry["word"], entry["concept"])
                    for entry in plan.get("inherit", []): editor.inherit_selected(entry)
                    for entry in plan.get("input_bindings", []): editor.add_input_binding(entry)
                    for entry in plan.get("relation_additions", []): editor.add_relation(entry)
                editor.finish()
                foreign_key_errors = connection.execute("PRAGMA foreign_key_check").fetchall()
                if foreign_key_errors:
                    raise ValueError(f"foreign key errors: {foreign_key_errors[:10]}")
                connection.executemany("INSERT OR REPLACE INTO sense_meta VALUES (?, ?)", [
                    ("dataset_id", dataset_id), ("base_dataset", meta["dataset_id"]),
                    ("correction_schema", CORRECTION_SCHEMA), ("correction_plan_sha256", plan_digest),
                ])
                if output_schema != meta["schema_version"]:
                    connection.execute("UPDATE sense_meta SET value=? WHERE key='schema_version'", (output_schema,))
                validate_dataset_layout(connection, output_schema)
                after = table_counts(connection, is_extended)
                if file_digest(base) != original_digest:
                    raise ValueError("source dataset changed during correction")
                connection.commit()
                connection.execute("VACUUM")
            except BaseException:
                connection.rollback()
                raise
            finally:
                connection.close()
        finally:
            source.close()
        result = {"dataset_id": dataset_id, "path": str(database), "schema_version": output_schema,
                  "base_dataset": meta["dataset_id"], "base_sha256": original_digest, "plan_sha256": plan_digest,
                  "before": before, "after": after, "delta": {t: after[t] - before[t] for t in after},
                  "operations": dict(editor.counts), "foreign_key_errors": [], "status": "complete"}
        with (directory / "corrections.json").open("x") as handle:
            json.dump(plan, handle, ensure_ascii=False, indent=2)
        with (directory / "build-report.json").open("x") as handle:
            json.dump(result, handle, ensure_ascii=False, indent=2)
        # Last fallible operation: normal lookup/base paths cannot see a partial build.
        pending_database.rename(database)
    except BaseException as error:
        if directory is not None:
            try:
                report = directory / "build-report.json"
                if report.exists():
                    report.rename(directory / "unpublished-build-report.json")
                with (directory / "failed-build.json").open("x") as handle:
                    json.dump({"status": "failed", "base_dataset": meta["dataset_id"], "error": str(error)}, handle, indent=2)
            except OSError as reporting_error:
                raise error from reporting_error
        raise
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", required=True, type=Path)
    parser.add_argument("--corrections", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    args = parser.parse_args()
    plan = json.loads(args.corrections.read_text())
    print(json.dumps(apply_corrections(args.base, plan, args.output_root), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
