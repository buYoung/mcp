"""Write the review sheets for mapping one-field words to more topics (step 3 of docs/related-terms/sense-mapping-log).

Usage: python3 prepare_mapping_sheets.py <base rt-sense/4 dataset.sqlite3> <output dir> [--pilot]

Targets are words with exactly one field (own senses and `input_senses` together) that are headwords of the
dataset or were added by the recent steps (`sense_word_groups.recent_words`). `--pilot` takes a fixed-seed sample of
100 recent and 100 other words; without it every target is written, pilot words first.

For each word a sheet shows its current topics, its related words, the words that list it, and candidate topics
outside its current topics, scored from where those words live:
  own related word in a topic          synonym/abbreviation 2, identifier/near 1, co_use/family 0.5 (form not read)
  word that lists it, through the persona of that relation   1
  topic label naming the word          2
Candidates in the word's current field are left out: the step widens words to other fields. The reviewer writes
topic ids, or a field id when no listed topic fits (a later sheet picks the topic in that field).
"""

import argparse
from collections import Counter, defaultdict
import json
import random
import sqlite3
import sys
from pathlib import Path

EXPERIMENTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402
from sense_word_groups import recent_words, word_fields  # noqa: E402

SEED = 20261017
PILOT_SIZE = 100
SHEET_SIZE = 100
KIND_WEIGHTS = {"synonym": 2, "abbreviation": 2, "identifier": 1, "near": 1, "co_use": 0.5, "family": 0.5}
CANDIDATES_SHOWN = 6
RELATED_SHOWN = 8
LISTED_BY_SHOWN = 6


def targets(index):
    fields, recent = word_fields(index), recent_words()
    own = set(index.senses)
    chosen = {key: ("recent" if key in recent else "original") for key, found in fields.items()
              if len(found) == 1 and (key in own or key in recent)}
    return chosen


def word_topics(connection):
    topics = defaultdict(set)
    for key, topic in connection.execute("SELECT t.word_key, p.topic FROM personas p JOIN terms t USING (term_id)"):
        topics[key].add(topic)
    for key, topic in connection.execute("SELECT i.word_key, p.topic FROM input_senses i JOIN personas p USING (persona)"):
        topics[key].add(topic)
    return topics


def candidate_scores(key, connection, topics_of, term_id, label_index, primary):
    scores = Counter()
    if term_id is not None:
        for related, kind in connection.execute("SELECT word_key, kind FROM relations WHERE term_id = ?", (term_id,)):
            for topic in topics_of.get(related, ()):
                scores[topic] += KIND_WEIGHTS.get(kind, 0)
    for (topic,) in connection.execute(
        "SELECT p.topic FROM relations r JOIN personas p USING (persona) WHERE r.word_key = ?", (key,)
    ):
        scores[topic] += 1
    for topic in label_index.get(key, ()):
        scores[topic] += 2
    current_fields = {primary[topic] for topic in topics_of.get(key, ())}
    ranked = sorted(scores.items(), key=lambda item: (-item[1], item[0]))  # ties by topic id, independent of set order
    return [topic for topic, score in ranked if score >= 1 and primary[topic] not in current_fields][:CANDIDATES_SHOWN]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("dataset", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--pilot", action="store_true")
    arguments = parser.parse_args()
    index = sl.SenseIndex.load(arguments.dataset, is_topic_split=True)
    chosen = targets(index)
    by_group = {group: sorted(k for k, g in chosen.items() if g == group) for group in ("recent", "original")}
    random_source = random.Random(SEED)
    pilot = [k for group in ("recent", "original") for k in random_source.sample(by_group[group], PILOT_SIZE)]
    rest = sorted(set(chosen) - set(pilot))
    keys = pilot if arguments.pilot else pilot + rest

    connection = sqlite3.connect(f"file:{arguments.dataset}?mode=ro", uri=True)
    topics_of = word_topics(connection)
    term_ids = dict(connection.execute("SELECT word_key, term_id FROM terms"))
    labels = dict(connection.execute("SELECT topic, label FROM topics"))
    primary = dict(connection.execute("SELECT topic, field FROM topic_fields WHERE rank = 0"))
    label_index = defaultdict(set)
    for topic, label in labels.items():
        for token in label.lower().replace("-", " ").split():
            label_index[token].add(topic)
    rows = []
    for number, key in enumerate(keys, 1):
        term_id = term_ids.get(key)
        related = [s for (s,) in connection.execute(
            "SELECT surface FROM relations WHERE term_id = ? AND kind <> 'form' ORDER BY rank LIMIT ?", (term_id, RELATED_SHOWN))] if term_id else []
        listed_by = [s for (s,) in connection.execute(
            "SELECT t.surface FROM relations r JOIN terms t USING (term_id) WHERE r.word_key = ? AND r.kind <> 'form' LIMIT ?",
            (key, LISTED_BY_SHOWN))]
        rows.append({
            "id": f"m{number:05d}", "word": index.surfaces[key], "group": chosen[key], "is_pilot": number <= len(pilot),
            "current": sorted(topics_of[key]), "related": related, "listed_by": listed_by,
            "candidates": candidate_scores(key, connection, topics_of, term_id, label_index, primary),
        })
    connection.close()
    arguments.output.mkdir(parents=True, exist_ok=True)
    (arguments.output / "words.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1) + "\n")

    def topic_text(topic):
        return f"{topic} {primary[topic]}/{labels[topic]}"

    for start in range(0, len(rows), SHEET_SIZE):
        lines = []
        for row in rows[start:start + SHEET_SIZE]:
            lines.append(f'{row["id"]} `{row["word"]}` [{row["group"]}] now: ' + "; ".join(topic_text(t) for t in row["current"]))
            if row["related"]:
                lines.append("  rel: " + ", ".join(row["related"]))
            if row["listed_by"]:
                lines.append("  by: " + ", ".join(row["listed_by"]))
            if row["candidates"]:
                lines.append("  cand: " + " | ".join(topic_text(t) for t in row["candidates"]))
        (arguments.output / f"sheet-{start // SHEET_SIZE + 1:03d}.txt").write_text("\n".join(lines) + "\n")
    print(len(rows), Counter(r["group"] for r in rows), f"pilot {len(pilot)}")


if __name__ == "__main__":
    main()
