#!/usr/bin/env python3
"""Development-only freeze/run/validate/review/offline/aggregate, never old publishers.

Validation and label gates do not write, including on failure. Writers allocate exclusive
new directories; failed native executions and partial outputs are retained in place.
"""
from __future__ import annotations

import argparse
import collections
import csv
import json
import os
from pathlib import Path
import random
import re
import shlex
import sqlite3
import subprocess
import sys
import time

import development_profile as dp
# dp verifies approved reused helper/case bytes before these imports.
import evaluate as original
from lifecycle import sandbox_profile
from lookup_handoff import serving_audit
from rtpoc_common import canonical_json, read_json, repo_relative, sha256_bytes

E, ROOT, D, RAW = dp.E, dp.ROOT, dp.D, dp.RAW


def run_paths(profile_path: Path, run_id: str) -> tuple[Path, Path]:
    dp.require(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,63}', run_id) is not None, 'run-id must be a single safe component')
    return RAW / profile_path.parent.name / run_id, D / 'runs' / profile_path.parent.name / run_id


def protocol(profile_path: Path, profile: dict) -> dict:
    path = profile_path.parent / 'evaluation-protocol.json'
    value = read_json(path)
    dp.require(value['profile_identity'] == dp.identity(profile_path), 'protocol/profile mismatch')
    dp.require(value['inputs'] == profile['cases'] and value['method'] == profile['method'], 'protocol cases/method mismatch')
    dp.require(value['code_identities'] == profile['code_identities'] and value['label_reuse'] is False, 'protocol code/label policy mismatch')
    expected = dict(dataset_id=profile['dataset']['dataset_id'], dataset_sqlite_sha256=profile['dataset']['sqlite']['sha256'],
                    normalization_version='rtn-2', policy_id='rtl-1', policy_sha256=profile['policy']['sha256'],
                    binary_sha256=profile['runtime']['binary_identity']['sha256'],
                    index_generation={name: t['index_generation'] for name, t in profile['targets'].items()})
    dp.require(value['frozen_identities'] == expected, 'protocol frozen identities mismatch')
    return value


def command_path(p: str | Path) -> str:
    return os.path.relpath(dp.local(p), E)


def lookup_argv(profile: dict, name: str | None, terms: Path, output: Path, repeat: int, timing: Path | None = None) -> list[str]:
    argv = [command_path(profile['runtime']['binary_identity']['path']), 'lookup', '--dataset', command_path(profile['dataset']['sqlite']['path']),
            '--expect-dataset-id', profile['dataset']['dataset_id'], '--policy', command_path(profile['policy']['path'])]
    if name is not None:
        argv += ['--state', command_path(profile['targets'][name]['state_dir'])]
    argv += ['--repeat', str(repeat), '--terms-file', command_path(terms), '--output', command_path(output)]
    if timing is not None:
        argv += ['--timing-output', command_path(timing)]
    return argv


def execute(argv: list[str], directory: Path, stem: str, measured: bool = False) -> dict:
    actual = ['/usr/bin/time', '-l', *argv] if measured else argv
    print(f'cwd={E}\n{shlex.join(actual)}', flush=True)
    started = time.perf_counter()
    try:
        process = subprocess.run(actual, cwd=E, capture_output=True)
    except OSError as error:
        dp.new_json(directory / f'{stem}-infrastructure-failure.json', dict(argv=actual, cwd=repo_relative(E), error=str(error), infrastructure_failure=True))
        raise
    wall = time.perf_counter() - started
    stdout, stderr = directory / f'{stem}-stdout.txt', directory / f'{stem}-stderr.txt'
    with stdout.open('xb') as f:
        f.write(process.stdout)
    with stderr.open('xb') as f:
        f.write(process.stderr)
    match = re.search(rb'(\d+)\s+maximum resident set size', process.stderr)
    evidence = dict(argv=actual, cwd=repo_relative(E), exit=process.returncode, wall_s=wall,
                    process_max_rss_bytes=int(match[1]) if match else None,
                    stdout_identity=dp.identity(stdout), stderr_identity=dp.identity(stderr), mode='executed')
    dp.new_json(directory / f'{stem}-command.json', evidence)
    dp.require(process.returncode == 0, f'native command failed; exact evidence retained: {stem} exit={process.returncode}')
    if measured:
        dp.require(match is not None, f'native time RSS missing: {stem}')
    return evidence


def native_checks(profile: dict, directory: Path) -> dict:
    binary = command_path(profile['runtime']['binary_identity']['path'])
    evidence = execute([binary, 'normalize-check', '--dataset', command_path(profile['dataset']['sqlite']['path'])], directory, 'normalize')
    normal = read_json(dp.checked(evidence['stdout_identity']))
    dp.require(normal['normalization_version'] == 'rtn-2' and normal['expressions_checked'] == 5965 and normal['surfaces_checked'] == 8445
               and normal['expression_mismatches'] == normal['surface_mismatches'] == 0, 'native normalization mismatch')
    stats = {}
    for name, target in profile['targets'].items():
        command = execute([binary, 'stats-words', '--state', command_path(target['state_dir'])], directory, f'stats-{name}')
        repository = read_json(dp.checked(command['stdout_identity']))['repository']
        dp.require(repository['status'] == 'ready' and repository['published_generation'] == repository['current_generation'] == target['index_generation'], f'{name}: stats not ready')
        for key in ('live_files', 'vocabulary_size', 'field_policy', 'analyzer_policy'):
            dp.require(repository[key] == target[key], f'{name}: native stats mismatch {key}')
        stats[name] = dict(command_identity=dp.identity(directory / f'stats-{name}-command.json'), repository=repository)
    audit = serving_audit()
    dp.require(audit['status'] == 'pass', 'static serving audit failed')
    result = dict(schema='rt-development-native-validation/1', normalization=normal,
                  normalization_command_identity=dp.identity(directory / 'normalize-command.json'), targets=stats,
                  serving_audit=dict(mode='static_executed', result=audit), build='not_run', indexing='not_run')
    dp.new_json(directory / 'native-validation.json', result)
    return result


def output_rows(path: Path) -> tuple[list[str], list[dict]]:
    raw = path.read_bytes()
    dp.require(raw.endswith(b'\n'), f'raw output missing trailing newline: {path}')
    lines = raw.decode('utf-8').splitlines()
    dp.require(all(line for line in lines), f'blank raw output line: {path}')
    return lines, [json.loads(line) for line in lines]


def timing_check(profile: dict, target: str, value: dict, rows: list[dict], repeat: int = 5) -> dict:
    dp.require(value['dataset_id'] == profile['dataset']['dataset_id'] and value['repository_status'] == 'ready'
               and value['build_profile'] == 'release' and value['repeat_outputs_identical'] is True, f'{target}: timing identity/determinism mismatch')
    passes = value['passes']
    dp.require(len(passes) == repeat and [p['pass'] for p in passes] == list(range(repeat)), f'{target}: timing pass coverage mismatch')
    for i, item in enumerate(passes):
        dp.require(item['lookups'] == 92 and len(item['per_lookup_us']) == 92, f'{target}: timing input coverage mismatch')
        dp.require(item['condition'] == ('first_in_process' if i == 0 else 'repeat_in_process'), f'{target}: pass condition mismatch')
        dp.require(all(isinstance(x, int) and x >= 0 for x in item['per_lookup_us']), f'{target}: invalid native timing')
    dp.require([row['lookup_us'] for row in rows] == passes[0]['per_lookup_us'], f'{target}: raw/timing equality mismatch')
    first = [x / 1000 for x in passes[0]['per_lookup_us']]
    repeats = [x / 1000 for p in passes[1:] for x in p['per_lookup_us']]
    def stats(values):
        return dict(n=len(values), median=original.percentile(values, 0.5), p90=original.percentile(values, 0.9), max=max(values))
    return dict(repeat_outputs_identical=True, first_pass_ms=stats(first), repeat_passes_ms=stats(repeats),
                process_start_to_ready_ms=value['process_start_to_ready_us'] / 1000,
                dataset_open_ms=value['dataset_open_us'] / 1000, stats_open_ms=value['stats_open_us'] / 1000)


def observation_check(cases: list[dict], outputs: dict, lines: dict, observations: list[dict]) -> None:
    expected = [original.compact_observation(case, name, output, len(line.encode()))
                for name, rows in outputs.items() for case, output, line in zip(cases, rows, lines[name])]
    expected_keys = {(case['case_id'], name) for case in cases for name in outputs}
    keys = [(row['case_id'], row['target']) for row in observations]
    dp.require(len(keys) == len(expected_keys) == 368 and set(keys) == expected_keys, 'missing/duplicate/extra case×target observations')
    dp.require(observations == expected, 'observations differ from raw outputs (including output_bytes/order)')


def run(profile_path: Path, profile: dict, run_id: str) -> None:
    frozen = protocol(profile_path, profile)
    before = dp.snapshot(profile_path, profile)
    raw, compact = run_paths(profile_path, run_id)
    dp.require(not raw.exists() and not compact.exists(), 'run directory collision; select a new run-id')
    raw.mkdir(parents=True, exist_ok=False)
    compact.mkdir(parents=True, exist_ok=False)
    try:
        dp.new_json(raw / 'identities-before.json', before)
        native_checks(profile, raw)
        cases = original.load_cases()
        terms = raw / 'terms.txt'
        with terms.open('x', encoding='utf-8') as f:
            f.write(''.join(c['input'] + '\n' for c in cases))
        outputs, lines, timings, lookup, commands = {}, {}, {}, {}, []
        for name in profile['targets']:
            output, timing = raw / f'{name}.jsonl', raw / f'{name}-timing.json'
            dp.require(not output.exists() and not timing.exists(), f'{name}: output collision')
            command = execute(lookup_argv(profile, name, terms, output, 5, timing), raw, f'lookup-{name}', measured=True)
            commands.append(dp.identity(raw / f'lookup-{name}-command.json'))
            lines[name], outputs[name] = output_rows(output)
            original.validate_outputs(frozen, name, outputs[name])
            timings[name] = read_json(timing)
            summary = timing_check(profile, name, timings[name], outputs[name])
            lookup[name] = summary | dict(process_wall_s=command['wall_s'], process_max_rss_bytes=command['process_max_rss_bytes'],
                                         output_bytes=dict(median=original.percentile([len(l.encode()) for l in lines[name]], 0.5), max=max(len(l.encode()) for l in lines[name])))
        observations = [original.compact_observation(case, name, output, len(line.encode()))
                        for name in profile['targets'] for case, output, line in zip(cases, outputs[name], lines[name])]
        observation_check(cases, outputs, lines, observations)
        observations_path = raw / 'observations.jsonl'
        with observations_path.open('x', encoding='utf-8') as f:
            for observation in observations:
                f.write(canonical_json(observation) + '\n')
        dp.validate(profile_path)
        after = dp.snapshot(profile_path, profile)
        dp.require(before == after, 'artifact hash/stat changed during measurement')
        dp.new_json(raw / 'identities-after.json', after)
        manifest = dict(schema='rt-development-measurement/1', profile_identity=dp.identity(profile_path), run_id=run_id,
                        protocol_identity=dp.identity(profile_path.parent / 'evaluation-protocol.json'), terms_identity=dp.identity(terms),
                        observations_identity=dp.identity(observations_path), native_validation_identity=dp.identity(raw / 'native-validation.json'),
                        snapshots=[dp.identity(raw / 'identities-before.json'), dp.identity(raw / 'identities-after.json')],
                        raw={name: dp.identity(raw / f'{name}.jsonl') for name in profile['targets']},
                        timing={name: dp.identity(raw / f'{name}-timing.json') for name in profile['targets']}, commands=commands,
                        coverage=dict(unique_inputs=92, targets=4, observations=368, repetitions=5, internal_lookups=1840),
                        dataset_build=dict(status='not_run', wall_s=None, max_rss_bytes=None), indexing='not_run', lookup=lookup)
        dp.new_json(raw / 'measurement-manifest.json', manifest)
        dp.new_json(compact / 'measurement-handoff.json', dict(schema='rt-development-measurement-handoff/1', profile_identity=dp.identity(profile_path),
                    run_id=run_id, measurement_manifest_identity=dp.identity(raw / 'measurement-manifest.json'), coverage=manifest['coverage'],
                    dataset_build=manifest['dataset_build'], lookup=lookup, semantic_status='pending_fresh_labels', offline='pending'))
        print(canonical_json(dict(run_id=run_id, profile=profile_path.parent.name, coverage=manifest['coverage'], build='not_run')))
    except (Exception, SystemExit) as error:
        # This is a writer command only. Read-only validation never reaches this checkpoint.
        dp.new_json(raw / 'failure.json', dict(schema='rt-development-failure/1', command='run', run_id=run_id,
                    profile_identity=dp.identity(profile_path), error_type=type(error).__name__, error=str(error),
                    partial_outputs_preserved=True, retry=False))
        raise


def load_run(profile_path: Path, profile: dict, run_id: str) -> tuple[Path, Path, dict, dict, dict, list[dict]]:
    raw, compact = run_paths(profile_path, run_id)
    handoff = read_json(compact / 'measurement-handoff.json')
    dp.require(handoff['profile_identity'] == dp.identity(profile_path) and handoff['run_id'] == run_id, 'measurement handoff identity mismatch')
    manifest = read_json(dp.checked(handoff['measurement_manifest_identity']))
    dp.require(manifest['profile_identity'] == dp.identity(profile_path) and manifest['run_id'] == run_id, 'measurement belongs to another profile/run')
    dp.require(manifest['protocol_identity'] == dp.identity(profile_path.parent / 'evaluation-protocol.json'), 'measurement protocol mismatch')
    frozen = protocol(profile_path, profile)
    for binding in dp.all_identities(manifest):
        dp.checked(binding)
    cases = original.load_cases()
    dp.require(dp.checked(manifest['terms_identity']).read_text() == ''.join(c['input'] + '\n' for c in cases), 'measurement terms differ from frozen cases')
    dp.require(set(manifest['raw']) == set(manifest['timing']) == set(profile['targets']), 'measurement target set mismatch')
    outputs, lines = {}, {}
    for name in profile['targets']:
        lines[name], outputs[name] = output_rows(dp.checked(manifest['raw'][name]))
        original.validate_outputs(frozen, name, outputs[name])
        timing_check(profile, name, read_json(dp.checked(manifest['timing'][name])), outputs[name])
    observations = [json.loads(line) for line in dp.checked(manifest['observations_identity']).read_text().splitlines()]
    observation_check(cases, outputs, lines, observations)
    recorded_before, recorded_after = (read_json(dp.checked(binding)) for binding in manifest['snapshots'])
    dp.require(recorded_before == recorded_after, 'recorded before/after artifacts differ')
    dp.require(dp.snapshot(profile_path, profile) == recorded_after, 'actual artifact hash/stat differs from measured run')
    return raw, compact, manifest, outputs, lines, observations


def review_sets(profile_path: Path, outputs: dict) -> tuple[dict, list, list, list]:
    merged = original.candidate_index({}, outputs=outputs)
    final = sorted(key for key, entry in merged.items() if any(outcome.startswith('final') for outcome in entry['outcomes'].values()))
    final_set = set(final)
    loss = sorted(key for key, entry in merged.items() if key not in final_set and any(not outcome.startswith('normalization') for outcome in entry['outcomes'].values()))
    sample = sorted(random.Random(original.REVIEW_SAMPLE_SEED).sample(loss, min(original.FILTER_LOSS_SAMPLE_MAX, len(loss))))
    return merged, final, loss, sample


def readable_packet(directory: Path, name: str, value: object) -> dict:
    """Lossless pretty JSON fragmented at line boundaries: <=120 lines and <=24KiB per read."""
    directory.mkdir(parents=True, exist_ok=False)
    text = json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n'
    lines = text.splitlines(keepends=True)
    dp.require(all(len(line.encode()) <= 4096 for line in lines), 'packet has over-cap primitive line')
    chunks, current, size = [], [], 0
    for line in lines:
        n = len(line.encode())
        if current and (len(current) == 120 or size + n > 24576):
            chunks.append(''.join(current))
            current, size = [], 0
        current.append(line)
        size += n
    if current:
        chunks.append(''.join(current))
    parts, start_line, start_byte = [], 1, 0
    for index, chunk in enumerate(chunks, 1):
        path = directory / f'part-{index:04d}.json.txt'
        with path.open('x', encoding='utf-8') as f:
            f.write(chunk)
        line_count, byte_count = len(chunk.splitlines()), len(chunk.encode())
        parts.append(dict(identity=dp.identity(path), source_line_start=start_line, source_line_end=start_line + line_count - 1,
                          source_byte_start=start_byte, source_byte_end_exclusive=start_byte + byte_count,
                          read=dict(offset=1, limit=line_count)))
        start_line += line_count
        start_byte += byte_count
    index = directory / 'packet-index.json'
    dp.new_json(index, dict(schema='rt-development-readable-packet/1', name=name, format='UTF-8 pretty JSON; concatenate ALL parts in order before parsing',
                           source_sha256=sha256_bytes(text.encode()), source_bytes=len(text.encode()), source_lines=len(lines),
                           index_reading=dict(offset=1, limit=120, continuation='Increase offset by120 until EOF; every ordered part is mandatory'), parts=parts))
    return dp.identity(index)


def read_packet(binding: dict) -> object:
    manifest = read_json(dp.checked(binding))
    chunks, expected_line, expected_byte = [], 1, 0
    for part in manifest['parts']:
        text = dp.checked(part['identity']).read_text()
        dp.require(part['source_line_start'] == expected_line and part['source_byte_start'] == expected_byte, 'packet continuation gap/overlap')
        dp.require(part['source_line_end'] == expected_line + len(text.splitlines()) - 1, 'packet line coverage mismatch')
        dp.require(part['source_byte_end_exclusive'] == expected_byte + len(text.encode()), 'packet byte coverage mismatch')
        dp.require(len(text.splitlines()) <= 120 and len(text.encode()) <= 24576, 'packet read cap mismatch')
        chunks.append(text)
        expected_line, expected_byte = part['source_line_end'] + 1, part['source_byte_end_exclusive']
    text = ''.join(chunks)
    dp.require(sha256_bytes(text.encode()) == manifest['source_sha256'] and expected_byte == manifest['source_bytes']
               and expected_line - 1 == manifest['source_lines'], 'packet lossless reconstruction mismatch')
    return json.loads(text)


def pair_packet(key: tuple, entry: dict, review_id: str, population: str, profile_path: Path, manifest: dict,
                outputs: dict, observation_packets: dict) -> dict:
    case = entry['case']
    position = original.load_cases().index(case)
    target_candidates = {name: next((c for c in rows[position]['candidates'] if c['exact_key'] == key[1]), None) for name, rows in outputs.items()}
    return dict(schema='rt-development-review-source/1', review_id=review_id, population=population,
                profile_identity=dp.identity(profile_path), run_id=manifest['run_id'],
                case={k: case[k] for k in ('case_id', 'input', 'intended_meaning', 'group', 'category', 'expected_any')},
                candidate=entry['candidate'], outcome_by_target=entry['outcomes'], full_candidates_by_target=target_candidates,
                raw_sources={name: dict(identity=manifest['raw'][name], line=position + 1,
                                       lossless_observation_packet=observation_packets[(name, case['case_id'])]) for name in outputs},
                instruction='Judge intended_meaning with ALL recorded source/license/gloss/concept/lexicalization evidence. expected_any is not gold. No historical labels or guessed defaults. Read every continuation part.')


def review_template(profile_path: Path, profile: dict, run_id: str) -> None:
    raw, compact, manifest, outputs, lines, observations = load_run(profile_path, profile, run_id)
    merged, final, loss, sample = review_sets(profile_path, outputs)
    review = raw / 'review'
    dp.require(not review.exists(), 'review directory collision; blank queue is immutable')
    review.mkdir()
    cases = original.load_cases()
    observation_packets = {}
    for name, rows in outputs.items():
        for case, output in zip(cases, rows):
            observation_packets[(name, case['case_id'])] = readable_packet(review / 'observations' / name / case['case_id'], f'{name}/{case["case_id"]}', output)
    rows, pairs = [], []
    for population, keys in (('final_pairs', final), ('filter_loss_sample', sample)):
        for key in keys:
            entry = merged[key]
            case, candidate = entry['case'], entry['candidate']
            evidence = [e for e in candidate['evidence'] if not e['is_policy_excluded']] or candidate['evidence']
            glosses = []
            for item in evidence:
                text = f"{item['relation_kind']}@{item['source_id']}: {(item['to_gloss'] or item['gloss'] or '')[:90]}"
                if text not in glosses:
                    glosses.append(text)
            review_id = f"{'F' if population == 'final_pairs' else 'L'}{len(rows) + 1:03d}"
            row = dict(review_id=review_id, population=population, case_id=case['case_id'], group=case['group'], category=case['category'],
                       input=case['input'], intended_meaning=case['intended_meaning'], candidate=key[1], primary_kind=candidate['primary_kind'] or '',
                       relation_kinds=','.join(candidate['relation_kinds']), sources=','.join(sorted({e['source_id'] for e in evidence})),
                       evidence_glosses=' || '.join(glosses[:3]), source_records=','.join(sorted({f"{e['from_source_record']}>{e['to_source_record']}" for e in evidence})[:3]),
                       outcome_by_target=';'.join(f'{t}={o}' for t, o in sorted(entry['outcomes'].items())), label='', rationale='')
            rows.append(row)
            packet = readable_packet(review / 'pairs' / review_id, review_id,
                                     pair_packet(key, entry, review_id, population, profile_path, manifest, outputs, observation_packets))
            pairs.append(dict(review_id=review_id, population=population, case_id=key[0], candidate=key[1], packet_identity=packet))
    table = review / 'blank-review.tsv'
    with table.open('x', encoding='utf-8', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=original.REVIEW_COLUMNS, delimiter='\t', lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)
    rubric = profile['method']['rubric'] | {'schema': 'rt-development-review-rubric/1', 'reviewer': 'future fresh native read-only annotator; no labels supplied',
                                         'historical_labels': 'forbidden', 'agent_labels_are_human_gold': False}
    dp.new_json(review / 'rubric.json', rubric)
    submission_schema = dict(schema='rt-development-label-submission/1', serialization='one JSON object per line', required_fields=['profile_sha256', 'run_id', 'queue_sha256', 'review_id', 'population', 'case_id', 'candidate', 'label', 'rationale'],
                             labels=original.LABELS, coverage='Exactly the final census plus seeded loss sample: no blank/duplicate/missing/extra/unbound rows. Labels/rationales must be nonempty.',
                             identity_values=dict(profile_sha256=profile_path.parent.name, run_id=run_id, queue_sha256='SHA-256 of queue-manifest.json'),
                             destination='review/submissions/<new-name>.jsonl; read-only labelers return submissions to the parent writer, not modify blank materials')
    dp.new_json(review / 'submission-schema.json', submission_schema)
    assignments = []
    for i in range(0, len(pairs), 20):
        assignments.append(readable_packet(review / 'assignments' / f'batch-{i // 20 + 1:03d}', f'batch-{i // 20 + 1:03d}',
                           dict(profile_identity=dp.identity(profile_path), run_id=run_id, pairs=pairs[i:i + 20], rubric_identity=dp.identity(review / 'rubric.json'),
                                instruction='Read packet-index with offsets, then every mandatory part; no writes, curation, historical labels, or resampling. Return one bound row per assigned pair.')))
    queue = dict(schema='rt-development-review-queue/1', profile_identity=dp.identity(profile_path), run_id=run_id,
                 measurement_manifest_identity=dp.identity(raw / 'measurement-manifest.json'), raw_identities=manifest['raw'],
                 ordered_final_keys=[list(key) for key in final], ordered_loss_population=[list(key) for key in loss], ordered_loss_sample=[list(key) for key in sample],
                 seed=original.REVIEW_SAMPLE_SEED, sample_max=original.FILTER_LOSS_SAMPLE_MAX,
                 interval_condition=dict(loss_population_coverage='census' if len(sample) == len(loss) else 'uniform_partial_sample',
                                         ci95='null' if len(sample) == len(loss) else 'Wilson95_without_FPC'),
                 counts=dict(final_pairs=len(final), filter_loss_population=len(loss), filter_loss_sample=len(sample), pending_labels=len(rows)),
                 table_identity=dp.identity(table), rubric_identity=dp.identity(review / 'rubric.json'), submission_schema_identity=dp.identity(review / 'submission-schema.json'),
                 pairs=pairs, assignments=assignments,
                 observation_packets=[dict(target=name, case_id=case_id, identity=binding) for (name, case_id), binding in observation_packets.items()],
                 reading=dict(offset=1, limit=120, continuation='Continue queue-manifest with offsets +120 to EOF. Assignment and pair/observation packet indexes enumerate all lossless continuations.'),
                 semantic_status='pending_fresh_labels', labels_reused=False)
    dp.new_json(review / 'queue-manifest.json', queue)
    validate_queue(profile_path, profile, run_id, outputs, manifest)
    dp.new_json(compact / 'evaluation-handoff.json', dict(schema='rt-development-evaluation-handoff/1', profile_identity=dp.identity(profile_path), run_id=run_id,
                measurement_manifest_identity=dp.identity(raw / 'measurement-manifest.json'), queue_manifest_identity=dp.identity(review / 'queue-manifest.json'),
                technical_status='measurement_and_queue_pass; fresh offline gate separate', semantic_status='pending_fresh_labels', pending_label_count=len(rows),
                limits=['Agent annotations are not human gold. Semantic metrics not computed.', 'Five repetitions are not independent semantic observations.']))
    report = technical_report(profile_path, profile, run_id, observations, merged)
    dp.new_json(compact / 'technical-report.json', report)
    print(canonical_json(queue['counts']))


def validate_queue(profile_path: Path, profile: dict, run_id: str, outputs: dict, manifest: dict) -> dict:
    raw, compact = run_paths(profile_path, run_id)
    queue = read_json(raw / 'review/queue-manifest.json')
    dp.require(queue['profile_identity'] == dp.identity(profile_path) and queue['run_id'] == run_id, 'review queue profile/run mismatch')
    dp.require(queue['measurement_manifest_identity'] == dp.identity(raw / 'measurement-manifest.json') and queue['raw_identities'] == manifest['raw'], 'queue/raw binding mismatch')
    merged, final, loss, sample = review_sets(profile_path, outputs)
    dp.require(queue['ordered_final_keys'] == [list(k) for k in final] and queue['ordered_loss_population'] == [list(k) for k in loss]
               and queue['ordered_loss_sample'] == [list(k) for k in sample], 'final/loss population or uniform sample differs')
    dp.require(queue['seed'] == original.REVIEW_SAMPLE_SEED and queue['sample_max'] == original.FILTER_LOSS_SAMPLE_MAX, 'review sampling method mismatch')
    dp.require(queue['interval_condition'] == dict(loss_population_coverage='census' if len(sample) == len(loss) else 'uniform_partial_sample',
                                                 ci95='null' if len(sample) == len(loss) else 'Wilson95_without_FPC'), 'loss CI/census condition mismatch')
    for binding in (queue['table_identity'], queue['rubric_identity'], queue['submission_schema_identity']):
        dp.checked(binding)
    with dp.checked(queue['table_identity']).open(newline='', encoding='utf-8') as f:
        reader = csv.DictReader(f, delimiter='\t')
        dp.require(reader.fieldnames == original.REVIEW_COLUMNS, 'review columns changed')
        rows = list(reader)
    expected = [(population, key) for population, keys in (('final_pairs', final), ('filter_loss_sample', sample)) for key in keys]
    dp.require(len(rows) == len(queue['pairs']) == len(expected), 'blank review coverage mismatch')
    dp.require(queue['counts'] == dict(final_pairs=len(final), filter_loss_population=len(loss), filter_loss_sample=len(sample), pending_labels=len(expected)), 'queue counts mismatch')
    observation_packets = {(r['target'], r['case_id']): r['identity'] for r in queue['observation_packets']}
    cases = original.load_cases()
    dp.require(len(observation_packets) == len(queue['observation_packets']) == 368, 'observation packet coverage mismatch')
    for name, native_rows in outputs.items():
        for case, output in zip(cases, native_rows):
            dp.require(read_packet(observation_packets[(name, case['case_id'])]) == output, 'lossless observation packet differs from native output')
    for i, ((population, key), row, pair) in enumerate(zip(expected, rows, queue['pairs']), 1):
        review_id = f"{'F' if population == 'final_pairs' else 'L'}{i:03d}"
        dp.require(row['review_id'] == pair['review_id'] == review_id and row['population'] == pair['population'] == population, 'review assignment mismatch')
        dp.require((row['case_id'], row['candidate']) == (pair['case_id'], pair['candidate']) == key, 'review key mismatch')
        dp.require(row['label'] == row['rationale'] == '', 'blank review contains labels or rationales')
        case = merged[key]['case']
        for field in ('input', 'intended_meaning', 'group', 'category'):
            dp.require(row[field] == case[field], f'review case metadata mismatch: {field}')
        expected_packet = pair_packet(key, merged[key], review_id, population, profile_path, manifest, outputs, observation_packets)
        dp.require(read_packet(pair['packet_identity']) == expected_packet, 'review evidence packet differs from fresh raw source')
    assigned = []
    for assignment in queue['assignments']:
        packet = read_packet(assignment)
        dp.require(packet['profile_identity'] == dp.identity(profile_path) and packet['run_id'] == run_id, 'assignment profile/run mismatch')
        assigned.extend(packet['pairs'])
    dp.require(assigned == queue['pairs'], 'assignments do not cover queue exactly once in order')
    if (compact / 'evaluation-handoff.json').exists():
        handoff = read_json(compact / 'evaluation-handoff.json')
        dp.require(handoff['queue_manifest_identity'] == dp.identity(raw / 'review/queue-manifest.json'), 'published queue identity changed')
    return queue


def offline_check(profile_path: Path, profile: dict, run_id: str) -> None:
    raw, compact, manifest, outputs, lines, observations = load_run(profile_path, profile, run_id)
    directory = raw / 'offline'
    dp.require(not directory.exists() and not (compact / 'offline-handoff.json').exists(), 'offline output collision')
    directory.mkdir()
    before = dp.snapshot(profile_path, profile)
    workspace = read_json(dp.checked(profile['workspaces_manifest_identity']))
    sbpl = sandbox_profile([dp.local(workspace['checkout_root'])], [dp.local(t['state_dir']) / 'index' for t in profile['targets'].values()])
    with (directory / 'sandbox.sbpl').open('x') as f:
        f.write(sbpl + '\n')
    commands = []
    try:
        for name in profile['targets']:
            output, timing = directory / f'{name}.jsonl', directory / f'{name}-timing.json'
            argv = ['/usr/bin/sandbox-exec', '-f', command_path(directory / 'sandbox.sbpl'),
                    *lookup_argv(profile, name, dp.checked(manifest['terms_identity']), output, 2, timing)]
            commands.append(execute(argv, directory, name))
            sandbox_lines, sandbox_rows = output_rows(output)
            original.validate_outputs(protocol(profile_path, profile), name, sandbox_rows)
            timing_check(profile, name, read_json(timing), sandbox_rows, repeat=2)
            def without_timing(rows):
                return [{k: v for k, v in row.items() if k != 'lookup_us'} for row in rows]
            dp.require(without_timing(sandbox_rows) == without_timing(outputs[name]), f'{name}: offline/native output differs except lookup_us')
        dp.validate(profile_path)
        after = dp.snapshot(profile_path, profile)
        dp.require(before == after, 'artifacts changed during offline check')
        dp.new_json(directory / 'offline-evidence.json', dict(schema='rt-development-offline/1', profile_identity=dp.identity(profile_path), run_id=run_id,
                    measurement_manifest_identity=dp.identity(raw / 'measurement-manifest.json'), native_raw_identities=manifest['raw'], status='pass',
                    method='Executed compatible sandbox: deny network*, checkout reads and index segment reads; allow index/meta.json. Fresh92×4 two-repeat outputs equal native except lookup_us.',
                    sandbox_identity=dp.identity(directory / 'sandbox.sbpl'), commands=commands, snapshots=dict(before=before, after=after),
                    raw={name: dp.identity(directory / f'{name}.jsonl') for name in outputs},
                    timing={name: dp.identity(directory / f'{name}-timing.json') for name in outputs}, negative_index_commands='not_run; no new fixtures'))
        dp.new_json(compact / 'offline-handoff.json', dict(schema='rt-development-offline-handoff/1', profile_identity=dp.identity(profile_path), run_id=run_id,
                    evidence_identity=dp.identity(directory / 'offline-evidence.json'), status='pass', mode='executed'))
        print('fresh offline check: pass')
    except (Exception, SystemExit) as error:
        dp.new_json(directory / 'failure.json', dict(error_type=type(error).__name__, error=str(error), commands=commands, partial_outputs_preserved=True, retry=False))
        raise


def validate_offline(profile_path: Path, profile: dict, run_id: str, manifest: dict, outputs: dict) -> dict | None:
    raw, compact = run_paths(profile_path, run_id)
    handoff_path = compact / 'offline-handoff.json'
    if not handoff_path.exists():
        return None
    handoff = read_json(handoff_path)
    evidence = read_json(dp.checked(handoff['evidence_identity']))
    dp.require(evidence['status'] == handoff['status'] == 'pass' and evidence['profile_identity'] == handoff['profile_identity'] == dp.identity(profile_path)
               and evidence['run_id'] == handoff['run_id'] == run_id, 'offline identity mismatch')
    dp.require(evidence['measurement_manifest_identity'] == dp.identity(raw / 'measurement-manifest.json') and evidence['native_raw_identities'] == manifest['raw'], 'offline evidence belongs to another native run')
    dp.require(evidence['snapshots']['before'] == evidence['snapshots']['after'] == dp.snapshot(profile_path, profile), 'offline artifact snapshots differ')
    dp.require(set(evidence['raw']) == set(evidence['timing']) == set(outputs), 'offline target coverage mismatch')
    for binding in dp.all_identities(evidence):
        dp.checked(binding)
    for name in outputs:
        _, rows = output_rows(dp.checked(evidence['raw'][name]))
        original.validate_outputs(protocol(profile_path, profile), name, rows)
        timing_check(profile, name, read_json(dp.checked(evidence['timing'][name])), rows, 2)
        dp.require([{k: v for k, v in r.items() if k != 'lookup_us'} for r in rows] == [{k: v for k, v in r.items() if k != 'lookup_us'} for r in outputs[name]], 'offline output equality mismatch')
    return evidence


def technical_report(profile_path: Path, profile: dict, run_id: str, observations: list, merged: dict) -> dict:
    raw, compact = run_paths(profile_path, run_id)
    coverage = {}
    case_map = {c['case_id']: c for c in original.load_cases()}
    for group in ('all', 'control', 'category_probe', 'random_probe'):
        rows = [o for o in observations if group == 'all' or o['group'] == group]
        unique = list({o['case_id']: o for o in rows}.values())
        expected_unique = [o for o in unique if case_map[o['case_id']]['expected_any']]
        with_expected = [o for o in rows if case_map[o['case_id']]['expected_any']]
        def found(o, final):
            expected = {original.normalize_term(t) for t in case_map[o['case_id']]['expected_any']}
            terms = ({original.normalize_term(f['candidate']) for f in o['final']} if final else
                     {original.normalize_term(key[1]) for key in merged if key[0] == o['case_id']})
            return bool(expected & terms)
        coverage[group] = dict(unique_inputs=len(unique), observations=len(rows), statuses=dict(collections.Counter(o['status'] for o in rows)),
                               dictionary_coverage=original.observed_proportion(sum(o['status'] != 'no_dictionary_entry' for o in unique), len(unique)),
                               nonempty_final=original.observed_proportion(sum(bool(o['final']) for o in rows), len(rows)),
                               expected_found_in_dictionary=original.observed_proportion(sum(found(o, False) for o in expected_unique), len(expected_unique)),
                               expected_found_in_final=original.observed_proportion(sum(found(o, True) for o in with_expected), len(with_expected)))
    attrition = {}
    for name in profile['targets']:
        stages, reasons = collections.Counter(), collections.Counter()
        for row in observations:
            if row['target'] == name:
                stages.update(row['stage_counts'])
                reasons.update(row['rejections'])
        attrition[name] = dict(stage_totals=dict(stages), rejection_reasons=dict(reasons))
    return dict(schema='rt-development-technical-report/1', profile_identity=dp.identity(profile_path), run_id=run_id,
                coverage=coverage, attrition=attrition, semantic_status='pending_fresh_labels', semantic_metrics=None,
                expected_any='Frozen expectations, not gold synonyms. Presence only, not correctness.',
                ci_rule='Unique-input dictionary/expected-dictionary denominator; input×target returns descriptive with ci95=null. Full loss-population census has ci95=null; only actual uniform partial loss-label samples use Wilson95, no FPC.',
                build='not_run', accuracy_claim=False)


def load_labels(profile_path: Path, run_id: str, labels_file: str, queue: dict) -> dict:
    raw, _ = run_paths(profile_path, run_id)
    path = dp.local(labels_file)
    dp.require(path.is_relative_to(raw / 'review/submissions') and path.suffix == '.jsonl', 'labels must be a new explicit run review/submissions/*.jsonl file; no historical label paths')
    binding = dp.identity(raw / 'review/queue-manifest.json')
    expected = {(p['case_id'], p['candidate']): p for p in queue['pairs']}
    indexed = {}
    for line in path.read_text().splitlines():
        row = json.loads(line)
        key = (row['case_id'], row['candidate'])
        dp.require(key in expected and key not in indexed, 'duplicate/extra label pair')
        pair = expected[key]
        dp.require(row['profile_sha256'] == profile_path.parent.name and row['run_id'] == run_id and row['queue_sha256'] == binding['sha256'], 'unbound label submission')
        dp.require(row['review_id'] == pair['review_id'] and row['population'] == pair['population'], 'label F/L population mismatch')
        dp.require(row['label'] in original.LABELS and isinstance(row['rationale'], str) and row['rationale'].strip(), 'blank/invalid label or rationale')
        indexed[key] = row
    dp.require(set(indexed) == set(expected), 'missing labels: exact final census AND seeded loss sample required')
    return indexed


def semantic_metrics(cases: list, observations: list, merged: dict, labels: dict) -> dict:
    """Original inspected formulas, separate from I/O; no inference for absent labels."""
    per_observation = []
    by_id = {c['case_id']: c for c in cases}
    for o in observations:
        values = [labels[(o['case_id'], f['candidate'])]['label'] for f in o['final']]
        expected = {original.normalize_term(t) for t in by_id[o['case_id']]['expected_any']}
        dictionary = {original.normalize_term(k[1]) for k in merged if k[0] == o['case_id']}
        final = {original.normalize_term(f['candidate']) for f in o['final']}
        per_observation.append(dict(case_id=o['case_id'], group=o['group'], category=o['category'], target=o['target'], status=o['status'],
             repository_status=o['repository_status'], final_count=len(values), has_synonym='synonym_in_intended_sense' in values,
             has_synonymish=any(v in original.SYNONYMISH for v in values), has_useful=any(v in original.USEFUL for v in values),
             alias_or_abbreviation_only=any(v in original.USEFUL for v in values) and not any(v in original.SYNONYMISH for v in values),
             nonempty_without_useful=bool(values) and not any(v in original.USEFUL for v in values), has_wrong_sense='wrong_sense' in values,
             expected_defined=bool(expected), expected_in_dictionary=bool(expected & dictionary), expected_in_final=bool(expected & final)))
    def summarize(rows):
        unique = list({r['case_id']: r for r in rows}.values())
        expected_inputs = [r for r in unique if r['expected_defined']]
        with_expected = [r for r in rows if r['expected_defined']]
        summary = dict(observations=len(rows), unique_inputs=len(unique), status=dict(collections.Counter(r['status'] for r in rows)),
                       dictionary_coverage=original.observed_proportion(sum(r['status'] != 'no_dictionary_entry' for r in unique), len(unique)),
                       nonempty_final=original.observed_proportion(sum(r['final_count'] > 0 for r in rows), len(rows)),
                       abstention_no_final=original.observed_proportion(sum(r['final_count'] == 0 for r in rows), len(rows)),
                       technical_unavailable=sum(r['repository_status'] != 'ready' for r in rows),
                       expected_found_in_dictionary=original.observed_proportion(sum(r['expected_in_dictionary'] for r in expected_inputs), len(expected_inputs)),
                       expected_found_in_final=original.observed_proportion(sum(r['expected_in_final'] for r in with_expected), len(with_expected)))
        for metric, flag in dict(supported_synonym_return_strict='has_synonym', supported_synonym_return_incl_near='has_synonymish',
                                useful_return='has_useful', alias_or_abbreviation_only='alias_or_abbreviation_only',
                                nonempty_without_useful='nonempty_without_useful', any_wrong_sense_in_final='has_wrong_sense').items():
            summary[metric] = original.observed_proportion(sum(r[flag] for r in rows), len(rows))
        return summary
    def label_table(items):
        counts = collections.Counter(items)
        total = collections.Counter(key for key, label in items)
        return {key: dict(n=total[key], labels={l: counts[(key, l)] for l in original.LABELS if counts[(key, l)]},
                         synonym_or_near=original.observed_proportion(sum(counts[(key, l)] for l in original.SYNONYMISH), total[key]),
                         useful=original.observed_proportion(sum(counts[(key, l)] for l in original.USEFUL), total[key]),
                         wrong_sense=original.observed_proportion(counts[(key, 'wrong_sense')], total[key])) for key in sorted(total)}
    final_keys = sorted({(o['case_id'], f['candidate']) for o in observations for f in o['final']})
    pooled = [(o, f) for o in observations for f in o['final']]
    unique = [(key, labels[key], merged[key]) for key in final_keys]
    loss = [row for row in labels.values() if row['population'] == 'filter_loss_sample']
    loss_population = sum(key not in set(final_keys) and any(not value.startswith('normalization') for value in entry['outcomes'].values()) for key, entry in merged.items())
    useful = sum(r['label'] in original.USEFUL for r in loss)
    is_loss_census = len(loss) == loss_population
    loss_proportion = original.observed_proportion if is_loss_census else original.wilson
    return dict(
        input_level={group: summarize([r for r in per_observation if group == 'all' or r['group'] == group]) for group in ('all', 'control', 'category_probe', 'random_probe')},
        input_level_by_target={name: summarize([r for r in per_observation if r['target'] == name]) for name in sorted({r['target'] for r in per_observation})},
        input_level_by_category={category: summarize([r for r in per_observation if r['category'] == category]) for category in sorted({r['category'] for r in per_observation})},
        final_candidates_unique=label_table([('all', row['label']) for _, row, _ in unique]).get('all'),
        final_candidates_pooled=label_table([('all', labels[(o['case_id'], f['candidate'])]['label']) for o, f in pooled]).get('all'),
        final_candidates_random_probe_unique=label_table([('random_probe', row['label']) for _, row, entry in unique if entry['case']['group'] == 'random_probe']).get('random_probe'),
        final_by_relation_kind_unique=label_table([(entry['candidate']['primary_kind'] or '', row['label']) for _, row, entry in unique]),
        final_by_source_unique=label_table([(','.join(sorted({e['source_id'] for e in entry['candidate']['evidence'] if not e['is_policy_excluded']})), row['label']) for _, row, entry in unique]),
        final_by_relation_kind_pooled=label_table([(f['primary_kind'], labels[(o['case_id'], f['candidate'])]['label']) for o, f in pooled]),
        filter_loss=dict(population=loss_population, sample=len(loss), labels=dict(collections.Counter(r['label'] for r in loss)),
                         interval_method='none: full loss-population census; ci95=null' if is_loss_census else 'Wilson95 for actual uniform partial filter-loss sample only; no finite-population correction',
                         useful_rate=loss_proportion(useful, len(loss)), synonym_or_near_rate=loss_proportion(sum(r['label'] in original.SYNONYMISH for r in loss), len(loss)),
                         wrong_sense_rate=loss_proportion(sum(r['label'] == 'wrong_sense' for r in loss), len(loss)),
                         estimated_useful_lost=round(useful / len(loss) * loss_population, 1) if loss else None),
        accuracy_claim=False, agent_labels_are_human_gold=False)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['freeze', 'publish-selection', 'run', 'validate', 'review-template', 'offline-check', 'labels-check', 'aggregate'])
    parser.add_argument('--candidate-handoff', default=repo_relative(dp.CANDIDATE))
    parser.add_argument('--profile')
    parser.add_argument('--run-id')
    parser.add_argument('--labels-file')
    parser.add_argument('--report-id')
    parser.add_argument('--replace-selector-sha256', help='explicit parent-controlled compare-and-swap of NEW active.json only')
    args = parser.parse_args()
    if args.command == 'freeze':
        dp.require(dp.local(args.candidate_handoff) == dp.CANDIDATE, 'only the accepted candidate may freeze')
        path, profile = dp.freeze()
        print(canonical_json(dp.identity(path)))
        return 0
    dp.require(args.profile is not None, '--profile required; historical replay never depends on active.json')
    path, profile = dp.validate(args.profile)
    protocol(path, profile)
    if args.command == 'publish-selection':
        # Explicit parent-controlled publication only after actual fresh run/queue/offline gates.
        dp.require(args.run_id is not None, '--run-id required for verified selector publication')
        raw, compact, manifest, outputs, lines, observations = load_run(path, profile, args.run_id)
        validate_queue(path, profile, args.run_id, outputs, manifest)
        dp.require(validate_offline(path, profile, args.run_id, manifest, outputs) is not None, 'fresh offline gate required before selection')
        selector = D / 'active.json'
        body = dict(schema='rt-development-selection/1', profile=dp.identity(path))
        if selector.exists():
            dp.require(args.replace_selector_sha256 is not None and dp.identity(selector)['sha256'] == args.replace_selector_sha256, 'selector changed or explicit replacement not authorized')
            temporary = D / f'.active-{path.parent.name}.json.tmp'
            dp.new_json(temporary, body)
            dp.require(dp.identity(selector)['sha256'] == args.replace_selector_sha256, 'selector changed before atomic replacement')
            os.replace(temporary, selector)
        else:
            dp.require(args.replace_selector_sha256 is None, 'expected selector is missing')
            dp.new_json(selector, body)
        dp.resolve_selector()
        print('development selection published after fresh gates; BROAD baseline unchanged')
        return 0
    if args.command == 'validate' and args.run_id is None:
        print('immutable profile/artifact validation: pass (read-only)')
        return 0
    dp.require(args.run_id is not None, '--run-id required')
    if args.command == 'run':
        run(path, profile, args.run_id)
        return 0
    if args.command == 'review-template':
        review_template(path, profile, args.run_id)
        return 0
    if args.command == 'offline-check':
        offline_check(path, profile, args.run_id)
        return 0
    raw, compact, manifest, outputs, lines, observations = load_run(path, profile, args.run_id)
    queue = validate_queue(path, profile, args.run_id, outputs, manifest)
    offline = validate_offline(path, profile, args.run_id, manifest, outputs)
    if args.command == 'validate':
        print(canonical_json(dict(status='pass', coverage=manifest['coverage'], blank_review=queue['counts'], offline='pass' if offline else 'pending', semantic_metrics=None)))
        return 0
    dp.require(args.labels_file is not None, '--labels-file required; no automatic labels')
    labels = load_labels(path, args.run_id, args.labels_file, queue)
    if args.command == 'labels-check':
        print('labels identity/coverage validation: pass (read-only)')
        return 0
    dp.require(offline is not None, 'fresh offline gate required before semantic aggregation')
    dp.require(args.report_id is not None and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,63}', args.report_id) is not None, 'new --report-id required')
    # All validation and missing-label rejection precede any write.
    metrics = semantic_metrics(original.load_cases(), observations, original.candidate_index({}, outputs=outputs), labels)
    directory = compact / 'reports' / args.report_id
    directory.mkdir(parents=True, exist_ok=False)
    dp.new_json(directory / 'semantic-report.json', dict(schema='rt-development-semantic-report/1', profile_identity=dp.identity(path), run_id=args.run_id,
                queue_identity=dp.identity(raw / 'review/queue-manifest.json'), labels_identity=dp.identity(dp.local(args.labels_file)),
                rubric=profile['method']['rubric'], metrics=metrics, technical_status='pass', annotation_status='fresh_submissions_validated; not human gold'))
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (ValueError, OSError, KeyError, sqlite3.Error) as error:
        print(f'development evaluation stopped: {error}', file=sys.stderr)
        raise SystemExit(1)
