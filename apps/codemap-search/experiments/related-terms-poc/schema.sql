-- Generated lookup artifact for the related-terms dataset (schema rt-dataset/1).
-- Every table mirrors one canonical JSONL record type; nested JSON objects are stored as
-- canonical JSON text so `build_dataset.py verify` can rebuild each JSONL line exactly.
-- The file is rebuilt from JSONL and never edited by hand.

CREATE TABLE dataset_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE sources (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    semantic_scope TEXT NOT NULL,
    license_spdx TEXT NOT NULL,
    attribution TEXT NOT NULL,
    upstream TEXT NOT NULL,
    revision_json TEXT NOT NULL,
    archive_sha256 TEXT NOT NULL
);

CREATE TABLE concepts (
    concept_id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(source_id),
    concept_kind TEXT NOT NULL,
    source_record TEXT NOT NULL,
    pos TEXT,
    gloss TEXT,
    domain_labels_json TEXT NOT NULL,
    attributes_json TEXT NOT NULL
);

CREATE TABLE expressions (
    expression_id TEXT PRIMARY KEY,
    exact_key TEXT NOT NULL UNIQUE,
    token_key TEXT NOT NULL,
    token_key_lossy INTEGER NOT NULL CHECK (token_key_lossy IN (0, 1)),
    token_count INTEGER NOT NULL,
    surfaces_json TEXT NOT NULL
);

CREATE TABLE lexicalizations (
    lexicalization_id TEXT PRIMARY KEY,
    concept_id TEXT NOT NULL REFERENCES concepts(concept_id),
    expression_id TEXT NOT NULL REFERENCES expressions(expression_id),
    surface TEXT NOT NULL,
    role TEXT NOT NULL,
    source_record TEXT NOT NULL,
    rank INTEGER NOT NULL
);

CREATE TABLE relations (
    relation_id TEXT PRIMARY KEY,
    relation_kind TEXT NOT NULL,
    direction TEXT NOT NULL,
    evidence_kind TEXT NOT NULL,
    source_id TEXT NOT NULL REFERENCES sources(source_id),
    from_expression_id TEXT NOT NULL REFERENCES expressions(expression_id),
    to_expression_id TEXT NOT NULL REFERENCES expressions(expression_id),
    from_lexicalization_id TEXT NOT NULL REFERENCES lexicalizations(lexicalization_id),
    to_lexicalization_id TEXT NOT NULL REFERENCES lexicalizations(lexicalization_id),
    from_concept_id TEXT NOT NULL REFERENCES concepts(concept_id),
    to_concept_id TEXT NOT NULL REFERENCES concepts(concept_id)
);

-- Query-key indexes used by the serving reader.
CREATE INDEX expressions_by_token_key ON expressions(token_key) WHERE token_key_lossy = 0;
CREATE INDEX relations_by_from_expression ON relations(from_expression_id, relation_kind);
CREATE INDEX lexicalizations_by_expression ON lexicalizations(expression_id);
CREATE INDEX lexicalizations_by_concept ON lexicalizations(concept_id);
