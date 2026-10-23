"""Turn the mapping review into `mappings.json` for `add_topic_mappings.py` (step 3 of docs/related-terms/sense-mapping-log).

Usage:
  python3 collect_mappings.py topics  <dataset.sqlite3> <work dir>   write fields/<field>.txt for the field picks
  python3 collect_mappings.py combine <dataset.sqlite3> <work dir>   write mappings.json

<work dir> holds `words.json` from `prepare_mapping_sheets.py` and the review files:
  decisions-a.txt   one line per word with additions: `<id> <topic or field> ...`; a word without a line gets none
  decisions-b.txt   one line per field pick: `<id> <field> <topic>`, or `<id> <field> -` when no topic of the field fits
Field sheets list every topic of the field and the words picked for it; a word is never offered a topic it has.
"""

import json
import sqlite3
import sys
from pathlib import Path


def read_lines(path):
    if not path.exists():
        return []
    return [line.split() for line in path.read_text().splitlines() if line.strip() and not line.startswith("#")]


def load(dataset, work):
    connection = sqlite3.connect(f"file:{dataset}?mode=ro", uri=True)
    labels = dict(connection.execute("SELECT topic, label FROM topics"))
    scopes = dict(connection.execute("SELECT topic, scope FROM topics"))
    primary = dict(connection.execute("SELECT topic, field FROM topic_fields WHERE rank = 0"))
    base_id = connection.execute("SELECT value FROM sense_meta WHERE key = 'dataset_id'").fetchone()[0]
    connection.close()
    words = {row["id"]: row for row in json.loads((work / "words.json").read_text())}
    return words, labels, scopes, primary, base_id


def parse_a(work, words, primary):
    """Topic picks and field picks per word id; raises on unknown ids, topics or fields and on current topics."""
    fields = set(primary.values())
    topics, field_picks = {}, {}
    for word_id, *items in read_lines(work / "decisions-a.txt"):
        if word_id not in words:
            raise SystemExit(f"unknown word id {word_id}")
        current = set(words[word_id]["current"])
        for item in items:
            if item in primary:
                if item in current:
                    raise SystemExit(f"{word_id}: {item} is a current topic")
                topics.setdefault(word_id, []).append(item)
            elif item in fields:
                field_picks.setdefault(word_id, []).append(item)
            else:
                raise SystemExit(f"{word_id}: unknown topic or field {item!r}")
    return topics, field_picks


def write_field_sheets(dataset, work):
    words, labels, scopes, primary, _ = load(dataset, work)
    _, field_picks = parse_a(work, words, primary)
    by_field = {}
    for word_id, fields in field_picks.items():
        for field in fields:
            by_field.setdefault(field, []).append(word_id)
    (work / "fields").mkdir(exist_ok=True)
    for field, word_ids in sorted(by_field.items()):
        lines = [f"# {field}: {len(word_ids)} words", "## topics"]
        lines += [f"{t} {labels[t]} — {scopes[t][:110]}" for t in sorted(primary, key=lambda t: labels[t].lower()) if primary[t] == field]
        lines.append("## words")
        for word_id in word_ids:
            row = words[word_id]
            lines.append(f'{word_id} `{row["word"]}` now: ' + "; ".join(f"{t} {primary[t]}/{labels[t]}" for t in row["current"])
                         + (f' | rel: {", ".join(row["related"][:5])}' if row["related"] else ""))
        (work / "fields" / f"{field}.txt").write_text("\n".join(lines) + "\n")
    print({field: len(ids) for field, ids in sorted(by_field.items())})


def combine(dataset, work):
    words, labels, _, primary, base_id = load(dataset, work)
    topics, field_picks = parse_a(work, words, primary)
    picked = {(word_id, field): topic for word_id, field, topic in read_lines(work / "decisions-b.txt")}
    missing = [(w, f) for w, fields in field_picks.items() for f in fields if (w, f) not in picked]
    if missing:
        raise SystemExit(f"field picks without a decision: {missing[:20]}")
    mappings, unplaced = [], []
    for word_id, row in words.items():
        chosen = [(t, "candidate") for t in topics.get(word_id, [])]
        for field in field_picks.get(word_id, []):
            topic = picked[(word_id, field)]
            if topic == "-":
                unplaced.append({"word": row["word"], "field": field})
                continue
            if primary.get(topic) != field or topic in row["current"]:
                raise SystemExit(f"{word_id}: {topic} is not a new topic of {field}")
            chosen.append((topic, "field"))
        for topic, source in dict(chosen).items():
            mappings.append({"word": row["word"], "topic": topic, "field": primary[topic], "label": labels[topic],
                             "group": row["group"], "source": source})
    document = {"base_dataset": base_id, "reviewer": "Claude Opus 5.5", "mappings": mappings,
                "fields_without_topic": unplaced}
    (work / "mappings.json").write_text(json.dumps(document, ensure_ascii=False, indent=1) + "\n")
    print(f"{len(mappings)} mappings for {len({m['word'] for m in mappings})} words, {len(unplaced)} field picks without a topic")


if __name__ == "__main__":
    command, dataset, work = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
    {"topics": write_field_sheets, "combine": combine}[command](dataset, work)
