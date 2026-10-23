"""Write the review sheets for re-classifying topics (step 6 of docs/related-terms/sense-mapping-log).

Usage: python3 prepare_topic_sheets.py <rt-sense/4 dataset.sqlite3> <output dir>

Writes `topics-NNN.md` (every topic, grouped by primary field) and `duplicates.md` (topics whose labels match,
ignoring case). For each topic a sheet shows its fields in rank order, label, scope, concept names, member words
(heaviest personas first), the primary fields of the members' other topics, and its sibling topics (same number
before the dot, split from one topic). The reviewer writes only the topics to change.
"""

import argparse
from collections import Counter, defaultdict
import sqlite3
from pathlib import Path

SHEET_SIZE = 100
WORDS_SHOWN = 12
CONCEPTS_SHOWN = 6
OTHER_FIELDS_SHOWN = 3
SIBLINGS_SHOWN = 6


def family(topic):
    return topic.split(".")[0]


def load(connection):
    topics = {topic: {"label": label, "scope": scope, "fields": [], "concepts": [], "words": Counter()}
              for topic, label, scope in connection.execute("SELECT topic, label, scope FROM topics")}
    for topic, field in connection.execute("SELECT topic, field FROM topic_fields ORDER BY topic, rank"):
        topics[topic]["fields"].append(field)
    for topic, name in connection.execute("SELECT topic, name FROM concepts ORDER BY concept"):
        topics[topic]["concepts"].append(name)
    word_topics = defaultdict(set)
    for surface, topic, weight in connection.execute(
            "SELECT t.surface, p.topic, p.source_persona_count FROM personas p JOIN terms t USING (term_id)"):
        topics[topic]["words"][surface] += weight
        word_topics[surface].add(topic)
    return topics, word_topics


def other_fields(topic, topics, word_topics):
    counts = Counter()
    for word in topics[topic]["words"]:
        counts.update({topics[other]["fields"][0] for other in word_topics[word] if other != topic})
    return counts


def entry(topic, topics, word_topics, families):
    info = topics[topic]
    words = sorted(info["words"].items(), key=lambda item: (-item[1], item[0].lower()))
    lines = [f"### {topic} [{', '.join(info['fields'])}] {info['label']}", f"- scope: {info['scope']}"]
    if info["concepts"]:
        lines.append(f"- concepts ({len(info['concepts'])}): {', '.join(info['concepts'][:CONCEPTS_SHOWN])}")
    lines.append(f"- words ({len(words)}): {', '.join(w for w, _ in words[:WORDS_SHOWN])}")
    others = other_fields(topic, topics, word_topics).most_common(OTHER_FIELDS_SHOWN)
    if others:
        lines.append(f"- members' other fields: {', '.join(f'{f} {n}' for f, n in others)}")
    siblings = [s for s in families[family(topic)] if s != topic]
    if siblings:
        lines.append("- siblings: " + "; ".join(f"{s} {topics[s]['fields'][0]} {topics[s]['label']}"
                                                for s in siblings[:SIBLINGS_SHOWN]))
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("dataset")
    parser.add_argument("output")
    arguments = parser.parse_args()
    connection = sqlite3.connect(f"file:{arguments.dataset}?mode=ro", uri=True)
    topics, word_topics = load(connection)
    connection.close()
    families = defaultdict(list)
    for topic in sorted(topics):
        families[family(topic)].append(topic)
    output = Path(arguments.output)
    output.mkdir(parents=True, exist_ok=True)
    ordered = sorted(topics, key=lambda t: (topics[t]["fields"][0], t))
    for number, start in enumerate(range(0, len(ordered), SHEET_SIZE), 1):
        body = [entry(t, topics, word_topics, families) for t in ordered[start:start + SHEET_SIZE]]
        (output / f"topics-{number:03d}.md").write_text("\n\n".join(body) + "\n")
    by_label = defaultdict(list)
    for topic in sorted(topics):
        by_label[topics[topic]["label"].lower()].append(topic)
    groups = [group for group in by_label.values() if len(group) > 1]
    (output / "duplicates.md").write_text("\n\n".join(
        f"## {topics[group[0]]['label']}\n\n" + "\n\n".join(entry(t, topics, word_topics, families) for t in group)
        for group in groups) + "\n")
    print(f"{len(topics)} topics in {(len(ordered) - 1) // SHEET_SIZE + 1} sheets, {len(groups)} duplicate labels -> {output}")


if __name__ == "__main__":
    main()
