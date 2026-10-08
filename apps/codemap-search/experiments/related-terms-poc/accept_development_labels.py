"""Scoped acceptance of the fresh development labels; read-only except the new report file.

Checks only what the semantic result depends on:
  1. frozen profile/run/queue/offline gates (evaluate_development.py validate)
  2. submission coverage (evaluate_development.py labels-check)
  3. submission bytes equal the recorded identity and the labelers' own outputs, in batch order
  4. labeler independence: raw tool events show only read/structured_output, reads stay inside each assignment allowlist
Repository changes outside these inputs are reported, never treated as failures.

Usage (cwd: repository root):
  python3 -B apps/codemap-search/experiments/related-terms-poc/accept_development_labels.py --output <new json path>
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

from rtpoc_common import REPO_ROOT, sha256_file, write_json

PROFILE_SHA = '29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd'
QUEUE_SHA = '81a238cd63009da71725172c6ae4ed0aece6f0974aef217a72f0cd8a212331fa'
RUN_ID = 'run-1'
WORKFLOW_RUN_ID = 'b064370d-8f2b-4b8e-b247-02b63fa7aff1'
EXPERIMENT = REPO_ROOT / 'apps/codemap-search/experiments/related-terms-poc'
PROFILE = EXPERIMENT / f'results/development/profiles/{PROFILE_SHA}/profile.json'
RUN_DIR = REPO_ROOT / f'.codemap/poc/related-terms/evaluation/development/{PROFILE_SHA}/{RUN_ID}'
SUBMISSION = RUN_DIR / 'review/submissions/development-labeling-v1-attempt-001.jsonl'
LABELING_DIR = REPO_ROOT / '.codemap/poc/related-terms/development-labeling-v1'
COLLECTION_HANDOFF = REPO_ROOT / '.codemap/poc/related-terms/development-labeling-verification-v2/collection-handoff.json'
RUNS_DIR = Path.home() / 'tmp/pi-subagents-uid-501/async-subagent-runs'
OUTPUTS_DIR = (Path.home() / '.pi/agent/sessions/--Users-buyong-workspace-private-buyong-mcp--/subagent-artifacts/outputs'
               / WORKFLOW_RUN_ID / 'fresh-evaluation-labels')
ALLOWED_TOOLS = {'read', 'structured_output'}
EVALUATOR = EXPERIMENT / 'evaluate_development.py'


def identity(path: Path) -> dict:
    return dict(path=path.relative_to(REPO_ROOT).as_posix() if path.is_relative_to(REPO_ROOT) else str(path),
                sha256=sha256_file(path), bytes=path.stat().st_size)


def run_evaluator(*arguments: str) -> dict:
    command = [sys.executable, '-B', str(EVALUATOR), *arguments, '--profile', str(PROFILE), '--run-id', RUN_ID]
    result = subprocess.run(command, cwd=REPO_ROOT, capture_output=True, text=True)
    return dict(command=command[2:], exit_code=result.returncode, stdout=result.stdout.strip(), stderr=result.stderr.strip(),
                passed=result.returncode == 0)


def check_submission_provenance(batches: list[dict]) -> dict:
    handoff = json.loads(COLLECTION_HANDOFF.read_text())
    recorded = handoff['actual_submission_identity']
    current = identity(SUBMISSION)
    submitted = [json.loads(line) for line in SUBMISSION.read_text().splitlines()]
    labeler_records = []
    for batch in batches:
        output = json.loads((OUTPUTS_DIR / f"{batch['batch_id']}.json").read_text())
        is_complete = output['status'] == 'complete' and output['remaining_review_ids'] == [] and output['blocker'] is None
        if not is_complete or [r['review_id'] for r in output['records']] != batch['review_ids']:
            return dict(passed=False, reason=f"labeler output incomplete or out of order: {batch['batch_id']}")
        labeler_records.extend(output['records'])
    is_identity_equal = {k: current[k] for k in ('sha256', 'bytes')} == {k: recorded[k] for k in ('sha256', 'bytes')}
    return dict(passed=is_identity_equal and submitted == labeler_records, submission=current, recorded_identity=recorded,
                records=len(submitted), equals_labeler_outputs=submitted == labeler_records)


def allowlist(assignment_path: Path) -> set[str]:
    assignment = json.loads(assignment_path.read_text())
    return {str(assignment_path)} | {str(REPO_ROOT / item['path']) for item in assignment['required_files']}


def check_labeler_reads(batches: list[dict]) -> dict:
    per_batch = []
    for batch in batches:
        ledger = json.loads((LABELING_DIR / f"collections/attempt-001/{batch['batch_id']}-ledger.json").read_text())
        run_id = ledger['publication']['run_id']
        allowed = allowlist(REPO_ROOT / batch['assignment_identity']['path'])
        tools, outside, errors, reads = set(), [], 0, 0
        with (RUNS_DIR / run_id / 'events.jsonl').open('rb') as events:
            for line in events:
                # Message bodies are skipped; only tool-call events are decoded.
                if b'"tool_execution_start"' not in line and b'"tool_execution_end"' not in line:
                    continue
                event = json.loads(line)
                if event.get('type') == 'tool_execution_end':
                    errors += event.get('isError') is True
                    continue
                if event.get('type') != 'tool_execution_start':
                    continue
                tools.add(event['toolName'])
                if event['toolName'] == 'read':
                    reads += 1
                    path = str(Path(event['args']['path']).resolve())
                    if path not in allowed:
                        outside.append(path)
        status = json.loads((RUNS_DIR / run_id / 'status.json').read_text())
        per_batch.append(dict(batch_id=batch['batch_id'], run_id=run_id, model=status['steps'][0].get('model'), reads=reads,
                              tools=sorted(tools), reads_outside_allowlist=outside, tool_errors=errors,
                              passed=tools <= ALLOWED_TOOLS and 'structured_output' in tools and not outside))
    review_root = str(RUN_DIR / 'review')
    is_scope_fresh = all(path.startswith(review_root) for batch in batches
                         for path in allowlist(REPO_ROOT / batch['assignment_identity']['path']))
    return dict(passed=is_scope_fresh and all(b['passed'] for b in per_batch), allowlist_inside_fresh_review_dir=is_scope_fresh,
                batches=per_batch)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--output', required=True)
    output = Path(parser.parse_args().output).resolve()
    if output.exists():
        raise SystemExit(f'refusing to overwrite {output}')
    batches = json.loads((LABELING_DIR / 'batches.json').read_text())
    checks = dict(
        frozen_inputs=run_evaluator('validate'),
        label_coverage=run_evaluator('labels-check', '--labels-file', str(SUBMISSION)),
        submission_provenance=check_submission_provenance(batches),
        labeler_independence=check_labeler_reads(batches),
    )
    git_status = subprocess.run(['git', 'status', '--short'], cwd=REPO_ROOT, capture_output=True, text=True).stdout
    report = dict(
        schema='rt-development-label-acceptance/1',
        passed=all(check['passed'] for check in checks.values()),
        checks=checks,
        result_inputs=[identity(path) for path in (PROFILE, RUN_DIR / 'review/queue-manifest.json', RUN_DIR / 'observations.jsonl', SUBMISSION)],
        out_of_scope_report=dict(note='reported only; not an acceptance condition', git_status_short=git_status.splitlines()),
    )
    if report['result_inputs'][1]['sha256'] != QUEUE_SHA:
        report['passed'] = False
        report['queue_identity_mismatch'] = True
    write_json(output, report)
    print(json.dumps(dict(passed=report['passed'], **{name: check['passed'] for name, check in checks.items()}), ensure_ascii=False))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
