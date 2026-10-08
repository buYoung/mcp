"""Explicit read-only classification batches; offline writer validation, never dataset build."""
from __future__ import annotations

import argparse
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

import prepare_development_curation as preparation
import project_development_curation as projection
from rtpoc_common import canonical_json, sha256_bytes, sha256_file

ROOT = preparation.CURATION / "readonly-batches-v2"
PLAN = ROOT / "manifest.json"
CONTRACT = preparation.EXPERIMENT_DIR / "contracts" / "development-curation-readonly-batches.md"
PREFIXES = {
    "wordnet-operations": ("operations", 236, "c381de85fb646f46c42c6b60fd8d2990156acc43078e0f4ea7ad3bb3ca825344"),
    "wordnet-technical": ("technical", 678, "44fa17b84e7c42916474f7534f3754273787bae552c9b9c6661920ef5e1dd2c4"),
    "catalogs": ("catalogs", 261, "349ac036bc4b8f4005de3afd3a55e6302968fce828c5894314d6c2c6c09394c7"),
    "abbreviations": ("abbreviations", 664, "490ba0e6a7e64d2a801b9161f80d97a22d9d43113f0fa06a130730a79274dec6"),
}
TARGETS = {"pages": 6, "concepts": 48, "lexicalizations": 160}
VERSION = "development-curation-readonly-batches/1"
NEXT_PAGES = {"wordnet-operations": "wordnet-operations:000039", "wordnet-technical": "wordnet-technical:000098",
              "catalogs": "catalogs:000046", "abbreviations": None}


def object_schema(properties: dict) -> dict:
    return {"type": "object", "properties": properties, "required": list(properties), "additionalProperties": False}


def array_schema(items: dict) -> dict:
    return {"type": "array", "items": items}


STRING = {"type": "string"}
RESULT_SCHEMA = object_schema({
    "status": {"type": "string", "enum": ["complete", "partial", "blocked"]},
    "batch_id": STRING, "assignment_sha256": STRING, "reviewed_page_ids": array_schema(STRING),
    "decisions": array_schema(object_schema({
        "concept_id": STRING, "decision": {"type": "string", "enum": ["include", "exclude", "uncertain"]},
        "development_rationale": STRING,
        "lexicalization_decisions": array_schema(object_schema({
            "lexicalization_id": STRING, "should_keep": {"type": "boolean"}, "rationale": STRING,
        })),
    })),
    "remaining_concept_ids": array_schema(STRING), "blocker": {"type": ["string", "null"]},
})


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)


def json_bytes(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def publish(path: Path, data: bytes) -> None:
    """Only new paths inside the new area; identical reuse, never replacement."""
    require(ROOT.resolve() == ROOT and path.is_absolute() and path.resolve().is_relative_to(ROOT), "write outside new area")
    require(path.resolve() == path, f"refuse symlink path: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        require(path.read_bytes() == data, f"refuse overwrite: {path}")
    else:
        with path.open("xb") as handle:
            handle.write(data)


def validate_v2(row: dict, raw: dict, pm: dict, raw_manifest: dict) -> None:
    cid = raw["concept"]["concept_id"]
    require(set(row) == projection.DECISION_FIELDS and row["schema_version"] == "development-curation-decision/2", "decision/2 exact 14 fields required")
    metadata = pm["concept_projections"][cid]
    require((row["packet_sha256"], row["projection_version"], row["projection_sha256"], row["projection_manifest_sha256"], row["reviewed_page_ids"]) == (
        raw_manifest["packet_sha256"][raw["packet_name"]], projection.VERSION, metadata["projection_sha256"],
        sha256_file(projection.ROOT / "manifest.json"), metadata["page_ids"]), f"decision/2 binding/pages mismatch: {cid}")
    legacy = {k: v for k, v in row.items() if k not in {
        "packet_sha256", "projection_version", "projection_sha256", "projection_manifest_sha256", "reviewed_page_ids",
    }}
    legacy["schema_version"] = "development-curation-decision/1"
    preparation.validate_decision(legacy, raw)


def inputs(prefix_root: Path | None = None):
    if prefix_root is None:
        prefix_root = Path(load(PLAN)["prefix_root"])
    require(prefix_root.is_absolute() and prefix_root.resolve() == prefix_root, "prefix_root must be caller-provided absolute path, not a symlink")
    proof = projection.verify()
    preservation = preparation.verify_preservation()
    pm = load(projection.ROOT / "manifest.json")
    rm = load(preparation.PACKETS / "manifest.json")
    records, prefix_rows, prefix_inventory, prefixes = {}, {}, [], set()
    for name, (folder, count, digest) in PREFIXES.items():
        rows = list(preparation.jsonl(preparation.PACKETS / f"{name}.jsonl"))
        records.update({r["concept"]["concept_id"]: r for r in rows})
        source = prefix_root / folder / "decisions.jsonl"
        require(sha256_file(source) == digest, f"fixed prefix hash mismatch: {name}")
        decisions = list(preparation.jsonl(source))
        require(len(decisions) == count, f"fixed prefix count mismatch: {name}")
        require([r["concept_id"] for r in decisions] == [r["concept"]["concept_id"] for r in rows[:count]], f"prefix is not exact initial packet range: {name}")
        for decision, raw in zip(decisions, rows):
            validate_v2(decision, raw, pm, rm)
            require(decision["concept_id"] not in prefixes, "duplicate prefix")
            prefixes.add(decision["concept_id"])
        prefix_rows[name] = decisions
        next_page = pm["concept_projections"][rows[count]["concept"]["concept_id"]]["page_ids"][0] if count < len(rows) else None
        require(next_page == NEXT_PAGES[name], f"prefix resume page differs: {name}: {next_page}")
        prefix_inventory.append({"packet_name": name, "source_path": str(source), "next_page_id": next_page,
                                 "snapshot_path": str(ROOT / "prefix" / f"{name}.jsonl"), "sha256": digest,
                                 "bytes": source.stat().st_size, "concept_count": count,
                                 "lexicalization_count": sum(len(r["lexicalization_rationales"]) for r in decisions),
                                 "direct_read_evidence": "not established by saved decisions"})
    require((len(records), len(prefixes), len(records) - len(prefixes), sum(len(r["lexicalizations"]) for r in records.values())) == (7147, 1839, 5308, 17830), "fixed denominators differ")
    return pm, rm, records, prefixes, prefix_rows, prefix_inventory, proof, preservation


def page_groups(pm: dict, name: str, pending: set[str]) -> list[list[dict]]:
    """Connected page components keep all oversized fragments and neighbours together."""
    pages = [p for p in pm["pages"] if p["page_id"].startswith(name + ":")]
    parent = {p["page_id"]: p["page_id"] for p in pages}

    def root(key):
        while parent[key] != key:
            key = parent[key]
        return key

    for meta in pm["concept_projections"].values():
        if meta["packet_name"] == name:
            ids = meta["page_ids"]
            for pid in ids[1:]:
                parent[root(pid)] = root(ids[0])
    groups = collections.OrderedDict()
    for page in pages:
        groups.setdefault(root(page["page_id"]), []).append(page)
    return [g for g in groups.values() if any(set(meta["page_ids"]).intersection(p["page_id"] for p in g)
            for cid, meta in pm["concept_projections"].items() if cid in pending)]


def planned_assignments(pm: dict, rm: dict, records: dict, prefixes: set[str]) -> list[dict]:
    pending = set(records) - prefixes
    sources = {s["source_id"]: s for s in rm["source_records"]}
    assignments = []
    for name in PREFIXES:
        groups = page_groups(pm, name, pending)
        page_to_cids = collections.defaultdict(set)
        for cid, meta in pm["concept_projections"].items():
            for pid in meta["page_ids"]:
                page_to_cids[pid].add(cid)

        def cids_for(pages):
            return set().union(*(page_to_cids[p["page_id"]] for p in pages)) & pending

        def fits(pages):
            cids = cids_for(pages)
            return len(pages) <= TARGETS["pages"] and len(cids) <= TARGETS["concepts"] and sum(len(records[c]["lexicalizations"]) for c in cids) <= TARGETS["lexicalizations"]

        batches, current = [], []
        for group in groups:
            if current and not fits(current + group):
                batches.append(current)
                current = []
            if not fits(group):
                batches.append(group)
            else:
                current.extend(group)
        if current:
            batches.append(current)
        for number, pages in enumerate(batches, 1):
            batch_id = f"{name}-readonly-{number:04d}"
            cids = sorted(cids_for(pages))
            concepts = []
            for cid in cids:
                raw, meta = records[cid], pm["concept_projections"][cid]
                lids = [lex["lexicalization_id"] for lex in raw["lexicalizations"]]
                concepts.append({"concept_id": cid, "packet_line": meta["packet_line"], "input_sha256": meta["input_sha256"],
                                 "projection_sha256": meta["projection_sha256"], "page_ids": meta["page_ids"],
                                 "source_id": raw["concept"]["source_id"], "source_record": raw["concept"]["source_record"],
                                 "lexicalization_ids": lids, "lexicalization_id_range": [min(lids), max(lids)] if lids else []})
            lids = sorted(lid for c in concepts for lid in c["lexicalization_ids"])
            visible = set().union(*(page_to_cids[p["page_id"]] for p in pages))
            assignments.append({"schema_version": VERSION, "batch_id": batch_id, "packet_name": name,
                "packet_path": str(preparation.PACKETS / f"{name}.jsonl"), "packet_sha256": rm["packet_sha256"][name],
                "raw_manifest_path": str(preparation.PACKETS / "manifest.json"), "raw_manifest_sha256": sha256_file(preparation.PACKETS / "manifest.json"),
                "projection_version": projection.VERSION, "projection_manifest_path": str(projection.ROOT / "manifest.json"),
                "projection_manifest_sha256": sha256_file(projection.ROOT / "manifest.json"),
                "source_identity": [sources[sid] for sid in sorted({records[c]["concept"]["source_id"] for c in cids})],
                "contract_path": str(CONTRACT), "contract_sha256": sha256_file(CONTRACT),
                "result_schema_path": str(ROOT / "result-schema.json"), "result_schema_sha256": sha256_bytes(json_bytes(RESULT_SCHEMA)),
                "output_path": str(ROOT / "results" / f"{batch_id}.json"),
                "source_pages": [{"path": str(projection.ROOT / p["file"]), "page_id": p["page_id"], "sha256": p["sha256"], "bytes": p["bytes"],
                                  "total_lines": len((projection.ROOT / p["file"]).read_text(encoding="utf-8").splitlines())} for p in pages],
                "concept_id_range": [cids[0], cids[-1]], "lexicalization_id_range": [lids[0], lids[-1]] if lids else [],
                "concept_count": len(cids), "lexicalization_count": len(lids), "concepts": concepts,
                "context_only_prefix_concept_ids": sorted(visible - set(cids)),
                "targets": TARGETS, "oversized_connected_group": not fits(pages),
                "instructions": "Read assignment, contract, result schema, and every source page with read only. All fragments required. Decide only assigned complete concepts and every original lexicalization. Return schema JSON via runtime structured_output; no shell/renderer/file changes. Page declarations alone are not read evidence."})
    assigned = [c["concept_id"] for a in assignments for c in a["concepts"]]
    lex = [lid for a in assignments for c in a["concepts"] for lid in c["lexicalization_ids"]]
    require(len(assigned) == len(set(assigned)) and set(assigned) == pending, "pending concept gap/overlap")
    expected_lex = {l["lexicalization_id"] for cid in pending for l in records[cid]["lexicalizations"]}
    require(len(lex) == len(set(lex)) and set(lex) == expected_lex, "pending lexicalization gap/overlap")
    assigned_pages = [p["page_id"] for a in assignments for p in a["source_pages"]]
    require(len(assigned_pages) == len(set(assigned_pages)), "page shared across batches")
    for assignment in assignments:
        pages = {p["page_id"] for p in assignment["source_pages"]}
        require(set(assignment["context_only_prefix_concept_ids"]).issubset(prefixes), "unassigned pending page neighbour")
        for concept in assignment["concepts"]:
            require(set(concept["page_ids"]).issubset(pages), "incomplete concept/oversized fragments split across batches")
    return assignments


def make_manifest(pm, rm, records, prefixes, prefix_inventory, assignments) -> dict:
    return {"schema_version": VERSION, "helper_path": str(Path(__file__).resolve()), "helper_sha256": sha256_file(Path(__file__)),
            "contract_path": str(CONTRACT), "contract_sha256": sha256_file(CONTRACT),
            "raw_manifest_path": str(preparation.PACKETS / "manifest.json"), "raw_manifest_sha256": sha256_file(preparation.PACKETS / "manifest.json"),
            "projection_manifest_path": str(projection.ROOT / "manifest.json"), "projection_manifest_sha256": sha256_file(projection.ROOT / "manifest.json"),
            "prefix_root": str(Path(prefix_inventory[0]["source_path"]).parent.parent),
            "prefixes": prefix_inventory, "prefix_concepts": len(prefixes), "pending_concepts": len(records) - len(prefixes),
            "candidate_concepts": len(records), "candidate_lexicalizations": sum(len(r["lexicalizations"]) for r in records.values()),
            "prefix_lexicalizations": sum(p["lexicalization_count"] for p in prefix_inventory),
            "pending_lexicalizations": sum(a["lexicalization_count"] for a in assignments),
            "targets": TARGETS, "result_schema_path": str(ROOT / "result-schema.json"), "result_schema_sha256": sha256_bytes(json_bytes(RESULT_SCHEMA)),
            "results_directory": str(ROOT / "results"), "evidence_directory": str(ROOT / "evidence"),
            "runs_binding_path": str(ROOT / "runs-binding.json"), "canonical_directory": str(ROOT / "canonical"),
            "verified_plan_path": str(ROOT / "verified-plan.json"), "execution_handoff_path": str(ROOT / "execution-handoff.json"),
            "shards": [{"id": a["batch_id"], "packet_name": a["packet_name"], "assignment_path": str(ROOT / "assignments" / f"{a['batch_id']}.json"),
                        "assignment_sha256": sha256_bytes(json_bytes(a)), "output_path": a["output_path"],
                        "concept_count": a["concept_count"], "lexicalization_count": a["lexicalization_count"]} for a in assignments],
            "limitations": ["Existing prefix decisions structurally validated; saved bytes are not direct-read evidence.",
                            "No new semantic classification, canonical merge, dataset build or activation performed by planning.",
                            "Metadata proves delivery of fixed text, not semantic correctness; independent review required."]}


def plan(create: bool, prefix_root: Path | None = None) -> dict:
    pm, rm, records, prefixes, _, inventory, proof, preservation = inputs(prefix_root)
    assignments = planned_assignments(pm, rm, records, prefixes)
    manifest = make_manifest(pm, rm, records, prefixes, inventory, assignments)
    artifacts = [(ROOT / "result-schema.json", json_bytes(RESULT_SCHEMA)), (PLAN, json_bytes(manifest))]
    for prefix in inventory:
        data = Path(prefix["source_path"]).read_bytes()
        require(sha256_bytes(data) == prefix["sha256"], "prefix changed before snapshot")
        artifacts.append((Path(prefix["snapshot_path"]), data))
    artifacts += [(Path(s["assignment_path"]), json_bytes(a)) for s, a in zip(manifest["shards"], assignments)]
    if create:
        require(not ROOT.is_symlink(), "new root is symlink")
        for folder in ("results", "evidence", "canonical"):
            path = ROOT / folder
            require(not path.is_symlink(), "new directory is symlink")
            path.mkdir(parents=True, exist_ok=True)
    for path, data in artifacts:
        if create:
            publish(path, data)
        else:
            require(path.resolve() == path and path.read_bytes() == data, f"plan artifact mismatch/symlink: {path}")
    evidence = {"status": "ready", "manifest_path": str(PLAN), "contract_path": str(CONTRACT), "helper_path": str(Path(__file__).resolve()),
                "prefix_concepts": manifest["prefix_concepts"], "pending_concepts": manifest["pending_concepts"],
                "candidate_concepts": manifest["candidate_concepts"], "candidate_lexicalizations": manifest["candidate_lexicalizations"],
                "prefix_lexicalizations": manifest["prefix_lexicalizations"], "pending_lexicalizations": manifest["pending_lexicalizations"],
                "shards": manifest["shards"], "projection_proof": proof, "preservation": preservation,
                "batch_stats": {name: {"batches": sum(a["packet_name"] == name for a in assignments),
                                      "concepts": sum(a["concept_count"] for a in assignments if a["packet_name"] == name),
                                      "lexicalizations": sum(a["lexicalization_count"] for a in assignments if a["packet_name"] == name)} for name in PREFIXES},
                "oversized_group_batches": [a["batch_id"] for a in assignments if a["oversized_connected_group"]],
                "summary": f"{len(assignments)} independent read-only batches; exact prefix bytes preserved; no new decisions or merge."}
    handoff = {"schema_version": VERSION, "cwd": str(preparation.REPO_ROOT), "manifest_path": str(PLAN),
               "model": "gpt-6.1-sol", "execution": "native background only; parent orchestration/judgment only",
               "classifier_tools": ["read", "structured_output"], "output_binding": "shards[].output_path; result-schema.json as runtime outputSchema",
               "required_reads": "assignment, contract, result schema, every source_pages entry; continue after truncation",
               "writer_commands": ["extract-evidence", "validate-results", "merge"],
               "writer_bindings": "Caller passes official trace_root and unique batch_id/run_id (+official runtime_result_path when needed) in new-area --bindings JSON; writer runs helper",
               "preparation_only": True, "semantic_classification_started": False, "merge_started": False}
    report = {**evidence, "command": ["python3", "-B", str(Path(__file__).resolve()), "plan", "--prefix-root", manifest["prefix_root"]], "cwd": str(preparation.REPO_ROOT)}
    for path, data in ((ROOT / "verified-plan.json", json_bytes(report)), (ROOT / "execution-handoff.json", json_bytes(handoff))):
        if create:
            publish(path, data)
        else:
            require(path.read_bytes() == data, f"plan report/handoff mismatch: {path}")
    return evidence


def check_schema(value, schema, label="result") -> None:
    kind = schema["type"]
    types = kind if isinstance(kind, list) else [kind]
    matches = {"object": type(value) is dict, "array": type(value) is list, "string": type(value) is str,
               "boolean": type(value) is bool, "null": value is None}
    require(any(matches[t] for t in types), f"{label}: wrong type")
    if "enum" in schema:
        require(value in schema["enum"], f"{label}: wrong enum")
    if kind == "object":
        require(set(value) == set(schema["properties"]), f"{label}: missing/extra fields")
        for key, child in schema["properties"].items():
            check_schema(value[key], child, label + "." + key)
    if kind == "array":
        for item in value:
            check_schema(item, schema["items"], label + "[]")


def convert_result(shard: dict, assignment: dict, result: dict, pm: dict, rm: dict, records: dict) -> list[dict]:
    check_schema(result, RESULT_SCHEMA)
    require((result["batch_id"], result["assignment_sha256"]) == (shard["id"], shard["assignment_sha256"]), "result assignment binding mismatch")
    expected = {c["concept_id"] for c in assignment["concepts"]}
    page_ids = [p["page_id"] for p in assignment["source_pages"]]
    reviewed = result["reviewed_page_ids"]
    require(len(reviewed) == len(set(reviewed)) and set(reviewed).issubset(page_ids), "unknown/duplicate reviewed page")
    require(reviewed == [pid for pid in page_ids if pid in reviewed], "reviewed pages out of order")
    remaining = result["remaining_concept_ids"]
    require(len(remaining) == len(set(remaining)) and set(remaining).issubset(expected), "unknown/duplicate remaining concept")
    seen, converted = set(), []
    for decision in result["decisions"]:
        cid = decision["concept_id"]
        require(cid in expected and cid not in seen and cid not in remaining, "unknown/duplicate/remaining decision ID")
        raw, meta = records[cid], pm["concept_projections"][cid]
        require(set(meta["page_ids"]).issubset(reviewed), f"missing declared pages for decision: {cid}")
        lexical = decision["lexicalization_decisions"]
        lids = [x["lexicalization_id"] for x in lexical]
        expected_lids = {x["lexicalization_id"] for x in raw["lexicalizations"]}
        require(len(lids) == len(set(lids)) and set(lids) == expected_lids, f"unknown/duplicate/missing lexicalization: {cid}")
        mapping = {x["lexicalization_id"]: x for x in lexical}
        row = {"schema_version": "development-curation-decision/2", "packet_name": raw["packet_name"], "concept_id": cid,
               "input_sha256": raw["input_sha256"], "decision": decision["decision"],
               "development_rationale": decision["development_rationale"],
               "keep_lexicalization_ids": [x["lexicalization_id"] for x in raw["lexicalizations"] if mapping[x["lexicalization_id"]]["should_keep"]],
               "lexicalization_rationales": {lid: x["rationale"] for lid, x in mapping.items()},
               "review_basis": "direct_source_sense_and_individual_surface", "packet_sha256": rm["packet_sha256"][raw["packet_name"]],
               "projection_version": projection.VERSION, "projection_sha256": meta["projection_sha256"],
               "projection_manifest_sha256": sha256_file(projection.ROOT / "manifest.json"), "reviewed_page_ids": meta["page_ids"]}
        validate_v2(row, raw, pm, rm)
        seen.add(cid)
        converted.append(row)
    require(seen | set(remaining) == expected, "unreported missing concepts")
    if result["status"] == "complete":
        require(not remaining and result["blocker"] is None and reviewed == page_ids, "incomplete result marked complete")
    else:
        require(bool(remaining), "non-complete result must report remaining concepts")
        if result["status"] == "blocked":
            require(isinstance(result["blocker"], str) and bool(result["blocker"].strip()), "blocked result needs reason")
    require(result["blocker"] is None or bool(result["blocker"].strip()), "empty blocker")
    return converted


def confirmed_read(args: dict, result: dict, path: Path) -> dict:
    """Match returned text to fixed lines; request args alone never prove delivery."""
    require(type(result) is dict, "read result object missing")
    content = result.get("content")
    require(type(content) is list and len(content) == 1 and content[0].get("type") == "text", "non-text/multipart read unsupported")
    text = content[0].get("text")
    require(type(text) is str, "read text missing")
    lines = path.read_text(encoding="utf-8").split("\n")
    offset = args.get("offset")
    limit = args.get("limit")
    require(offset is None or type(offset) is int and offset >= 1, "invalid read offset")
    require(limit is None or type(limit) is int and limit >= 1, "invalid read limit")
    start = (offset or 1) - 1
    require(start < len(lines), "read offset beyond fixed source")
    end = min(start + limit, len(lines)) if limit is not None else len(lines)
    trunc = result.get("details", {}).get("truncation") if isinstance(result.get("details"), dict) else None
    truncated = False
    if trunc and trunc.get("truncated"):
        require(not trunc.get("firstLineExceedsLimit") and type(trunc.get("outputLines")) is int and trunc["outputLines"] > 0, "incomplete read truncation evidence")
        end = min(end, start + trunc["outputLines"])
        truncated = True
    continuation = re.search(r"\n\n\[Showing lines (\d+)-(\d+) of (\d+)(?: \((?:50|50\.0)KB limit\))?\. Use offset=(\d+) to continue\.\]$", text)
    if continuation:
        lo, hi, total, next_offset = map(int, continuation.groups())
        require(lo == start + 1 and total == len(lines) and start < hi <= end and next_offset == hi + 1,
                "read continuation range inconsistent with fixed input/request")
        if truncated:
            require(hi == end, "truncation metadata/returned range mismatch")
        end = hi
        truncated = True
    body = "\n".join(lines[start:end])
    suffixes = [""]
    if end < len(lines):
        suffixes += [f"\n\n[{len(lines)-end} more lines in file. Use offset={end+1} to continue.]",
                     f"\n\n[{len(lines)-end} more lines in file. Use offset={end+1} to continue.]\n",
                     f"\n\n[Showing lines {start+1}-{end} of {len(lines)}. Use offset={end+1} to continue.]",
                     f"\n\n[Showing lines {start+1}-{end} of {len(lines)} (50.0KB limit). Use offset={end+1} to continue.]",
                     f"\n\n[Showing lines {start+1}-{end} of {len(lines)} (50KB limit). Use offset={end+1} to continue.]"]
    require(any(text == body + suffix for suffix in suffixes), "returned read text does not equal fixed source range / unsupported truncation wire")
    # A trailing empty split element is not a semantic line requiring an extra call.
    semantic_total = len(lines) - (lines[-1] == "")
    return {"path": str(path), "fixed_file_sha256": sha256_file(path), "total_lines": semantic_total,
            "returned_line_range": [start + 1, min(end, semantic_total)], "truncated": truncated, "is_error": False,
            "returned_text_sha256": sha256_bytes(text.encode("utf-8")),
            "binding_basis": "returned text compared to fixed source lines; file SHA computed, not supplied by trace"}


def extract_run(shard: dict, assignment: dict, binding: dict, trace_root: Path) -> dict:
    run_id = binding["run_id"]
    require(type(run_id) is str and re.fullmatch(r"[0-9a-f-]{36}", run_id) is not None, "invalid run ID")
    directory = trace_root / run_id
    status_path = directory / "status.json"
    events_path = directory / "events.jsonl"
    require(all(p.resolve() == p for p in (directory, status_path, events_path)), "runtime metadata path must not be a symlink")
    status = load(status_path)
    require(status.get("runId") == run_id and status.get("state") == "complete" and status.get("mode") == "single", "run not complete single native classification")
    require(status.get("cwd") == str(preparation.REPO_ROOT), "run cwd mismatch")
    steps = status.get("steps")
    require(type(steps) is list and len(steps) == 1 and steps[0].get("status") == "complete" and steps[0].get("exitCode") == 0, "run step not complete exit0")
    step = steps[0]
    model = step.get("model")
    require(type(model) is str and re.fullmatch(r"(?:[^/:]+/)?gpt-6\.1-sol(?::(?:minimal|low|medium|high|xhigh))?", model) is not None, "runtime model mismatch")
    result_path = Path(binding.get("runtime_result_path", str(directory / "workflow-result.json")))
    official_paths = {directory / "workflow-result.json", trace_root.parent / "async-subagent-results" / f"{run_id}.json"}
    require(result_path in official_paths and result_path.resolve() == result_path, "runtime result path outside official run locations / symlink")
    runtime_result = load(result_path)
    require(runtime_result.get("id") == run_id and runtime_result.get("state") == "complete" and runtime_result.get("success") is True and runtime_result.get("exitCode") == 0, "official result not complete exit0")
    results = runtime_result.get("results")
    require(type(results) is list and len(results) == 1, "official result single child required")
    child = results[0]
    require(child.get("model") == model and child.get("success") is True and child.get("savedOutputPath") == shard["output_path"], "official model/output binding mismatch")
    require(not any(child.get(k) for k in ("error", "outputPartial", "structuredOutputFailed", "outputSaveError", "stopped", "timedOut", "interrupted")), "official child incomplete/error")
    require(not child.get("externalProcess") and not child.get("externalJob") and not child.get("runner"), "external/non-native classifier unsupported")
    output_path = Path(shard["output_path"])
    require(output_path.resolve() == output_path, "output must not be a symlink")
    output = load(output_path)
    require(child.get("structuredOutput") == output and step.get("structuredOutput") == output, "official structured value differs from bound JSON")
    allowed = {str(Path(shard["assignment_path"])): shard["assignment_sha256"], str(CONTRACT): sha256_file(CONTRACT),
               assignment["result_schema_path"]: assignment["result_schema_sha256"]}
    allowed.update({p["path"]: p["sha256"] for p in assignment["source_pages"]})
    starts, ended, calls = {}, set(), []
    coverage = collections.defaultdict(set)
    output_start_line = None
    # Stream, discard non-tool events immediately. Never return/save reasoning or unrelated bodies.
    with events_path.open(encoding="utf-8") as handle:
        for event_line, line in enumerate(handle, 1):
            if '"tool_execution_start"' not in line and '"tool_execution_end"' not in line:
                continue
            event = json.loads(line)
            kind = event.get("type")
            if kind not in ("tool_execution_start", "tool_execution_end"):
                continue
            require(event.get("subagentRunId") == run_id, "native tool event run binding mismatch")
            call_id, tool = event.get("toolCallId"), event.get("toolName")
            require(type(call_id) is str and type(tool) is str, "native tool call identity missing")
            require(tool in ("read", "structured_output"), "classifier used a non-read/non-output tool")
            if kind == "tool_execution_start":
                require(output_start_line is None, "tool start after structured_output submission")
                require(call_id not in starts, "duplicate tool start")
                args = event.get("args", {})
                if tool == "read":
                    requested = args.get("path")
                    require(type(requested) is str, "read path missing")
                    path = Path(requested)
                    path = path if path.is_absolute() else preparation.REPO_ROOT / path
                    require(str(path) in allowed and path.resolve() == path, "classification read outside assigned inputs")
                    require(sha256_file(path) == allowed[str(path)], "classification read input hash changed")
                    starts[call_id] = (args, path, event_line)
                else:
                    require(set(starts) == ended, "structured_output submitted before prior reads completed")
                    missing = {path: len(set(range(1, len(Path(path).read_text(encoding="utf-8").splitlines()) + 1)) - coverage[path]) for path in allowed}
                    require(not any(missing.values()), f"missing successful read coverage before structured_output: { {p: n for p, n in missing.items() if n} }")
                    require(args == {"value": output}, "structured_output value differs from bound result")
                    starts[call_id] = None
                    output_start_line = event_line
            else:
                require(call_id in starts, "tool end without a preceding registered start")
                require(call_id not in ended, "duplicate tool end")
                ended.add(call_id)
                require(tool == ("structured_output" if starts[call_id] is None else "read"), "tool start/end mismatch")
                require(event.get("isError") is False, "tool execution failed")
                if starts[call_id] is not None:
                    require(tool == "read", "tool start/end mismatch")
                    require(event.get("isError") is False, "read tool failed")
                    args, path, start_line = starts[call_id]
                    call = {"call_id": call_id, "tool_name": "read", "start_event_line": start_line, "end_event_line": event_line,
                            **confirmed_read(args, event.get("result"), path)}
                    calls.append(call)
                    lo, hi = call["returned_line_range"]
                    coverage[call["path"]].update(range(lo, hi + 1))
    require(set(starts) == ended and sum(v is None for v in starts.values()) == 1, "unfinished tool call / exactly one structured_output required")
    return {"schema_version": "development-curation-read-evidence/1", "batch_id": shard["id"], "run_id": run_id,
            "assignment_sha256": shard["assignment_sha256"], "model": model, "output_path": shard["output_path"],
            "output_sha256": sha256_file(Path(shard["output_path"])), "status_path": str(status_path),
            "runtime_result_path": str(result_path), "events_path": str(events_path), "read_calls": calls,
            "structured_output_start_event_line": output_start_line, "coverage_complete_before_output": True,
            "coverage_complete": True, "limitations": "Text delivery only, not reasoning correctness; reviewer must compare official runtime metadata and extractor, not trust ledger alone."}


def results(bindings_path: Path, save_evidence: bool, merge: bool) -> dict:
    verified = plan(False)
    manifest = load(PLAN)
    pm, rm, records, prefixes, prefix_rows, _, _, _ = inputs()
    require(bindings_path.is_absolute() and bindings_path.resolve().is_relative_to(ROOT) and bindings_path.resolve() == bindings_path, "bindings must be a new-area absolute JSON path, not a symlink")
    bindings = load(bindings_path)
    require(set(bindings) == {"trace_root", "runs"} and type(bindings["runs"]) is list, "bindings need trace_root/runs")
    trace_root = Path(bindings["trace_root"])
    require(trace_root.is_absolute() and trace_root.resolve() == trace_root, "trace_root must be caller-provided official absolute path, not a symlink")
    runs = {}
    for binding in bindings["runs"]:
        require(set(binding) in ({"batch_id", "run_id"}, {"batch_id", "run_id", "runtime_result_path"}), "binding fields mismatch")
        require(binding["batch_id"] not in runs, "duplicate batch run binding")
        runs[binding["batch_id"]] = binding
    require(set(runs) == {s["id"] for s in manifest["shards"]}, "missing/unknown batch runs")
    require(len({b["run_id"] for b in runs.values()}) == len(runs), "one run reused across batches")
    converted, ledgers, incomplete = {}, [], []
    for shard in manifest["shards"]:
        assignment = load(Path(shard["assignment_path"]))
        output_path = Path(shard["output_path"])
        require(output_path.resolve() == output_path, "result must not be a symlink")
        output = load(output_path)
        rows = convert_result(shard, assignment, output, pm, rm, records)
        ledger = extract_run(shard, assignment, runs[shard["id"]], trace_root)
        ledgers.append(ledger)
        if save_evidence:
            publish(ROOT / "evidence" / f"{ledger['batch_id']}-{ledger['run_id']}.json", json_bytes(ledger))
        if output["status"] != "complete":
            incomplete.append(shard["id"])
        for row in rows:
            require(row["concept_id"] not in converted and row["concept_id"] not in prefixes, "result overlap/prefix modification")
            converted[row["concept_id"]] = row
    if save_evidence:
        publish(ROOT / "runs-binding.json", json_bytes(bindings))
    require(not incomplete, f"incomplete batches; no canonical merge: {incomplete}")
    require(set(converted) == set(records) - prefixes, "incomplete pending decisions")
    evidence = {"status": "complete", "validated_new_concepts": len(converted), "prefix_concepts": len(prefixes),
                "candidate_concepts": len(records), "read_evidence_runs": len(ledgers), "merged": False,
                "manifest_path": verified["manifest_path"]}
    if merge:
        # New isolated canonical files only. Prefix segments remain exact original bytes.
        paths = []
        for name in PREFIXES:
            snapshot = ROOT / "prefix" / f"{name}.jsonl"
            tail = [converted[cid] for cid in sorted(converted) if converted[cid]["packet_name"] == name]
            data = snapshot.read_bytes() + b"".join((canonical_json(row) + "\n").encode("utf-8") for row in tail)
            path = ROOT / "canonical" / f"{name}.jsonl"
            publish(path, data)
            require(path.read_bytes().startswith(snapshot.read_bytes()), "prefix bytes changed in canonical")
            paths.append(path)
        validations = []
        for name, path in zip(PREFIXES, paths):
            command = [sys.executable, "-B", str(preparation.EXPERIMENT_DIR / "project_development_curation.py"), "validate-decisions", str(path), "--packet", name]
            print(json.dumps({"validation_command": command, "cwd": str(preparation.REPO_ROOT)}, ensure_ascii=False), file=sys.stderr, flush=True)
            completed = subprocess.run(command, cwd=preparation.REPO_ROOT, capture_output=True, text=True)
            validations.append({"command": command, "cwd": str(preparation.REPO_ROOT), "exit_code": completed.returncode,
                                "stdout": completed.stdout, "stderr": completed.stderr})
            if completed.returncode != 0:
                publish(ROOT / "canonical" / "validation-failure.json", json_bytes({"complete": False, "validations": validations}))
                raise ValueError("official validate-decisions failed; canonical files are not complete")
        merged_path = ROOT / "canonical" / "decisions.jsonl"
        publish(merged_path, b"".join(path.read_bytes() for path in paths))
        require(sum(1 for _ in preparation.jsonl(merged_path)) == 7147, "merged denominator mismatch")
        evidence.update({"merged": True, "canonical_path": str(merged_path), "canonical_sha256": sha256_file(merged_path), "official_validations": validations})
        publish(ROOT / "canonical" / "validation.json", json_bytes(evidence))
    return evidence


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("plan")
    command.add_argument("--prefix-root", type=Path, required=True)
    commands.add_parser("verify-plan")
    for name in ("extract-evidence", "validate-results", "merge"):
        command = commands.add_parser(name)
        command.add_argument("--bindings", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command in ("plan", "verify-plan"):
            value = plan(args.command == "plan", args.prefix_root if args.command == "plan" else None)
        else:
            value = results(args.bindings, args.command in ("extract-evidence", "merge"), args.command == "merge")
        print(json.dumps(value, ensure_ascii=False, indent=2))
    except (ValueError, KeyError, TypeError, OSError) as error:
        if args.command in ("extract-evidence", "merge") and ROOT.is_dir():
            failure = {"status": "blocked", "complete": False, "command": args.command, "cwd": str(preparation.REPO_ROOT),
                       "error": str(error), "limitation": "Stopped; prior verified ledgers may exist. No completed merge declared."}
            publish(ROOT / "evidence" / f"failure-{sha256_bytes(json_bytes(failure))[:16]}.json", json_bytes(failure))
        raise SystemExit(f"ERROR: {error}") from error


if __name__ == "__main__":
    main()
