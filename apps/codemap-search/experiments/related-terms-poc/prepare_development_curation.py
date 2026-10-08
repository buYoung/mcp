"""Offline review packets only; never builds/activates a serving dataset.

Selection is not semantic approval. Existing source archives and the pinned
rtn-2 dataset are read-only inputs. No evaluation cases or repository scans.
"""
from __future__ import annotations

import argparse
import collections
import json
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path

from rtpoc_common import (
    DATA_ROOT, EXPERIMENT_DIR, REPO_ROOT, SOURCES_LOCK_PATH,
    canonical_json, exact_key, read_json, sha256_bytes, sha256_file, write_json,
)

DATASET_ID = "rtd1-8efd9676107a983d"
DATASET = DATA_ROOT / "datasets" / DATASET_ID
CURATION = DATA_ROOT / "development-curation"
PACKETS = CURATION / "packets"
RULES = EXPERIMENT_DIR / "contracts" / "development-curation-rules.json"
CONTRACT = EXPERIMENT_DIR / "contracts" / "development-curation.md"
KINDS = ("sources", "concepts", "expressions", "lexicalizations", "relations")
KEYS = dict(zip(KINDS, ("source_id", "concept_id", "expression_id", "lexicalization_id", "relation_id")))


def jsonl(path: Path):
    with path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            value = json.loads(line)
            if canonical_json(value) + "\n" != line:
                raise ValueError(f"noncanonical JSONL: {path}:{line_number}")
            yield value


def input_inventory() -> dict:
    manifest = read_json(DATASET / "manifest.json")
    if (manifest["dataset_id"], manifest["schema_version"], manifest["normalization_version"]) != (
        DATASET_ID, "rt-dataset/1", "rtn-2"
    ):
        raise ValueError("unexpected source dataset contract")
    for kind in KINDS:
        if sha256_file(DATASET / f"{kind}.jsonl") != manifest["jsonl_sha256"][kind]:
            raise ValueError(f"source dataset hash mismatch: {kind}")
    identity = "rt-dataset/1\nrtn-2\n" + "".join(
        f"{kind}\t{manifest['jsonl_sha256'][kind]}\n" for kind in KINDS
    )
    if "rtd1-" + sha256_bytes(identity.encode())[:16] != DATASET_ID:
        raise ValueError("dataset ID mismatch")
    lock = read_json(SOURCES_LOCK_PATH)
    for source in lock["sources"]:
        for path_key, hash_key in (("archive_path", "archive_sha256"),
                                   ("license_archive_path", "license_archive_sha256")):
            if path_key in source and sha256_file(REPO_ROOT / source[path_key]) != source[hash_key]:
                raise ValueError(f"source archive hash mismatch: {source['source_id']}")
    return {
        "dataset_manifest": manifest,
        "dataset_manifest_sha256": sha256_file(DATASET / "manifest.json"),
        "sources_lock_sha256": sha256_file(SOURCES_LOCK_PATH),
        "source_archives": [{k: v for k, v in source.items() if k in (
            "source_id", "archive_path", "archive_sha256", "license_archive_path", "license_archive_sha256"
        )} for source in lock["sources"]],
        "rules_sha256": sha256_file(RULES),
        "script_sha256": sha256_file(Path(__file__)),
        "contract_sha256": sha256_file(CONTRACT),
    }


def checked_records(kind: str, manifest: dict):
    previous = None
    count = 0
    for record in jsonl(DATASET / f"{kind}.jsonl"):
        key = record[KEYS[kind]]
        if previous is not None and key <= previous:
            raise ValueError(f"duplicate or unsorted source {kind}: {key}")
        previous = key
        count += 1
        yield record
    if count != manifest["record_counts"][kind]:
        raise ValueError(f"source count mismatch: {kind}")


def selection(concept: dict, lexicalizations: list[dict], rules: dict) -> tuple[str | None, list[str]]:
    kind = concept["concept_kind"]
    if kind == "github_topic_reference":
        return None, ["github_topic_reference"]
    if kind in ("github_topic", "wikidata_item"):
        return "catalogs", ["catalog_all_records"]
    if kind in ("abbreviation_sense", "acronym_sense"):
        return "abbreviations", ["abbreviation_all_records"]
    if kind != "wordnet_synset":
        raise ValueError(f"unregistered concept kind: {kind}")
    surfaces = {lex["surface"] for lex in lexicalizations}
    reasons = []
    if concept["pos"] in rules["operations_pos"] and surfaces.intersection(rules["operations_lemmas"]):
        reasons.append("operations_lemma_all_verb_senses")
    if concept["pos"] in rules["state_pos"] and surfaces.intersection(rules["state_lemmas"]):
        reasons.append("state_lemma_all_adjective_senses")
    if reasons:
        return "wordnet-operations", reasons
    reasons = technical_reasons(concept, lexicalizations, rules)
    return ("wordnet-technical", reasons) if reasons else (None, ["wordnet_not_selected"])


def technical_reasons(concept: dict, lexicalizations: list[dict], rules: dict) -> list[str]:
    reasons = []
    if set(concept["domain_labels"]).intersection(rules["technical_domain_labels"]):
        reasons.append("direct_technical_domain")
    pattern = rules["strong_technical_pattern"]
    if re.search(pattern, concept["gloss"] or "", re.I):
        reasons.append("strong_technical_gloss")
    if any(re.search(pattern, lex["surface"], re.I) for lex in lexicalizations):
        reasons.append("strong_technical_surface")
    if {exact_key(lex["surface"]) for lex in lexicalizations}.intersection(rules["ambiguous_technical_surfaces"]):
        reasons.append("ambiguous_technical_exact_surface_all_senses")
    return reasons


def old_selection(concept: dict, lexicalizations: list[dict], rules: dict) -> str | None:
    if concept["concept_kind"] != "wordnet_synset":
        return selection(concept, lexicalizations, rules)[0]
    surfaces = {lex["surface"] for lex in lexicalizations}
    if surfaces.intersection(rules["operations_lemmas"] + rules["state_lemmas"]):
        return "wordnet-operations"
    text = "\n".join([concept["gloss"] or ""] + sorted(surfaces))
    if set(concept["domain_labels"]).intersection(rules["technical_domain_labels"]) or re.search(
        rules["superseded_draft"]["technical_pattern"], text, re.I
    ):
        return "wordnet-technical"
    return None


def archive_members(source: dict) -> dict[str, bytes]:
    prefix = source["source_id"] + "/"
    with tarfile.open(REPO_ROOT / source["archive_path"]) as archive:
        result = {}
        for member in archive.getmembers():
            if member.isfile():
                if not member.name.startswith(prefix):
                    raise ValueError(f"unexpected archive prefix: {member.name}")
                result[member.name[len(prefix):]] = archive.extractfile(member).read()
        return result


def source_material() -> dict:
    sources = {s["source_id"]: s for s in read_json(SOURCES_LOCK_PATH)["sources"]}
    synsets, senses = {}, {}
    with zipfile.ZipFile(REPO_ROOT / sources["oewn-2025"]["archive_path"]) as archive:
        for name in sorted(archive.namelist()):
            data = json.loads(archive.read(name))
            if name.startswith("entries-"):
                for lemma, by_pos in data.items():
                    for pos, entry in by_pos.items():
                        for sense in entry.get("sense", []):
                            if sense["id"] in senses:
                                raise ValueError(f"duplicate source sense: {sense['id']}")
                            senses[sense["id"]] = {
                                "archive_member": name, "lemma": lemma, "entry_pos": pos,
                                "sense": sense,
                                "entry_metadata": {k: v for k, v in entry.items() if k != "sense"},
                            }
            elif name != "frames.json":
                for sid, synset in data.items():
                    if sid in synsets:
                        raise ValueError(f"duplicate source synset: {sid}")
                    synsets[sid] = {"archive_member": name, "synset": synset}
    members = {sid: archive_members(source) for sid, source in sources.items() if sid != "oewn-2025"}
    entities = {}
    wd = members["wikidata-sel1"]
    for request in json.loads(wd["requests.json"])["requests"]:
        for qid, entity in json.loads(wd[request["file"]]).get("entities", {}).items():
            if "missing" not in entity:
                if qid in entities and entities[qid]["entity"] != entity:
                    raise ValueError(f"conflicting source entity: {qid}")
                entities[qid] = {"archive_member": request["file"], "entity": entity}
    return {"synsets": synsets, "senses": senses, "members": members, "entities": entities}


def original_context(concept: dict, lexicalizations: list[dict], material: dict) -> dict:
    sid, record, kind = concept["source_id"], concept["source_record"], concept["concept_kind"]
    if kind == "wordnet_synset":
        synset = material["synsets"][record]
        originals = []
        for lex in lexicalizations:
            sense = material["senses"][lex["source_record"]]
            if sense["sense"]["synset"] != record or sense["lemma"] != lex["surface"]:
                raise ValueError(f"source sense mismatch: {lex['lexicalization_id']}")
            originals.append({"lexicalization_id": lex["lexicalization_id"], **sense})
        return {**synset, "member_senses": originals}
    if kind in ("github_topic", "acronym_sense"):
        return {"archive_member": record, "text": material["members"][sid][record].decode("utf-8")}
    if kind == "wikidata_item":
        raw = material["entities"][concept["attributes"]["entity_id"]]
        if raw["entity"]["lastrevid"] != concept["attributes"]["lastrevid"]:
            raise ValueError(f"source revision mismatch: {concept['concept_id']}")
        return raw
    if kind == "abbreviation_sense":
        match = re.fullmatch(r"data/abbrs/\.json\[(\d+)\]\.abbrs\[(\d+)\]", record)
        if not match:
            raise ValueError(f"unregistered abbreviation locator: {record}")
        entry = json.loads(material["members"][sid]["data/abbrs/.json"])[int(match[1])]
        return {"archive_member": "data/abbrs/.json", "source_record": record,
                "word": entry["word"], "abbreviation": entry["abbrs"][int(match[2])]}
    raise ValueError(f"unregistered candidate kind: {kind}")


def write_records(path: Path, records) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(canonical_json(record) + "\n")


def generate(directory: Path) -> dict:
    inputs = input_inventory()
    original = inputs["dataset_manifest"]
    rules = read_json(RULES)
    sources = list(checked_records("sources", original))
    concepts = {r["concept_id"]: r for r in checked_records("concepts", original)}
    lexicalizations = collections.defaultdict(list)
    expression_ids = set()
    for lex in checked_records("lexicalizations", original):
        if lex["concept_id"] not in concepts:
            raise ValueError(f"dangling lexicalization: {lex['lexicalization_id']}")
        lexicalizations[lex["concept_id"]].append(lex)
        expression_ids.add(lex["expression_id"])
    all_expression_ids = {e["expression_id"] for e in checked_records("expressions", original)}
    if not expression_ids.issubset(all_expression_ids):
        raise ValueError("dangling expression ID")
    packets = {name: [] for name in rules["packet_priority"]}
    by_id = {}
    rejects = []
    transitions = collections.Counter()
    matched_rules = collections.Counter()
    technical_overlap = collections.Counter()
    kind_counts = collections.Counter()
    old_counts = collections.Counter()
    for cid, concept in concepts.items():
        lex = lexicalizations[cid]
        kind_counts[concept["concept_kind"]] += 1
        name, reasons = selection(concept, lex, rules)
        old = old_selection(concept, lex, rules)
        old_counts[old or "default-reject"] += 1
        transitions[f"{old or 'default-reject'} -> {name or 'default-reject'}"] += 1
        if concept["concept_kind"] == "wordnet_synset":
            technical_overlap.update(technical_reasons(concept, lex, rules))
        if name is None:
            rejects.append({"concept_id": cid, "source_id": concept["source_id"],
                            "lexicalization_ids": [l["lexicalization_id"] for l in lex],
                            "reason": reasons[0], "review_status": "not_semantically_reviewed"})
        else:
            matched_rules.update(reasons)
            row = {"schema_version": "development-curation-packet/1", "packet_name": name,
                   "concept": concept, "lexicalizations": lex, "selection_reasons": reasons,
                   "source_input": None, "original_relations": []}
            packets[name].append(row)
            by_id[cid] = row
    if any(old_counts[k] != v for k, v in rules["superseded_draft"]["counts"].items() if k != "candidate_total"):
        raise ValueError("superseded draft counts not reproduced")
    relation_counts = collections.Counter()
    candidate_relation_counts = collections.Counter()
    for relation in checked_records("relations", original):
        relation_counts[relation["relation_kind"]] += 1
        for cid in {relation["from_concept_id"], relation["to_concept_id"]}:
            if cid not in concepts:
                raise ValueError(f"dangling relation concept: {cid}")
            if cid in by_id:
                by_id[cid]["original_relations"].append(relation)
                candidate_relation_counts[relation["relation_kind"]] += 1
    material = source_material()
    counts, hashes, sizes, batch_plan = {}, {}, {}, []
    for name, rows in packets.items():
        for row in rows:
            row["source_input"] = original_context(row["concept"], row["lexicalizations"], material)
            row["input_sha256"] = sha256_bytes(canonical_json(row).encode())
        path = directory / f"{name}.jsonl"
        write_records(path, rows)
        counts[name] = {"records": len(rows), "lexicalizations": sum(len(r["lexicalizations"]) for r in rows),
                        "original_relation_observations": sum(len(r["original_relations"]) for r in rows)}
        hashes[name] = sha256_file(path)
        sizes[name] = path.stat().st_size
        for start in range(0, len(rows), 150):
            end = min(start + 150, len(rows))
            batch = rows[start:end]
            batch_plan.append({"batch_id": f"{name}:{start+1:04d}-{end:04d}", "packet_name": name,
                               "start_line": start + 1, "record_count": len(batch),
                               "lexicalization_count": sum(len(r["lexicalizations"]) for r in batch),
                               "bytes": sum(len((canonical_json(r) + '\n').encode()) for r in batch)})
    reject_path = directory / "default-reject.jsonl"
    write_records(reject_path, rejects)
    reject_reasons = collections.Counter(r["reason"] for r in rejects)
    manifest = {"schema_version": "development-curation-manifest/1", "input_inventory": inputs,
                "source_records": sources, "rules": rules,
                "denominators": {"source_concepts": len(concepts), "source_lexicalizations": sum(map(len, lexicalizations.values())),
                                 "source_concept_kinds": dict(kind_counts),
                                 "candidate_concepts": len(by_id),
                                 "candidate_lexicalizations": sum(x["lexicalizations"] for x in counts.values()),
                                 "default_reject_concepts": len(rejects),
                                 "default_reject_lexicalizations": sum(len(r["lexicalization_ids"]) for r in rejects),
                                 "directly_reviewed_concepts": 0},
                "packet_counts": counts, "packet_sha256": hashes, "packet_bytes": sizes,
                "default_reject": {"sha256": sha256_file(reject_path), "bytes": reject_path.stat().st_size,
                                   "reasons": dict(reject_reasons)},
                "source_relation_counts": dict(relation_counts),
                "candidate_relation_observation_counts": dict(candidate_relation_counts),
                "selection_rule_counts_priority_deduplicated": dict(matched_rules),
                "technical_rule_counts_before_operations_priority": dict(technical_overlap),
                "superseded_draft_counts": dict(old_counts), "draft_to_approved_transitions": dict(transitions),
                "batch_plan": batch_plan,
                "limitations": ["Automatic candidate selection only; zero agent semantic decisions.",
                                "Default-rejected WordNet senses were not directly reviewed; vocabulary coverage is not established.",
                                "Relations crossing candidate concepts may occur in both packets as original observations, not new edges.",
                                "No cases/evaluation labels, network, LLM at lookup, repository statistics, or dataset build used."]}
    write_json(directory / "manifest.json", manifest)
    return manifest


def verify_packets(directory: Path, check_inputs: bool = True) -> dict:
    manifest = read_json(directory / "manifest.json")
    if check_inputs and manifest["input_inventory"] != input_inventory():
        raise ValueError("packet inputs changed")
    rules = manifest["rules"]
    known_sources = {s["source_id"] for s in manifest["source_records"]}
    seen, seen_lex = set(), set()
    packet_total = 0
    for name in rules["packet_priority"]:
        path = directory / f"{name}.jsonl"
        if sha256_file(path) != manifest["packet_sha256"][name] or path.stat().st_size != manifest["packet_bytes"][name]:
            raise ValueError(f"packet hash/size mismatch: {name}")
        count = lex_count = relation_count = 0
        previous = None
        for row in jsonl(path):
            payload = {k: v for k, v in row.items() if k != "input_sha256"}
            if sha256_bytes(canonical_json(payload).encode()) != row["input_sha256"]:
                raise ValueError("record input hash mismatch")
            cid = row["concept"]["concept_id"]
            if cid in seen or (previous is not None and cid <= previous):
                raise ValueError(f"duplicate or unsorted concept: {cid}")
            previous = cid
            seen.add(cid)
            if row["concept"]["source_id"] not in known_sources:
                raise ValueError(f"unregistered source: {cid}")
            if row["packet_name"] != name or selection(row["concept"], row["lexicalizations"], rules) != (name, row["selection_reasons"]):
                raise ValueError(f"packet selection mismatch: {cid}")
            for lex in row["lexicalizations"]:
                lid = lex["lexicalization_id"]
                if lid in seen_lex or lex["concept_id"] != cid:
                    raise ValueError(f"duplicate or misassigned lexicalization: {lid}")
                seen_lex.add(lid)
            relations = row["original_relations"]
            if len({r["relation_id"] for r in relations}) != len(relations):
                raise ValueError(f"duplicate original relation: {cid}")
            if any(cid not in (r["from_concept_id"], r["to_concept_id"]) for r in relations):
                raise ValueError(f"misassigned original relation: {cid}")
            count += 1
            lex_count += len(row["lexicalizations"])
            relation_count += len(relations)
        if {"records": count, "lexicalizations": lex_count, "original_relation_observations": relation_count} != manifest["packet_counts"][name]:
            raise ValueError(f"packet count mismatch: {name}")
        packet_total += count
    rejects = directory / "default-reject.jsonl"
    if sha256_file(rejects) != manifest["default_reject"]["sha256"]:
        raise ValueError("default-reject hash mismatch")
    reject_count = 0
    reasons = collections.Counter()
    for row in jsonl(rejects):
        cid = row["concept_id"]
        if cid in seen or row["reason"] not in rules["default_reject_reasons"]:
            raise ValueError(f"duplicate or unregistered default reject: {cid}")
        seen.add(cid)
        reasons[row["reason"]] += 1
        reject_count += 1
        for lid in row["lexicalization_ids"]:
            if lid in seen_lex:
                raise ValueError(f"duplicate default lexicalization: {lid}")
            seen_lex.add(lid)
    original = manifest["input_inventory"]["dataset_manifest"]
    original_ids = {c["concept_id"] for c in checked_records("concepts", original)}
    original_lex = {l["lexicalization_id"] for l in checked_records("lexicalizations", original)}
    if seen != original_ids or seen_lex != original_lex:
        raise ValueError("missing or unregistered concept/lexicalization IDs in partition")
    if (packet_total, reject_count, dict(reasons)) != (
        manifest["denominators"]["candidate_concepts"], manifest["denominators"]["default_reject_concepts"],
        manifest["default_reject"]["reasons"]
    ):
        raise ValueError("partition denominator mismatch")
    return {"candidate_records": packet_total, "default_reject_records": reject_count,
            "partition_concepts": len(seen), "partition_lexicalizations": len(seen_lex),
            "packet_sha256": manifest["packet_sha256"]}


def prepare() -> dict:
    CURATION.mkdir(exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".packets-", dir=CURATION))
    try:
        generate(temporary)
        evidence = verify_packets(temporary)
        if PACKETS.exists():
            old_files = {p.name: sha256_file(p) for p in PACKETS.iterdir() if p.is_file()}
            new_files = {p.name: sha256_file(p) for p in temporary.iterdir() if p.is_file()}
            if old_files != new_files or len(old_files) != len(list(PACKETS.iterdir())):
                raise ValueError("existing packets differ; refusing to overwrite")
            evidence["publication"] = "identical_existing_packets_reused"
        else:
            temporary.rename(PACKETS)
            evidence["publication"] = "validated_temporary_directory_renamed"
        return evidence
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)


def validate_decision(row: dict, packet: dict) -> None:
    fields = {"schema_version", "packet_name", "concept_id", "input_sha256", "decision",
              "keep_lexicalization_ids", "development_rationale", "lexicalization_rationales", "review_basis"}
    if set(row) != fields or row["schema_version"] != "development-curation-decision/1":
        raise ValueError("decision fields/schema mismatch")
    if (row["concept_id"], row["packet_name"], row["input_sha256"]) != (
        packet["concept"]["concept_id"], packet["packet_name"], packet["input_sha256"]
    ):
        raise ValueError("decision input identity/hash mismatch")
    if row["decision"] not in ("include", "exclude", "uncertain"):
        raise ValueError("unregistered decision")
    if row["review_basis"] != "direct_source_sense_and_individual_surface":
        raise ValueError("direct review declaration required")
    if not isinstance(row["development_rationale"], str) or not row["development_rationale"].strip():
        raise ValueError("development rationale required")
    allowed = {lex["lexicalization_id"] for lex in packet["lexicalizations"]}
    keep = row["keep_lexicalization_ids"]
    if not isinstance(keep, list) or any(not isinstance(x, str) for x in keep) or len(set(keep)) != len(keep) or not set(keep).issubset(allowed):
        raise ValueError("duplicate/unregistered retained lexicalization ID")
    if (row["decision"] == "include") != bool(keep):
        raise ValueError("include requires retained IDs; exclude/uncertain must retain none")
    rationales = row["lexicalization_rationales"]
    if not isinstance(rationales, dict) or set(rationales) != allowed or any(
        not isinstance(v, str) or not v.strip() for v in rationales.values()
    ):
        raise ValueError("exactly one nonempty rationale per original lexicalization required")


def validate_decisions(path: Path, batch_id: str | None) -> dict:
    verify_packets(PACKETS)
    manifest = read_json(PACKETS / "manifest.json")
    expected = {}
    batches = [b for b in manifest["batch_plan"] if b["batch_id"] == batch_id] if batch_id else manifest["batch_plan"]
    if not batches:
        raise ValueError(f"unregistered batch ID: {batch_id}")
    for batch in batches:
        rows = list(jsonl(PACKETS / f"{batch['packet_name']}.jsonl"))
        start = batch["start_line"] - 1
        for row in rows[start:start + batch["record_count"]]:
            expected[row["concept"]["concept_id"]] = row
    seen = set()
    decisions = collections.Counter()
    for row in jsonl(path):
        cid = row.get("concept_id")
        if cid in seen or cid not in expected:
            raise ValueError(f"duplicate/unregistered decision concept: {cid}")
        validate_decision(row, expected[cid])
        seen.add(cid)
        decisions[row["decision"]] += 1
    if seen != set(expected):
        raise ValueError(f"missing decisions: {len(set(expected) - seen)}")
    return {"scope": batch_id or "all_candidates", "decision_records": len(seen),
            "decision_counts": dict(decisions), "decisions_sha256": sha256_file(path),
            "semantic_correctness": "requires independent human/agent review; not machine established"}


def verify_preservation() -> dict:
    baseline = read_json(CURATION / "preservation-baseline.json")
    changed = [name for name, digest in baseline["files_sha256"].items()
               if not (REPO_ROOT / name).is_file() or sha256_file(REPO_ROOT / name) != digest]
    staged = subprocess.check_output(["git", "diff", "--cached", "--name-only"], cwd=REPO_ROOT, text=True).splitlines()
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, text=True).strip()
    if changed or staged or head != baseline["head"]:
        raise ValueError(f"preservation failure: changed={changed}, staged={staged}, head={head}")
    return {"unchanged_preexisting_files": len(baseline["files_sha256"]), "no_staged_files": True, "head": head}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "verify", "verify-preservation"):
        commands.add_parser(name)
    decisions = commands.add_parser("validate-decisions")
    decisions.add_argument("path", type=Path)
    decisions.add_argument("--batch-id")
    args = parser.parse_args()
    try:
        if args.command == "prepare":
            result = prepare()
        elif args.command == "verify":
            result = verify_packets(PACKETS)
        elif args.command == "verify-preservation":
            result = verify_preservation()
        else:
            result = validate_decisions(args.path, args.batch_id)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (ValueError, KeyError, TypeError) as error:
        raise SystemExit(f"ERROR: {error}") from error


if __name__ == "__main__":
    main()
