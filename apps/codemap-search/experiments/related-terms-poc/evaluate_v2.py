#!/usr/bin/env python3
"""Development-set measurement for the v2 dataset, rtl-2 policy, and rt-stats/2 state (rtpoc2).

  run        lookup the frozen 92 cases.json inputs on the four dev repositories; write raw outputs and observations
  queue      build the label queue: every final dictionary pair, a seeded filter-loss sample, every morphology pair
  aggregate  semantic metrics from a labels file (same formulas as evaluate_development.py) plus morphology tables

Labels follow the v1 rubric (results/development/.../review/rubric.json) so v1 and v2 share one measuring instrument.
Usage (cwd: repository root):
  python3 -B apps/codemap-search/experiments/related-terms-poc/evaluate_v2.py run --run-id ID --dataset-dir DIR
"""
from __future__ import annotations

import argparse
import collections
import json
import random
import subprocess
from pathlib import Path

import evaluate as original
from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, REPO_ROOT, sha256_file, write_json

TARGETS = ('express', 'flask', 'gin', 'tokio')
BINARY = EXPERIMENT_DIR / 'lookup-v2/target/release/rtpoc2'
POLICY = EXPERIMENT_DIR / 'lookup-v2/contracts/lookup-policy-rtl-2.json'
STATE_ROOT = DATA_ROOT / 'workspaces/state-v2'
RUN_ROOT = DATA_ROOT / 'evaluation/development-v2'
V1_RUBRIC = DATA_ROOT / 'evaluation/development/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/run-1/review/rubric.json'
ASSIGNMENT_PAIRS = 40


def identity(path: Path) -> dict:
    return dict(path=path.relative_to(REPO_ROOT).as_posix(), sha256=sha256_file(path), bytes=path.stat().st_size)


def read_outputs(run: Path) -> dict[str, list[dict]]:
    return {target: [json.loads(line) for line in (run / f'{target}.jsonl').read_text().splitlines()] for target in TARGETS}


def morphology_words(output: dict) -> list[dict]:
    return (output.get('morphology') or {}).get('words') or []


def morphology_summary(output: dict) -> dict:
    morphology = output.get('morphology') or {}
    return dict(status=morphology.get('status'), query_absent_in_repository=morphology.get('query_absent_in_repository'),
                lemma_fallback=(output.get('lemma_fallback') or {}).get('matched'),
                words=[[m['word'], m['type'], m['file_count']] for m in morphology_words(output)])


def run(run_id: str, dataset_dir: Path) -> None:
    run_dir = RUN_ROOT / run_id
    if run_dir.exists():
        raise SystemExit(f'run already exists: {run_dir}')
    run_dir.mkdir(parents=True)
    cases = original.load_cases()
    terms = run_dir / 'terms.txt'
    terms.write_text(''.join(case['input'] + '\n' for case in cases))
    manifest = json.loads((dataset_dir / 'manifest.json').read_text())
    commands = {}
    for target in TARGETS:
        state = STATE_ROOT / target
        argv = [str(BINARY), 'lookup', '--dataset', str(dataset_dir / 'dataset.sqlite3'), '--expect-dataset-id', manifest['dataset_id'],
                '--policy', str(POLICY), '--state', str(state), '--terms-file', str(terms), '--output', str(run_dir / f'{target}.jsonl')]
        result = subprocess.run(argv, cwd=REPO_ROOT, capture_output=True, text=True)
        commands[target] = dict(argv=[str(Path(a).relative_to(REPO_ROOT)) if a.startswith(str(REPO_ROOT)) else a for a in argv],
                                exit_code=result.returncode, stderr=result.stderr[-2000:])
        if result.returncode != 0:
            write_json(run_dir / 'failure.json', commands)
            raise SystemExit(f'lookup failed for {target}: {result.stderr[-500:]}')
    outputs = read_outputs(run_dir)
    observations = []
    for target, rows in outputs.items():
        if len(rows) != len(cases):
            raise SystemExit(f'{target}: {len(rows)} outputs for {len(cases)} cases')
        for case, output in zip(cases, rows):
            if output['input'] != case['input']:
                raise SystemExit(f'{target}: output order mismatch at {case["case_id"]}')
            line = json.dumps(output, ensure_ascii=False, separators=(',', ':'))
            observation = original.compact_observation(case, target, output, len(line.encode()))
            observation['morphology'] = morphology_summary(output)
            observations.append(observation)
    (run_dir / 'observations.jsonl').write_text(''.join(json.dumps(o, ensure_ascii=False) + '\n' for o in observations))
    write_json(run_dir / 'run-manifest.json', dict(
        schema='rt-development-v2-run/1', run_id=run_id, dataset_id=manifest['dataset_id'], dataset=identity(dataset_dir / 'dataset.sqlite3'),
        policy=identity(POLICY), binary=identity(BINARY), cases=identity(original.CASES_PATH),
        stats={t: identity(STATE_ROOT / t / 'published.json') for t in TARGETS}, commands=commands,
        observations=identity(run_dir / 'observations.jsonl')))
    status = collections.Counter(o['status'] for o in observations)
    print(json.dumps(dict(observations=len(observations), status=status,
                          registered_inputs=len({o['input'] for o in observations if o['matches']}),
                          nonempty_final=sum(bool(o['final']) for o in observations),
                          nonempty_morphology=sum(bool(o['morphology']['words']) for o in observations)), ensure_ascii=False))


def pair_record(review_id: str, population: str, case: dict, candidate: str, kind: str, glosses: list[str], sources: list[str], outcomes: dict) -> dict:
    return dict(review_id=review_id, population=population, case_id=case['case_id'], input=case['input'],
                intended_meaning=case['intended_meaning'], candidate=candidate, kind=kind, evidence=glosses[:3], sources=sources,
                outcome_by_target=outcomes)


def queue(run_id: str) -> None:
    run_dir = RUN_ROOT / run_id
    review = run_dir / 'review'
    if review.exists():
        raise SystemExit('review queue already exists')
    outputs = read_outputs(run_dir)
    merged = original.candidate_index({}, outputs=outputs)
    final_keys = sorted(k for k, e in merged.items() if any(o.startswith('final') for o in e['outcomes'].values()))
    loss_population = sorted(k for k, e in merged.items()
                             if k not in final_keys and any(not o.startswith('normalization') for o in e['outcomes'].values()))
    loss_sample = sorted(random.Random(original.REVIEW_SAMPLE_SEED).sample(loss_population, min(original.FILTER_LOSS_SAMPLE_MAX, len(loss_population))))
    pairs = []
    for population, keys, prefix in (('final_pairs', final_keys, 'F'), ('filter_loss_sample', loss_sample, 'L')):
        for key in keys:
            entry = merged[key]
            candidate = entry['candidate']
            evidence = [e for e in candidate['evidence'] if not e['is_policy_excluded']] or candidate['evidence']
            glosses = list(dict.fromkeys(f"{e['relation_kind']}@{e['source_id']}: {(e['to_gloss'] or e['gloss'] or '')[:90]}" for e in evidence))
            pairs.append(pair_record(f'{prefix}{len(pairs) + 1:03d}', population, entry['case'], key[1], candidate['primary_kind'] or '',
                                     glosses, sorted({e['source_id'] for e in evidence}), entry['outcomes']))
    morphology = {}
    for target, rows in outputs.items():
        for case, output in zip(original.load_cases(), rows):
            for item in morphology_words(output):
                entry = morphology.setdefault((case['case_id'], item['word']), dict(case=case, type=item['type'], outcomes={}))
                entry['outcomes'][target] = f"{item['type']}:{item['file_count']}"
    for (case_id, word), entry in sorted(morphology.items()):
        pairs.append(pair_record(f'M{len(pairs) + 1:03d}', 'morphology', entry['case'], word, entry['type'],
                                 [f"morphology:{entry['type']} (same Snowball stem or prefix family in the repository vocabulary)"],
                                 ['repository-vocabulary'], entry['outcomes']))
    review.mkdir()
    (review / 'pairs.jsonl').write_text(''.join(json.dumps(p, ensure_ascii=False) + '\n' for p in pairs))
    (review / 'assignments').mkdir()
    for number, start in enumerate(range(0, len(pairs), ASSIGNMENT_PAIRS), 1):
        chunk = pairs[start:start + ASSIGNMENT_PAIRS]
        (review / 'assignments' / f'batch-{number:03}.jsonl').write_text(''.join(json.dumps(p, ensure_ascii=False) + '\n' for p in chunk))
    rubric = json.loads(V1_RUBRIC.read_text())
    rubric['populations']['morphology'] = 'every unique (case, word) returned in the separate morphology list in any target'
    rubric['morphology_note'] = 'morphology pairs come from repository vocabulary, not from a dictionary sense; judge them against intended_meaning like any other pair'
    write_json(review / 'rubric.json', rubric)
    counts = collections.Counter(p['population'] for p in pairs)
    write_json(review / 'queue-manifest.json', dict(schema='rt-development-v2-queue/1', run_id=run_id, counts=counts,
                                                    loss_population=len(loss_population), pairs=identity(review / 'pairs.jsonl'),
                                                    observations=identity(run_dir / 'observations.jsonl')))
    print(json.dumps(dict(counts=counts, loss_population=len(loss_population)), ensure_ascii=False))


def aggregate(run_id: str, labels_path: Path, report_id: str) -> None:
    import evaluate_development
    run_dir = RUN_ROOT / run_id
    report = run_dir / 'reports' / report_id / 'semantic-report.json'
    if report.exists():
        raise SystemExit('report already exists')
    pairs = {p['review_id']: p for p in map(json.loads, (run_dir / 'review/pairs.jsonl').read_text().splitlines())}
    rows = [json.loads(line) for line in labels_path.read_text().splitlines() if line.strip()]
    if sorted(r['review_id'] for r in rows) != sorted(pairs) or any(r['label'] not in original.LABELS or not r['rationale'].strip() for r in rows):
        raise SystemExit('labels must cover every queued pair exactly once with a valid label and rationale')
    dictionary_labels, morphology_rows = {}, []
    for row in rows:
        pair = pairs[row['review_id']]
        if (row['case_id'], row['candidate']) != (pair['case_id'], pair['candidate']):
            raise SystemExit(f"label pair mismatch: {row['review_id']}")
        if pair['population'] == 'morphology':
            morphology_rows.append((pair, row['label']))
        else:
            dictionary_labels[(pair['case_id'], pair['candidate'])] = dict(row, population=pair['population'])
    outputs = read_outputs(run_dir)
    observations = [json.loads(line) for line in (run_dir / 'observations.jsonl').read_text().splitlines()]
    metrics = evaluate_development.semantic_metrics(original.load_cases(), observations, original.candidate_index({}, outputs=outputs), dictionary_labels)
    by_type = collections.defaultdict(collections.Counter)
    for pair, label in morphology_rows:
        by_type[pair['kind']][label] += 1
        by_type['all'][label] += 1
    metrics['morphology'] = {kind: dict(n=sum(c.values()), labels=dict(c),
                                        useful_or_form=original.observed_proportion(sum(v for k, v in c.items() if k in original.USEFUL | {'spelling_or_form_variant'}), sum(c.values())),
                                        wrong_sense=original.observed_proportion(c['wrong_sense'], sum(c.values())))
                             for kind, c in sorted(by_type.items())}
    morphology_observations = [o for o in observations if o['morphology']['words']]
    metrics['morphology_coverage'] = dict(
        observations_with_morphology=original.observed_proportion(len(morphology_observations), len(observations)),
        unregistered_inputs_with_morphology=sorted({o['input'] for o in morphology_observations if o['status'] == 'no_dictionary_entry'}))
    report.parent.mkdir(parents=True)
    write_json(report, dict(schema='rt-development-v2-semantic-report/1', run_id=run_id, labels=identity(labels_path),
                            agent_labels_are_human_gold=False, metrics=metrics))
    print(json.dumps(dict(final_unique=metrics['final_candidates_unique'], morphology=metrics['morphology'].get('all')), ensure_ascii=False))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['run', 'queue', 'aggregate'])
    parser.add_argument('--run-id', required=True)
    parser.add_argument('--dataset-dir')
    parser.add_argument('--labels-file')
    parser.add_argument('--report-id')
    args = parser.parse_args()
    if args.command == 'run':
        run(args.run_id, Path(args.dataset_dir).resolve())
    elif args.command == 'queue':
        queue(args.run_id)
    else:
        aggregate(args.run_id, Path(args.labels_file).resolve(), args.report_id)


if __name__ == '__main__':
    main()
