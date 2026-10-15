"""Verify explicit spelling branches using synthetic data and the real CLI entrypoint."""
import contextlib
import io
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
from unittest.mock import patch

import sense_lookup as lookup
from spelling_aliases import CONFIRMED_SPELLINGS, spelling_queries, spelling_variants
import verify_sense_additions as fixtures


def candidates(result):
    return {word for sense in result['senses'] for ring in ('direct','near','same_topic') for word in sense[ring]}


def main():
    with tempfile.TemporaryDirectory(prefix='verify-explicit-spellings-') as temporary:
        root=Path(temporary)
        base=fixtures.fixture(root/'base')
        entries=root/'entries.json'
        entries.write_text(json.dumps({
            'fields':[{'field':'security','label':'Security','scope':'Synthetic'}],
            'topics':[{'key':key,'label':key,'scope':'Synthetic','fields':[field]} for key,field in
                      [('policy-topic','security'),('original-base-topic','testing'),('second-hop-topic','security'),('count-topic','testing')]],
            'concepts':[{'key':key,'name':key,'topic':topic} for key,topic in
                        [('policy','policy-topic'),('police','original-base-topic'),('second-hop','second-hop-topic'),('count','count-topic')]],
            'personas':[{'word':word,'concept':concept} for word,concept in
                        [('polices',1),('policing-peer',1),('conut',1),('policy','policy'),('policy-peer','policy'),
                         ('police','police'),('original-base-peer','police'),('policy-peer','second-hop'),('beyond','second-hop'),
                         ('count','count'),('count-peer','count')]]
        }))
        built=fixtures.builder.add_entries(fixtures.arguments(base,entries,root/'datasets'))
        database=Path(built['path'])
        with sqlite3.connect(database) as db:
            persona,term=db.execute("SELECT p.persona,p.term_id FROM personas p JOIN terms t USING(term_id) WHERE t.word_key='policy'").fetchone()
            db.execute("INSERT INTO relations VALUES (?,1,'policies','policies','form',?)",(term,persona))
            db.execute("INSERT INTO input_senses VALUES ('policies',?)",(persona,))
        index=lookup.SenseIndex.load(database,is_topic_split=True)
        matches={
            'polices':[{'term':'police','span':'polices','scope':'whole','match':'lemma'}],
            'policies':[{'term':'policy','span':'policies','scope':'whole','match':'lemma'}],
        }
        options={'is_exact_first':True,'is_inflection_evidence_used':False}
        before=lookup.lookup('polices',matches['polices'],index,**options)
        after=lookup.lookup('polices',matches['polices'],index,spelling_matches=matches,**options)
        assert {'anchor','policing-peer','conut'}<=candidates(before)
        assert candidates(after)==candidates(before)|{'policy-peer'}
        assert {s['field'] for s in after['senses']}=={'testing','security'}
        assert not {'policy','policies','polices','beyond','count-peer','original-base-peer'}&candidates(after)
        assert lookup.describe_resolution(after).startswith('polices → polices + policies')
        relaxed=lookup.lookup('polices',matches['polices'],index,spelling_matches=matches,is_exact_first=False,is_inflection_evidence_used=False)
        assert 'original-base-peer' in candidates(relaxed)
        # A mapping for a different input cannot change status or original matching.
        assert lookup.lookup('unregistered',[],index,spelling_matches=matches,**options)==lookup.lookup('unregistered',[],index,**options)
        assert spelling_variants('policess')==('policess',)
        assert spelling_variants('POLICES')[-1]=='policies'
        assert spelling_variants('replicy')[-1]=='policy'
        assert len(CONFIRMED_SPELLINGS)==11
        assert spelling_queries(['polices','policess'])==['polices','policess','policies']
        assert not set(CONFIRMED_SPELLINGS.values())&set(CONFIRMED_SPELLINGS), 'no chained spelling rewrites'

        calls=[]
        def matcher(queries,rtpoc3,dataset):
            calls.append((queries,str(rtpoc3),str(dataset)))
            return matches
        argv=['sense_lookup.py','polices','--sense-dataset',str(database),'--dataset','chosen-lexical.sqlite3',
              '--rtpoc3','chosen-matcher','--no-evidence-inflections','--skip-syntax-parts','--format','json']
        real_resolve=lookup.cl.resolve
        with patch.object(lookup.cl,'run_rtpoc3',side_effect=matcher), patch.object(lookup.cl,'resolve',wraps=real_resolve) as resolve, patch.object(sys,'argv',argv), contextlib.redirect_stdout(io.StringIO()) as stdout:
            lookup.main()
        cli=json.loads(stdout.getvalue())
        assert candidates(cli)==candidates(after)
        assert calls==[(['polices','policies'],'chosen-matcher','chosen-lexical.sqlite3')]
        assert [call.args[0] for call in resolve.call_args_list]==['polices','policies']
        assert all(call.args[3] is True and call.args[5] is None for call in resolve.call_args_list)
        assert all('polices' in call.args[4] and 'policies' not in call.args[4] for call in resolve.call_args_list)
        # Evidence is loaded from the caller's dataset once and passed to both final resolvers.
        class Evidence:
            def rank(self,span,base):return 0
        evidence=Evidence()
        with patch.object(lookup.cl,'run_rtpoc3',side_effect=matcher), patch.object(lookup.cl.InflectionEvidence,'load',return_value=evidence) as load, patch.object(lookup.cl,'resolve',wraps=real_resolve) as resolve, patch.object(sys,'argv',[a for a in argv if a!='--no-evidence-inflections']), contextlib.redirect_stdout(io.StringIO()) as stdout:
            lookup.main()
        load.assert_called_once_with('chosen-lexical.sqlite3')
        assert all(call.args[5] is evidence for call in resolve.call_args_list)
        assert candidates(json.loads(stdout.getvalue()))==candidates(after)
    print(json.dumps({'pass':True,'confirmed_pairs':11,'original_meanings_preserved':True,
        'corrected_input_only_lemma_resolved':True,'no_fuzzy_or_recursive_aliases':True,
        'no_returned_word_expansion':True,'caller_exact_first_choice_preserved':True,
        'cli_query_expansion_and_custom_paths_verified':True,'syntax_and_inflection_options_reach_both_resolvers':True,
        'final_cli_candidates_verified':True},indent=2))

if __name__=='__main__':main()
