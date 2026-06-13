#!/usr/bin/env python3
"""축 B 채점 — 에이전트가 반환한 locations를 데이터셋 앵커로 채점.

recall   = essential(class=edit) 앵커 적중 / 전체 essential
precision = 반환 위치 중 '유효'(essential ∪ accepted_alternate ∪ mechanical ∪ understand) 적중 / 전체 반환
            (accepted_alternate·mechanical·understand는 precision 면제 = 유효로 계산, recall 가점 없음)
over_return = 어떤 앵커에도 안 맞은 반환 위치 수
F2 = (1+4)·P·R / (4P + R)   (β=2, recall 가중)

매칭: point=±tol, region=allowed_lines 범위, region_multi=allowed_regions 중 하나.
파일경로는 정규화 비교(레포 상대경로).

usage:
  python3 score_axisB.py <dataset.json> <task_id> <runs.json>
  runs.json: [{"arm":"baseline","rep":1,"locations":["src/x.rs:10",...],
               "tokens":12345,"tool_calls":7}, ...]   # tokens/tool_calls 선택
출력: 런별 표 + arm별 평균.
"""
import json, sys, re
from statistics import mean

TOL = 3

def norm_path(p):
    return p.strip().lstrip("./").replace("\\", "/")

def parse_loc(s):
    m = re.match(r"^(.*?):(\d+)", s.strip())
    if not m:
        return None
    return norm_path(m.group(1)), int(m.group(2))

def anchor_targets(task):
    """returns (essential, valid_extra) where each is list of (path, matcher)."""
    def mk(a):
        path = norm_path(a["loc"].rsplit(":", 1)[0]) if ":" in a["loc"] else norm_path(a["loc"])
        line = int(a["loc"].rsplit(":", 1)[1]) if ":" in a["loc"] else None
        match = a.get("match", "point")
        if match == "region":
            lo, hi = a["allowed_lines"]
            return (path, ("range", [(lo, hi)]))
        if match == "region_multi":
            return (path, ("range", [tuple(r) for r in a["allowed_regions"]]))
        return (path, ("point", line))
    essential = [mk(a) for a in task.get("essential_anchors", [])]
    extra = []
    for key in ("accepted_alternates", "mechanical_wiring", "understand_anchors", "context_optional"):
        for a in task.get(key, []):
            loc = a["loc"]
            path = norm_path(loc.rsplit(":", 1)[0])
            line = int(loc.rsplit(":", 1)[1])
            extra.append((path, ("point", line)))
    return essential, extra

def hits(loc, target):
    (lp, ll), (tp, (kind, val)) = loc, target
    if lp != tp:
        return False
    if kind == "point":
        return val is not None and abs(ll - val) <= TOL
    # kind == "range": hit if within any allowed region
    return any(lo <= ll <= hi for lo, hi in val)

def score_run(task, locations):
    locs = [parse_loc(s) for s in locations]
    locs = [l for l in locs if l]
    essential, extra = anchor_targets(task)
    # recall: each essential anchor hit by >=1 returned loc
    hit_ess = sum(1 for t in essential if any(hits(l, t) for l in locs))
    recall = hit_ess / len(essential) if essential else 0.0
    # precision: returned loc valid if hits any essential or extra
    valid = sum(1 for l in locs if any(hits(l, t) for t in essential + extra))
    precision = valid / len(locs) if locs else 0.0
    over = len(locs) - valid
    f2 = (5 * precision * recall / (4 * precision + recall)) if (precision + recall) else 0.0
    return dict(recall=recall, precision=precision, f2=f2, over_return=over,
                hit_essential=hit_ess, n_essential=len(essential), n_returned=len(locs))

def main():
    if len(sys.argv) < 4:
        print("usage: score_axisB.py <dataset.json> <task_id> <runs.json>"); sys.exit(2)
    ds = json.load(open(sys.argv[1]))
    task = next(t for t in ds["tasks"] if t["id"] == sys.argv[2])
    runs = json.load(open(sys.argv[3]))
    print(f"# {sys.argv[2]} — essential={len(task['essential_anchors'])}\n")
    print(f"{'arm':<10} {'rep':>3} {'recall':>7} {'prec':>6} {'F2':>6} {'over':>4} {'tok':>7} {'calls':>5}")
    by_arm = {}
    for r in runs:
        sc = score_run(task, r["locations"])
        by_arm.setdefault(r["arm"], []).append({**sc, **r})
        print(f"{r['arm']:<10} {r.get('rep',0):>3} {sc['recall']:>7.2f} {sc['precision']:>6.2f} "
              f"{sc['f2']:>6.2f} {sc['over_return']:>4} {r.get('tokens',0):>7} {r.get('tool_calls',0):>5}")
    print("\n## arm 평균")
    print(f"{'arm':<10} {'recall':>7} {'prec':>6} {'F2':>6} {'over':>5} {'tok':>8} {'calls':>6}")
    for arm, rs in by_arm.items():
        print(f"{arm:<10} {mean(x['recall'] for x in rs):>7.2f} {mean(x['precision'] for x in rs):>6.2f} "
              f"{mean(x['f2'] for x in rs):>6.2f} {mean(x['over_return'] for x in rs):>5.1f} "
              f"{mean(x.get('tokens',0) for x in rs):>8.0f} {mean(x.get('tool_calls',0) for x in rs):>6.1f}")

if __name__ == "__main__":
    main()
