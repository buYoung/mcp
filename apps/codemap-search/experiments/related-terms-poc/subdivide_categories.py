"""Prepare and check the split of the 52 parent categories into narrow subcategories (internal experiment).

`prepare` writes, for each parent, the canonical words generated under it (`input/<parent>.tsv`, with the
word's other parents for context) and the parent scopes (`parents.json`). The subdivision itself is
written by hand or by agents as `parents/<parent>.json`:

  {"parent": "async",
   "subcategories": [{"id": "async/promises", "label": "Promises", "scope": "...", "members": ["Promise", ...]}],
   "dropped": [{"term": "Kafka", "reason": "message broker, no async-programming sense"}]}

`check` applies the same rules as `build_category_dataset.py`.

Run from the repository root:
  python3 apps/codemap-search/experiments/related-terms-poc/subdivide_categories.py prepare \\
      --thesaurus .codemap/poc/related-terms/thesaurus-v1 \\
      --contract apps/codemap-search/experiments/related-terms-poc/contracts/thesaurus-categories.json \\
      --subdivision .codemap/poc/related-terms/thesaurus-v1/subdivision-1
  python3 apps/codemap-search/experiments/related-terms-poc/subdivide_categories.py check ... [parent ...]
"""

import argparse
import json
import os
import sys

from build_category_dataset import canonical_terms, parent_words, subdivision_errors


def prepare(arguments, scopes, terms):
    os.makedirs(os.path.join(arguments.subdivision, "input"), exist_ok=True)
    os.makedirs(os.path.join(arguments.subdivision, "parents"), exist_ok=True)
    with open(os.path.join(arguments.subdivision, "parents.json"), "w") as handle:
        json.dump([{"id": p, "scope": s} for p, s in scopes.items()], handle, indent=1)
        handle.write("\n")
    for parent in scopes:
        words = parent_words(terms, parent)
        lines = ["word\tother_parents"]
        for key in sorted(words, key=lambda k: (k, words[k])):
            others = ",".join(sorted(terms[key]["parents"] - {parent}))
            lines.append(f"{words[key]}\t{others}")
        with open(os.path.join(arguments.subdivision, "input", f"{parent}.tsv"), "w") as handle:
            handle.write("\n".join(lines) + "\n")
        print(f"{parent}\t{len(words)}")


def check(arguments, scopes, terms):
    failed = False
    for parent in arguments.parents or list(scopes):
        path = os.path.join(arguments.subdivision, "parents", f"{parent}.json")
        if not os.path.exists(path):
            print(f"{parent}: missing {path}")
            failed = True
            continue
        document = json.load(open(path))
        errors = subdivision_errors(document, parent, parent_words(terms, parent))
        if errors:
            failed = True
            print(f"{parent}: {len(errors)} errors")
            for error in errors[:30]:
                print(f"  {error}")
            continue
        sizes = sorted(len(s["members"]) for s in document["subcategories"])
        print(f"{parent}: ok  {len(sizes)} subcategories, sizes {sizes[0]}-{sizes[-1]} (median {sizes[len(sizes) // 2]}), dropped {len(document.get('dropped', []))}")
    sys.exit(1 if failed else 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=("prepare", "check"))
    parser.add_argument("parents", nargs="*")
    parser.add_argument("--thesaurus", required=True)
    parser.add_argument("--contract", required=True)
    parser.add_argument("--subdivision", required=True)
    arguments = parser.parse_args()
    scopes = {row["id"]: row["scope"] for row in json.load(open(arguments.contract))["categories"]}
    terms = canonical_terms(arguments.thesaurus, scopes)
    (prepare if arguments.command == "prepare" else check)(arguments, scopes, terms)


if __name__ == "__main__":
    main()
