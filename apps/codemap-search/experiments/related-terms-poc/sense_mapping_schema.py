"""The `rt-sense/4` layout: a word maps to topics, and a concept inside the topic is optional.

Compared with rt-sense/3:
- `personas` holds (word, topic, concept or NULL). One row per word, topic and concept; a NULL concept maps the word
  to the topic alone. `source_persona_count` keeps how many generated personas the row stands for; a word's senses
  are ordered by it, as rt-sense/3 ordered them by persona rows. A topic mapped by review alone has 0: its sense
  follows the generated and bound senses, and does not stop the word from taking its lemma (exact-first).
- `senses` is gone. A persona's field is its topic's primary field (`topic_fields.rank = 0`), so moving a topic to
  another field moves its words with it.
- `relations`, `input_senses` and `input_relations` are unchanged and still point at personas.
"""

from reviewed_relations import RELATION_KINDS, create_input_relations

MAPPING_SCHEMA = "rt-sense/4"

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
CREATE TABLE personas (
    persona INTEGER PRIMARY KEY,
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    topic TEXT NOT NULL REFERENCES topics(topic),
    concept INTEGER REFERENCES concepts(concept),
    source_persona_count INTEGER NOT NULL CHECK (source_persona_count >= 0)
);
CREATE UNIQUE INDEX personas_by_mapping ON personas(term_id, topic, ifnull(concept, -1));
CREATE INDEX personas_by_topic ON personas(topic);
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
CREATE TABLE input_senses (
    word_key TEXT NOT NULL,
    persona INTEGER NOT NULL REFERENCES personas(persona),
    PRIMARY KEY (word_key, persona)
) WITHOUT ROWID;
CREATE VIEW concept_members AS
    SELECT DISTINCT c.concept, c.name, c.topic, t.surface, t.term_id
    FROM personas AS p JOIN concepts AS c USING (concept) JOIN terms AS t USING (term_id);
CREATE VIEW topic_members AS
    SELECT DISTINCT p.topic, tp.label, t.surface, t.term_id
    FROM personas AS p JOIN topics AS tp USING (topic) JOIN terms AS t USING (term_id);
CREATE VIEW term_topics AS
    SELECT t.surface, tf.field, p.topic, tp.label AS topic_label, c.name AS concept
    FROM personas AS p JOIN terms AS t USING (term_id) JOIN topics AS tp USING (topic)
    JOIN topic_fields AS tf ON tf.topic = p.topic AND tf.rank = 0 LEFT JOIN concepts AS c USING (concept);
"""


def create_schema(connection):
    """Statement by statement, so the caller's transaction is not committed by executescript."""
    for statement in SCHEMA_SQL.split(";\n"):
        if statement.strip():
            connection.execute(statement)
    create_input_relations(connection)


def validate_mapping_layout(connection):
    """Reject a dataset whose rows break the rt-sense/4 rules; raise ValueError naming the first broken rule."""
    tables = {name for (name,) in connection.execute("SELECT name FROM sqlite_master WHERE type = 'table'")}
    if "senses" in tables or not {"personas", "input_senses", "input_relations"} <= tables:
        raise ValueError("rt-sense/4 has personas, input_senses and input_relations and no senses table")
    columns = [row[1] for row in connection.execute("PRAGMA table_info(personas)")]
    if columns != ["persona", "term_id", "topic", "concept", "source_persona_count"]:
        raise ValueError(f"rt-sense/4 personas columns: {columns}")
    checks = {
        "topic without exactly one primary field":
            "SELECT 1 FROM topics tp WHERE (SELECT count(*) FROM topic_fields tf WHERE tf.topic = tp.topic AND tf.rank = 0) <> 1",
        "persona concept in another topic":
            "SELECT 1 FROM personas p JOIN concepts c USING (concept) WHERE c.topic <> p.topic",
        "concept without personas":
            "SELECT 1 FROM concepts c WHERE NOT EXISTS (SELECT 1 FROM personas p WHERE p.concept = c.concept)",
    }
    for rule, query in checks.items():
        if connection.execute(query + " LIMIT 1").fetchone():
            raise ValueError(f"rt-sense/4: {rule}")
    if connection.execute("PRAGMA foreign_key_check").fetchone():
        raise ValueError("rt-sense/4: dangling reference")
    for key, persona, related, surface, kind, rank in connection.execute("SELECT * FROM input_relations"):
        if (not all(isinstance(v, str) and v.strip() for v in (key, related, surface))
                or type(persona) is not int or type(rank) is not int or rank < 0 or kind not in RELATION_KINDS):
            raise ValueError("rt-sense/4: invalid input_relations payload")
