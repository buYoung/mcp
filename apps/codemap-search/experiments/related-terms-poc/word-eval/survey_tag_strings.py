"""Go struct 태그에 해당하는 다른 언어 표기(속성·애너테이션·데코레이터) 안의 문자열 값을 조사한다(판정 없음, 수집 전 조사).

사용법(tree-sitter-language-pack이 있는 Python): python survey_tag_strings.py
gin을 뺀 11개 저장소의 표본 파일(sample-files.json)을 다시 파싱해, 아래 표기 안에 있는 문자열 리터럴을 모은다.
  rust: #[...] #![...]   java: @Annotation(...)   csharp: [Attribute(...)]
  python·javascript·typescript·tsx: @decorator(...)   php: #[Attribute(...)]   c·cpp: [[...]] __attribute__ __declspec
주석 안의 문서 태그(@param, JSDoc, XML 문서 주석 등)는 주석이므로 보지 않는다.
값 모양은 기계 분류(identifier, list, path, query, sentence, other)이고, 새 부분어는 Go 태그와 같은 규칙([,=| ])으로 나눈 뒤
collect_words.parts_of로 쪼개, 그 저장소 식별자에서 나온 부분어에 없는 것을 센다.
출력: tag-string-survey.json, tag-string-survey.md
"""
import collections
import json
import re
from pathlib import Path

from tree_sitter_language_pack import get_parser

import collect_repo
import collect_words
import languages
from word_eval_paths import work_dir
REPOSITORIES = ["express", "flask", "keycloak", "tokio", "redis", "terminal", "vector", "bitwarden-server",
                "gutenberg", "woocommerce", "sentry"]
# 언어별 표기 노드: 바깥 묶음(container)과 이름을 읽을 안쪽 노드(item)
TAG_NODES = {
    "rust": ({"attribute_item", "inner_attribute_item"}, {"attribute"}),
    "java": ({"annotation"}, {"annotation"}),
    "csharp": ({"attribute_list"}, {"attribute"}),
    "python": ({"decorator"}, {"decorator"}),
    "javascript": ({"decorator"}, {"decorator"}),
    "typescript": ({"decorator"}, {"decorator"}),
    "tsx": ({"decorator"}, {"decorator"}),
    "php": ({"attribute_list"}, {"attribute"}),
    "c": ({"attribute_declaration", "attribute_specifier", "ms_declspec_modifier"}, {"attribute", "attribute_specifier", "ms_declspec_modifier"}),
    "cpp": ({"attribute_declaration", "attribute_specifier", "ms_declspec_modifier"}, {"attribute", "attribute_specifier", "ms_declspec_modifier"}),
}
STRING_NODES = {"string_literal", "raw_string_literal", "string", "encapsed_string", "template_string",
                "verbatim_string_literal", "text_block", "concatenated_string", "interpolated_string_expression"}
QUOTE_PREFIX = re.compile(r'^[A-Za-z@$]*#*("""|\'\'\'|"|\'|`)')
QUOTE_SUFFIX = re.compile(r'("""|\'\'\'|"#*|\'|`)$')
GO_TAG_SPLIT = re.compile(r"[,=| ]")
QUERY_START = re.compile(r"^\s*(select|update|delete|insert|from)\b", re.IGNORECASE)


def string_value(node):
    text = node.text.decode(errors="replace")
    if not QUOTE_PREFIX.match(text):
        return None
    return QUOTE_SUFFIX.sub("", QUOTE_PREFIX.sub("", text, count=1), count=1)


def shape_of(value):
    words = re.findall(r"[A-Za-z]+", value)
    if not value.strip():
        return "empty"
    if QUERY_START.match(value):
        return "query"
    if re.search(r"\s", value.strip()) and (len(words) >= 3 or re.search(r"[.!?]$", value.strip())):
        return "sentence"
    if "/" in value or "\\" in value:
        return "path"
    if re.fullmatch(r"[\w.:\-$@*{}\[\]<>]+", value):
        return "identifier"
    if re.fullmatch(r"[\w.:\-$@*{}\[\]<>]+([,|; =]+[\w.:\-$@*{}\[\]<>]+)+", value.strip()):
        return "list"
    return "other"


def key_of(node):
    """`rename = "x"`, `name = "x"`, `ErrorMessage = "x"`, `selector: "x"`의 앞 이름."""
    previous = node.prev_sibling
    if previous is not None and previous.type in ("=", ":") and previous.prev_sibling is not None:
        return previous.prev_sibling.text.decode(errors="replace")[:40]
    parent = node.parent
    if parent is not None and parent.type in ("element_value_pair", "keyword_argument", "pair", "attribute_argument", "argument") \
            and parent.named_children and parent.named_children[0] != node:
        return parent.named_children[0].text.decode(errors="replace").rstrip(":= ")[:40]
    return None


def item_name(node):
    match = re.match(r"[#@!\[\]\s]*([\w:.\\]+)", node.text.decode(errors="replace"))
    return match.group(1)[:40] if match else "?"


def enclosing(node, types):
    parent = node.parent
    while parent is not None:
        if parent.type in types:
            return parent
        parent = parent.parent
    return None


def is_inside_comment(node):
    return enclosing(node, {t for t in ("comment", "line_comment", "block_comment", "doc_comment")}) is not None


def survey(repository_name):
    directory = work_dir() / repository_name
    repository = Path(json.loads((directory / "summary.json").read_text())["repository"])
    files = [f["file"] for f in json.loads((directory / "sample-files.json").read_text())["files"]]
    candidates, _ = collect_repo.candidate_files(repository)
    is_cpp_repository = any(Path(f).suffix in collect_repo.CPP_EXTENSIONS for f, _ in candidates)
    known_parts = {part for item in json.loads((directory / "unit-input.json").read_text())
                   for part in collect_words.parts_of(item["name"])}

    records = []
    parsers = {}
    for relative in files:
        path = repository / relative
        language = collect_repo.language_of(path, is_cpp_repository)
        if language not in TAG_NODES:
            continue
        containers, items = TAG_NODES[language]
        parser = parsers.setdefault(language, get_parser(language))
        source = path.read_bytes()
        if language in ("c", "cpp"):
            source = languages.blank_sal_annotations(source)
        tree = parser.parse(source)
        lines = source.decode(errors="replace").splitlines()
        for node in languages.walk(tree.root_node):
            if node.type not in STRING_NODES or not node.is_named or (node.parent and node.parent.type in STRING_NODES):
                continue
            container = enclosing(node, containers)
            if container is None or is_inside_comment(node):
                continue
            value = string_value(node)
            if value is None:
                continue
            item = enclosing(node, items) or container
            line = node.start_point[0] + 1
            tokens = [t for t in GO_TAG_SPLIT.split(value) if t]
            parts = sorted({p for t in tokens for p in collect_words.parts_of(t)})
            records.append({
                "language": language, "tag": item_name(item), "key": key_of(node), "value": value[:200],
                "shape": shape_of(value), "parts": parts, "new_parts": [p for p in parts if p not in known_parts],
                "where": f"{relative}:{line}: {lines[line - 1].strip()[:150]}",
            })

    def new_parts(rows):
        return sorted({p for r in rows for p in r["new_parts"]})

    by_shape = collections.defaultdict(list)
    for record in records:
        by_shape[record["shape"]].append(record)
    return {
        "repository": repository_name, "string_count": len(records),
        "distinct_values": len({r["value"] for r in records}),
        "by_language": dict(collections.Counter(r["language"] for r in records)),
        "top_tags": collections.Counter(r["tag"] for r in records).most_common(12),
        "top_keys": collections.Counter(r["key"] for r in records if r["key"]).most_common(12),
        "by_shape": {shape: {"count": len(rows), "distinct_values": len({r["value"] for r in rows}),
                             "new_parts": new_parts(rows),
                             "examples": [r["where"] for r in rows[:: max(1, len(rows) // 5)][:5]]}
                     for shape, rows in sorted(by_shape.items(), key=lambda item: -len(item[1]))},
        "new_parts_without_sentence": new_parts([r for r in records if r["shape"] != "sentence"]),
        "records": records,
    }


def main():
    results = [survey(name) for name in REPOSITORIES]
    (work_dir() / "tag-string-survey.json").write_text(json.dumps(results, ensure_ascii=False, indent=1))
    shapes = ("identifier", "list", "path", "query", "sentence", "other", "empty")
    lines = ["# 다른 언어의 태그 상당 표기 안 문자열 값 조사 (판정 없음, 수집 전 조사)", "",
             "| 저장소 | 언어별 문자열 수 | 문자열 | 서로 다른 값 | " + " | ".join(shapes) + " | 새 부분어(문장 제외) | 새 부분어(문장) |",
             "|---|---|---|---|" + "---|" * len(shapes) + "---|---|"]
    for result in results:
        shape_counts = [str(result["by_shape"].get(s, {}).get("count", 0)) for s in shapes]
        sentence_new = len(result["by_shape"].get("sentence", {}).get("new_parts", []))
        lines.append(f"| {result['repository']} | {', '.join(f'{k} {v}' for k, v in result['by_language'].items()) or '-'} | "
                     f"{result['string_count']} | {result['distinct_values']} | " + " | ".join(shape_counts)
                     + f" | {len(result['new_parts_without_sentence'])} | {sentence_new} |")
    for result in results:
        if not result["string_count"]:
            continue
        lines += ["", f"## {result['repository']}",
                  "- 표기 이름 상위: " + ", ".join(f"`{t}` {n}" for t, n in result["top_tags"]),
                  "- 키 상위: " + (", ".join(f"`{k}` {n}" for k, n in result["top_keys"]) or "-"),
                  "- 새 부분어(문장 제외) 일부: " + ", ".join(f"`{p}`" for p in result["new_parts_without_sentence"][:40])]
        for shape, entry in result["by_shape"].items():
            lines.append(f"- {shape} {entry['count']}개(서로 다른 값 {entry['distinct_values']}):")
            lines += [f"  - {example}" for example in entry["examples"]]
    (work_dir() / "tag-string-survey.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines[:4 + len(results)]))


if __name__ == "__main__":
    main()
