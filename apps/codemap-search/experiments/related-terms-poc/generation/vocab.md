# Vocabulary generation (thesaurus-v1, stage A)

Goal: build a broad, top-down vocabulary of software-development terms. Later, every term gets a list of related words (synonyms, abbreviations, word forms, near words) so that a code-search tool can expand an LLM's search terms. Breadth matters more than precision here: filtering happens later.

Category definitions: `apps/codemap-search/experiments/related-terms-poc/contracts/thesaurus-categories.json` (relative to the repository root)

For EACH category assigned to you, write 150 to 300 distinct terms that developers actually use in code, identifiers, comments, docs, or code-search queries for that category. Cover all of these kinds:

- word: single English words used in code for that category, including nouns (listener, mutex), verbs used in function names (emit, fetch, parse, evict), and adjectives (async, stale, idle).
- phrase: multi-word concepts written with single spaces in lower case (event loop, race condition, connection pool).
- abbreviation: short forms developers write (ctx, cfg, req, res, msg, k8s, i18n, auth, db).
- proper_noun: well-known tools, libraries, frameworks, protocols, products and standards in that category, in their usual casing (GitHub, PostgreSQL, Redis, gRPC, OAuth).
- extension: file extensions with the leading dot (.csv, .yaml, .lock).
- identifier: well-known API or identifier names written as in code (addEventListener, setTimeout, EventEmitter, useState). Keep these to widely known names.

Rules:
- ASCII only. Skip any term that needs a non-ASCII character.
- No made-up terms, no typos, no numbering.
- Duplicates inside one category are not allowed; overlap with other categories is fine.

Output: one file per category at
{vocab_dir}/<category-id>.jsonl
Each line is one JSON object: {"term": "...", "kind": "word|phrase|abbreviation|proper_noun|extension|identifier", "category": "<category-id>"}

Write files with a Bash heredoc or the Write tool. If a file is long, write it in several appends. After writing, validate each file with:
python3 -c "import json,sys; rows=[json.loads(l) for l in open(sys.argv[1])]; assert all(r['term'].isascii() for r in rows); print(len(rows), len({r['term'] for r in rows}))" <file>
Fix any problem, then reply with one line per category: "<category-id>: <count>".
