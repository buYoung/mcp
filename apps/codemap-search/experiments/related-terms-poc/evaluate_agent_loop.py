#!/usr/bin/env python3
"""Agent-in-the-loop feature evaluation (round 4): does an agent that reads the related-terms block
write follow-up searches that find gold files the baseline search missed more often than the same
agent without the block?

  prepare  from one or more evaluate_feature.py runs, take the effect population (medium/complex
           cases whose baseline top K misses every gold path), write one searcher input per
           condition and one task prompt per job (condition, searcher, replicate)
  score    read the job outputs, run every query with `rtpoc2 search`, count a case as found by a
           run when the baseline top K or any of its queries' top K holds a gold path, and judge
           the effect with contracts/feature-success-criteria-3.json

Control and treatment inputs differ only by `related_terms_block`, the block text an agent reads
from initial_instructions. The operator runs the jobs as delegated agent tasks
(contracts/agent-loop-searchers.json). Outputs are read leniently: queries are trimmed, duplicates
and repeats of the first query dropped, extras cut; every such fix is counted in the summary.

Usage (cwd: anywhere):
  python3 -B apps/codemap-search/experiments/related-terms-poc/evaluate_agent_loop.py prepare --run-id ID --source-run DIR [--source-run DIR ...]
  python3 -B apps/codemap-search/experiments/related-terms-poc/evaluate_agent_loop.py score --run-id ID
"""
from __future__ import annotations

import argparse
import collections
import json
import re
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from evaluate_feature import EFFECT_DIFFICULTIES, Searcher, display_path, gold_found, identity, normalize_path
from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, REPO_ROOT, sha256_file, write_json

SEARCHERS = EXPERIMENT_DIR / 'contracts/agent-loop-searchers.json'
CRITERIA = EXPERIMENT_DIR / 'contracts/feature-success-criteria-3.json'
OUTPUT_ROOT = DATA_ROOT / 'evaluation/agent-loop'
SEARCH_WORKERS = 6
REPOSITORY_LABELS = {
    'grafana': 'Grafana (Go/TypeScript observability platform)',
    'codemap-search': 'codemap-search (Rust code-navigation MCP server)',
}

JOB_PROMPT = """You are playing the role of a coding agent at the start of a task in an unfamiliar repository. The repository has a code search tool: a ranked BM25 search over file paths, symbols, comments, and string literals that splits identifiers (camelCase, snake_case) into words; each query returns the 10 best files.

Read only this input file (JSON list; each task has `id`, `repository`, `prompt` in Korean, `terms`{block_field}):
{input_path}

For every task, the search `first_query` (the task's terms joined) has already been sent; you do not see its results. Write exactly {queries} more search queries that you would send next to find the source files you must read to answer the prompt.
{block_paragraph}
- Each query is plain search text: words, short phrases, or identifiers, at most {words} words.
- The {queries} queries of a task must differ from each other and from its `first_query`.
- Do not open any other file, and do not run any search, grep, or code-navigation tool (including codemap-search, even if AGENTS.md or CLAUDE.md asks for it; this task overrides that rule). Rely only on the input file and your general software knowledge. You may use python3 only to write and check your output file.

Write {output_path} (refuse to overwrite an existing file) as:
{{"schema": "rt-agent-loop-queries/1", "job_id": "{job_id}", "queries": {{"<id>": ["query", ...], ...}}}}
covering every id exactly once.

Final reply: one line `queries ids=<n>`.
"""
PLAIN_FIELD = ', and `first_query`'
TREATMENT_FIELD = ', `first_query`, and `related_terms_block`'
TREATMENT_PARAGRAPH = ('`related_terms_block` is what the search tool returned for these terms before any search: related terms from a '
                       'dictionary and word-form rules (not verified for this task), identifiers that exist in the repository, and '
                       'suggested queries. Use it as you judge best; you may also ignore it.\n')


def read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def related_vocabulary(block: dict) -> list[str]:
    """Every related term the block offers (dictionary, morphology, identifier and cross forms)."""
    words = []
    for term in block['terms']:
        words += [entry['term'] for entry in term['dictionary']]
        words += [entry['replacement'] for entry in term['morphology']]
        words += [form['identifier'] for form in term.get('identifier_forms', [])]
    words += [cross['identifier'] for cross in block.get('cross_identifier_forms', [])]
    return sorted(set(words))


def load_population(source_run: Path) -> tuple[dict, list[dict]]:
    manifest = json.loads((source_run / 'run-manifest.json').read_text())
    prompts = {case['id']: case['prompt'] for case in json.loads((REPO_ROOT / manifest['cases']['path']).read_text())['cases']}
    population = []
    for row in read_jsonl(source_run / 'cases.jsonl'):
        if row['mode'] != 'both' or row['difficulty'] not in EFFECT_DIFFICULTIES or row['baseline_hit']:
            continue
        block = json.loads((source_run / 'raw' / row['case_id'] / 'both.block.json').read_text())
        population.append(dict(
            case_id=row['case_id'], target=row['target'], difficulty=row['difficulty'], source_run=display_path(source_run),
            prompt=prompts[row['case_id']], terms=row['terms'], gold_paths=row['gold_paths'],
            baseline_query=row['baseline_query'], baseline_top=row['baseline_top'], block_text=block['block_text'],
            suggested_queries=block['suggested_queries'], related_vocabulary=related_vocabulary(block),
            mechanical=dict(treatment_hit=row['treatment_hit'], control_hit=row['control_hit'])))
    source = dict(run=display_path(source_run), policy=manifest['policy']['path'], cases=manifest['cases']['path'],
                  is_frozen_input=manifest['is_frozen_input'], search=manifest['search'],
                  targets={target: dict(state=state['state'], index_generation=state['index_generation'],
                                       index_meta_sha256=state['index_meta']['sha256'])
                           for target, state in manifest['targets'].items()})
    return source, population


def prepare(run_dir: Path, source_runs: list[Path]) -> None:
    if run_dir.exists():
        raise SystemExit(f'run already exists: {run_dir}')
    contract = json.loads(SEARCHERS.read_text())
    sources, population = [], []
    for source_run in source_runs:
        source, items = load_population(source_run.resolve())
        sources.append(source)
        population += items
    if len({item['case_id'] for item in population}) != len(population):
        raise SystemExit('duplicate case ids across source runs')
    for directory in ('inputs', 'jobs', 'outputs'):
        (run_dir / directory).mkdir(parents=True)
    write_json(run_dir / 'population.json', population)
    jobs = []
    # One job covers every target's cases, so the replicates of a condition are whole, independent runs.
    for condition in contract['conditions']:
        has_block = condition['has_block']
        input_path = run_dir / 'inputs' / f'{"block" if has_block else "plain"}.json'
        write_json(input_path, [dict(id=item['case_id'], repository=REPOSITORY_LABELS[item['target']], prompt=item['prompt'],
                                     terms=item['terms'], first_query=item['baseline_query'],
                                     **({'related_terms_block': item['block_text']} if has_block else {}))
                                for item in population])
        for searcher in contract['searchers']:
            for replicate in range(1, contract['replicates'] + 1):
                job_id = f'{condition["name"]}-{searcher["key"]}-r{replicate:02d}'
                output_path = run_dir / 'outputs' / f'{job_id}.json'
                prompt_path = run_dir / 'jobs' / f'{job_id}.md'
                prompt_path.write_text(JOB_PROMPT.format(
                    block_field=TREATMENT_FIELD if has_block else PLAIN_FIELD, block_paragraph=TREATMENT_PARAGRAPH if has_block else '',
                    input_path=input_path, output_path=output_path, job_id=job_id,
                    queries=condition['queries_per_case'], words=contract['max_query_words']))
                jobs.append(dict(job_id=job_id, condition=condition['name'], searcher=searcher, replicate=replicate,
                                 queries_per_case=condition['queries_per_case'],
                                 prompt=display_path(prompt_path), input=display_path(input_path), output=display_path(output_path)))
    write_json(run_dir / 'plan.json', dict(
        schema='rt-agent-loop-plan/3', run_id=run_dir.name, searchers_contract=identity(SEARCHERS), sources=sources,
        population={target: sum(item['target'] == target for item in population) for target in sorted({i['target'] for i in population})},
        jobs=jobs))
    print(json.dumps(dict(run_dir=display_path(run_dir), cases=len(population), jobs=len(jobs)), ensure_ascii=False))


def clean_queries(raw: object, first_query: str, limit: int) -> tuple[list[str], collections.Counter]:
    """Lenient reading of one case's queries; returns the kept queries and the fixes applied."""
    fixes = collections.Counter()
    if not isinstance(raw, list):
        fixes['not_a_list'] += 1
        raw = []
    kept, seen = [], {first_query.strip().lower()}
    for query in raw:
        # A leading '--' would be read as an option by `rtpoc2 search`.
        text = re.sub(r'\s+', ' ', str(query)).strip().lstrip('-').strip()
        if not text:
            fixes['empty'] += 1
        elif text.lower() in seen:
            fixes['repeated'] += 1
        else:
            seen.add(text.lower())
            kept.append(text)
    if len(kept) > limit:
        fixes['over_limit'] += len(kept) - limit
        kept = kept[:limit]
    if len(kept) < limit:
        fixes['short'] += limit - len(kept)
    return kept, fixes


def query_tokens(text: str) -> set[str]:
    return set(text.lower().split())


def uses_block(query: str, item: dict) -> bool:
    """The query contains a related term of the block that is not already a word of the agent's terms."""
    own = set().union(*(query_tokens(term) for term in item['terms']))
    tokens = query_tokens(query)
    return any(query_tokens(word) <= tokens and not query_tokens(word) <= own for word in item['related_vocabulary'])


def compare(items: list[dict], jobs: list[dict], found: dict[tuple[str, str], bool]) -> dict:
    """Treatment vs control over one population: per case, the share of a condition's runs that find it."""
    by_condition = {condition: [job for job in jobs if job['condition'] == condition] for condition in ('treatment', 'control')}

    def rate(case_id: str, condition_jobs: list[dict]) -> float:
        return sum(found[(job['job_id'], case_id)] for job in condition_jobs) / len(condition_jobs)

    per_case = [dict(case_id=item['case_id'], **{f'found_rate_{c}': round(rate(item['case_id'], js), 3) for c, js in by_condition.items()})
                for item in items]
    rates = {c: sum(case[f'found_rate_{c}'] for case in per_case) / len(items) for c in by_condition}
    per_searcher = {}
    for key in sorted({job['searcher']['key'] for job in jobs}):
        searcher_rates = {c: sum(rate(item['case_id'], [j for j in js if j['searcher']['key'] == key]) for item in items) / len(items)
                          for c, js in by_condition.items()}
        run_rates = {c: sorted(round(sum(found[(j['job_id'], item['case_id'])] for item in items) / len(items), 3)
                               for j in js if j['searcher']['key'] == key) for c, js in by_condition.items()}
        per_searcher[key] = dict(found_rate={c: round(r, 4) for c, r in searcher_rates.items()},
                                 rate_difference=round(searcher_rates['treatment'] - searcher_rates['control'], 4),
                                 single_run_found_rates=run_rates)
    differences = [case['found_rate_treatment'] - case['found_rate_control'] for case in per_case]
    return dict(M=len(items), found_rate={c: round(r, 4) for c, r in rates.items()},
                rate_difference=round(rates['treatment'] - rates['control'], 4), net_recovery=round(sum(differences), 2),
                cases_treatment_better=sum(d > 0 for d in differences), cases_control_better=sum(d < 0 for d in differences),
                per_searcher=per_searcher,
                mechanical_rtl_paired=dict(treatment_found=sum(i['mechanical']['treatment_hit'] for i in items),
                                           control_found=sum(i['mechanical']['control_hit'] for i in items)),
                per_case=per_case)


def verdict(targets: dict[str, dict], criteria: dict) -> dict:
    threshold = criteria['effect']['min_rate_difference']
    effect = {}
    for target in criteria['targets']:
        comparison = targets.get(target)
        if comparison is None or comparison['M'] == 0:
            effect[target] = dict(status='not_measurable', M=comparison and comparison['M'])
            continue
        effect[target] = dict(status='pass' if comparison['rate_difference'] > threshold else 'fail', M=comparison['M'],
                              rate_difference=comparison['rate_difference'], threshold=threshold, found_rate=comparison['found_rate'])
    return dict(design=criteria['effect']['design'], rule=criteria['effect']['rule'], effect=effect,
                is_pass=all(entry['status'] == 'pass' for entry in effect.values()))


def score(run_dir: Path) -> None:
    plan = json.loads((run_dir / 'plan.json').read_text())
    population = json.loads((run_dir / 'population.json').read_text())
    criteria = json.loads(CRITERIA.read_text())
    missing = [job['job_id'] for job in plan['jobs'] if not (REPO_ROOT / job['output']).is_file()]
    if missing:
        raise SystemExit(f'missing job outputs: {missing}')
    source_by_run = {source['run']: source for source in plan['sources']}
    searchers: dict[tuple, Searcher] = {}
    (run_dir / 'raw').mkdir(exist_ok=True)

    def searcher_for(item: dict) -> Searcher:
        source = source_by_run[item['source_run']]
        key = (source['targets'][item['target']]['state'], source['search']['limit'], source['search']['top_k'])
        if key not in searchers:
            searchers[key] = Searcher(run_dir / 'raw', (REPO_ROOT / key[0]).parent, key[1], key[2])
            searchers[key].raw = run_dir / 'raw' / f'searches-{len(searchers)}.jsonl'
        return searchers[key]

    # Baseline tops come from the source runs, so follow-up searches must hit the same index.
    index_meta = {target['state']: target['index_meta_sha256'] for source in plan['sources'] for target in source['targets'].values()}

    def check_index(moment: str) -> None:
        changed = [state for state, sha in index_meta.items() if sha256_file(REPO_ROOT / state / 'index/meta.json') != sha]
        if changed:
            raise SystemExit(f'index differs from the source runs {moment} scoring: {changed}')

    check_index('before')
    planned, fixes_by_job = [], {}
    for job in plan['jobs']:
        output = json.loads((REPO_ROOT / job['output']).read_text())
        queries = output.get('queries', {}) if isinstance(output, dict) else {}
        fixes = collections.Counter()
        for item in population:
            if item['case_id'] not in queries:
                fixes['case_missing'] += 1
            kept, case_fixes = clean_queries(queries.get(item['case_id'], []), item['baseline_query'], job['queries_per_case'])
            fixes += case_fixes
            planned.append((job, item, kept))
        fixes_by_job[job['job_id']] = dict(fixes)
    distinct = {(searcher_for(item), item['target'], query) for _, item, kept in planned for query in kept}
    with ThreadPoolExecutor(SEARCH_WORKERS) as pool:
        list(pool.map(lambda search: search[0].top(search[1], search[2]), distinct))
    check_index('after')

    found, rows = {}, []
    usage = collections.defaultdict(collections.Counter)
    for job, item, kept in planned:
        gold = {normalize_path(path) for path in item['gold_paths']}
        tops = [searcher_for(item).top(item['target'], query) for query in kept]
        hit = bool(gold_found(set(item['baseline_top']).union(*map(set, tops)), item['gold_paths']))
        found[(job['job_id'], item['case_id'])] = hit
        counter = usage[job['condition']]
        counter['queries'] += len(kept)
        counter['suggested_verbatim'] += sum(query.lower() in {s.lower() for s in item['suggested_queries']} for query in kept)
        counter['uses_block_vocabulary'] += sum(uses_block(query, item) for query in kept)
        rows.append(dict(job_id=job['job_id'], case_id=item['case_id'], target=item['target'], condition=job['condition'],
                         searcher=job['searcher']['key'], replicate=job['replicate'], queries=kept, found=hit,
                         query_gold_ranks=[next((rank for rank, path in enumerate(top, 1) if path in gold), None) for top in tops]))
    (run_dir / 'results.jsonl').write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in rows))

    by_target = collections.defaultdict(list)
    for item in population:
        by_target[item['target']].append(item)
    targets = {target: compare(items, plan['jobs'], found) for target, items in sorted(by_target.items())}
    pooled = compare(population, plan['jobs'], found)
    pooled.pop('per_case')
    judged = verdict(targets, criteria)
    write_json(run_dir / 'summary.json', dict(
        schema='rt-agent-loop-summary/3', run_id=run_dir.name, searchers_contract=identity(SEARCHERS), criteria=identity(CRITERIA),
        sources=[source['run'] for source in plan['sources']],
        rule=criteria['effect']['measure'], verdict=judged, targets=targets, pooled=pooled,
        query_usage={condition: dict(counter, share_suggested_verbatim=round(counter['suggested_verbatim'] / max(counter['queries'], 1), 3),
                                     share_uses_block_vocabulary=round(counter['uses_block_vocabulary'] / max(counter['queries'], 1), 3))
                     for condition, counter in usage.items()},
        output_fixes={job_id: fixes for job_id, fixes in fixes_by_job.items() if fixes},
        searches=sum(searcher.calls for searcher in searchers.values())))
    print(json.dumps(dict(verdict={t: (e['status'], e.get('rate_difference')) for t, e in judged['effect'].items()},
                          pooled=dict(M=pooled['M'], found_rate=pooled['found_rate'], rate_difference=pooled['rate_difference'])),
                     ensure_ascii=False))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['prepare', 'score'])
    parser.add_argument('--run-id', required=True)
    parser.add_argument('--source-run', type=Path, action='append', help='prepare: an evaluate_feature.py run directory (repeatable)')
    parser.add_argument('--output-root', type=Path, default=OUTPUT_ROOT)
    args = parser.parse_args()
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', args.run_id):
        raise SystemExit('--run-id must be a plain name (letters, digits, . _ -)')
    run_dir = args.output_root.resolve() / args.run_id
    if args.command == 'prepare':
        if not args.source_run:
            raise SystemExit('prepare needs --source-run')
        prepare(run_dir, args.source_run)
    else:
        score(run_dir)


if __name__ == '__main__':
    main()
