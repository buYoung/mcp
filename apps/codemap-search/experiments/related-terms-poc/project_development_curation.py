"""Byte-bounded, lossless semantic reading views of immutable curation packets."""
from __future__ import annotations

import argparse
import collections
import json
import shutil
import tempfile
from pathlib import Path

import prepare_development_curation as preparation
from rtpoc_common import canonical_json, read_json, sha256_bytes, sha256_file, write_json

VERSION = "development-curation-projection/1"
ROOT = preparation.CURATION / "projection"
CONTRACT = preparation.EXPERIMENT_DIR / "contracts" / "development-curation-projection.md"
PAGE_LIMIT = 24 * 1024
ITEM_LIMIT = 18 * 1024
FRAGMENT_TEXT_LIMIT = 6 * 1024
DECISION_FIELDS = {
    "schema_version", "packet_name", "concept_id", "input_sha256", "decision",
    "keep_lexicalization_ids", "development_rationale", "lexicalization_rationales", "review_basis",
    "packet_sha256", "projection_version", "projection_sha256", "projection_manifest_sha256", "reviewed_page_ids",
}


def encoded(value) -> bytes:
    return (canonical_json(value) + "\n").encode("utf-8")


def projected_record(raw: dict, packet_hash: str, line: int, source: dict) -> dict:
    relations = raw["original_relations"]
    result = {"schema_version": VERSION, "packet_name": raw["packet_name"],
              "packet_sha256": packet_hash, "input_sha256": raw["input_sha256"],
              "concept": raw["concept"], "lexicalizations": raw["lexicalizations"],
              "source": source, "source_input": raw["source_input"],
              "selection_reasons": raw["selection_reasons"],
              "original_relations_summary": {
                  "record_count": len(relations),
                  "relation_kind_counts": dict(collections.Counter(r["relation_kind"] for r in relations)),
                  "records_sha256": sha256_bytes(canonical_json(relations).encode("utf-8")),
                  "raw_locator": {"packet_file": raw["packet_name"] + ".jsonl", "line": line,
                                  "field": "original_relations"},
                  "note": "Original observations only. No new or transitive relations; related_topic/similar/antonym cannot serve.",
              }}
    result["projection_sha256"] = sha256_bytes(canonical_json(result).encode("utf-8"))
    return result


def text_fragments(text: str) -> list[str]:
    fragments, current, size = [], [], 0
    for character in text:
        length = len(character.encode("utf-8"))
        if size + length > FRAGMENT_TEXT_LIMIT:
            fragments.append("".join(current))
            current, size = [], 0
        current.append(character)
        size += length
    if current:
        fragments.append("".join(current))
    return fragments


def record_items(record: dict, line: int) -> list[dict]:
    common = {"concept_id": record["concept"]["concept_id"], "packet_line": line,
              "projection_sha256": record["projection_sha256"]}
    normal = {**common, "oversized": False, "record": record}
    if len(encoded(normal)) <= ITEM_LIMIT:
        return [normal]
    pieces = text_fragments(canonical_json(record))
    return [{**common, "oversized": True, "fragment_index": i + 1, "fragment_count": len(pieces),
             "fragment_utf8_sha256": sha256_bytes(text.encode("utf-8")), "record_fragment": text}
            for i, text in enumerate(pieces)]


def reading_page(name: str, number: int, items: list[dict], metadata: dict, packet_hash: str,
                 final: bool = False) -> dict:
    concepts = sorted({item["concept_id"] for item in items})
    lexicalizations = sorted({lid for cid in concepts for lid in metadata[cid]["lexicalization_ids"]})
    lines = [item["packet_line"] for item in items]
    return {"schema_version": "development-curation-reading-page/1", "projection_version": VERSION,
            "packet_name": name, "packet_sha256": packet_hash, "page_id": f"{name}:{number:06d}",
            "next_page_cursor": None if final else f"{name}:{number+1:06d}",
            "coverage": {"packet_line_range": [min(lines), max(lines)],
                         "concept_id_range": [concepts[0], concepts[-1]], "record_count": len(concepts),
                         "complete_record_count": sum(not item["oversized"] for item in items),
                         "lexicalization_id_range": [lexicalizations[0], lexicalizations[-1]] if lexicalizations else [],
                         "lexicalization_count": len(lexicalizations),
                         "counts_note": "Oversized record counts describe its complete raw identity, not fields visible in one fragment.",
                         "oversized_fragments": [{k: item[k] for k in ("concept_id", "fragment_index", "fragment_count")}
                                                 for item in items if item["oversized"]]},
            "items": items}


def inventory() -> dict:
    return {"raw_manifest_sha256": sha256_file(preparation.PACKETS / "manifest.json"),
            "script_sha256": sha256_file(Path(__file__)), "contract_sha256": sha256_file(CONTRACT),
            "projection_version": VERSION, "page_byte_limit": PAGE_LIMIT}


def generate(directory: Path) -> None:
    preparation.verify_packets(preparation.PACKETS)
    raw_manifest = read_json(preparation.PACKETS / "manifest.json")
    sources = {s["source_id"]: s for s in raw_manifest["source_records"]}
    record_metadata, packet_stats, page_inventory = {}, {}, []
    pages_dir = directory / "pages"
    pages_dir.mkdir()
    for name in raw_manifest["rules"]["packet_priority"]:
        records = []
        packet_hash = raw_manifest["packet_sha256"][name]
        for line, raw in enumerate(preparation.jsonl(preparation.PACKETS / f"{name}.jsonl"), 1):
            record = projected_record(raw, packet_hash, line, sources[raw["concept"]["source_id"]])
            records.append(record)
            record_metadata[raw["concept"]["concept_id"]] = {
                "packet_name": name, "packet_line": line, "input_sha256": raw["input_sha256"],
                "projection_sha256": record["projection_sha256"],
                "lexicalization_ids": [lex["lexicalization_id"] for lex in raw["lexicalizations"]], "page_ids": [],
            }
        projected_path = directory / f"{name}.jsonl"
        preparation.write_records(projected_path, records)
        page_groups, current = [], []
        oversized = []
        for line, record in enumerate(records, 1):
            items = record_items(record, line)
            if items[0]["oversized"]:
                oversized.append({"concept_id": record["concept"]["concept_id"], "packet_line": line,
                                  "projection_record_bytes": len(encoded(record)), "fragment_count": len(items)})
            for item in items:
                proposed = current + [item]
                page = reading_page(name, len(page_groups) + 1, proposed, record_metadata, packet_hash)
                if len(encoded(page)) > PAGE_LIMIT:
                    if not current:
                        raise ValueError(f"single reading item exceeds page limit: {item['concept_id']}")
                    page_groups.append(current)
                    current = [item]
                    if len(encoded(reading_page(name, len(page_groups)+1, current, record_metadata, packet_hash))) > PAGE_LIMIT:
                        raise ValueError("oversized fragment envelope exceeds page limit")
                else:
                    current = proposed
        if current:
            page_groups.append(current)
        page_bytes = 0
        for number, items in enumerate(page_groups, 1):
            page = reading_page(name, number, items, record_metadata, packet_hash, final=number == len(page_groups))
            path = pages_dir / (page["page_id"].replace(":", "-") + ".json")
            path.write_bytes(encoded(page))
            page_bytes += path.stat().st_size
            page_inventory.append({"page_id": page["page_id"], "file": "pages/" + path.name,
                                   "sha256": sha256_file(path), "bytes": path.stat().st_size,
                                   "coverage": page["coverage"], "next_page_cursor": page["next_page_cursor"]})
            for cid in {item["concept_id"] for item in items}:
                record_metadata[cid]["page_ids"].append(page["page_id"])
        packet_stats[name] = {"records": len(records),
                              "lexicalizations": sum(len(r["lexicalizations"]) for r in records),
                              "projection_bytes": projected_path.stat().st_size, "projection_sha256": sha256_file(projected_path),
                              "reading_pages": len(page_groups), "reading_page_bytes": page_bytes,
                              "oversized_records": oversized}
    write_json(directory / "manifest.json", {
        "schema_version": "development-curation-projection-manifest/1", "input_inventory": inventory(),
        "packet_stats": packet_stats, "pages": page_inventory, "concept_projections": record_metadata,
        "directly_reviewed_concepts": 0,
        "limitations": ["Lossless semantic projection; only repeated full canonical relation records summarized.",
                        "Required source context and every lexicalization are preserved; no semantic judgment performed.",
                        "Read every page/fragment associated with a concept before declaring direct review.",
                        "Page-ID declarations are structurally checked, not proof that an agent actually read or reasoned correctly."]})


def verify(directory: Path = ROOT) -> dict:
    preparation.verify_packets(preparation.PACKETS)
    manifest = read_json(directory / "manifest.json")
    if manifest["input_inventory"] != inventory():
        raise ValueError("projection inputs changed")
    raw_manifest = read_json(preparation.PACKETS / "manifest.json")
    sources = {s["source_id"]: s for s in raw_manifest["source_records"]}
    records = {}
    for name, stats in manifest["packet_stats"].items():
        path = directory / f"{name}.jsonl"
        if sha256_file(path) != stats["projection_sha256"] or path.stat().st_size != stats["projection_bytes"]:
            raise ValueError(f"projection file hash/size mismatch: {name}")
        rows = list(preparation.jsonl(path))
        raw_rows = list(preparation.jsonl(preparation.PACKETS / f"{name}.jsonl"))
        if len(rows) != len(raw_rows) or len(rows) != stats["records"]:
            raise ValueError("projection/raw missing or extra records")
        for line, (record, raw) in enumerate(zip(rows, raw_rows), 1):
            expected = projected_record(raw, raw_manifest["packet_sha256"][name], line,
                                        sources[raw["concept"]["source_id"]])
            # Whole-object comparison proves every semantic field and original
            # lexicalization survives, and reconstructs relation summary/locator.
            if record != expected:
                raise ValueError(f"lossy/misassigned projection: {raw['concept']['concept_id']}")
            cid = record["concept"]["concept_id"]
            if cid in records:
                raise ValueError(f"duplicate projected concept: {cid}")
            records[cid] = record
    reconstructed, fragments, observations = {}, collections.defaultdict(list), collections.defaultdict(list)
    bytes_total = 0
    for entry in manifest["pages"]:
        path = directory / entry["file"]
        if sha256_file(path) != entry["sha256"] or path.stat().st_size != entry["bytes"] or entry["bytes"] > PAGE_LIMIT:
            raise ValueError("reading page hash/size/limit mismatch")
        page = json.loads(path.read_text(encoding="utf-8"))
        number = int(page["page_id"].split(":")[1])
        count = manifest["packet_stats"][page["packet_name"]]["reading_pages"]
        expected_page = reading_page(page["packet_name"], number, page["items"], manifest["concept_projections"],
                                     raw_manifest["packet_sha256"][page["packet_name"]], final=number == count)
        if page != expected_page or (page["page_id"], page["coverage"], page["next_page_cursor"]) != (
            entry["page_id"], entry["coverage"], entry["next_page_cursor"]
        ):
            raise ValueError("page metadata/cursor mismatch")
        bytes_total += entry["bytes"]
        for item in page["items"]:
            cid = item["concept_id"]
            if cid not in records or item["projection_sha256"] != records[cid]["projection_sha256"]:
                raise ValueError("unregistered reading item/input hash")
            metadata = manifest["concept_projections"][cid]
            if item["packet_line"] != metadata["packet_line"] or page["packet_name"] != metadata["packet_name"]:
                raise ValueError("reading page packet/line mismatch")
            if entry["page_id"] not in observations[cid]:
                observations[cid].append(entry["page_id"])
            if item["oversized"]:
                if sha256_bytes(item["record_fragment"].encode("utf-8")) != item["fragment_utf8_sha256"]:
                    raise ValueError("fragment hash mismatch")
                fragments[cid].append(item)
            else:
                if cid in reconstructed or cid in fragments:
                    raise ValueError("duplicate reading record")
                reconstructed[cid] = item["record"]
    for cid, pieces in fragments.items():
        if cid in reconstructed:
            raise ValueError("fragmented and complete record overlap")
        count = pieces[0]["fragment_count"]
        if len(pieces) != count or [p["fragment_index"] for p in pieces] != list(range(1, count+1)) or any(
            p["fragment_count"] != count for p in pieces
        ):
            raise ValueError(f"missing/duplicate/out-of-order fragments: {cid}")
        reconstructed[cid] = json.loads("".join(p["record_fragment"] for p in pieces))
    if reconstructed != records or set(manifest["concept_projections"]) != set(records):
        raise ValueError("page reconstruction differs from complete semantic projection")
    for cid, record in records.items():
        metadata = manifest["concept_projections"][cid]
        if metadata["page_ids"] != observations[cid] or metadata["input_sha256"] != record["input_sha256"] or metadata["projection_sha256"] != record["projection_sha256"] or metadata["lexicalization_ids"] != [l["lexicalization_id"] for l in record["lexicalizations"]]:
            raise ValueError(f"reading coverage metadata mismatch: {cid}")
    if len(records) != raw_manifest["denominators"]["candidate_concepts"]:
        raise ValueError("missing packet in projection")
    return {"records": len(records), "lexicalizations": sum(len(r["lexicalizations"]) for r in records.values()),
            "reading_pages": len(manifest["pages"]), "reading_page_bytes": bytes_total,
            "maximum_page_bytes": max(p["bytes"] for p in manifest["pages"]), "page_byte_limit": PAGE_LIMIT,
            "oversized_records": len(fragments),
            "proof": "All semantic objects and lexicalizations equal raw packets; all relation summaries restored/compared; all pages/fragments reassembled without loss.",
            "projection_manifest_sha256": sha256_file(directory / "manifest.json")}


def prepare() -> dict:
    temporary = Path(tempfile.mkdtemp(prefix=".projection-", dir=preparation.CURATION))
    try:
        generate(temporary)
        evidence = verify(temporary)
        if ROOT.exists():
            old = {p.relative_to(ROOT).as_posix(): sha256_file(p) for p in ROOT.rglob("*") if p.is_file()}
            new = {p.relative_to(temporary).as_posix(): sha256_file(p) for p in temporary.rglob("*") if p.is_file()}
            if old != new:
                raise ValueError("existing projection differs; refusing overwrite")
            evidence["publication"] = "identical_existing_projection_reused"
        else:
            temporary.rename(ROOT)
            evidence["publication"] = "validated_temporary_directory_renamed"
        return evidence
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)


def read_page(cursor: str) -> None:
    manifest = read_json(ROOT / "manifest.json")
    if manifest["input_inventory"] != inventory():
        raise ValueError("projection inputs changed")
    matches = [p for p in manifest["pages"] if p["page_id"] == cursor]
    if len(matches) != 1:
        raise ValueError(f"unregistered page cursor: {cursor}")
    entry = matches[0]
    path = ROOT / entry["file"]
    if sha256_file(path) != entry["sha256"]:
        raise ValueError("reading page hash mismatch")
    raw = read_json(preparation.PACKETS / "manifest.json")
    name = entry["page_id"].split(":", 1)[0]
    if sha256_file(preparation.PACKETS / f"{name}.jsonl") != raw["packet_sha256"][name]:
        raise ValueError("underlying raw packet changed")
    print(path.read_text(encoding="utf-8"), end="")


def validate_decisions(path: Path, packet_name: str | None) -> dict:
    verify()
    manifest = read_json(ROOT / "manifest.json")
    raw_manifest = read_json(preparation.PACKETS / "manifest.json")
    if packet_name is not None and packet_name not in manifest["packet_stats"]:
        raise ValueError("unregistered packet name")
    expected = {}
    for name in manifest["packet_stats"]:
        if packet_name is None or packet_name == name:
            for raw in preparation.jsonl(preparation.PACKETS / f"{name}.jsonl"):
                expected[raw["concept"]["concept_id"]] = raw
    seen, counts = set(), collections.Counter()
    manifest_hash = sha256_file(ROOT / "manifest.json")
    for row in preparation.jsonl(path):
        cid = row.get("concept_id")
        if cid in seen or cid not in expected:
            raise ValueError(f"duplicate/unregistered decision concept: {cid}")
        if set(row) != DECISION_FIELDS or row["schema_version"] != "development-curation-decision/2":
            raise ValueError("decision/2 fields/schema mismatch")
        raw = expected[cid]
        metadata = manifest["concept_projections"][cid]
        if (row["packet_sha256"], row["projection_version"], row["projection_sha256"], row["projection_manifest_sha256"]) != (
            raw_manifest["packet_sha256"][raw["packet_name"]], VERSION,
            metadata["projection_sha256"], manifest_hash
        ):
            raise ValueError("decision raw packet/projection version/hash mismatch")
        if row["reviewed_page_ids"] != metadata["page_ids"]:
            raise ValueError(f"every associated reading page/oversized fragment required: {cid}")
        legacy = {k: v for k, v in row.items() if k not in (
            "packet_sha256", "projection_version", "projection_sha256", "projection_manifest_sha256", "reviewed_page_ids"
        )}
        legacy["schema_version"] = "development-curation-decision/1"
        preparation.validate_decision(legacy, raw)
        seen.add(cid)
        counts[row["decision"]] += 1
    if seen != set(expected):
        raise ValueError(f"missing decisions: {len(set(expected) - seen)}")
    return {"scope": packet_name or "all_candidates", "decision_records": len(seen), "decision_counts": dict(counts),
            "decisions_sha256": sha256_file(path), "projection_manifest_sha256": manifest_hash,
            "semantic_correctness": "independent review required; not machine established"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "verify"):
        commands.add_parser(name)
    page = commands.add_parser("page")
    page.add_argument("cursor")
    decisions = commands.add_parser("validate-decisions")
    decisions.add_argument("path", type=Path)
    decisions.add_argument("--packet", choices=("wordnet-operations", "wordnet-technical", "catalogs", "abbreviations"))
    args = parser.parse_args()
    try:
        if args.command == "page":
            read_page(args.cursor)
            return
        if args.command == "prepare":
            result = prepare()
        elif args.command == "verify":
            result = verify()
        else:
            result = validate_decisions(args.path, args.packet)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (ValueError, KeyError, TypeError) as error:
        raise SystemExit(f"ERROR: {error}") from error


if __name__ == "__main__":
    main()
