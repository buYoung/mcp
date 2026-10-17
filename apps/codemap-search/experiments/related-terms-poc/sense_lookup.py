"""Sense lookup PoC: input word -> its senses -> its own related words (direct), words sharing a concept (near)
or a topic (same topic), one hop.

Reads `rt-sense/2`, the input-scoped relation extension `rt-sense/3` and the word-to-topic layout `rt-sense/4`
(`sense_mapping_schema.py`), and resolves input through `rt-related/1` with
`rtpoc3 related`, exactly like `category_lookup.py`, except that an input with senses of its own is not widened to
its lemma (`--no-exact-first` restores that widening). A sense is the group of a word's personas that fall in
one topic (one field with `--no-split-senses-by-topic`). A word's senses are its own, then those of the personas it
was generated in as a related word (`input_senses`), which a dataset may also give words with senses of their own
for topics they have none in. For each sense the result lists the words the input was generated with in that sense,
then the words of its concepts, then the other words of its topics; related words outside every sense (`form` links
among them) follow per input word. `--reverse` adds the words that list the input as related, for measurement only.
An input's senses go by persona count; senses with the same count come most similar to the input first: the cosine
between the input's embedding and the mean embedding of each sense's topic words, both stored in `rt-related/1`
(`--no-similarity-order` restores the topic size order). Each word appears once, in the first group that reaches it. Nothing is reached through a returned word's own
relations or concepts, and nothing is cut off.

Usage: python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py token cache '$state'
"""

import argparse
from array import array
from collections import defaultdict
from dataclasses import dataclass, field as dataclass_field
from functools import cached_property, lru_cache
import json
import math
from pathlib import Path
import sqlite3
import sys

import category_lookup as cl
from term_canonical import canonical_surface, exact_key, word_key
from spelling_aliases import spelling_queries, spelling_variants
from reviewed_relations import INPUT_RELATIONS_SCHEMA, SUPPORTED_SCHEMAS, validate_dataset_layout
from sense_mapping_schema import MAPPING_SCHEMA, validate_mapping_layout

# rts3-48356e35e1f1ea28 in the rt-sense/4 layout plus topics mapped by review to words with a single field, then
# topics re-classified (fields, merges, moves) and topics added by review; earlier versions stay next to it under datasets/
DEFAULT_SENSE_DATASET = Path(__file__).resolve().parent / "datasets/rts4-e5083ab344e1336d/dataset.sqlite3"
SCHEMA = "rt-sense-lookup/2"
SENSE_SCHEMA = "rt-sense/2"
READABLE_SCHEMAS = (*SUPPORTED_SCHEMAS, MAPPING_SCHEMA)
# persona, word, field, concept (NULL in rt-sense/4 when the word maps to the topic alone), topic, and how many
# generated personas the row stands for (the sense order weight; one row is one persona before rt-sense/4)
PERSONA_QUERIES = {
    MAPPING_SCHEMA: "SELECT p.persona, t.word_key, tf.field, p.concept, p.topic, p.source_persona_count FROM personas AS p "
                    "JOIN terms AS t USING (term_id) JOIN topic_fields AS tf ON tf.topic = p.topic AND tf.rank = 0",
    **dict.fromkeys(SUPPORTED_SCHEMAS,
                    "SELECT p.persona, t.word_key, s.field, p.concept, c.topic, 1 FROM personas AS p "
                    "JOIN senses AS s USING (sense) JOIN terms AS t ON t.term_id = p.term_id JOIN concepts AS c USING (concept)"),
}


def persona_rows(connection):
    """(persona, word, field, concept, topic, weight) rows of any readable sense dataset, for scripts that read it directly."""
    schema = connection.execute("SELECT value FROM sense_meta WHERE key = 'schema_version'").fetchone()[0]
    return connection.execute(PERSONA_QUERIES[schema])


@dataclass
class Sense:
    field: str
    concepts: frozenset
    topics: frozenset
    direct: tuple
    personas: int
    is_own: bool = True


def sense_order(sense, is_topic_split, topic_words):
    """Without topic split, by persona count then field, as before. With it, a word's own senses come before the
    senses read through other words' personas, each by persona count, and senses of topics mapped by review alone
    (rt-sense/4 persona count 0) come last. Senses with the same persona count go by the number of words in their
    topic, largest first, so that revising a topic's field does not reorder them; field and topic break what is left."""
    if is_topic_split:
        topic_size = max((len(topic_words.get(topic, ())) for topic in sense.topics), default=0)
        return (sense.personas == 0, not sense.is_own, -sense.personas, -topic_size, sense.field, sorted(sense.topics))
    return (-sense.personas, sense.field)


@dataclass
class SenseIndex:
    surfaces: dict
    senses: dict
    unsorted_direct: dict
    reverse: dict
    concept_words: dict
    topic_words: dict
    concept_names: dict
    topic_labels: dict
    field_labels: dict
    vocabulary: cl.Vocabulary
    persona_info: dict
    persona_words: dict
    input_personas: dict
    is_input_sibling_included: bool = True
    is_topic_split: bool = False
    input_relation_words: dict = dataclass_field(default_factory=dict)
    # (vector dataset, topic) -> (sum of the topic words' vectors, the words that have one), filled on first use
    topic_vector_sums: dict = dataclass_field(default_factory=dict)

    @cached_property
    def generated_headwords(self):
        """Words with a generated sense of their own. A word whose own senses are only topics mapped by review
        (rt-sense/4 persona count 0) still takes its lemma, like a word read only through `input_senses`."""
        return frozenset(key for key, senses in self.senses.items() if any(sense.personas for sense in senses))

    def senses_of(self, key):
        """A word's own senses, then classifications through source personas (`input_senses`): generated related
        words or explicit reviewed bindings, one sense per topic (per field without topic split), holding the
        persona's head word, concept and topic. A dataset gives these personas to words without senses of their own,
        and may also give words with senses the personas of topics they have none in (rule `head_topics`).
        The dataset rule decides whether the other words generated with it are direct words; datasets without a rule
        retain their earlier head-and-siblings behavior. rt-sense/3 may also store finite reviewed direct words
        for this exact input/source binding, independently of the global sibling rule."""
        own = self.senses.get(key, [])
        if key not in self.input_personas:
            return own
        parts = {}
        for persona in self.input_personas[key]:
            head, field, concept, topic = self.persona_info[persona]
            part = parts.setdefault((field, topic if self.is_topic_split else None),
                                    {"concepts": set(), "topics": set(), "direct": [], "personas": 0})
            if concept is not None:
                part["concepts"].add(concept)
            part["topics"].add(topic)
            part["direct"].append(head)
            if self.is_input_sibling_included:
                part["direct"].extend(w for w in self.persona_words.get(persona, ()) if w != key)
            # Explicit data for this input/binding only, never donor/source expansion.
            part["direct"].extend(self.input_relation_words.get((key, persona), ()))
            part["personas"] += 1
        senses = [Sense(field, frozenset(p["concepts"]), frozenset(p["topics"]), tuple(dict.fromkeys(p["direct"])), p["personas"],
                        is_own=False) for (field, _), p in parts.items()]
        return list(own) + sorted(senses, key=lambda s: sense_order(s, self.is_topic_split, self.topic_words))

    @classmethod
    def load(cls, dataset_path, is_topic_split=False):
        path = Path(dataset_path)
        if not path.exists():
            raise SystemExit(f"sense dataset unavailable: {path}")
        connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        try:
            meta = dict(connection.execute("SELECT key, value FROM sense_meta"))
            if meta.get("schema_version") not in READABLE_SCHEMAS:
                raise SystemExit(f"sense dataset incompatible: {meta.get('schema_version')!r}, expected {READABLE_SCHEMAS}")
            try:
                if meta["schema_version"] == MAPPING_SCHEMA:
                    validate_mapping_layout(connection)
                else:
                    validate_dataset_layout(connection, meta["schema_version"])
            except (ValueError, sqlite3.DatabaseError) as error:
                raise SystemExit(f"sense dataset layout incompatible: {error}") from error
            rule = json.loads(meta["input_sense_rule"]) if "input_sense_rule" in meta else {"direct": "head_and_siblings"}
            if rule.get("direct") not in ("head", "head_and_siblings"):
                raise SystemExit(f"input sense direct rule incompatible: {rule.get('direct')!r}")
            surfaces = dict(connection.execute("SELECT word_key, surface FROM terms"))
            concept_names = dict(connection.execute("SELECT concept, name FROM concepts"))
            topic_labels = dict(connection.execute("SELECT topic, label FROM topics"))
            field_labels = dict(connection.execute("SELECT field, label FROM fields"))
            parts = defaultdict(lambda: {"concepts": set(), "topics": set(), "direct": [], "personas": 0})
            persona_info, concept_words, topic_words = {}, defaultdict(set), defaultdict(set)
            for persona, key, field, concept, topic, weight in connection.execute(PERSONA_QUERIES[meta["schema_version"]]):
                part = parts[(key, field, topic if is_topic_split else None)]
                if concept is not None:
                    part["concepts"].add(concept)
                    concept_words[concept].add(key)
                part["topics"].add(topic)
                part["personas"] += weight
                persona_info[persona] = (key, field, concept, topic)
                topic_words[topic].add(key)
            unsorted_direct, reverse, persona_words = defaultdict(list), defaultdict(list), defaultdict(list)
            for head, key, surface, persona in connection.execute(
                "SELECT t.word_key, r.word_key, r.surface, r.persona FROM relations AS r JOIN terms AS t USING (term_id) "
                "ORDER BY r.term_id, r.rank"
            ):
                surfaces.setdefault(key, surface)
                if persona is None:
                    unsorted_direct[head].append(key)
                else:
                    _, field, _, topic = persona_info[persona]
                    parts[(head, field, topic if is_topic_split else None)]["direct"].append(key)
                    persona_words[persona].append(key)
                reverse[key].append(head)
            input_personas, input_relation_words = defaultdict(list), defaultdict(list)
            if connection.execute("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'input_senses'").fetchone():
                for key, persona in connection.execute("SELECT word_key, persona FROM input_senses ORDER BY word_key, persona"):
                    input_personas[key].append(persona)
            if meta["schema_version"] in (INPUT_RELATIONS_SCHEMA, MAPPING_SCHEMA):
                for key, persona, related, surface in connection.execute(
                    "SELECT input_word_key,source_persona,related_word_key,surface FROM input_relations "
                    "ORDER BY input_word_key,source_persona,rank,related_word_key"
                ):
                    surfaces.setdefault(related, surface)
                    input_relation_words[(key, persona)].append(related)
                    reverse[related].append(key)
        finally:
            connection.close()
        senses = defaultdict(list)
        for (key, field, _), part in parts.items():
            senses[key].append(Sense(field, frozenset(part["concepts"]), frozenset(part["topics"]), tuple(part["direct"]), part["personas"]))
        for key in senses:
            senses[key].sort(key=lambda s: sense_order(s, is_topic_split, topic_words))
        # Input resolution only asks whether a word is classified, so a word's topics tag it (a topic-only persona
        # has no concept).
        tags = {key: frozenset(t for s in senses.get(key, ()) for t in s.topics) for key in surfaces}
        for key, personas in input_personas.items():
            if not tags.get(key):
                tags[key] = frozenset(persona_info[p][3] for p in personas)
        vocabulary = cl.Vocabulary(surfaces, tags, dict(concept_words), {})
        return cls(surfaces, dict(senses), dict(unsorted_direct), dict(reverse), dict(concept_words), dict(topic_words),
                   concept_names, topic_labels, field_labels, vocabulary, persona_info, dict(persona_words), dict(input_personas),
                   rule["direct"] == "head_and_siblings", is_topic_split, dict(input_relation_words))


@dataclass
class TermVectors:
    """Word embeddings stored in an `rt-related/1` dataset, read by exact key on first use (no model runs here)."""
    dataset_path: Path
    connection: sqlite3.Connection
    vectors: dict = dataclass_field(default_factory=dict)

    @classmethod
    @lru_cache(maxsize=4)
    def load(cls, dataset_path):
        path = Path(dataset_path)
        if not path.exists():
            raise SystemExit(f"vector dataset unavailable: {path}")
        return cls(path, sqlite3.connect(f"file:{path}?mode=ro", uri=True, check_same_thread=False))

    def of(self, surface):
        """The word's int8 vector, read under its spelling and then its canonical form; None when not stored."""
        if surface not in self.vectors:
            self.vectors[surface] = None
            for key in dict.fromkeys((exact_key(surface), exact_key(canonical_surface(surface)))):
                row = self.connection.execute("SELECT vector FROM terms WHERE exact_key = ?", (key,)).fetchone()
                if row and row[0]:
                    self.vectors[surface] = array("b", row[0])
                    break
        return self.vectors[surface]


def vector_sum(vectors):
    total = [0] * len(vectors[0])
    for vector in vectors:
        total = [t + v for t, v in zip(total, vector)]
    return total


def cosine(left, right):
    norm = math.sqrt(sum(a * a for a in left)) * math.sqrt(sum(b * b for b in right))
    return sum(a * b for a, b in zip(left, right)) / norm if norm else None


def query_vector(seed, index, vectors):
    """The input span's own vector, else the sum of its words' vectors."""
    vector = vectors.of(seed.span)
    if vector is not None:
        return vector
    found = [v for v in (vectors.of(index.surfaces[t]) for t in seed.terms) if v is not None]
    return vector_sum(found) if found else None


def topic_vector_sum(index, vectors, topic):
    key = (vectors.dataset_path, topic)
    if key not in index.topic_vector_sums:
        found = {w: v for w in sorted(index.topic_words.get(topic, ())) if (v := vectors.of(index.surfaces[w])) is not None}
        index.topic_vector_sums[key] = (vector_sum(list(found.values())) if found else None, frozenset(found))
    return index.topic_vector_sums[key]


def sense_similarity(query, sense, index, vectors, own):
    """Cosine between the query and the mean of the sense's topic words other than the input's own words (`own`).
    A topic with no other word with a vector stands for itself through the sense's direct related words."""
    total, members = None, 0
    for topic in sense.topics:
        topic_sum, words = topic_vector_sum(index, vectors, topic)
        if topic_sum is None:
            continue
        removed = [vectors.of(index.surfaces[w]) for w in words & own]
        if len(words) - len(removed) <= 0:
            continue
        part = vector_sum([topic_sum] + [[-v for v in r] for r in removed]) if removed else topic_sum
        total = part if total is None else vector_sum([total, part])
        members += len(words) - len(removed)
    if not members:
        found = [v for v in (vectors.of(index.surfaces.get(w, w)) for w in sense.direct if w not in own) if v is not None]
        if not found:
            return None
        total = vector_sum(found)
    return cosine(query, total)


def seed_senses(seed, index, vectors=None):
    """The seed words' senses merged by topic (by field without topic split), in `sense_order`. With `vectors` and
    topic split, senses with the same persona count go by similarity to the input (`sense_similarity`) instead of
    topic size, most similar first and senses without a similarity after them; review-only senses still come last
    and a word's own senses before those read through other words' personas. `sense_order` breaks what is left."""
    merged = {}
    for term in seed.terms:
        for sense in index.senses_of(term):
            group = (sense.field, sense.topics) if index.is_topic_split else sense.field
            if group in merged:
                old = merged[group]
                merged[group] = Sense(sense.field, old.concepts | sense.concepts, old.topics | sense.topics,
                                      tuple(dict.fromkeys(old.direct + sense.direct)), old.personas + sense.personas,
                                      old.is_own or sense.is_own)
            else:
                merged[group] = sense
    senses = sorted(merged.values(), key=lambda s: sense_order(s, index.is_topic_split, index.topic_words))
    query = query_vector(seed, index, vectors) if vectors is not None and index.is_topic_split else None
    if query is None:
        return senses
    own = set(seed.terms) | {word_key(seed.span)}
    similarities = [sense_similarity(query, sense, index, vectors, own) for sense in senses]
    order = sorted(range(len(senses)), key=lambda i: (senses[i].personas == 0, not senses[i].is_own, -senses[i].personas,
                                                      similarities[i] is None, -(similarities[i] or 0), i))
    return [senses[i] for i in order]


def lookup(query, matches, index, is_direct_included=True, is_reverse_included=False, is_syntax_skipped=False, is_exact_first=False,
           is_inflection_evidence_used=True, inflection_dataset=cl.DEFAULT_DATASET, spelling_matches=None,
           is_similarity_ordered=True, vector_dataset=cl.DEFAULT_DATASET):
    """`matches` are `rtpoc3 related` matches for `canonical_surface(query)`.

    Groups are filled nearest first (direct, unsorted direct, near, same topic, reverse); a word already placed
    is not repeated. Without `is_direct_included` the direct groups stay empty, as in `rt-sense-lookup/1`.
    With `is_syntax_skipped`, input parts that are only code syntax (`fn`, `.go`, `[Mm]`) are not looked up.
    With `is_exact_first`, an input that is itself a word with senses is not widened to its lemma (`blocking` stays
    `blocking`, not also `block`); an input whose own senses come only from `input_senses` or from topics mapped by
    review still takes its lemma.
    With `is_inflection_evidence_used`, competing lemmas are resolved through the input resolution dataset's relations.
    `spelling_matches`, when supplied, maps the queries from `spelling_queries` to matcher results and enables
    confirmed spelling alternatives. None preserves the original precomputed-match API. Each interpretation uses
    the same caller options; original meanings are retained, and returned words never become new query seeds.
    With `is_similarity_ordered`, a seed's senses with the same persona count go by similarity to the seed, from the
    embeddings stored in `vector_dataset` (`seed_senses`); without it, by topic size as before.
    """
    word = canonical_surface(query)
    headwords = index.generated_headwords if is_exact_first else frozenset()
    inflections = cl.InflectionEvidence.load(inflection_dataset) if is_inflection_evidence_used else None
    resolution = cl.resolve(word, matches, index.vocabulary, is_syntax_skipped, headwords, inflections)
    has_matches = bool(matches)
    # Only interpreted inputs and selected seeds are self-matches. Discarded hits
    # must not suppress otherwise valid one-hop neighbors.
    excluded = {word_key(word)}
    if spelling_matches is not None:
        for alternative in spelling_variants(word)[1:]:
            alternative_matches = spelling_matches.get(alternative, [])
            extra = cl.resolve(alternative, alternative_matches, index.vocabulary, is_syntax_skipped, headwords, inflections)
            has_matches = has_matches or bool(alternative_matches) or bool(extra.seeds)
            if extra.seeds:
                if not resolution.seeds:
                    resolution.mode = extra.mode
                resolution.seeds.extend(extra.seeds)
                resolution.matched_keys.update(extra.matched_keys)
                excluded.add(word_key(alternative))
    for seed in resolution.seeds:
        excluded.update(seed.terms)
        excluded.add(word_key(seed.span))
    # Reverse links are read for the input itself when it is not a word with senses.
    reverse_seeds = [(s.span, set(s.terms) | {word_key(s.span)}) for s in resolution.seeds] or [(word, {word_key(word)})]

    groups = {}
    placed = set(excluded)

    def place(group, keys):
        fresh = [k for k in dict.fromkeys(keys) if k not in placed]
        placed.update(fresh)
        groups[group] = fresh

    vectors = TermVectors.load(vector_dataset) if is_similarity_ordered else None
    seeds = [(seed, seed_senses(seed, index, vectors)) for seed in resolution.seeds]
    for seed, senses in seeds:
        for sense in senses:
            place((id(seed), sense.field, sense.topics, "direct"), sense.direct if is_direct_included else ())
    for seed, _ in seeds:
        place((id(seed), "direct"), [k for t in seed.terms for k in index.unsorted_direct.get(t, ())] if is_direct_included else ())
    for seed, senses in seeds:
        for sense in senses:
            place((id(seed), sense.field, sense.topics, "near"), sorted(set().union(*(index.concept_words[c] for c in sense.concepts)) - placed,
                                                       key=lambda k: index.surfaces[k].lower()))
    for seed, senses in seeds:
        for sense in senses:
            place((id(seed), sense.field, sense.topics, "same_topic"), sorted(set().union(*(index.topic_words[t] for t in sense.topics)) - placed,
                                                             key=lambda k: index.surfaces[k].lower()))
    for span, keys in reverse_seeds:
        place((span, "reverse"), sorted({h for k in keys for h in index.reverse.get(k, ())}, key=lambda k: index.surfaces[k].lower())
              if is_reverse_included else ())

    def words(group):
        return [index.surfaces[k] for k in groups[group]]

    senses = [{
        "span": seed.span, "field": sense.field, "field_label": index.field_labels[sense.field],
        "topics": [{"topic": t, "label": index.topic_labels[t]} for t in sorted(sense.topics)],
        "concepts": [{"concept": c, "name": index.concept_names[c]} for c in sorted(sense.concepts)],
        "direct": words((id(seed), sense.field, sense.topics, "direct")),
        "near": words((id(seed), sense.field, sense.topics, "near")),
        "same_topic": words((id(seed), sense.field, sense.topics, "same_topic")),
    } for seed, seed_sense_list in seeds for sense in seed_sense_list]
    unsorted = [{"span": seed.span, "direct": words((id(seed), "direct"))} for seed, _ in seeds]
    reverse = [{"span": span, "words": words((span, "reverse"))} for span, _ in reverse_seeds] if is_reverse_included else []
    count = len(placed - excluded)
    status = "ok" if count else ("no_sense" if has_matches else "no_match")
    return {
        "schema": SCHEMA, "input": query, "status": status,
        "resolution": {"mode": resolution.mode, "seeds": [{"span": s.span, "terms": [index.surfaces[t] for t in s.terms], "match": s.match} for s in resolution.seeds]},
        "senses": senses, "unsorted_direct": unsorted, "reverse": reverse, "candidate_count": count,
    }


RINGS = (("direct", "직접 관계어"), ("near", "가까운 연관어"), ("same_topic", "같은 주제"))
UNSORTED_LABEL = "뜻 미배정 직접 관계어"
TEXT_EPILOG = """텍스트 출력: 입력 해석, 뜻 미배정 직접 관계어(주로 같은 단어의 다른 표기), 그다음 뜻마다 직접 관계어 · 가까운 연관어 · 같은 주제.
한 번 실행한 결과 안에서 같은 단어는 처음 나온 곳에만 쓴다. 괄호 안은 그 단어들을 낸 입력 단어다.
인자 하나가 입력 하나다. 여러 단어로 된 표현은 따옴표로 묶는다. 사전에 그 표현이 없으면 단어별로 나눠 찾는다."""


def describe_resolution(result):
    """`input → seed + seed` in input order, the seed's dictionary words in parentheses when they differ from it."""
    seeds = result["resolution"]["seeds"]
    if not seeds:
        return f'{result["input"]} → 사전에 없음'
    word = canonical_surface(result["input"])
    spans = []
    has_spelling_branch = any(s["span"] in spelling_variants(word)[1:] for s in seeds)
    for seed in sorted(seeds, key=lambda s: (has_spelling_branch and word.find(s["span"]) < 0, word.find(s["span"]))):
        others = [t for t in seed["terms"] if word_key(t) != word_key(seed["span"])]
        spans.append(seed["span"] + (f'({"/".join(others)})' if others else ""))
    text = " + ".join(spans)
    described = result["input"] if text == word else f'{result["input"]} → {text}'
    return described if result["candidate_count"] else f"{described} (관련어 없음)"


def call_blocks(results):
    """Groups of one call keyed by seed words and field, so inputs that resolve to the same seed share one group.

    Each input's related words outside every sense come first, then the senses in the order each input lists them.
    """
    blocks = {}
    for result in results:
        seed_keys = {s["span"]: tuple(sorted({word_key(t) for t in s["terms"]})) for s in result["resolution"]["seeds"]}
        groups = [((seed_keys[g["span"]], None, ()), g["span"], UNSORTED_LABEL, {"direct": g["direct"]}) for g in result["unsorted_direct"]]
        groups += [((seed_keys[s["span"]], s["field"], tuple(t["topic"] for t in s["topics"])), s["span"], f'{s["field_label"]} — {", ".join(t["label"] for t in s["topics"])}',
                    {ring: s[ring] for ring, _ in RINGS}) for s in result["senses"]]
        for key, span, label, rings in groups:
            block = blocks.setdefault(key, {"label": label, "spans": [], **{ring: [] for ring, _ in RINGS}})
            if span not in block["spans"]:
                block["spans"].append(span)
            for ring, words in rings.items():
                block[ring].extend(words)
    return sorted(blocks.values(), key=lambda b: b["label"] != UNSORTED_LABEL)


def render_text(results):
    """One call: how each input was read, then every group of the call once.

    A word is written once per call, in the first group that reaches it, and a group with nothing left is not written.
    Every candidate of every input is still written; only repeats and empty lines are gone.
    """
    shown = {word_key(r["input"]) for r in results} | {word_key(t) for r in results for s in r["resolution"]["seeds"] for t in s["terms"]}
    is_span_shown = len(results) > 1 or any(s["span"] != canonical_surface(r["input"]) for r in results for s in r["resolution"]["seeds"])

    def fresh(words):
        new = [w for w in dict.fromkeys(words) if word_key(w) not in shown]
        shown.update(word_key(w) for w in new)
        return new

    body, total = [], 0
    for block in call_blocks(results):
        spans = f' ({", ".join(block["spans"])})' if is_span_shown else ""
        rows = [(title, fresh(block[ring])) for ring, title in RINGS]
        rows = [(title, words) for title, words in rows if words]
        if not rows:
            continue
        total += sum(len(words) for _, words in rows)
        if block["label"] == UNSORTED_LABEL:
            body.append(f"## {UNSORTED_LABEL}{spans}: {', '.join(rows[0][1])}")
            continue
        body.append(f'## {block["label"]}{spans}')
        body += [f"  {title}: {', '.join(words)}" for title, words in rows]
    head = [f"# 관련어 {total}개 · 입력 {len(results)}개", "입력 해석: " + " · ".join(describe_resolution(r) for r in results)]
    return "\n".join(head + body) + "\n"


def render_text_by_input(result):
    """The text form used before `render_text`, one input at a time (kept to reproduce earlier runs)."""
    lines = [f'# {result["input"]}  [{result["resolution"]["mode"]}]  뜻 {len(result["senses"])}개 · 후보 {result["candidate_count"]}개']
    for sense in result["senses"]:
        via = f' ({sense["span"]})' if sense["span"] != canonical_surface(result["input"]) else ""
        topics = ", ".join(t["label"] for t in sense["topics"])
        lines.append(f'## {sense["field_label"]}{via} — {topics}')
        lines.append(f'  직접 관계어 {len(sense["direct"])}: {", ".join(sense["direct"])}')
        lines.append(f'  가까운 연관어 {len(sense["near"])}: {", ".join(sense["near"])}')
        lines.append(f'  같은 주제 {len(sense["same_topic"])}: {", ".join(sense["same_topic"])}')
    for group in result["unsorted_direct"]:
        if group["direct"]:
            lines.append(f'## 뜻 미배정 직접 관계어 ({group["span"]}) {len(group["direct"])}: {", ".join(group["direct"])}')
    for group in result["reverse"]:
        lines.append(f'## 역방향 관계어 ({group["span"]}) {len(group["words"])}: {", ".join(group["words"])}')
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog=TEXT_EPILOG, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("terms", nargs="*")
    parser.add_argument("--terms-file")
    parser.add_argument("--sense-dataset", default=DEFAULT_SENSE_DATASET, help="rt-sense/2, rt-sense/3 or rt-sense/4 SQLite")
    parser.add_argument("--dataset", default=cl.DEFAULT_DATASET, help="rt-related/1 SQLite used by rtpoc3 for input resolution")
    parser.add_argument("--rtpoc3", default=cl.DEFAULT_RTPOC3)
    parser.add_argument("--reverse", action="store_true", help="also list words that name the input as related (measurement only)")
    parser.add_argument("--skip-syntax-parts", action="store_true", help="do not look up input parts that are only code syntax")
    parser.add_argument("--exact-first", action=argparse.BooleanOptionalAction, default=True,
                        help="an input that has senses itself is not widened to its lemma (--no-exact-first: earlier behavior)")
    parser.add_argument("--evidence-inflections", action=argparse.BooleanOptionalAction, default=True,
                        help="prefer lemmas evidenced by form links or the base's related words (--no-evidence-inflections: earlier behavior)")
    parser.add_argument("--split-senses-by-topic", action=argparse.BooleanOptionalAction, default=True,
                        help="one sense per topic of a word (--no-split-senses-by-topic: earlier behavior, one per field)")
    parser.add_argument("--similarity-order", action=argparse.BooleanOptionalAction, default=True,
                        help="order an input's senses with the same persona count by similarity to the input, from the "
                             "--dataset embeddings (--no-similarity-order: earlier behavior, by topic size)")
    parser.add_argument("--format", choices=("text", "text-by-input", "json"), default="text",
                        help="text: one call, nearest rings first; text-by-input: the earlier per-input text")
    parser.add_argument("--output")
    args = parser.parse_args()

    index = SenseIndex.load(args.sense_dataset, args.split_senses_by_topic)
    queries = [l.strip() for l in Path(args.terms_file).read_text().splitlines() if l.strip()] if args.terms_file else args.terms
    if not queries:
        parser.error("no input terms")
    stored = cl.run_rtpoc3(spelling_queries(queries), args.rtpoc3, args.dataset)
    results = [lookup(q, stored.get(canonical_surface(q), []), index, is_reverse_included=args.reverse,
                      is_syntax_skipped=args.skip_syntax_parts, is_exact_first=args.exact_first,
                      is_inflection_evidence_used=args.evidence_inflections, inflection_dataset=args.dataset,
                      spelling_matches=stored, is_similarity_ordered=args.similarity_order, vector_dataset=args.dataset)
               for q in queries]
    if args.format == "text":
        text = render_text(results)
    elif args.format == "text-by-input":
        text = "".join(render_text_by_input(r) for r in results)
    else:
        text = "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in results)
    if args.output:
        Path(args.output).write_text(text)
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
