"""Build the `rt-sense/2` dataset read by `sense_lookup.py` (internal experiment).

Inputs, in the sense-graph directory written by `induce_sense_graph.py`:
- `personas.jsonl`, `concepts.jsonl`: word personas and the concepts and topics they were clustered into,
- `names/topics-*.jsonl`: one line per automatic topic, naming it or splitting it into named groups of
  concepts when it mixes subjects (`{"topic": 10, "groups": [{"label", "scope", "concepts": [..]}]}`),
- `names/field-list.json`: the fields (`{"fields": [{"field", "label", "scope"}]}`), and
  `names/field-assign-*.json`: the 1-3 fields of every named topic, first = primary (`{"t10": ["field-id", ...]}`).

A word's senses are the primary fields its personas reach (persona → concept → topic → first field);
a topic's further fields are kept in `topic_fields`. Every related word a word was generated with
(`relations/*.jsonl`, all kinds) is kept in `relations`, with the persona whose group holds it; `form` links and
words of personas left out of the graph have no persona. The 52 generation categories are kept only in
`term_legacy_categories` for comparison.

Subcommands (run from the repository root):
  carry-names   name the topics that continue a previous named graph (`--previous`): names/topics-carried.jsonl,
                names/field-assign-carried.json, field-list.json; the rest go to naming/open-topics-<n>.jsonl
  check-topics  validate names/topics-*.jsonl (optionally one shard)
  topic-labels  write naming/topic-labels.jsonl (field namer) and naming/topic-labels-<n>.jsonl (field assigners)
  check-fields  validate names/field-list.json and names/field-assign-*.json (optionally one shard)
  build         write datasets/rts2-<hash>/dataset.sqlite3
  add-input-senses  copy a built dataset (`--base`) into a new one that adds `input_senses` (no `--graph` needed)
  add-entries   copy a built dataset (`--base`) into a new one that adds the words, senses, concepts, topics and
                fields listed in `--entries` (no `--graph` needed)
  python3 apps/codemap-search/experiments/related-terms-poc/build_sense_dataset.py build \\
      --graph .codemap/poc/related-terms/thesaurus-v1/sense-graph-1 \\
      --thesaurus .codemap/poc/related-terms/thesaurus-v1 \\
      --contract apps/codemap-search/experiments/related-terms-poc/contracts/thesaurus-categories.json \\
      --output-root .codemap/poc/related-terms/thesaurus-v1/datasets
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import glob
import hashlib
import json
import os
import re
import shutil
import sqlite3
import sys

from build_category_dataset import canonical_terms, relation_rows
from reviewed_relations import INPUT_RELATIONS_SCHEMA, SUPPORTED_SCHEMAS, validate_dataset_layout
from term_canonical import canonical_surface, word_key

SCHEMA_VERSION = "rt-sense/2"
FIELD_ID = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")
MIN_FIELDS, MAX_FIELDS = 15, 45
MAX_FIELDS_PER_TOPIC = 3
ASSIGN_SHARDS = 3
NAMING_SHARDS = 3
CARRY_CONTAINMENT = 0.6
# Relation kinds that count as evidence for a topic a word with senses of its own has none in (`--head-topic-evidence`).
HEAD_TOPIC_KINDS = ["abbreviation", "co_use", "identifier", "near", "synonym"]

SCHEMA_SQL = """
CREATE TABLE sense_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE terms (term_id INTEGER PRIMARY KEY, surface TEXT NOT NULL, word_key TEXT NOT NULL UNIQUE, kinds TEXT NOT NULL);
CREATE TABLE fields (field TEXT PRIMARY KEY, label TEXT NOT NULL, scope TEXT NOT NULL);
CREATE TABLE topics (topic TEXT PRIMARY KEY, label TEXT NOT NULL, scope TEXT NOT NULL, source_topic INTEGER NOT NULL);
CREATE TABLE topic_fields (
    topic TEXT NOT NULL REFERENCES topics(topic),
    field TEXT NOT NULL REFERENCES fields(field),
    rank INTEGER NOT NULL,
    PRIMARY KEY (topic, field)
) WITHOUT ROWID;
CREATE TABLE concepts (concept INTEGER PRIMARY KEY, name TEXT NOT NULL, topic TEXT NOT NULL REFERENCES topics(topic));
CREATE TABLE senses (
    sense INTEGER PRIMARY KEY,
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    field TEXT NOT NULL REFERENCES fields(field),
    UNIQUE (term_id, field)
);
CREATE TABLE personas (
    persona INTEGER PRIMARY KEY,
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    concept INTEGER NOT NULL REFERENCES concepts(concept),
    sense INTEGER NOT NULL REFERENCES senses(sense)
);
CREATE INDEX personas_by_term ON personas(term_id);
CREATE INDEX personas_by_concept ON personas(concept);
CREATE TABLE relations (
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    rank INTEGER NOT NULL,
    word_key TEXT NOT NULL,
    surface TEXT NOT NULL,
    kind TEXT NOT NULL,
    persona INTEGER REFERENCES personas(persona),
    PRIMARY KEY (term_id, word_key)
) WITHOUT ROWID;
CREATE INDEX relations_by_word ON relations(word_key);
CREATE TABLE term_legacy_categories (term_id INTEGER NOT NULL REFERENCES terms(term_id), category TEXT NOT NULL, PRIMARY KEY (term_id, category)) WITHOUT ROWID;
CREATE VIEW concept_members AS
    SELECT DISTINCT c.concept, c.name, c.topic, t.surface, t.term_id
    FROM personas AS p JOIN concepts AS c USING (concept) JOIN terms AS t USING (term_id);
CREATE VIEW topic_members AS
    SELECT DISTINCT c.topic, tp.label, t.surface, t.term_id
    FROM personas AS p JOIN concepts AS c USING (concept) JOIN topics AS tp ON tp.topic = c.topic JOIN terms AS t USING (term_id);
CREATE VIEW term_senses AS
    SELECT t.surface, s.sense, s.field, f.label AS field_label,
           (SELECT group_concat(DISTINCT tp.label) FROM personas AS p JOIN concepts AS c USING (concept) JOIN topics AS tp ON tp.topic = c.topic WHERE p.sense = s.sense) AS topics,
           (SELECT group_concat(DISTINCT c.name) FROM personas AS p JOIN concepts AS c USING (concept) WHERE p.sense = s.sense) AS concepts
    FROM senses AS s JOIN terms AS t USING (term_id) JOIN fields AS f USING (field);
"""


def read_jsonl(path):
    with open(path) as handle:
        return [json.loads(line) for line in handle if line.strip()]


def load_graph(graph):
    concepts = {c["concept"]: c for c in read_jsonl(os.path.join(graph, "concepts.jsonl"))}
    topics = defaultdict(list)
    for concept in concepts.values():
        topics[concept["topic"]].append(concept["concept"])
    return concepts, dict(topics)


def topic_name_errors(rows, topics):
    """Every automatic topic is named once; its concepts are split over groups without loss or overlap."""
    errors, seen = [], set()
    for row in rows:
        topic = row.get("topic")
        if topic not in topics:
            errors.append(f"unknown topic {topic!r}")
            continue
        if topic in seen:
            errors.append(f"topic {topic} named twice")
        seen.add(topic)
        placed = []
        for group in row.get("groups", []):
            if not group.get("label") or not group.get("scope"):
                errors.append(f"topic {topic}: every group needs a label and a scope")
            placed += group.get("concepts", [])
        if sorted(placed) != sorted(topics[topic]):
            errors.append(f"topic {topic}: groups hold {sorted(placed)}, expected each of {sorted(topics[topic])} exactly once")
    return errors, seen


def named_topics(graph, topics, shard=None):
    rows, errors = [], []
    pattern = f"topics-{shard}.jsonl" if shard else "topics-*.jsonl"
    for path in sorted(glob.glob(os.path.join(graph, "names", pattern))):
        rows += read_jsonl(path)
    problems, seen = topic_name_errors(rows, topics)
    errors += problems
    if not shard:
        missing = sorted(set(topics) - seen)
        if missing:
            errors.append(f"{len(missing)} topics not named, e.g. {missing[:10]}")
    named = {}
    for row in rows:
        groups = row.get("groups", [])
        for index, group in enumerate(groups):
            topic_id = f"t{row['topic']}" if len(groups) == 1 else f"t{row['topic']}.{index + 1}"
            named[topic_id] = {"label": group.get("label", ""), "scope": group.get("scope", ""), "source_topic": row["topic"],
                               "concepts": group.get("concepts", [])}
    return named, errors


def load_fields(graph, shard=None):
    document = json.load(open(os.path.join(graph, "names", "field-list.json")))
    pattern = f"field-assign-{shard}.json" if shard else "field-assign-*.json"
    assigned = {}
    for path in sorted(glob.glob(os.path.join(graph, "names", pattern))):
        assigned.update(json.load(open(path)))
    return {"fields": document.get("fields", []), "topics": assigned}


def shard_topics(graph, shard):
    return {row["topic"] for row in read_jsonl(os.path.join(graph, "naming", f"topic-labels-{shard}.jsonl"))}


def field_errors(document, named, partial=False):
    errors = []
    fields = {f.get("field"): f for f in document.get("fields", [])}
    if not MIN_FIELDS <= len(fields) <= MAX_FIELDS:
        errors.append(f"{len(fields)} fields, expected {MIN_FIELDS}-{MAX_FIELDS}")
    for field_id, field in fields.items():
        if not field_id or not FIELD_ID.fullmatch(field_id):
            errors.append(f"bad field id {field_id!r}")
        if not field.get("label") or not field.get("scope"):
            errors.append(f"field {field_id}: label and scope are required")
    assigned = document.get("topics", {})
    for topic_id in named:
        chosen = assigned.get(topic_id)
        if not chosen:
            errors.append(f"topic {topic_id} has no field")
        elif len(chosen) > MAX_FIELDS_PER_TOPIC or len(set(chosen)) != len(chosen):
            errors.append(f"topic {topic_id}: 1-{MAX_FIELDS_PER_TOPIC} distinct fields, got {chosen}")
        elif unknown := [f for f in chosen if f not in fields]:
            errors.append(f"topic {topic_id}: unknown fields {unknown}")
    for topic_id in set(assigned) - set(named):
        errors.append(f"unknown topic {topic_id}")
    unused = set(fields) - {f for chosen in assigned.values() for f in chosen}
    if unused and not partial:
        errors.append(f"unused fields {sorted(unused)}")
    return errors


def report(errors, ok_message):
    if errors:
        print(f"{len(errors)} errors")
        for error in errors[:40]:
            print(f"  {error}")
        sys.exit(1)
    print(ok_message)


def carry_names(arguments, concepts, topics):
    """Reuse a previous graph's topic names and fields where every concept of a new topic continues a previous topic.

    A concept continues the previous named topic holding most of its words when that topic holds at least
    CARRY_CONTAINMENT of them; a topic is carried only when all its concepts continue some previous topic, and
    its concepts are grouped by the topic they continue. Other topics are left for the namers.
    """
    old_concepts, old_topics = load_graph(arguments.previous)
    old_named, errors = named_topics(arguments.previous, old_topics)
    report(errors, f"previous graph: {len(old_named)} named topics")
    old_fields = load_fields(arguments.previous)["topics"]
    owners = defaultdict(set)
    for topic_id, topic in old_named.items():
        for concept in topic["concepts"]:
            for word in old_concepts[concept]["words"]:
                owners[word_key(word)].add(topic_id)

    carried, carried_fields, open_topics = [], {}, []
    for topic in sorted(topics):
        groups = {}
        for concept in topics[topic]:
            words = {word_key(w) for w in concepts[concept]["words"]}
            counts = Counter(owner for word in words for owner in owners.get(word, ()))
            best = min(counts.items(), key=lambda kv: (-kv[1], kv[0])) if counts else None
            if not best or best[1] < CARRY_CONTAINMENT * len(words):
                open_topics.append(topic)
                break
            groups.setdefault(best[0], []).append(concept)
        else:
            row = {"topic": topic, "groups": [{"label": old_named[t]["label"], "scope": old_named[t]["scope"], "concepts": members}
                                              for t, members in groups.items()]}
            carried.append(row)
            for index, old_id in enumerate(groups):
                new_id = f"t{topic}" if len(groups) == 1 else f"t{topic}.{index + 1}"
                carried_fields[new_id] = old_fields[old_id]

    names = os.path.join(arguments.graph, "names")
    os.makedirs(names, exist_ok=True)
    with open(os.path.join(names, "topics-carried.jsonl"), "w") as handle:
        handle.writelines(json.dumps(row, ensure_ascii=False) + "\n" for row in carried)
    with open(os.path.join(names, "field-assign-carried.json"), "w") as handle:
        json.dump(carried_fields, handle, ensure_ascii=False, indent=1)
    with open(os.path.join(arguments.previous, "names", "field-list.json")) as source, open(os.path.join(names, "field-list.json"), "w") as target:
        target.write(source.read())
    by_id = {c: concepts[c] for c in concepts}
    for shard in range(NAMING_SHARDS):
        with open(os.path.join(arguments.graph, "naming", f"open-topics-{shard + 1}.jsonl"), "w") as handle:
            for topic in open_topics[shard::NAMING_SHARDS]:
                handle.write(json.dumps({"topic": topic, "concepts": [
                    {"concept": c, "name": by_id[c]["name"], "words": by_id[c]["words"][:12]} for c in topics[topic]]}, ensure_ascii=False) + "\n")
    print(json.dumps({"topics": len(topics), "carried": len(carried), "carried_named_topics": len(carried_fields), "open": len(open_topics)}))


def write_topic_labels(arguments, concepts, named):
    assigned = set(load_fields(arguments.graph)["topics"]) if os.path.exists(os.path.join(arguments.graph, "names", "field-list.json")) else set()
    rows = []
    for topic_id, topic in sorted(named.items(), key=lambda kv: (kv[1]["source_topic"], kv[0])):
        if topic_id in assigned:
            continue
        words = [w for c in topic["concepts"] for w in concepts[c]["words"][:4]]
        rows.append({"topic": topic_id, "label": topic["label"], "scope": topic["scope"], "words": list(dict.fromkeys(words))[:10]})
    targets = {"topic-labels.jsonl": rows, **{f"topic-labels-{n + 1}.jsonl": rows[n::ASSIGN_SHARDS] for n in range(ASSIGN_SHARDS)}}
    for name, subset in targets.items():
        with open(os.path.join(arguments.graph, "naming", name), "w") as handle:
            handle.writelines(json.dumps(row, ensure_ascii=False) + "\n" for row in subset)
        print(f"naming/{name}: {len(subset)} topics")


def read_relations(thesaurus, terms, co_use=None):
    """Related words per word key in generation order, spellings of the head merged, first kind kept; co-use words last."""
    relations = defaultdict(dict)
    for term, related in relation_rows(thesaurus, co_use):
        head = word_key(term)
        if head not in terms:
            continue
        for surface, kind in related:
            key = word_key(surface)
            if key != head and key not in relations[head]:
                relations[head][key] = (terms[key]["surface"] if key in terms else canonical_surface(surface), kind)
    return relations


def build(arguments, concepts, named, fields_document):
    scopes = {row["id"]: row["scope"] for row in json.load(open(arguments.contract))["categories"]}
    terms = canonical_terms(arguments.thesaurus, scopes)
    personas = read_jsonl(os.path.join(arguments.graph, "personas.jsonl"))
    topic_of_concept = {c: topic_id for topic_id, topic in named.items() for c in topic["concepts"]}
    topic_fields = fields_document["topics"]
    fields = {f["field"]: f for f in fields_document["fields"]}

    keys = sorted(terms)
    term_ids = {key: i for i, key in enumerate(keys, 1)}
    senses, persona_rows, persona_of = {}, [], {}
    for persona in sorted(personas, key=lambda p: p["persona"]):
        field = topic_fields[topic_of_concept[persona["concept"]]][0]
        sense = senses.setdefault((persona["word"], field), len(senses) + 1)
        persona_rows.append((persona["persona"], term_ids[persona["word"]], persona["concept"], sense))
        for neighbor in persona["neighbors"]:
            persona_of[(persona["word"], word_key(neighbor))] = persona["persona"]
    relations = read_relations(arguments.thesaurus, terms, arguments.co_use)
    relation_rows = [(term_ids[head], rank, key, surface, kind, persona_of.get((head, key)))
                     for head in sorted(relations) for rank, (key, (surface, kind)) in enumerate(relations[head].items())]

    digest = hashlib.sha256(SCHEMA_VERSION.encode() + b"\0")
    for key in keys:
        digest.update(f"{key}\t{terms[key]['surface']}\n".encode())
    for field_id in sorted(fields):
        digest.update(json.dumps(fields[field_id], sort_keys=True).encode())
    for topic_id in sorted(named):
        digest.update(json.dumps([topic_id, named[topic_id]["label"], named[topic_id]["scope"], named[topic_id]["concepts"], topic_fields[topic_id]]).encode())
    for row in persona_rows:
        digest.update(json.dumps(row).encode())
    for row in relation_rows:
        digest.update(json.dumps(row, ensure_ascii=False).encode())
    dataset_id = f"rts2-{digest.hexdigest()[:16]}"

    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=True)
    database_path = os.path.join(output_dir, "dataset.sqlite3")
    if os.path.exists(database_path):
        os.remove(database_path)
    connection = sqlite3.connect(database_path)
    connection.executescript(SCHEMA_SQL)
    meta = {"schema_version": SCHEMA_VERSION, "dataset_id": dataset_id,
            "key_rule": "term_canonical.word_key", "graph": arguments.graph}
    connection.executemany("INSERT INTO sense_meta VALUES (?, ?)", sorted(meta.items()))
    connection.executemany("INSERT INTO terms VALUES (?, ?, ?, ?)",
                           [(term_ids[k], terms[k]["surface"], k, json.dumps(sorted(terms[k]["kinds"]))) for k in keys])
    connection.executemany("INSERT INTO fields VALUES (?, ?, ?)", [(f, fields[f]["label"], fields[f]["scope"]) for f in sorted(fields)])
    connection.executemany("INSERT INTO topics VALUES (?, ?, ?, ?)",
                           [(t, named[t]["label"], named[t]["scope"], named[t]["source_topic"]) for t in sorted(named)])
    connection.executemany("INSERT INTO topic_fields VALUES (?, ?, ?)",
                           [(t, f, rank) for t in sorted(named) for rank, f in enumerate(topic_fields[t])])
    connection.executemany("INSERT INTO concepts VALUES (?, ?, ?)",
                           [(c, concepts[c]["name"], topic_of_concept[c]) for c in sorted(concepts)])
    connection.executemany("INSERT INTO senses VALUES (?, ?, ?)",
                           [(sense, term_ids[word], field) for (word, field), sense in sorted(senses.items(), key=lambda kv: kv[1])])
    connection.executemany("INSERT INTO personas VALUES (?, ?, ?, ?)", persona_rows)
    connection.executemany("INSERT INTO relations VALUES (?, ?, ?, ?, ?, ?)", relation_rows)
    connection.executemany("INSERT INTO term_legacy_categories VALUES (?, ?)",
                           [(term_ids[k], p) for k in keys for p in sorted(terms[k]["parents"])])
    connection.commit()
    connection.execute("VACUUM")
    connection.close()

    senses_per_word = defaultdict(int)
    for word, _ in senses:
        senses_per_word[word] += 1
    distribution = defaultdict(int)
    for count in senses_per_word.values():
        distribution[count] += 1
    result = {
        "dataset_id": dataset_id, "path": database_path, "schema_version": SCHEMA_VERSION,
        "terms": len(keys), "words_with_senses": len(senses_per_word), "senses": len(senses),
        "senses_per_word": dict(sorted(distribution.items())),
        "personas": len(persona_rows), "concepts": len(concepts), "topics": len(named),
        "relations": len(relation_rows), "relations_without_persona": sum(row[5] is None for row in relation_rows),
        "relations_to_other_words": sum(row[2] not in terms for row in relation_rows),
        "automatic_topics_split": sum(1 for t in named if "." in t and t.endswith(".1")),
        "fields": len(fields), "topics_with_several_fields": sum(len(v) > 1 for v in topic_fields.values()),
    }
    with open(os.path.join(output_dir, "build-report.json"), "w") as handle:
        json.dump(result, handle, indent=2)
        handle.write("\n")
    return result


INPUT_SENSES_SQL = """
CREATE TABLE input_senses (
    word_key TEXT NOT NULL,
    persona INTEGER NOT NULL REFERENCES personas(persona),
    PRIMARY KEY (word_key, persona)
) WITHOUT ROWID;
"""


def head_topic_rows(base, head_rule):
    """(word_key, persona) for words with senses of their own, from the personas of topics they have none in, when at
    least `min_heads` different head words of that topic list the word with one of `relation_kinds`."""
    kinds = head_rule["relation_kinds"]
    own_topics = defaultdict(set)
    for key, topic in base.execute(
            "SELECT t.word_key, c.topic FROM personas AS p JOIN terms AS t USING (term_id) JOIN concepts AS c USING (concept)"):
        own_topics[key].add(topic)
    evidence = defaultdict(lambda: defaultdict(list))
    for key, persona, head, topic in base.execute(
            "SELECT r.word_key, r.persona, p.term_id, c.topic FROM relations AS r JOIN personas AS p ON p.persona = r.persona "
            "JOIN concepts AS c ON c.concept = p.concept WHERE r.kind IN (" + ", ".join("?" for _ in kinds) + ")", kinds):
        if key in own_topics and topic not in own_topics[key]:
            evidence[key][topic].append((persona, head))
    return [(key, persona) for key, topics in evidence.items() for found in topics.values()
            if len({head for _, head in found}) >= head_rule["min_heads"] for persona, _ in found]


def add_input_senses(arguments):
    """A new dataset: `--base` plus `input_senses`, the personas each related word was generated in, for related words
    that have no senses of their own. The rule chooses eligible relation kinds and whether direct words include
    siblings; it is stored in the dataset, not applied as a lookup filter. The word joins no concept.
    With `--head-topic-evidence N`, words with senses of their own also get the personas of topics they have none in,
    when N different head words of the topic list them (`head_topics` in the rule), so `bind` is also read as LDAP bind.
    The base dataset is copied, not modified."""
    is_legacy = arguments.input_sense_rule == "legacy"
    rule = {"relation_kinds": None if is_legacy else ["abbreviation", "identifier", "near", "synonym"],
            "direct": "head_and_siblings" if is_legacy else "head"}
    if arguments.head_topic_evidence:
        rule["head_topics"] = {"relation_kinds": HEAD_TOPIC_KINDS, "min_heads": arguments.head_topic_evidence}
    rule_json = json.dumps(rule, sort_keys=True)
    base_path = os.path.join(arguments.base, "dataset.sqlite3")
    base = sqlite3.connect(f"file:{base_path}?mode=ro", uri=True)
    try:
        meta = dict(base.execute("SELECT key, value FROM sense_meta"))
        if meta.get("schema_version") != SCHEMA_VERSION:
            raise SystemExit(f"sense dataset incompatible: {meta.get('schema_version')!r}, expected {SCHEMA_VERSION}")
        base_id = meta["dataset_id"]
        kinds = rule["relation_kinds"] or []
        kind_clause = " AND r.kind IN (" + ", ".join("?" for _ in kinds) + ")" if kinds else ""
        rows = base.execute(
            "SELECT DISTINCT r.word_key, r.persona FROM relations AS r WHERE r.persona IS NOT NULL AND r.word_key NOT IN "
            "(SELECT t.word_key FROM terms AS t JOIN personas AS p USING (term_id))" + kind_clause +
            " ORDER BY r.word_key, r.persona", kinds
        ).fetchall()
        head_rows = head_topic_rows(base, rule["head_topics"]) if "head_topics" in rule else []
        rows = sorted(set(rows) | set(head_rows))
    finally:
        base.close()
    digest = hashlib.sha256(SCHEMA_VERSION.encode() + b"\0" + base_id.encode() + b"\0input_senses\0")
    if not is_legacy:
        digest.update(rule_json.encode() + b"\0")
    for row in rows:
        digest.update(json.dumps(row, ensure_ascii=False).encode())
    dataset_id = f"rts2-{digest.hexdigest()[:16]}"
    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=True)
    database_path = os.path.join(output_dir, "dataset.sqlite3")
    report_path = os.path.join(output_dir, "build-report.json")
    if os.path.exists(database_path):
        with open(report_path) as handle:
            result = json.load(handle)
        if result["dataset_id"] != dataset_id:
            raise SystemExit(f"sense dataset report incompatible: {report_path}")
        return result
    shutil.copyfile(base_path, database_path)
    connection = sqlite3.connect(database_path)
    connection.execute("DROP TABLE IF EXISTS input_senses")
    connection.executescript(INPUT_SENSES_SQL)
    connection.executemany("INSERT INTO input_senses VALUES (?, ?)", rows)
    connection.executemany("INSERT OR REPLACE INTO sense_meta VALUES (?, ?)", [
        ("dataset_id", dataset_id), ("base_dataset", base_id),
        ("input_senses", "related words without senses of their own, read through the personas they were generated in"
         + ("; words with senses of their own also through the personas of topics they have none in (rule head_topics)"
            if "head_topics" in rule else "")),
        ("input_sense_rule", rule_json),
    ])
    connection.commit()
    connection.execute("VACUUM")
    connection.close()
    result = {"dataset_id": dataset_id, "path": database_path, "schema_version": SCHEMA_VERSION, "base_dataset": base_id,
              "input_sense_words": len({key for key, _ in rows}), "input_sense_rows": len(rows), "input_sense_rule": rule,
              "head_topic_words": len({key for key, _ in head_rows}), "head_topic_rows": len(head_rows)}
    with open(report_path, "w") as handle:
        json.dump(result, handle, indent=2)
        handle.write("\n")
    return result


def add_entries(arguments):
    """A new dataset: `--base` plus the entries of `--entries`. The base dataset is copied, not modified, and nothing
    in it is removed or moved; entries only add rows. The base's rt-sense/2 or rt-sense/3 schema is preserved.

    Entries (JSON):
      fields:   [{"field", "label", "scope"}]
      topics:   [{"key", "label", "scope", "fields": [field, ...]}]   first field = primary; ids become `x1`, `x2`, ...
      concepts: [{"key", "name", "topic": existing topic id or new topic key}]
      personas: [{"word", "concept": existing concept id or new concept key}]
      inherit:  [{"word", "from"}]   the word joins every concept `from` has a persona in, and gets `from`'s input senses
    A persona's sense is the word's sense in the primary field of the concept's topic, as in `build`.
    """
    entries = json.load(open(arguments.entries))
    base_path = os.path.join(arguments.base, "dataset.sqlite3")
    base = sqlite3.connect(f"file:{base_path}?mode=ro", uri=True)
    try:
        meta = dict(base.execute("SELECT key, value FROM sense_meta"))
        schema_version = meta.get("schema_version")
        if schema_version not in SUPPORTED_SCHEMAS:
            raise SystemExit(f"sense dataset incompatible: {schema_version!r}, expected {SUPPORTED_SCHEMAS}")
        if schema_version == INPUT_RELATIONS_SCHEMA:
            try:
                validate_dataset_layout(base, schema_version)
            except (ValueError, sqlite3.DatabaseError) as error:
                raise SystemExit(f"sense dataset layout incompatible: {error}") from error
    finally:
        base.close()
    entries_json = json.dumps(entries, ensure_ascii=False, sort_keys=True)
    digest = hashlib.sha256(schema_version.encode() + b"\0" + meta["dataset_id"].encode() + b"\0entries\0" + entries_json.encode())
    prefix = "rts3" if schema_version == INPUT_RELATIONS_SCHEMA else "rts2"
    dataset_id = f"{prefix}-{digest.hexdigest()[:16]}"
    output_dir = os.path.join(arguments.output_root, dataset_id)
    try:
        os.makedirs(output_dir, exist_ok=False)
    except FileExistsError:
        raise SystemExit(f"dataset output already exists; preserving it: {output_dir}")
    database_path = os.path.join(output_dir, "dataset.sqlite3")
    pending_path = os.path.join(output_dir, ".dataset.sqlite3.pending")
    report_path = os.path.join(output_dir, "build-report.json")
    try:
        shutil.copyfile(base_path, pending_path)
        connection = sqlite3.connect(pending_path)
        try:
            connection.execute("PRAGMA journal_mode=DELETE")
            added = insert_entries(connection, entries)
            connection.executemany("INSERT OR REPLACE INTO sense_meta VALUES (?, ?)", [
                ("dataset_id", dataset_id), ("base_dataset", meta["dataset_id"]),
                ("entries", os.path.abspath(arguments.entries)),
            ])
            connection.commit()
            connection.execute("VACUUM")
        finally:
            connection.close()
        result = {"dataset_id": dataset_id, "path": database_path, "schema_version": schema_version,
                  "base_dataset": meta["dataset_id"], "entries": os.path.abspath(arguments.entries), **added}
        with open(report_path, "x") as handle:
            json.dump(result, handle, indent=2, ensure_ascii=False)
            handle.write("\n")
        os.rename(pending_path, database_path)
    except BaseException as error:
        try:
            if os.path.exists(report_path):
                os.rename(report_path, os.path.join(output_dir, "unpublished-build-report.json"))
            with open(os.path.join(output_dir, "failed-build.json"), "x") as handle:
                json.dump({"status": "failed", "base_dataset": meta["dataset_id"], "error": str(error)}, handle, indent=2)
        except OSError as reporting_error:
            raise error from reporting_error
        raise
    return result


def insert_entries(connection, entries):
    """Insert entry rows; the caller owns the transaction and final publication."""
    def next_id(table, column):
        return (connection.execute(f"SELECT max({column}) FROM {table}").fetchone()[0] or 0) + 1

    fields = {f for (f,) in connection.execute("SELECT field FROM fields")}
    new_fields = [f for f in entries.get("fields", []) if f["field"] not in fields]
    connection.executemany("INSERT INTO fields VALUES (?, ?, ?)", [(f["field"], f["label"], f["scope"]) for f in new_fields])
    fields |= {f["field"] for f in new_fields}

    primary_field = dict(connection.execute("SELECT topic, field FROM topic_fields WHERE rank = 0"))
    topic_ids = {}
    used_topic_ids = {topic for (topic,) in connection.execute("SELECT topic FROM topics")}
    number = 1
    for topic in entries.get("topics", []):
        unknown = [f for f in topic["fields"] if f not in fields]
        if unknown or not 1 <= len(topic["fields"]) <= MAX_FIELDS_PER_TOPIC:
            raise SystemExit(f"topic {topic['key']}: bad fields {topic['fields']}")
        while f"x{number}" in used_topic_ids:
            number += 1
        topic_id = topic_ids[topic["key"]] = f"x{number}"
        used_topic_ids.add(topic_id)
        number += 1
        connection.execute("INSERT INTO topics VALUES (?, ?, ?, ?)", (topic_id, topic["label"], topic["scope"], -1))
        connection.executemany("INSERT INTO topic_fields VALUES (?, ?, ?)", [(topic_id, f, rank) for rank, f in enumerate(topic["fields"])])
        primary_field[topic_id] = topic["fields"][0]

    concept_topic = dict(connection.execute("SELECT concept, topic FROM concepts"))
    concept_ids, concept_id = {}, next_id("concepts", "concept")
    for concept in entries.get("concepts", []):
        topic_id = topic_ids.get(concept["topic"], concept["topic"])
        if topic_id not in primary_field:
            raise SystemExit(f"concept {concept['key']}: unknown topic {concept['topic']!r}")
        concept_ids[concept["key"]] = concept_id
        concept_topic[concept_id] = topic_id
        connection.execute("INSERT INTO concepts VALUES (?, ?, ?)", (concept_id, concept["name"], topic_id))
        concept_id += 1

    term_ids = dict(connection.execute("SELECT word_key, term_id FROM terms"))
    sense_ids = {(term, field): sense for sense, term, field in connection.execute("SELECT sense, term_id, field FROM senses")}
    memberships = set(connection.execute("SELECT term_id, concept FROM personas"))
    counts = Counter()

    def term_of(word):
        key = word_key(word)
        if key not in term_ids:
            term_ids[key] = next_id("terms", "term_id")
            connection.execute("INSERT INTO terms VALUES (?, ?, ?, ?)", (term_ids[key], canonical_surface(word), key, json.dumps(["added"])))
            counts["terms"] += 1
        return term_ids[key]

    def add_persona(word, concept):
        term = term_of(word)
        if (term, concept) in memberships:
            counts["personas_already_present"] += 1
            return
        field = primary_field[concept_topic[concept]]
        if (term, field) not in sense_ids:
            sense_ids[(term, field)] = next_id("senses", "sense")
            connection.execute("INSERT INTO senses VALUES (?, ?, ?)", (sense_ids[(term, field)], term, field))
            counts["senses"] += 1
        connection.execute("INSERT INTO personas VALUES (?, ?, ?, ?)", (next_id("personas", "persona"), term, concept, sense_ids[(term, field)]))
        memberships.add((term, concept))
        counts["personas"] += 1

    for persona in entries.get("personas", []):
        concept = concept_ids.get(persona["concept"], persona["concept"])
        if concept not in concept_topic:
            raise SystemExit(f"{persona['word']}: unknown concept {persona['concept']!r}")
        add_persona(persona["word"], concept)
    unresolved = []
    for entry in entries.get("inherit", []):
        source = word_key(entry["from"])
        concepts = [c for (c,) in connection.execute(
            "SELECT DISTINCT p.concept FROM personas p JOIN terms t USING (term_id) WHERE t.word_key = ? ORDER BY p.concept", (source,))]
        inputs = [p for (p,) in connection.execute("SELECT persona FROM input_senses WHERE word_key = ?", (source,))]
        if not concepts and not inputs:
            unresolved.append(entry)
            continue
        for concept in concepts:
            add_persona(entry["word"], concept)
        if inputs:
            term_of(entry["word"])
            before = connection.total_changes
            connection.executemany("INSERT OR IGNORE INTO input_senses VALUES (?, ?)", [(word_key(entry["word"]), p) for p in inputs])
            counts["input_senses"] += connection.total_changes - before
        counts["inherited_words"] += 1

    return {"new_fields": len(new_fields), "new_topics": len(topic_ids), "new_concepts": len(concept_ids),
            **dict(counts), "inherit_unresolved": unresolved}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=("carry-names", "check-topics", "topic-labels", "check-fields", "build", "add-input-senses",
                                            "add-entries"))
    parser.add_argument("--graph", help="required by every command but add-input-senses")
    parser.add_argument("--base", help="add-input-senses / add-entries: the dataset directory to extend")
    parser.add_argument("--entries", help="add-entries: JSON of fields, topics, concepts, personas and inherit entries to add")
    parser.add_argument("--input-sense-rule", choices=("legacy", "identity-heads-only"), default="identity-heads-only",
                        help="add-input-senses: identity-heads-only keeps abbreviation/identifier/near/synonym personas "
                             "and only their head words as direct words; legacy restores all personas and siblings")
    parser.add_argument("--head-topic-evidence", type=int,
                        help="add-input-senses: also give words with senses of their own the personas of topics they have "
                             "none in, when at least this many head words of the topic list them")
    parser.add_argument("--shard", type=int, help="check-topics / check-fields: only that shard")
    parser.add_argument("--thesaurus")
    parser.add_argument("--contract")
    parser.add_argument("--output-root")
    parser.add_argument("--previous", help="carry-names: a previous graph directory with names/")
    parser.add_argument("--co-use", help="co-use relation directory (batches/co-use-*.jsonl), stored as kind `co_use`")
    arguments = parser.parse_args()
    if arguments.command == "add-input-senses":
        if not (arguments.base and arguments.output_root):
            parser.error("add-input-senses needs --base and --output-root")
        print(json.dumps(add_input_senses(arguments), indent=2))
        return
    if arguments.command == "add-entries":
        if not (arguments.base and arguments.entries and arguments.output_root):
            parser.error("add-entries needs --base, --entries and --output-root")
        print(json.dumps(add_entries(arguments), indent=2, ensure_ascii=False))
        return
    if not arguments.graph:
        parser.error(f"{arguments.command} needs --graph")
    concepts, topics = load_graph(arguments.graph)
    if arguments.command == "carry-names":
        carry_names(arguments, concepts, topics)
        return
    named, errors = named_topics(arguments.graph, topics, arguments.shard if arguments.command == "check-topics" else None)
    if arguments.command == "check-topics":
        report(errors, f"ok: {len(named)} named topics from {len(topics) if not arguments.shard else 'shard'} automatic topics")
        return
    report(errors, f"topics ok: {len(named)} named topics")
    if arguments.command == "topic-labels":
        write_topic_labels(arguments, concepts, named)
        return
    fields_document = load_fields(arguments.graph, arguments.shard if arguments.command == "check-fields" else None)
    scope = {t: named[t] for t in shard_topics(arguments.graph, arguments.shard)} if arguments.command == "check-fields" and arguments.shard else named
    report(field_errors(fields_document, scope, partial=scope is not named), f"fields ok: {len(fields_document['fields'])} fields, {len(scope)} topics assigned")
    if arguments.command == "build":
        print(json.dumps(build(arguments, concepts, named, fields_document), indent=2))


if __name__ == "__main__":
    main()
