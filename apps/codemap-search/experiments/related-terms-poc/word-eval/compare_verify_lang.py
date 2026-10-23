"""언어별 수집 규칙 검증: 수집 결과(<저장소>/definitions.jsonl)와 분석 에이전트 목록(<작업 폴더>/batches/verify-lang/<언어>.json)을 대조한다.

사용법: python3 compare_verify_lang.py [언어 ...]
같은 (파일, 줄, 이름)이면 일치, 남은 것은 같은 이름끼리 2줄 안에서 다시 맞춘다. 결과는 verify-lang-report.json.
"""
import collections
import json
import sys
from pathlib import Path
from word_eval_paths import CHECKOUTS, batches_dir, work_dir

ROOT = CHECKOUTS
WORK = work_dir()
BATCHES = batches_dir()
LINE_TOLERANCE = 2


def take(pool, record, tolerance):
    for index, candidate in enumerate(pool):
        if candidate["name"] == record["name"] and abs(candidate["line"] - record["line"]) <= tolerance:
            return pool.pop(index)
    return None


def main(selected):
    plan = json.loads((WORK / "verify-languages-plan.json").read_text())
    report_path = WORK / "verify-lang-report.json"
    report = json.loads(report_path.read_text()) if report_path.exists() else {}
    for language in selected or sorted(plan):
        result_path = Path(f"{BATCHES}/verify-lang/{language}.json")
        if not result_path.exists():
            continue
        files = {f"{repo}/{file}" for repo, file, _ in plan[language]}
        collector = collections.defaultdict(list)
        for repo in {repo for repo, _, _ in plan[language]}:
            for record in map(json.loads, open(WORK / repo / "definitions.jsonl")):
                key = f"{repo}/{record['file']}"
                if key in files:
                    collector[key].append(record)
        analyst = collections.defaultdict(list)
        for record in json.loads(result_path.read_text()):
            analyst[record["file"]].append(record)
        totals, analyst_only, collector_only, kind_mismatch = collections.Counter(), [], [], []
        for file in sorted(files):
            pool = list(collector[file])
            lines = (ROOT / file).read_text(errors="replace").splitlines()
            remaining = list(analyst[file])
            matched = []
            for tolerance in (0, LINE_TOLERANCE):
                rest = []
                for record in remaining:
                    found = take(pool, record, tolerance)
                    (matched.append((found, record)) if found else rest.append(record))
                remaining = rest
            source = lambda n: lines[n - 1].strip()[:140] if 0 < n <= len(lines) else ""
            totals.update(collector=len(collector[file]), analyst=len(analyst[file]), matched=len(matched),
                          analyst_only=len(remaining), collector_only=len(pool),
                          kind_mismatch=sum(1 for c, a in matched if c["kind"] != a["kind"]))
            analyst_only += [{"file": file, "line": r["line"], "name": r["name"], "kind": r["kind"], "note": r.get("note"),
                              "source": source(r["line"])} for r in remaining]
            collector_only += [{"file": file, "line": r["line"], "name": r["name"], "kind": r["kind"], "source": source(r["line"])}
                               for r in pool]
            kind_mismatch += [{"file": file, "line": c["line"], "name": c["name"], "collector": c["kind"], "analyst": a["kind"],
                               "source": source(c["line"])} for c, a in matched if c["kind"] != a["kind"]]
        report[language] = {"totals": dict(totals), "analyst_only": analyst_only, "collector_only": collector_only,
                            "kind_mismatch": kind_mismatch}
        t = totals
        print(f"{language:11s} 수집기={t['collector']:4d} 분석={t['analyst']:4d} 일치={t['matched']:4d} "
              f"분석만={t['analyst_only']:3d} 수집기만={t['collector_only']:3d} 종류다름={t['kind_mismatch']:3d}")
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main(sys.argv[1:])
