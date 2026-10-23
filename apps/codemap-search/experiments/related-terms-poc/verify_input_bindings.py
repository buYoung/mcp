"""Verify explicit input classification, final lookup, and immutable failure boundaries."""
import copy
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile

import correct_sense_dataset as corrections
import sense_lookup
import verify_sense_additions as fixtures


def make_fixture(root, rule):
    database = fixtures.fixture(root / 'base')
    with sqlite3.connect(database) as db:
        db.execute('DROP TABLE input_senses')
        db.executescript(fixtures.builder.INPUT_SENSES_SQL)
        if rule is None:
            db.execute("DELETE FROM sense_meta WHERE key='input_sense_rule'")
        db.execute("INSERT INTO fields VALUES ('other', 'Other', 'Synthetic')")
        for topic in ['x2', 'x3']:
            db.execute("INSERT INTO topics VALUES (?, ?, 'Synthetic', -1)", (topic, topic))
            db.execute("INSERT INTO topic_fields VALUES (?, 'other', 0)", (topic,))
        db.execute("INSERT INTO concepts VALUES (2, 'Second hop', 'x2')")
        db.executemany("INSERT INTO terms VALUES (?, ?, ?, '[]')", [(2,'target','target'),(3,'peer','peer'),(4,'beyond','beyond')])
        db.executemany("INSERT INTO senses VALUES (?, ?, ?)", [(2,2,'testing'),(3,3,'testing'),(4,3,'other'),(5,4,'other')])
        db.executemany("INSERT INTO personas VALUES (?, ?, ?, ?)", [(2,2,1,2),(3,3,1,3),(4,3,2,4),(5,4,2,5)])
    return database


def binding(word='target'):
    return {'word':word,'source_persona':1,'source_head':'anchor','concept':1}


def plan(dataset='fixture-base'):
    return {'schema_version':corrections.CORRECTION_SCHEMA,'base_dataset':dataset,'input_bindings':[binding()]}


def check_lookup(database, field='testing'):
    index = sense_lookup.SenseIndex.load(database, is_topic_split=True)
    for word in ['target', 'reviewed input']:
        key = sense_lookup.word_key(word)
        assert key not in index.senses
        assert all(not sense.is_own for sense in index.senses_of(key))
        result = sense_lookup.lookup(word, [], index, is_exact_first=True, is_inflection_evidence_used=False)
        words = {w for sense in result['senses'] for ring in ('direct','near','same_topic') for w in sense[ring]}
        assert words == {'anchor','peer'}, words
        assert {sense['field'] for sense in result['senses']} == {field}
        assert result['candidate_count'] == 2
        assert 'anchor' in sense_lookup.render_text([result])
    assert 'target' not in index.concept_words[1] and 'reviewedinput' not in index.concept_words[1]


def check_success(root, rule):
    root.mkdir()
    base = make_fixture(root, rule)
    digest = fixtures.digest(base)
    proposal = plan()
    proposal['remove_own'] = [{'word':'target','concept':1}]
    proposal['input_bindings'].append(binding('reviewed input'))
    source = root / 'plan.json'
    source.write_text(json.dumps(proposal))
    command = [sys.executable, str(Path(corrections.__file__)), '--base', str(base), '--corrections', str(source), '--output-root', str(root/'datasets')]
    report = json.loads(subprocess.run(command, check=True, capture_output=True, text=True).stdout)
    database = Path(report['path'])
    assert report['delta']['input_senses'] == 2 and report['delta']['personas'] == -1
    assert report['delta']['relations'] == 0 and report['delta']['terms'] == 1
    assert report['foreign_key_errors'] == []
    check_lookup(database)
    next_plan = plan(report['dataset_id'])
    next_plan['input_bindings'].append(binding('reviewed input'))
    repeated = corrections.apply_corrections(database, next_plan, root/'repeated')
    assert repeated['operations']['input_senses_already_present'] == 2
    assert all(delta == 0 for delta in repeated['delta'].values())
    check_lookup(repeated['path'])
    moved_plan = copy.deepcopy(next_plan)
    moved_plan['moves'] = [{'concept':1,'from_topic':'x1','to_topic':'x3'}]
    moved = corrections.apply_corrections(database, moved_plan, root/'moved')
    check_lookup(moved['path'], 'other')
    published_digest = fixtures.digest(database)
    try:
        corrections.apply_corrections(base, proposal, root/'datasets')
    except FileExistsError:
        pass
    else:
        raise AssertionError('existing output overwritten')
    assert fixtures.digest(database) == published_digest and fixtures.digest(base) == digest


def check_rejections(root):
    root.mkdir()
    base = make_fixture(root, 'head')
    digest = fixtures.digest(base)
    bad = []
    for key, value in [('source_persona',9999),('source_head','peer'),('concept',2),('source_persona',True),('word','anchor')]:
        proposal = plan(); proposal['input_bindings'][0][key] = value; bad.append(proposal)
    duplicate = plan(); duplicate['input_bindings'].append(binding('TARGET')); bad.append(duplicate)
    extra = plan(); extra['input_bindings'][0]['unexpected'] = 1; bad.append(extra)
    removed_source = plan(); removed_source['remove_own'] = [{'word':'anchor','concept':1}]; bad.append(removed_source)
    new_source = plan()
    new_source['personas'] = [{'word':'new-source','concept':1}]
    new_source['input_bindings'][0].update(source_persona=6, source_head='new-source')
    bad.append(new_source)
    reused_source = copy.deepcopy(new_source)
    reused_source['remove_own'] = [{'word':'beyond','concept':2}]
    reused_source['input_bindings'][0]['source_persona'] = 5
    bad.append(reused_source)
    inherited_source = copy.deepcopy(new_source)
    del inherited_source['personas']
    inherited_source['inherit'] = [{'word':'new-source','from':'anchor','concepts':[1]}]
    bad.append(inherited_source)
    for number, proposal in enumerate(bad):
        output = root / f'rejected-{number}'
        try:
            corrections.apply_corrections(base, proposal, output)
        except ValueError:
            pass
        else:
            raise AssertionError(f'invalid plan accepted: {proposal}')
        assert not list(output.glob('*/dataset.sqlite3'))
        for directory in output.glob('*'):
            assert (directory/'failed-build.json').is_file()
            try:
                sense_lookup.SenseIndex.load(directory/'dataset.sqlite3')
            except SystemExit:
                pass
            else:
                raise AssertionError('failed dataset consumable')
        assert fixtures.digest(base) == digest
    return len(bad)


def main():
    with tempfile.TemporaryDirectory(prefix='verify-input-bindings-') as temporary:
        root = Path(temporary)
        check_success(root/'head', 'head')
        check_success(root/'legacy', None)
        rejections = check_rejections(root/'failures')
    print(json.dumps({'pass':True,'direct_rules_verified':['head','legacy head_and_siblings'],
        'cli_to_lookup_verified':True,'input_not_own':True,'no_relations_synthesized':True,
        'one_hop_only':True,'new_word_and_own_conversion_verified':True,'existing_binding_idempotent':True,
        'source_topic_field_move_verified':True,'invalid_plans_rejected':rejections,
        'failed_output_unpublished':True,'source_and_existing_output_preserved':True}, indent=2))

if __name__ == '__main__': main()
