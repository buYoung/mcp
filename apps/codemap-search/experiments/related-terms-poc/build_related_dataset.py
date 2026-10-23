"""Build the `rt-related/1` dataset read by `rtpoc3 related` (internal experiment).

Inputs, merged into one flat term/relation table:
- thesaurus-v1 LLM relations (`relations/*.jsonl`, generated per head term from the
  52-category vocabulary in `contracts/thesaurus-categories.json`; `*.codex.jsonl` rows
  get their own source),
- the round-4 v2 curated dataset and the original `rt-dataset/1` dataset (antonyms excluded).

Every term gets a jina-embeddings-v2-base-code vector (unit length, int8-quantized) so the
lookup can rank candidates by cosine similarity without running a model. Keys follow
`rtk-1`; `lookup-v3/src/related_keys.rs` implements the same rules and
`rtpoc3 related-check` verifies them. Filtering of noisy relations is deliberately left
for after the PoC.

Run with the embedding venv (fastembed + numpy):
  .codemap/poc/related-terms/tools/embed-venv/bin/python build_related_dataset.py \\
      --thesaurus .codemap/poc/related-terms/thesaurus-v1 \\
      --v2-dataset <v2 dataset.sqlite3> --original-dataset <original dataset.sqlite3> \\
      --models .codemap/poc/related-terms/tools/models --output-root <dir>
"""

from __future__ import annotations

import argparse
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys
import time

SCHEMA_VERSION = "rt-related/1"
KEYS_VERSION = "rtk-1"
LLM_SOURCE = "llm-thesaurus-v1"
# Batches finished by Codex after the Claude runs were stopped (`relations/*.codex.jsonl`).
LLM_CODEX_SOURCE = "llm-thesaurus-v1-codex"
EMBEDDING_MODEL = "jinaai/jina-embeddings-v2-base-code"
VECTOR_DIMENSIONS = 768
QUANTIZATION = "int8:round(127*unit_vector)"
EXCLUDED_RELATION_KINDS = {"antonym"}
# rt-dataset/1 kinds folded into one name per meaning.
RELATION_KIND_NAMES = {
    "abbreviation_expansion": "abbreviation",
    "abbreviation_of": "abbreviation",
}
LLM_KINDS = {"synonym", "abbreviation", "form", "family", "near", "identifier"}
CAMEL_NAME = re.compile(r"^[A-Za-z][A-Za-z0-9]*$")

SCHEMA_SQL = """
CREATE TABLE related_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE sources (source TEXT PRIMARY KEY, license_spdx TEXT NOT NULL, description TEXT NOT NULL);
CREATE TABLE terms (
    term_id INTEGER PRIMARY KEY,
    surface TEXT NOT NULL,
    exact_key TEXT NOT NULL UNIQUE,
    match_key TEXT NOT NULL,
    joined_key TEXT NOT NULL,
    is_protected INTEGER NOT NULL CHECK (is_protected IN (0, 1)),
    is_atomic INTEGER NOT NULL CHECK (is_atomic IN (0, 1)),
    vector BLOB
);
CREATE INDEX terms_by_match_key ON terms(match_key) WHERE match_key <> '';
CREATE INDEX terms_by_joined_key ON terms(joined_key) WHERE joined_key <> '';
CREATE TABLE relations (
    from_term INTEGER NOT NULL REFERENCES terms(term_id),
    to_term INTEGER NOT NULL REFERENCES terms(term_id),
    kind TEXT NOT NULL,
    source TEXT NOT NULL REFERENCES sources(source),
    PRIMARY KEY (from_term, to_term, kind, source)
) WITHOUT ROWID;
CREATE INDEX relations_by_to_term ON relations(to_term);
"""


def exact_key(surface: str) -> str:
    """Lowercase, collapse whitespace runs, trim (same as rtn-2 `exact_key`)."""
    return " ".join(surface.lower().split())


def is_protected(key: str) -> bool:
    """Symbols carry meaning: a leading dot (`.net`, `.csv`) or `+`/`#` (`c++`, `c#`)."""
    has_alphanumeric = any(c.isascii() and c.isalnum() for c in key)
    return has_alphanumeric and (key.startswith(".") or "+" in key or "#" in key)


def term_keys(key: str) -> tuple[str, str]:
    """`(match_key, joined_key)`: alphanumeric runs joined by a space / concatenated."""
    if not key.isascii() or is_protected(key):
        return "", ""
    words = [word for word in re.split(r"[^a-z0-9]+", key) if word]
    return " ".join(words), "".join(words)


def is_camel_name(surface: str) -> bool:
    """One alphanumeric run with an inner capital (`GitHub`, `JavaScript`, `macOS`)."""
    return bool(CAMEL_NAME.match(surface)) and any(c.isupper() for c in surface[1:]) and any(c.islower() for c in surface)


class TermTable:
    """Terms keyed by exact key; the first surface seen wins (sources are added in priority order)."""

    def __init__(self) -> None:
        self.surface_by_key: dict[str, str] = {}
        self.atomic_keys: set[str] = set()

    def add(self, surface: str) -> str | None:
        surface = " ".join(surface.split())
        key = exact_key(surface)
        if not key:
            return None
        self.surface_by_key.setdefault(key, surface)
        return key


def load_llm(thesaurus: str, terms: TermTable, relations: set, report: dict) -> None:
    vocab = json.load(open(os.path.join(thesaurus, "vocab-merged.json")))
    # Atomic names are camel-cased proper nouns from the category vocabulary (`JavaScript`,
    # `GitHub`, `PostgreSQL`); the lookup does not split them into words. Identifiers such as
    # `EventEmitter` or `EventListener` stay splittable even when an LLM listed them as family.
    for head in vocab.values():
        key = terms.add(head["term"])
        if key and "proper_noun" in head["kinds"] and is_camel_name(head["term"].strip()):
            terms.atomic_keys.add(key)
    head_count = pair_count = 0
    for path in sorted(glob.glob(os.path.join(thesaurus, "relations", "*.jsonl"))):
        source = LLM_CODEX_SOURCE if path.endswith(".codex.jsonl") else LLM_SOURCE
        for line in open(path):
            if not line.strip():
                continue
            row = json.loads(line)
            head_key = terms.add(row["term"])
            head_count += 1
            for word, kind in row["related"]:
                if kind not in LLM_KINDS or not word.strip() or not word.isascii():
                    report["llm_rejected_pairs"] = report.get("llm_rejected_pairs", 0) + 1
                    continue
                word_key = terms.add(word)
                if head_key and word_key and head_key != word_key:
                    relations.add((head_key, word_key, kind, source))
                    pair_count += 1
    report["llm_heads"] = head_count
    report["llm_pairs"] = pair_count


def load_rt_dataset(path: str, label: str, terms: TermTable, relations: set, sources: dict, report: dict) -> None:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    for source_id, license_spdx, name in connection.execute("SELECT source_id, license_spdx, name FROM sources"):
        sources.setdefault(source_id, (license_spdx, name))
    for key, surfaces_json in connection.execute("SELECT exact_key, surfaces_json FROM expressions ORDER BY exact_key"):
        surfaces = json.loads(surfaces_json)
        terms.surface_by_key.setdefault(key, surfaces[0] if surfaces else key)
    count = 0
    query = """
        SELECT f.exact_key, t.exact_key, r.relation_kind, r.source_id
        FROM relations r
        JOIN expressions f ON f.expression_id = r.from_expression_id
        JOIN expressions t ON t.expression_id = r.to_expression_id
    """
    for from_key, to_key, kind, source_id in connection.execute(query):
        if kind in EXCLUDED_RELATION_KINDS or from_key == to_key:
            continue
        relations.add((from_key, to_key, RELATION_KIND_NAMES.get(kind, kind), source_id))
        count += 1
    report[f"{label}_relations"] = count


def embed_surfaces(surfaces: list[str], models: str, cache_path: str) -> dict[str, bytes]:
    """Unit-length int8 vectors per surface, cached by (model, surface) across builds."""
    import numpy as np
    from fastembed import TextEmbedding

    cache = sqlite3.connect(cache_path)
    cache.execute("CREATE TABLE IF NOT EXISTS vectors (model TEXT NOT NULL, text TEXT NOT NULL, vector BLOB NOT NULL, PRIMARY KEY (model, text))")
    vectors: dict[str, bytes] = {}
    missing: list[str] = []
    for surface in surfaces:
        row = cache.execute("SELECT vector FROM vectors WHERE model = ? AND text = ?", (EMBEDDING_MODEL, surface)).fetchone()
        if row:
            vectors[surface] = row[0]
        else:
            missing.append(surface)
    if missing:
        model = TextEmbedding(EMBEDDING_MODEL, cache_dir=models)
        started = time.time()
        batch_size = 4096
        for offset in range(0, len(missing), batch_size):
            chunk = missing[offset : offset + batch_size]
            embedded = np.array(list(model.embed(chunk, batch_size=256)), dtype=np.float32)
            embedded /= np.linalg.norm(embedded, axis=1, keepdims=True)
            quantized = np.clip(np.round(embedded * 127), -127, 127).astype(np.int8)
            rows = [(EMBEDDING_MODEL, text, vector.tobytes()) for text, vector in zip(chunk, quantized)]
            cache.executemany("INSERT OR REPLACE INTO vectors VALUES (?, ?, ?)", rows)
            cache.commit()
            for text, vector in zip(chunk, quantized):
                vectors[text] = vector.tobytes()
            done = offset + len(chunk)
            print(f"embedded {done}/{len(missing)} ({done / (time.time() - started):.0f}/s)", file=sys.stderr)
    return vectors


def build(arguments: argparse.Namespace) -> dict:
    report: dict = {}
    terms = TermTable()
    relations: set[tuple[str, str, str, str]] = set()
    sources: dict[str, tuple[str, str]] = {
        LLM_SOURCE: ("NOASSERTION", "LLM-generated code thesaurus (thesaurus-v1; Claude Sonnet agents, 52 categories)"),
        LLM_CODEX_SOURCE: ("NOASSERTION", "LLM-generated code thesaurus (thesaurus-v1; Codex gpt-6.1-sol xhigh, remaining heads)"),
    }
    load_llm(arguments.thesaurus, terms, relations, report)
    load_rt_dataset(arguments.v2_dataset, "v2", terms, relations, sources, report)
    load_rt_dataset(arguments.original_dataset, "original", terms, relations, sources, report)

    keys = sorted(terms.surface_by_key)
    term_id_by_key = {key: index + 1 for index, key in enumerate(keys)}
    surfaces = sorted({terms.surface_by_key[key] for key in keys})
    vectors = embed_surfaces(surfaces, arguments.models, arguments.embed_cache)

    digest = hashlib.sha256()
    for part in (SCHEMA_VERSION, KEYS_VERSION, EMBEDDING_MODEL, QUANTIZATION):
        digest.update(part.encode() + b"\0")
    for key in keys:
        surface = terms.surface_by_key[key]
        digest.update(f"{key}\t{surface}\t{int(key in terms.atomic_keys)}\n".encode())
        digest.update(vectors[surface])
    sorted_relations = sorted(relations)
    for relation in sorted_relations:
        digest.update(("\t".join(relation) + "\n").encode())
    dataset_id = f"rtr1-{digest.hexdigest()[:16]}"

    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=True)
    database_path = os.path.join(output_dir, "dataset.sqlite3")
    temporary_path = database_path + ".tmp"
    if os.path.exists(temporary_path):
        os.remove(temporary_path)
    connection = sqlite3.connect(temporary_path)
    connection.executescript(SCHEMA_SQL)
    meta = {
        "schema_version": SCHEMA_VERSION,
        "keys_version": KEYS_VERSION,
        "dataset_id": dataset_id,
        "vector_model": EMBEDDING_MODEL,
        "vector_dimensions": str(VECTOR_DIMENSIONS),
        "vector_quantization": QUANTIZATION,
        "excluded_relation_kinds": json.dumps(sorted(EXCLUDED_RELATION_KINDS)),
    }
    connection.executemany("INSERT INTO related_meta VALUES (?, ?)", sorted(meta.items()))
    connection.executemany(
        "INSERT INTO sources VALUES (?, ?, ?)",
        [(source, license_spdx, description) for source, (license_spdx, description) in sorted(sources.items())],
    )
    term_rows = []
    for key in keys:
        surface = terms.surface_by_key[key]
        match_key, joined_key = term_keys(key)
        term_rows.append(
            (term_id_by_key[key], surface, key, match_key, joined_key, int(is_protected(key)), int(key in terms.atomic_keys), vectors[surface])
        )
    connection.executemany("INSERT INTO terms VALUES (?, ?, ?, ?, ?, ?, ?, ?)", term_rows)
    connection.executemany(
        "INSERT INTO relations VALUES (?, ?, ?, ?)",
        [(term_id_by_key[from_key], term_id_by_key[to_key], kind, source) for from_key, to_key, kind, source in sorted_relations],
    )
    connection.commit()
    connection.execute("VACUUM")
    connection.close()
    os.replace(temporary_path, database_path)

    kind_counts: dict[str, int] = {}
    source_counts: dict[str, int] = {}
    for _, _, kind, source in sorted_relations:
        kind_counts[kind] = kind_counts.get(kind, 0) + 1
        source_counts[source] = source_counts.get(source, 0) + 1
    report.update(
        {
            "dataset_id": dataset_id,
            "path": database_path,
            "terms": len(keys),
            "protected_terms": sum(1 for key in keys if is_protected(key)),
            "atomic_terms": len(terms.atomic_keys),
            "relations": len(sorted_relations),
            "relations_by_kind": dict(sorted(kind_counts.items())),
            "relations_by_source": dict(sorted(source_counts.items())),
        }
    )
    with open(os.path.join(output_dir, "build-report.json"), "w") as handle:
        json.dump(report, handle, indent=2)
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--thesaurus", required=True)
    parser.add_argument("--v2-dataset", required=True)
    parser.add_argument("--original-dataset", required=True)
    parser.add_argument("--models", required=True, help="fastembed model cache directory")
    parser.add_argument("--embed-cache", help="vector cache SQLite (default: <thesaurus>/embed-cache.sqlite3)")
    parser.add_argument("--output-root", required=True)
    arguments = parser.parse_args()
    if not arguments.embed_cache:
        arguments.embed_cache = os.path.join(arguments.thesaurus, "embed-cache.sqlite3")
    print(json.dumps(build(arguments), indent=2))


if __name__ == "__main__":
    main()
