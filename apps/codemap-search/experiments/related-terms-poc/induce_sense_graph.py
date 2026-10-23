"""Induce word personas, concepts and topics from the thesaurus relations (internal experiment).

Bottom-up, without the 52 generation categories:
1. Ego clustering: each word's own related words are grouped by their second-order context (the words
   *they* are related to, idf-weighted). `token` splits into lexer / OAuth / JWT / LLM-count / design-token
   groups, while spelling overlap (every neighbor containing "token") plays no part. Each group is a
   persona of the word. Embeddings are not used: they cluster by spelling.
2. Persona graph: a relation u→v links the persona of u whose group holds v to the persona of v whose
   group holds u, or to v's persona whose context is closest to u when v does not list u.
3. Louvain at two resolutions: personas → concepts → topics. A concept is named by its most connected word.

Topics are then named (and split where they mix subjects) and grouped into fields by agents; their inputs
are written to `naming/` and checked by `build_sense_dataset.py check`. A word's senses are the fields its
personas end up in.

Run from the repository root with the embedding venv (numpy):
  .codemap/poc/related-terms/tools/embed-venv/bin/python \\
      apps/codemap-search/experiments/related-terms-poc/induce_sense_graph.py \\
      --thesaurus .codemap/poc/related-terms/thesaurus-v1 \\
      --contract apps/codemap-search/experiments/related-terms-poc/contracts/thesaurus-categories.json \\
      --output .codemap/poc/related-terms/thesaurus-v1/sense-graph-1
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import glob
import json
import math
import os
import random

import numpy as np

from build_category_dataset import canonical_terms, relation_rows
from term_canonical import word_key

SEED = 20261009
EGO_THRESHOLD = 0.08
EGO_LINK_BONUS = 0.1
EGO_MIN_NEIGHBORS = 8
KIND_WEIGHTS = {"synonym": 1.5, "abbreviation": 1.5, "identifier": 1.0, "family": 1.0, "near": 0.7, "co_use": 0.5}
RESOLUTIONS = {"concept": 300.0, "topic": 15.0}
NAMING_SHARDS = 3
NAMING_WORDS_PER_CONCEPT = 12


def read_relations(thesaurus, co_use=None):
    """Out-links per word key (spellings merged), keeping the first kind seen; `form` links are spelling variants."""
    links, surface_of = defaultdict(dict), {}
    for term, related in relation_rows(thesaurus, co_use):
        head = word_key(term)
        for surface, kind in related:
            key = word_key(surface)
            surface_of.setdefault(key, surface)
            if key != head and kind in KIND_WEIGHTS:
                links[head].setdefault(key, kind)
    return links, surface_of


def context_vectors(links):
    """idf-weighted bag of out-links and in-links for every key, vocabulary or not."""
    context = defaultdict(set)
    for head, related in links.items():
        for key in related:
            context[head].add(key)
            context[key].add(head)
    frequency = Counter(key for keys in context.values() for key in keys)
    idf = {key: math.log(len(context) / count) for key, count in frequency.items()}
    return context, idf


def average_linkage(similarity, threshold):
    n = len(similarity)
    sim = similarity.astype(np.float64).copy()
    np.fill_diagonal(sim, -np.inf)
    size = np.ones(n)
    members = [[i] for i in range(n)]
    while n > 1:
        i, j = divmod(int(np.argmax(sim)), n)
        if sim[i, j] < threshold:
            break
        merged = (sim[i] * size[i] + sim[j] * size[j]) / (size[i] + size[j])
        sim[i, :], sim[:, i] = merged, merged
        sim[i, i] = -np.inf
        sim[j, :], sim[:, j] = -np.inf, -np.inf
        size[i] += size[j]
        members[i] += members[j]
        members[j] = []
    return [m for m in members if m]


def ego_personas(head, neighbors, links, context, idf):
    """Group a word's neighbors into personas; every neighbor is mapped to exactly one persona."""
    if len(neighbors) < EGO_MIN_NEIGHBORS:
        return [list(range(len(neighbors)))], None
    columns = {}
    rows = []
    for key in neighbors:
        row = {}
        for other in context[key]:
            if other != head:
                row[columns.setdefault(other, len(columns))] = idf[other]
        rows.append(row)
    matrix = np.zeros((len(neighbors), len(columns)), dtype=np.float32)
    for i, row in enumerate(rows):
        if row:
            matrix[i, list(row)] = list(row.values())
    norms = np.linalg.norm(matrix, axis=1, keepdims=True)
    matrix /= np.where(norms == 0, 1, norms)
    similarity = matrix @ matrix.T
    position = {key: i for i, key in enumerate(neighbors)}
    for i, key in enumerate(neighbors):
        for other in links.get(key, ()):
            j = position.get(other)
            if j is not None and j != i:
                similarity[i, j] += EGO_LINK_BONUS
                similarity[j, i] += EGO_LINK_BONUS
    clusters = average_linkage(similarity, EGO_THRESHOLD)
    minimum = max(3, round(0.04 * len(neighbors)))
    groups = [c for c in clusters if len(c) >= minimum]
    if len(groups) <= 1:
        return [list(range(len(neighbors)))], None
    leftovers = [i for c in clusters if len(c) < minimum for i in c]
    for i in leftovers:
        best = max(range(len(groups)), key=lambda g: float(similarity[i, groups[g]].mean()))
        groups[best].append(i)
    centroids = [matrix[g].sum(axis=0) for g in groups]
    return groups, (columns, centroids)


def louvain(adjacency, resolution, rng):
    """Weighted Louvain with a resolution parameter; adjacency is a symmetric list of {node: weight}."""
    partition = list(range(len(adjacency)))
    graph = adjacency
    while True:
        community, moved = local_moving(graph, resolution, rng)
        partition = [community[c] for c in partition]
        if not moved:
            break
        graph = aggregate(graph, community)
    labels = {c: i for i, c in enumerate(sorted(set(partition)))}
    return [labels[c] for c in partition]


def local_moving(graph, resolution, rng):
    degree = [sum(edges.values()) for edges in graph]
    total = sum(degree) or 1.0
    community = list(range(len(graph)))
    community_degree = list(degree)
    moved_any, improved = False, True
    order = list(range(len(graph)))
    while improved:
        improved = False
        rng.shuffle(order)
        for node in order:
            current = community[node]
            weights = defaultdict(float)
            for other, weight in graph[node].items():
                if other != node:
                    weights[community[other]] += weight
            community_degree[current] -= degree[node]
            best, best_gain = current, weights.get(current, 0.0) - resolution * community_degree[current] * degree[node] / total
            for candidate, weight in weights.items():
                gain = weight - resolution * community_degree[candidate] * degree[node] / total
                if gain > best_gain + 1e-12:
                    best, best_gain = candidate, gain
            community_degree[best] += degree[node]
            if best != current:
                community[node] = best
                improved = moved_any = True
    labels = {c: i for i, c in enumerate(sorted(set(community)))}
    return [labels[c] for c in community], moved_any


def aggregate(graph, community):
    merged = [defaultdict(float) for _ in range(max(community) + 1)]
    for node, edges in enumerate(graph):
        for other, weight in edges.items():
            merged[community[node]][community[other]] += weight
    return [dict(edges) for edges in merged]


def induce(arguments):
    rng = random.Random(SEED)
    scopes = {row["id"]: row["scope"] for row in json.load(open(arguments.contract))["categories"]}
    terms = canonical_terms(arguments.thesaurus, scopes)
    links, surface_of = read_relations(arguments.thesaurus, arguments.co_use)
    context, idf = context_vectors(links)

    # 1. Personas from ego clustering.
    personas, persona_of, centroid_of = [], {}, {}
    for head in sorted(terms):
        neighbors = sorted(links.get(head, ()))
        if not neighbors:
            continue
        groups, centroids = ego_personas(head, neighbors, links, context, idf)
        for index, group in enumerate(groups):
            persona_id = len(personas)
            personas.append({"persona": persona_id, "word": head, "neighbors": [neighbors[i] for i in group]})
            for i in group:
                persona_of[(head, neighbors[i])] = persona_id
        if centroids:
            columns, vectors = centroids
            centroid_of[head] = (columns, [personas[-len(groups) + g]["persona"] for g in range(len(groups))], vectors)
    personas_by_word = defaultdict(list)
    for persona in personas:
        personas_by_word[persona["word"]].append(persona["persona"])

    def closest_persona(word, visitor):
        """The persona of `word` whose neighbor context best matches `visitor`'s context."""
        options = personas_by_word[word]
        if len(options) == 1 or word not in centroid_of:
            return options[0]
        columns, ids, vectors = centroid_of[word]
        scores = [sum(vector[columns[k]] * idf[k] for k in context[visitor] if k in columns) for vector in vectors]
        return ids[int(np.argmax(scores))]

    # 2. Persona graph over vocabulary words. A link counts in proportion to the neighbors the two
    #    personas share, so links between personas about the same thing dominate stray ones.
    neighbor_sets = [set(persona["neighbors"]) for persona in personas]
    adjacency = [defaultdict(float) for _ in personas]
    for head in personas_by_word:
        for other, kind in links[head].items():
            if other not in personas_by_word or other == head:
                continue
            source = persona_of[(head, other)]
            target = persona_of.get((other, head))
            if target is None:
                target = closest_persona(other, head)
            shared = len(neighbor_sets[source] & neighbor_sets[target]) + 1 + (head in neighbor_sets[target])
            weight = KIND_WEIGHTS[kind] * shared / math.sqrt((len(neighbor_sets[source]) + 1) * (len(neighbor_sets[target]) + 1))
            adjacency[source][target] += weight
            adjacency[target][source] += weight
    adjacency = [dict(edges) for edges in adjacency]
    connected = [i for i, edges in enumerate(adjacency) if edges]

    # 3. Concepts and topics.
    resolutions = {**RESOLUTIONS, **{k: v for k, v in vars(arguments).items() if k in RESOLUTIONS and v is not None}}
    concept_of = louvain(adjacency, resolutions["concept"], rng)
    concept_graph = aggregate(adjacency, concept_of)
    topic_of_concept = louvain(concept_graph, resolutions["topic"], rng)

    # Concept names: the member word with the strongest links inside the concept.
    concept_members = defaultdict(list)
    for persona_id, concept in enumerate(concept_of):
        if adjacency[persona_id]:
            concept_members[concept].append(persona_id)
    concepts = []
    for concept, members in sorted(concept_members.items()):
        inside = set(members)
        strength = {p: sum(w for q, w in adjacency[p].items() if q in inside) for p in members}
        ordered = sorted(members, key=lambda p: (-strength[p], personas[p]["word"]))
        concepts.append({
            "concept": concept,
            "name": terms[personas[ordered[0]]["word"]]["surface"],
            "topic": topic_of_concept[concept],
            "personas": ordered,
            "words": list(dict.fromkeys(terms[personas[p]["word"]]["surface"] for p in ordered)),
        })
    concept_ids = {c["concept"] for c in concepts}
    topics = defaultdict(list)
    for concept in concepts:
        topics[concept["topic"]].append(concept["concept"])

    os.makedirs(arguments.output, exist_ok=True)
    with open(os.path.join(arguments.output, "personas.jsonl"), "w") as handle:
        for persona in personas:
            if adjacency[persona["persona"]] and concept_of[persona["persona"]] in concept_ids:
                handle.write(json.dumps({
                    "persona": persona["persona"], "word": persona["word"], "surface": terms[persona["word"]]["surface"],
                    "concept": concept_of[persona["persona"]],
                    "neighbors": [surface_of.get(k, k) for k in persona["neighbors"]],
                }, ensure_ascii=False) + "\n")
    with open(os.path.join(arguments.output, "concepts.jsonl"), "w") as handle:
        for concept in concepts:
            handle.write(json.dumps(concept, ensure_ascii=False) + "\n")

    # Naming inputs: topics in shards for the topic namers.
    by_id = {c["concept"]: c for c in concepts}
    os.makedirs(os.path.join(arguments.output, "naming"), exist_ok=True)
    ordered_topics = sorted(topics)
    for shard in range(NAMING_SHARDS):
        with open(os.path.join(arguments.output, "naming", f"topics-{shard + 1}.jsonl"), "w") as handle:
            for topic in ordered_topics[shard::NAMING_SHARDS]:
                handle.write(json.dumps({"topic": topic, "concepts": [
                    {"concept": c, "name": by_id[c]["name"], "words": by_id[c]["words"][:NAMING_WORDS_PER_CONCEPT]}
                    for c in topics[topic]]}, ensure_ascii=False) + "\n")

    words_per_concept = sorted(len(c["words"]) for c in concepts)
    concepts_per_topic = sorted(len(members) for members in topics.values())
    stats = {
        "seed": SEED, "ego_threshold": EGO_THRESHOLD, "resolutions": resolutions,
        "words": len(terms), "words_with_relations": len(personas_by_word),
        "personas": len(personas), "connected_personas": len(connected),
        "concepts": len(concepts), "topics": len(topics),
        "words_per_concept": {"median": words_per_concept[len(words_per_concept) // 2], "p90": words_per_concept[int(.9 * len(words_per_concept))], "max": words_per_concept[-1]},
        "concepts_per_topic": {"median": concepts_per_topic[len(concepts_per_topic) // 2], "p90": concepts_per_topic[int(.9 * len(concepts_per_topic))], "max": concepts_per_topic[-1]},
        "personas_per_word": dict(sorted(Counter(len(ids) for ids in personas_by_word.values()).items())),
    }
    with open(os.path.join(arguments.output, "stats.json"), "w") as handle:
        json.dump(stats, handle, indent=2)
    return stats


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--thesaurus", required=True)
    parser.add_argument("--contract", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--co-use", help="co-use relation directory (batches/co-use-*.jsonl), read after the generated relations")
    for level in RESOLUTIONS:
        parser.add_argument(f"--{level}-resolution", dest=level, type=float, help=f"Louvain resolution for {level}s (default {RESOLUTIONS[level]})")
    print(json.dumps(induce(parser.parse_args()), indent=2))


if __name__ == "__main__":
    main()
