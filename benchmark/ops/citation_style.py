"""Per arm and mode: how the grader split facts into correct/supported, and how answers cite lines. Read-only.
Usage: citation_style.py <campaign folder> [...]"""
import collections
import json
import pathlib
import re
import sys

RANGE = re.compile(r'\.[A-Za-z]{1,5}:(\d+)\s*[-–]\s*(\d+)')
SINGLE = re.compile(r'\.[A-Za-z]{1,5}:(\d+)(?![\d]*\s*[-–]\s*\d)')
for arg in sys.argv[1:]:
    folder = pathlib.Path(arg)
    ledger = json.loads((folder / 'ledger.json').read_text())
    table = collections.defaultdict(collections.Counter)
    for slot in ledger['slots']:
        if slot.get('status') == 'out_of_scope' or slot.get('grade_state') != 'validated':
            continue
        key = (slot['model'], slot['mode'], slot['arm'])
        for fact in json.loads(pathlib.Path(slot['judgment']).read_text())['facts']:
            correct, supported = fact['correct']['value'], fact['supported']['value']
            table[key][f"correct={correct} supported={supported}"] += 1
            table[key]['fulfilled'] += fact['state'] == 'fulfilled'
        answer = (pathlib.Path(slot['folder']) / 'answer.txt').read_text()
        ranges = RANGE.findall(answer)
        table[key]['answers'] += 1
        table[key]['range citations'] += len(ranges)
        table[key]['range lines'] += sum(int(b) - int(a) + 1 for a, b in ranges)
        table[key]['single-line citations'] += len(SINGLE.findall(answer))
        table[key]['markdown links to absolute paths'] += len(re.findall(r'\]\(/', answer))
        table[key]['answer chars'] += len(answer)
    print(folder.name)
    for key, counts in sorted(table.items()):
        print('  ', key, dict(sorted(counts.items())))
