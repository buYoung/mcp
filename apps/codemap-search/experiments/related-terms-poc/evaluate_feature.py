#!/usr/bin/env python3
"""Held-out feature evaluation: does the rt-block/2 related-terms block recover gold files that the
baseline search misses, beyond a budget-matched control that searches the original terms only?

Per case: baseline = all terms joined by spaces -> top K. Per mode (dictionary, morphology, both):
rtpoc2 block (rtl-2.4, dataset v2) -> each suggested query -> top K; treatment set = baseline top K
plus every suggested-query top K. Control: the first k control_queries of the same block (k = number
of suggested queries) -> top K each; control set = baseline top K plus those. Since rtl-2.2 (H13
paired_append) suggested query i is control query i plus one related term, so the two sets differ
only by the related terms; rtl-2.3 (H14-H16) adds repository-verified identifier forms and rtl-2.4 (H17)
searches a single-word term's related term alone against the word alone. Searches use
`rtpoc2 search` (the product's ranked search) with the MCP search candidate limit, and top K is cut
from `displayed`, the order the MCP search tool renders (directory-diversified detail head). The
candidate limit changes the ranking (a larger BM25 pool is re-ranked), so it and K are options
recorded in the run manifest.

label-queue.jsonl lists every related term the 'both' block showed (unique per case, term,
candidate) for a separate precision labeling pass.

Inputs default to the frozen files and are verified against the round's freeze.json and
method-freeze.json before any work. --cases/--terms replace them for development and smoke runs
only (no freeze check), and then --output-root is required.
Usage (cwd: anywhere):
  python3 -B apps/codemap-search/experiments/related-terms-poc/evaluate_feature.py --run-id ID
"""
from __future__ import annotations

import argparse
import collections
import json
import math
import re
import statistics
import subprocess
import threading
from pathlib import Path

from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, REPO_ROOT, sha256_file, split_identifier, write_json

BINARY = EXPERIMENT_DIR / 'lookup-v2/target/release/rtpoc2'
POLICY = EXPERIMENT_DIR / 'lookup-v2/contracts/lookup-policy-rtl-2.4.json'
EXPECTED_POLICY_ID = 'rtl-2.4'
EXPECTED_BLOCK_SCHEMA = 'rt-block/2'
DATASET_DIR = DATA_ROOT / 'development-curation-v2/dataset-v2/rtd1-2df12da52eb7a387'
# Round 4 (heldout-5): this run produces the blocks and baselines that evaluate_agent_loop.py judges.
# Earlier harnesses, policies, and binaries are archived under evaluation/feature-test/<run>/method-snapshot/.
FREEZE = DATA_ROOT / 'eval-targets/heldout-5/freeze.json'
METHOD_FREEZE = DATA_ROOT / 'eval-targets/heldout-5/method-freeze.json'
CRITERIA = EXPERIMENT_DIR / 'contracts/feature-success-criteria-3.json'
FROZEN_CASES = EXPERIMENT_DIR / 'cases-feature-test-5.json'
FROZEN_TERMS = EXPERIMENT_DIR / 'terms-feature-test-5.json'
STATE_ROOT = DATA_ROOT / 'workspaces/state-test'
OUTPUT_ROOT = DATA_ROOT / 'evaluation/feature-test'
MODES = ('dictionary', 'morphology', 'both')
# The MCP search tool ranks with this candidate limit (tools/search DEFAULT_SEARCH_LIMIT); the
# BM25 candidate pool depends on it, so top-K is cut from the same ranking an agent sees.
DEFAULT_SEARCH_LIMIT = 100
EFFECT_DIFFICULTIES = ('medium', 'complex')


def identity(path: Path) -> dict:
    return dict(path=display_path(path), sha256=sha256_file(path), bytes=path.stat().st_size)


def display_path(path: Path) -> str:
    path = path.resolve()
    return path.relative_to(REPO_ROOT).as_posix() if path.is_relative_to(REPO_ROOT) else str(path)


def check_entries(freeze_path: Path, entries: dict) -> dict:
    """Every {path, sha256, bytes} entry of a freeze file must match the file on disk."""
    checks = {}
    for name, entry in entries.items():
        path = REPO_ROOT / entry['path']
        actual = dict(sha256=sha256_file(path), bytes=path.stat().st_size) if path.is_file() else None
        checks[name] = dict(path=entry['path'], expected_sha256=entry['sha256'], actual_sha256=actual and actual['sha256'],
                            is_match=actual == dict(sha256=entry['sha256'], bytes=entry['bytes']))
    failed = [name for name, check in checks.items() if not check['is_match']]
    if failed:
        raise SystemExit(f'frozen files differ from {display_path(freeze_path)}: {failed}; refusing to run')
    return checks


def verify_freeze(is_frozen_input: bool) -> dict:
    """Inputs (freeze.json) always, and the method (method-freeze.json) whenever it exists;
    a run on the frozen inputs refuses to start without a method freeze."""
    freeze = json.loads(FREEZE.read_text())
    if (REPO_ROOT / freeze['cases']['path']).resolve() != FROZEN_CASES.resolve() \
            or (REPO_ROOT / freeze['terms']['path']).resolve() != FROZEN_TERMS.resolve():
        raise SystemExit('freeze.json names other cases/terms files than this harness reads')
    record = dict(freeze=identity(FREEZE), schema=freeze['schema'], frozen_at=freeze['frozen_at'],
                  checks=check_entries(FREEZE, {name: freeze[name] for name in ('cases', 'terms', 'prompts_only')}))
    if METHOD_FREEZE.is_file():
        method = json.loads(METHOD_FREEZE.read_text())
        record['method_freeze'] = dict(identity(METHOD_FREEZE), frozen_at=method['frozen_at'],
                                       checks=check_entries(METHOD_FREEZE, method['files']))
    elif is_frozen_input:
        raise SystemExit(f'{display_path(METHOD_FREEZE)} is missing; freeze the method before the held-out run')
    return record


def load_inputs(cases_path: Path, terms_path: Path) -> tuple[dict, list[dict]]:
    cases_file = json.loads(cases_path.read_text())
    terms = json.loads(terms_path.read_text())['terms']
    cases = []
    for case in cases_file['cases']:
        if case['id'] not in terms:
            raise SystemExit(f'no terms for case {case["id"]}')
        if case['target'] not in cases_file['targets']:
            raise SystemExit(f'case {case["id"]} names unknown target {case["target"]}')
        cases.append(dict(case, terms=terms[case['id']]))
    return cases_file['targets'], cases


def run_command(argv: list[str]) -> str:
    result = subprocess.run(argv, cwd=REPO_ROOT, capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(f'command failed ({result.returncode}): {argv[:3]}...: {result.stderr[-800:]}')
    return result.stdout


def check_target_state(state_root: Path, target: str, snapshot_path: str) -> dict:
    """The target state must be published from the frozen snapshot; returns identities."""
    state = state_root / target
    published = json.loads((state / 'published.json').read_text())
    snapshot = (REPO_ROOT / snapshot_path).resolve()
    if Path(published['workspace']['root']).resolve() != snapshot:
        raise SystemExit(f'{target}: state indexed {published["workspace"]["root"]}, not {snapshot}')
    indexed = set(json.loads((state / 'indexed-files.json').read_text()))
    return dict(state=display_path(state), published=identity(state / 'published.json'), index_meta=identity(state / 'index/meta.json'),
                index_generation=published['index_generation'], live_files=published['live_files'],
                vocabulary_size=published['vocabulary_size'], indexed_paths=indexed)


class Searcher:
    """`rtpoc2 search` with a per-(target, query) cache; every distinct search is saved raw.
    `top` may run from several threads: searches open the index read-only."""

    def __init__(self, raw_dir: Path, state_root: Path, limit: int, top_k: int):
        self.cache: dict[tuple[str, str], list[str]] = {}
        self.state_root = state_root
        self.empty_searches = 0
        self.raw = raw_dir / 'searches.jsonl'
        self.limit = limit
        self.top_k = top_k
        self.calls = 0
        self.lock = threading.Lock()

    def top(self, target: str, query: str) -> list[str]:
        key = (target, query)
        if key not in self.cache:
            output = json.loads(run_command(
                [str(BINARY), 'search', '--state', str(self.state_root / target), '--limit', str(self.limit), query]))
            if output['query'] != query or output['limit'] != self.limit:
                raise SystemExit(f'search echoed a different request for {query!r}')
            with self.lock:
                if key not in self.cache:
                    self.calls += 1
                    self.empty_searches += not output['results']
                    self.cache[key] = output['displayed'][:self.top_k]
                    with self.raw.open('a') as handle:
                        handle.write(json.dumps(dict(target=target, **output), ensure_ascii=False) + '\n')
        return self.cache[key]


def normalize_path(path: str) -> str:
    return path.removeprefix('./')


def gold_found(paths: set[str], gold: list[str]) -> list[str]:
    return [g for g in gold if normalize_path(g) in paths]


def is_identifier_shaped(term: str) -> bool:
    """Harness-side re-derivation of the block's identifier rule (Python split_identifier port)."""
    term = term.strip()
    return '::' in term or '.' in term or len(split_identifier(term)) > 1


def term_words(terms: list[str]) -> set[str]:
    words = set()
    for term in terms:
        words.update(term.lower().split())
        words.update(term.lower().replace('/', ' ').replace('-', ' ').replace('_', ' ').split())
    return words


def identifier_audit(case_terms: list[str], block: dict) -> dict:
    """Identifier-shaped terms are never expanded: block passthrough + harness re-derivation."""
    by_input = {term['input']: term for term in block['terms']}
    shaped, expanded = [], []
    for raw in case_terms:
        term = by_input[raw.strip()]
        if term['reason'] == 'identifier_shaped' or is_identifier_shaped(raw):
            shaped.append(raw)
            joined = any(raw.strip() in cross['terms'] for cross in block.get('cross_identifier_forms', []))
            if term['handling'] != 'passthrough' or term['dictionary'] or term['morphology'] \
                    or term.get('identifier_forms') or joined:
                expanded.append(raw)
    return dict(identifier_shaped_terms=shaped, identifier_terms_expanded=expanded)


def control_violations(case_terms: list[str], control_queries: list[str], block: dict) -> list[str]:
    """Control queries may contain only words of the input terms (whole terms or index tokens)."""
    allowed = term_words(case_terms)
    for term in block['terms']:
        allowed.update(term['index_tokens'])
    return [query for query in control_queries if not set(query.lower().split()) <= allowed]


def check_block(block: dict, dataset_id: str, generation: str) -> None:
    problems = []
    if block['schema'] != EXPECTED_BLOCK_SCHEMA:
        problems.append(f'schema {block["schema"]}')
    if block['policy']['policy_id'] != EXPECTED_POLICY_ID:
        problems.append(f'policy {block["policy"]["policy_id"]}')
    if block['dataset']['dataset_id'] != dataset_id:
        problems.append(f'dataset {block["dataset"]["dataset_id"]}')
    if block['stats']['status'] != 'ready' or block['stats']['index_generation'] != generation:
        problems.append(f'stats {block["stats"]["status"]} {block["stats"]["index_generation"]}')
    if block['block_bytes'] > block['max_block_bytes']:
        problems.append('block over max_block_bytes')
    # The "Suggested queries:" line is the last line of block_text whenever there are queries.
    last_line = block['block_text'].rstrip('\n').rpartition('\n')[2]
    if block['suggested_queries'] and not block['block_omissions']['is_byte_truncated'] \
            and last_line != 'Suggested queries: ' + ' | '.join(block['suggested_queries']):
        problems.append('suggested_queries differ from the queries block_text shows')
    snake_parts = block['stats'].get('snake_parts')
    if snake_parts is not None and snake_parts['status'] != 'ready':
        problems.append(f'snake_parts {snake_parts["status"]}')
    if problems:
        raise SystemExit(f'block rejected: {problems}')


def evaluate_case(case: dict, case_dir: Path, searcher: Searcher, dataset_id: str, target_state: dict) -> list[dict]:
    target = case['target']
    gold = case['gold_paths']
    baseline_query = ' '.join(case['terms'])
    baseline = searcher.top(target, baseline_query)
    baseline_set = set(baseline)
    baseline_hit = bool(gold_found(baseline_set, gold))
    terms_file = case_dir / 'terms.txt'
    terms_file.write_text(''.join(term + '\n' for term in case['terms']))
    write_json(case_dir / 'baseline.search.json', dict(query=baseline_query, top=baseline))
    rows = []
    for mode in MODES:
        block_path = case_dir / f'{mode}.block.json'
        run_command([str(BINARY), 'block', '--dataset', str(DATASET_DIR / 'dataset.sqlite3'), '--policy', str(POLICY),
                     '--state', str(searcher.state_root / target), '--task-query', case['prompt'], '--mode', mode,
                     '--terms-file', str(terms_file), '--output', str(block_path)])
        block = json.loads(block_path.read_text())
        check_block(block, dataset_id, target_state['index_generation'])
        suggested = block['suggested_queries']
        control = block['control_queries'][:len(suggested)]
        treatment_tops = [dict(query=q, top=searcher.top(target, q)) for q in suggested]
        control_tops = [dict(query=q, top=searcher.top(target, q)) for q in control]
        treatment_set = baseline_set.union(*(set(item['top']) for item in treatment_tops))
        control_set = baseline_set.union(*(set(item['top']) for item in control_tops))
        treatment_hit = bool(gold_found(treatment_set, gold))
        control_hit = bool(gold_found(control_set, gold))
        write_json(case_dir / f'{mode}.searches.json', dict(treatment=treatment_tops, control=control_tops))
        rows.append(dict(
            case_id=case['id'], target=target, difficulty=case['difficulty'], mode=mode,
            gold_paths=gold, gold_paths_indexed=[g for g in gold if normalize_path(g) in target_state['indexed_paths']],
            terms=case['terms'], baseline_query=baseline_query, baseline_top=baseline,
            baseline_hit=baseline_hit, treatment_hit=treatment_hit, control_hit=control_hit,
            recovered=treatment_hit and not baseline_hit, recovered_over_control=treatment_hit and not control_hit,
            gold_found=dict(baseline=gold_found(baseline_set, gold), treatment=gold_found(treatment_set, gold),
                            control=gold_found(control_set, gold)),
            treatment_searches=1 + len(suggested), control_searches=1 + len(control),
            control_budget_shortfall=len(suggested) - len(control),
            extra_result_rows=sum(len(item['top']) for item in treatment_tops),
            extra_distinct_paths=len(treatment_set - baseline_set),
            control_extra_result_rows=sum(len(item['top']) for item in control_tops),
            block_bytes=block['block_bytes'], block_omissions=block['block_omissions'],
            **identifier_audit(case['terms'], block),
            control_query_violations=control_violations(case['terms'], control, block),
            suggested_queries=suggested, control_queries=control, query_pairs=block.get('query_pairs', [])))
    return rows


def pair_candidates(block: dict) -> dict[tuple[str, str], tuple]:
    """(input, related text of a query pair) -> (candidate, source, kind, gloss), in BlockTerm::related order."""
    candidates = {}
    for term in block['terms']:
        identifiers = [(f['identifier'], (f['identifier'], 'identifier_form', 'identifier_form', None)) for f in term.get('identifier_forms', [])]
        dictionary = [(e['term'], (e['term'], 'dictionary', e['relation_kind'], e['gloss'])) for e in term['dictionary']]
        morphology = [(e['word'], (e['replacement'], 'morphology', e['type'], None)) for e in term['morphology']]
        ordered = identifiers + (morphology + dictionary if term['query_absent_in_repository'] else dictionary + morphology)
        for related, candidate in ordered:
            candidates.setdefault((term['input'], related), candidate)
    for cross in block.get('cross_identifier_forms', []):
        candidates.setdefault((' + '.join(cross['terms']), cross['identifier']),
                              (cross['identifier'], 'identifier_form', 'cross_identifier_form', None))
    return candidates


def label_queue(rows_by_case: dict[str, dict]) -> list[dict]:
    """Every related term the 'both' block showed, in its term lines or inside a shown suggested query,
    unique per (case, input term, candidate, source)."""
    pairs = []
    for case_id, item in rows_by_case.items():
        case, block = item['case'], item['block']
        queued = set()
        for term in block['terms']:
            dictionary = [(e['term'], 'dictionary', e['relation_kind'], e['gloss']) for e in term['dictionary']]
            morphology = [(e['replacement'], 'morphology', e['type'], None) for e in term['morphology']]
            identifiers = [(f['identifier'], 'identifier_form', 'identifier_form', None) for f in term.get('identifier_forms', [])]
            # Rendered order (see BlockTerm::rendered_expansions), cut to what block_text shows.
            rendered = identifiers + (morphology + dictionary if term['query_absent_in_repository'] else dictionary + morphology)
            shown = rendered[:term['shown_expansions']]
            for candidate, source, kind, gloss in dict.fromkeys(shown):
                queued.add((term['input'], candidate, source))
                pairs.append(dict(review_id=f'T{len(pairs) + 1:04d}', case_id=case_id, target=case['target'],
                                  prompt=case['prompt'], input=term['input'], candidate=candidate, source=source,
                                  kind=kind, gloss=gloss))
        # H15 cross-term identifiers, cut to what block_text shows (see capped_block).
        for cross in block.get('cross_identifier_forms', [])[:block.get('shown_cross_identifier_forms', 0)]:
            queued.add((' + '.join(cross['terms']), cross['identifier'], 'identifier_form'))
            pairs.append(dict(review_id=f'T{len(pairs) + 1:04d}', case_id=case_id, target=case['target'],
                              prompt=case['prompt'], input=' + '.join(cross['terms']), candidate=cross['identifier'],
                              source='identifier_form', kind='cross_identifier_form', gloss=None))
        # Related terms visible only inside a shown suggested query (their line was cut for size).
        by_pair = pair_candidates(block)
        for pair in block.get('query_pairs', []):
            candidate, source, kind, gloss = by_pair[(pair['term'], pair['related'])]
            if (pair['term'], candidate, source) not in queued:
                queued.add((pair['term'], candidate, source))
                pairs.append(dict(review_id=f'T{len(pairs) + 1:04d}', case_id=case_id, target=case['target'],
                                  prompt=case['prompt'], input=pair['term'], candidate=candidate, source=source,
                                  kind=kind, gloss=gloss))
    return pairs


def summarize(rows: list[dict]) -> list[dict]:
    groups: dict[tuple, list[dict]] = collections.defaultdict(list)
    for row in rows:
        groups[(row['target'], row['difficulty'], row['mode'])].append(row)
        if row['difficulty'] in EFFECT_DIFFICULTIES:
            groups[(row['target'], '+'.join(EFFECT_DIFFICULTIES), row['mode'])].append(row)
    summary = []
    for (target, difficulty, mode), items in sorted(groups.items()):
        misses = [row for row in items if not row['baseline_hit']]
        entry = dict(
            target=target, difficulty=difficulty, mode=mode, cases=len(items),
            baseline_hits=sum(row['baseline_hit'] for row in items), baseline_misses=len(misses),
            recovered=sum(row['recovered'] for row in misses),
            recovered_beyond_control=sum(row['recovered'] and row['recovered_over_control'] for row in misses),
            control_recovered=sum(row['control_hit'] for row in misses),
            max_block_bytes=max(row['block_bytes'] for row in items),
            mean_block_bytes=round(statistics.mean(row['block_bytes'] for row in items), 1),
            extra_result_rows_per_case=round(statistics.mean(row['extra_result_rows'] for row in items), 2),
            control_extra_result_rows_per_case=round(statistics.mean(row['control_extra_result_rows'] for row in items), 2),
            mean_treatment_searches=round(statistics.mean(row['treatment_searches'] for row in items), 2),
            cases_with_control_budget_shortfall=sum(row['control_budget_shortfall'] > 0 for row in items),
            control_term_violations=dict(
                identifier_terms_expanded=sum(bool(row['identifier_terms_expanded']) for row in items),
                control_queries_with_non_input_words=sum(bool(row['control_query_violations']) for row in items)))
        if difficulty == '+'.join(EFFECT_DIFFICULTIES):
            m = len(misses)
            entry.update(M=m, effect_threshold=math.ceil(m / 3))
        summary.append(entry)
    return summary


def verdict(rows: list[dict], summary: list[dict], criteria: dict) -> dict:
    """Effect per target in the judged mode, and the hard limits over every mode; precision is
    judged later from the labels of label-queue.jsonl."""
    effect_group = '+'.join(EFFECT_DIFFICULTIES)
    effect = {}
    for target in criteria['targets']:
        group = next((g for g in summary if g['target'] == target and g['difficulty'] == effect_group
                      and g['mode'] == criteria['judged_mode']), None)
        if group is None or group['M'] == 0:
            effect[target] = dict(status='not_measurable', M=group and group['M'])
            continue
        if 'judged_by' in criteria['effect']:
            # Criteria 3: the effect is judged from agent-written searches; the mechanical arms are reported only.
            effect[target] = dict(status='pending', judged_by=criteria['effect']['judged_by'], M=group['M'],
                                  mechanical_recovered_beyond_control=group['recovered_beyond_control'])
            continue
        is_pass = group['recovered_beyond_control'] >= group['effect_threshold']
        effect[target] = dict(status='pass' if is_pass else 'fail', M=group['M'], threshold=group['effect_threshold'],
                              recovered_beyond_control=group['recovered_beyond_control'], recovered=group['recovered'])
    limits = criteria['hard_limits']
    over_bytes = sorted({(row['case_id'], row['mode']) for row in rows if row['block_bytes'] > limits['max_block_bytes']})
    expanded = sorted({(row['case_id'], row['mode']) for row in rows if row['identifier_terms_expanded']})
    hard = dict(status='pass' if not over_bytes and not expanded else 'fail',
                over_max_block_bytes=over_bytes, identifier_terms_expanded=expanded)
    return dict(judged_mode=criteria['judged_mode'], effect=effect, hard_limits=hard,
                precision=dict(status='pending', note=f"label label-queue.jsonl, then judge: {criteria['precision']['rule']}"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--run-id', required=True)
    parser.add_argument('--cases', type=Path, help='development/smoke only, with --terms: replace the frozen cases file')
    parser.add_argument('--terms', type=Path, help='development/smoke only, with --cases: replace the frozen terms file')
    parser.add_argument('--output-root', type=Path, help=f'default {display_path(OUTPUT_ROOT)}; required with --cases/--terms')
    parser.add_argument('--state-root', type=Path, help=f'smoke only: replace {display_path(STATE_ROOT)} (e.g. the development states)')
    args = parser.parse_args()
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', args.run_id):
        raise SystemExit('--run-id must be a plain name (letters, digits, . _ -)')
    criteria = json.loads(CRITERIA.read_text())
    search_limit, top_k = criteria['search']['candidate_limit'], criteria['search']['top_k']
    if search_limit != DEFAULT_SEARCH_LIMIT:
        raise SystemExit(f'criteria candidate limit {search_limit} differs from the MCP search default {DEFAULT_SEARCH_LIMIT}')

    # The frozen run replaces nothing; a development or smoke run replaces both inputs and may not
    # name a frozen file, so it never reads held-out cases, terms, or freeze records.
    if (args.cases is None) != (args.terms is None):
        raise SystemExit('--cases and --terms go together')
    is_frozen_input = args.cases is None
    if is_frozen_input:
        cases_path, terms_path = FROZEN_CASES.resolve(), FROZEN_TERMS.resolve()
    else:
        cases_path, terms_path = args.cases.resolve(), args.terms.resolve()
        if {cases_path, terms_path} & {FROZEN_CASES.resolve(), FROZEN_TERMS.resolve()}:
            raise SystemExit('development runs may not read the frozen held-out inputs')
    freeze = verify_freeze(is_frozen_input) if is_frozen_input else dict(status='not_frozen_input')
    if not is_frozen_input and args.output_root is None:
        raise SystemExit('--output-root is required with non-frozen --cases/--terms (keeps smoke runs out of the evaluation tree)')
    if is_frozen_input and args.state_root is not None:
        raise SystemExit('--state-root is for smoke runs only')
    state_root = (args.state_root or STATE_ROOT).resolve()
    run_dir = (args.output_root or OUTPUT_ROOT).resolve() / args.run_id
    if run_dir.exists():
        raise SystemExit(f'run already exists: {run_dir}')

    targets, cases = load_inputs(cases_path, terms_path)
    dataset_id = json.loads((DATASET_DIR / 'manifest.json').read_text())['dataset_id']
    target_states = {target: check_target_state(state_root, target, targets[target]['snapshot_path'])
                     for target in sorted({case['target'] for case in cases})}
    run_dir.mkdir(parents=True)
    raw_dir = run_dir / 'raw'
    raw_dir.mkdir()
    searcher = Searcher(raw_dir, state_root, search_limit, top_k)
    rows = []
    shown_blocks = {}
    for case in cases:
        case_dir = raw_dir / case['id']
        case_dir.mkdir()
        rows.extend(evaluate_case(case, case_dir, searcher, dataset_id, target_states[case['target']]))
        shown_blocks[case['id']] = dict(case=case, block=json.loads((case_dir / 'both.block.json').read_text()))
    # The product engine recreates an index it cannot open; a changed index invalidates the run.
    changed = [t for t, state in target_states.items() if identity(state_root / t / 'index/meta.json') != state['index_meta']]
    if changed:
        raise SystemExit(f'index changed during the run: {changed}; results are invalid')
    (run_dir / 'cases.jsonl').write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in rows))
    queue = label_queue(shown_blocks)
    (run_dir / 'label-queue.jsonl').write_text(''.join(json.dumps(pair, ensure_ascii=False) + '\n' for pair in queue))
    summary = summarize(rows)
    write_json(run_dir / 'summary.json', dict(
        schema='rt-feature-test-summary/2', run_id=args.run_id, is_frozen_input=is_frozen_input,
        criteria=identity(CRITERIA), verdict=verdict(rows, summary, criteria),
        effect_rule='recovered = treatment hit and baseline miss; recovered_beyond_control additionally requires a control miss. '
                    'M counts medium+complex cases with a baseline miss; these mechanical arms are reported only, the effect is '
                    'judged by evaluate_agent_loop.py (criteria 3).',
        groups=summary))
    write_json(run_dir / 'run-manifest.json', dict(
        schema='rt-feature-test-run/1', run_id=args.run_id, is_frozen_input=is_frozen_input, freeze_check=freeze,
        cases=identity(cases_path), terms=identity(terms_path), policy=identity(POLICY), binary=identity(BINARY),
        dataset=dict(identity(DATASET_DIR / 'dataset.sqlite3'), dataset_id=dataset_id),
        targets={target: dict(commit=targets[target].get('commit'), snapshot_path=targets[target]['snapshot_path'],
                              **{k: v for k, v in state.items() if k != 'indexed_paths'})
                 for target, state in target_states.items()},
        criteria=identity(CRITERIA),
        search=dict(command='rtpoc2 search', limit=search_limit, top_k=top_k, order='displayed (MCP search render order)',
                    distinct_searches=searcher.calls, empty_searches=searcher.empty_searches,
                    index_meta_unchanged=True),
        label_queue=dict(identity(run_dir / 'label-queue.jsonl'), pairs=len(queue)),
        modes=list(MODES), case_count=len(cases)))
    print(json.dumps(dict(run_dir=display_path(run_dir), cases=len(cases), rows=len(rows), searches=searcher.calls), ensure_ascii=False))


if __name__ == '__main__':
    main()
