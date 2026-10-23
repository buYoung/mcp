"""Review which concept each word meaning (persona) belongs to, with two independent agent passes (internal experiment).

Question for every membership: an LLM searching code with a word of this concept gets this word as a close related
word; in the meaning its related words show, would code relevant to the concept likely also use this word?
Answers: keep, move (to a listed concept), split (its related words cover several meanings), merge (the concept and a
listed concept are one), detach (no listed concept fits: start a new concept under a listed topic). A wrong set of
related words is only flagged (`evidence_wrong`); the generated relation files are never edited.

Only memberships the first pass wants to change go to a second pass, which judges them without seeing the first
answer. A change is applied only where both passes agree; disagreements are kept as they are and logged. Every
applied change is logged as word + related words so it can be re-applied after the graph is recomputed.

Subcommands (run from the repository root):
  items   --graph G --review R            write R/pass-1/batch-NN.jsonl (whole concepts, plus referenced concepts/topics)
  check   --graph G --review R --pass N   validate R/pass-N/decisions-NN.jsonl (optionally --batch NN)
  second  --graph G --review R            write R/pass-2/batch-NN.jsonl with the items pass 1 wants to change
  apply   --graph G --review R --output O write graph O (personas, concepts, names) and R/changes.jsonl, R/disagreements.jsonl
  third   --graph G --previous-review P --review R
                                          write blind pass-3 batches for disagreements where neither answer is keep
  apply-third --graph G --previous-review P --review R --output O
                                          apply a third answer only when its action and target match either prior answer
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import glob
import json
import os
import re
import shutil
import sys

from build_sense_dataset import load_fields, load_graph, named_topics, read_jsonl
from term_canonical import word_key

ITEMS_PER_BATCH = 400
EVIDENCE_SHOWN = 15
MEMBERS_SHOWN = 25
REFERENCE_MEMBERS = 10
ALTERNATIVES = 5
TOPIC_CHOICES = 3
ACTIONS = ("keep", "move", "split", "merge", "detach")


def write_jsonl(path, rows):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as handle:
        handle.writelines(json.dumps(row, ensure_ascii=False) + "\n" for row in rows)


class Graph:
    """Personas, concepts and named topics of one sense graph."""

    def __init__(self, path):
        self.path = path
        self.concepts, self.auto_topics = load_graph(path)
        self.named, errors = named_topics(path, self.auto_topics)
        if errors:
            sys.exit(f"{path}: topic names are incomplete ({errors[0]})")
        self.personas = {p["persona"]: p for p in read_jsonl(os.path.join(path, "personas.jsonl"))}
        self.topic_of_concept = {c: t for t, topic in self.named.items() for c in topic["concepts"]}
        self.members = defaultdict(list)
        for persona in self.personas.values():
            self.members[persona["concept"]].append(persona["persona"])
        self.member_words = {c: {word_key(self.personas[p]["surface"]) for p in ps} for c, ps in self.members.items()}
        self.concepts_of_word, self.topics_of_word = defaultdict(set), defaultdict(set)
        for concept, words in self.member_words.items():
            for word in words:
                self.concepts_of_word[word].add(concept)
                self.topics_of_word[word].add(self.topic_of_concept[concept])

    def surfaces(self, concept, limit):
        seen = []
        for p in self.members[concept]:
            surface = self.personas[p]["surface"]
            if surface not in seen:
                seen.append(surface)
        return seen[:limit]

    def alternatives(self, persona):
        """Other concepts holding the most of this persona's related words."""
        counts = Counter(c for k in {word_key(w) for w in persona["neighbors"]} for c in self.concepts_of_word.get(k, ()))
        counts.pop(persona["concept"], None)
        return [c for c, _ in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))[:ALTERNATIVES]]

    def topic_choices(self, persona):
        """Topics holding the most of this persona's related words."""
        counts = Counter(t for k in {word_key(w) for w in persona["neighbors"]} for t in self.topics_of_word.get(k, ()))
        return [t for t, _ in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))[:TOPIC_CHOICES]] or [self.topic_of_concept[persona["concept"]]]


def item_id(persona_id):
    return f"M{persona_id:06d}"


def review_batches(graph, personas_by_concept, item_filter=None, items_per_batch=ITEMS_PER_BATCH):
    """Batches of whole concepts in topic order, each with the referenced concepts and topics appended."""
    ordered = sorted(personas_by_concept, key=lambda c: (graph.topic_of_concept[c], c))
    batches, current, size = [], [], 0
    for concept in ordered:
        items = [p for p in personas_by_concept[concept] if item_filter is None or item_id(p) in item_filter]
        if not items:
            continue
        if current and size + len(items) > items_per_batch:
            batches.append(current)
            current, size = [], 0
        current.append((concept, items))
        size += len(items)
    if current:
        batches.append(current)

    rendered = []
    for batch in batches:
        rows, concepts_used, topics_used = [], set(), set()
        for concept, items in batch:
            entries = []
            for p in items:
                persona = graph.personas[p]
                alternatives, topics = graph.alternatives(persona), graph.topic_choices(persona)
                concepts_used.update(alternatives)
                topics_used.update(topics)
                entries.append({"item": item_id(p), "word": persona["surface"], "evidence": persona["neighbors"][:EVIDENCE_SHOWN],
                                "alternatives": alternatives, "topics": topics})
            topics_used.add(graph.topic_of_concept[concept])
            rows.append({"type": "review", "concept": concept, "name": graph.concepts[concept]["name"], "topic": graph.topic_of_concept[concept],
                         "members": graph.surfaces(concept, MEMBERS_SHOWN), "items": entries})
        reviewed = {concept for concept, _ in batch}
        for concept in sorted(concepts_used - reviewed):
            topics_used.add(graph.topic_of_concept[concept])
            rows.append({"type": "concept", "concept": concept, "name": graph.concepts[concept]["name"], "topic": graph.topic_of_concept[concept],
                         "members": graph.surfaces(concept, REFERENCE_MEMBERS)})
        for topic in sorted(topics_used):
            rows.append({"type": "topic", "topic": topic, "label": graph.named[topic]["label"], "scope": graph.named[topic]["scope"]})
        rendered.append(rows)
    return rendered


def write_batches(directory, batches):
    for old in glob.glob(os.path.join(directory, "batch-*.jsonl")):
        os.remove(old)
    for n, rows in enumerate(batches, 1):
        write_jsonl(os.path.join(directory, f"batch-{n:02d}.jsonl"), rows)
    items = sum(len(r["items"]) for rows in batches for r in rows if r["type"] == "review")
    print(f"{directory}: {len(batches)} batches, {items} items")


def batch_items(path):
    """{item: (allowed concept ids, allowed topic ids, evidence words)} of one batch file."""
    allowed = {}
    for row in read_jsonl(path):
        if row["type"] == "review":
            for item in row["items"]:
                allowed[item["item"]] = (set(item["alternatives"]), set(item["topics"]), item["evidence"], row["concept"])
    return allowed


def decision_errors(decision, allowed):
    alternatives, topics, evidence, current = allowed
    action = decision.get("action")
    if action not in ACTIONS:
        return f"unknown action {action!r}"
    if action == "move" and decision.get("to") not in alternatives:
        return f"move target {decision.get('to')!r} is not a listed alternative"
    if action == "merge" and decision.get("with") not in alternatives:
        return f"merge target {decision.get('with')!r} is not a listed alternative"
    if action == "detach":
        if decision.get("topic") not in topics:
            return f"detach topic {decision.get('topic')!r} is not a listed topic"
        if not str(decision.get("name", "")).strip():
            return "detach needs a name for the new concept"
    if action == "split":
        parts = decision.get("parts") or []
        targets = [p.get("to") for p in parts]
        words = [w for p in parts for w in p.get("evidence", [])]
        if len(parts) < 2 or len(set(map(str, targets))) != len(targets):
            return "split needs 2+ parts with distinct targets"
        if any(t != current and t not in alternatives for t in targets):
            return "split targets must be the current concept or listed alternatives"
        if current not in targets:
            return "one split part must stay in the current concept"
        if len(words) != len(set(words)) or not set(words) <= set(evidence):
            return "split evidence must be shown related words, each used once"
    return None


def check(arguments):
    directory = os.path.join(arguments.review, f"pass-{arguments.pass_number}")
    pattern = f"batch-{arguments.batch}.jsonl" if arguments.batch else "batch-*.jsonl"
    errors, counts = [], Counter()
    paths = sorted(glob.glob(os.path.join(directory, pattern)))
    if not paths:
        errors.append(f"no batches found in {directory}")
    for path in paths:
        allowed = batch_items(path)
        output = path.replace("batch-", "decisions-")
        if not os.path.exists(output):
            errors.append(f"{os.path.basename(output)} missing")
            continue
        seen = set()
        for n, line in enumerate(open(output), 1):
            try:
                decision = json.loads(line)
            except ValueError:
                errors.append(f"{os.path.basename(output)}:{n} is not JSON")
                continue
            item = decision.get("item")
            if item not in allowed:
                errors.append(f"{os.path.basename(output)}:{n} unknown item {item!r}")
                continue
            if item in seen:
                errors.append(f"{item} answered twice")
            seen.add(item)
            problem = decision_errors(decision, allowed[item])
            if problem:
                errors.append(f"{item}: {problem}")
            counts[decision.get("action")] += 1
        for item in sorted(set(allowed) - seen)[:5]:
            errors.append(f"{item} not answered in {os.path.basename(output)}")
        if set(allowed) - seen:
            errors.append(f"{len(set(allowed) - seen)} items not answered in {os.path.basename(output)}")
    if errors:
        print(f"{len(errors)} errors")
        for error in errors[:40]:
            print(f"  {error}")
        sys.exit(1)
    print(f"ok: {dict(counts)}")


def decisions(review, pass_number):
    found = {}
    for path in sorted(glob.glob(os.path.join(review, f"pass-{pass_number}", "decisions-*.jsonl"))):
        for row in read_jsonl(path):
            found[row["item"]] = row
    return found


def items(arguments):
    graph = Graph(arguments.graph)
    write_batches(os.path.join(arguments.review, "pass-1"), review_batches(graph, graph.members))


def second(arguments):
    graph = Graph(arguments.graph)
    proposed = {item for item, d in decisions(arguments.review, 1).items() if d["action"] != "keep"}
    write_batches(os.path.join(arguments.review, "pass-2"), review_batches(graph, graph.members, proposed))


def pending_changes(review):
    """Disagreements where both independent answers requested a change."""
    return {row["item"]: row for row in read_jsonl(os.path.join(review, "disagreements.jsonl"))
            if row["first"]["action"] != "keep" and row.get("second") and row["second"]["action"] != "keep"}


def third(arguments):
    graph = Graph(arguments.graph)
    pending = pending_changes(arguments.previous_review)
    directory = os.path.join(arguments.review, "pass-3")
    if os.path.exists(directory):
        sys.exit(f"{directory} already exists; resume its decisions instead of replacing batches")
    available = {item_id(p) for p in graph.personas}
    if set(pending) - available:
        sys.exit("the input graph is missing pending personas")
    write_batches(directory, review_batches(graph, graph.members, set(pending), arguments.items_per_batch))
    with open(os.path.join(arguments.review, "prepare-summary.json"), "w") as handle:
        json.dump({"graph": arguments.graph, "previous_review": arguments.previous_review, "items": len(pending),
                   "input": "original membership information; no prior answers",
                   "agreement": "same action and target; split compares every moved evidence word"}, handle, indent=2)
    shutil.copy(os.path.join(arguments.previous_review, "brief.md"), os.path.join(arguments.review, "brief.md"))


def decision_signature(answer, current):
    """Ignore prose and ordering; split targets include the full evidence assignment away from the current concept."""
    action = answer["action"]
    if action == "move":
        return action, answer["to"]
    if action == "merge":
        return action, answer["with"]
    if action == "detach":
        return action, answer["topic"]
    if action == "split":
        moved = sorted((word, part["to"]) for part in answer["parts"] if part["to"] != current for word in part["evidence"])
        return action, tuple(moved)
    return (action,)


def third_answers(arguments, graph):
    """Match only after blind decisions have been saved; keep and three-way disagreements never change the graph."""
    pending = pending_changes(arguments.previous_review)
    answers = decisions(arguments.review, 3)
    if set(answers) != set(pending):
        sys.exit("pass 3 must answer exactly every non-keep disagreement")
    allowed = {}
    for path in sorted(glob.glob(os.path.join(arguments.review, "pass-3", "batch-*.jsonl"))):
        allowed.update(batch_items(path))
    if set(allowed) != set(pending):
        sys.exit("pass-3 batches must contain exactly every non-keep disagreement")
    matched = {}
    for item, answer in answers.items():
        problem = decision_errors(answer, allowed[item])
        if problem:
            sys.exit(f"{item}: {problem}")
        current = allowed[item][3]
        if graph.personas[int(item[1:])]["concept"] != current:
            sys.exit(f"{item}: membership changed since the original review")
        if answer["action"] != "keep":
            for previous in (pending[item]["first"], pending[item]["second"]):
                if decision_signature(answer, current) == decision_signature(previous, current):
                    matched[item] = previous
                    break
    return answers, matched, pending


def continued_topic(graph, topic, persona):
    """Attach a new concept to a descendant of its agreed original topic, retaining the split graph's names and fields."""
    ancestry_path = os.path.join(graph.path, "naming", "named-topics.json")
    if not os.path.exists(ancestry_path):
        if topic in graph.named:
            return topic
        sys.exit(f"cannot continue topic {topic}: no topic ancestry")
    ancestry = json.load(open(ancestry_path))
    candidates = [t for t, row in graph.named.items() if ancestry[str(row["source_topic"])]["named_topic"] == topic]
    if not candidates:
        sys.exit(f"cannot continue topic {topic}: no surviving descendant")
    counts = Counter(t for word in {word_key(w) for w in persona["neighbors"]} for t in graph.topics_of_word.get(word, ())
                     if t in candidates)
    current = graph.topic_of_concept[persona["concept"]]
    return min(candidates, key=lambda t: (-counts[t], t != current, t))


def agreed_change(first, second_answer, current):
    """The change both passes agree on, or None."""
    action = first["action"]
    if action != second_answer["action"] or action == "keep":
        return None
    if action == "move" and first["to"] == second_answer["to"]:
        return {"action": "move", "to": first["to"]}
    if action == "merge" and first["with"] == second_answer["with"]:
        return {"action": "merge", "with": first["with"]}
    if action == "detach" and first["topic"] == second_answer["topic"]:
        return {"action": "detach", "topic": first["topic"], "name": first["name"].strip()}
    if action == "split":
        target_of = {}
        for answer in (first, second_answer):
            for part in answer["parts"]:
                for word in part["evidence"]:
                    target_of.setdefault(word, []).append(part["to"])
        moved = defaultdict(list)
        for word, targets in target_of.items():
            if len(targets) == 2 and targets[0] == targets[1] and targets[0] != current:
                moved[targets[0]].append(word)
        if moved:
            return {"action": "split", "moved": dict(moved)}
    return None


def apply(arguments):
    graph = Graph(arguments.graph)
    is_third = arguments.command == "apply-third"
    pending = {}
    if is_third:
        first, second_pass, pending = third_answers(arguments, graph)
    else:
        first, second_pass = decisions(arguments.review, 1), decisions(arguments.review, 2)
    personas = {p: dict(row) for p, row in graph.personas.items()}
    concepts = {c: dict(row) for c, row in graph.concepts.items()}
    names_rows = []
    for path in sorted(glob.glob(os.path.join(graph.path, "names", "topics-*.jsonl"))):
        names_rows += read_jsonl(path)
    group_of_topic = {}
    for row in names_rows:
        for index, group in enumerate(row["groups"]):
            group_of_topic[f"t{row['topic']}" if len(row["groups"]) == 1 else f"t{row['topic']}.{index + 1}"] = group

    changes, disagreements, merges, flagged = [], [], [], 0
    next_persona = max(personas) + 1
    next_concept = max(concepts) + 1
    new_concepts = {}
    for item, answer in sorted(first.items()):
        persona_id = int(item[1:])
        persona = graph.personas[persona_id]
        flagged += bool(answer.get("evidence_wrong"))
        if answer["action"] == "keep":
            continue
        record = {"item": item, "persona": persona_id, "word": persona["surface"], "evidence": persona["neighbors"],
                  "concept": graph.concepts[persona["concept"]]["name"], "first": answer, "second": second_pass.get(item)}
        if is_third:
            record.update({"first": pending[item]["first"], "second": pending[item]["second"], "third": answer})
        change = agreed_change(answer, second_pass[item], persona["concept"]) if item in second_pass else None
        if not change:
            disagreements.append(record)
            continue
        if is_third and change["action"] == "detach":
            record["agreed_topic"] = change["topic"]
            change["topic"] = continued_topic(graph, change["topic"], persona)
        targets = ([change["to"]] if change["action"] == "move" else [change["with"]] if change["action"] == "merge"
                   else list(change["moved"]) if change["action"] == "split" else [])
        if any(target not in concepts for target in targets):
            sys.exit(f"{item}: an agreed target no longer exists in the input graph")
        record["applied"] = change
        if change["action"] == "move":
            personas[persona_id]["concept"] = change["to"]
            record["to"] = graph.concepts[change["to"]]["name"]
        elif change["action"] == "merge":
            merges.append((persona["concept"], change["with"]))
            record["to"] = graph.concepts[change["with"]]["name"]
        elif change["action"] == "detach":
            key = (change["topic"], change["name"].lower())
            if key not in new_concepts:
                topic = change["topic"]
                new_concepts[key] = next_concept
                concepts[next_concept] = {"concept": next_concept, "name": change["name"], "topic": automatic_topic(topic), "personas": [], "words": []}
                group_of_topic[topic]["concepts"].append(next_concept)
                next_concept += 1
            personas[persona_id]["concept"] = new_concepts[key]
            record["to"] = change["name"]
        elif change["action"] == "split":
            moved_words = {w for words in change["moved"].values() for w in words}
            personas[persona_id]["neighbors"] = [w for w in persona["neighbors"] if w not in moved_words]
            for target, words in change["moved"].items():
                personas[next_persona] = {"persona": next_persona, "word": persona["word"], "surface": persona["surface"], "concept": target,
                                          "neighbors": words, "split_from": persona_id}
                next_persona += 1
            record["to"] = {graph.concepts[t]["name"]: words for t, words in change["moved"].items()}
        changes.append(record)

    # Merges: union-find over concept pairs; the concept with more members survives.
    parent = {}

    def find(c):
        while parent.get(c, c) != c:
            c = parent[c]
        return c

    size = Counter(p["concept"] for p in personas.values())
    for a, b in merges:
        ra, rb = find(a), find(b)
        if ra != rb:
            keep, drop = (ra, rb) if (size[ra], -ra) >= (size[rb], -rb) else (rb, ra)
            parent[drop] = keep
            size[keep] += size[drop]
    for persona in personas.values():
        persona["concept"] = find(persona["concept"])
    for row in names_rows:
        for group in row["groups"]:
            group["concepts"] = [c for c in group["concepts"] if find(c) == c]
    names_rows = [row for row in names_rows if any(group["concepts"] for group in row["groups"])]

    # Concept members and words from the personas now in them.
    in_concept = defaultdict(list)
    for p in sorted(personas):
        in_concept[personas[p]["concept"]].append(p)
    output_concepts = []
    for concept, row in sorted(concepts.items()):
        if find(concept) != concept or not in_concept[concept]:
            continue
        auto_topic = next(r["topic"] for r in names_rows for g in r["groups"] if concept in g["concepts"])
        output_concepts.append({**row, "topic": auto_topic, "personas": in_concept[concept],
                                "words": list(dict.fromkeys(personas[p]["surface"] for p in in_concept[concept]))})
    kept = {c["concept"] for c in output_concepts}
    for row in names_rows:
        for group in row["groups"]:
            group["concepts"] = [c for c in group["concepts"] if c in kept]

    os.makedirs(os.path.join(arguments.output, "names"), exist_ok=True)
    write_jsonl(os.path.join(arguments.output, "personas.jsonl"), [personas[p] for p in sorted(personas) if personas[p]["concept"] in kept])
    write_jsonl(os.path.join(arguments.output, "concepts.jsonl"), output_concepts)
    write_jsonl(os.path.join(arguments.output, "names", "topics-reviewed.jsonl"), names_rows)
    live_topics = {f"t{r['topic']}" if len(r["groups"]) == 1 else f"t{r['topic']}.{i + 1}" for r in names_rows for i in range(len(r["groups"]))}
    fields = {t: f for t, f in load_fields(graph.path)["topics"].items() if t in live_topics}
    with open(os.path.join(arguments.output, "names", "field-assign-reviewed.json"), "w") as handle:
        json.dump(fields, handle, ensure_ascii=False, indent=1)
    shutil.copy(os.path.join(graph.path, "names", "field-list.json"), os.path.join(arguments.output, "names", "field-list.json"))
    write_jsonl(os.path.join(arguments.review, "changes.jsonl"), changes)
    write_jsonl(os.path.join(arguments.review, "disagreements.jsonl"), disagreements)
    summary = {"first_pass": dict(Counter(d["action"] for d in first.values())), "second_pass": dict(Counter(d["action"] for d in second_pass.values())),
               "applied": dict(Counter(c["applied"]["action"] for c in changes)), "disagreements": len(disagreements),
               "evidence_flagged": flagged, "concepts": len(output_concepts), "new_concepts": len(new_concepts),
               "merged_concepts": sum(1 for c in concepts if find(c) != c), "personas": sum(1 for p in personas.values() if p["concept"] in kept)}
    if is_third:
        summary.pop("first_pass")
        summary.pop("second_pass")
        summary.update({"eligible": len(pending), "third_pass": dict(Counter(d["action"] for d in first.values())),
                        "kept": sum(d["action"] == "keep" for d in first.values()),
                        "affected_headwords": len({word_key(row["word"]) for row in changes}),
                        "matched_first": sum(decision_signature(row["third"], graph.personas[row["persona"]]["concept"]) ==
                                             decision_signature(row["first"], graph.personas[row["persona"]]["concept"]) for row in changes),
                        "graph": arguments.graph, "previous_review": arguments.previous_review})
    with open(os.path.join(arguments.review, "apply-summary.json"), "w") as handle:
        json.dump(summary, handle, indent=2)
    print(json.dumps(summary))


def automatic_topic(topic_id):
    """The automatic topic a named topic id (`t10` or `t10.2`) belongs to."""
    return int(re.match(r"t(\d+)", topic_id).group(1))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=("items", "check", "second", "apply", "third", "apply-third"))
    parser.add_argument("--graph", required=True)
    parser.add_argument("--review", required=True)
    parser.add_argument("--pass", dest="pass_number", type=int, default=1)
    parser.add_argument("--batch", help="check: only batch NN")
    parser.add_argument("--output", help="apply / apply-third: directory for the reviewed graph")
    parser.add_argument("--previous-review", help="third / apply-third: the original two-pass review")
    parser.add_argument("--items-per-batch", type=int, default=ITEMS_PER_BATCH, help="third: items per batch, keeping whole concepts together")
    arguments = parser.parse_args()
    if arguments.command in ("third", "apply-third"):
        if not arguments.previous_review or os.path.realpath(arguments.review) == os.path.realpath(arguments.previous_review):
            parser.error(f"{arguments.command} needs a separate --review and --previous-review")
    if arguments.command in ("apply", "apply-third") and not arguments.output:
        parser.error(f"{arguments.command} needs --output")
    if arguments.command == "apply-third" and os.path.exists(arguments.output):
        parser.error("apply-third needs a new output directory")
    if arguments.items_per_batch < 1:
        parser.error("--items-per-batch must be positive")
    {"items": items, "check": check, "second": second, "apply": apply, "third": third, "apply-third": apply}[arguments.command](arguments)


if __name__ == "__main__":
    main()
