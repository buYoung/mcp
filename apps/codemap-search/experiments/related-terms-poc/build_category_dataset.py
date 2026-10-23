"""Build the `rt-category/2` dataset read by `category_lookup.py` (internal experiment).

Inputs:
- thesaurus-v1 `vocab-merged.json`: one entry per generated spelling with every category it was generated under,
- `contracts/thesaurus-categories.json`: the 52 parent category ids and scopes,
- a subdivision directory (`subdivide_categories.py`): `parents/<parent>.json` splitting each parent into
  narrow subcategories and dropping words that have no sense in that parent.

Spellings that differ only by symbol decorations (`$state`, `@Component`, `todo!`, `.json`) or by case and
inner separators (`AbortSignal`, `abort signal`) are merged into one word by `term_canonical.word_key`, so the
dataset never stores them separately; the merged spellings are listed in `build-report.json`. Lookup categories are the subcategories; the parent
categories remain as `parent_categories` / `term_parents` for provenance. Tags outside the contract
(`repo-common`, a collection-source mark) are kept in `term_source_tags` and are not categories.

Run from the repository root:
  python3 apps/codemap-search/experiments/related-terms-poc/build_category_dataset.py \\
      --thesaurus .codemap/poc/related-terms/thesaurus-v1 \\
      --contract apps/codemap-search/experiments/related-terms-poc/contracts/thesaurus-categories.json \\
      --subdivision .codemap/poc/related-terms/thesaurus-v1/subdivision-1 \\
      --output-root .codemap/poc/related-terms/thesaurus-v1/datasets
"""

from __future__ import annotations

import argparse
from collections import defaultdict
import glob
import hashlib
import json
import os
import re
import sqlite3

from term_canonical import canonical_surface, word_key

SCHEMA_VERSION = "rt-category/2"
MAX_SUBCATEGORIES_PER_WORD = 2
MIN_SUBCATEGORY_SIZE = 3
MAX_SUBCATEGORY_SIZE = 60
CATCH_ALL_SLUGS = {"misc", "miscellaneous", "other", "others", "general", "common", "etc"}

SCHEMA_SQL = """
CREATE TABLE category_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE parent_categories (parent TEXT PRIMARY KEY, scope TEXT NOT NULL);
CREATE TABLE categories (
    category TEXT PRIMARY KEY,
    parent TEXT NOT NULL REFERENCES parent_categories(parent),
    label TEXT NOT NULL,
    scope TEXT NOT NULL
);
CREATE TABLE terms (
    term_id INTEGER PRIMARY KEY,
    surface TEXT NOT NULL,
    word_key TEXT NOT NULL UNIQUE,
    kinds TEXT NOT NULL
);
CREATE TABLE term_categories (
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    category TEXT NOT NULL REFERENCES categories(category),
    PRIMARY KEY (term_id, category)
) WITHOUT ROWID;
CREATE INDEX term_categories_by_category ON term_categories(category, term_id);
CREATE TABLE term_parents (
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    parent TEXT NOT NULL REFERENCES parent_categories(parent),
    PRIMARY KEY (term_id, parent)
) WITHOUT ROWID;
CREATE TABLE dropped_memberships (
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    parent TEXT NOT NULL REFERENCES parent_categories(parent),
    reason TEXT NOT NULL,
    PRIMARY KEY (term_id, parent)
) WITHOUT ROWID;
CREATE TABLE term_source_tags (
    term_id INTEGER NOT NULL REFERENCES terms(term_id),
    tag TEXT NOT NULL,
    PRIMARY KEY (term_id, tag)
) WITHOUT ROWID;
CREATE VIEW category_members AS
    SELECT tc.category, c.parent, t.surface, t.term_id
    FROM term_categories AS tc JOIN terms AS t USING (term_id) JOIN categories AS c USING (category);
CREATE VIEW term_category_list AS
    SELECT t.term_id, t.surface,
           (SELECT count(*) FROM term_categories AS tc WHERE tc.term_id = t.term_id) AS category_count,
           (SELECT group_concat(category, ',') FROM
                (SELECT category FROM term_categories AS tc WHERE tc.term_id = t.term_id ORDER BY category)) AS categories
    FROM terms AS t;
CREATE VIEW category_sizes AS
    SELECT c.category, c.parent, c.label, count(tc.term_id) AS terms
    FROM categories AS c LEFT JOIN term_categories AS tc USING (category)
    GROUP BY c.category;
"""


def canonical_terms(thesaurus: str, scopes: dict) -> dict:
    """Merge generated spellings into one word per `word_key`.

    The stored surface is an undecorated spelling (else the stripped form of a decorated one), preferring
    the spelling generated under the most categories, then a spaced phrase.
    """
    vocab = json.load(open(os.path.join(thesaurus, "vocab-merged.json")))
    terms = {}
    for row in sorted(vocab.values(), key=lambda r: r["term"]):
        term = terms.setdefault(word_key(row["term"]), {"spellings": {}, "kinds": set(), "parents": set(), "tags": set()})
        term["spellings"][row["term"]] = len(row["categories"])
        term["kinds"].update(row["kinds"])
        term["parents"].update(c for c in row["categories"] if c in scopes)
        term["tags"].update(c for c in row["categories"] if c not in scopes)
    for term in terms.values():
        spellings = term["spellings"]
        bare = [s for s in spellings if canonical_surface(s) == s] or [canonical_surface(s) for s in spellings]
        term["surface"] = min(bare, key=lambda s: (-spellings.get(s, 0), " " not in s, s))
        term["spellings"] = sorted(s for s in spellings if s != term["surface"])
    return terms


def relation_rows(thesaurus, co_use=None):
    """`(head, [(surface, kind)])` from the generated relations, then from `co_use` batches (kind `co_use`) when given."""
    for path in sorted(glob.glob(os.path.join(thesaurus, "relations", "*.jsonl"))):
        with open(path) as handle:
            for line in handle:
                row = json.loads(line)
                yield row["term"], row["related"]
    if co_use:
        for path in sorted(glob.glob(os.path.join(co_use, "batches", "co-use-*.jsonl"))):
            with open(path) as handle:
                for line in handle:
                    row = json.loads(line)
                    yield row["word"], [(surface, "co_use") for surface in row["co_used"]]


def parent_words(terms: dict, parent: str) -> dict:
    return {key: term["surface"] for key, term in terms.items() if parent in term["parents"]}


def subdivision_errors(document: dict, parent: str, words: dict) -> list[str]:
    """Every word of the parent goes to 1-2 subcategories or is dropped with a reason; sizes stay narrow."""
    errors = []
    if document.get("parent") != parent:
        errors.append(f"parent is {document.get('parent')!r}, expected {parent!r}")
    id_pattern = re.compile(rf"{re.escape(parent)}/[a-z0-9]+(-[a-z0-9]+)*")
    placed, seen_ids = defaultdict(list), set()
    for subcategory in document.get("subcategories", []):
        category = subcategory.get("id", "")
        if not id_pattern.fullmatch(category):
            errors.append(f"bad id {category!r}: use {parent}/<kebab-slug>")
        if category in seen_ids:
            errors.append(f"duplicate id {category}")
        seen_ids.add(category)
        if category.rsplit("/", 1)[-1] in CATCH_ALL_SLUGS:
            errors.append(f"{category}: catch-all subcategories are not allowed")
        if not subcategory.get("label") or not subcategory.get("scope"):
            errors.append(f"{category}: label and scope are required")
        members = [word_key(m) for m in subcategory.get("members", [])]
        if not MIN_SUBCATEGORY_SIZE <= len(members) <= MAX_SUBCATEGORY_SIZE:
            errors.append(f"{category}: {len(members)} members, allowed {MIN_SUBCATEGORY_SIZE}-{MAX_SUBCATEGORY_SIZE}")
        for member, key in zip(subcategory.get("members", []), members):
            if key not in words:
                errors.append(f"{category}: {member!r} is not a word of {parent}")
            elif category in placed[key]:
                errors.append(f"{category}: {member!r} listed twice")
            else:
                placed[key].append(category)
    dropped = {}
    for row in document.get("dropped", []):
        key = word_key(row.get("term", ""))
        if key not in words:
            errors.append(f"dropped {row.get('term')!r} is not a word of {parent}")
        elif not row.get("reason"):
            errors.append(f"dropped {row.get('term')!r} needs a reason")
        else:
            dropped[key] = row["reason"]
    for key, surface in sorted(words.items()):
        if key in dropped and placed.get(key):
            errors.append(f"{surface!r} is both dropped and placed in {placed[key]}")
        elif key not in dropped and not placed.get(key):
            errors.append(f"{surface!r} is not placed")
        elif len(placed.get(key, ())) > MAX_SUBCATEGORIES_PER_WORD:
            errors.append(f"{surface!r} is in {len(placed[key])} subcategories, allowed {MAX_SUBCATEGORIES_PER_WORD}")
    return errors


def read_subdivision(subdivision: str, terms: dict, scopes: dict):
    categories, memberships, dropped, errors = {}, defaultdict(set), {}, []
    for parent in sorted(scopes):
        path = os.path.join(subdivision, "parents", f"{parent}.json")
        if not os.path.exists(path):
            errors.append(f"{parent}: missing {path}")
            continue
        document = json.load(open(path))
        problems = subdivision_errors(document, parent, parent_words(terms, parent))
        errors.extend(f"{parent}: {p}" for p in problems)
        if problems:
            continue
        for subcategory in document["subcategories"]:
            categories[subcategory["id"]] = (parent, subcategory["label"], subcategory["scope"])
            for member in subcategory["members"]:
                memberships[word_key(member)].add(subcategory["id"])
        for row in document.get("dropped", []):
            dropped[(word_key(row["term"]), parent)] = row["reason"]
    if errors:
        raise SystemExit("subdivision invalid:\n  " + "\n  ".join(errors[:40]))
    return categories, memberships, dropped


def build(arguments: argparse.Namespace) -> dict:
    contract = json.load(open(arguments.contract))["categories"]
    scopes = {row["id"]: row["scope"] for row in contract}
    terms = canonical_terms(arguments.thesaurus, scopes)
    categories, memberships, dropped = read_subdivision(arguments.subdivision, terms, scopes)

    digest = hashlib.sha256(SCHEMA_VERSION.encode() + b"\0")
    for parent in sorted(scopes):
        digest.update(f"{parent}\t{scopes[parent]}\n".encode())
    for category in sorted(categories):
        digest.update(("\t".join([category, *categories[category]]) + "\n").encode())
    for key in sorted(terms):
        term = terms[key]
        fields = [key, term["surface"], ",".join(sorted(term["kinds"])), ",".join(sorted(term["parents"])),
                  ",".join(sorted(term["tags"])), ",".join(sorted(memberships.get(key, ())))]
        digest.update("\t".join(fields).encode() + b"\n")
    for (key, parent), reason in sorted(dropped.items()):
        digest.update(f"{key}\t{parent}\t{reason}\n".encode())
    dataset_id = f"rtc2-{digest.hexdigest()[:16]}"

    output_dir = os.path.join(arguments.output_root, dataset_id)
    os.makedirs(output_dir, exist_ok=True)
    database_path = os.path.join(output_dir, "dataset.sqlite3")
    if os.path.exists(database_path):
        os.remove(database_path)
    connection = sqlite3.connect(database_path)
    connection.executescript(SCHEMA_SQL)
    meta = {
        "schema_version": SCHEMA_VERSION,
        "dataset_id": dataset_id,
        "key_rule": "term_canonical.word_key: symbol decorations stripped, lowercase, inner separators removed",
        "non_category_tags": json.dumps(sorted({t for term in terms.values() for t in term["tags"]})),
    }
    connection.executemany("INSERT INTO category_meta VALUES (?, ?)", sorted(meta.items()))
    connection.executemany("INSERT INTO parent_categories VALUES (?, ?)", sorted(scopes.items()))
    connection.executemany("INSERT INTO categories VALUES (?, ?, ?, ?)", [(c, *categories[c]) for c in sorted(categories)])
    term_ids = {}
    for term_id, key in enumerate(sorted(terms), 1):
        term = terms[key]
        term_ids[key] = term_id
        connection.execute("INSERT INTO terms VALUES (?, ?, ?, ?)", (term_id, term["surface"], key, json.dumps(sorted(term["kinds"]))))
        connection.executemany("INSERT INTO term_categories VALUES (?, ?)", [(term_id, c) for c in sorted(memberships.get(key, ()))])
        connection.executemany("INSERT INTO term_parents VALUES (?, ?)", [(term_id, p) for p in sorted(term["parents"])])
        connection.executemany("INSERT INTO term_source_tags VALUES (?, ?)", [(term_id, t) for t in sorted(term["tags"])])
    connection.executemany(
        "INSERT INTO dropped_memberships VALUES (?, ?, ?)",
        [(term_ids[key], parent, reason) for (key, parent), reason in sorted(dropped.items())],
    )
    connection.commit()
    connection.execute("VACUUM")
    connection.close()

    sizes = sorted(sum(c in m for m in memberships.values()) for c in categories)
    categorized = [key for key in terms if memberships.get(key)]
    report = {
        "dataset_id": dataset_id,
        "path": database_path,
        "schema_version": SCHEMA_VERSION,
        "parent_categories": len(scopes),
        "categories": len(categories),
        "category_size": {"min": sizes[0], "median": sizes[len(sizes) // 2], "max": sizes[-1]},
        "terms": len(terms),
        "categorized_terms": len(categorized),
        "single_category_terms": sum(len(memberships[key]) == 1 for key in categorized),
        "term_categories": sum(len(m) for m in memberships.values()),
        "term_parents": sum(len(t["parents"]) for t in terms.values()),
        "dropped_memberships": len(dropped),
        "terms_with_only_source_tags": sum(not memberships.get(key) and bool(t["tags"]) for key, t in terms.items()),
        "merged_spellings": {terms[key]["surface"]: terms[key]["spellings"] for key in sorted(terms) if terms[key]["spellings"]},
        "inputs": {
            "vocab_merged": os.path.join(arguments.thesaurus, "vocab-merged.json"),
            "contract": arguments.contract,
            "subdivision": arguments.subdivision,
        },
    }
    with open(os.path.join(output_dir, "build-report.json"), "w") as handle:
        json.dump(report, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--thesaurus", required=True)
    parser.add_argument("--contract", required=True)
    parser.add_argument("--subdivision", required=True)
    parser.add_argument("--output-root", required=True)
    report = build(parser.parse_args())
    print(json.dumps({k: v for k, v in report.items() if k != "merged_spellings"}, indent=2))


if __name__ == "__main__":
    main()
