"""판정한 단어가 주석에서 온 것이 아닌지 기계로 대조한다(판정 없음).

사용법(tree-sitter-language-pack이 있는 Python): python verify_comment_source.py
저장소마다
  1. 수집 때와 같은 파일(gin은 저장소 전체 Go 파일, 나머지는 sample-files.json)을 다시 파싱해
     이름 잎 노드가 주석 노드 안에 있는 경우를 센다.
  2. 판정 단어마다 개념 단위 판정 원본에서 출처 식별자를 모두 찾아,
     주석 밖 코드에서 이름 잎 노드로 나온 출처가 하나라도 있는지 본다.
  3. 출처가 Go struct 태그(문자열 값)뿐인 단어는 따로 적는다.
출력: comment-source-check.json, comment-source-check.md
"""
import collections
import json
import re
from pathlib import Path
from word_eval_paths import batches_dir, work_dir

from tree_sitter_language_pack import get_parser

import collect_repo
import languages

WORK = work_dir()
BATCHES = batches_dir()
REPOSITORIES = ["gin", "express", "flask", "keycloak", "tokio", "redis", "terminal", "vector", "bitwarden-server",
                "gutenberg", "woocommerce", "sentry"]
GIN_EXCLUDED_DIRECTORIES = {".git", "vendor", "node_modules"}  # collect_words.py와 같은 조건
UNIT_JUDGEMENT_ROOT = BATCHES / "units"


def normalize(text):
    return " ".join(text.lower().split())


def is_inside(node, type_check):
    parent = node.parent
    while parent is not None:
        if type_check(parent.type):
            return True
        parent = parent.parent
    return False


def collected_files(repository_name, repository):
    if repository_name == "gin":
        files = sorted(str(p.relative_to(repository)) for p in repository.rglob("*.go")
                       if not GIN_EXCLUDED_DIRECTORIES & set(p.relative_to(repository).parts[:-1]))
        return files, False
    sampled = json.loads((WORK / repository_name / "sample-files.json").read_text())
    candidates, _ = collect_repo.candidate_files(repository)
    is_cpp_repository = any(Path(f).suffix in collect_repo.CPP_EXTENSIONS for f, _ in candidates)
    return [f["file"] for f in sampled["files"]], is_cpp_repository


def scan_names(repository, files, is_cpp_repository):
    """이름별로 주석 밖 코드 등장 수, 주석 안 등장 수, 구문 오류 노드 밖 등장 수를 센다."""
    parsers = {}
    in_code, in_comment, outside_error = collections.Counter(), collections.Counter(), collections.Counter()
    comment_examples = collections.defaultdict(list)
    for relative in files:
        path = repository / relative
        language = collect_repo.language_of(path, is_cpp_repository)
        parser = parsers.setdefault(language, get_parser(language))
        source = path.read_bytes()
        if language in ("c", "cpp"):
            source = languages.blank_sal_annotations(source)
        tree = parser.parse(source)
        for node in languages.walk(tree.root_node):
            if node.type not in languages.NAME_LEAVES:
                continue
            name = languages.clean_name(node.text.decode(errors="replace"))
            if not name or name == "_" or not re.fullmatch(r"[\w$]+", name):
                continue
            if is_inside(node, lambda t: "comment" in t):
                in_comment[name] += 1
                if len(comment_examples[name]) < 2:
                    comment_examples[name].append(f"{relative}:{node.start_point[0] + 1}")
                continue
            in_code[name] += 1
            if not is_inside(node, lambda t: t == "ERROR"):
                outside_error[name] += 1
    return in_code, in_comment, outside_error, comment_examples


def unit_sources(repository_name):
    directory = UNIT_JUDGEMENT_ROOT if repository_name == "gin" else UNIT_JUDGEMENT_ROOT / repository_name
    pattern = "u[0-9].json" if repository_name == "gin" else "u*.json"
    sources = collections.defaultdict(set)
    for path in sorted(directory.glob(pattern)):
        for record in json.loads(path.read_text()):
            for unit in record["units"]:
                sources[normalize(unit["text"])].add((record["name"], record.get("where")))
    return sources


def check(repository_name):
    directory = WORK / repository_name
    repository = Path(json.loads((directory / "summary.json").read_text())["repository"])
    judge_file = "judge-input-r2.json" if repository_name == "gin" else "judge-input.json"
    judged_words = [item["word"] for item in json.loads((directory / judge_file).read_text())]
    status_by_text = {row["text"]: row["dataset_status"] for row in json.loads((directory / "units.json").read_text())}
    files, is_cpp_repository = collected_files(repository_name, repository)
    in_code, in_comment, outside_error, comment_examples = scan_names(repository, files, is_cpp_repository)
    sources = unit_sources(repository_name)

    groups = collections.defaultdict(list)
    for word in judged_words:
        text = normalize(word)
        word_sources = sources.get(text, set())
        identifier_names = {name for name, where in word_sources if where in ("defined", "used_only")}
        status = status_by_text.get(text, "unknown")
        if not word_sources:
            group = "no_source"
        elif any(in_code[name] for name in identifier_names):
            group = "code_identifier" if any(outside_error[name] for name in identifier_names) else "parse_error_only"
        elif identifier_names:
            group = "identifier_not_found_in_code"
        else:
            group = "struct_tag_only"
        groups[group].append({"word": word, "dataset_status": status,
                              "sources": sorted(name for name, _ in word_sources)[:4],
                              "comment_hits": sum(in_comment[name] for name in identifier_names)})
    return {
        "repository": repository_name, "files": len(files), "judged_words": len(judged_words),
        "name_leaves_in_code": sum(in_code.values()), "name_leaves_in_comment": sum(in_comment.values()),
        "comment_examples": {name: examples for name, examples in list(comment_examples.items())[:10]},
        "groups": {group: {"count": len(rows),
                           "by_status": dict(collections.Counter(r["dataset_status"] for r in rows)),
                           "words": rows if group != "code_identifier" else []}
                   for group, rows in groups.items()},
    }


def main():
    results = [check(name) for name in REPOSITORIES]
    (WORK / "comment-source-check.json").write_text(json.dumps(results, ensure_ascii=False, indent=1))
    lines = ["# 판정 단어의 출처 대조: 주석에서 온 단어가 있는가 (판정 없음, 기계 대조)", "",
             "| 저장소 | 파일 | 판정 단어 | 주석 밖 코드 식별자에서 나옴 | 그중 데이터셋에 없음 | struct 태그에서만 나옴 | 구문 오류 노드에서만 나옴 | 출처 못 찾음 | 주석 안 이름 잎 노드 |",
             "|---|---|---|---|---|---|---|---|---|"]
    for result in results:
        groups = result["groups"]
        count = lambda group: groups.get(group, {}).get("count", 0)
        lines.append(f"| {result['repository']} | {result['files']} | {result['judged_words']} | {count('code_identifier')} | "
                     f"{groups.get('code_identifier', {}).get('by_status', {}).get('missing', 0)} | {count('struct_tag_only')} | "
                     f"{count('parse_error_only')} | {count('no_source') + count('identifier_not_found_in_code')} | "
                     f"{result['name_leaves_in_comment']} |")
    for result in results:
        for group in ("struct_tag_only", "parse_error_only", "no_source", "identifier_not_found_in_code"):
            rows = result["groups"].get(group, {}).get("words", [])
            if rows:
                lines += ["", f"## {result['repository']} — {group} ({len(rows)})"]
                lines += [f"- `{r['word']}` ({r['dataset_status']}) ← {', '.join(r['sources'])}" for r in rows]
    (WORK / "comment-source-check.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
