#!/usr/bin/env python3
"""Precision judgment and final verdict of one held-out feature run (criteria 3).

  assign  shuffle <run>/label-queue.jsonl once with the criteria seed, then hand out the next
          sample_step pairs of that order as labeling assignments (review/assignments/batch-NNN.jsonl);
          a later call extends the sample only after judge found the current one inconclusive
  judge   check the labels cover the assigned pairs exactly once, compute the pooled useful rate and
          its Wilson 95% interval, and decide pass / fail / inconclusive; a conclusive judgment writes
          <run>/final-verdict.json with the agent-loop effect and the hard limits of summary.json

Usage (cwd: anywhere):
  python3 -B apps/codemap-search/experiments/related-terms-poc/judge_feature_precision.py assign --run-id ID
  python3 -B apps/codemap-search/experiments/related-terms-poc/judge_feature_precision.py judge --run-id ID --labels-file FILE --agent-loop-run AGENT_RUN_ID
"""
from __future__ import annotations

import argparse
import collections
import json
import math
import random
from pathlib import Path

from evaluate_feature import display_path
from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, sha256_file, write_json

RUN_ROOT = DATA_ROOT / 'evaluation/feature-test'
AGENT_LOOP_ROOT = DATA_ROOT / 'evaluation/agent-loop'
CRITERIA = EXPERIMENT_DIR / 'contracts/feature-success-criteria-3.json'
RUBRIC = EXPERIMENT_DIR / 'contracts/feature-precision-rubric-2.json'
# Earlier rubrics stay judgeable: judge uses the rubric recorded at assign time.
RUBRICS = (RUBRIC, EXPERIMENT_DIR / 'contracts/feature-precision-rubric.json')
ASSIGNMENT_PAIRS = 40
WILSON_Z = 1.96


def read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def assign(run_dir: Path) -> None:
    criteria = json.loads(CRITERIA.read_text())['precision']
    queue = read_jsonl(run_dir / 'label-queue.jsonl')
    assignments = run_dir / 'review/assignments'
    manifest_path = assignments / 'manifest.json'
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
        if manifest['queue_sha256'] != sha256_file(run_dir / 'label-queue.jsonl'):
            raise SystemExit('label-queue.jsonl changed after assignment')
        if manifest['assigned_pairs'] >= manifest['pairs']:
            raise SystemExit('every pair is already assigned')
        judgment_path = run_dir / 'review' / f'judgment-{manifest["assigned_pairs"]:05}.json'
        if not judgment_path.is_file() or json.loads(judgment_path.read_text())['status'] != 'inconclusive':
            raise SystemExit(f'extend only after judge finds the {manifest["assigned_pairs"]} assigned pairs inconclusive')
        order = manifest['order']
    else:
        # The seeded order is fixed before any label exists, so every step labels a prefix of it.
        order = [pair['review_id'] for pair in queue]
        random.Random(criteria['seed']).shuffle(order)
        assignments.mkdir(parents=True)
        manifest = dict(queue_sha256=sha256_file(run_dir / 'label-queue.jsonl'), rubric_sha256=sha256_file(RUBRIC),
                        seed=criteria['seed'], sample_step=criteria['sample_step'], pairs=len(queue),
                        assigned_pairs=0, steps=[], order=order)
    if manifest['assigned_pairs'] >= len(order):
        raise SystemExit('every pair is already assigned')
    by_id = {pair['review_id']: pair for pair in queue}
    start, end = manifest['assigned_pairs'], min(manifest['assigned_pairs'] + criteria['sample_step'], len(order))
    first_batch = sum(len(step['batches']) for step in manifest['steps']) + 1
    batches = []
    for number, offset in enumerate(range(start, end, ASSIGNMENT_PAIRS), first_batch):
        name = f'batch-{number:03}.jsonl'
        (assignments / name).write_text(''.join(json.dumps(by_id[review_id], ensure_ascii=False) + '\n'
                                                for review_id in order[offset:min(offset + ASSIGNMENT_PAIRS, end)]))
        batches.append(name)
    manifest['steps'].append(dict(step=len(manifest['steps']) + 1, pairs=[start, end], batches=batches))
    manifest['assigned_pairs'] = end
    write_json(manifest_path, manifest)
    print(json.dumps(dict(pairs=len(queue), assigned_pairs=end, new_batches=batches), ensure_ascii=False))


def wilson_interval(successes: int, n: int) -> tuple[float, float]:
    center = (successes + WILSON_Z ** 2 / 2) / (n + WILSON_Z ** 2)
    margin = WILSON_Z * math.sqrt(successes * (n - successes) / n + WILSON_Z ** 2 / 4) / (n + WILSON_Z ** 2)
    return center - margin, center + margin


def precision(pairs: list[dict], labels: dict[str, str], useful: set[str]) -> dict:
    counts = collections.Counter(labels[pair['review_id']] for pair in pairs)
    k = sum(count for label, count in counts.items() if label in useful)
    return dict(n=len(pairs), useful=k, rate=round(k / len(pairs), 4) if pairs else None, labels=dict(sorted(counts.items())))


def judge(run_dir: Path, labels_file: Path, agent_loop_dir: Path) -> None:
    output = run_dir / 'final-verdict.json'
    if output.exists():
        raise SystemExit('final-verdict.json already exists')
    criteria = json.loads(CRITERIA.read_text())
    queue = read_jsonl(run_dir / 'label-queue.jsonl')
    manifest = json.loads((run_dir / 'review/assignments/manifest.json').read_text())
    if manifest['queue_sha256'] != sha256_file(run_dir / 'label-queue.jsonl'):
        raise SystemExit('label-queue.jsonl changed after assignment')
    rubric = next((path for path in RUBRICS if sha256_file(path) == manifest['rubric_sha256']), None)
    if rubric is None:
        raise SystemExit('assignment rubric not found')
    rubric_labels = set(json.loads(rubric.read_text())['labels'])
    by_id = {pair['review_id']: pair for pair in queue}
    assigned = set(manifest['order'][:manifest['assigned_pairs']])
    labels = {}
    for row in read_jsonl(labels_file):
        pair = by_id.get(row['review_id'])
        if pair is None or row['review_id'] not in assigned or row['review_id'] in labels \
                or row['candidate'] != pair['candidate'] or row['case_id'] != pair['case_id'] or row['label'] not in rubric_labels:
            raise SystemExit(f'invalid, unassigned, or duplicate label record: {row.get("review_id")}')
        labels[row['review_id']] = row['label']
    if set(labels) != assigned:
        raise SystemExit(f'labels cover {len(labels)} of {len(assigned)} assigned pairs')

    useful = set(criteria['precision']['useful_labels'])
    threshold = criteria['precision']['min_useful_rate']
    sample = [by_id[review_id] for review_id in manifest['order'][:manifest['assigned_pairs']]]
    pooled = precision(sample, labels, useful)
    lower, upper = wilson_interval(pooled['useful'], pooled['n'])
    is_complete = len(sample) == len(queue)
    if lower >= threshold or (is_complete and pooled['rate'] >= threshold):
        status = 'pass'
    elif upper < threshold or is_complete:
        status = 'fail'
    else:
        status = 'inconclusive'
    judgment = dict(status=status, threshold=threshold, labeled_pairs=len(sample), queue_pairs=len(queue),
                    wilson_95=[round(lower, 4), round(upper, 4)], pooled=pooled,
                    per_target={target: precision([p for p in sample if p['target'] == target], labels, useful)
                                for target in criteria['targets']},
                    per_source={source: precision([p for p in sample if p['source'] == source], labels, useful)
                                for source in sorted({pair['source'] for pair in sample})},
                    labels_file=dict(path=str(labels_file), sha256=sha256_file(labels_file)))
    write_json(run_dir / 'review' / f'judgment-{len(sample):05}.json', judgment)
    if status == 'inconclusive':
        print(json.dumps(dict(precision=pooled['rate'], wilson_95=judgment['wilson_95'], status=status,
                              next='run assign again to label the next pairs'), ensure_ascii=False))
        return

    agent_loop = json.loads((agent_loop_dir / 'summary.json').read_text())
    feature_run = display_path(run_dir)
    if agent_loop['criteria']['sha256'] != sha256_file(CRITERIA) or feature_run not in agent_loop['sources']:
        raise SystemExit(f'{agent_loop_dir.name} was not scored from {feature_run} with these criteria')
    effect = agent_loop['verdict']['effect']
    hard_limits = json.loads((run_dir / 'summary.json').read_text())['verdict']['hard_limits']
    write_json(output, dict(
        schema='rt-feature-final-verdict/2', run_id=run_dir.name, criteria_sha256=sha256_file(CRITERIA),
        rubric=dict(path=str(rubric.relative_to(EXPERIMENT_DIR)), sha256=sha256_file(rubric)),
        agent_labels_are_human_gold=False,
        effect=dict(agent_loop_run=agent_loop['run_id'], rule=agent_loop['verdict']['rule'], per_target=effect,
                    pooled_reported_only=agent_loop['pooled']),
        hard_limits=hard_limits, precision=judgment,
        overall=dict(
            is_pass=status == 'pass' and hard_limits['status'] == 'pass' and all(e['status'] == 'pass' for e in effect.values()),
            note='every target effect must pass; a not_measurable target is reported, not passed')))
    print(json.dumps(dict(precision=pooled['rate'], wilson_95=judgment['wilson_95'], precision_status=status,
                          effect={t: (e['status'], e.get('rate_difference')) for t, e in effect.items()},
                          hard_limits=hard_limits['status']), ensure_ascii=False))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['assign', 'judge'])
    parser.add_argument('--run-id', required=True)
    parser.add_argument('--run-root', type=Path, default=RUN_ROOT)
    parser.add_argument('--labels-file', type=Path)
    parser.add_argument('--agent-loop-run', help='judge: the evaluate_agent_loop.py run scored from this feature run')
    parser.add_argument('--agent-loop-root', type=Path, default=AGENT_LOOP_ROOT)
    args = parser.parse_args()
    run_dir = args.run_root.resolve() / args.run_id
    if args.command == 'assign':
        assign(run_dir)
    else:
        if args.labels_file is None or args.agent_loop_run is None:
            raise SystemExit('judge needs --labels-file and --agent-loop-run')
        judge(run_dir, args.labels_file.resolve(), args.agent_loop_root.resolve() / args.agent_loop_run)


if __name__ == '__main__':
    main()
