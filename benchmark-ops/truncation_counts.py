"""Per arm and mode: how often a tool result reached the model (or a script) cut short. Read-only.
pi campaigns: the harness counters in each ledger slot. CLI campaigns: tool outputs in rollout.jsonl that start the
client's own "Warning: truncated output" notice.
Usage: truncation_counts.py <campaign folder> [...]"""
import collections
import json
import pathlib
import sys

PI_KEYS = ('read_tool_content_truncated', 'read_tool_limit_reached', 'bash_content_truncated',
           'bash_nested_structured_content_truncated', 'codemode_output_truncated', 'adapter_output_guard_truncated',
           'adapter_nested_content_truncated')

for folder in map(pathlib.Path, sys.argv[1:]):
    ledger = json.loads((folder / 'ledger.json').read_text())
    groups = collections.defaultdict(lambda: collections.Counter())
    for slot in ledger['slots']:
        if slot.get('status') == 'out_of_scope':
            continue
        out = groups[(slot['mode'], slot['arm'])]
        out['answers'] += 1
        counts = {}
        if slot.get('backend') == 'pi':
            observed = slot.get('observations') or {}
            counts = {key: observed[key] for key in PI_KEYS if observed.get(key)}
        else:
            rollout = pathlib.Path(slot['folder']) / 'rollout.jsonl'
            cut = 0
            for line in rollout.read_text(errors='replace').splitlines() if rollout.exists() else []:
                try:
                    payload = json.loads(line).get('payload') or {}
                except ValueError:
                    continue
                if payload.get('type') in ('function_call_output', 'custom_tool_call_output'):
                    output = payload.get('output')
                    text = output if isinstance(output, str) else json.dumps(output, ensure_ascii=False)
                    cut += 'Warning: truncated output' in text
            counts = {'cli_output_truncated': cut} if cut else {}
        for key, value in counts.items():
            out[key] += value
        out['answers_with_any'] += bool(counts)
    print(folder.name)
    for key in sorted(groups):
        print('  ', key, dict(groups[key]))
