"""Verbatim answer units and reversible source-root anonymization."""
from __future__ import annotations

import re
from pathlib import Path

from .core import digest, read_json, require


def units(text):
    result=[];offset=0
    for line in text.splitlines(keepends=True):
        content=line.rstrip('\r\n')
        if content.strip():
            result.append({'id':f'u{len(result)+1:03d}','text':content,'start':offset,'end':offset+len(content)})
        offset+=len(line)
    return result



def mask_answer(run, source: Path, experiment: Path, *, seed: int):
    """Unique neutral prefixes keep global quote inversion unambiguous."""
    raw = run['answer']
    ident = digest({'seed': seed, 'run': run['id'], 'answer': raw})[:24]
    prefixes = {str(Path(run['source_snapshot']))} if run.get('source_snapshot') else set()
    prefixes.add(str(source))
    folder = experiment / 'runs' / run['id']
    if (folder / 'copy.json').exists():
        prefixes.add(read_json(folder / 'copy.json')['source'])
    # Source roots precede workspace/experiment roots so source-relative suffixes survive.
    replacements = []
    for number, prefix in enumerate(sorted(prefixes, key=lambda x: (-len(x), x)), 1):
        if prefix in raw:
            replacements.append([prefix, f'/frozen-grafana/source-{number:03d}'])
    other_prefixes = [str(folder), str(experiment)]
    for number, prefix in enumerate(sorted(set(other_prefixes), key=lambda x: (-len(x), x)), 1):
        if prefix in raw:
            replacements.append([prefix, f'/anonymous-workspace/location-{number:03d}'])
    replacements.append([run['id'], ident])
    # Replace nonoverlapping raw spans once; retain exact offsets for quote inversion.
    candidates = [(original, anonymous) for original, anonymous in replacements if original in raw]
    for _, anonymous in candidates:
        require(anonymous not in raw, 'Neutral marker already occurs in raw answer')
    translation = dict(candidates)
    alternatives = []
    for original in sorted(translation, key=lambda x: (-len(x), x)):
        boundary = r"(?=$|/|[\s<>\"'`\)\]\},;:!?\#]|\.(?=\s|$))" if original.startswith('/') else ''
        alternatives.append(re.escape(original) + boundary)
    pattern = re.compile('|'.join(alternatives) or r'(?!x)x')
    parts, positions, used = [], [], []
    raw_cursor = masked_cursor = 0
    for match in pattern.finditer(raw):
        before = raw[raw_cursor:match.start()]
        parts.append(before); masked_cursor += len(before)
        anonymous = translation[match.group()]
        positions.append({'raw_start': match.start(), 'raw_end': match.end(),
                          'masked_start': masked_cursor, 'masked_end': masked_cursor + len(anonymous),
                          'original': match.group(), 'anonymous': anonymous})
        parts.append(anonymous); masked_cursor += len(anonymous); raw_cursor = match.end()
        pair = [match.group(), anonymous]
        if pair not in used:
            used.append(pair)
    parts.append(raw[raw_cursor:]); masked = ''.join(parts)
    restored = masked
    for original, anonymous in reversed(used):
        restored = restored.replace(anonymous, original)
    require(restored == raw, 'Answer masking is not reversible')
    flags = []
    for name, pattern in [
        ('token_self_report', r'(?i)(?:\d[\d,]*\s*(?:tokens?|토큰)|(?:tokens?\s*(?:used|usage)|토큰\s*(?:사용|소비|비용))\s*[:=]?\s*\d)'),
        ('unmasked_absolute_workspace', r'/(?:Users|home|tmp|private/var|var/folders)/[^\s<>`"\']+'),
    ]:
        matches = list(re.finditer(pattern, masked))
        if matches:
            flags.append({'kind': name, 'spans': [[m.start(), m.end()] for m in matches],
                          'policy': 'warning only; root must classify the exact raw span before public packet sealing'})
    return ident, masked, used, positions, flags
