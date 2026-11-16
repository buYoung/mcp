"""Operator helpers for a smoke or benchmark run on the harness: any cache, any arms, no model calls.

  pin <tag> <full commit>
      Point the harness subject (codemap-search) at a tagged commit: edits targets.lock.json, its line in SHA256SUMS,
      TARGET_LOCK_SHA256 in route_contract.py and the subject assertion in checks.py. The tag must resolve to the commit.
  inputs <cache> <tag> <arms: all | comma list> <modes: direct | codemode | direct,codemode> [--full] [--check] [--slots=<regex>]
      Write the run approval of each solver (operator-input/run-<tag>-<model>.json) for the chosen arms and modes of the
      prepared plans. Never overwrites. --check compares with the existing files instead (the actor text is ignored).
      The approval is a smoke scope naming the chosen slots. --full writes the full scope instead (a comparison
      campaign); the chosen arms and modes must then cover every slot of the plan. --slots keeps only the slot ids the
      regular expression matches (for a small smoke, e.g. --slots='^git-sync-job-dispatch-.*-1$').
  state <cache> <tag> <model>
      Print "<kind> <campaign folder>" for the campaign started from that approval; kind is none, completed, review
      (answers wait for flag review) or unfinished.
  review <cache> <campaign folder> <tag>
      Write approve-review inputs for flags that are known Grafana code values and print their paths. Any other flag
      stops the command (exit 3) and nothing is written.
  report <cache> <tag> [--by-mode]   |   report <cache> --campaigns [--by-mode] [--label=<name>] <campaign folder> ...
      Aggregate graded answers per model and arm and compare codemap-search with every other arm, and every other arm
      with rg (fulfilled facts: exact two-sided permutation p with arm labels shuffled within each question; tokens:
      ratio). Writes bench_report-<cache>-<tag or label or "campaigns">.json into benchmark/results. --by-mode keeps each
      measurement mode apart (groups "<model> <mode>", file suffix -by-mode) and adds the calls to the agent's own basic
      tools per answer (pi read/grep/find/ls or, in older campaigns, bash; shell commands for the CLI harness).
      A campaign folder may be an absolute path, so campaigns of another cache can be pooled; that comparison crosses
      batches and must be reported as such.

The cache is a folder name under /Volumes/Samsung_990_PRO. pin and inputs work on
the harness BENCH_CLIENT names (pi or codex) in the checkout that holds this file. BENCH_MODELS (space separated)
limits inputs and report to those solver models; the default is both."""
import hashlib
import itertools
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path('/Volumes/Samsung_990_PRO')
REPO = Path(__file__).resolve().parents[2]
OUT = REPO / 'benchmark/results'
POINTERS = {'gpt-6.1-sol': '07-readiness.json', 'claude-opus-5-5': '07-readiness-claude-opus-5-5.json'}
MODELS = [model for model in POINTERS if model in (os.environ.get('BENCH_MODELS') or ' '.join(POINTERS)).split()]
SUBJECT = 'codemap-search'
BASELINE = 'rg'
CITE = re.compile(r'([A-Za-z0-9_.@\-]+(?:/[A-Za-z0-9_.@\-]+)+\.[A-Za-z0-9]+):(\d+)(?:-(\d+))?')
TEST = re.compile(r'(?i)^(?:(.*/)?(tests|test|benches|bench|__tests__)/.*|(.*/)?(test_[^/]*|[^/.]*_test([.][^/]*)?|[^/]*[.](test|spec)[.][^/]*))$')
GENERATED = re.compile(r'^(?:(.*/)?(generated(/.*)?|[^/]*(_gen|_generated|[.]gen|[.]generated|[.]g|[.]pb)[.][^/]+))$')
# Flags verified as Grafana code values of the v6 questions.
SUFFIX = ' 풀이 모델의 성능·토큰 사용량 자기 보고가 아님.'
FETCH = ('Grafana 코드 값: pkg/services/ngalert/notifier/external_am_syncer.go:587 '
         'fetchCtx, cancel := context.WithTimeout(ctx, 10*time.Second).')
JOBS = ('Grafana 코드 값: pkg/registry/apis/provisioning/register.go:954 '
        'WithTimeout(NewJobsConnector(...), 30*time.Second).')
AMPOLL = ('Grafana 코드 값: pkg/setting/setting_unified_alerting.go:27 alertmanagerDefaultConfigPollInterval = time.Minute '
          '(conf/defaults.ini:1532 alertmanager_config_poll_interval = 60s).')
POLL = ('Grafana 코드 값: public/app/features/alerting/unified/utils/constants.ts:20 '
        'AUTO_SYNC_CONFIG_POLL_INTERVAL_MS = 30000 (useAutoSyncConfiguration.tsx:56 pollingInterval).')


def stop(message, code=1):
    print(message, file=sys.stderr)
    sys.exit(code)


def harness():
    client = os.environ.get('BENCH_CLIENT')
    if client not in ('pi', 'codex'):
        stop('set BENCH_CLIENT to pi or codex')
    return REPO / 'benchmark' / client


def cache_path(name):
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', name):
        stop(f'cache must be a plain folder name: {name!r}')
    return ROOT / name


def check_tag(tag):
    if not re.fullmatch(r'[a-z0-9][a-z0-9-]*', tag):
        stop(f'tag must be lowercase letters, digits and hyphens: {tag!r}')
    return tag


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def replace_once(text, pattern, replacement, what):
    result, count = re.subn(pattern, replacement, text)
    if count != 1:
        stop(f'{what}: expected one match, found {count}')
    return result


def apply_pin(root, tag, commit):
    """Edit the four pin files under a harness folder. Returns the new lock sha256."""
    targets = Path(root) / 'data/grafana-comparison-targets-v1'
    lock = targets / 'targets.lock.json'
    text = lock.read_text()
    start = text.index('"subject": {')
    end = text.index('}', start)
    block = text[start:end]
    version = re.search(r'"version": "([^"]+)"', block).group(1)
    block = replace_once(block, r'"git_ref": "[^"]+"', f'"git_ref": "{tag}"', 'lock git_ref')
    block = replace_once(block, r'"commit": "[0-9a-f]{40}"', f'"commit": "{commit}"', 'lock commit')
    lock.write_text(text[:start] + block + text[end:])
    digest = sha256(lock)
    sums = targets / 'SHA256SUMS'
    sums.write_text(replace_once(sums.read_text(), r'(?m)^[0-9a-f]{64}(  targets\.lock\.json)$', digest + r'\1', 'SHA256SUMS line'))
    contract = Path(root) / 'route_contract.py'
    contract.write_text(replace_once(contract.read_text(), r'(?m)^TARGET_LOCK_SHA256 = "[0-9a-f]{64}"$',
                                     f'TARGET_LOCK_SHA256 = "{digest}"', 'TARGET_LOCK_SHA256'))
    checks = Path(root) / 'checks.py'
    checks.write_text(replace_once(checks.read_text(), r'\("' + re.escape(version) + r'", "[^"]+", "[0-9a-f]{40}"\)\)',
                                   f'("{version}", "{tag}", "{commit}"))', 'checks.py subject assertion'))
    return digest


def pin(tag, commit):
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        stop('give the full 40-character commit')
    resolved = subprocess.run(['git', '-C', str(REPO), 'rev-parse', f'{tag}^{{commit}}'], capture_output=True, text=True)
    if resolved.returncode != 0 or resolved.stdout.strip() != commit:
        stop(f'tag {tag} does not resolve to {commit} in {REPO} (create it first: git tag {tag} {commit})')
    print('targets.lock.json sha256', apply_pin(harness(), tag, commit))
    print('edited: targets.lock.json, SHA256SUMS, route_contract.py, checks.py. Review the diff, commit, then prepare.')


def plan_record(cache, model):
    pointer = harness() / 'evidence' / POINTERS[model]
    if not pointer.exists():
        stop(f'{model}: no readiness pointer at {pointer} (prepare this cache with this harness first)')
    record = json.loads(pointer.read_text())
    readiness = cache / 'route-preparation/plans' / record['plan_id'] / 'readiness.json'
    if not readiness.exists() or json.loads(readiness.read_text()) != record:
        stop(f'{model}: the harness readiness pointer and the plan in {cache.name} differ (prepare this cache first)')
    if record['models']['solver']['model'] != model:
        stop(f'{model}: pointer is for {record["models"]["solver"]["model"]}')
    return record, readiness


def inputs(cache, tag, arms, modes, check, full=False, pattern=None):
    out = cache / 'operator-input'
    for model in MODELS:
        record, readiness = plan_record(cache, model)
        planned = record['planned_runs']
        chosen = sorted({row['arm'] for row in planned}) if arms == ['all'] else arms
        if set(chosen) - set(record['arms']):
            stop(f'{model}: arms not in the plan: {sorted(set(chosen) - set(record["arms"]))}; plan has {record["arms"]}')
        keep = (lambda ident: True) if pattern is None else (lambda ident: re.search(pattern, ident) is not None)
        slots = [row['id'] for row in planned if row['arm'] in chosen and row['mode'] in modes and keep(row['id'])]
        counts = {arm: sum(row['arm'] == arm and row['mode'] in modes and keep(row['id']) for row in planned) for arm in chosen}
        if not slots or min(counts.values()) == 0 or set(modes) - {row['mode'] for row in planned}:
            stop(f'{model}: no slots for some arm or mode: {counts}, modes {modes}')
        approval = {'plan_id': record['plan_id'], 'readiness': str(readiness), 'readiness_sha256': sha256(readiness),
                    'solver_model': model,
                    'actor': f"operator (user-approved Grafana v6 run '{tag}': arms {','.join(chosen)}; modes "
                             f"{','.join(modes)}; exactly {len(slots)} solver runs for {model})",
                    'confirmed': True, 'scope': {'kind': 'smoke', 'slot_ids': slots}}
        if full:
            if len(slots) != len(planned):
                stop(f'{model}: --full needs every slot of the plan; chosen {len(slots)} of {len(planned)}')
            approval['scope'] = {'kind': 'full'}
        path = out / f'run-{tag}-{model}.json'
        if check:
            existing = json.loads(path.read_text())
            same = {k: v for k, v in existing.items() if k != 'actor'} == {k: v for k, v in approval.items() if k != 'actor'}
            print(model, 'same as existing' if same else 'DIFFERENT from existing', len(slots), counts, path)
            if not same:
                sys.exit(1)
        else:
            out.mkdir(parents=True, exist_ok=True)
            with path.open('x') as stream:
                json.dump(approval, stream, indent=1)
            print(model, record['plan_id'], len(slots), counts, path)


def find_campaigns(cache, tag, model):
    approval = json.loads((cache / 'operator-input' / f'run-{tag}-{model}.json').read_text())
    found = []
    for folder in sorted((cache / 'route-preparation/campaigns').glob('*')):
        manifest = folder / 'manifest.json'
        if not manifest.exists():
            continue
        granted = json.loads(manifest.read_text())['authorization']
        if all(granted.get(key) == approval[key] for key in ('actor', 'plan_id', 'readiness_sha256', 'solver_model', 'scope')):
            found.append(folder)
    return found


def campaign_kind(folder):
    ledger = json.loads((folder / 'ledger.json').read_text())
    slots = [slot for slot in ledger['slots'] if slot.get('status') != 'out_of_scope']
    if ledger.get('status') == 'completed' and all(slot.get('grade_state') == 'validated' for slot in slots):
        return 'completed'
    if all(slot.get('phase') == 'terminal' for slot in slots) and any(slot.get('grade_state') == 'review_required' for slot in slots):
        return 'review'
    return 'unfinished'


def state(cache, tag, model):
    found = find_campaigns(cache, tag, model)
    if len(found) > 1:
        stop(f'more than one campaign for run-{tag}-{model}.json: {[f.name for f in found]}')
    print(f'{campaign_kind(found[0])} {found[0].name}' if found else 'none -')


def known_evidence(slot_id, flag):
    rules, text, context = flag['rules'], flag['text'], flag['context']
    if rules != ['performance_self_report']:
        return None
    if text == '10초' and slot_id.startswith('alertmanager-config-autosync-') \
            and any(word in context for word in ('timeout', '타임아웃', '제한')) \
            and any(word in context for word in ('fetch', '프록시', 'proxy', 'Proxy', '가져오', '가져옵', '조회', 'ExtraConfig')):
        return FETCH
    if text == '30초' and slot_id.startswith('alertmanager-config-autosync-') and (
            any(word in context for word in ('polling', '폴링'))
            or '간격' in context and any(word in context for word in ('Config/default', '화면'))):
        return POLL
    if text == '60초' and slot_id.startswith('alertmanager-config-autosync-') and 'PollInterval' in context:
        return AMPOLL
    if text == '30초' and slot_id.startswith('git-sync-job-dispatch-') and 'timeout' in context and 'Jobs' in context:
        return JOBS
    return None


def review(cache, campaign, tag):
    folder = cache / 'route-preparation/campaigns' / campaign
    ledger = json.loads((folder / 'ledger.json').read_text())
    approvals = []
    for slot in ledger['slots']:
        if slot.get('grade_state') != 'review_required':
            continue
        draft = json.loads(Path(slot['review']).read_text())
        decisions = []
        for flag in draft['flags']:
            reason = known_evidence(slot['id'], flag)
            if reason is None:
                stop(f"{slot['id']}: flag {flag['id']} {flag['rules']} [{flag['text']}] needs a person: {flag['context'][:200]!r}", 3)
            decisions.append({'flag_id': flag['id'], 'decision': 'legitimate_source_identifier', 'evidence': reason + SUFFIX})
        approvals.append((f"review-{tag}-{slot['model']}-{slot['id']}-r{draft['revision']}.json", {
            'run_root': str(folder), 'slot_id': slot['id'], 'approval': {
                'reviewer': 'operator (bench_ops.py review: known Grafana code values)', 'revision': draft['revision'],
                'raw_sha256': draft['raw_sha256'], 'draft_sha256': draft['draft_sha256'], 'decisions': decisions}}))
    for name, approval in approvals:
        path = cache / 'operator-input' / name
        text = json.dumps(approval, ensure_ascii=False, indent=1)
        if path.exists():
            if path.read_text() != text:
                stop(f'{path} exists with other content')
        else:
            with path.open('x') as stream:
                stream.write(text)
            os.chmod(path, 0o600)
        print(path)


def usage_value(usage, key):
    item = usage.get(key) or {}
    return item.get('value') if item.get('validity') == 'valid' else None


def permutation(a, b):
    """a, b: lists of (question, score). Returns (mean difference, two-sided p, one-sided p for a > b)."""
    strata = []
    for question in sorted({q for q, _ in a + b}):
        values = [v for q, v in a if q == question] + [v for q, v in b if q == question]
        k = sum(q == question for q, _ in a)
        strata.append([(sum(values[i] for i in pick), sum(values)) for pick in itertools.combinations(range(len(values)), k)])
    na, nb = len(a), len(b)
    observed = sum(v for _, v in a) / na - sum(v for _, v in b) / nb
    diffs = []
    for combo in itertools.product(*strata):
        chosen = sum(c for c, _ in combo)
        diffs.append(chosen / na - (sum(t for _, t in combo) - chosen) / nb)
    two = sum(abs(d) >= abs(observed) - 1e-12 for d in diffs) / len(diffs)
    one = sum(d >= observed - 1e-12 for d in diffs) / len(diffs)
    return observed, two, one


def report(cache, label, folders, by_mode=False):
    result = {}
    for campaign in folders:
        ledger = json.loads((campaign / 'ledger.json').read_text())
        for slot in ledger['slots']:
            if slot.get('status') == 'out_of_scope':
                continue
            group = f"{slot['model']} {slot['mode']}" if by_mode else slot['model']
            out = result.setdefault(group, {}).setdefault(slot['arm'], {
                'campaigns': [], 'answers': 0, 'ungraded': [], 'fulfilled': 0, 'facts': 0, 'core': 0, 'core_fulfilled': 0,
                'extended': 0, 'extended_fulfilled': 0, 'zero_core': 0, 'total': 0, 'uncached': 0, 'calls': 0, 'shell': 0,
                'citing_test_or_generated': 0, 'per_question': {}, 'scores': []})
            if campaign.name not in out['campaigns']:
                out['campaigns'].append(campaign.name)
            if slot.get('grade_state') != 'validated':
                out['ungraded'].append((campaign.name, slot['id'], slot.get('grade_state'), slot.get('status')))
                continue
            facts = json.loads(Path(slot['judgment']).read_text())['facts']
            fulfilled = sum(f['state'] == 'fulfilled' for f in facts)
            core = sum(f['level'] == 'core' and f['state'] == 'fulfilled' for f in facts)
            total, cached = usage_value(slot['usage'], 'total_tokens'), usage_value(slot['usage'], 'cached_input_tokens')
            out['answers'] += 1
            out['fulfilled'] += fulfilled
            out['facts'] += len(facts)
            out['core'] += sum(f['level'] == 'core' for f in facts)
            out['core_fulfilled'] += core
            out['extended'] += sum(f['level'] != 'core' for f in facts)
            out['extended_fulfilled'] += fulfilled - core
            out['zero_core'] += core == 0
            out['total'] += total
            out['uncached'] += total - cached
            out['calls'] += (slot.get('exploration_attempts') or {}).get('total') or 0
            # Calls to the agent's own basic tools (key kept as 'shell'): the pi harness records read_tool_calls
            # (bash_calls before 2026-11-15), the CLI harness counts command_execution rows.
            basic = slot.get('read_tool_calls') if slot.get('read_tool_calls') is not None else slot.get('bash_calls')
            out['shell'] += basic if basic is not None else sum(
                count for tool, count in (slot.get('tool_call_counts') or {}).items() if tool.endswith(':command_execution'))
            answer = Path(slot['folder']) / 'answer.txt'
            cited = {m.group(1) for m in CITE.finditer(answer.read_text())} if answer.exists() else set()
            out['citing_test_or_generated'] += any(TEST.match(p) or GENERATED.match(p) for p in cited)
            q = out['per_question'].setdefault(slot['question_id'], {'fulfilled': 0, 'core': 0, 'facts': 0, 'max': 0, 'answers': 0, 'runs': []})
            q['fulfilled'] += fulfilled
            q['core'] += core
            q['facts'] += len(facts)
            q['answers'] += 1
            q['max'] = max(q['max'], fulfilled)
            q['runs'].append((slot['attempt'], fulfilled))
            out['scores'].append((slot['question_id'], fulfilled))
    for arms in result.values():
        subject = arms.get(SUBJECT)
        if not subject or not subject['answers']:
            continue
        subject['vs'] = {}
        for arm, other in arms.items():
            if arm == SUBJECT or not other['answers']:
                continue
            diff, two, one = permutation(subject['scores'], other['scores'])
            subject['vs'][arm] = {'mean_difference': diff, 'p_two_sided': two, 'p_one_sided_better': one,
                                  'token_ratio': (subject['total'] / subject['answers']) / (other['total'] / other['answers'])}
    for arms in result.values():
        baseline = arms.get(BASELINE)
        if not baseline or not baseline['answers']:
            continue
        for arm, other in arms.items():
            if arm in (SUBJECT, BASELINE) or not other['answers']:
                continue
            diff, two, one = permutation(other['scores'], baseline['scores'])
            other['vs_baseline'] = {'mean_difference': diff, 'p_two_sided': two, 'p_one_sided_better': one,
                                    'token_ratio': (other['total'] / other['answers']) / (baseline['total'] / baseline['answers'])}
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / f"bench_report-{cache.name}-{label}{'-by-mode' if by_mode else ''}.json").write_text(json.dumps(result, ensure_ascii=False, indent=1))
    for model, arms in result.items():
        print(f'### {model}')
        for arm, a in arms.items():
            n = a['answers'] or 1
            questions = ' '.join(f"{q.split('-')[0]}={v['fulfilled']}/{v['facts']}(core {v['core']}, max {v['max']}, runs {sorted(v['runs'])})"
                                 for q, v in sorted(a['per_question'].items()))
            print(f"{arm}: answers={a['answers']} ungraded={len(a['ungraded'])} fulfilled={a['fulfilled']}/{a['facts']} "
                  f"core={a['core_fulfilled']}/{a['core']} ext={a['extended_fulfilled']}/{a['extended']} zero_core={a['zero_core']} "
                  f"{questions} total/ans={a['total'] / n:,.0f} uncached/ans={a['uncached'] / n:,.0f} calls={a['calls'] / n:.1f} "
                  f"citing_tg={a['citing_test_or_generated']}/{a['answers']} ungraded_rows={a['ungraded']}"
                  + (f" basic/ans={a['shell'] / n:.1f}" if by_mode else ''))
        for arm, v in (arms.get(SUBJECT) or {}).get('vs', {}).items():
            print(f"{SUBJECT} vs {arm}: fulfilled {v['mean_difference']:+.2f}/answer (p two-sided {v['p_two_sided']:.4f}), "
                  f"tokens x{v['token_ratio']:.2f}")
        for arm, a in arms.items():
            if 'vs_baseline' in a:
                v = a['vs_baseline']
                print(f"{arm} vs {BASELINE}: fulfilled {v['mean_difference']:+.2f}/answer (p two-sided {v['p_two_sided']:.4f}), "
                      f"tokens x{v['token_ratio']:.2f}")


def main(argv):
    if len(argv) < 2 or argv[1] not in ('pin', 'inputs', 'state', 'review', 'report'):
        stop(__doc__)
    command, args = argv[1], argv[2:]
    if command == 'pin' and len(args) == 2:
        pin(args[0], args[1])
    elif command == 'inputs' and len(args) >= 4 and len(set(args[4:])) == len(args[4:]) \
            and all(option in ('--check', '--full') or option.startswith('--slots=') for option in args[4:]) \
            and sum(option.startswith('--slots=') for option in args[4:]) <= 1:
        pattern = next((option[len('--slots='):] for option in args[4:] if option.startswith('--slots=')), None)
        inputs(cache_path(args[0]), check_tag(args[1]), args[2].split(','), args[3].split(','), '--check' in args[4:],
               '--full' in args[4:], pattern)
    elif command == 'state' and len(args) == 3 and args[2] in POINTERS:
        state(cache_path(args[0]), check_tag(args[1]), args[2])
    elif command == 'review' and len(args) == 3:
        review(cache_path(args[0]), args[1], check_tag(args[2]))
    elif command == 'report' and len(args) >= 3 and args[1] == '--campaigns':
        cache = cache_path(args[0])
        options = [arg for arg in args[2:] if arg.startswith('--')]
        names = [arg for arg in args[2:] if not arg.startswith('--')]
        if not names or any(option != '--by-mode' and not option.startswith('--label=') for option in options):
            stop(__doc__)
        label = next((check_tag(option[len('--label='):]) for option in options if option.startswith('--label=')), 'campaigns')
        report(cache, label, [Path(name) if name.startswith('/') else cache / 'route-preparation/campaigns' / name for name in names],
               by_mode='--by-mode' in options)
    elif command == 'report' and (len(args) == 2 or len(args) == 3 and args[2] == '--by-mode'):
        cache, tag = cache_path(args[0]), check_tag(args[1])
        folders = [folder for model in MODELS if (cache / 'operator-input' / f'run-{tag}-{model}.json').exists()
                   for folder in find_campaigns(cache, tag, model)]
        if not folders:
            stop(f'no campaign for tag {tag} in {cache.name}')
        report(cache, tag, folders, by_mode=len(args) == 3)
    else:
        stop(__doc__)


if __name__ == '__main__':
    main(sys.argv)
