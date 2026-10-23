"""Synthetic finite-edge verification: provenance, origins, options, compatibility, publication."""
import copy
import json
from pathlib import Path
import sqlite3
import tempfile
from unittest.mock import patch

import correct_sense_dataset as corrections
import reviewed_relations as edges
import sense_lookup as lookup
import verify_input_bindings as fixtures


def fixture(root, rule):
    root.mkdir()
    db = fixtures.make_fixture(root, rule)
    with sqlite3.connect(db) as c:
        c.executemany("INSERT INTO terms VALUES (?,?,?,'[]')", [(5,'committed','committed'),(6,'finite input','finiteinput')])
        c.executemany("INSERT INTO senses VALUES (?,?,?)", [(6,5,'testing'),(7,6,'other')])
        c.executemany("INSERT INTO personas VALUES (?,?,?,?)", [(6,5,1,6),(7,6,2,7)])
        c.executemany("INSERT INTO input_senses VALUES (?,?)", [('committed',3),('target',3)])
        c.executemany("INSERT INTO relations (term_id,rank,word_key,surface,kind,persona) VALUES (?,?,?,?,?,?)",
                      [(1,10,'storedliteral','stored literal','synonym',1),(1,20,'finiteinput','finite input','near',1),
                       (3,7,'peeronly','peer-only','near',3)])
    return db


def operation(origin='own'):
    return {'source':{'term_id':1,'head':'anchor','persona':1,'concept':1,'topic':'x1',
                      'rank':10 if origin=='own' else 20,'word_key':'storedliteral' if origin=='own' else 'finiteinput',
                      'surface':'stored literal' if origin=='own' else 'finite input','kind':'synonym' if origin=='own' else 'near'},
            'target':{'word':'committed','head':'committed' if origin=='own' else 'peer',
                      'persona':6 if origin=='own' else 3,'concept':1,'topic':'x1','origin':origin},
            'kind':'near','rank':10 if origin=='own' else 20}


def plan(dataset='fixture-base'):
    return {'schema_version':corrections.CORRECTION_SCHEMA,'base_dataset':dataset,
            'relation_additions':[operation(),operation('input')]}


def result(db, word, split=True, **options):
    index=lookup.SenseIndex.load(db,is_topic_split=split)
    defaults={'is_exact_first':True,'is_inflection_evidence_used':False}
    return lookup.lookup(word,[],index,**(defaults|options))


def candidates(r):
    return {w for s in r['senses'] for k in ['direct','near','same_topic'] for w in s[k]}


def success(root, rule):
    base=fixture(root,rule);before=corrections.file_digest(base)
    built=corrections.apply_corrections(base,plan(),root/'outputs');db=Path(built['path'])
    assert built['schema_version']==edges.INPUT_RELATIONS_SCHEMA and built['dataset_id'].startswith('rts3-')
    assert {k:v for k,v in built['delta'].items() if v}=={'relations':1,'input_relations':1}
    with sqlite3.connect(db) as c:
        assert c.execute('PRAGMA foreign_key_check').fetchall()==[]
        assert c.execute("SELECT surface,kind,rank,persona FROM relations WHERE term_id=5").fetchone()==('stored literal','near',10,6)
        assert c.execute('SELECT * FROM input_relations').fetchall()==[('committed',3,'finiteinput','finite input','near',20)]
        assert c.execute('SELECT kind FROM relations WHERE term_id=1 AND rank=10').fetchone()==('synonym',)
    for split in [False,True]:
        index=lookup.SenseIndex.load(db,is_topic_split=split)
        assert any(not s.is_own and 'finiteinput' in s.direct for s in index.senses_of('committed'))
        assert all('finiteinput' not in s.direct for s in index.senses_of('peer'))
        for word in ['peer','target','anchor']:
            assert result(base,word,split)==result(db,word,split),word
        for exact in [False,True]:
            actual=result(db,'committed',split,is_exact_first=exact)
            assert {'stored literal','finite input'}<=candidates(actual)
            assert 'beyond' not in candidates(actual)
            assert 'finite input' in lookup.render_text([actual])
            assert result(base,'committed',split,is_direct_included=False,is_exact_first=exact)==result(db,'committed',split,is_direct_included=False,is_exact_first=exact)
        alias=result(db,'comitted',split,spelling_matches={})
        assert {'stored literal','finite input'}<=candidates(alias)
        assert not result(db,'comitted',split)['resolution']['seeds']
        for word in ['stored literal','finite input']:
            reverse=result(db,word,split,is_reverse_included=True)
            assert 'committed' in {w for r in reverse['reverse'] for w in r['words']}
            assert result(db,word,split)['reverse']==[]
    repeated=corrections.apply_corrections(db,plan(built['dataset_id']),root/'repeat')
    assert all(v==0 for v in repeated['delta'].values())
    assert result(db,'committed')==result(repeated['path'],'committed')
    moved={'schema_version':corrections.CORRECTION_SCHEMA,'base_dataset':built['dataset_id'],
           'moves':[{'concept':1,'from_topic':'x1','to_topic':'x3'}]}
    moved_report=corrections.apply_corrections(db,moved,root/'moved')
    assert 'finite input' in candidates(result(moved_report['path'],'committed'))
    assert all(s['field']=='other' for s in result(moved_report['path'],'committed')['senses'])
    blocked={'schema_version':corrections.CORRECTION_SCHEMA,'base_dataset':built['dataset_id'],'remove_input':[{'word':'committed','concept':1}]}
    try:corrections.apply_corrections(db,blocked,root/'dependent')
    except ValueError:pass
    else:raise AssertionError('dependent input removed')
    assert not list((root/'dependent').glob('*/dataset.sqlite3'))
    native=plan();native['relation_additions']=native['relation_additions'][:1]
    native_report=corrections.apply_corrections(base,native,root/'native')
    assert native_report['schema_version']==edges.LEGACY_SCHEMA and 'input_relations' not in native_report['after']
    bad=plan(built['dataset_id']);bad['relation_additions'][0]['kind']='co_use'
    try:corrections.apply_corrections(db,bad,root/'collision')
    except ValueError:pass
    else:raise AssertionError('existing tuple overwritten')
    assert corrections.file_digest(base)==before
    return base,db


def invalid_plans(root,base):
    bad=[]
    for section,key,value in [('source','rank',99),('source','kind','identifier'),('source','surface','Stored Literal'),
                              ('source','term_id',999),('source','persona',True),('target','topic','x3')]:
        p=plan();p['relation_additions'][0][section][key]=value;bad.append(p)
    p=plan();p['relation_additions'][0]['target'].update(head='peer',persona=3);bad.append(p)
    p=plan();p['relation_additions'][1]['target'].update(word='anchor');bad.append(p)
    p=plan();p['relation_additions'].append(copy.deepcopy(p['relation_additions'][0]));bad.append(p)
    for operation_name,value in [('remove_own',{'word':'anchor','concept':1}),('remove_own',{'word':'committed','concept':1}),
                                 ('remove_own',{'word':'peer','concept':1}),('remove_input',{'word':'committed','concept':1}),
                                 ('moves',{'concept':1,'from_topic':'x1','to_topic':'x3'})]:
        p=plan();p[operation_name]=[value];bad.append(p)
    for persona in [7,8]:
        p=plan();p['personas']=[{'word':'new-target','concept':1}]
        p['relation_additions'][0]['target'].update(word='new-target',head='new-target',persona=persona)
        if persona==7:p['remove_own']=[{'word':'finite input','concept':2}]
        bad.append(p)
    for n,p in enumerate(bad):
        destination=root/f'bad-{n}'
        try:corrections.apply_corrections(base,p,destination)
        except ValueError:pass
        else:raise AssertionError(f'invalid plan accepted: {n}')
        assert not destination.exists(),n
    return len(bad)


def failures_and_layout(root,base,db):
    original=corrections.CorrectionEditor.add_relation
    def fail(editor,entry):
        original(editor,entry)
        raise ValueError('injected failure after DDL/insert')
    with patch.object(corrections.CorrectionEditor,'add_relation',fail):
        try:corrections.apply_corrections(base,plan(),root/'rollback')
        except ValueError:pass
        else:raise AssertionError('injection ignored')
    pending=next((root/'rollback').glob('*/.dataset.sqlite3.pending'))
    assert not (pending.parent/'dataset.sqlite3').exists() and (pending.parent/'failed-build.json').exists()
    with sqlite3.connect(pending) as c:
        assert not edges.has_input_relations(c)
        assert c.execute('SELECT count(*) FROM relations WHERE term_id=5').fetchone()==(0,)
    collision=plan()
    collision['relation_additions'][1]['target']=copy.deepcopy(collision['relation_additions'][0]['target'])
    collision['relation_additions'][1]['rank']=10
    try:corrections.apply_corrections(base,collision,root/'rank-collision')
    except ValueError:pass
    else:raise AssertionError('duplicate native target rank accepted')
    assert not list((root/'rank-collision').glob('*/dataset.sqlite3'))
    import shutil
    broken = [["DROP TABLE input_relations"], ["UPDATE sense_meta SET value='rt-sense/2' WHERE key='schema_version'"],
              ["UPDATE input_relations SET source_persona=999"], ["UPDATE input_relations SET kind='unknown'"],
              ["UPDATE personas SET sense=4 WHERE persona=3"],
              ["DROP TABLE input_relations", "CREATE TABLE input_relations(input_word_key TEXT NOT NULL,source_persona INTEGER NOT NULL,related_word_key TEXT NOT NULL,surface TEXT NOT NULL,kind TEXT NOT NULL,rank INTEGER NOT NULL,PRIMARY KEY(input_word_key,source_persona,related_word_key))"]]
    for n,statements in enumerate(broken):
        path=root/f'corrupt-{n}.sqlite3';shutil.copyfile(db,path)
        with sqlite3.connect(path) as c:
            for sql in statements:c.execute(sql)
        try:lookup.SenseIndex.load(path)
        except SystemExit:pass
        else:raise AssertionError(f'corrupt extension accepted: {n}')


def exact_identity(connection, persona, rank=None):
    columns=('term_id','head','persona','concept','topic','sense','field')
    row=connection.execute('SELECT p.term_id,t.surface,p.persona,p.concept,c.topic,p.sense,s.field FROM personas p JOIN terms t USING(term_id) JOIN concepts c USING(concept) JOIN senses s USING(sense) WHERE p.persona=?',(persona,)).fetchone()
    value=dict(zip(columns,row))
    if rank is not None:
        value.update(zip(('rank','word_key','surface','kind'),connection.execute('SELECT rank,word_key,surface,kind FROM relations WHERE persona=? AND rank=?',(persona,rank)).fetchone()))
    return value


def exact_fixture(root, rule, extended):
    base=fixture(root,rule)
    with sqlite3.connect(base) as db:
        db.executemany('INSERT INTO relations VALUES (?,?,?,?,?,?)',[(1,30,'wrongfamily','wrong family','family',1),(5,8,'finiteinput','finite input','near',6)])
        db.executemany('INSERT INTO input_senses VALUES (?,?)',[('target',1),('finiteinput',1)])
        if extended:
            edges.create_input_relations(db)
            db.execute("UPDATE sense_meta SET value='rt-sense/3' WHERE key='schema_version'")
            db.execute("INSERT INTO input_relations VALUES ('target',3,'targetedge','target edge','near',9)")
        source=exact_identity(db,1)
        proposal={'schema_version':corrections.CORRECTION_SCHEMA,'base_dataset':'fixture-base',
            'relation_edits':[
                {'mode':'transfer_new_head','source':exact_identity(db,1,10),'target':exact_identity(db,6),'rank':2},
                {'mode':'consolidate_existing','source':exact_identity(db,1,20),'target':exact_identity(db,6),'rank':8},
                {'mode':'remove','source':exact_identity(db,1,30)},
                {'mode':'reattribute_same_head','source':exact_identity(db,3,7),'target':exact_identity(db,4)}],
            'input_detachments':[{'word_key':w,'source':source} for w in ['target','finiteinput']],
            'input_bindings':[{'word':w,'source_persona':3,'source_head':'peer','concept':1} for w in ['target','finite input']],
            'remove_own':[{'word':'anchor','concept':1}]}
    return base,proposal


def exact_success(root,rule,extended):
    base,proposal=exact_fixture(root,rule,extended);digest=corrections.file_digest(base)
    built=corrections.apply_corrections(base,proposal,root/'out');after=Path(built['path'])
    assert built['schema_version']==('rt-sense/3' if extended else 'rt-sense/2')
    with sqlite3.connect(after) as db:
        assert db.execute('SELECT * FROM relations WHERE term_id=1').fetchall()==[]
        assert db.execute('SELECT * FROM relations WHERE term_id=5 ORDER BY rank').fetchall()==[(5,2,'storedliteral','stored literal','synonym',6),(5,8,'finiteinput','finite input','near',6)]
        assert db.execute('SELECT * FROM relations WHERE term_id=3').fetchall()==[(3,7,'peeronly','peer-only','near',4)]
        assert db.execute('SELECT persona FROM input_senses WHERE word_key=? ORDER BY persona',('target',)).fetchall()==[(3,)]
        assert db.execute('SELECT persona FROM input_senses WHERE word_key=?',('finiteinput',)).fetchall()==[(3,)]
        assert db.execute('SELECT persona FROM personas WHERE persona=1').fetchall()==[]
        assert db.execute('SELECT sense FROM senses WHERE sense=1').fetchall()==[]
        assert db.execute('PRAGMA foreign_key_check').fetchall()==[]
        if extended:assert db.execute('SELECT * FROM input_relations').fetchall()==[('target',3,'targetedge','target edge','near',9)]
    for split in [False,True]:
        for exact in [False,True]:
            actual=result(after,'committed',split,is_exact_first=exact)
            assert {'stored literal','finite input'}<=candidates(actual) and 'beyond' not in candidates(actual)
            without=result(after,'committed',split,is_direct_included=False,is_exact_first=exact)
            assert 'stored literal' not in candidates(without)
            reverse=result(after,'stored literal',split,is_reverse_included=True)
            assert {w for r in reverse['reverse'] for w in r['words']}=={'committed'}
            assert result(after,'stored literal',split)['reverse']==[]
        index=lookup.SenseIndex.load(after,is_topic_split=split)
        assert any(not s.is_own and 1 in s.concepts and 'peer' in s.direct for s in index.senses_of('finiteinput'))
        assert 'stored literal' in lookup.render_text([result(after,'committed',split)])
    assert corrections.file_digest(base)==digest
    return base,proposal


def exact_rejections_and_rollback(root,base,proposal):
    bad=[]
    for section,key,value in [('source','head','ANCHOR'),('source','sense',2),('source','field','other'),('source','rank',11),('source','surface','Stored Literal'),('source','kind','near'),('source','persona',True),('target','persona',999),('target','concept',2),('target','topic','x3')]:
        p=copy.deepcopy(proposal);p['relation_edits'][0][section][key]=value;bad.append(p)
    for key,value in [('mode','guess'),('rank',True),('rank',8),('extra',1)]:
        p=copy.deepcopy(proposal);p['relation_edits'][0][key]=value;bad.append(p)
    p=copy.deepcopy(proposal);p['relation_edits'].append(p['relation_edits'][0]);bad.append(p)
    p=copy.deepcopy(proposal);p['input_detachments'].append(p['input_detachments'][0]);bad.append(p)
    p=copy.deepcopy(proposal);p['input_detachments'][0]['word_key']='absent';bad.append(p)
    p=copy.deepcopy(proposal);p['relation_edits'].pop(2);bad.append(p)
    p=copy.deepcopy(proposal);p['input_detachments'].pop();bad.append(p)
    p=copy.deepcopy(proposal);p['input_bindings'].append({'word':'target','source_persona':1,'source_head':'anchor','concept':1});bad.append(p)
    p=copy.deepcopy(proposal);p['remove_own'].append({'word':'committed','concept':1});bad.append(p)
    for operation_name,entry in [('moves',{'concept':1,'from_topic':'x1','to_topic':'x3'}),('remove_input',{'word':'target','concept':1}),('personas',{'word':'new','concept':1}),('inherit',{'word':'new','from':'anchor','concepts':[1]}),('relation_additions',operation())]:
        p=copy.deepcopy(proposal);p[operation_name]=[entry];bad.append(p)
    p=copy.deepcopy(proposal);p['relation_edits'][2]={'mode':'transfer_new_head','source':p['relation_edits'][2]['source'],'target':p['relation_edits'][0]['target'],'rank':2};bad.append(p)
    p=copy.deepcopy(proposal);p['relation_edits'][3]['target']=p['relation_edits'][0]['target'];bad.append(p)
    p=copy.deepcopy(proposal);p['relation_edits'][0]['target']['persona']=7;p['remove_own'].append({'word':'finite input','concept':2});p['personas']=[{'word':'new','concept':1}];bad.append(p)
    digest=corrections.file_digest(base)
    for number,p in enumerate(bad):
        output=root/f'exact-bad-{number}'
        try:corrections.apply_corrections(base,p,output)
        except ValueError:pass
        else:raise AssertionError(f'exact invalid plan accepted: {number}')
        assert not output.exists()
    original=corrections.apply_input_detachments
    def fail(connection,p):
        original(connection,p)
        raise ValueError('injected after exact edits and detach')
    with patch.object(corrections,'apply_input_detachments',fail):
        try:corrections.apply_corrections(base,proposal,root/'exact-rollback')
        except ValueError:pass
        else:raise AssertionError('exact rollback injection ignored')
    pending=next((root/'exact-rollback').glob('*/.dataset.sqlite3.pending'))
    assert not (pending.parent/'dataset.sqlite3').exists() and (pending.parent/'failed-build.json').exists()
    with sqlite3.connect(pending) as db:
        db.execute('ATTACH DATABASE ? AS original',(base.resolve().as_uri()+'?mode=ro',))
        for table in list(corrections.TABLES)+['sense_meta','input_relations']:
            assert not db.execute(f'SELECT * FROM main.{table} EXCEPT SELECT * FROM original.{table}').fetchall()
            assert not db.execute(f'SELECT * FROM original.{table} EXCEPT SELECT * FROM main.{table}').fetchall()
    rename=Path.rename
    def fail_rename(path,target):
        if path.name=='.dataset.sqlite3.pending':raise OSError('injected exact publication failure')
        return rename(path,target)
    with patch.object(Path,'rename',fail_rename):
        try:corrections.apply_corrections(base,proposal,root/'exact-publication')
        except OSError:pass
        else:raise AssertionError('exact publication injection ignored')
    assert not list((root/'exact-publication').glob('*/dataset.sqlite3'))
    assert list((root/'exact-publication').glob('*/failed-build.json'))
    import shutil
    blocked=root/'exact-dependent.sqlite3';shutil.copyfile(base,blocked)
    with sqlite3.connect(blocked) as db:db.execute("INSERT INTO input_relations VALUES ('target',1,'blocked','blocked','near',10)")
    try:corrections.apply_corrections(blocked,proposal,root/'exact-dependent')
    except ValueError:pass
    else:raise AssertionError('dependent exact binding detached')
    assert not (root/'exact-dependent').exists()
    assert corrections.file_digest(base)==digest
    return len(bad)+1


def reclassification_fixture(root, rule, extended):
    base,_=exact_fixture(root,rule,extended)
    with sqlite3.connect(base) as db:
        db.executemany('INSERT INTO relations VALUES (?,?,?,?,?,?)',[
            (4,8,'finiteinput','finite input','near',5),
            (1,40,'kindchange','kind change','co_use',1),
            (5,9,'kindchange','kind change','near',6),
            (1,41,'held','held','near',1)])
        db.execute("INSERT INTO input_senses VALUES ('finiteinput',3)")
        if extended:db.execute("INSERT INTO input_relations VALUES ('finiteinput',3,'otheredge','other edge','near',11)")
        proposal={'schema_version':corrections.CORRECTION_SCHEMA,'base_dataset':'fixture-base',
            'relation_edits':[
                {'mode':'reclassify_new_head','source':exact_identity(db,1,10),'target':exact_identity(db,5),'rank':9},
                {'mode':'consolidate_reviewed_existing','source':exact_identity(db,1,20),'target':exact_identity(db,5,8)},
                {'mode':'consolidate_reviewed_existing','source':exact_identity(db,1,40),'target':exact_identity(db,6,9)}],
            'input_detachments':[{'word_key':'finiteinput','source':exact_identity(db,1)}],
            'input_bindings':[{'word':'finite input','source_persona':5,'source_head':'beyond','concept':2}]}
    return base,proposal


def reclassification_success(root, rule, extended):
    base,proposal=reclassification_fixture(root,rule,extended);digest=corrections.file_digest(base)
    built=corrections.apply_corrections(base,proposal,root/'out');after=Path(built['path'])
    assert built['schema_version']==('rt-sense/3' if extended else 'rt-sense/2')
    with sqlite3.connect(after) as db:
        db.execute('ATTACH DATABASE ? AS original',(base.resolve().as_uri()+'?mode=ro',))
        removed={(1,10,'storedliteral','stored literal','synonym',1),(1,20,'finiteinput','finite input','near',1),(1,40,'kindchange','kind change','co_use',1)}
        added={(4,9,'storedliteral','stored literal','synonym',5)}
        for table in list(corrections.TABLES)+['sense_meta']+(['input_relations'] if extended else []):
            a=set(db.execute(f'SELECT * FROM main.{table} EXCEPT SELECT * FROM original.{table}'))
            r=set(db.execute(f'SELECT * FROM original.{table} EXCEPT SELECT * FROM main.{table}'))
            if table=='relations':assert a==added and r==removed
            elif table=='input_senses':assert a=={('finiteinput',5)} and r=={('finiteinput',1)}
            elif table=='sense_meta':assert {x[0] for x in a|r}<={'dataset_id','base_dataset','correction_schema','correction_plan_sha256'}
            else:assert not a and not r,table
        assert db.execute('SELECT * FROM relations WHERE term_id=5 AND rank=9').fetchone()==(5,9,'kindchange','kind change','near',6)
        assert db.execute("SELECT persona FROM input_senses WHERE word_key='finiteinput' ORDER BY persona").fetchall()==[(3,),(5,)]
        assert db.execute('SELECT 1 FROM personas WHERE persona=1').fetchone()
        assert db.execute('PRAGMA foreign_key_check').fetchall()==[]
    for split in [False,True]:
        for exact in [False,True]:
            actual=result(after,'beyond',split,is_exact_first=exact)
            assert 'stored literal' in candidates(actual) and 'stored literal' in lookup.render_text([actual])
            assert 'stored literal' not in candidates(result(after,'anchor',split,is_exact_first=exact))
            assert 'stored literal' not in candidates(result(after,'beyond',split,is_direct_included=False,is_exact_first=exact))
            reverse=result(after,'stored literal',split,is_reverse_included=True,is_exact_first=exact)
            assert {w for group in reverse['reverse'] for w in group['words']}=={'beyond'}
            assert result(after,'stored literal',split)['reverse']==[]
        index=lookup.SenseIndex.load(after,is_topic_split=split)
        assert any(not s.is_own and 2 in s.concepts and 'beyond' in s.direct for s in index.senses_of('finiteinput'))
    assert corrections.file_digest(base)==digest
    return base,proposal


def reclassification_rejections(root, base, proposal):
    bad=[]
    for section in ['source','target']:
        for key,value in [('term_id',999),('head','ABSENT'),('persona',True),('concept',999),('topic','absent'),('sense',999),('field','absent'),('rank',99),('word_key','absent'),('surface','changed'),('kind','synonym')]:
            p=copy.deepcopy(proposal);p['relation_edits'][1][section][key]=value;bad.append(p)
    for key,value in [('rank',True),('rank',-1),('rank',8),('extra',1)]:
        p=copy.deepcopy(proposal);p['relation_edits'][0][key]=value;bad.append(p)
    p=copy.deepcopy(proposal);del p['relation_edits'][1]['target']['kind'];bad.append(p)
    p=copy.deepcopy(proposal);p['relation_edits'][1]['rank']=8;bad.append(p)
    with sqlite3.connect(base) as db:
        for target in [exact_identity(db,1),exact_identity(db,6),exact_identity(db,7)]:
            p=copy.deepcopy(proposal);p['relation_edits'][0]['target']=target
            if target['persona']==7:p['relation_edits'][0]['source']=exact_identity(db,1,20)
            bad.append(p)
        for target in [exact_identity(db,1,20),exact_identity(db,6,8),exact_identity(db,6,9)]:
            p=copy.deepcopy(proposal);p['relation_edits'][1]['target']=target;bad.append(p)
        p=copy.deepcopy(proposal);p['relation_edits'][0]['mode']='transfer_new_head';bad.append(p)
        for index in [1,2]:
            p=copy.deepcopy(proposal);old=p['relation_edits'][index];target={k:v for k,v in old['target'].items() if k not in ['rank','word_key','surface','kind']}
            p['relation_edits'][index]={'mode':'consolidate_existing','source':old['source'],'target':target,'rank':old['target']['rank']};bad.append(p)
        p=copy.deepcopy(proposal);p['relation_edits'].append(copy.deepcopy(p['relation_edits'][0]));bad.append(p)
        p=copy.deepcopy(proposal);p['relation_edits'][2]={'mode':'reclassify_new_head','source':exact_identity(db,1,40),'target':exact_identity(db,5),'rank':9};bad.append(p)
        p=copy.deepcopy(proposal);p['relation_edits'].append({'mode':'remove','source':exact_identity(db,5,8)});bad.append(p)
        p=copy.deepcopy(proposal);p['remove_own']=[{'word':'beyond','concept':2}];bad.append(p)
        p=copy.deepcopy(proposal);p['input_bindings'].append({'word':'finite input','source_persona':1,'source_head':'anchor','concept':1});bad.append(p)
    for index,p in enumerate(bad):
        output=root/f'reclassification-bad-{index}'
        try:corrections.apply_corrections(base,p,output)
        except ValueError:pass
        else:raise AssertionError(f'reclassification invalid plan accepted: {index}')
        assert not output.exists()
    import shutil
    mutations=[(0,'source',1,10,'kind','mystery'),(2,'target',5,9,'kind','family'),(1,'target',4,8,'surface','finite-input')]
    for index,(operation,section,term,rank,column,value) in enumerate(mutations):
        changed=root/f'reclassification-mutated-{index}.sqlite3';shutil.copyfile(base,changed)
        with sqlite3.connect(changed) as db:db.execute(f'UPDATE relations SET {column}=? WHERE term_id=? AND rank=?',(value,term,rank))
        p=copy.deepcopy(proposal);p['relation_edits'][operation][section][column]=value;output=root/f'reclassification-mutated-out-{index}'
        try:corrections.apply_corrections(changed,p,output)
        except ValueError:pass
        else:raise AssertionError(f'invalid reviewed kind/literal accepted: {index}')
        assert not output.exists()
    for index in range(3):
        changed=root/f'reclassification-conflict-{index}.sqlite3';shutil.copyfile(base,changed)
        p=copy.deepcopy(proposal)
        with sqlite3.connect(changed) as db:
            if index==0:
                db.execute("INSERT INTO input_relations VALUES ('finiteinput',1,'blocked','blocked','near',12)")
            elif index==1:
                db.execute("INSERT INTO relations VALUES (3,25,'oldroute','old route','near',4)")
                p['relation_edits'].append({'mode':'transfer_new_head','source':exact_identity(db,4,25),'target':exact_identity(db,5),'rank':9})
            else:
                db.execute("INSERT INTO relations VALUES (4,88,'storedliteral','stored literal','synonym',5)")
        output=root/f'reclassification-conflict-out-{index}'
        try:corrections.apply_corrections(changed,p,output)
        except ValueError:pass
        else:raise AssertionError(f'reviewed dependency/rank/key conflict accepted: {index}')
        assert not output.exists()
    return len(bad)+len(mutations)+3


def reclassification_failures(root, base, proposal):
    import reviewed_graph_edits as graph
    original_edits=corrections.apply_relation_edits;original_detach=corrections.apply_input_detachments
    digest=corrections.file_digest(base)
    def fail_edits(connection,p):
        original_edits(connection,p)
        raise ValueError('injected after reviewed native edits')
    def fail_detach(connection,p):
        original_detach(connection,p)
        raise ValueError('injected after reviewed input edits')
    def stale_recipient(connection,p):
        connection.execute("UPDATE relations SET kind='co_use' WHERE term_id=4 AND rank=8")
        return original_edits(connection,p)
    def zero_rowcount(connection,p):
        class ConditionalWriteFault:
            def execute(self,sql,args=()):
                cursor=connection.execute(sql,args)
                if sql.startswith('DELETE FROM relations WHERE'):
                    from types import SimpleNamespace
                    return SimpleNamespace(rowcount=0)
                return cursor
        return graph.apply_relation_edits(ConditionalWriteFault(),p)
    for index,(name,fail) in enumerate([('apply_relation_edits',fail_edits),('apply_input_detachments',fail_detach),('apply_relation_edits',stale_recipient),('apply_relation_edits',zero_rowcount)]):
        output=root/f'reclassification-rollback-{index}'
        with patch.object(corrections,name,fail):
            try:corrections.apply_corrections(base,proposal,output)
            except ValueError:pass
            else:raise AssertionError('reviewed transaction fault ignored')
        pending=next(output.glob('*/.dataset.sqlite3.pending'))
        assert not (pending.parent/'dataset.sqlite3').exists() and (pending.parent/'failed-build.json').exists()
        with sqlite3.connect(pending) as db:
            db.execute('ATTACH DATABASE ? AS original',(base.resolve().as_uri()+'?mode=ro',))
            for (table,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").fetchall():
                assert not db.execute(f'SELECT * FROM main.{table} EXCEPT SELECT * FROM original.{table}').fetchall()
                assert not db.execute(f'SELECT * FROM original.{table} EXCEPT SELECT * FROM main.{table}').fetchall()
    rename=Path.rename
    def fail_rename(path,target):
        if path.name=='.dataset.sqlite3.pending':raise OSError('injected reviewed publication failure')
        return rename(path,target)
    output=root/'reclassification-publication'
    with patch.object(Path,'rename',fail_rename):
        try:corrections.apply_corrections(base,proposal,output)
        except OSError:pass
        else:raise AssertionError('reviewed publication fault ignored')
    assert not list(output.glob('*/dataset.sqlite3')) and list(output.glob('*/failed-build.json')) and list(output.glob('*/unpublished-build-report.json'))
    assert corrections.file_digest(base)==digest


def input_transfer_fixture(root, rule, extended):
    base,_=exact_fixture(root,rule,extended)
    with sqlite3.connect(base) as db:
        db.execute("UPDATE relations SET kind='co_use' WHERE term_id=1 AND rank=10")
        db.execute("INSERT INTO input_senses VALUES ('anchor',5)")
        db.execute("INSERT INTO terms VALUES (8,'Stored Literal','storedliteral','[]')")
        db.execute("INSERT INTO relations VALUES (6,90,'secondhoponly','second hop only','near',7)")
        proposal={'schema_version':corrections.CORRECTION_SCHEMA,'base_dataset':'fixture-base','relation_edits':[
            {'mode':'transfer_to_existing_input_binding','source':exact_identity(db,1,10),
             'target':dict(exact_identity(db,5),input_word_key='anchor'),'rank':3}]}
    return base,proposal


def input_transfer_success(root, rule, extended):
    base,proposal=input_transfer_fixture(root,rule,extended);digest=corrections.file_digest(base)
    built=corrections.apply_corrections(base,proposal,root/'out');after=Path(built['path'])
    assert built['schema_version']=='rt-sense/3' and built['dataset_id'].startswith('rts3-')
    assert {k:v for k,v in built['delta'].items() if v}=={'relations':-1,'input_relations':1}
    assert built['operations']['relation_edits_transfer_to_existing_input_binding']==1
    with sqlite3.connect(after) as db:
        db.execute('ATTACH DATABASE ? AS original',(base.resolve().as_uri()+'?mode=ro',))
        for table in corrections.TABLES:
            added=set(db.execute(f'SELECT * FROM main.{table} EXCEPT SELECT * FROM original.{table}'))
            removed=set(db.execute(f'SELECT * FROM original.{table} EXCEPT SELECT * FROM main.{table}'))
            assert not added
            assert removed==({(1,10,'storedliteral','stored literal','co_use',1)} if table=='relations' else set()),table
        assert db.execute("SELECT * FROM input_relations WHERE input_word_key='anchor'").fetchall()==[('anchor',5,'storedliteral','stored literal','co_use',3)]
        assert db.execute("SELECT * FROM input_relations WHERE input_word_key<>'anchor'").fetchall()==([('target',3,'targetedge','target edge','near',9)] if extended else [])
        assert db.execute('PRAGMA foreign_key_check').fetchall()==[]
    for split in [False,True]:
        index=lookup.SenseIndex.load(after,is_topic_split=split)
        assert any(not s.is_own and s.concepts==frozenset({2}) and 'storedliteral' in s.direct for s in index.senses_of('anchor'))
        assert all('storedliteral' not in s.direct for s in index.senses_of('beyond'))
        assert 'anchor' not in index.concept_words[2] and 'anchor' not in index.topic_words['x2']
        for exact in [False,True]:
            actual=result(after,'anchor',split,is_exact_first=exact)
            assert 'Stored Literal' in candidates(actual) and 'second hop only' not in candidates(actual)
            assert 'Stored Literal' in lookup.render_text([actual])
            assert result(base,'anchor',split,is_exact_first=exact,is_direct_included=False)==result(after,'anchor',split,is_exact_first=exact,is_direct_included=False)
            reverse=result(after,'stored literal',split,is_reverse_included=True,is_exact_first=exact)
            assert {w for r in reverse['reverse'] for w in r['words']}=={'anchor'}
            assert result(after,'stored literal',split)['reverse']==[]
        for word in ['beyond','peer']:
            assert result(base,word,split)==result(after,word,split),word
        for word in ['target','finite input']:
            before_result,after_result=result(base,word,split),result(after,word,split)
            if rule=='head':assert before_result==after_result,word
            else:
                # Legacy sibling inputs lose the deliberately removed native donor,
                # but do not inherit the new relation scoped only to anchor/5.
                assert candidates(before_result)-candidates(after_result)=={'Stored Literal'},word
                assert not candidates(after_result)-candidates(before_result),word
    assert corrections.file_digest(base)==digest
    return base,proposal


def input_transfer_scope_isolation(root):
    base,proposal=input_transfer_fixture(root,'head',True)
    with sqlite3.connect(base) as db:
        db.executemany('INSERT INTO input_senses VALUES (?,?)',[('anchor',7),('committed',5)])
        db.executemany('INSERT INTO relations VALUES (?,?,?,?,?,?)',[
            (1,60,'otherclassifier','other classifier','near',1),(5,61,'otherinput','other input','near',6)])
        for persona,rank,target,key in [(1,60,7,'anchor'),(6,61,5,'committed')]:
            proposal['relation_edits'].append({'mode':'transfer_to_existing_input_binding','source':exact_identity(db,persona,rank),
                'target':dict(exact_identity(db,target),input_word_key=key),'rank':3})
        proposal['relation_edits'].append({'mode':'reclassify_new_head','source':exact_identity(db,3,7),'target':exact_identity(db,5),'rank':3})
    built=corrections.apply_corrections(base,proposal,root/'out')
    with sqlite3.connect(built['path']) as db:
        assert db.execute('SELECT input_word_key,source_persona,related_word_key,rank FROM input_relations WHERE rank=3 ORDER BY 1,2').fetchall()==[
            ('anchor',5,'storedliteral',3),('anchor',7,'otherclassifier',3),('committed',5,'otherinput',3)]
        assert db.execute('SELECT word_key,rank FROM relations WHERE term_id=4').fetchall()==[('peeronly',3)]


def input_transfer_rejections(root, base, proposal):
    bad=[]
    for section in ['source','target']:
        for key,value in [('term_id',999),('head','ANCHOR'),('persona',True),('concept',999),('topic','absent'),('sense',999),('field','absent')]:
            p=copy.deepcopy(proposal);p['relation_edits'][0][section][key]=value;bad.append(p)
    for key,value in [('rank',99),('word_key','absent'),('surface','changed'),('kind','near')]:
        p=copy.deepcopy(proposal);p['relation_edits'][0]['source'][key]=value;bad.append(p)
    for key,value in [('rank',True),('rank',None),('rank',-1),('kind','near'),('extra',1)]:
        p=copy.deepcopy(proposal);p['relation_edits'][0][key]=value;bad.append(p)
    for key,value in [('input_word_key','Anchor'),('input_word_key','target'),('input_word_key',None),('input_word_key','beyond'),('origin','own'),('kind','co_use')]:
        p=copy.deepcopy(proposal);p['relation_edits'][0]['target'][key]=value;bad.append(p)
    for section,key in [('target','input_word_key'),('source','kind')]:
        p=copy.deepcopy(proposal);del p['relation_edits'][0][section][key];bad.append(p)
    p=copy.deepcopy(proposal);del p['relation_edits'][0]['rank'];bad.append(p)
    p=copy.deepcopy(proposal);p['relation_edits'].append(copy.deepcopy(p['relation_edits'][0]));bad.append(p)
    with sqlite3.connect(base) as db:
        for persona in [1,6,7]:
            p=copy.deepcopy(proposal);p['relation_edits'][0]['target']=dict(exact_identity(db,persona),input_word_key='anchor');bad.append(p)
        p=copy.deepcopy(bad[-1]);p['input_bindings']=[{'word':'anchor','source_persona':7,'source_head':'finite input','concept':2}];bad.append(p)
        p=copy.deepcopy(proposal);p['relation_edits'][0]['source']=exact_identity(db,1,30);bad.append(p)
        p=copy.deepcopy(proposal);p['relation_edits'].append({'mode':'transfer_to_existing_input_binding','source':exact_identity(db,1,20),'target':p['relation_edits'][0]['target'],'rank':3});bad.append(p)
        p=copy.deepcopy(proposal);p['input_detachments']=[{'word_key':'anchor','source':exact_identity(db,5)}];bad.append(p)
        p=copy.deepcopy(bad[-1]);p['remove_own']=[{'word':'beyond','concept':2}];bad.append(p)
        p=copy.deepcopy(proposal);p['relation_edits'].append({'mode':'reclassify_new_head','source':exact_identity(db,3,7),'target':exact_identity(db,1),'rank':60});bad.append(p)
    for operation_name,value in [('remove_input',{'word':'anchor','concept':2}),('moves',{'concept':2,'from_topic':'x2','to_topic':'x3'}),('personas',{'word':'new','concept':2}),('inherit',{'word':'new','from':'anchor','concepts':[1]}),('relation_additions',operation())]:
        p=copy.deepcopy(proposal);p[operation_name]=[value];bad.append(p)
    digest=corrections.file_digest(base)
    for n,p in enumerate(bad):
        out=root/f'input-transfer-bad-{n}'
        try:corrections.apply_corrections(base,p,out)
        except ValueError:pass
        else:raise AssertionError(f'input transfer invalid plan accepted: {n}')
        assert not out.exists()
    import shutil
    changes=["INSERT INTO input_relations VALUES ('anchor',5,'storedliteral','stored literal','co_use',99)",
             "INSERT INTO input_relations VALUES ('anchor',5,'different','different','near',3)",
             "DELETE FROM input_senses WHERE word_key='anchor' AND persona=5",
             "UPDATE relations SET word_key='anchor',surface='anchor' WHERE term_id=1 AND rank=10",
             "UPDATE relations SET word_key='beyond',surface='beyond' WHERE term_id=1 AND rank=10"]
    for n,sql in enumerate(changes):
        changed=root/f'input-transfer-change-{n}.sqlite3';shutil.copyfile(base,changed);p=copy.deepcopy(proposal)
        with sqlite3.connect(changed) as db:
            db.execute(sql)
            if n>=3:p['relation_edits'][0]['source']=exact_identity(db,1,10)
        out=root/f'input-transfer-change-out-{n}'
        try:corrections.apply_corrections(changed,p,out)
        except ValueError:pass
        else:raise AssertionError(f'input transfer occupied/missing/self accepted: {n}')
        assert not out.exists()
    assert corrections.file_digest(base)==digest
    return len(bad)+len(changes)


def input_transfer_failures(root, base, proposal):
    original=corrections.apply_relation_edits;digest=corrections.file_digest(base)
    with sqlite3.connect(base) as db:is_extended=edges.has_input_relations(db)
    stages=['after_insert','after_delete','zero_insert','zero_delete','late_binding','late_rank','late_source','finish']
    if not is_extended:stages.append('after_ddl')
    for stage in stages:
        def fail(connection,p):
            if stage=='late_binding':connection.execute("DELETE FROM input_senses WHERE word_key='anchor' AND persona=5")
            if stage=='late_rank':connection.execute("INSERT INTO input_relations VALUES ('anchor',5,'collision','collision','near',3)")
            if stage=='late_source':connection.execute("UPDATE relations SET kind='near' WHERE term_id=1 AND rank=10")
            class ConditionalWriteFault:
                def execute(self,sql,args=()):
                    cursor=connection.execute(sql,args)
                    action='insert' if sql.startswith('INSERT INTO input_relations') else 'delete' if sql.startswith('DELETE FROM relations WHERE') else None
                    if action and stage=='after_'+action:raise ValueError('injected '+stage)
                    if action and stage=='zero_'+action:
                        from types import SimpleNamespace
                        return SimpleNamespace(rowcount=0)
                    return cursor
            return original(ConditionalWriteFault(),p)
        finish=corrections.CorrectionEditor.finish
        def fail_finish(editor):
            finish(editor);raise ValueError('injected finish failure')
        create=corrections.create_input_relations
        def fail_create(connection):
            create(connection);raise ValueError('injected after DDL failure')
        name='finish' if stage=='finish' else 'create_input_relations' if stage=='after_ddl' else 'apply_relation_edits'
        owner=corrections.CorrectionEditor if stage=='finish' else corrections
        replacement=fail_finish if stage=='finish' else fail_create if stage=='after_ddl' else fail
        out=root/f'input-transfer-failure-{is_extended}-{stage}'
        with patch.object(owner,name,replacement):
            try:corrections.apply_corrections(base,proposal,out)
            except ValueError:pass
            else:raise AssertionError(f'input transfer fault ignored: {stage}')
        pending=next(out.glob('*/.dataset.sqlite3.pending'))
        assert not (pending.parent/'dataset.sqlite3').exists() and (pending.parent/'failed-build.json').exists()
        with sqlite3.connect(pending) as db:
            assert edges.has_input_relations(db)==is_extended
            db.execute('ATTACH DATABASE ? AS original',(base.resolve().as_uri()+'?mode=ro',))
            for table in list(corrections.TABLES)+['sense_meta']+(['input_relations'] if is_extended else []):
                assert not db.execute(f'SELECT * FROM main.{table} EXCEPT SELECT * FROM original.{table}').fetchall()
                assert not db.execute(f'SELECT * FROM original.{table} EXCEPT SELECT * FROM main.{table}').fetchall()
    rename=Path.rename
    def fail_rename(path,target):
        if path.name=='.dataset.sqlite3.pending':raise OSError('input transfer publication failure')
        return rename(path,target)
    out=root/f'input-transfer-publication-{is_extended}'
    with patch.object(Path,'rename',fail_rename):
        try:corrections.apply_corrections(base,proposal,out)
        except OSError:pass
        else:raise AssertionError('input transfer publication fault ignored')
    assert not list(out.glob('*/dataset.sqlite3')) and list(out.glob('*/unpublished-build-report.json'))
    with sqlite3.connect(next(out.glob('*/.dataset.sqlite3.pending'))) as db:
        assert db.execute("SELECT count(*) FROM input_relations WHERE input_word_key='anchor'").fetchone()==(1,)
    assert corrections.file_digest(base)==digest


def main():
    with tempfile.TemporaryDirectory(prefix='verify-reviewed-relations-') as temp:
        root=Path(temp)
        base,db=success(root/'head','head')
        success(root/'legacy',None)
        rejected=invalid_plans(root,base)
        failures_and_layout(root,base,db)
        for rule,extended in [('head',False),(None,False),('head',True),(None,True)]:
            exact_base,exact_plan=exact_success(root/f'exact-{rule}-{extended}',rule,extended)
        exact_rejected=exact_rejections_and_rollback(root,exact_base,exact_plan)
        for rule,extended in [('head',False),(None,False),('head',True),(None,True)]:
            reviewed_base,reviewed_plan=reclassification_success(root/f'reclassified-{rule}-{extended}',rule,extended)
        reviewed_rejected=reclassification_rejections(root,reviewed_base,reviewed_plan)
        reclassification_failures(root,reviewed_base,reviewed_plan)
        for rule,extended in [('head',False),(None,False),('head',True),(None,True)]:
            input_base,input_plan=input_transfer_success(root/f'input-transfer-{rule}-{extended}',rule,extended)
            if rule=='head':input_transfer_failures(root,input_base,input_plan)
        input_rejected=input_transfer_rejections(root,input_base,input_plan)
        input_transfer_scope_isolation(root/'input-transfer-isolation')
    print(json.dumps({'pass':True,'rejected_before_reservation':rejected,'head_and_legacy':True,'origin_scoped':True,
                      'one_hop':True,'direct_and_reverse_options':True,'explicit_spelling_path':True,'idempotence':True,
                      'source_kind_and_literal_preserved':True,'transactional_DDL_rollback':True,'malformed_extension_rejected':True,
                      'exact_edit_schema_rule_combinations':4,'exact_rejected_before_reservation':exact_rejected,
                      'exact_input_sibling_preservation':True,'exact_all_modes_rollback':True,'exact_publication_failure_safe':True,
                      'reviewed_reclassification_schema_rule_combinations':4,'reviewed_rejected_before_reservation':reviewed_rejected,
                      'reviewed_recipient_preserved':True,'reviewed_transaction_rollback':True,'reviewed_publication_failure_safe':True,
                      'input_transfer_schema_rule_combinations':4,'input_transfer_rejected_before_reservation':input_rejected,
                      'input_transfer_scoped_payload_and_origin':True,'input_transfer_terms_first_and_one_hop':True,
                      'input_transfer_transaction_failures':17,'input_transfer_publication_failures':2,
                      'input_transfer_separate_rank_spaces':True},indent=2))
if __name__=='__main__':main()
