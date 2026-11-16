# -*- coding: utf-8 -*-
"""Per-slot and per-arm facts of campaigns that run the other tools (no model calls, read only).

Usage: tools4_slots.py [--slots] <campaign folder> ...
Prints per (mode, arm): answers, fulfilled facts, tokens, calls per tool, basic-tool calls, guard rejections, output
limits, and whether any codebase-memory-mcp temporary entry was removed at cleanup (cross-campaign interference check).
--slots also prints one line per slot."""
import collections
import json
import sys
from pathlib import Path

SHOW = '--slots' in sys.argv[1:]


def removed_entries(value, found):
    if isinstance(value, dict):
        for key, item in value.items():
            if key == 'cbm_temporary_removed':
                found.append(item)
            else:
                removed_entries(item, found)
    elif isinstance(value, list):
        for item in value:
            removed_entries(item, found)


def usage(slot, key):
    item = (slot.get('usage') or {}).get(key) or {}
    return item.get('value') if item.get('validity') == 'valid' else None


for name in [arg for arg in sys.argv[1:] if not arg.startswith('--')]:
    root = Path(name)
    ledger = json.loads((root / 'ledger.json').read_text())
    manifest = json.loads((root / 'manifest.json').read_text())
    print(root.name, 'status', ledger['status'], 'scope', manifest['authorization']['scope']['kind'])
    groups = collections.defaultdict(lambda: collections.Counter())
    tools = collections.defaultdict(lambda: collections.Counter())
    cbm_records, cbm_removed = 0, []
    for slot in ledger['slots']:
        if slot.get('status') == 'out_of_scope':
            continue
        key = (slot['mode'], slot['arm'])
        out = groups[key]
        out['slots'] += 1
        out['completed'] += slot.get('status') == 'completed'
        out['valid'] += slot.get('conditions_valid') is True
        out['graded'] += slot.get('grade_state') == 'validated'
        fulfilled = facts = None
        if slot.get('grade_state') == 'validated':
            rows = json.loads(Path(slot['judgment']).read_text())['facts']
            fulfilled, facts = sum(row['state'] == 'fulfilled' for row in rows), len(rows)
            out['fulfilled'] += fulfilled
            out['facts'] += facts
            out['core'] += sum(row['level'] == 'core' and row['state'] == 'fulfilled' for row in rows)
            out['correct'] += sum(bool((row.get('correct') or {}).get('value')) for row in rows)
        total = usage(slot, 'total_tokens')
        out['tokens'] += total or 0
        attempts = slot.get('exploration_attempts') or {}
        out['calls'] += attempts.get('total') or 0
        out['relay_rejected'] += attempts.get('rejected') or 0
        out['errored'] += attempts.get('errored') or 0
        counts = slot.get('tool_call_counts') or {}
        for tool, count in counts.items():
            tools[key][tool] += count
        basic = slot.get('read_tool_calls')
        if basic is None:
            basic = sum(count for tool, count in counts.items() if tool.endswith(':command_execution'))
        out['basic'] += basic
        out['answers_using_basic'] += bool(basic)
        out['host_rejected'] += slot.get('read_tool_rejected_before_admission') or 0
        folder = Path(slot['folder']) if slot.get('folder') else None
        guard_rejected = 0
        if folder and (folder / 'read-calls.jsonl').exists():
            guard_rejected = sum(not json.loads(line)['dispatched'] for line in (folder / 'read-calls.jsonl').read_text().splitlines() if line.strip())
        out['guard_rejected'] += guard_rejected
        observed = slot.get('observations') or {}
        for field in ('read_tool_limit_reached', 'read_tool_content_truncated', 'codemode_output_truncated',
                      'adapter_output_guard_truncated', 'adapter_nested_content_truncated'):
            out[field] += observed.get(field) or 0
        if slot['arm'] == 'codebase-memory-mcp' and folder:
            for path in list(folder.glob('*.json')) + list(folder.glob('*/*.json')):
                try:
                    found = []
                    removed_entries(json.loads(path.read_text()), found)
                except (ValueError, OSError):
                    continue
                cbm_records += len(found)
                cbm_removed += [(slot['id'], path.name, item) for item in found if item]
        if SHOW:
            print(f"   {slot['id']}: {slot.get('status')}/{slot.get('grade_state')} valid={slot.get('conditions_valid')} "
                  f"fulfilled={fulfilled}/{facts} tokens={total} calls={attempts.get('total')} basic={basic} "
                  f"guard_rejected={guard_rejected} relay_rejected={attempts.get('rejected')} tools={dict(sorted(counts.items()))} "
                  f"errors={slot.get('error') or slot.get('condition_errors')}")
    for key in sorted(groups):
        out = groups[key]
        n = out['slots']
        print('  ', key, {'slots': n, 'completed': out['completed'], 'valid': out['valid'], 'graded': out['graded'],
                          'fulfilled': f"{out['fulfilled']}/{out['facts']}", 'core': out['core'], 'correct': out['correct'],
                          'tokens/ans': round(out['tokens'] / n), 'calls/ans': round(out['calls'] / n, 1),
                          'basic/ans': round(out['basic'] / n, 1), 'answers_using_basic': out['answers_using_basic'],
                          'guard_rejected': out['guard_rejected'], 'relay_rejected': out['relay_rejected'], 'errored': out['errored'],
                          'limits': {field: out[field] for field in ('read_tool_limit_reached', 'read_tool_content_truncated',
                                                                    'codemode_output_truncated', 'adapter_output_guard_truncated',
                                                                    'adapter_nested_content_truncated') if out[field]}})
        print('      tools/ans', {tool: round(count / n, 1) for tool, count in sorted(tools[key].items())})
    print('   codebase-memory-mcp cleanup records', cbm_records, 'with removed entries', len(cbm_removed), cbm_removed[:5])
