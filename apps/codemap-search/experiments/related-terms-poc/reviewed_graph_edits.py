"""Exact, reviewed edge edits; no inferred targets or cascading deletion.

Opt-in correction plans use relation_edits and input_detachments. Persona identities
are {term_id, head, persona, concept, topic, sense, field}; relation sources also
include {rank, word_key, surface, kind}. remove has no target; reattribute_same_head
has a target identity; consolidate_existing/transfer_new_head also require rank.
reclassify_new_head requires rank and a different head/concept, preserving the donor
payload. consolidate_reviewed_existing requires a full existing target relation:
its key/literal must match, while concept or kind must differ. Neither new mode
changes the four legacy modes or permits an arbitrary new literal/kind.
transfer_to_existing_input_binding transfers a native donor to its own head's
existing input context in a different concept. Its target is a complete classifier
identity plus input_word_key; a separately supplied rank is required. The original
key/literal/kind are preserved, without creating an own membership or native edge
on the classifier head. An occupied scoped key/rank is rejected, not consolidated.
Input detachments are {word_key, source: persona_identity}.

These narrowly scoped plans may combine exact operations with input_bindings and
remove_own, not bulk moves/removals/inheritance/copies. Legacy-only plans retain
their original path. Every reference is checked in the unedited base before the
caller reserves output; conditional writes check donor tuples again in transaction.
"""
from collections import Counter

from reviewed_relations import INPUT_COLUMNS, RELATION_KINDS, checked_persona, has_input_relations
from term_canonical import word_key

PERSONA_KEYS = frozenset(('term_id', 'head', 'persona', 'concept', 'topic', 'sense', 'field'))
ROW_KEYS = frozenset(('rank', 'word_key', 'surface', 'kind'))
RELATION_COLUMNS = ('term_id', 'rank', 'word_key', 'surface', 'kind', 'persona')
RELATION_WHERE = ' AND '.join(f'{column}=?' for column in RELATION_COLUMNS)
EXACT_OPERATIONS = frozenset(('relation_edits', 'input_detachments'))
INPUT_TRANSFER_MODE = 'transfer_to_existing_input_binding'
MODES = {'remove': {'mode', 'source'},
         'reattribute_same_head': {'mode', 'source', 'target'},
         'consolidate_existing': {'mode', 'source', 'target', 'rank'},
         'transfer_new_head': {'mode', 'source', 'target', 'rank'},
         'reclassify_new_head': {'mode', 'source', 'target', 'rank'},
         'consolidate_reviewed_existing': {'mode', 'source', 'target'},
         INPUT_TRANSFER_MODE: {'mode', 'source', 'target', 'rank'}}
INSERTION_MODES = frozenset(('transfer_new_head', 'reclassify_new_head'))


def has_exact_edits(plan):
    return any(plan.get(key) for key in EXACT_OPERATIONS)


def has_input_transfers(plan):
    return any(entry['mode'] == INPUT_TRANSFER_MODE for entry in plan.get('relation_edits', []))


def validate_identity_shape(value, is_relation=False, is_input=False):
    expected = PERSONA_KEYS | ROW_KEYS if is_relation else PERSONA_KEYS
    if is_input:
        expected = expected | {'input_word_key'}
    if not isinstance(value, dict) or set(value) != expected:
        raise ValueError('exact edit requires complete persona/row identity')
    integers = {'term_id', 'persona', 'concept', 'sense'}
    if any(type(value[key]) is not int or value[key] <= 0 for key in integers):
        raise ValueError('exact edit persona IDs must be positive integers')
    if is_relation and (type(value['rank']) is not int or value['rank'] < 0):
        raise ValueError('exact edit rank must be a nonnegative integer')
    for key in expected - integers - {'rank'}:
        if not isinstance(value[key], str) or not value[key].strip():
            raise ValueError('exact edit identity strings must be nonempty')
    if not word_key(value['head']):
        raise ValueError('exact edit head must have a canonical key')


def validate_exact_shapes(plan):
    for operation in EXACT_OPERATIONS:
        if not isinstance(plan.get(operation, []), list):
            raise ValueError(f'{operation} must be an array')
    for entry in plan.get('relation_edits', []):
        if not isinstance(entry, dict) or not isinstance(entry.get('mode'), str) or entry['mode'] not in MODES:
            raise ValueError('unknown exact relation edit mode')
        if set(entry) != MODES[entry['mode']]:
            raise ValueError('unexpected exact relation edit keys')
        validate_identity_shape(entry['source'], True)
        if 'target' in entry:
            validate_identity_shape(entry['target'], entry['mode'] == 'consolidate_reviewed_existing',
                                    entry['mode'] == INPUT_TRANSFER_MODE)
        if 'rank' in entry and (type(entry['rank']) is not int or entry['rank'] < 0):
            raise ValueError('exact recipient rank must be a nonnegative integer')
    for entry in plan.get('input_detachments', []):
        if not isinstance(entry, dict) or set(entry) != {'word_key', 'source'}:
            raise ValueError('input_detachments requires word_key and source')
        if not isinstance(entry['word_key'], str) or not entry['word_key'].strip():
            raise ValueError('input detachment requires a nonempty stored key')
        validate_identity_shape(entry['source'])
    if has_exact_edits(plan) and any(plan.get(key) for key in ('moves', 'remove_input', 'personas', 'inherit', 'relation_additions')):
        raise ValueError('exact edge plans only combine with input_bindings and remove_own')


def validate_persona_identity(connection, expected):
    term = checked_persona(connection, expected['persona'], expected['head'], expected['concept'], expected['topic'])
    actual = connection.execute(
        'SELECT p.term_id,t.surface,p.sense,s.field FROM personas p JOIN terms t USING(term_id) '
        'JOIN senses s USING(sense) WHERE p.persona=?', (expected['persona'],)).fetchone()
    if term != expected['term_id'] or actual != (expected['term_id'], expected['head'], expected['sense'], expected['field']):
        raise ValueError('stale exact persona identity')


def relation_tuple(source):
    return tuple(source[column] for column in RELATION_COLUMNS)


def validate_source_row(connection, source):
    validate_persona_identity(connection, source)
    if not connection.execute(f'SELECT 1 FROM relations WHERE {RELATION_WHERE}', relation_tuple(source)).fetchone():
        raise ValueError('stale exact donor relation')


def recipient_tuple(entry):
    source, target = entry['source'], entry['target']
    if entry['mode'] == 'consolidate_reviewed_existing':
        return relation_tuple(target)
    return (target['term_id'], entry.get('rank', source['rank']), source['word_key'], source['surface'], source['kind'], target['persona'])


def input_recipient_tuple(entry):
    source, target = entry['source'], entry['target']
    return (target['input_word_key'], target['persona'], source['word_key'], source['surface'], source['kind'], entry['rank'])


def validate_input_recipient(connection, entry):
    source, target = entry['source'], entry['target']
    key, persona = target['input_word_key'], target['persona']
    if key != word_key(source['head']) or key == word_key(target['head']):
        raise ValueError('input transfer requires the donor head and a different classifier head')
    if source['concept'] == target['concept'] or source['persona'] == persona:
        raise ValueError('input transfer requires a different classifier concept/persona')
    if source['kind'] not in RELATION_KINDS:
        raise ValueError('unsupported input transfer donor kind')
    if source['word_key'] in (key, word_key(target['head'])):
        raise ValueError('input transfer must not create a self relation')
    if not connection.execute('SELECT 1 FROM terms WHERE word_key=?', (key,)).fetchone() or not connection.execute(
            'SELECT 1 FROM input_senses WHERE word_key=? AND persona=?', (key, persona)).fetchone():
        raise ValueError('input transfer requires an existing term and input binding')
    if has_input_relations(connection) and connection.execute(
            'SELECT 1 FROM input_relations WHERE input_word_key=? AND source_persona=? '
            'AND (related_word_key=? OR rank=?)', (key, persona, source['word_key'], entry['rank'])).fetchone():
        raise ValueError('occupied input transfer recipient key/rank')


def validate_recipient(connection, entry):
    source, target, mode = entry['source'], entry['target'], entry['mode']
    validate_persona_identity(connection, target)
    if mode == INPUT_TRANSFER_MODE:
        validate_input_recipient(connection, entry)
        return
    if mode == 'reattribute_same_head':
        if source['term_id'] != target['term_id'] or source['persona'] == target['persona']:
            raise ValueError('reattribution requires the same head and a different existing persona')
        return
    if mode in ('reclassify_new_head', 'consolidate_reviewed_existing'):
        if source['term_id'] == target['term_id']:
            raise ValueError('reviewed cross-head edits require a different head')
        if source['kind'] not in RELATION_KINDS:
            raise ValueError('unsupported reviewed donor kind')
        if mode == 'reclassify_new_head':
            if source['concept'] == target['concept']:
                raise ValueError('reclassification requires a different concept')
        else:
            if target['kind'] not in RELATION_KINDS:
                raise ValueError('unsupported reviewed recipient kind')
            if (source['word_key'], source['surface']) != (target['word_key'], target['surface']):
                raise ValueError('reviewed consolidation must preserve the stored key and literal')
            if source['concept'] == target['concept'] and source['kind'] == target['kind']:
                raise ValueError('reviewed consolidation requires a concept or kind change')
            validate_source_row(connection, target)
    elif source['term_id'] == target['term_id'] or source['concept'] != target['concept']:
        raise ValueError('cross-head edits require a different head in the same concept')
    if source['word_key'] == word_key(target['head']):
        raise ValueError('exact transfer must not create a self relation')
    actual = connection.execute('SELECT * FROM relations WHERE term_id=? AND word_key=?',
                                (target['term_id'], source['word_key'])).fetchone()
    if mode in ('consolidate_existing', 'consolidate_reviewed_existing'):
        if actual != recipient_tuple(entry):
            raise ValueError('stale consolidation recipient')
    elif actual is not None or connection.execute('SELECT 1 FROM relations WHERE term_id=? AND rank=?',
                                                   (target['term_id'], entry['rank'])).fetchone():
        raise ValueError('occupied exact transfer recipient key/rank')


def validate_detachment(connection, entry):
    validate_persona_identity(connection, entry['source'])
    key, persona = entry['word_key'], entry['source']['persona']
    if not connection.execute('SELECT 1 FROM input_senses WHERE word_key=? AND persona=?', (key, persona)).fetchone():
        raise ValueError('missing exact input detachment')
    if has_input_relations(connection) and connection.execute(
            'SELECT 1 FROM input_relations WHERE input_word_key=? AND source_persona=?', (key, persona)).fetchone():
        raise ValueError('exact input detachment has unreviewed input_relations')


def validate_exact_preconditions(connection, plan):
    validate_exact_shapes(plan)
    if not has_exact_edits(plan):
        return
    edits, detachments = plan.get('relation_edits', []), plan.get('input_detachments', [])
    donors, detached = set(), set()
    sources, targets, recipient_keys, inserted_ranks = set(), set(), set(), set()
    input_keys, input_ranks, required_bindings = set(), set(), set()
    for entry in edits:
        source = entry['source']
        validate_source_row(connection, source)
        identity = (source['term_id'], source['word_key'])
        if identity in donors:
            raise ValueError('duplicate exact donor edit')
        donors.add(identity); sources.add(source['persona'])
        if 'target' in entry:
            validate_recipient(connection, entry)
            target = entry['target']; targets.add(target['persona'])
            if entry['mode'] == INPUT_TRANSFER_MODE:
                binding = (target['input_word_key'], target['persona'])
                recipient, rank = (*binding, source['word_key']), (*binding, entry['rank'])
                if recipient in input_keys or rank in input_ranks:
                    raise ValueError('duplicate planned input recipient key/rank')
                input_keys.add(recipient); input_ranks.add(rank); required_bindings.add(binding)
                continue
            recipient = (target['term_id'], source['word_key'])
            if recipient in recipient_keys:
                raise ValueError('duplicate exact recipient key')
            recipient_keys.add(recipient)
            if entry['mode'] in INSERTION_MODES:
                rank = (target['term_id'], entry['rank'])
                if rank in inserted_ranks:
                    raise ValueError('duplicate planned recipient rank')
                inserted_ranks.add(rank)
    for entry in detachments:
        validate_detachment(connection, entry)
        identity = (entry['word_key'], entry['source']['persona'])
        if identity in detached:
            raise ValueError('duplicate exact input detachment')
        detached.add(identity)
    # No chained edits: a recipient cannot depend on another edited source. Same-head
    # reattribution necessarily retains its own PK, but cannot be another donor's PK.
    if targets & sources:
        raise ValueError('exact recipient persona is also an edited donor')
    for entry in edits:
        if 'target' in entry and entry['mode'] not in ('reattribute_same_head', INPUT_TRANSFER_MODE):
            if (entry['target']['term_id'], entry['source']['word_key']) in donors:
                raise ValueError('exact recipient row is also a donor')
    if required_bindings & detached:
        raise ValueError('input transfer recipient binding is scheduled for detachment')
    added_bindings = {(word_key(e['word']), e['source_persona']) for e in plan.get('input_bindings', [])}
    if added_bindings & detached:
        raise ValueError('conflicting add and detach of the same input binding')
    removed_personas = set()
    for removal in plan.get('remove_own', []):
        records = connection.execute('SELECT p.persona,p.term_id FROM personas p JOIN terms t USING(term_id) '
                                     'WHERE t.word_key=? AND p.concept=?', (word_key(removal['word']), removal['concept'])).fetchall()
        if not records:
            raise ValueError('missing own removal in exact plan')
        for persona, term in records:
            removed_personas.add(persona)
            for (key,) in connection.execute('SELECT word_key FROM relations WHERE persona=?', (persona,)):
                if (term, key) not in donors:
                    raise ValueError('own removal leaves an unreviewed relation')
            for (key,) in connection.execute('SELECT word_key FROM input_senses WHERE persona=?', (persona,)):
                if (key, persona) not in detached:
                    raise ValueError('own removal leaves an unreviewed input binding')
    if targets & removed_personas or {e['source_persona'] for e in plan.get('input_bindings', [])} & removed_personas:
        raise ValueError('exact recipient source is scheduled for removal')


def apply_relation_edits(connection, plan):
    counts = Counter()
    for entry in plan.get('relation_edits', []):
        source, mode = entry['source'], entry['mode']
        validate_source_row(connection, source)
        if 'target' in entry:
            validate_recipient(connection, entry)
        if mode == 'reattribute_same_head':
            cursor = connection.execute(f'UPDATE relations SET persona=? WHERE {RELATION_WHERE}',
                                        (entry['target']['persona'], *relation_tuple(source)))
        else:
            if mode in INSERTION_MODES:
                connection.execute('INSERT INTO relations (term_id,rank,word_key,surface,kind,persona) VALUES (?,?,?,?,?,?)',
                                   recipient_tuple(entry))
            elif mode == INPUT_TRANSFER_MODE:
                inserted = connection.execute(f'INSERT INTO input_relations ({",".join(INPUT_COLUMNS)}) VALUES (?,?,?,?,?,?)',
                                              input_recipient_tuple(entry))
                if inserted.rowcount != 1:
                    raise ValueError('input transfer recipient was not inserted')
            cursor = connection.execute(f'DELETE FROM relations WHERE {RELATION_WHERE}', relation_tuple(source))
        if cursor.rowcount != 1:
            raise ValueError('exact donor changed during transaction')
        counts['relation_edits_' + mode] += 1
    return counts


def apply_input_detachments(connection, plan):
    counts = Counter()
    for entry in plan.get('input_detachments', []):
        validate_detachment(connection, entry)
        cursor = connection.execute('DELETE FROM input_senses WHERE word_key=? AND persona=?',
                                    (entry['word_key'], entry['source']['persona']))
        if cursor.rowcount != 1:
            raise ValueError('exact input changed during transaction')
        counts['input_bindings_detached'] += 1
    return counts
