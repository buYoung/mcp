"""Draw the sense evaluation words from identifiers collected in the 12 evaluation repositories.

Usage: python3 prepare_eval_set.py <collect dir> <output dir>

<collect dir> holds `<repository>/definitions.jsonl` written by `word-eval/collect_repo.py`. A unit is one to three
adjacent parts of a defined name (`AuthUserKey` -> `auth`, `auth user`, `user key`, ...) whose word key has a field in
the base dataset. Units are drawn per repository with a fixed seed and without repeating a word across repositories:

  target   (10) word whose fields, own or through `input_senses`, are exactly one; the words a mapping step widens
  multi    (6)  word of the same groups with two or more fields
  input    (4)  word classified only through `input_senses`

Writes `candidates.json` (unit, group, field count, one definition with its code lines) and one Markdown sheet per
repository for judging. The sheets do not show the dataset's senses, so the judgement reads only the code.
"""

import collections
import json
import random
import sys
from pathlib import Path

EXPERIMENTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402
from sense_word_groups import DATASETS, recent_words, word_fields  # noqa: E402
from term_canonical import word_key  # noqa: E402

BASE_DATASET = DATASETS / "rts3-48356e35e1f1ea28/dataset.sqlite3"
CHECKOUTS = EXPERIMENTS.parents[3] / ".codemap/poc/related-terms/workspaces/checkouts"
REPOSITORIES = ["bitwarden-server", "express", "flask", "gin", "gutenberg", "keycloak", "redis", "sentry", "terminal",
                "tokio", "vector", "woocommerce"]
QUOTAS = {"target": 10, "multi": 6, "input": 4}
SEED = 20261017
MAX_PARTS = 3
# Kinds that name something on their own; parameters and locals come last when choosing the example definition.
KIND_ORDER = ["function", "method", "class", "struct", "interface", "type", "enum", "field", "property", "constant",
              "interface_method", "variable", "parameter"]


def group_of(key, fields, own, recent):
    if key in own or key in recent:
        return "target" if len(fields[key]) == 1 else "multi"
    return "input"


def units_of(parts):
    for size in range(1, MAX_PARTS + 1):
        for start in range(len(parts) - size + 1):
            yield " ".join(parts[start:start + size])


def code_lines(repository, relative, line, radius=1):
    lines = (CHECKOUTS / repository / relative).read_text(errors="replace").splitlines()
    first = max(1, line - radius)
    return [f"{number}: {lines[number - 1].rstrip()[:200]}" for number in range(first, min(len(lines), line + radius) + 1)]


def repository_units(collect_dir, repository, fields):
    """Units with a field, each with its defining occurrences (non-test, non-generated)."""
    occurrences = collections.defaultdict(list)
    for row in map(json.loads, (collect_dir / repository / "definitions.jsonl").open()):
        if row["is_test"] or row["is_generated"]:
            continue
        parts = [part.lower() for part in row["parts"] if part]
        for unit in dict.fromkeys(units_of(parts)):
            if word_key(unit) in fields:
                occurrences[unit].append(row)
    return occurrences


def example_of(rows):
    rank = {kind: index for index, kind in enumerate(KIND_ORDER)}
    return min(rows, key=lambda r: (rank.get(r["kind"], len(KIND_ORDER)), r["file"], r["line"]))


def main(collect_dir, output_dir):
    collect_dir, output_dir = Path(collect_dir), Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    index = sl.SenseIndex.load(BASE_DATASET, is_topic_split=True)
    fields, own = word_fields(index), set(index.senses)
    recent = recent_words()
    random_source = random.Random(SEED)
    chosen_keys, candidates = set(), []
    for repository in REPOSITORIES:
        occurrences = repository_units(collect_dir, repository, fields)
        by_group = collections.defaultdict(list)
        for unit in sorted(occurrences):
            by_group[group_of(word_key(unit), fields, own, recent)].append(unit)
        for group, quota in QUOTAS.items():
            pool = [u for u in by_group[group] if word_key(u) not in chosen_keys]
            random_source.shuffle(pool)
            for unit in pool[:quota]:
                chosen_keys.add(word_key(unit))
                row = example_of(occurrences[unit])
                candidates.append({
                    "id": f"{repository}-{len(candidates) + 1:03d}", "repository": repository, "word": unit,
                    "group": group, "field_count": len(fields[word_key(unit)]), "is_recent": word_key(unit) in recent,
                    "definition_count": len(occurrences[unit]), "identifier": row["name"], "kind": row["kind"],
                    "language": row["language"], "file": row["file"], "line": row["line"],
                    "code": code_lines(repository, row["file"], row["line"]),
                })
    (output_dir / "candidates.json").write_text(json.dumps(candidates, ensure_ascii=False, indent=1) + "\n")
    for repository in REPOSITORIES:
        lines = [f"# {repository}", ""]
        for item in (c for c in candidates if c["repository"] == repository):
            lines += [f'## {item["id"]} `{item["word"]}`', "",
                      f'`{item["identifier"]}` ({item["kind"]}, {item["language"]}) {item["file"]}:{item["line"]}', "",
                      "```", *item["code"], "```", ""]
        (output_dir / f"{repository}.md").write_text("\n".join(lines))
    print(len(candidates), collections.Counter(c["group"] for c in candidates))


if __name__ == "__main__":
    main(*sys.argv[1:3])
