#!/usr/bin/env python3
"""Development curation v2: WordNet judgment batches and the merged v2 decision file.

  batches  write compact WordNet synset batches for judges (rubric: contracts/curation-v2-rubric.md)
  merge    validate judge outputs and write decisions.jsonl for build_development_dataset_v2.py

Decision bases in v2:
  wordnet_v2_judgment          every WordNet candidate synset re-judged from scratch under the v2 rubric
  abbreviation_source_scope    abbrcode / se-abbr entries kept whole; the sources' own scope is code naming
  v1_carried                   GitHub Explore / Wikidata catalog decisions reused unchanged from v1
"""
from __future__ import annotations

import argparse
import collections
import json
from pathlib import Path

from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, canonical_json, sha256_file, write_json

PROJECTION = DATA_ROOT / 'development-curation/projection'
V1_DECISIONS = DATA_ROOT / 'development-curation/semantic-adjudication-v2/effective/decisions.jsonl'
AREA = DATA_ROOT / 'development-curation-v2'
RUBRIC = EXPERIMENT_DIR / 'contracts/curation-v2-rubric.md'
WORDNET_PACKETS = ('wordnet-operations', 'wordnet-technical')
ABBREVIATION_SOURCES = ('abbrcode-bd33b1c', 'se-abbr-8e1d77e')
BATCH_BYTES = 40_000


def projection_rows(packet: str):
    with (PROJECTION / f'{packet}.jsonl').open() as stream:
        for line in stream:
            yield json.loads(line)


def compact_synset(row: dict) -> dict:
    synset = row['source_input']['synset']
    return dict(id=row['concept']['concept_id'].rsplit(':', 1)[-1], pos=synset.get('partOfSpeech'),
                lexfile=row['concept'].get('attributes', {}).get('lexfile'),
                definition=' / '.join(synset.get('definition', [])), examples=synset.get('example', [])[:2],
                m=sorted({lex['surface'] for lex in row['lexicalizations']}))


def write_batches() -> None:
    if (AREA / 'batches').exists():
        raise SystemExit('batches already exist; refusing to overwrite')
    synsets = sorted((compact_synset(row) for packet in WORDNET_PACKETS for row in projection_rows(packet)),
                     key=lambda record: record['id'])
    if len({record['id'] for record in synsets}) != len(synsets):
        raise SystemExit('duplicate synset id across WordNet packets')
    batches, current, size = [], [], 0
    for record in synsets:
        line = json.dumps(record, ensure_ascii=False)
        if current and size + len(line.encode()) > BATCH_BYTES:
            batches.append(current)
            current, size = [], 0
        current.append(line)
        size += len(line.encode()) + 1
    batches.append(current)
    (AREA / 'batches').mkdir(parents=True)
    manifest = []
    for number, lines in enumerate(batches, 1):
        path = AREA / 'batches' / f'wordnet-{number:03}.jsonl'
        path.write_text('\n'.join(lines) + '\n')
        manifest.append(dict(batch=path.stem, records=len(lines), bytes=path.stat().st_size, sha256=sha256_file(path)))
    write_json(AREA / 'batches/manifest.json', dict(rubric_sha256=sha256_file(RUBRIC), synsets=len(synsets), batches=manifest))
    print(json.dumps(dict(synsets=len(synsets), batches=len(manifest)), ensure_ascii=False))


def read_judgments(batch: str, expected: list[dict]) -> list[dict]:
    path = AREA / 'judgments' / f'{batch}.jsonl'
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    ids = [row['id'] for row in rows]
    if sorted(ids) != sorted(record['id'] for record in expected) or len(set(ids)) != len(ids):
        raise SystemExit(f'{batch}: judgment ids do not cover the batch exactly once')
    members = {record['id']: set(record['m']) for record in expected}
    for row in rows:
        if set(row) != {'id', 'keep', 'reason'} or not set(row['keep']) <= members[row['id']] or not row['reason'].strip():
            raise SystemExit(f'{batch}: invalid judgment for {row["id"]}')
    return rows


def merge() -> None:
    output = AREA / 'decisions.jsonl'
    if output.exists():
        raise SystemExit('decisions.jsonl already exists; refusing to overwrite')
    manifest = json.loads((AREA / 'batches/manifest.json').read_text())
    if manifest['rubric_sha256'] != sha256_file(RUBRIC):
        raise SystemExit('rubric changed after batches were frozen')
    lexicalizations = {row['concept']['concept_id']: row['lexicalizations']
                       for packet in (*WORDNET_PACKETS, 'abbreviations', 'catalogs') for row in projection_rows(packet)}
    decisions = []
    for entry in manifest['batches']:
        batch_path = AREA / 'batches' / f"{entry['batch']}.jsonl"
        if sha256_file(batch_path) != entry['sha256']:
            raise SystemExit(f"{entry['batch']}: batch file changed")
        expected = [json.loads(line) for line in batch_path.read_text().splitlines()]
        for row in read_judgments(entry['batch'], expected):
            concept_id = f'oewn-2025:synset:{row["id"]}'
            keep = sorted(lex['lexicalization_id'] for lex in lexicalizations[concept_id] if lex['surface'] in row['keep'])
            decisions.append(dict(concept_id=concept_id, decision='include' if keep else 'exclude',
                                  keep_lexicalization_ids=keep, basis='wordnet_v2_judgment', reason=row['reason']))
    for concept_id, lexes in lexicalizations.items():
        if concept_id.split(':', 1)[0] in ABBREVIATION_SOURCES:
            decisions.append(dict(concept_id=concept_id, decision='include',
                                  keep_lexicalization_ids=sorted(lex['lexicalization_id'] for lex in lexes),
                                  basis='abbreviation_source_scope', reason='출처 범위가 코드 식별자 약어'))
    judged = {row['concept_id'] for row in decisions}
    with V1_DECISIONS.open() as stream:
        for line in stream:
            v1 = json.loads(line)
            if v1['concept_id'] not in judged:
                decisions.append(dict(concept_id=v1['concept_id'], decision='include' if v1['decision'] == 'include' else 'exclude',
                                      keep_lexicalization_ids=sorted(v1['keep_lexicalization_ids']) if v1['decision'] == 'include' else [],
                                      basis='v1_carried', reason=v1['development_rationale']))
    decisions.sort(key=lambda row: row['concept_id'])
    if len(decisions) != len(lexicalizations) or len({row['concept_id'] for row in decisions}) != len(decisions):
        raise SystemExit('merged decisions do not cover the candidate concepts exactly once')
    output.write_text(''.join(canonical_json(row) + '\n' for row in decisions))
    summary = collections.Counter((row['basis'], row['decision']) for row in decisions)
    write_json(AREA / 'decisions-summary.json', dict(
        decisions_sha256=sha256_file(output), rubric_sha256=manifest['rubric_sha256'],
        counts={f'{basis}/{decision}': count for (basis, decision), count in sorted(summary.items())},
        kept_lexicalizations=sum(len(row['keep_lexicalization_ids']) for row in decisions)))
    print(json.dumps({f'{b}/{d}': c for (b, d), c in sorted(summary.items())}, ensure_ascii=False))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['batches', 'merge'])
    command = parser.parse_args().command
    write_batches() if command == 'batches' else merge()


if __name__ == '__main__':
    main()
