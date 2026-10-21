"""수집기 결과(definitions.jsonl)와 분석 에이전트가 원문을 읽고 만든 목록을 대조한다.

사용법: python3 compare_verify.py <수집 결과 디렉터리> <에이전트 결과 디렉터리> <저장소 경로>
같은 (파일, 줄, 이름)이면 일치로 보고 종류를 비교한다. 남은 항목은 줄 번호가 2줄 안에서 다른 같은 이름끼리 다시 맞춘다.
"""
import collections
import json
import sys
from pathlib import Path

LINE_TOLERANCE = 2


def take_match(pool, record, tolerance):
    for index, candidate in enumerate(pool):
        if candidate["name"] == record["name"] and abs(candidate["line"] - record["line"]) <= tolerance:
            return pool.pop(index)
    return None


def main(collected_directory, analyst_directory, repository):
    collected_directory, repository = Path(collected_directory), Path(repository)
    files = set(json.loads((collected_directory / "verify-files.json").read_text()))
    analyst = [record for path in sorted(Path(analyst_directory).glob("v*.json"))
               for record in json.loads(path.read_text())]
    collector = [record for record in map(json.loads, open(collected_directory / "definitions.jsonl"))
                 if record["file"] in files]
    unknown_files = {record["file"] for record in analyst} - files
    if unknown_files:
        print(f"배정되지 않은 파일: {sorted(unknown_files)}", file=sys.stderr)

    by_file = collections.defaultdict(lambda: {"collector": [], "analyst": []})
    for record in collector:
        by_file[record["file"]]["collector"].append(record)
    for record in analyst:
        by_file[record["file"]]["analyst"].append(record)

    rows, totals = [], collections.Counter()
    lines_cache = {}
    for file in sorted(files):
        pool = list(by_file[file]["collector"])
        remaining_analyst, matched = [], []
        for tolerance in (0, LINE_TOLERANCE):
            remaining_analyst = []
            for record in (by_file[file]["analyst"] if tolerance == 0 else leftovers):
                found = take_match(pool, record, tolerance)
                if found:
                    matched.append((found, record, tolerance))
                else:
                    remaining_analyst.append(record)
            leftovers = remaining_analyst
        source_lines = lines_cache.setdefault(file, (repository / file).read_text().splitlines())

        def context(line):
            return source_lines[line - 1].strip() if 0 < line <= len(source_lines) else ""

        kind_mismatches = [(c, a) for c, a, _ in matched if c["kind"] != a["kind"]]
        counts = {
            "collector": len(by_file[file]["collector"]), "analyst": len(by_file[file]["analyst"]),
            "matched": len(matched), "matched_with_line_offset": sum(1 for *_, t in matched if t),
            "analyst_only": len(remaining_analyst), "collector_only": len(pool), "kind_mismatch": len(kind_mismatches),
        }
        totals.update(counts)
        rows.append({
            "file": file, **counts,
            "analyst_only_items": [{**r, "source": context(r["line"])} for r in remaining_analyst],
            "collector_only_items": [{"name": r["name"], "line": r["line"], "kind": r["kind"], "source": context(r["line"])}
                                     for r in pool],
            "kind_mismatch_items": [{"name": c["name"], "line": c["line"], "collector_kind": c["kind"],
                                     "analyst_kind": a["kind"], "note": a.get("note"), "source": context(c["line"])}
                                    for c, a in kind_mismatches],
        })
    report = {"totals": dict(totals), "files": rows}
    (collected_directory / "verify-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=1))
    print(json.dumps(report["totals"], ensure_ascii=False))
    for row in rows:
        print(f'{row["file"]:42s} 수집기={row["collector"]:4d} 분석={row["analyst"]:4d} 일치={row["matched"]:4d} '
              f'분석만={row["analyst_only"]:3d} 수집기만={row["collector_only"]:3d} 종류다름={row["kind_mismatch"]:3d}')


if __name__ == "__main__":
    main(*sys.argv[1:4])
