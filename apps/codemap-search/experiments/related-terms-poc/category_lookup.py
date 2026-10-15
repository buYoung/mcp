"""Category lookup PoC: input word -> its categories -> every word in those categories, one hop only.

Reads two SQLite datasets only: `rt-category/2` (built by `build_category_dataset.py`) for the narrow
subcategories, and `rt-related/1` through `rtpoc3 related` for input resolution (case, separators,
joined forms, lemmas). The categories of returned words are read only for the overlap score;
they never add candidates. Input and matched spellings are keyed by `term_canonical.word_key`, so
`$state`, `todo!`, `.json` or `AbortSignal`/`abort signal` resolve to the one stored word.
"""

import argparse
from collections import defaultdict
from dataclasses import dataclass, field
from functools import lru_cache
import json
from pathlib import Path
import re
import sqlite3
import subprocess
import sys
import tempfile

from syntax_parts import is_syntax_part
from term_canonical import canonical_surface, word_key


REPO = Path(__file__).resolve().parents[4]
DATASETS = REPO / ".codemap/poc/related-terms/thesaurus-v1/datasets"
DEFAULT_CATEGORY_DATASET = DATASETS / "rtc2-80e77d8fb9ef2b41/dataset.sqlite3"
DEFAULT_RTPOC3 = Path(__file__).resolve().parent / "lookup-v3/target/release/rtpoc3"
DEFAULT_DATASET = DATASETS / "rtr1-930791c83bea6132/dataset.sqlite3"

SCHEMA = "rt-category-lookup/1"
CATEGORY_SCHEMA = "rt-category/2"
INFLECTION_MATCHES = ("lemma", "joined_lemma")
PLAIN_WORD = re.compile(r"[A-Za-z][A-Za-z ]*")


@dataclass
class Vocabulary:
    surfaces: dict
    tags: dict
    members: dict
    scopes: dict

    @classmethod
    def load(cls, dataset_path):
        path = Path(dataset_path)
        if not path.exists():
            raise SystemExit(f"category dataset unavailable: {path}")
        connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        try:
            meta = dict(connection.execute("SELECT key, value FROM category_meta"))
            if meta.get("schema_version") != CATEGORY_SCHEMA:
                raise SystemExit(f"category dataset incompatible: {meta.get('schema_version')!r}, expected {CATEGORY_SCHEMA}")
            scopes = dict(connection.execute("SELECT category, scope FROM categories"))
            surfaces = dict(connection.execute("SELECT word_key, surface FROM terms"))
            grouped, members = defaultdict(set), defaultdict(set)
            for term_key, category in connection.execute(
                "SELECT t.word_key, tc.category FROM term_categories AS tc JOIN terms AS t USING (term_id)"
            ):
                grouped[term_key].add(category)
                members[category].add(term_key)
        finally:
            connection.close()
        tags = {term_key: frozenset(grouped.get(term_key, ())) for term_key in surfaces}
        return cls(surfaces, tags, dict(members), scopes)


@dataclass
class Seed:
    span: str
    terms: list
    match: str
    categories: frozenset


@dataclass
class Resolution:
    mode: str
    seeds: list = field(default_factory=list)
    matched_keys: set = field(default_factory=set)


@dataclass
class InflectionEvidence:
    dataset_path: Path
    aliases: dict
    forms: dict = field(default_factory=dict)
    related: dict = field(default_factory=dict)

    @classmethod
    @lru_cache(maxsize=4)
    def load(cls, dataset_path):
        """Input spellings listed by their base, with explicit `form` links read in both directions."""
        path = Path(dataset_path)
        if not path.exists():
            raise SystemExit(f"inflection dataset unavailable: {path}")
        connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        try:
            # Protected spellings such as `.settings` have no joined key, but may share a word_key.
            aliases = defaultdict(list)
            for term_id, surface in connection.execute("SELECT term_id, surface FROM terms WHERE joined_key = ''"):
                key = word_key(surface)
                if key.isascii() and key.isalpha() and key.endswith(("ing", "ed", "s")):
                    aliases[key].append(term_id)
        finally:
            connection.close()
        return cls(path, dict(aliases))

    def load_span(self, span):
        """Read only the matched input's incoming relations and outgoing `form` links."""
        connection = sqlite3.connect(f"file:{self.dataset_path}?mode=ro", uri=True)
        try:
            term_ids = set(self.aliases.get(span, ()))
            for term_id, surface in connection.execute(
                "SELECT term_id, surface FROM terms WHERE joined_key = ? AND joined_key <> ''", (span,)
            ):
                if word_key(surface) == span:
                    term_ids.add(term_id)
            forms, related = set(), set()
            for term_id in term_ids:
                for surface, kind in connection.execute(
                    "SELECT t.surface, r.kind FROM relations AS r JOIN terms AS t ON t.term_id = r.from_term WHERE r.to_term = ?",
                    (term_id,),
                ):
                    key = word_key(surface)
                    if key != span:
                        related.add(key)
                        if kind == "form":
                            forms.add(key)
                for surface, in connection.execute(
                    "SELECT t.surface FROM relations AS r JOIN terms AS t ON t.term_id = r.to_term "
                    "WHERE r.from_term = ? AND r.kind = 'form'", (term_id,)
                ):
                    key = word_key(surface)
                    if key != span:
                        forms.add(key)
        finally:
            connection.close()
        self.forms[span], self.related[span] = frozenset(forms), frozenset(related)

    def rank(self, span, base):
        if not span.isascii() or not span.isalpha() or not span.endswith(("ing", "ed", "s")):
            return 0
        if span not in self.related:
            self.load_span(span)
        if base in self.forms.get(span, ()):
            return 2
        return int(base in self.related.get(span, ()))


def select_inflections(matches, vocabulary, inflections):
    """Prefer evidenced bases of each matched span. Without evidence, keep the existing lemma matches."""
    ranks, highest = {}, defaultdict(int)
    for i, m in enumerate(matches):
        term_key = word_key(m["term"])
        if (m["match"] not in INFLECTION_MATCHES or not vocabulary.tags.get(term_key)
                or not PLAIN_WORD.fullmatch(canonical_surface(m["term"])) or term_key == word_key(m["span"])):
            continue
        group = (m["scope"], word_key(m["span"]))
        rank = inflections.rank(group[1], term_key)
        ranks[i] = (group, rank)
        highest[group] = max(highest[group], rank)
    return [m for i, m in enumerate(matches) if i not in ranks or ranks[i][1] == highest[ranks[i][0]]]


def same_word(span, matches, vocabulary, headwords=frozenset()):
    """Categorized words that are the span itself: spelling variants and plain English inflections.

    Matches that keep other symbols, such as `--source-map` or `logs.*`, are different tokens and are skipped.
    A span that is itself one of `headwords` stays that word alone: its inflections are not added (exact-first).
    """
    target = word_key(span)
    if target in headwords:
        return [target], ["exact"]
    words, kinds = set(), set()
    for m in matches:
        term_key = word_key(m["term"])
        if not vocabulary.tags.get(term_key):
            continue
        if term_key == target or (m["match"] in INFLECTION_MATCHES and PLAIN_WORD.fullmatch(canonical_surface(m["term"]))):
            words.add(term_key)
            kinds.add(m["match"])
    return sorted(words), sorted(kinds)


def seed(span, words, kinds, vocabulary):
    return Seed(span, words, "+".join(kinds), frozenset().union(*(vocabulary.tags[w] for w in words)))


def resolve(query, matches, vocabulary, is_syntax_skipped=False, headwords=frozenset(), inflections=None):
    """With `is_syntax_skipped`, input parts that are only code syntax (`syntax_parts`) do not become seeds.
    With `headwords`, an input or part that is itself a headword is not widened to its inflections (`same_word`).
    With `inflections`, data-backed bases take precedence; rejected bases are not treated as input spellings."""
    if inflections is not None:
        matches = select_inflections(matches, vocabulary, inflections)
    matched_keys = {word_key(query)} | {word_key(m["term"]) for m in matches}
    words, kinds = same_word(query, [m for m in matches if m["scope"] == "whole"], vocabulary, headwords)
    direct = word_key(query)
    if vocabulary.tags.get(direct) and direct not in words:
        words, kinds = sorted([*words, direct]), sorted({*kinds, "exact"})
    if words:
        return Resolution("whole", [seed(query, words, kinds, vocabulary)], matched_keys)

    # The whole input is not a categorized word: look up each input part as its own word.
    by_span = defaultdict(list)
    for m in matches:
        if m["scope"] == "part":
            by_span[m["span"]].append(m)
    ranked = []
    for span, span_matches in by_span.items():
        if is_syntax_skipped and is_syntax_part(span, query):
            continue
        words, kinds = same_word(span, span_matches, vocabulary, headwords)
        if words:
            ranked.append((-len(span.split()), span, words, kinds))
    ranked.sort()
    used, seeds = set(), []
    for _, span, words, kinds in ranked:
        tokens = set(span.split())
        if tokens & used:
            continue
        used |= tokens
        seeds.append(seed(span, words, kinds, vocabulary))
    return Resolution("parts" if seeds else "none", seeds, matched_keys)


def overlap(seed_categories, candidate_categories):
    shared = seed_categories & candidate_categories
    return {
        "shared": len(shared),
        "seed_size": len(seed_categories),
        "candidate_size": len(candidate_categories),
        "jaccard_pct": round(100 * len(shared) / len(seed_categories | candidate_categories), 1),
        "coverage_pct": round(100 * len(shared) / len(seed_categories), 1),
        "shared_categories": sorted(shared),
    }


def score_key(row):
    return (row["jaccard_pct"], row["shared"])


def lookup(query, matches, vocabulary):
    """`matches` are `rtpoc3 related` matches for `canonical_surface(query)`."""
    word = canonical_surface(query)
    resolution = resolve(word, matches, vocabulary)
    excluded = set(resolution.matched_keys)
    for seed in resolution.seeds:
        excluded.update(seed.terms)
        excluded.add(word_key(seed.span))

    best = {}
    for seed in resolution.seeds:
        pool = set().union(*(vocabulary.members[c] for c in seed.categories))
        for candidate in pool:
            if candidate in excluded:
                continue
            row = overlap(seed.categories, vocabulary.tags[candidate])
            row["via"] = seed.span
            if candidate not in best or score_key(row) > score_key(best[candidate]):
                best[candidate] = row

    candidates = [
        {"term": vocabulary.surfaces[c], "categories": sorted(vocabulary.tags[c]), **row}
        for c, row in best.items()
    ]
    candidates.sort(key=lambda r: (-r["jaccard_pct"], -r["shared"], r["candidate_size"], r["term"].lower()))
    status = "ok" if candidates else ("no_category" if matches else "no_match")
    return {
        "schema": SCHEMA,
        "input": query,
        "status": status,
        "resolution": {
            "mode": resolution.mode,
            "seeds": [
                {"span": s.span, "terms": [vocabulary.surfaces[t] for t in s.terms], "match": s.match, "categories": sorted(s.categories)}
                for s in resolution.seeds
            ],
        },
        "candidate_count": len(candidates),
        "candidates": candidates,
    }


def run_rtpoc3(queries, rtpoc3, dataset):
    with tempfile.TemporaryDirectory() as tmp:
        terms = Path(tmp) / "terms.txt"
        terms.write_text("".join(q + "\n" for q in queries))
        result = subprocess.run(
            [str(rtpoc3), "related", "--dataset", str(dataset), "--format", "json", "--terms-file", str(terms)],
            check=True, capture_output=True, text=True,
        )
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    return {row["input"]: row["matches"] for row in rows}


def render_text(result):
    seeds = result["resolution"]["seeds"]
    resolved = " | ".join(f'{"/".join(s["terms"])} → {", ".join(s["categories"])}' for s in seeds) or "카테고리 없음"
    lines = [f'# {result["input"]}  [{result["resolution"]["mode"]}] {resolved}  ({result["candidate_count"]}개)']
    for row in result["candidates"]:
        via = f'  <- {row["via"]}' if len(seeds) > 1 else ""
        lines.append(
            f'  {row["jaccard_pct"]:5.1f}%  공유 {row["shared"]}/{row["seed_size"]}·후보 {row["candidate_size"]}개  '
            f'{row["term"]}  [{", ".join(row["shared_categories"])}]{via}'
        )
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("terms", nargs="*")
    parser.add_argument("--terms-file")
    parser.add_argument("--category-dataset", default=DEFAULT_CATEGORY_DATASET, help="rt-category/1 SQLite")
    parser.add_argument("--dataset", default=DEFAULT_DATASET, help="rt-related/1 SQLite used by rtpoc3 for input resolution")
    parser.add_argument("--rtpoc3", default=DEFAULT_RTPOC3)
    parser.add_argument("--matches", help="rtpoc3 related JSONL output to reuse instead of running rtpoc3")
    parser.add_argument("--format", choices=("text", "json"), default="text")
    parser.add_argument("--output")
    args = parser.parse_args()

    vocabulary = Vocabulary.load(args.category_dataset)
    queries = [l.strip() for l in Path(args.terms_file).read_text().splitlines() if l.strip()] if args.terms_file else args.terms
    if not queries:
        parser.error("no input terms")
    words = sorted({canonical_surface(q) for q in queries})
    if args.matches:
        stored = {}
        with open(args.matches) as f:
            for line in f:
                if line.strip():
                    row = json.loads(line)
                    stored[row["input"]] = row["matches"]
        missing = [w for w in words if w not in stored]
        if missing:
            stored.update(run_rtpoc3(missing, args.rtpoc3, args.dataset))
    else:
        stored = run_rtpoc3(words, args.rtpoc3, args.dataset)

    results = [lookup(q, stored.get(canonical_surface(q), []), vocabulary) for q in queries]
    text = "".join(render_text(r) if args.format == "text" else json.dumps(r, ensure_ascii=False) + "\n" for r in results)
    if args.output:
        Path(args.output).write_text(text)
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
