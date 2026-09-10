"""Promoted, source-grounded grading helpers; no historical experiment imports."""
import copy
import re
from pathlib import Path

from .v2 import grading
from .v2.core import SPEC, file_digest, read_json, digest, require


SEMANTICS='''
인용 범위 해석을 정확히 구분한다. Markdown 링크의 표시문에 파일·행 범위가 명시되어 있으면 그 명시된 범위를 평가한다.
예: [file.go:10-20](path/file.go:10)은 같은 파일 10~20행을 명시한 인용이다. 링크 목적지의 10행은 이동 위치이며 표시된 범위를 10행 하나로 축소하지 않는다.
파일은 링크 목적지로 식별하고 표시문의 파일명/접미 경로가 같은 파일인지 확인한다. 표시문에 범위가 없을 때는 목적지/본문에 명시된 행만 사용한다. 실제로 단일 행만 제시한 인용을 함수 전체로 자동 확대하지 않는다.
원문 단위 answer_units는 답안의 비어 있지 않은 원래 줄을 순서대로 보여 준다. id는 채점용 메타데이터다. 단위 밖 새 문장이나 정답 내용을 덧붙이지 않았다.
각 fact와 major_errors의 answer_unit_id에는 실제 해당 주장이 들어 있는 단위 id를 선택한다. 누락 사실은 none을 쓴다. correct=true 및 major_errors에는 비어 있지 않은 실제 단위를 선택한다.
원문 인용문은 프로그램이 선택된 단위에서 그대로 가져오므로 인용문을 직접 재작성하지 않는다. 한 fact의 근거는 답안의 모든 단위에 명시된 인용을 함께 평가한다.
source_supplement는 고정 원문이며 답안이 인용하지 않은 근거를 대신 채우지 않는다. citation_index는 링크 표시문에 이미 명시된 범위를 기계적으로 추출한 보조 자료일 뿐 추가 인용이 아니다.
표현상 선행 공백 생략과 trim 동작 주장을 구별한다. 익명 경로의 pkg/, packages/, public/ 이하가 원문 상대경로다. 외부 도구는 쓰지 않는다.
'''


def units(text):
    result=[];offset=0
    for line in text.splitlines(keepends=True):
        content=line.rstrip('\r\n')
        if content.strip():
            result.append({'id':f'u{len(result)+1:03d}','text':content,'start':offset,'end':offset+len(content)})
        offset+=len(line)
    return result


def label_ranges(answer):
    result=[]
    for match in re.finditer(r'\[([^\]\n]+)\]\(([^)]+)\)',answer):
        label,target=match.groups()
        path=re.search(r'((?:pkg|packages|public)/[^\s<>:#?]+)',target)
        span=re.search(r'(?::L?|#L|\bL)(\d+)\s*[-–—]\s*L?(\d+)',label)
        if not span: span=re.fullmatch(r'\s*L?(\d+)\s*[-–—]\s*L?(\d+)\s*',label.strip('` '))
        if path and span:
            begin,end=map(int,span.groups())
            hinted=re.search(r'([\w./-]+\.[A-Za-z]+):L?\d+',label)
            if hinted and not (path[1] == hinted[1] or path[1].endswith('/' + hinted[1])): continue
            if 1<=begin<=end:
                result.append({'path':path[1],'start_line':begin,'end_line':end,'basis':'explicit_link_label_range'})
    return result


def render_control(answer):
    # Preserve every original word and range; change only citation presentation.
    return re.sub(r'((?:pkg|packages|public)/[^\s;()`:]+):(\d+)-(\d+)',
        lambda m:f'[{m[1]}:{m[2]}-{m[3]}]({m[1]}:{m[2]})',answer)


def unit_schema(maximum):
    schema=grading.judgment_schema()
    options=['none']+[f'u{i:03d}' for i in range(1,maximum+1)]
    def visit(value):
        if isinstance(value,dict):
            props=value.get('properties')
            if isinstance(props,dict) and 'answer_quote' in props:
                props.pop('answer_quote');props['answer_unit_id']={'type':'string','enum':options}
                value['required']=['answer_unit_id' if x=='answer_quote' else x for x in value['required']]
            for child in value.values():visit(child)
        elif isinstance(value,list):
            for child in value:visit(child)
    visit(schema)
    return schema


def decode(raw,registry):
    decoded=copy.deepcopy(raw)
    for judgment in decoded['judgments']:
        record=registry[judgment['answer_id']]
        lookup={u['id']:u['text'] for u in record['units']}|{'none':''}
        for item in judgment['facts']+judgment['major_errors']:
            key=item.pop('answer_unit_id')
            require(key in lookup,'Unit belongs to a different answer')
            item['answer_quote']=lookup[key]
    return decoded


def source_supplement(question, answers, source, *, full_files=True):
    mandatory = {e["path"] for e in question["evidence"]}
    paths = set(mandatory)
    for answer in answers:
        for match in re.finditer(r"(?:pkg|packages|public)/[^\s`<>\[\]()\"',;:#]+", answer["answer"]):
            candidate = match.group().rstrip(".")
            p = (source / candidate).resolve()
            if p.is_relative_to(source.resolve()) and p.is_file():
                paths.add(candidate)
    result = []
    for relative in sorted(paths):
        path = source / relative
        text = path.read_text()
        source_lines = text.splitlines()
        indices = set(range(len(source_lines))) if relative in mandatory and full_files else set()
        if relative in mandatory and not full_files:
            for evidence in question["evidence"]:
                if evidence["path"] == relative:
                    indices.update(range(max(0, evidence["start_line"] - 7), min(len(source_lines), evidence["end_line"] + 6)))
        cited_ranges = []
        if relative not in mandatory or not full_files:
            pattern = re.compile(re.escape(relative) + r"(?::(?:L)?|#L)(\d+)(?:[-–—](?:L)?(\d+))?")
            for answer in answers:
                for match in pattern.finditer(answer["answer"]):
                    start, end = int(match[1]), int(match[2] or match[1])
                    cited_ranges.append([start, end])
                    if 1 <= start <= end and start <= len(source_lines):
                        indices.update(range(max(0, start - 7), min(len(source_lines), end + 6)))
            if not indices:
                indices.update(range(min(80, len(source_lines))))
        result.append({"path": relative, "sha256": file_digest(path),
                       "full_file": len(indices) == len(source_lines), "explicit_cited_ranges": cited_ranges,
                       "source_line_count": len(source_lines),
                       "context_does_not_expand_answer_citation": True,
                       "numbered_source": "\n".join(f"{i+1}: {source_lines[i]}" for i in sorted(indices))})
    return result


def mask_answer(run, source: Path, experiment: Path):
    """Unique neutral prefixes keep global quote inversion unambiguous."""
    raw = run['answer']
    ident = digest({'seed': SPEC['seed'], 'run': run['id'], 'answer': raw})[:24]
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
        ('version_identifier', r'(?<![\w-])(?:B-[146]|R[123]-C[1-5]|candidate-\d{3}-)(?![\w-])'),
        ('version_self_report', r'(?i)(?:version|variant|group|버전|집단)\s*[:=]?\s*[`"\']?(?:A|B)(?:\b|[`"\'])'),
        ('token_self_report', r'(?i)(?:\d[\d,]*\s*(?:tokens?|토큰)|(?:tokens?\s*(?:used|usage)|토큰\s*(?:사용|소비|비용))\s*[:=]?\s*\d)'),
        ('unmasked_absolute_workspace', r'/(?:Users|home|tmp|private/var|var/folders)/[^\s<>`"\']+'),
    ]:
        matches = list(re.finditer(pattern, masked))
        if matches:
            flags.append({'kind': name, 'spans': [[m.start(), m.end()] for m in matches],
                          'policy': 'warning only; root must classify the exact raw span before public packet sealing'})
    return ident, masked, used, positions, flags
