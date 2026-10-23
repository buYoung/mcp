"""Finite, reviewed relation copies and the input-scoped rt-sense/3 extension.

Source and recipient identities are checked in the unedited base by the caller.
No words, memberships, spelling variants or recursive expansions are inferred.
"""
from collections import defaultdict

from term_canonical import word_key

LEGACY_SCHEMA = "rt-sense/2"
INPUT_RELATIONS_SCHEMA = "rt-sense/3"
SUPPORTED_SCHEMAS = (LEGACY_SCHEMA, INPUT_RELATIONS_SCHEMA)
RELATION_KINDS = frozenset(("synonym", "near", "identifier", "co_use", "abbreviation"))
INPUT_COLUMNS = ("input_word_key", "source_persona", "related_word_key", "surface", "kind", "rank")


def has_input_relations(connection):
    return connection.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='input_relations'").fetchone() is not None


def create_input_relations(connection):
    """One transactional DDL statement; never executescript's implicit commit."""
    connection.execute("""CREATE TABLE input_relations (
        input_word_key TEXT NOT NULL REFERENCES terms(word_key) ON DELETE RESTRICT,
        source_persona INTEGER NOT NULL,
        related_word_key TEXT NOT NULL,
        surface TEXT NOT NULL,
        kind TEXT NOT NULL,
        rank INTEGER NOT NULL CHECK(rank >= 0),
        PRIMARY KEY(input_word_key, source_persona, related_word_key),
        FOREIGN KEY(input_word_key, source_persona) REFERENCES input_senses(word_key, persona) ON DELETE RESTRICT
    )""")


def validate_dataset_layout(connection, schema):
    """Old readers reject /3; new readers reject hidden or malformed extensions."""
    if schema not in SUPPORTED_SCHEMAS:
        raise ValueError(f"unsupported sense schema: {schema!r}")
    present = has_input_relations(connection)
    if schema == LEGACY_SCHEMA:
        if present:
            raise ValueError("input_relations requires rt-sense/3")
        return
    if not present:
        raise ValueError("rt-sense/3 requires input_relations")
    columns = connection.execute("PRAGMA table_info(input_relations)").fetchall()
    expected = list(zip(INPUT_COLUMNS, ("TEXT", "INTEGER", "TEXT", "TEXT", "TEXT", "INTEGER"), (1, 2, 3, 0, 0, 0)))
    if [(r[1], r[2].upper(), r[5]) for r in columns] != expected or any(r[3] != 1 for r in columns):
        raise ValueError("incompatible input_relations columns/primary key")
    foreign = defaultdict(list)
    for row in connection.execute("PRAGMA foreign_key_list(input_relations)"):
        if row[5] not in ("NO ACTION", "RESTRICT") or row[6] not in ("NO ACTION", "RESTRICT"):
            raise ValueError("input_relations must not cascade")
        foreign[row[0]].append((row[1], row[2], row[3], row[4]))
    targets = {tuple((table, source, target) for _, table, source, target in sorted(rows)) for rows in foreign.values()}
    if targets != {(('terms', 'input_word_key', 'word_key'),),
                   (('input_senses', 'input_word_key', 'word_key'), ('input_senses', 'source_persona', 'persona'))}:
        raise ValueError("incompatible input_relations foreign keys")
    if connection.execute("PRAGMA foreign_key_check(input_relations)").fetchone():
        raise ValueError("dangling input_relations binding")
    if connection.execute(
        "SELECT 1 FROM input_relations r LEFT JOIN personas p ON p.persona=r.source_persona "
        "LEFT JOIN terms t ON t.term_id=p.term_id LEFT JOIN senses s ON s.sense=p.sense "
        "LEFT JOIN concepts c ON c.concept=p.concept LEFT JOIN topic_fields tf ON tf.topic=c.topic AND tf.rank=0 "
        "WHERE p.persona IS NULL OR t.term_id IS NULL OR s.sense IS NULL OR tf.field IS NULL "
        "OR p.term_id IS NOT s.term_id OR s.field IS NOT tf.field LIMIT 1"
    ).fetchone():
        raise ValueError("invalid input relation classification owner/field")
    for key, persona, related, surface, kind, rank in connection.execute("SELECT * FROM input_relations"):
        if (not all(isinstance(v, str) and v.strip() for v in (key, related, surface))
                or type(persona) is not int or type(rank) is not int or rank < 0 or kind not in RELATION_KINDS):
            raise ValueError("invalid input_relations payload")


def validate_relation_shape(entry):
    if not isinstance(entry, dict) or set(entry) != {"source", "target", "kind", "rank"}:
        raise ValueError("relation_additions requires source, target, kind, rank")
    source, target = entry["source"], entry["target"]
    if not isinstance(source, dict) or set(source) != {"term_id", "head", "persona", "concept", "topic", "rank", "word_key", "surface", "kind"}:
        raise ValueError("relation source requires the complete reviewed row identity")
    if not isinstance(target, dict) or set(target) != {"word", "head", "persona", "concept", "topic", "origin"}:
        raise ValueError("relation target requires word/head/persona/concept/topic/origin")
    for obj, integer_keys, text_keys in [(source, ("term_id", "persona", "concept", "rank"), ("head", "topic", "word_key", "surface", "kind")),
                                         (target, ("persona", "concept"), ("word", "head", "topic"))]:
        if any(type(obj[k]) is not int for k in integer_keys):
            raise ValueError("relation IDs/rank must be integers")
        if any(not isinstance(obj[k], str) or not obj[k].strip() for k in text_keys):
            raise ValueError("relation strings must be nonempty")
    if (type(entry["rank"]) is not int or entry["rank"] < 0 or source["rank"] < 0
            or not isinstance(entry["kind"], str) or entry["kind"] not in RELATION_KINDS):
        raise ValueError("invalid target relation kind/rank")
    if target["origin"] not in ("own", "input") or source["concept"] != target["concept"]:
        raise ValueError("reviewed relation requires own/input target in the same concept")
    if any(not word_key(v) for v in (source["head"], target["head"], target["word"])):
        raise ValueError("relation requires nonempty canonical head/word keys")


def relation_identity(entry):
    target = entry["target"]
    return (target["origin"], word_key(target["word"]),
            target["persona"] if target["origin"] == "input" else None, entry["source"]["word_key"])


def checked_persona(connection, persona, head, concept, topic):
    row = connection.execute(
        "SELECT p.term_id,t.word_key,p.concept,c.topic,s.term_id,s.field,tf.field "
        "FROM personas p JOIN terms t USING(term_id) JOIN concepts c USING(concept) "
        "JOIN senses s USING(sense) JOIN topic_fields tf ON tf.topic=c.topic AND tf.rank=0 WHERE p.persona=?", (persona,),
    ).fetchone()
    if (row is None or row[1:4] != (word_key(head), concept, topic) or row[0] != row[4] or row[5] != row[6]):
        raise ValueError(f"stale/invalid relation persona: {persona}/{head}/{concept}/{topic}")
    return row[0]


def validate_relation(connection, entry):
    source, target = entry["source"], entry["target"]
    term = checked_persona(connection, source["persona"], source["head"], source["concept"], source["topic"])
    actual = connection.execute("SELECT surface,kind,rank,persona FROM relations WHERE term_id=? AND word_key=?",
                                (source["term_id"], source["word_key"])).fetchone()
    if term != source["term_id"] or actual != (source["surface"], source["kind"], source["rank"], source["persona"]):
        raise ValueError("stale/unknown donor relation row")
    owner = checked_persona(connection, target["persona"], target["head"], target["concept"], target["topic"])
    row = connection.execute("SELECT term_id FROM terms WHERE word_key=?", (word_key(target["word"]),)).fetchone()
    if row is None:
        raise ValueError("relation target word must already exist")
    if target["origin"] == "own":
        if row[0] != owner:
            raise ValueError("relation recipient does not own the target word")
    elif not connection.execute("SELECT 1 FROM input_senses WHERE word_key=? AND persona=?",
                                (word_key(target["word"]), target["persona"])).fetchone():
        raise ValueError("relation recipient lacks the reviewed input binding")
    return row[0]


def validate_relation_preconditions(connection, plan):
    removed = {(word_key(r["word"]), r["concept"]) for r in plan.get("remove_own", [])}
    removed_input = {(word_key(r["word"]), r["concept"]) for r in plan.get("remove_input", [])}
    moved = {r["concept"] for r in plan.get("moves", [])}
    for entry in plan.get("relation_additions", []):
        validate_relation(connection, entry)
        source, target = entry["source"], entry["target"]
        if ((word_key(source["head"]), source["concept"]) in removed
                or (word_key(target["head"]), target["concept"]) in removed
                or source["concept"] in moved or target["concept"] in moved
                or (target["origin"] == "input" and (word_key(target["word"]), target["concept"]) in removed_input)):
            raise ValueError("reviewed relation conflicts with planned source/recipient removal or movement")


def add_reviewed_relation(connection, entry):
    """Insert only the exact donor key/literal; keep the separately reviewed kind/rank."""
    term = validate_relation(connection, entry)
    source, target = entry["source"], entry["target"]
    if target["origin"] == "own":
        table = "relations"
        where = "term_id=? AND word_key=?"
        identity = (term, source["word_key"])
        actual = connection.execute(f"SELECT surface,kind,rank,persona FROM {table} WHERE {where}", identity).fetchone()
        expected = (source["surface"], entry["kind"], entry["rank"], target["persona"])
        rank_where, rank_args = "term_id=? AND rank=?", (term, entry["rank"])
        values = (term, source["word_key"], *expected)
        columns = "term_id,word_key,surface,kind,rank,persona"
    else:
        table = "input_relations"
        where = "input_word_key=? AND source_persona=? AND related_word_key=?"
        identity = (word_key(target["word"]), target["persona"], source["word_key"])
        actual = connection.execute(f"SELECT surface,kind,rank FROM {table} WHERE {where}", identity).fetchone()
        expected = (source["surface"], entry["kind"], entry["rank"])
        rank_where, rank_args = "input_word_key=? AND source_persona=? AND rank=?", (*identity[:2], entry["rank"])
        values = (*identity, *expected)
        columns = ",".join(INPUT_COLUMNS)
    if actual is not None:
        if actual != expected:
            raise ValueError(f"conflicting {table} relation at {identity}")
        return table, False
    if connection.execute(f"SELECT 1 FROM {table} WHERE {rank_where}", rank_args).fetchone():
        raise ValueError(f"conflicting {table} target rank")
    connection.execute(f"INSERT INTO {table} ({columns}) VALUES (?,?,?,?,?,?)", values)
    return table, True
