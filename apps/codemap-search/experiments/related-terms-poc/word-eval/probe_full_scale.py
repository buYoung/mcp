"""전체 코드로 넓힐 때의 규모를 미리 잰다(사전 측정, 평가셋 아님. 판정 없음, 수만 센다).

사용법(tree-sitter-language-pack이 있는 Python): python probe_full_scale.py
저장소마다 collect_repo.py와 같은 후보 규칙(제외 디렉터리, 생성 표시, 긴 줄)으로 모든 후보 파일을 tree-sitter로 읽어
collect_repo.py와 같은 조건의 이름(식별자 잎 노드)을 센다. 태그 값·정의 분류·개념 단위 판정은 하지 않는다.
세는 것
  - 후보 파일·줄 수, 언어별 파일 수, 구문 오류 파일 수, 읽지 못한 파일
  - 지원하지 않는 확장자 파일 수(제외 디렉터리 밖, 상위 15개)
  - 전체 고유 이름 수, 테스트 파일에만 나온 이름 수
  - 표본 단위 판정 입력(unit-input.json)의 이름 가운데 전체에도 있는 수, 전체에만 있는 새 이름 수
출력: full-scale-probe.json
"""
import collections
import json
import re
import sys
import time
from pathlib import Path
from word_eval_paths import CHECKOUTS, work_dir

from tree_sitter_language_pack import get_parser

import collect_repo
import languages

WORK = work_dir()
REPOSITORIES = ["bitwarden-server", "express", "flask", "gin", "gutenberg", "keycloak", "redis", "sentry",
                "terminal", "tokio", "vector", "woocommerce"]
NON_SOURCE = {".md", ".json", ".txt", ".png", ".svg", ".jpg", ".gif", ".lock", ".yml", ".yaml", ".snap", ".map", ".ico",
              ".woff", ".woff2", ".ttf", ".eot", ".pdf", ".csv", ".xml", ".html", ".css", ".scss", ".less", ".toml",
              ".cfg", ".ini", ".properties", ".mo", ".po", ".pot", ".gz", ".zip", ".jar", ".webp", ".mp4", ".bin"}


def unsupported_files(repository):
    counts = collections.Counter()
    for path in repository.rglob("*"):
        relative = path.relative_to(repository)
        if not path.is_file() or collect_repo.EXCLUDED_DIRECTORIES & set(relative.parts[:-1]):
            continue
        if path.suffix and path.suffix not in collect_repo.EXTENSIONS:
            counts[path.suffix.lower()] += 1
    return counts


def probe(name):
    repository = CHECKOUTS / name
    started = time.time()
    files, skipped = collect_repo.candidate_files(repository)
    is_cpp_repository = any(Path(f).suffix in collect_repo.CPP_EXTENSIONS for f, _ in files)
    parsers, names, test_names, non_test_names = {}, set(), set(), set()
    languages_seen, parse_errors, unreadable = collections.Counter(), 0, []
    for relative, _ in files:
        path = repository / relative
        language = collect_repo.language_of(path, is_cpp_repository)
        try:
            parser = parsers.setdefault(language, get_parser(language))
            source = path.read_bytes()
            if language in ("c", "cpp"):
                source = languages.blank_sal_annotations(source)
            tree = parser.parse(source)
        except Exception as error:  # 읽지 못한 파일과 이유를 남긴다
            unreadable.append({"file": relative, "error": f"{type(error).__name__}: {error}"[:200]})
            continue
        languages_seen[language] += 1
        parse_errors += tree.root_node.has_error
        is_test = bool(collect_repo.TEST_PATH.search(relative))
        for node in languages.walk(tree.root_node):
            if node.type in languages.NAME_LEAVES:
                text = languages.clean_name(node.text.decode(errors="replace"))
                if not text or text == "_" or not re.fullmatch(r"[\w$]+", text):
                    continue
                names.add(text)
                (test_names if is_test else non_test_names).add(text)
    sample_names = {item["name"] for item in json.loads((WORK / name / "unit-input.json").read_text())
                    if item["where"] != "struct_tag"}
    unsupported = unsupported_files(repository)
    return {
        "repository": name, "seconds": round(time.time() - started),
        "candidate_files": len(files), "candidate_lines": sum(n for _, n in files), "skipped": dict(skipped),
        "languages": dict(languages_seen.most_common()), "parse_error_files": parse_errors, "unreadable": unreadable,
        "unsupported_source_like": dict(collections.Counter(
            {k: v for k, v in unsupported.items() if k not in NON_SOURCE}).most_common(15)),
        "unique_names": len(names), "test_only_names": len(test_names - non_test_names),
        "sample_names": len(sample_names), "sample_names_in_full": len(sample_names & names),
        "new_names": len(names - sample_names), "new_names_not_test_only": len((names - sample_names) - (test_names - non_test_names)),
    }


def main(names):
    rows = []
    for name in names:
        row = probe(name)
        rows.append(row)
        print(json.dumps(row, ensure_ascii=False), flush=True)
    (WORK / "full-scale-probe.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main(sys.argv[1:] or REPOSITORIES)
