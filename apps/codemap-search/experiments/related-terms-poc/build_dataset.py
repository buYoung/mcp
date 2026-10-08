#!/usr/bin/env python3
"""Build the canonical related-terms JSONL dataset and its generated SQLite artifact (offline).

    python3 build_dataset.py build                 # archives -> JSONL -> SQLite -> verify -> stats
    python3 build_dataset.py sqlite <dataset-dir>  # regenerate SQLite from the JSONL only
    python3 build_dataset.py verify <dataset-dir>  # round trip, integrity, provenance checks

Inputs are the archives pinned by sources.lock.json. Outputs go to the ignored
`.codemap/poc/related-terms/datasets/<dataset-id>/`; compact statistics go to
results/dataset-stats.json. See contracts/dataset.md for the record contract.
"""

from __future__ import annotations

import argparse
import collections
import io
import json
import platform
import re
import shutil
import sqlite3
import sys
import tarfile
import tempfile
import urllib.parse
import zipfile
from pathlib import Path

from rtpoc_common import (
    DATASETS_DIR,
    EXPERIMENT_DIR,
    NORMALIZATION_VERSION,
    REPO_ROOT,
    RESULTS_DIR,
    SCHEMA_VERSION,
    SOURCES_LOCK_PATH,
    canonical_json,
    exact_key,
    parse_front_matter,
    read_json,
    repo_relative,
    sha256_bytes,
    sha256_file,
    split_csv_field,
    token_key,
    write_json,
)

JSONL_FILES = ("sources", "concepts", "expressions", "lexicalizations", "relations")
PRIMARY_KEYS = {
    "sources": "source_id",
    "concepts": "concept_id",
    "expressions": "expression_id",
    "lexicalizations": "lexicalization_id",
    "relations": "relation_id",
}
SQLITE_NAME = "dataset.sqlite3"
SCHEMA_PATH = EXPERIMENT_DIR / "schema.sql"
RECORD_SCHEMA_PATH = EXPERIMENT_DIR / "contracts" / "dataset.schema.json"
ATTRIBUTION_PATH = EXPERIMENT_DIR / "ATTRIBUTION.md"
REQUIRED_INDEXES = (
    "expressions_by_token_key",
    "relations_by_from_expression",
    "lexicalizations_by_expression",
    "lexicalizations_by_concept",
)
# Allowed evidence per relation kind, and whether both endpoints must share one concept.
# Synonym/alias/abbreviation evidence is membership in one source record; it never spans
# concepts, so no relation is produced by transitive closure through a shared expression.
RELATION_RULES = {
    "synonym": ({"wordnet_same_synset"}, True),
    "alias": ({"github_topic_same_topic", "wikidata_same_item"}, True),
    "abbreviation_expansion": ({"abbrcode_entry", "se_abbr_entry"}, True),
    "abbreviation_of": ({"abbrcode_entry", "se_abbr_entry"}, True),
    "similar": ({"wordnet_similar_synset"}, False),
    "antonym": ({"wordnet_sense_antonym"}, False),
    "related_topic": ({"github_topic_related"}, False),
}
ABBREVIATION_ROLES = {
    "abbreviation_expansion": ("abbreviation", "expansion"),
    "abbreviation_of": ("expansion", "abbreviation"),
}
# Exploratory probes recorded with the dataset statistics. They are known diagnostic
# controls from the authoring research, not an evaluation sample.
DIAGNOSTIC_PROBES = (
    "delete",
    "erase",
    "retry",
    "deserialize",
    "serialize",
    "enqueue",
    "dequeue",
    "memoize",
    "memoization",
    "event loop",
    "res",
    "dir",
)

SOURCE_DESCRIPTIONS = {
    "oewn-2025": ("Open English WordNet 2025", "general_english_lexicon"),
    "github-explore-c8d7bc5": ("GitHub Explore topics", "software_topic_catalog"),
    "wikidata-sel1": ("Wikidata structured entities (selection 1)", "structured_concepts"),
    "abbrcode-bd33b1c": ("abbreviations-in-code", "code_identifier_abbreviations"),
    "se-abbr-8e1d77e": ("software-engineering-abbreviations-acronyms", "software_acronyms"),
}


class DatasetCollector:
    """Accumulates canonical records and enforces identity rules while importing."""

    def __init__(self) -> None:
        self.sources: dict[str, dict] = {}
        self.concepts: dict[str, dict] = {}
        self.expressions: dict[str, dict] = {}
        self.lexicalizations: dict[str, dict] = {}
        self.relations: dict[str, dict] = {}
        self.raw_counts: dict[str, dict] = collections.defaultdict(dict)
        self.exclusions: collections.Counter = collections.Counter()
        self.skipped_same_expression_pairs: collections.Counter = collections.Counter()

    def add_source(self, lock_entry: dict) -> None:
        name, scope = SOURCE_DESCRIPTIONS[lock_entry["source_id"]]
        self.sources[lock_entry["source_id"]] = {
            "record_type": "source",
            "source_id": lock_entry["source_id"],
            "name": name,
            "semantic_scope": scope,
            "license_spdx": lock_entry["license"]["spdx"],
            "attribution": lock_entry["license"]["notice"],
            "upstream": lock_entry["repository"],
            "revision": lock_entry["revision"],
            "archive_sha256": lock_entry["archive_sha256"],
        }

    def add_concept(self, concept_id: str, source_id: str, kind: str, source_record: str, *, pos=None, gloss=None,
                    domain_labels=(), attributes=None) -> None:
        if concept_id in self.concepts:
            raise ValueError(f"duplicate concept {concept_id}")
        self.concepts[concept_id] = {
            "record_type": "concept",
            "concept_id": concept_id,
            "source_id": source_id,
            "concept_kind": kind,
            "source_record": source_record,
            "pos": pos,
            "gloss": gloss,
            "domain_labels": sorted(set(domain_labels)),
            "attributes": attributes or {},
        }

    def add_lexicalization(self, lexicalization_id: str, concept_id: str, surface: str, role: str,
                           source_record: str, rank: int) -> str | None:
        """Register `surface` under a concept; returns the lexicalization id or None if excluded."""
        surface = surface.strip()
        key = exact_key(surface)
        if not key:
            self.exclusions[f"{self.concepts[concept_id]['source_id']}:empty_surface"] += 1
            return None
        if lexicalization_id in self.lexicalizations:
            raise ValueError(f"duplicate lexicalization {lexicalization_id}")
        expression_id = "x:" + key
        expression = self.expressions.get(expression_id)
        if expression is None:
            normalized_tokens, is_lossy = token_key(key)
            expression = {
                "record_type": "expression",
                "expression_id": expression_id,
                "exact_key": key,
                "token_key": normalized_tokens,
                "token_key_lossy": is_lossy,
                "token_count": len(normalized_tokens.split()) if normalized_tokens else 0,
                "surfaces": [],
            }
            self.expressions[expression_id] = expression
        if surface not in expression["surfaces"]:
            expression["surfaces"].append(surface)
        self.lexicalizations[lexicalization_id] = {
            "record_type": "lexicalization",
            "lexicalization_id": lexicalization_id,
            "concept_id": concept_id,
            "expression_id": expression_id,
            "surface": surface,
            "role": role,
            "source_record": source_record,
            "rank": rank,
        }
        return lexicalization_id

    def add_relation(self, kind: str, direction: str, evidence_kind: str, from_lex: str, to_lex: str) -> None:
        source = self.lexicalizations[from_lex]
        target = self.lexicalizations[to_lex]
        if source["expression_id"] == target["expression_id"]:
            self.skipped_same_expression_pairs[evidence_kind] += 1
            return
        relation_id = "r:" + sha256_bytes(canonical_json([kind, evidence_kind, from_lex, to_lex]).encode())[:24]
        if relation_id in self.relations:
            raise ValueError(f"duplicate relation {relation_id}")
        self.relations[relation_id] = {
            "record_type": "relation",
            "relation_id": relation_id,
            "relation_kind": kind,
            "direction": direction,
            "evidence_kind": evidence_kind,
            "source_id": self.concepts[source["concept_id"]]["source_id"],
            "from_expression_id": source["expression_id"],
            "to_expression_id": target["expression_id"],
            "from_lexicalization_id": from_lex,
            "to_lexicalization_id": to_lex,
            "from_concept_id": source["concept_id"],
            "to_concept_id": target["concept_id"],
        }

    def add_symmetric_pairs(self, kind: str, evidence_kind: str, lexicalization_ids: list[str]) -> None:
        for from_lex in lexicalization_ids:
            for to_lex in lexicalization_ids:
                if from_lex != to_lex:
                    self.add_relation(kind, "symmetric_pair", evidence_kind, from_lex, to_lex)

    def records(self) -> dict[str, list[dict]]:
        for expression in self.expressions.values():
            expression["surfaces"].sort()
        return {
            name: sorted(getattr(self, name).values(), key=lambda record, name=name: record[PRIMARY_KEYS[name]])
            for name in JSONL_FILES
        }


# --- source archive access ------------------------------------------------------------------


def load_lock() -> dict[str, dict]:
    lock = read_json(SOURCES_LOCK_PATH)
    entries = {entry["source_id"]: entry for entry in lock["sources"]}
    for entry in entries.values():
        path = REPO_ROOT / entry["archive_path"]
        if sha256_file(path) != entry["archive_sha256"]:
            raise SystemExit(f"archive hash mismatch for {entry['source_id']}: {entry['archive_path']}")
    return entries


def tar_members(lock_entry: dict) -> dict[str, bytes]:
    prefix = lock_entry["source_id"] + "/"
    with tarfile.open(REPO_ROOT / lock_entry["archive_path"]) as archive:
        return {
            member.name[len(prefix) :]: archive.extractfile(member).read()
            for member in archive.getmembers()
            if member.isfile()
        }


# --- importers ------------------------------------------------------------------------------


def import_wordnet(collector: DatasetCollector, lock_entry: dict) -> None:
    source_id = lock_entry["source_id"]
    synsets: dict[str, dict] = {}
    lexfiles: dict[str, str] = {}
    entries: dict[str, dict] = {}
    with zipfile.ZipFile(REPO_ROOT / lock_entry["archive_path"]) as archive:
        for name in sorted(archive.namelist()):
            data = json.loads(archive.read(name))
            if name.startswith("entries-"):
                entries.update(data)
            elif name != "frames.json":
                for synset_id, synset in data.items():
                    synsets[synset_id] = synset
                    lexfiles[synset_id] = name[: -len(".json")]

    # Bulk JSON links lemma -> part of speech -> senses[{id, synset}]; the synset map is keyed
    # by synset id and lists member lemmas. Resolve each membership to its sense key.
    sense_by_member: dict[tuple[str, str], tuple[str, int]] = {}
    sense_records: dict[str, dict] = {}
    form_count = 0
    for lemma, by_pos in entries.items():
        for body in by_pos.values():
            form_count += len(body.get("form", []))
            for rank, sense in enumerate(body.get("sense", [])):
                key = (lemma, sense["synset"])
                if key in sense_by_member:
                    raise ValueError(f"lemma {lemma!r} has two senses in {sense['synset']}")
                sense_by_member[key] = (sense["id"], rank)
                sense_records[sense["id"]] = sense

    def domain_label(synset_id: str) -> str:
        return synsets[synset_id]["members"][0]

    lexicalizations_by_synset: dict[str, list[str]] = {}
    lexicalization_by_sense: dict[str, str] = {}
    for synset_id in sorted(synsets):
        synset = synsets[synset_id]
        concept_id = f"{source_id}:synset:{synset_id}"
        collector.add_concept(
            concept_id,
            source_id,
            "wordnet_synset",
            synset_id,
            pos=synset["partOfSpeech"],
            gloss=synset["definition"][0] if synset.get("definition") else None,
            domain_labels=[domain_label(topic) for topic in synset.get("domain_topic", [])],
            attributes={
                "lexfile": lexfiles[synset_id],
                "ili": synset.get("ili"),
                "definition_count": len(synset.get("definition", [])),
                "wikidata": sorted(synset.get("wikidata", [])) if isinstance(synset.get("wikidata"), list)
                else ([synset["wikidata"]] if synset.get("wikidata") else []),
            },
        )
        lexicalization_ids = []
        for member in synset["members"]:
            sense_id, rank = sense_by_member[(member, synset_id)]
            lexicalization_id = collector.add_lexicalization(
                f"{source_id}:sense:{sense_id}", concept_id, member, "wordnet_member", sense_id, rank
            )
            if lexicalization_id:
                lexicalization_ids.append(lexicalization_id)
                lexicalization_by_sense[sense_id] = lexicalization_id
        lexicalizations_by_synset[synset_id] = lexicalization_ids
        collector.add_symmetric_pairs("synonym", "wordnet_same_synset", lexicalization_ids)

    similar_links = 0
    for synset_id in sorted(synsets):
        for target in synsets[synset_id].get("similar", []):
            similar_links += 1
            for from_lex in lexicalizations_by_synset[synset_id]:
                for to_lex in lexicalizations_by_synset[target]:
                    collector.add_relation("similar", "declared", "wordnet_similar_synset", from_lex, to_lex)

    antonym_links = 0
    for sense_id in sorted(sense_records):
        for target in sense_records[sense_id].get("antonym", []):
            antonym_links += 1
            if target not in lexicalization_by_sense:
                collector.exclusions[f"{source_id}:antonym_target_missing"] += 1
                continue
            collector.add_relation("antonym", "declared", "wordnet_sense_antonym",
                                   lexicalization_by_sense[sense_id], lexicalization_by_sense[target])

    member_counts = [len(synset["members"]) for synset in synsets.values()]
    collector.raw_counts[source_id] = {
        "synsets": len(synsets),
        "synsets_with_multiple_members": sum(count > 1 for count in member_counts),
        "memberships": sum(member_counts),
        "raw_directed_member_pairs": sum(count * (count - 1) for count in member_counts),
        "entry_lemmas": len(entries),
        "similar_links": similar_links,
        "sense_antonym_links": antonym_links,
        "domain_tagged_synsets": sum(1 for synset in synsets.values() if synset.get("domain_topic")),
    }
    collector.exclusions[f"{source_id}:morphological_form_variants_not_imported"] += form_count
    collector.exclusions[f"{source_id}:examples_and_other_relations_not_imported"] += 1


def explore_topics(lock_entry: dict) -> dict[str, dict]:
    topics = {}
    for path, data in tar_members(lock_entry).items():
        if path.startswith("topics/") and path.endswith("/index.md"):
            topics[path.split("/")[1]] = parse_front_matter(data.decode("utf-8"))
    return topics


def import_github_explore(collector: DatasetCollector, lock_entry: dict, wikidata_by_topic: dict[str, str]) -> None:
    source_id = lock_entry["source_id"]
    topics = explore_topics(lock_entry)
    lexicalizations_by_topic: dict[str, list[str]] = {}
    primary_by_topic: dict[str, list[str]] = {}
    alias_total = 0

    for slug in sorted(topics):
        fields = topics[slug]
        concept_id = f"{source_id}:topic:{slug}"
        aliases = split_csv_field(fields.get("aliases"))
        related = split_csv_field(fields.get("related"))
        alias_total += len(aliases)
        collector.add_concept(
            concept_id,
            source_id,
            "github_topic",
            f"topics/{slug}/index.md",
            gloss=fields.get("short_description") or None,
            attributes={
                "related": related,
                "wikipedia_url": fields.get("wikipedia_url"),
                "wikidata_concept": wikidata_by_topic.get(slug),
            },
        )
        primary = [
            collector.add_lexicalization(f"{concept_id}#slug", concept_id, slug, "topic_slug", f"topics/{slug}", 0)
        ]
        if fields.get("display_name"):
            primary.append(
                collector.add_lexicalization(
                    f"{concept_id}#display_name", concept_id, fields["display_name"], "topic_display_name",
                    f"topics/{slug}#display_name", 0,
                )
            )
        alias_ids = [
            collector.add_lexicalization(
                f"{concept_id}#alias:{index}", concept_id, alias, "topic_alias", f"topics/{slug}#aliases[{index}]", index
            )
            for index, alias in enumerate(aliases)
        ]
        primary = [lex for lex in primary if lex]
        lexicalizations = primary + [lex for lex in alias_ids if lex]
        primary_by_topic[slug] = primary
        lexicalizations_by_topic[slug] = lexicalizations
        collector.add_symmetric_pairs("alias", "github_topic_same_topic", lexicalizations)

    reference_topics = 0
    related_links = 0
    for slug in sorted(topics):
        for target in split_csv_field(topics[slug].get("related")):
            related_links += 1
            if target not in topics and target not in primary_by_topic:
                # A related slug without its own topic page is still a declared GitHub topic name.
                reference_id = f"{source_id}:topic-ref:{target}"
                collector.add_concept(reference_id, source_id, "github_topic_reference", f"related:{target}")
                lex = collector.add_lexicalization(
                    f"{reference_id}#slug", reference_id, target, "topic_slug", f"related:{target}", 0
                )
                primary_by_topic[target] = [lex] if lex else []
                reference_topics += 1
            for from_lex in lexicalizations_by_topic[slug]:
                for to_lex in primary_by_topic[target]:
                    collector.add_relation("related_topic", "declared", "github_topic_related", from_lex, to_lex)

    collector.raw_counts[source_id] = {
        "topic_pages": len(topics),
        "topic_pages_with_aliases": sum(1 for fields in topics.values() if split_csv_field(fields.get("aliases"))),
        "declared_aliases": alias_total,
        "declared_related_links": related_links,
        "related_slugs_without_topic_page": reference_topics,
        "topic_pages_with_wikipedia_url": sum(1 for fields in topics.values() if fields.get("wikipedia_url")),
    }


def import_wikidata(collector: DatasetCollector, lock_entry: dict, explore_entry: dict) -> dict[str, str]:
    """Import selected entities; returns topic slug -> Wikidata concept id for cross references."""
    source_id = lock_entry["source_id"]
    members = tar_members(lock_entry)
    plan = json.loads(members["requests.json"])
    entities: dict[str, dict] = {}
    for request in plan["requests"]:
        for entity_id, entity in json.loads(members[request["file"]]).get("entities", {}).items():
            if "missing" not in entity:
                entities[entity_id] = entity
    by_title = {
        entity["sitelinks"]["enwiki"]["title"]: entity_id
        for entity_id, entity in entities.items()
        if entity.get("sitelinks", {}).get("enwiki")
    }
    redirects = plan.get("redirect_mapping", {})

    selected_by: dict[str, list[str]] = collections.defaultdict(list)
    wikidata_by_topic: dict[str, str] = {}
    unresolved = 0

    topics = explore_topics(explore_entry)
    for slug in sorted(topics):
        url = topics[slug].get("wikipedia_url")
        if not url:
            continue
        parts = urllib.parse.urlsplit(url)
        if parts.netloc != "en.wikipedia.org" or not parts.path.startswith("/wiki/") or parts.fragment:
            continue
        title = urllib.parse.unquote(parts.path[len("/wiki/") :]).replace("_", " ")
        entity_id = by_title.get(title) or by_title.get(redirects.get(title, ""))
        if not entity_id:
            unresolved += 1
            continue
        selected_by[entity_id].append(slug)
    for entity_id in plan["diagnostic_ids"]:
        selected_by[entity_id].append("authoring_diagnostic")

    alias_total = 0
    for entity_id in sorted(selected_by):
        entity = entities[entity_id]
        concept_id = f"{source_id}:{entity_id}@r{entity['lastrevid']}"
        for slug in selected_by[entity_id]:
            if slug != "authoring_diagnostic":
                wikidata_by_topic[slug] = concept_id
        collector.add_concept(
            concept_id,
            source_id,
            "wikidata_item",
            f"{entity_id}@{entity['lastrevid']}",
            gloss=entity.get("descriptions", {}).get("en", {}).get("value"),
            attributes={
                "entity_id": entity_id,
                "lastrevid": entity["lastrevid"],
                "enwiki_title": entity.get("sitelinks", {}).get("enwiki", {}).get("title"),
                "selected_by": sorted(selected_by[entity_id]),
            },
        )
        lexicalizations = []
        label = entity.get("labels", {}).get("en", {}).get("value")
        if label:
            lexicalizations.append(
                collector.add_lexicalization(f"{concept_id}#label", concept_id, label, "wikidata_label",
                                             f"{entity_id}#labels.en", 0)
            )
        aliases = [alias["value"] for alias in entity.get("aliases", {}).get("en", [])]
        alias_total += len(aliases)
        for index, alias in enumerate(aliases):
            lexicalizations.append(
                collector.add_lexicalization(f"{concept_id}#alias:{index}", concept_id, alias, "wikidata_alias",
                                             f"{entity_id}#aliases.en[{index}]", index)
            )
        collector.add_symmetric_pairs("alias", "wikidata_same_item", [lex for lex in lexicalizations if lex])

    collector.raw_counts[source_id] = {
        "requested_titles": plan["title_count"],
        "url_exclusions": len(plan["url_exclusions"]),
        "redirect_resolved_titles": len(redirects),
        "resolved_entities": len(entities),
        "selected_entities": len(selected_by),
        "topic_links_unresolved": unresolved,
        "selected_entities_with_english_aliases": sum(
            1 for entity_id in selected_by if entities[entity_id].get("aliases", {}).get("en")
        ),
        "english_aliases": alias_total,
    }
    return wikidata_by_topic


def import_abbrcode(collector: DatasetCollector, lock_entry: dict) -> None:
    source_id = lock_entry["source_id"]
    data = json.loads(tar_members(lock_entry)["data/abbrs/.json"])
    pairs = 0
    expansions_by_abbr: dict[str, set] = collections.defaultdict(set)
    for entry_index, entry in enumerate(data):
        word = entry["word"]
        for abbr_index, item in enumerate(entry["abbrs"]):
            pairs += 1
            abbr = item["abbr"]
            expansions_by_abbr[exact_key(abbr)].add(exact_key(word))
            concept_id = f"{source_id}:entry:{word}|{abbr}"
            record = f"data/abbrs/.json[{entry_index}].abbrs[{abbr_index}]"
            collector.add_concept(
                concept_id, source_id, "abbreviation_sense", record,
                attributes={"degree": item.get("degree"), "context": item.get("context")},
            )
            short = collector.add_lexicalization(f"{concept_id}#abbreviation", concept_id, abbr, "abbreviation", record, 0)
            full = collector.add_lexicalization(f"{concept_id}#expansion", concept_id, word, "expansion", record, 0)
            if short and full:
                collector.add_relation("abbreviation_expansion", "declared", "abbrcode_entry", short, full)
                collector.add_relation("abbreviation_of", "declared", "abbrcode_entry", full, short)
    collector.raw_counts[source_id] = {
        "words": len(data),
        "word_abbreviation_pairs": pairs,
        "ambiguous_abbreviations": sum(1 for expansions in expansions_by_abbr.values() if len(expansions) > 1),
        "degrees": dict(collections.Counter(item.get("degree") for entry in data for item in entry["abbrs"])),
    }


def import_se_abbreviations(collector: DatasetCollector, lock_entry: dict) -> None:
    source_id = lock_entry["source_id"]
    header = re.compile(r"^#\s+(?:(.+?):\s+)?(.+?)\s*$")
    conflicts = 0
    entries = 0
    expansions_by_abbr: dict[str, set] = collections.defaultdict(set)
    for path, data in sorted(tar_members(lock_entry).items()):
        if not (path.startswith("dictionary/") and path.endswith(".md")):
            continue
        entries += 1
        text = data.decode("utf-8")
        match = header.match(text.split("\n", 1)[0])
        file_abbr, file_name = path.rsplit("/", 1)[1][: -len(".md")].split("-", 1)
        header_abbr, expansion = (match.group(1), match.group(2)) if match else (None, file_name.replace("_", " "))
        # The file name is the entry identity; a different title abbreviation is recorded, not trusted.
        is_conflict = header_abbr is not None and header_abbr.lower() != file_abbr.lower()
        conflicts += is_conflict
        overview = re.search(r"## Overview\s*\n-\s*(.+)", text)
        expansions_by_abbr[exact_key(file_abbr)].add(exact_key(expansion))
        concept_id = f"{source_id}:entry:{path}"
        collector.add_concept(
            concept_id, source_id, "acronym_sense", path,
            gloss=overview.group(1).strip() if overview else None,
            attributes={"title_abbreviation": header_abbr, "title_abbreviation_conflict": is_conflict},
        )
        short = collector.add_lexicalization(f"{concept_id}#abbreviation", concept_id, file_abbr, "abbreviation", path, 0)
        full = collector.add_lexicalization(f"{concept_id}#expansion", concept_id, expansion, "expansion", path, 0)
        if short and full:
            collector.add_relation("abbreviation_expansion", "declared", "se_abbr_entry", short, full)
            collector.add_relation("abbreviation_of", "declared", "se_abbr_entry", full, short)
    collector.raw_counts[source_id] = {
        "dictionary_entries": entries,
        "title_abbreviation_conflicts": conflicts,
        "ambiguous_abbreviations": sum(1 for expansions in expansions_by_abbr.values() if len(expansions) > 1),
    }


# --- JSONL / SQLite --------------------------------------------------------------------------


def write_jsonl(directory: Path, records: dict[str, list[dict]]) -> dict[str, str]:
    hashes = {}
    for name in JSONL_FILES:
        payload = "".join(canonical_json(record) + "\n" for record in records[name]).encode("utf-8")
        (directory / f"{name}.jsonl").write_bytes(payload)
        hashes[name] = sha256_bytes(payload)
    return hashes


def dataset_id_from_hashes(hashes: dict[str, str]) -> str:
    joined = "".join(f"{name}\t{hashes[name]}\n" for name in JSONL_FILES)
    return "rtd1-" + sha256_bytes(f"{SCHEMA_VERSION}\n{NORMALIZATION_VERSION}\n{joined}".encode())[:16]


def read_jsonl(directory: Path, name: str) -> list[dict]:
    with (directory / f"{name}.jsonl").open(encoding="utf-8") as handle:
        return [json.loads(line) for line in handle]


ROW_CODECS = {
    "sources": (
        ["source_id", "name", "semantic_scope", "license_spdx", "attribution", "upstream", "revision_json", "archive_sha256"],
        {"revision_json": "revision"},
    ),
    "concepts": (
        ["concept_id", "source_id", "concept_kind", "source_record", "pos", "gloss", "domain_labels_json", "attributes_json"],
        {"domain_labels_json": "domain_labels", "attributes_json": "attributes"},
    ),
    "expressions": (
        ["expression_id", "exact_key", "token_key", "token_key_lossy", "token_count", "surfaces_json"],
        {"surfaces_json": "surfaces"},
    ),
    "lexicalizations": (
        ["lexicalization_id", "concept_id", "expression_id", "surface", "role", "source_record", "rank"],
        {},
    ),
    "relations": (
        ["relation_id", "relation_kind", "direction", "evidence_kind", "source_id", "from_expression_id",
         "to_expression_id", "from_lexicalization_id", "to_lexicalization_id", "from_concept_id", "to_concept_id"],
        {},
    ),
}


def record_to_row(name: str, record: dict) -> tuple:
    columns, json_columns = ROW_CODECS[name]
    row = []
    for column in columns:
        if column in json_columns:
            row.append(canonical_json(record[json_columns[column]]))
        elif column == "token_key_lossy":
            row.append(int(record[column]))
        else:
            row.append(record[column])
    return tuple(row)


def row_to_record(name: str, row: tuple) -> dict:
    columns, json_columns = ROW_CODECS[name]
    record = {"record_type": name[:-1]}
    for column, value in zip(columns, row):
        if column in json_columns:
            record[json_columns[column]] = json.loads(value)
        elif column == "token_key_lossy":
            record[column] = bool(value)
        else:
            record[column] = value
    return record


def build_sqlite(directory: Path) -> Path:
    """Generate dataset.sqlite3 from the directory's JSONL files alone."""
    manifest = read_json(directory / "manifest.json")
    temporary = directory / (SQLITE_NAME + ".tmp")
    temporary.unlink(missing_ok=True)
    connection = sqlite3.connect(temporary)
    try:
        connection.execute("PRAGMA journal_mode = DELETE")
        connection.execute("PRAGMA foreign_keys = ON")
        connection.executescript(SCHEMA_PATH.read_text(encoding="utf-8"))
        with connection:
            meta = {
                "dataset_id": manifest["dataset_id"],
                "schema_version": manifest["schema_version"],
                "normalization_version": manifest["normalization_version"],
                "jsonl_sha256": canonical_json(manifest["jsonl_sha256"]),
                "record_counts": canonical_json(manifest["record_counts"]),
            }
            connection.executemany("INSERT INTO dataset_meta VALUES (?, ?)", sorted(meta.items()))
            for name in JSONL_FILES:
                columns, _ = ROW_CODECS[name]
                placeholders = ", ".join("?" for _ in columns)
                connection.executemany(
                    f"INSERT INTO {name} ({', '.join(columns)}) VALUES ({placeholders})",
                    (record_to_row(name, record) for record in read_jsonl(directory, name)),
                )
        violations = connection.execute("PRAGMA foreign_key_check").fetchall()
        if violations:
            raise SystemExit(f"foreign key violations: {violations[:5]}")
        connection.execute("VACUUM")
    finally:
        connection.close()
    final = directory / SQLITE_NAME
    temporary.replace(final)
    return final


def logical_dump(connection: sqlite3.Connection) -> dict[str, list[str]]:
    dump = {}
    for name in JSONL_FILES:
        columns, _ = ROW_CODECS[name]
        key = PRIMARY_KEYS[name]
        rows = connection.execute(f"SELECT {', '.join(columns)} FROM {name} ORDER BY {key}").fetchall()
        dump[name] = [canonical_json(row_to_record(name, row)) for row in rows]
    return dump


JSON_TYPES = {"string": str, "object": dict, "array": list, "boolean": bool, "null": type(None)}


def schema_errors(record: dict, definition: dict) -> list[str]:
    """Check the JSON Schema subset used by contracts/dataset.schema.json."""
    errors = [f"missing {key}" for key in definition["required"] if key not in record]
    if definition.get("additionalProperties") is False:
        errors += [f"unexpected {key}" for key in record if key not in definition["properties"]]
    for key, rule in definition["properties"].items():
        if key not in record:
            continue
        value = record[key]
        if "const" in rule and value != rule["const"]:
            errors.append(f"{key} != {rule['const']}")
        if "enum" in rule and value not in rule["enum"]:
            errors.append(f"{key}={value!r} not in enum")
        if "type" in rule:
            allowed = rule["type"] if isinstance(rule["type"], list) else [rule["type"]]
            is_integer = isinstance(value, int) and not isinstance(value, bool)
            if not any(is_integer if name == "integer" else isinstance(value, JSON_TYPES[name]) for name in allowed):
                errors.append(f"{key} has type {type(value).__name__}")
    return errors


def verify_dataset(directory: Path) -> dict:
    """Round trip, integrity, and provenance checks. Raises SystemExit on any failure."""
    failures: list[str] = []
    record_schema = read_json(RECORD_SCHEMA_PATH)["$defs"]
    manifest = read_json(directory / "manifest.json")
    hashes = {name: sha256_file(directory / f"{name}.jsonl") for name in JSONL_FILES}
    if hashes != manifest["jsonl_sha256"]:
        failures.append("jsonl hashes differ from manifest")
    if dataset_id_from_hashes(hashes) != manifest["dataset_id"] or directory.name != manifest["dataset_id"]:
        failures.append("dataset id does not match JSONL content")

    jsonl_lines: dict[str, list[str]] = {}
    records: dict[str, list[dict]] = {}
    for name in JSONL_FILES:
        lines = (directory / f"{name}.jsonl").read_text(encoding="utf-8").splitlines()
        parsed = [json.loads(line) for line in lines]
        if any(canonical_json(record) != line for record, line in zip(parsed, lines)):
            failures.append(f"{name}: non-canonical JSON line")
        ids = [record[PRIMARY_KEYS[name]] for record in parsed]
        if ids != sorted(ids) or len(ids) != len(set(ids)):
            failures.append(f"{name}: ids not sorted/unique")
        if not parsed:
            failures.append(f"{name}: empty")
        invalid = [(record[PRIMARY_KEYS[name]], errors) for record in parsed
                   if (errors := schema_errors(record, record_schema[name[:-1]]))]
        if invalid:
            failures.append(f"{name}: {len(invalid)} schema violations, first {invalid[0]}")
        jsonl_lines[name] = lines
        records[name] = parsed

    sqlite_path = directory / SQLITE_NAME
    before = sorted(path.name for path in directory.iterdir())
    connection = sqlite3.connect(f"file:{sqlite_path}?mode=ro", uri=True)
    try:
        if connection.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
            failures.append("sqlite integrity_check failed")
        if connection.execute("PRAGMA journal_mode").fetchone()[0] != "delete":
            failures.append("sqlite journal mode is not delete")
        if connection.execute("PRAGMA foreign_key_check").fetchall():
            failures.append("sqlite foreign key violations")
        indexes = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type = 'index'")}
        missing_indexes = [name for name in REQUIRED_INDEXES if name not in indexes]
        if missing_indexes:
            failures.append(f"missing indexes {missing_indexes}")
        meta = dict(connection.execute("SELECT key, value FROM dataset_meta"))
        if meta.get("dataset_id") != manifest["dataset_id"]:
            failures.append("sqlite dataset_id differs")
        dump = logical_dump(connection)
        for name in JSONL_FILES:
            if dump[name] != jsonl_lines[name]:
                failures.append(f"{name}: sqlite rows differ from JSONL")
        logical_sha256 = sha256_bytes(canonical_json({"meta": meta, "tables": dump}).encode())
        query_plan = " ".join(
            str(row[-1])
            for row in connection.execute(
                "EXPLAIN QUERY PLAN SELECT * FROM relations WHERE from_expression_id = ?", ("x:delete",)
            )
        )
        if "relations_by_from_expression" not in query_plan:
            failures.append(f"relation lookup does not use its index: {query_plan}")
    finally:
        connection.close()
    after = sorted(path.name for path in directory.iterdir())
    if before != after:
        failures.append(f"read-only open changed directory listing: {set(after) - set(before)}")

    # Provenance: every serving relation resolves to lexicalizations, concepts, and a licensed source.
    lexicalizations = {record["lexicalization_id"]: record for record in records["lexicalizations"]}
    concepts = {record["concept_id"]: record for record in records["concepts"]}
    sources = {record["source_id"]: record for record in records["sources"]}
    expressions = {record["expression_id"] for record in records["expressions"]}
    token_keys = {record["expression_id"]: record["token_key"] for record in records["expressions"]}
    rule_violations = collections.Counter()
    equal_token_key_pairs = collections.Counter()
    for relation in records["relations"]:
        evidence_kinds, is_same_concept = RELATION_RULES.get(relation["relation_kind"], (set(), True))
        if relation["evidence_kind"] not in evidence_kinds:
            rule_violations["evidence_kind"] += 1
        if is_same_concept and relation["from_concept_id"] != relation["to_concept_id"]:
            rule_violations["cross_concept"] += 1
        roles = ABBREVIATION_ROLES.get(relation["relation_kind"])
        if roles and (
            lexicalizations[relation["from_lexicalization_id"]]["role"],
            lexicalizations[relation["to_lexicalization_id"]]["role"],
        ) != roles:
            rule_violations["abbreviation_roles"] += 1
        from_token_key = token_keys.get(relation["from_expression_id"])
        if from_token_key and from_token_key == token_keys.get(relation["to_expression_id"]):
            equal_token_key_pairs[relation["relation_kind"]] += 1
    if rule_violations:
        failures.append(f"relation rule violations {dict(rule_violations)}")
    dangling = 0
    for relation in records["relations"]:
        source = lexicalizations.get(relation["from_lexicalization_id"])
        target = lexicalizations.get(relation["to_lexicalization_id"])
        if (
            source is None
            or target is None
            or source["expression_id"] != relation["from_expression_id"]
            or target["expression_id"] != relation["to_expression_id"]
            or source["concept_id"] != relation["from_concept_id"]
            or target["concept_id"] != relation["to_concept_id"]
            or relation["from_concept_id"] not in concepts
            or relation["source_id"] not in sources
            or not sources[relation["source_id"]]["license_spdx"]
        ):
            dangling += 1
    for lexicalization in records["lexicalizations"]:
        if lexicalization["concept_id"] not in concepts or lexicalization["expression_id"] not in expressions:
            dangling += 1
    if dangling:
        failures.append(f"{dangling} dangling identifiers")
    if failures:
        raise SystemExit("dataset verification failed:\n  " + "\n  ".join(failures))
    return {
        "status": "pass",
        "logical_sqlite_sha256": logical_sha256,
        "sqlite_sha256": sha256_file(sqlite_path),
        "dangling_identifiers": dangling,
        "relation_rule_violations": 0,
        "schema_violations": 0,
        "relations_between_equal_token_keys": dict(sorted(equal_token_key_pairs.items())),
        "required_indexes": list(REQUIRED_INDEXES),
        "read_only_open_created_files": False,
    }


# --- statistics -------------------------------------------------------------------------------


def serving_statistics(records: dict[str, list[dict]]) -> dict:
    relations = records["relations"]
    expressions = records["expressions"]
    concepts_by_expression = collections.defaultdict(set)
    for lexicalization in records["lexicalizations"]:
        concepts_by_expression[lexicalization["expression_id"]].add(lexicalization["concept_id"])
    outgoing = collections.Counter(relation["from_expression_id"] for relation in relations)
    lexicalizations_per_concept = collections.Counter(lex["concept_id"] for lex in records["lexicalizations"])
    by_kind_source = collections.Counter((relation["relation_kind"], relation["source_id"]) for relation in relations)
    abbreviation_targets = collections.defaultdict(set)
    for relation in relations:
        if relation["relation_kind"] == "abbreviation_expansion":
            abbreviation_targets[relation["from_expression_id"]].add(relation["to_expression_id"])

    expression_index = {record["expression_id"]: record for record in expressions}
    probes = {}
    for probe in DIAGNOSTIC_PROBES:
        expression_id = "x:" + exact_key(probe)
        kinds = collections.Counter(
            relation["relation_kind"] for relation in relations if relation["from_expression_id"] == expression_id
        )
        probes[probe] = {
            "in_dictionary": expression_id in expression_index,
            "concepts": len(concepts_by_expression.get(expression_id, ())),
            "outgoing_relations_by_kind": dict(sorted(kinds.items())),
        }
    return {
        "record_counts": {name: len(records[name]) for name in JSONL_FILES},
        "relations_by_kind_and_source": {
            f"{kind}/{source}": count for (kind, source), count in sorted(by_kind_source.items())
        },
        "expressions_multi_token": sum(1 for record in expressions if record["token_count"] > 1),
        "expressions_with_lossy_token_key": sum(1 for record in expressions if record["token_key_lossy"]),
        "expressions_in_multiple_concepts": sum(1 for concepts in concepts_by_expression.values() if len(concepts) > 1),
        "expressions_without_outgoing_relations": sum(1 for record in expressions if not outgoing[record["expression_id"]]),
        "concepts_with_single_lexicalization": sum(1 for count in lexicalizations_per_concept.values() if count == 1),
        "ambiguous_abbreviations_in_serving_relations": sum(1 for targets in abbreviation_targets.values() if len(targets) > 1),
        "diagnostic_probes": probes,
    }


def build() -> int:
    lock = load_lock()
    collector = DatasetCollector()
    for entry in lock.values():
        collector.add_source(entry)
    import_wordnet(collector, lock["oewn-2025"])
    wikidata_by_topic = import_wikidata(collector, lock["wikidata-sel1"], lock["github-explore-c8d7bc5"])
    import_github_explore(collector, lock["github-explore-c8d7bc5"], wikidata_by_topic)
    import_abbrcode(collector, lock["abbrcode-bd33b1c"])
    import_se_abbreviations(collector, lock["se-abbr-8e1d77e"])
    records = collector.records()

    DATASETS_DIR.mkdir(parents=True, exist_ok=True)
    # Build the complete artifact off to the side. A failed build or verification must
    # never remove a published SQLite file, including when rebuilding the same content id.
    with tempfile.TemporaryDirectory(prefix=".staging-", dir=DATASETS_DIR) as temporary:
        staging_root = Path(temporary)
        staging = staging_root / "canonical"
        staging.mkdir()
        hashes = write_jsonl(staging, records)
        dataset_id = dataset_id_from_hashes(hashes)
        write_json(
            staging / "manifest.json",
            {
                "dataset_id": dataset_id,
                "schema_version": SCHEMA_VERSION,
                "normalization_version": NORMALIZATION_VERSION,
                "jsonl_sha256": hashes,
                "record_counts": {name: len(records[name]) for name in JSONL_FILES},
            },
        )
        shutil.copyfile(ATTRIBUTION_PATH, staging / "ATTRIBUTION.md")
        licenses = staging / "LICENSES"
        licenses.mkdir()
        for entry in lock.values():
            archive = entry.get("license_archive_path", entry["archive_path"])
            members = tar_members({**entry, "archive_path": archive})
            for name in entry["license_files"]:
                (licenses / f"{entry['source_id']}-{Path(name).name}").write_bytes(members[name])

        # verify_dataset validates the directory name against its content identity.
        candidate = staging.rename(staging_root / dataset_id)
        build_sqlite(candidate)
        candidate_verification = verify_dataset(candidate)
        final = DATASETS_DIR / dataset_id
        reproduced = None
        if final.exists():
            verification = verify_dataset(final)
            reproduced = (
                all(sha256_file(final / f"{name}.jsonl") == hashes[name] for name in JSONL_FILES)
                and verification["logical_sqlite_sha256"] == candidate_verification["logical_sqlite_sha256"]
            )
            if not reproduced:
                raise SystemExit("rebuilt content differs from the published dataset; existing artifact preserved")
            # Same logical content: keep the published directory and open reader handles intact.
        else:
            candidate.rename(final)
            verification = candidate_verification

    write_json(
        RESULTS_DIR / "dataset-stats.json",
        {
            "dataset_id": dataset_id,
            "schema_version": SCHEMA_VERSION,
            "normalization_version": NORMALIZATION_VERSION,
            "dataset_dir": repo_relative(final),
            "jsonl_sha256": hashes,
            "sqlite_sha256": verification["sqlite_sha256"],
            "logical_sqlite_sha256": verification["logical_sqlite_sha256"],
            "sqlite_bytes": (final / SQLITE_NAME).stat().st_size,
            "jsonl_bytes": {name: (final / f"{name}.jsonl").stat().st_size for name in JSONL_FILES},
            "raw_source_counts": collector.raw_counts,
            "serving_counts": serving_statistics(records),
            "exclusions": dict(sorted(collector.exclusions.items())),
            "skipped_same_expression_pairs": dict(sorted(collector.skipped_same_expression_pairs.items())),
            "corpus_support": "not_measured",
            "verification": verification,
            "regeneration_matches_existing": reproduced,
            "environment": build_environment(),
        },
    )
    print(f"dataset {dataset_id}: {verification['status']} (reproduced={reproduced})")
    return 0


def build_environment() -> dict:
    return {
        "python": platform.python_version(),
        "sqlite": sqlite3.sqlite_version,
        "platform": platform.platform(),
        "builder_sha256": sha256_file(Path(__file__)),
        "common_sha256": sha256_file(EXPERIMENT_DIR / "rtpoc_common.py"),
        "schema_sql_sha256": sha256_file(SCHEMA_PATH),
        "sources_lock_sha256": sha256_file(SOURCES_LOCK_PATH),
    }


HANDOFF_PATH = RESULTS_DIR / "dataset-handoff.json"


def write_handoff(directory: Path) -> int:
    """Re-inspect the built dataset and emit the child-02 handoff from observed values only."""
    stats = read_json(RESULTS_DIR / "dataset-stats.json")
    manifest = read_json(directory / "manifest.json")
    if stats["dataset_id"] != manifest["dataset_id"]:
        raise SystemExit("dataset-stats.json describes a different dataset")
    verification = verify_dataset(directory)
    lock = read_json(SOURCES_LOCK_PATH)
    source_hashes_ok = all(
        sha256_file(REPO_ROOT / entry["archive_path"]) == entry["archive_sha256"] for entry in lock["sources"]
    )
    connection = sqlite3.connect(f"file:{directory / SQLITE_NAME}?mode=ro", uri=True)
    try:
        sqlite_counts = {
            name: connection.execute(f"SELECT COUNT(*) FROM {name}").fetchone()[0] for name in JSONL_FILES
        }
    finally:
        connection.close()
    counts_match = sqlite_counts == manifest["record_counts"] == stats["serving_counts"]["record_counts"]
    integrity_ok = (
        verification["status"] == "pass"
        and source_hashes_ok
        and counts_match
        and stats["regeneration_matches_existing"] is True
        and verification["logical_sqlite_sha256"] == stats["logical_sqlite_sha256"]
    )
    artifacts = {f"{name}.jsonl": manifest["jsonl_sha256"][name] for name in JSONL_FILES}
    artifacts[SQLITE_NAME] = verification["sqlite_sha256"]
    for path in sorted(directory.glob("LICENSES/*")) + [directory / "ATTRIBUTION.md", directory / "manifest.json"]:
        artifacts[path.relative_to(directory).as_posix()] = sha256_file(path)
    write_json(
        HANDOFF_PATH,
        {
            "handoff": "related-terms-poc/dataset",
            "dataset_id": manifest["dataset_id"],
            "schema_version": manifest["schema_version"],
            "normalization_version": manifest["normalization_version"],
            "dataset_dir": repo_relative(directory),
            "jsonl_paths": [repo_relative(directory / f"{name}.jsonl") for name in JSONL_FILES],
            "sqlite_path": repo_relative(directory / SQLITE_NAME),
            "artifact_hashes": artifacts,
            "logical_sqlite_sha256": verification["logical_sqlite_sha256"],
            "source_lock_path": repo_relative(SOURCES_LOCK_PATH),
            "source_lock_sha256": sha256_file(SOURCES_LOCK_PATH),
            "attribution_path": repo_relative(ATTRIBUTION_PATH),
            "contract_paths": [
                repo_relative(EXPERIMENT_DIR / "contracts" / "dataset.md"),
                repo_relative(RECORD_SCHEMA_PATH),
                repo_relative(SCHEMA_PATH),
            ],
            "stats_path": repo_relative(RESULTS_DIR / "dataset-stats.json"),
            "counts": {
                "records": manifest["record_counts"],
                "relations_by_kind_and_source": stats["serving_counts"]["relations_by_kind_and_source"],
                "raw_source_counts": stats["raw_source_counts"],
            },
            "build_commands": [
                {"cwd": repo_relative(EXPERIMENT_DIR), "command": "python3 acquire_sources.py verify",
                 "outcome": "pass" if source_hashes_ok else "fail"},
                {"cwd": repo_relative(EXPERIMENT_DIR), "command": "python3 build_dataset.py build",
                 "outcome": f"pass; regeneration_matches_existing={stats['regeneration_matches_existing']}"},
                {"cwd": repo_relative(EXPERIMENT_DIR), "command": f"python3 build_dataset.py handoff {repo_relative(directory)}",
                 "outcome": verification["status"]},
            ],
            "environment": stats["environment"],
            "integrity_status": {
                "status": "pass" if integrity_ok else "fail",
                "source_archives_match_lock": source_hashes_ok,
                "sqlite_counts_match_manifest": counts_match,
                "read_only_open_created_files": verification["read_only_open_created_files"],
                "dangling_identifiers": verification["dangling_identifiers"],
                "relation_rule_violations": verification["relation_rule_violations"],
                "schema_violations": verification["schema_violations"],
                "regeneration_matches_existing": stats["regeneration_matches_existing"],
                "runtime_network_required": False,
            },
            "limitations": [
                "Raw upstream counts are not code-domain coverage; serving relations were not quality-reviewed here.",
                "WordNet is general English: e.g. its only 'retry' sense is legal (rehear); 'deserialize', 'enqueue', "
                "'dequeue', 'memoize' have no dictionary entry; 'memoization' exists in Wikidata without aliases.",
                "No morphology, spelling unification, or NFKC normalization; WordNet spelling variants in one synset "
                "remain synonym relations and are counted separately as equal-token-key pairs.",
                "Aliases come from curated catalogs (GitHub topics, Wikidata) and include broader category names; "
                "they are not synonyms.",
                "Corpus (public-code) support was not measured; no co-occurrence relations exist in this dataset.",
                "SQLite file bytes matched across two builds on one machine only; cross-platform byte identity is "
                "not claimed. Logical identity is logical_sqlite_sha256.",
                "Dataset size (JSONL ~314 MB, SQLite ~228 MB) is unoptimized for distribution.",
            ],
        },
    )
    print(f"handoff {'pass' if integrity_ok else 'FAIL'}: {repo_relative(HANDOFF_PATH)}")
    return 0 if integrity_ok else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("build")
    for name in ("sqlite", "verify", "handoff"):
        command = sub.add_parser(name)
        command.add_argument("dataset_dir", type=Path)
    args = parser.parse_args()
    if args.command == "build":
        return build()
    if args.command == "sqlite":
        build_sqlite(args.dataset_dir)
        print(json.dumps(verify_dataset(args.dataset_dir), indent=2))
        return 0
    if args.command == "handoff":
        return write_handoff(args.dataset_dir)
    print(json.dumps(verify_dataset(args.dataset_dir), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
