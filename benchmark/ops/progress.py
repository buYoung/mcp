"""Print the in-scope slot states of the newest campaign in each given cache. Read-only.
Usage: progress.py <cache name> [...]"""
import collections
import json
import pathlib
import sys

for cache in sys.argv[1:]:
    folders = sorted(pathlib.Path('/Volumes/Samsung_990_PRO', cache, 'route-preparation/campaigns').glob('*/ledger.json'),
                     key=lambda p: p.stat().st_mtime)
    if not folders:
        print(cache, 'no campaign yet')
        continue
    led = json.loads(folders[-1].read_text())
    slots = [s for s in led['slots'] if s.get('status') != 'out_of_scope']
    print(cache, folders[-1].parent.name, 'status', led.get('status'), 'in-scope', len(slots))
    print('  ', dict(collections.Counter(f"{s.get('phase')}/{s.get('status')}/{s.get('grade_state')}" for s in slots)))
    for s in slots:
        if s.get('status') not in ('planned', None) and (s.get('status') != 'completed' or s.get('conditions_valid') is not True or s.get('error')):
            print('   !', s['id'], s.get('phase'), s.get('status'), 'valid', s.get('conditions_valid'), 'eligible', s.get('answer_eligible'),
                  'err', str(s.get('error'))[:200], str(s.get('condition_errors'))[:200])
    done = [s for s in slots if s.get('status') == 'completed']
    for s in done[-4:]:
        usage = s.get('usage') or {}
        total = (usage.get('total_tokens') or {}).get('value')
        print('   ok', s['id'], 'calls', (s.get('exploration_attempts') or {}).get('total'), 'tools', s.get('tool_call_counts'),
              'tokens', total, 'sec', round(s.get('elapsed_seconds') or 0))
