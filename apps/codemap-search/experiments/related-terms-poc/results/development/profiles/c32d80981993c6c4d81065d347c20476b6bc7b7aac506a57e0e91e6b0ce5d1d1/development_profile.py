"""Immutable development profile identities; all validation is read-only, including failure."""
from __future__ import annotations

import collections
import hashlib
import json
import re
import sqlite3
import subprocess
from pathlib import Path

# Verify approved helper bytes using stdlib BEFORE importing any reused helper.
E = Path(__file__).resolve().parent
ROOT = E.parents[3]
APPROVED_INVENTORY = ROOT / '.codemap/poc/related-terms/development-curation/development-dataset-candidate-v1/preservation-before.json'
CASES_SHA = '1a870e296b7203ab4cffd29e2154e6db0bd04553904062f7fc4e3686e8fc7b81'


def check_approved_helpers() -> dict:
    raw = APPROVED_INVENTORY.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != '9942cc997def9176605df6d115c2d476e443ffd6e6d27001a281f8f660593f51':
        raise ValueError('approved helper preservation inventory changed')
    approved = {r['path']: r for r in json.loads(raw)['files']}
    fixed = {'evaluate.py': 'ac6aac915face8328468b7f15e52b55775c2818def5a263afa827b77ebdab32b',
             'rtpoc_common.py': 'af51ee25d2fdde9c76e3e2cdbd2f5c7a8715b4880ef52b701dd1dbfefe66927f', 'cases.json': CASES_SHA}
    bindings = []
    for name in ('evaluate.py', 'rtpoc_common.py', 'workspaces.py', 'lifecycle.py', 'lookup_handoff.py', 'cases.json'):
        p = E / name
        rel = p.relative_to(ROOT).as_posix()
        expected = {k: approved[rel][k] for k in ('path', 'sha256', 'bytes')}
        if name in fixed and expected['sha256'] != fixed[name]:
            raise ValueError(f'unapproved historical input identity: {name}')
        h = hashlib.sha256()
        with p.open('rb') as f:
            while block := f.read(1024 * 1024):
                h.update(block)
        if h.hexdigest() != expected['sha256'] or p.stat().st_size != expected['bytes']:
            raise ValueError(f'approved reused input missing/changed: {name}')
        bindings.append(expected)
    return dict(inventory_identity=dict(path=APPROVED_INVENTORY.relative_to(ROOT).as_posix(), sha256=digest, bytes=len(raw)), approved_inputs=bindings)


APPROVED_INPUTS = check_approved_helpers()
from rtpoc_common import canonical_json, read_json, repo_relative, sha256_file

D = E / 'results/development'
AREA = ROOT / '.codemap/poc/related-terms/development-integration-v1'
RAW = ROOT / '.codemap/poc/related-terms/evaluation/development'
HEAD = '46995808c69cc452bd2fcdd9fd146244d0a4c734'
CANDIDATE = ROOT / '.codemap/poc/related-terms/development-curation/development-dataset-candidate-v1/candidate-handoff.json'
CANDIDATE_SHA = '92b14352e0c6126774bbf072dfaa500fde635226d7f69edc8cb6e49b11165b4e'
ID = 'rtd1-47baa61927c4b090'
COUNTS = dict(concepts=3810, expressions=5965, lexicalizations=8445, relations=15578, sources=5)
REPRODUCTION = {
    'full_builder_relocation': 'not_supported',
    'requirements': 'Isolated reproduction environment using the original absolute repository root recorded in frozen provenance, all frozen prerequisites, and absent owned candidate/output paths. Portable serving SQLite/JSONL is not cross-root builder support.',
    'original_root_locator': {'path': '.codemap/poc/related-terms/development-curation/semantic-adjudication-v2/final-decisions-handoff.json', 'fields': ['cwd', 'material[].path']},
    'original_inventory_locator': {'path': '.codemap/poc/related-terms/development-curation/semantic-adjudication-v2/preservation-before.json', 'field': '[].path'},
}


def require(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def local(path: str | Path) -> Path:
    p = Path(path)
    resolved = (ROOT / p).resolve() if not p.is_absolute() else p.resolve()
    require(resolved.is_relative_to(ROOT), f'outside repository: {path}')
    return resolved


def identity(path: str | Path) -> dict:
    p = local(path)
    require(p.is_file() and not p.is_symlink(), f'not a regular artifact: {path}')
    return dict(path=repo_relative(p), sha256=sha256_file(p), bytes=p.stat().st_size)


def checked(binding: dict) -> Path:
    p = local(binding['path'])
    require(identity(p) == {k: binding[k] for k in ('path', 'sha256', 'bytes')}, f'artifact missing/changed: {binding["path"]}')
    return p


def new_json(path: Path, value: object, canonical: bool = False) -> None:
    with path.open('x', encoding='utf-8') as f:
        f.write((canonical_json(value) if canonical else json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2)) + '\n')


def all_identities(value: object) -> list[dict]:
    found = {}
    def visit(node):
        if isinstance(node, dict):
            if {'path', 'sha256', 'bytes'} <= node.keys():
                binding = {k: node[k] for k in ('path', 'sha256', 'bytes')}
                previous = found.setdefault(binding['path'], binding)
                require(previous == binding, f'conflicting identities: {binding["path"]}')
            for child in node.values():
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)
    visit(value)
    return [found[k] for k in sorted(found)]


def snapshot(profile_path: Path, profile: dict) -> list[dict]:
    result = []
    companions = [profile_path.parent / name for name in ('evaluation-protocol.json', 'dataset-handoff.json', 'lookup-handoff.json', 'evaluate_development.py', 'development_profile.py')]
    for binding in all_identities(profile) + [identity(profile_path), *[identity(p) for p in companions]]:
        p = checked(binding)
        s = p.stat()
        result.append(dict(identity=binding, stat=dict(bytes=s.st_size, mode=s.st_mode, mtime_ns=s.st_mtime_ns, ctime_ns=s.st_ctime_ns)))
    return result


def dataset_check(dataset: dict) -> dict:
    p = checked(dataset['sqlite'])
    # immutable + mode=ro forbids sidecar creation and any repair.
    with sqlite3.connect(p.as_uri() + '?mode=ro&immutable=1', uri=True) as db:
        meta = dict(db.execute('SELECT key,value FROM dataset_meta'))
        for key in ('dataset_id', 'schema_version', 'normalization_version'):
            require(meta[key] == dataset[key], f'dataset metadata mismatch: {key}')
        counts = {table: db.execute(f'SELECT COUNT(*) FROM {table}').fetchone()[0] for table in COUNTS}
        require(counts == dataset['counts'] == COUNTS, 'dataset counts mismatch')
        objects = set(db.execute('SELECT type,name FROM sqlite_master'))
        require(all(('table', table) in objects for table in ['dataset_meta', *COUNTS]), 'dataset tables missing')
        require(all(('index', name) in objects for name in ('expressions_by_token_key', 'relations_by_from_expression', 'lexicalizations_by_expression', 'lexicalizations_by_concept')), 'dataset indexes missing')
        require(db.execute('PRAGMA integrity_check').fetchall() == [('ok',)], 'dataset integrity failure')
        require(not db.execute('PRAGMA foreign_key_check').fetchall(), 'dataset FK failure')
    return dict(status='pass', actual_counts=counts, meta_identity_checked=True, sqlite_open='mode=ro&immutable=1')


def validate(profile_path: str | Path) -> tuple[Path, dict]:
    p = local(profile_path)
    require(p.parent.parent == D / 'profiles' and p.name == 'profile.json', 'not an immutable development profile locator')
    require(re.fullmatch(r'[a-f0-9]{64}', p.parent.name) is not None, 'invalid profile key')
    require(sha256_file(p) == p.parent.name, 'profile file SHA differs from directory identity')
    profile = read_json(p)
    require(p.read_bytes() == (canonical_json(profile) + '\n').encode(), 'profile canonical bytes mismatch')
    validate_inputs(profile)
    return p, profile


def validate_inputs(profile: dict) -> None:
    approved = check_approved_helpers()
    require(profile['approved_input_gate'] == approved, 'profile approved-input provenance differs')
    require(profile['schema'] == 'rt-development-profile/1', 'unsupported profile')
    require(profile['dataset']['dataset_id'] == ID, 'wrong development dataset')
    require(profile['method']['measurement_conditions']['repeat'] == 5, 'repeat policy changed')
    for binding in all_identities(profile):
        checked(binding)
    head = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    require(head == profile['runtime']['product_head'] == HEAD, 'product HEAD mismatch')
    require(profile['candidate_handoff_identity'] == identity(CANDIDATE) and profile['candidate_handoff_identity']['sha256'] == CANDIDATE_SHA, 'unapproved candidate handoff')
    require(profile['dataset']['schema_version'] == 'rt-dataset/1' and profile['dataset']['normalization_version'] == 'rtn-2', 'dataset schema/normalization differs')
    dataset_check(profile['dataset'])
    require(profile['cases']['identity']['sha256'] == CASES_SHA, 'cases differ from approved fixed92 identity')
    original_method = read_json(checked(profile['method']['original_cohort_selection_identity']))
    require(original_method['inputs']['cases_sha256'] == profile['cases']['identity']['sha256'], 'cases differ from original protocol identity')
    cases = read_json(checked(profile['cases']['identity']))['cases']
    require(len(cases) == 92 and [c['case_id'] for c in cases] == profile['cases']['ordered_case_ids'], 'case order/count mismatch')
    require(len({c['input'] for c in cases}) == len({c['case_id'] for c in cases}) == 92, 'duplicate cases')
    require(dict(collections.Counter(c['group'] for c in cases)) == dict(control=16, category_probe=46, random_probe=30), 'cohort changed')
    policy = read_json(checked(profile['policy']))
    require(policy['policy_id'] == profile['policy']['policy_id'] == 'rtl-1', 'policy mismatch')
    require(profile['policy']['sha256'] == '1defb4278d812ce843a619ec48554e996f620ac46d7639077e33bb80847a51d7', 'policy differs from approved rtl-1 identity')
    require(policy['commonness_max_file_fraction'] == 0.25 and policy['final_limit'] == 10, 'policy limits changed')
    require(set(profile['targets']) == {'express', 'flask', 'gin', 'tokio'}, 'target population changed')
    for name, target in profile['targets'].items():
        published = read_json(checked(target['published_identity']))
        meta = read_json(checked(target['index_meta_identity']))
        require(target['index_generation'] == target['index_meta_identity']['sha256'] == published['index_generation'], f'{name}: generation mismatch')
        require(meta['opstamp'] == published['index_opstamp'] == target['index_opstamp'], f'{name}: opstamp mismatch')
        require(published['workspace']['git_head'] == target['commit'], f'{name}: commit mismatch')
        require(published['stats_sha256'] == target['stats_identity']['sha256'], f'{name}: stats SHA mismatch')
        for key in ('live_files', 'vocabulary_size', 'field_policy', 'analyzer_policy'):
            require(published[key] == target[key], f'{name}: {key} mismatch')
        with sqlite3.connect(checked(target['stats_identity']).as_uri() + '?mode=ro&immutable=1', uri=True) as db:
            generation = db.execute("SELECT value FROM stats_meta WHERE key='index_generation'").fetchone()[0]
            require(generation == target['index_generation'], f'{name}: stats generation mismatch')


def freeze() -> tuple[Path, dict]:
    import evaluate as original
    require(sha256_file(CANDIDATE) == CANDIDATE_SHA, 'candidate handoff mismatch')
    h = read_json(CANDIDATE)
    require(h['dataset_id'] == ID and h['counts'] == COUNTS and not h['activated'], 'candidate mismatch')
    original_protocol_path = E / 'results/evaluation-protocol.json'
    require(sha256_file(original_protocol_path) == '8518dd56802ee7e0fe0e1cf6d3ca1d618ee3b720ba076cc1fa61b3f249b0bf00', 'original method file changed')
    old = read_json(original_protocol_path)
    require(original.canonical_hash({k: v for k, v in old.items() if k != 'protocol_sha256'}) == old['protocol_sha256'], 'original method identity invalid')
    require(sha256_file(E / 'cases.json') == old['inputs']['cases_sha256'] == CASES_SHA, 'cases differ from original approved protocol')
    cases = original.load_cases()
    require([c['input'] for c in cases if c['group'] == 'random_probe'] == old['random_probe_selection']['selected'], 'original cohort order differs')
    source_checks = read_json(AREA / 'confirmed-identities.json')['checks']
    for binding in source_checks:
        checked(binding)
    manifest_binding = identity(E / 'workspaces.json')
    workspace_manifest = read_json(checked(manifest_binding))
    lookup = read_json(E / 'results/lookup-handoff.json')
    preserved_paths = read_json(AREA / 'protected-inventory-before.json')['files']
    targets = {}
    for target in workspace_manifest['targets']:
        name = target['name']
        baseline = lookup['targets'][name]
        state = local(baseline['state_dir'])
        published = read_json(state / 'published.json')
        prefix = repo_relative(state / 'index') + '/'
        index_artifacts = [identity(r['path']) for r in preserved_paths if r['path'].startswith(prefix)]
        targets[name] = dict(
            commit=target['commit'], state_dir=repo_relative(state),
            published_identity=identity(state / 'published.json'), index_meta_identity=identity(state / 'index/meta.json'),
            index_artifact_identities=index_artifacts, stats_identity=identity(local(baseline['stats_path'])),
            **{k: published[k] for k in ('index_generation', 'index_opstamp', 'live_files', 'vocabulary_size', 'field_policy', 'analyzer_policy')})
    code_names = ['evaluate_development.py', 'development_profile.py', 'evaluate.py', 'rtpoc_common.py', 'workspaces.py', 'lifecycle.py', 'lookup_handoff.py']
    profile = dict(
        schema='rt-development-profile/1', approved_input_gate=check_approved_helpers(), candidate_handoff_identity=identity(CANDIDATE),
        dataset=dict(dataset_id=ID, schema_version=h['schema_version'], normalization_version=h['normalization_version'], counts=h['counts'],
                     sqlite=identity(local(h['dataset_dir']) / 'dataset.sqlite3'), manifest_identity=identity(local(h['dataset_dir']) / 'manifest.json'),
                     curation_binding_identity=h['curation_binding'], derivation_sha256=h['derivation_sha256'], artifact_identities=h['artifacts']),
        policy=identity(E / 'contracts/lookup-policy.json') | {'policy_id': 'rtl-1'},
        runtime=dict(binary_identity=lookup['binary'] | {'bytes': local(lookup['binary']['path']).stat().st_size}, product_head=HEAD,
                     crate_identities=[identity(E / 'Cargo.toml'), identity(E / 'Cargo.lock')],
                     source_identities=[identity(E / name) for name in lookup['crate']['source_sha256']],
                     output_schema_identity=identity(E / 'contracts/lookup.schema.json')),
        code_identities=[identity(E / name) for name in code_names],
        cases=dict(identity=identity(E / 'cases.json'), count=len(cases), ordered_case_ids=[c['case_id'] for c in cases],
                   by_group=dict(collections.Counter(c['group'] for c in cases)), by_category=dict(collections.Counter(c['category'] for c in cases))),
        workspaces_manifest_identity=manifest_binding, targets=targets,
        method=dict(rubric={k: old['review'][k] for k in ('method', 'labels', 'populations', 'uncertainty')}, metrics=old['metrics'],
                    measurement_conditions=old['measurement_conditions'] | {'dataset_build': 'not_run; verified existing development SQLite reused'},
                    review_sampling=dict(seed=original.REVIEW_SAMPLE_SEED, maximum=original.FILTER_LOSS_SAMPLE_MAX,
                                         interval_rule='ci95=null for a full loss-population census (sample==population); Wilson95 only for an actual uniform partial sample, without FPC. Final census, controls/categories and repeated observations always descriptive.'),
                    original_cohort_selection=old['random_probe_selection'], original_cohort_selection_identity=identity(original_protocol_path)),
        evidence_refs=dict(independent_reviews=[identity(AREA / 'candidate-review/data.json'), identity(AREA / 'candidate-review/pipeline.json')],
                           contract_map=identity(AREA / 'contract-map-retained.json'),
                           reused_baseline=[identity(E / 'results' / name) for name in ('lookup-handoff.json', 'dataset-handoff.json', 'index-stats.json', 'lifecycle-evidence.json', 'offline-evidence.json')],
                           classification='Candidate reviews and prior Cargo/lifecycle/offline evidence reused, not fresh native network denial. New native checks and measurements are separate run evidence.'),
        source_provenance_identities=[identity(E / 'sources.lock.json'), identity(E / 'ATTRIBUTION.md'), *h['implementation_files']],
        source_archive_identities=[identity(b['path']) for b in read_json(AREA / 'confirmed-identities.json')['source_archive_checks']],
        frozen_curation_identities=[h[k] for k in ('final_handoff_identity', 'final_decision_identity', 'curation_policy_identity', 'evidence_identity')],
        reproduction_note=REPRODUCTION,
    )
    # Reject every input mismatch BEFORE any success profile/handoff/selector is written.
    validate_inputs(profile)
    # Profile identity is the actual canonical UTF-8 file hash, not a self-referential field.
    raw = (canonical_json(profile) + '\n').encode()
    key = hashlib.sha256(raw).hexdigest()
    directory = D / 'profiles' / key
    directory.mkdir(parents=True, exist_ok=False)
    p = directory / 'profile.json'
    with p.open('xb') as f:
        f.write(raw)
    validate(p)
    for name in ('evaluate_development.py', 'development_profile.py'):
        (directory / name).write_bytes((E / name).read_bytes())
    protocol = dict(schema='rt-development-evaluation-protocol/1', profile_identity=identity(p),
                    inputs=profile['cases'], method=profile['method'],
                    frozen_identities=dict(dataset_id=ID, dataset_sqlite_sha256=profile['dataset']['sqlite']['sha256'],
                                           normalization_version='rtn-2', policy_id='rtl-1', policy_sha256=profile['policy']['sha256'],
                                           binary_sha256=profile['runtime']['binary_identity']['sha256'],
                                           index_generation={name: t['index_generation'] for name, t in targets.items()}),
                    code_identities=profile['code_identities'], semantic_status='pending_fresh_labels', label_reuse=False)
    new_json(directory / 'evaluation-protocol.json', protocol)
    new_json(directory / 'dataset-handoff.json', dict(schema='rt-development-dataset-handoff/1', profile_identity=identity(p),
             candidate_handoff_identity=profile['candidate_handoff_identity'], dataset_identity=profile['dataset'],
             read_only_database_validation=dataset_check(profile['dataset']), evidence_refs=profile['evidence_refs'], reproduction_note=REPRODUCTION))
    new_json(directory / 'lookup-handoff.json', dict(schema='rt-development-lookup-handoff/1', profile_identity=identity(p),
             dataset_handoff_identity=identity(directory / 'dataset-handoff.json'), policy=profile['policy'], runtime=profile['runtime'],
             targets=targets, native_ready_checks='pending; see run evidence', build='not_run', automatic_rebuild=False))
    return p, profile


def resolve_selector() -> tuple[Path, dict]:
    selector = read_json(D / 'active.json')
    require(selector['schema'] == 'rt-development-selection/1', 'unsupported selector')
    return validate(checked(selector['profile']))
