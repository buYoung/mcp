"""Validate one relation batch output against its input batch (thesaurus-v1 stage B)."""
import json
import sys

KINDS = {"synonym", "abbreviation", "form", "family", "near", "identifier"}


def main(input_path: str, output_path: str) -> int:
    heads = [json.loads(line)["term"] for line in open(input_path) if line.strip()]
    problems: list[str] = []
    rows = []
    for number, line in enumerate(open(output_path), 1):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as error:
            problems.append(f"line {number}: invalid JSON ({error})")
    seen_heads = [row.get("term") for row in rows]
    missing = [head for head in heads if head not in seen_heads]
    extra = [head for head in seen_heads if head not in heads]
    duplicates = sorted({head for head in seen_heads if seen_heads.count(head) > 1})
    if missing:
        problems.append(f"missing heads ({len(missing)}): {missing[:20]}")
    if extra:
        problems.append(f"heads not in input ({len(extra)}): {extra[:20]}")
    if duplicates:
        problems.append(f"duplicate heads: {duplicates[:20]}")
    pair_count = 0
    for row in rows:
        related = row.get("related")
        if not isinstance(related, list):
            problems.append(f"{row.get('term')!r}: related is not a list")
            continue
        words = set()
        for item in related:
            if not (isinstance(item, list) and len(item) == 2 and all(isinstance(part, str) for part in item)):
                problems.append(f"{row['term']!r}: bad item {item!r}")
                continue
            word, kind = item
            pair_count += 1
            if kind not in KINDS:
                problems.append(f"{row['term']!r}: unknown kind {kind!r} for {word!r}")
            if not word.strip() or not word.isascii():
                problems.append(f"{row['term']!r}: non-ASCII or empty word {word!r}")
            if word.strip().lower() == str(row["term"]).strip().lower():
                problems.append(f"{row['term']!r}: repeats the head")
            if word.lower() in words:
                problems.append(f"{row['term']!r}: duplicate word {word!r}")
            words.add(word.lower())
    if problems:
        print("\n".join(problems[:60]))
        print(f"{len(problems)} problem(s)")
        return 1
    print(f"OK {len(rows)} heads, {pair_count} pairs")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
