"""태그 표기(속성·애너테이션·데코레이터) 안의 값을 개념 단위 판정 입력으로 모은다(Go struct 태그와 같은 기준).

사용법(tree-sitter-language-pack이 있는 Python): python collect_tag_values.py
기준(사용자 확정, 2026-10-11)
  - 값만 넣는다. 공백이 들어간 문자열(문장·문구·질의문·표시 이름)은 넣지 않는다.
  - 테스트 파일의 값은 넣지 않는다(collect_repo.TEST_PATH).
  - 주석 안 문서 태그(@param, JSDoc 등)는 보지 않는다.
  - 값은 Go struct 태그와 같은 규칙([,=| ])으로 나누고 빈 값과 `-`는 뺀다. 표기 이름·키는 식별자로 이미 모았으므로 넣지 않는다.
출력(gin 제외 11개 저장소): <저장소>/tag-input.json(unit-input.json과 같은 형식, where=tag_value), <저장소>/tag-input-summary.json
gin: 기존 struct_tag 항목 가운데 테스트 파일에만 나오는 것을 gin/tag-test-only.json에 적는다(기존 파일은 그대로 둔다).
"""
import collections
import json
import re
from pathlib import Path
from word_eval_paths import work_dir

from tree_sitter_language_pack import get_parser

import collect_repo
import languages
from survey_tag_strings import (REPOSITORIES, STRING_NODES, TAG_NODES, enclosing, is_inside_comment, item_name, key_of,
                                string_value)

WORK = work_dir()
GO_TAG_SPLIT = re.compile(r"[,=| ]")
GIN_EXCLUDED_DIRECTORIES = {".git", "vendor", "node_modules"}


def collect(repository_name):
    directory = WORK / repository_name
    repository = Path(json.loads((directory / "summary.json").read_text())["repository"])
    files = [f["file"] for f in json.loads((directory / "sample-files.json").read_text())["files"]]
    candidates, _ = collect_repo.candidate_files(repository)
    is_cpp_repository = any(Path(f).suffix in collect_repo.CPP_EXTENSIONS for f, _ in candidates)

    skipped = collections.Counter()
    counts, examples, qualifiers = collections.Counter(), collections.defaultdict(list), collections.defaultdict(collections.Counter)
    parsers = {}
    for relative in files:
        path = repository / relative
        language = collect_repo.language_of(path, is_cpp_repository)
        if language not in TAG_NODES:
            continue
        containers, items = TAG_NODES[language]
        is_test = bool(collect_repo.TEST_PATH.search(relative))
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
            if is_test:
                skipped["test_file"] += 1
                continue
            if not value.strip():
                skipped["empty"] += 1
                continue
            if re.search(r"\s", value.strip()):
                skipped["has_whitespace"] += 1
                continue
            key = key_of(node)
            qualifier = item_name(enclosing(node, items) or container) + (f"({key})" if key else "")
            line = node.start_point[0] + 1
            for token in GO_TAG_SPLIT.split(value.strip()):
                if not token or token == "-":
                    continue
                counts[token] += 1
                qualifiers[token][qualifier[:60]] += 1
                if len(examples[token]) < 2 and all(not e.startswith(relative + ":") for e in examples[token]):
                    examples[token].append(f"{relative}:{line}: {lines[line - 1].strip()[:150]}")

    items = sorted(({"name": token, "where": "tag_value", "kinds": None,
                     "qualifiers": [q for q, _ in qualifiers[token].most_common(3)],
                     "count": count, "examples": examples[token]} for token, count in counts.items()),
                   key=lambda x: (x["name"].lower(), x["where"]))
    (directory / "tag-input.json").write_text(json.dumps(items, ensure_ascii=False, indent=1))
    known_names = {item["name"] for item in json.loads((directory / "unit-input.json").read_text())}
    summary = {"repository": repository_name, "items": len(items), "occurrences": sum(counts.values()),
               "items_already_in_unit_input": sum(1 for item in items if item["name"] in known_names),
               "skipped_strings": dict(skipped)}
    (directory / "tag-input-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))
    return summary


def gin_test_only_tags():
    """gin struct 태그 키·값(collect_repo와 같은 규칙) 가운데 테스트 파일에만 나오는 것."""
    directory = WORK / "gin"
    repository = Path(json.loads((directory / "summary.json").read_text())["repository"])
    parser = get_parser("go")
    files_by_word = collections.defaultdict(set)
    for path in sorted(repository.rglob("*.go")):
        relative = path.relative_to(repository)
        if GIN_EXCLUDED_DIRECTORIES & set(relative.parts[:-1]):
            continue
        tree = parser.parse(path.read_bytes())
        for node in languages.walk(tree.root_node):
            if node.type == "field_declaration" and node.child_by_field_name("tag"):
                tag = node.child_by_field_name("tag").text.decode(errors="replace")
                for match in re.finditer(r'(\w+):"([^"]*)"', tag):
                    for word in [match.group(1)] + [v for v in re.split(r"[,=| ]", match.group(2)) if v and v != "-"]:
                        files_by_word[word].add(str(relative))
    struct_tag_names = {item["name"] for item in json.loads((directory / "unit-input.json").read_text())
                        if item["where"] == "struct_tag"}
    test_only = sorted(word for word in struct_tag_names
                       if files_by_word.get(word) and all(collect_repo.TEST_PATH.search(f) for f in files_by_word[word]))
    unmatched = sorted(struct_tag_names - set(files_by_word))
    result = {"struct_tag_items": len(struct_tag_names), "test_only": test_only, "unmatched": unmatched}
    (directory / "tag-test-only.json").write_text(json.dumps(result, ensure_ascii=False, indent=1))
    return {"struct_tag_items": len(struct_tag_names), "test_only": len(test_only), "unmatched": len(unmatched)}


def main():
    for name in REPOSITORIES:
        print(json.dumps(collect(name), ensure_ascii=False))
    print("gin", json.dumps(gin_test_only_tags(), ensure_ascii=False))


if __name__ == "__main__":
    main()
