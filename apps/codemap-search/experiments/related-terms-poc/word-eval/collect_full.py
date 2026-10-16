"""P3 stages 1–2 only: full candidate collection, sample comparison, no model judgement.
Run outputs are exclusive-create; COMPLETE is written only after independent validation.
"""
import collections as C
import hashlib
import importlib.metadata as metadata
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

import collect_repo as rules
import languages
import survey_tag_strings as tags
from word_eval_paths import REPO, SCRIPTS, work_dir
from tree_sitter_language_pack import get_parser

REPOS = ['bitwarden-server', 'express', 'flask', 'gin', 'gutenberg', 'keycloak', 'redis', 'sentry', 'terminal', 'tokio', 'vector', 'woocommerce']

def dump(path, value):
    with path.open('x') as f:
        json.dump(value, f, ensure_ascii=False, indent=1)
        f.write('\n')

def emit(f, value):
    f.write(json.dumps(value, ensure_ascii=False) + '\n')

def commit(path):
    return subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True).strip()

def walk_code(root):
    stack = [root]
    while stack:
        node = stack.pop()
        if 'comment' in node.type:
            continue
        yield node
        stack.extend(reversed(node.children))

def tag_values(node, language):
    if language == 'go' and node.type == 'field_declaration':
        tag = node.child_by_field_name('tag')
        if tag:
            for match in re.finditer(r'(\w+):"([^"]*)"', tag.text.decode(errors='replace')):
                yield match.group(2), match.group(1)
    elif language in tags.TAG_NODES and node.type in tags.STRING_NODES and node.is_named:
        if node.parent and node.parent.type in tags.STRING_NODES:
            return
        containers, items = tags.TAG_NODES[language]
        container = tags.enclosing(node, containers)
        if container is not None and not tags.is_inside_comment(node):
            value = tags.string_value(node)
            if value is not None:
                key = tags.key_of(node)
                yield value, tags.item_name(tags.enclosing(node, items) or container) + (f'({key})' if key else '')

def value_decision(value, is_test):
    """Reject whitespace on the raw extracted value, before any normalization."""
    if is_test:
        return 'test_file'
    if re.search(r'\s', value):
        # Keep the existing empty-value reason without letting strip admit a value.
        return 'empty' if not value.strip() else 'has_whitespace'
    return 'empty' if not value else None


def scan(source, language, is_test):
    """File-local exact counts plus first occurrence and all qualifier frequencies."""
    if language in ('c', 'cpp'):
        source = languages.blank_sal_annotations(source)
    parser = PARSERS.get(language)
    if parser is None:
        parser = PARSERS[language] = get_parser(language)
    tree = parser.parse(source)
    counts = {'identifier': C.Counter(), 'tag_value': C.Counter()}
    quals = {'identifier': C.defaultdict(C.Counter), 'tag_value': C.defaultdict(C.Counter)}
    locations = {'identifier': {}, 'tag_value': {}}
    skipped = C.Counter()
    for node in walk_code(tree.root_node):
        line = node.start_point[0] + 1
        if node.type in languages.NAME_LEAVES:
            name = languages.clean_name(node.text.decode(errors='replace'))
            if name and name != '_' and re.fullmatch(r'[\w$]+', name):
                counts['identifier'][name] += 1
                locations['identifier'].setdefault(name, line)
                parent = node.parent
                if parent is not None and parent.type in rules.QUALIFIED_PARENTS and parent.named_children and parent.named_children[-1] == node and parent.named_children[0] != node:
                    quals['identifier'][name][parent.named_children[0].text.decode(errors='replace')] += 1
        for value, qualifier in tag_values(node, language):
            reason = value_decision(value, is_test)
            if reason:
                skipped[reason] += 1
                continue
            for token in re.split(r'[,=| ]', value.strip()):
                if not token or token == '-':
                    continue
                counts['tag_value'][token] += 1
                quals['tag_value'][token][qualifier] += 1
                locations['tag_value'].setdefault(token, line)
    return counts, quals, locations, tree.root_node.has_error, skipped

PARSERS = {}

def collect_repo(name, out, old):
    started = time.time()
    repo = Path(old['repository'])
    out.mkdir()
    files, candidates, exclusions, unreadable, unsupported = [], [], [], [], C.defaultdict(list)
    for path in sorted(repo.rglob('*')):
        if not path.is_file():
            continue
        rel = str(path.relative_to(repo))
        dirs = sorted(rules.EXCLUDED_DIRECTORIES & set(Path(rel).parts[:-1]))
        if dirs:
            exclusions.append({'file': rel, 'reason': 'excluded_directory', 'directories': dirs})
            continue
        if path.suffix not in rules.EXTENSIONS:
            unsupported[path.suffix.lower() or '<no-extension>'].append(rel)
            continue
        try:
            source = path.read_bytes()
        except OSError as e:
            unreadable.append({'file': rel, 'phase': 'candidate', 'error': str(e)})
            continue
        lines = source.decode(errors='replace').splitlines()
        reason = 'generated' if rules.GENERATED_MARKER.search(source[:2048]) else 'long_lines' if any(len(x) > rules.MINIFIED_LINE_LENGTH for x in lines) else None
        if reason:
            exclusions.append({'file': rel, 'reason': reason, 'lines': len(lines)})
        else:
            candidates.append((rel, len(lines)))
    cpp = any(Path(f).suffix in rules.CPP_EXTENSIONS for f, _ in candidates)
    if name == 'gin':
        sample_files = sorted({json.loads(line)['file'] for line in (work_dir()/name/'definitions.jsonl').open()})
        sample_basis = 'distinct files in legacy definitions.jsonl (sample-files.json absent)'
        expected_sample_lines = None
    else:
        sample = json.loads((work_dir()/name/'sample-files.json').read_text())
        sample_files = [x['file'] for x in sample['files']]
        sample_basis = 'sample-files.json'
        expected_sample_lines = sample['sampled_lines']
    sample_set = set(sample_files)
    aggregate = {'identifier': {}, 'tag_value': {}}
    sample_counts = {'identifier': C.Counter(), 'tag_value': C.Counter()}
    errors, skipped_tags, langs = [], C.Counter(), C.Counter()
    with (out/'file-names.jsonl').open('x') as evidence:
        for rel, line_count in candidates:
            path = repo/rel
            language = rules.language_of(path, cpp)
            is_test = bool(rules.TEST_PATH.search(rel))
            try:
                raw = path.read_bytes()
                counts, quals, locations, has_error, skipped = scan(raw, language, is_test)
            except Exception as e:
                unreadable.append({'file': rel, 'phase': 'analysis', 'error': f'{type(e).__name__}: {e}'})
                continue
            if has_error:
                errors.append(rel)
            langs[language] += 1
            skipped_tags.update(skipped)
            files.append({'file': rel, 'lines': line_count, 'language': language, 'is_test': is_test, 'parse_error': has_error, 'sha256': hashlib.sha256(raw).hexdigest()})
            directory = str(Path(rel).parent)
            for channel in counts:
                if rel in sample_set:
                    sample_counts[channel].update(counts[channel])
                for token, count in counts[channel].items():
                    row = {'channel': channel, 'name': token, 'file': rel, 'directory': directory, 'count': count, 'is_test': is_test, 'line': locations[channel][token], 'qualifiers': dict(quals[channel][token])}
                    emit(evidence, row)
                    a = aggregate[channel].setdefault(token, {'count': 0, 'test_count': 0, 'files': set(), 'directories': set(), 'qualifiers': C.Counter(), 'examples': []})
                    a['count'] += count
                    a['test_count'] += count if is_test else 0
                    a['files'].add(rel)
                    a['directories'].add(directory)
                    a['qualifiers'].update(quals[channel][token])
                    # Prefer distinct directories; file-level evidence retains every location.
                    example = {'file': rel, 'directory': directory, 'line': locations[channel][token]}
                    if len(a['examples']) < 3:
                        a['examples'].append(example)
                    elif directory not in {e['directory'] for e in a['examples']}:
                        for i in range(2, -1, -1):
                            if sum(e['directory'] == a['examples'][i]['directory'] for e in a['examples']) > 1:
                                a['examples'][i] = example
                                break
    with (out/'names.jsonl').open('x') as f:
        for channel, names in aggregate.items():
            for token, a in sorted(names.items()):
                emit(f, {'channel': channel, 'name': token, 'count': a['count'], 'test_count': a['test_count'], 'file_count': len(a['files']), 'directory_count': len(a['directories']), 'is_test_only': a['count'] == a['test_count'], 'qualifiers': dict(a['qualifiers']), 'examples': a['examples']})
    legacy = json.loads((work_dir()/name/'unit-input.json').read_text())
    legacy_ids = {x['name'] for x in legacy if x['where'] != 'struct_tag'}
    legacy_tags = {x['name'] for x in legacy if x['where'] == 'struct_tag'}
    if (work_dir()/name/'tag-input.json').exists():
        legacy_tags |= {x['name'] for x in json.loads((work_dir()/name/'tag-input.json').read_text())}
    comparisons = {}
    for channel, legacy_names in [('identifier', legacy_ids), ('tag_value', legacy_tags)]:
        full = set(aggregate[channel])
        shared = full & legacy_names
        total = sum(a['count'] for a in aggregate[channel].values())
        comparisons[channel] = {'full_unique': len(full), 'legacy_sample_unique': len(legacy_names), 'legacy_sample_in_full': len(shared), 'sample_in_full_ratio': len(shared)/len(legacy_names) if legacy_names else None, 'full_in_sample_type_ratio': len(shared)/len(full) if full else None, 'full_occurrences': total, 'full_occurrences_of_sample_names': sum(aggregate[channel][n]['count'] for n in shared), 'full_in_sample_occurrence_ratio': sum(aggregate[channel][n]['count'] for n in shared)/total if total else None, 'new_names': len(full-legacy_names), 'test_only_names': sum(a['count'] == a['test_count'] for a in aggregate[channel].values()), 'sample_reparsed_unique': len(sample_counts[channel]), 'sample_reparsed_occurrences': sum(sample_counts[channel].values())}
        dump(out/f'{channel}-differences.json', {'missing_legacy_sample_names': sorted(legacy_names-full), 'new_names': sorted(full-legacy_names), 'legacy_not_in_reparsed_sample': sorted(legacy_names-set(sample_counts[channel])), 'reparsed_sample_not_in_legacy': sorted(set(sample_counts[channel])-legacy_names)})
    sample_audit = []
    for rel in sample_files:
        path = repo/rel
        raw = path.read_bytes()
        language = rules.language_of(path, cpp)
        counts, _, _, _, _ = scan(raw, language, bool(rules.TEST_PATH.search(rel)))
        sample_audit.append({'file': rel, 'lines': len(raw.decode(errors='replace').splitlines()), 'candidate': rel in dict(candidates), 'exclusion': next((e for e in exclusions if e['file'] == rel), None), 'identifier_names': sorted(counts['identifier']) if rel not in dict(candidates) else [], 'identifier_occurrences': sum(counts['identifier'].values())})
    dump(out/'sample-audit.json', {'basis': sample_basis, 'expected_lines': expected_sample_lines, 'files': sample_audit})
    dump(out/'manifest.json', {'candidates': [{'file': f, 'lines': n} for f,n in candidates], 'analyzed': files, 'exclusions': exclusions, 'unreadable': unreadable, 'unsupported': {ext: {'count':len(paths), 'files':paths} for ext,paths in sorted(unsupported.items())}, 'parse_errors': errors})
    result = {'repository': name, 'commit': commit(repo), 'candidate_files': len(candidates), 'candidate_lines': sum(n for _,n in candidates), 'analyzed_files':len(files), 'analyzed_lines':sum(f['lines'] for f in files), 'languages':dict(langs), 'parse_error_files':len(errors), 'unreadable_files':len(unreadable), 'excluded_by_reason':dict(C.Counter(e['reason'] for e in exclusions)), 'unsupported_files':sum(len(v) for v in unsupported.values()), 'skipped_tag_strings':dict(skipped_tags), 'sample_files':len(sample_files), 'sample_lines':sum(x['lines'] for x in sample_audit), 'comparison':comparisons, 'seconds':round(time.time()-started,2)}
    dump(out/'summary.json', result)
    return result

def main():
    out = Path(sys.argv[1]).resolve()
    if not out.is_relative_to(Path.cwd().resolve()):
        raise ValueError('Output outside assigned cwd')
    out.mkdir(parents=True, exist_ok=False)
    old = {n:json.loads((work_dir()/n/'summary.json').read_text()) for n in REPOS}
    preflight = [{'repository':n, 'expected':old[n]['commit'], 'actual':commit(Path(old[n]['repository']))} for n in REPOS]
    dump(out/'preflight.json', preflight)
    if any(x['actual'] != x['expected'] for x in preflight):
        raise RuntimeError('Commit mismatch: stopped before collection')
    dump(out/'provenance.json', {'project_head':commit(REPO), 'tree_sitter':metadata.version('tree-sitter'), 'tree_sitter_language_pack':metadata.version('tree-sitter-language-pack'), 'command':sys.argv, 'cwd':str(Path.cwd()), 'rules_sha256':{p:hashlib.sha256((SCRIPTS/p).read_bytes()).hexdigest() for p in ['collect_repo.py','collect_words.py','languages.py','collect_tag_values.py','probe_full_scale.py','survey_tag_strings.py']}, 'aggregation':'exact case-sensitive cleaned identifier spelling; identifier and tag_value channels never merged; counts sum syntactic occurrences across files; qualifier strings untruncated; no definitions/classification/model judgement', 'unsupported':'all non-supported extensions outside excluded directories, including non-code assets; observation only', 'probe':'full-scale-probe.json is premeasurement only, not an evaluation set'})
    results = []
    for name in REPOS:
        result = collect_repo(name, out/name, old[name])
        results.append(result)
        print(json.dumps(result, ensure_ascii=False), flush=True)
    dump(out/'summary.json', results)
    subprocess.run([sys.executable, str(SCRIPTS / 'validate_full.py'), str(out)], check=True)
    dump(out/'COMPLETE.json', {'repositories':REPOS, 'stages':[1,2], 'stage3_started':False})

if __name__ == '__main__':
    main()
