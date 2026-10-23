"""Independently rebuild name totals from file evidence and check comparison denominators."""
import collections as C
import json
from pathlib import Path
import sys
from collect_full import REPOS, dump
from word_eval_paths import work_dir


def validate(root):
    results = []
    for name in REPOS:
        path = root/name
        summary = json.loads((path/'summary.json').read_text())
        manifest = json.loads((path/'manifest.json').read_text())
        candidates = {x['file']:x for x in manifest['candidates']}
        analyzed = {x['file']:x for x in manifest['analyzed']}
        assert len(candidates) == summary['candidate_files']
        assert sum(x['lines'] for x in candidates.values()) == summary['candidate_lines']
        assert len(analyzed) == summary['analyzed_files']
        assert sum(x['lines'] for x in analyzed.values()) == summary['analyzed_lines']
        assert not manifest['unreadable'], 'Unreadable files prevent COMPLETE'
        assert candidates.keys() == analyzed.keys()
        assert len(manifest['parse_errors']) == summary['parse_error_files']
        totals = {}
        sample = json.loads((path/'sample-audit.json').read_text())
        sample_files = {x['file'] for x in sample['files'] if x['candidate']}
        sample_counts = C.Counter()
        for line in (path/'file-names.jsonl').open():
            row = json.loads(line)
            key = (row['channel'], row['name'])
            assert row['count'] > 0
            assert row['file'] in analyzed
            assert row['directory'] == str(Path(row['file']).parent)
            assert row['is_test'] == analyzed[row['file']]['is_test']
            if row['channel'] == 'tag_value':
                assert not row['is_test']
                assert row['name'] and row['name'] != '-' and not any(c.isspace() for c in row['name'])
            a = totals.setdefault(key, {'count':0,'test_count':0,'files':set(),'dirs':set(),'quals':C.Counter()})
            assert row['file'] not in a['files'], 'duplicate per-file evidence'
            a['count'] += row['count']
            a['test_count'] += row['count'] if row['is_test'] else 0
            a['files'].add(row['file'])
            a['dirs'].add(row['directory'])
            a['quals'].update(row['qualifiers'])
            if row['file'] in sample_files:
                sample_counts[key] += row['count']
        seen = set()
        for line in (path/'names.jsonl').open():
            row = json.loads(line)
            key = (row['channel'],row['name'])
            assert key not in seen
            seen.add(key)
            a = totals[key]
            assert row['count'] == a['count']
            assert row['test_count'] == a['test_count']
            assert row['is_test_only'] == (a['count'] == a['test_count'])
            assert row['file_count'] == len(a['files'])
            assert row['directory_count'] == len(a['dirs'])
            assert row['qualifiers'] == a['quals']
            assert len({x['file'] for x in row['examples']}) == len(row['examples'])
            assert all(x['file'] in a['files'] for x in row['examples'])
        assert seen == totals.keys()
        legacy = json.loads((work_dir()/name/'unit-input.json').read_text())
        channels = {'identifier':{x['name'] for x in legacy if x['where'] != 'struct_tag'}, 'tag_value':{x['name'] for x in legacy if x['where'] == 'struct_tag'}}
        if (work_dir()/name/'tag-input.json').exists():
            channels['tag_value'] |= {x['name'] for x in json.loads((work_dir()/name/'tag-input.json').read_text())}
        for channel, sample_names in channels.items():
            full = {n for c,n in totals if c == channel}
            shared = full & sample_names
            total = sum(a['count'] for (c,n),a in totals.items() if c == channel)
            weighted = sum(totals[channel,n]['count'] for n in shared)
            s = summary['comparison'][channel]
            assert s['full_unique'] == len(full)
            assert s['legacy_sample_unique'] == len(sample_names)
            assert s['legacy_sample_in_full'] == len(shared)
            assert s['new_names'] == len(full-sample_names)
            assert s['test_only_names'] == sum(a['count'] == a['test_count'] for (c,n),a in totals.items() if c == channel)
            assert s['full_occurrences'] == total
            assert s['full_occurrences_of_sample_names'] == weighted
            assert s['sample_in_full_ratio'] == (len(shared)/len(sample_names) if sample_names else None)
            assert s['full_in_sample_type_ratio'] == (len(shared)/len(full) if full else None)
            assert s['full_in_sample_occurrence_ratio'] == (weighted/total if total else None)
            assert s['sample_reparsed_occurrences'] == sum(v for (c,n),v in sample_counts.items() if c == channel)
            assert s['sample_reparsed_unique'] == sum(c == channel for c,n in sample_counts)
            missing = set(json.loads((path/f'{channel}-differences.json').read_text())['missing_legacy_sample_names'])
            assert missing == sample_names-full
            if channel == 'identifier':
                excluded_names = {n for x in sample['files'] if not x['candidate'] for n in x['identifier_names']}
                assert missing <= excluded_names, f'Unexplained sample missing names: {missing-excluded_names}'
        assert sum(x['lines'] for x in sample['files']) == summary['sample_lines']
        if sample['expected_lines'] is not None:
            assert sample['expected_lines'] == summary['sample_lines']
        probe = next(x for x in json.loads((work_dir()/'full-scale-probe.json').read_text()) if x['repository'] == name)
        assert probe['candidate_files'] == summary['candidate_files']
        assert probe['candidate_lines'] == summary['candidate_lines']
        assert probe['unique_names'] == summary['comparison']['identifier']['full_unique']
        results.append({'repository':name, 'status':'passed', 'file_name_rows':sum(len(a['files']) for a in totals.values()), 'validated_names':len(totals), 'probe_candidate_and_identifier_totals_match':True})
        print(json.dumps(results[-1]), flush=True)
    dump(root/'validation.json', results)
    return results

if __name__ == '__main__':
    validate(Path(sys.argv[1]))
