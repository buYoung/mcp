"""코드베이스에서 정의된 이름(변수·함수·상수·타입 등)을 tree-sitter로 모은다. 지금은 Go만 지원한다.

사용법(tree-sitter-language-pack이 있는 Python): python collect_words.py <저장소 경로> <출력 디렉터리>
출력:
  definitions.jsonl  정의 위치 하나당 한 줄(name, kind, role, scope, owner, file, line, column, is_test, is_generated, parts)
  words.json         이름별 집계(kinds, definition_count, occurrence_count, file_count, is_test_only, parts)
  summary.json       종류별 개수, 파일 수, 수집 조건
"""
import collections
import importlib.metadata
import json
import re
import subprocess
import sys
from pathlib import Path

from tree_sitter_language_pack import get_parser

EXCLUDED_DIRECTORIES = {".git", "vendor", "node_modules"}
GENERATED_MARKER = re.compile(rb"^// Code generated .* DO NOT EDIT\.$", re.MULTILINE)
# 사전 조회(syntax_parts.subparts)와 같은 규칙으로 쪼갠다.
SUBPART = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[0-9]+")
NAME_LEAVES = {"identifier", "type_identifier", "field_identifier", "package_identifier", "label_name"}
SCOPE_NODES = {"block", "func_literal"}
TYPE_KIND = {"struct_type": "struct", "interface_type": "interface"}
SHORT_DECLARATION_PARENTS = {"range_clause", "receive_statement"}


def parts_of(name):
    return [p.lower() for piece in re.split(r"\.|-|::|_", name) for p in SUBPART.findall(piece)]


def text_of(node, source):
    return source[node.start_byte:node.end_byte].decode()


def receiver_type(receiver, source):
    """`(c *Context)` -> Context, `(s *S[T, U])` -> S."""
    for node in walk(receiver):
        if node.type == "type_identifier":
            return text_of(node, source)
    return None


def walk(node):
    yield node
    for child in node.children:
        yield from walk(child)


def has_short_declaration(node):
    return any(child.type == ":=" for child in node.children)


def collect_go(tree, source):
    """정의 위치마다 (node, kind, role, owner, is_local)를 낸다."""
    found = []

    def add(name_node, kind, owner, is_local, role=None):
        # 쉼표처럼 같은 필드에 묶인 익명 노드와 빈 식별자 `_`는 이름이 아니다.
        if name_node is not None and name_node.is_named and text_of(name_node, source) != "_":
            found.append((name_node, kind, role, owner, is_local))

    def visit(node, owner, is_local):
        kind = node.type
        if kind == "package_clause":
            add(node.named_children[0], "package", None, False)
        elif kind == "import_spec":
            alias = node.child_by_field_name("name")
            if alias is not None and alias.type == "package_identifier":
                add(alias, "import_alias", owner, is_local)
        elif kind == "function_declaration":
            name = node.child_by_field_name("name")
            add(name, "function", None, False)
            owner = text_of(name, source)
        elif kind == "method_declaration":
            name = node.child_by_field_name("name")
            receiver = node.child_by_field_name("receiver")
            add(name, "method", receiver_type(receiver, source), False)
            owner = f"{receiver_type(receiver, source)}.{text_of(name, source)}"
            # `func (s *S[T, U])`의 T, U는 메서드 안에서 새로 선언되는 타입 매개변수다.
            for generic in (n for n in walk(receiver) if n.type == "type_arguments"):
                for argument in generic.named_children:
                    add(argument.named_children[0] if argument.named_children else argument,
                        "type_parameter", owner, False)
        elif kind == "type_spec":
            name = node.child_by_field_name("name")
            add(name, TYPE_KIND.get(node.child_by_field_name("type").type, "type"), owner, is_local)
            owner = text_of(name, source)
        elif kind == "type_alias":
            name = node.child_by_field_name("name")
            add(name, "type_alias", owner, is_local)
            owner = text_of(name, source)
        elif kind == "type_parameter_declaration":
            for name in node.children_by_field_name("name"):
                add(name, "type_parameter", owner, is_local)
        elif kind in ("parameter_declaration", "variadic_parameter_declaration"):
            parameter_list, holder = node.parent, node.parent.parent
            role = next((field for field in ("receiver", "result")
                         if holder is not None and holder.child_by_field_name(field) == parameter_list), "parameter")
            for name in node.children_by_field_name("name"):
                add(name, "parameter", owner, is_local, role)
        elif kind == "field_declaration":
            for name in node.children_by_field_name("name"):
                add(name, "field", owner, is_local)
        elif kind == "method_elem":
            name = node.child_by_field_name("name")
            add(name, "interface_method", owner, is_local)
        elif kind == "const_spec":
            for name in node.children_by_field_name("name"):
                add(name, "constant", owner, is_local)
        elif kind == "var_spec":
            for name in node.children_by_field_name("name"):
                add(name, "variable", owner, is_local)
        elif kind == "short_var_declaration" or (kind in SHORT_DECLARATION_PARENTS and has_short_declaration(node)):
            left = node.child_by_field_name("left")
            for name in (left.named_children if left is not None else []):
                if name.type == "identifier":
                    add(name, "variable", owner, True)
        elif kind == "type_switch_statement":
            alias = node.child_by_field_name("alias")
            for name in (alias.named_children if alias is not None else []):
                add(name, "variable", owner, True)
        elif kind == "labeled_statement":
            add(node.child_by_field_name("label"), "label", owner, True)
        for child in node.children:
            visit(child, owner, is_local or child.type in SCOPE_NODES)

    visit(tree.root_node, None, False)
    return found


def main(repository, output_directory):
    repository, output_directory = Path(repository).resolve(), Path(output_directory)
    output_directory.mkdir(parents=True, exist_ok=True)
    parser = get_parser("go")
    files = sorted(p for p in repository.rglob("*.go")
                   if not EXCLUDED_DIRECTORIES & set(p.relative_to(repository).parts[:-1]))
    definitions, occurrences, occurrence_files = [], collections.Counter(), collections.defaultdict(set)
    for path in files:
        source = path.read_bytes()
        relative = str(path.relative_to(repository))
        is_test = path.name.endswith("_test.go") or "testdata" in path.relative_to(repository).parts
        is_generated = bool(GENERATED_MARKER.search(source))
        tree = parser.parse(source)
        if tree.root_node.has_error:
            print(f"구문 오류 포함: {relative}", file=sys.stderr)
        for node in walk(tree.root_node):
            if node.type in NAME_LEAVES:
                name = text_of(node, source)
                occurrences[name] += 1
                occurrence_files[name].add(relative)
        for name_node, kind, role, owner, is_local in collect_go(tree, source):
            name = text_of(name_node, source)
            definitions.append({
                "name": name, "kind": kind, "role": role, "scope": "local" if is_local else "package",
                "owner": owner, "file": relative, "line": name_node.start_point[0] + 1,
                "column": name_node.start_point[1] + 1, "is_test": is_test, "is_generated": is_generated,
                "parts": parts_of(name),
            })

    words = {}
    for record in definitions:
        word = words.setdefault(record["name"], {
            "name": record["name"], "parts": record["parts"], "kinds": collections.Counter(),
            "definition_count": 0, "definition_files": set(), "is_test_only": True,
        })
        word["kinds"][record["kind"]] += 1
        word["definition_count"] += 1
        word["definition_files"].add(record["file"])
        word["is_test_only"] &= record["is_test"]
    word_list = sorted(({
        "name": w["name"], "parts": w["parts"], "kinds": dict(w["kinds"]),
        "definition_count": w["definition_count"], "definition_file_count": len(w["definition_files"]),
        "occurrence_count": occurrences[w["name"]], "occurrence_file_count": len(occurrence_files[w["name"]]),
        "is_test_only": w["is_test_only"],
    } for w in words.values()), key=lambda w: (-w["occurrence_count"], w["name"]))

    with open(output_directory / "definitions.jsonl", "w") as handle:
        for record in definitions:
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")
    (output_directory / "words.json").write_text(json.dumps(word_list, ensure_ascii=False, indent=1))
    commit = subprocess.run(["git", "-C", str(repository), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    summary = {
        "repository": str(repository), "commit": commit, "language": "go",
        "tree_sitter": importlib.metadata.version("tree-sitter"),
        "tree_sitter_language_pack": importlib.metadata.version("tree-sitter-language-pack"),
        "file_count": len(files), "test_file_count": sum(1 for p in files if p.name.endswith("_test.go")),
        "excluded_directories": sorted(EXCLUDED_DIRECTORIES),
        "definition_count": len(definitions), "unique_name_count": len(word_list),
        "unique_part_count": len({p for w in word_list for p in w["parts"]}),
        "definitions_by_kind": dict(collections.Counter(r["kind"] for r in definitions).most_common()),
        "unique_names_by_kind": dict(collections.Counter(k for w in word_list for k in w["kinds"]).most_common()),
        "test_only_name_count": sum(w["is_test_only"] for w in word_list),
    }
    (output_directory / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))
    print(json.dumps(summary, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main(*sys.argv[1:3])
