"""Verify version preservation and repeated add-entries builds with synthetic SQLite fixtures.

Run from the repository root:
  python3 apps/codemap-search/experiments/related-terms-poc/verify_sense_additions.py

No live dataset is changed. Fixed cases and table-delta invariants follow verify_related.py.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
import tempfile
from unittest.mock import patch

import build_sense_dataset as builder
import correct_sense_dataset as corrections
import sense_lookup


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture(directory: Path) -> Path:
    directory.mkdir()
    database = directory / "dataset.sqlite3"
    with sqlite3.connect(database) as connection:
        connection.executescript(builder.SCHEMA_SQL)
        connection.execute("CREATE TABLE input_senses (word_key TEXT, persona INTEGER, PRIMARY KEY(word_key, persona))")
        connection.executemany("INSERT INTO sense_meta VALUES (?, ?)", [
            ("schema_version", builder.SCHEMA_VERSION), ("dataset_id", "fixture-base"),
            ("input_sense_rule", json.dumps({"direct": "head"})),
        ])
        connection.execute("INSERT INTO fields VALUES ('testing', 'Testing', 'Synthetic fixture')")
        # x1 already exists in a base that has been extended once.
        connection.execute("INSERT INTO topics VALUES ('x1', 'Existing topic', 'Synthetic fixture', -1)")
        connection.execute("INSERT INTO topic_fields VALUES ('x1', 'testing', 0)")
        connection.execute("INSERT INTO concepts VALUES (1, 'Existing concept', 'x1')")
        connection.execute("INSERT INTO terms VALUES (1, 'anchor', 'anchor', '[]')")
        connection.execute("INSERT INTO senses VALUES (1, 1, 'testing')")
        connection.execute("INSERT INTO personas VALUES (1, 1, 1, 1)")
    return database


def arguments(base: Path, entries: Path, output: Path) -> argparse.Namespace:
    return argparse.Namespace(base=str(base.parent), entries=str(entries), output_root=str(output))


def table_counts(database: Path) -> dict:
    with sqlite3.connect(f"file:{database}?mode=ro", uri=True) as connection:
        return {table: connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
                for table in ("fields", "topics", "concepts", "terms", "senses", "personas", "input_senses")}


def check_repeated_extension(root: Path) -> dict:
    base = fixture(root / "base")
    base_digest = digest(base)
    first_entries = root / "first.json"
    first_entries.write_text(json.dumps({
        "topics": [{"key": "next", "label": "Next topic", "scope": "Synthetic fixture", "fields": ["testing"]}],
        "concepts": [{"key": "next-concept", "name": "Next concept", "topic": "next"}],
        "personas": [{"word": "first", "concept": "next-concept"}, {"word": "peer", "concept": "next-concept"}],
    }))
    first = builder.add_entries(arguments(base, first_entries, root / "datasets"))
    first_db = Path(first["path"])
    first_digest = digest(first_db)
    second_entries = root / "second.json"
    second_entries.write_text(json.dumps({
        "topics": [{"key": "last", "label": "Last topic", "scope": "Synthetic fixture", "fields": ["testing"]}],
        "concepts": [{"key": "last-concept", "name": "Last concept", "topic": "last"}],
        "personas": [{"word": "second", "concept": "last-concept"}],
        "inherit": [{"word": "firsts", "from": "first"}],
    }))
    second = builder.add_entries(arguments(first_db, second_entries, root / "datasets"))
    second_db = Path(second["path"])
    assert digest(base) == base_digest
    assert digest(first_db) == first_digest
    before, after = table_counts(first_db), table_counts(second_db)
    for table in before:
        report_key = {"fields": "new_fields", "topics": "new_topics", "concepts": "new_concepts"}.get(table, table)
        assert after[table] - before[table] == second.get(report_key, 0), (table, before, after, second)
    index = sense_lookup.SenseIndex.load(second_db, is_topic_split=True)
    assert index.senses_of("firsts")
    assert {c for s in index.senses_of("firsts") for c in s.concepts} == {c for s in index.senses_of("first") for c in s.concepts}
    # Check the final consumer, not only inserted rows.
    result = sense_lookup.lookup("firsts", [], index, is_exact_first=True, is_inflection_evidence_used=False)
    words = {w for s in result["senses"] for ring in ("direct", "near", "same_topic") for w in s[ring]}
    assert "peer" in words, result
    assert "second" not in words, result
    with sqlite3.connect(f"file:{second_db}?mode=ro", uri=True) as connection:
        assert connection.execute("PRAGMA foreign_key_check").fetchall() == []
        assert connection.execute("SELECT count(*) FROM topics").fetchone()[0] == 3
        assert connection.execute("SELECT label FROM topics WHERE topic='x1'").fetchone()[0] == "Existing topic"
    return {"pass": True, "base_preserved": True, "prior_version_preserved": True, "table_deltas_verified": True,
            "inherited_sense_reaches_lookup": True}


def check_existing_version(root: Path) -> dict:
    base = fixture(root / "base")
    entries = root / "entries.json"
    entries.write_text(json.dumps({"personas": [{"word": "additional", "concept": 1}]}))
    args = arguments(base, entries, root / "datasets")
    result = builder.add_entries(args)
    database = Path(result["path"])
    # A harmless metadata marker detects destructive reconstruction on the second call.
    with sqlite3.connect(database) as connection:
        connection.execute("INSERT INTO sense_meta VALUES ('preservation_marker', 'keep')")
    before = digest(database)
    try:
        builder.add_entries(args)
    except (SystemExit, FileExistsError):
        assert digest(database) == before
        return {"pass": True, "existing_version_refused": True, "bytes_preserved": True}
    raise AssertionError("existing dataset was not refused")


def check_lookup_invariants(root: Path) -> dict:
    database = fixture(root / "base")
    with sqlite3.connect(database) as connection:
        connection.execute("INSERT INTO fields VALUES ('files-io', 'Files and I/O', 'Synthetic fixture')")
        for topic, field in [("x2", "files-io"), ("x3", "testing"), ("x4", "testing")]:
            connection.execute("INSERT INTO topics VALUES (?, ?, 'Synthetic fixture', -1)", (topic, topic))
            connection.execute("INSERT INTO topic_fields VALUES (?, ?, 0)", (topic, field))
        for concept, topic in [(2, "x2"), (3, "x3"), (4, "x4")]:
            connection.execute("INSERT INTO concepts VALUES (?, ?, ?)", (concept, f"Concept {concept}", topic))
        connection.execute("INSERT INTO senses VALUES (2, 1, 'files-io')")
        connection.execute("INSERT INTO personas VALUES (2, 1, 2, 2)")
        term_id, sense_id, persona_id = 2, 3, 3
        # Many candidates establish absence of an output cap; a shared member has another topic.
        members = [(f"candidate{i:04d}", 1) for i in range(1200)]
        members += [("shared", 1), ("field-peer", 2), ("beyond", 3), ("anchors", 4), ("own-peer", 4)]
        ids = {}
        for word, concept in members:
            field = "files-io" if concept == 2 else "testing"
            connection.execute("INSERT INTO terms VALUES (?, ?, ?, '[]')", (term_id, word, builder.word_key(word)))
            connection.execute("INSERT INTO senses VALUES (?, ?, ?)", (sense_id, term_id, field))
            connection.execute("INSERT INTO personas VALUES (?, ?, ?, ?)", (persona_id, term_id, concept, sense_id))
            ids[word] = (term_id, sense_id, persona_id)
            term_id, sense_id, persona_id = term_id + 1, sense_id + 1, persona_id + 1
        shared_term, shared_sense, _ = ids["shared"]
        connection.execute("INSERT INTO personas VALUES (?, ?, 3, ?)", (persona_id, shared_term, shared_sense))
        # An input-only spelling has no own persona; it can still take the base-form path.
        own_term, _, own_persona = ids["anchors"]
        connection.execute("INSERT INTO relations VALUES (?, 1, 'anchored', 'anchored', 'form', ?)", (own_term, own_persona))
        connection.execute("INSERT INTO input_senses VALUES ('anchored', ?)", (own_persona,))
    index = sense_lookup.SenseIndex.load(database, is_topic_split=True)

    def query(word, matches=()):
        return sense_lookup.lookup(word, list(matches), index, is_exact_first=True, is_inflection_evidence_used=False)

    def words(result):
        return {word for sense in result["senses"] for ring in ("direct", "near", "same_topic") for word in sense[ring]}

    result = query("anchor")
    expected = {f"candidate{i:04d}" for i in range(1200)} | {"shared", "field-peer"}
    assert words(result) == expected
    assert result["candidate_count"] == len(expected)
    assert {sense["field"] for sense in result["senses"]} == {"testing", "files-io"}
    assert "beyond" not in words(result), "returned members must not expand into their other topics"
    matches = [{"term": "anchor", "scope": "whole", "span": "anchors", "match": "lemma"}]
    own = query("anchors", matches)
    assert own["resolution"]["seeds"][0]["terms"] == ["anchors"]
    assert "own-peer" in words(own) and "shared" not in words(own)
    input_only = query("anchored", [{**matches[0], "span": "anchored"}])
    assert set(input_only["resolution"]["seeds"][0]["terms"]) == {"anchor", "anchored"}
    assert expected | {"own-peer", "anchors"} == words(input_only)
    spelling = query("$anchor")
    assert words(spelling) == expected
    return {"pass": True, "multiple_fields": True, "one_hop_only": True,
            "all_1202_candidates_returned": True, "own_head_exact_first": True,
            "input_only_base_form_path": True, "surface_normalization": True}


def check_unused_match_exclusion(root: Path) -> dict:
    database = fixture(root / "base")
    entries = root / "entries.json"
    entries.write_text(json.dumps({"personas": [
        {"word": "anchor body", "concept": 1}, {"word": "peer", "concept": 1},
    ]}))
    report = builder.add_entries(arguments(database, entries, root / "datasets"))
    index = sense_lookup.SenseIndex.load(report["path"], is_topic_split=True)
    matches = [
        {"term": "anchor body", "scope": "whole", "span": "anchor body", "match": "exact"},
        {"term": "anchor", "scope": "part", "span": "anchor", "match": "exact"},
    ]
    result = sense_lookup.lookup("anchor body", matches, index, is_exact_first=True, is_inflection_evidence_used=False)
    assert result["resolution"]["seeds"] == [{"span": "anchor body", "terms": ["anchor body"], "match": "exact"}]
    candidates = {word for sense in result["senses"] for ring in ("direct", "near", "same_topic") for word in sense[ring]}
    assert candidates == {"anchor", "peer"}, ("an unused partial match suppressed a one-hop candidate", result)
    assert result["candidate_count"] == 2
    return {"pass": True, "whole_exact_seed_unchanged": True, "unused_partial_match_does_not_filter_candidates": True}


def correction_fixture(root: Path) -> Path:
    database = fixture(root / "base")
    with sqlite3.connect(database) as connection:
        connection.execute("INSERT INTO fields VALUES ('files-io', 'Files and I/O', 'Synthetic fixture')")
        for topic, field in [("x2", "files-io"), ("x3", "testing")]:
            connection.execute("INSERT INTO topics VALUES (?, ?, 'Synthetic fixture', -1)", (topic, topic))
            connection.execute("INSERT INTO topic_fields VALUES (?, ?, 0)", (topic, field))
        connection.executemany("INSERT INTO concepts VALUES (?, ?, ?)", [(2, "I/O peer", "x2"), (3, "Unrelated meaning", "x3")])
        connection.executemany("INSERT INTO terms VALUES (?, ?, ?, '[]')", [
            (2, "io-peer", builder.word_key("io-peer")), (3, "anchors", "anchors"),
            (4, "bad-peer", builder.word_key("bad-peer")),
        ])
        connection.executemany("INSERT INTO senses VALUES (?, ?, ?)", [(2, 2, "files-io"), (3, 3, "testing"), (4, 4, "testing")])
        connection.executemany("INSERT INTO personas VALUES (?, ?, ?, ?)", [(2, 1, 3, 1), (3, 2, 2, 2), (4, 3, 3, 3), (5, 4, 3, 4)])
        connection.executemany("INSERT INTO input_senses VALUES (?, ?)", [("anchor", 3), ("anchors", 2)])
        connection.execute("INSERT INTO relations VALUES (1, 1, 'unreviewed', 'unreviewed', 'near', 2)")
    return database


def check_reviewed_corrections(root: Path) -> dict:
    database = correction_fixture(root)
    before_digest = digest(database)
    plan = {
        "schema_version": corrections.CORRECTION_SCHEMA, "base_dataset": "fixture-base",
        "moves": [{"concept": 1, "from_topic": "x1", "to_topic": "x2"}],
        "remove_input": [{"word": "anchors", "concept": 3}],
        "remove_own": [{"word": "anchors", "concept": 3}],
        "personas": [{"word": "good-peer", "concept": 1}],
        "inherit": [{"word": "anchors", "from": "anchor", "concepts": [1, 2]}],
    }
    report = corrections.apply_corrections(database, plan, root / "datasets")
    corrected = Path(report["path"])
    assert digest(database) == before_digest
    with sqlite3.connect(f"file:{corrected}?mode=ro", uri=True) as connection:
        actual = corrections.table_counts(connection)
        assert actual == report["after"]
        assert report["delta"] == {table: actual[table] - report["before"][table] for table in corrections.TABLES}
        assert connection.execute("PRAGMA foreign_key_check").fetchall() == []
    index = sense_lookup.SenseIndex.load(corrected, is_topic_split=True)
    derived = index.senses_of("anchors")
    assert {c for s in derived for c in s.concepts} == {1, 2}
    assert {s.field for s in derived} == {"files-io"}
    assert 3 in {c for s in index.senses_of("anchor") for c in s.concepts}, "source's other valid meaning must remain"
    result = sense_lookup.lookup("anchors", [], index, is_exact_first=True, is_inflection_evidence_used=False)
    words = {w for s in result["senses"] for ring in ("direct", "near", "same_topic") for w in s[ring]}
    assert {"anchor", "io-peer", "good-peer"} <= words
    assert not {"bad-peer", "unreviewed"} & words
    corrected_digest = digest(corrected)
    try:
        corrections.apply_corrections(database, plan, root / "datasets")
    except FileExistsError:
        pass
    else:
        raise AssertionError("correction build overwrote an existing version")
    assert digest(corrected) == corrected_digest
    return {"pass": True, "selective_own_and_input_inheritance": True, "topic_primary_field_reassigned": True,
            "unrelated_source_meanings_preserved": True, "corrected_lookup_verified": True, "table_deltas_verified": True}


def check_correction_failure_safety(root: Path) -> dict:
    database = correction_fixture(root)
    before_digest = digest(database)
    base_plan = {"schema_version": corrections.CORRECTION_SCHEMA, "base_dataset": "fixture-base"}
    plans = [
        {**base_plan, "remove_own": [{"word": "anchor", "concept": 3}]},
        {**base_plan, "inherit": [{"word": "anchors", "from": "anchor", "concepts": [99999]}]},
        {**base_plan, "moves": [{"concept": 1, "from_topic": "x3", "to_topic": "x2"}]},
    ]
    for number, plan in enumerate(plans):
        output = root / f"failure-{number}"
        try:
            corrections.apply_corrections(database, plan, output)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid correction was accepted")
        assert digest(database) == before_digest
        assert not list(output.glob("*/build-report.json"))
        failures = list(output.glob("*/failed-build.json"))
        assert len(failures) == 1
        assert not (failures[0].parent / "dataset.sqlite3").exists()
        pending = failures[0].parent / ".dataset.sqlite3.pending"
        with sqlite3.connect(pending.resolve().as_uri() + "?mode=ro", uri=True) as connection:
            assert dict(connection.execute("SELECT key,value FROM sense_meta"))["dataset_id"] == "fixture-base"
    return {"pass": True, "dependent_link_deletion_refused": True, "unknown_inheritance_refused": True,
            "stale_move_refused": True, "failed_versions_not_marked_complete": True, "original_preserved": True}


def check_correction_publication_safety(root: Path, is_additions: bool = False) -> dict:
    database = fixture(root / "base")
    with sqlite3.connect(database) as connection:
        connection.execute("DROP TABLE input_senses")
        connection.executescript(builder.INPUT_SENSES_SQL)
        connection.execute("PRAGMA journal_mode=WAL")
    original_digest = digest(database)
    plan = {"schema_version": corrections.CORRECTION_SCHEMA, "base_dataset": "fixture-base",
            "personas": [{"word": "new-peer", "concept": 1}]}
    entries = root / "entries.json"
    entries.write_text(json.dumps({"personas": plan["personas"]}))
    real_connect = sqlite3.connect
    real_open = open if is_additions else Path.open
    real_rename = builder.os.rename if is_additions else Path.rename

    def apply(output):
        if is_additions:
            return builder.add_entries(arguments(database, entries, output))
        return corrections.apply_corrections(database, plan, output)

    class VacuumFailure(sqlite3.Connection):
        def execute(self, sql, parameters=()):
            if sql == "VACUUM":
                raise sqlite3.OperationalError("injected VACUUM failure after commit")
            return super().execute(sql, parameters)

    def connect(*args, **kwargs):
        if not kwargs.get("uri"):
            kwargs["factory"] = VacuumFailure
        return real_connect(*args, **kwargs)

    def open_report(path, *args, **kwargs):
        if Path(path).name == "build-report.json":
            raise OSError("injected report publication failure")
        return real_open(path, *args, **kwargs)

    def rename_database(path, target):
        if Path(target).name == "dataset.sqlite3":
            raise OSError("injected database publication failure")
        return real_rename(path, target)

    for failure, injection in [
        ("vacuum", patch.object(corrections.sqlite3, "connect", side_effect=connect)),
        ("report", patch("builtins.open", open_report) if is_additions else patch.object(Path, "open", open_report)),
        ("publish", patch.object(builder.os, "rename", rename_database) if is_additions else patch.object(Path, "rename", rename_database)),
    ]:
        output = root / failure
        with injection:
            try:
                apply(output)
            except (sqlite3.OperationalError, OSError):
                pass
            else:
                raise AssertionError(f"{failure} failure was not propagated")
        directory, = output.iterdir()
        published = directory / "dataset.sqlite3"
        assert not published.exists(), f"{failure}: failed version is publicly readable"
        assert (directory / "failed-build.json").is_file()
        assert not (directory / "build-report.json").exists()
        try:
            sense_lookup.SenseIndex.load(published)
        except SystemExit as error:
            assert str(error) == f"sense dataset unavailable: {published}"
        else:
            raise AssertionError(f"{failure}: failed version was accepted by lookup")
        try:
            corrections.apply_corrections(directory, plan, root / f"from-{failure}")
        except FileNotFoundError:
            pass
        else:
            raise AssertionError(f"{failure}: failed version was accepted as a base")
        assert digest(database) == original_digest

    report = apply(root / "success")
    published = Path(report["path"])
    index = sense_lookup.SenseIndex.load(published)
    result = sense_lookup.lookup("anchor", [], index, is_inflection_evidence_used=False)
    assert "new-peer" in sense_lookup.render_text([result])
    assert json.loads((published.parent / "build-report.json").read_text()) == report
    assert not (published.parent / ".dataset.sqlite3.pending").exists()
    assert digest(database) == original_digest
    return {"pass": True, "post_commit_failures_not_published": ["vacuum", "report", "publish"],
            "failed_lookup_and_base_rejected": True, "wal_source_published_standalone": True,
            "successful_lookup_verified": True, "original_preserved": True}


def main() -> int:
    results = []
    with tempfile.TemporaryDirectory(prefix="verify-sense-additions-") as directory:
        for name, check in [("repeated-extension", check_repeated_extension), ("existing-version", check_existing_version),
                            ("lookup-invariants", check_lookup_invariants),
                            ("unused-match-exclusion", check_unused_match_exclusion),
                            ("reviewed-corrections", check_reviewed_corrections),
                            ("correction-failure-safety", check_correction_failure_safety),
                            ("correction-publication-safety", check_correction_publication_safety),
                            ("additions-publication-safety", lambda root: check_correction_publication_safety(root, is_additions=True))]:
            root = Path(directory) / name
            root.mkdir()
            try:
                result = check(root)
            except (Exception, SystemExit) as error:
                result = {"pass": False, "error": f"{type(error).__name__}: {error}"}
            results.append({"case": name, **result})
    print(json.dumps({"checks": results, "pass": all(r["pass"] for r in results)}, indent=2))
    return 0 if all(r["pass"] for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
