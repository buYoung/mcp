"""데이터셋 추가 판정 묶음 하나를 확인한다(판정 없음, 기계 확인).

사용법: python3 check_gap_batch.py <묶음>...   예: w06 x01 p03 i01
  - 입력·결과의 수, 순서, text가 같은지
  - (w·x) 판정 분포, 목록에 없는 개념·주제 id, 건너뛴 항목
  - (i) 목록에 없는 개념·주제 id, 이어받지 않는 항목
"""
import collections
import json
import sys
from word_eval_paths import batches_dir

BATCHES = batches_dir()

TMP = BATCHES / "gap"


def check(name):
    kind = {"p": "phrase", "i": "inherit"}.get(name[0], "word")
    items = json.loads((TMP / f"{kind}-input" / f"{name}.json").read_text())
    result_path = TMP / kind / f"{name}.json"
    if not result_path.exists():
        print(name, "결과 없음")
        return
    results = json.loads(result_path.read_text())
    print(name, "입력", len(items), "결과", len(results), "순서·text 일치", [i["text"] for i in items] == [r.get("text") for r in results])
    if kind == "phrase":
        return
    concepts = {json.loads(line)["concept"] for line in open(TMP / "catalog" / "concepts.jsonl")}
    topics = {json.loads(line)["topic"] for line in open(TMP / "catalog" / "topics.jsonl")}
    if kind == "word":
        print(" ", dict(collections.Counter(f"{r['kind']}/{r.get('type')}/{r.get('decision')}" for r in results).most_common()))
    unknown = []
    for result in results:
        for sense in result.get("senses") or []:
            attach = sense.get("attach") or {}
            if "concept" in attach and not attach.get("new_concept") and int(attach["concept"]) not in concepts:
                unknown.append((result["text"], attach))
            if attach.get("topic") and attach["topic"] not in topics:
                unknown.append((result["text"], attach))
    print("  목록에 없는 id", unknown)
    if kind == "inherit":
        print("  이어받지 않음", [(r["text"], len(r.get("senses") or [])) for r in results if not r.get("inherit")])
        return
    print("  건너뜀", [(r["text"], r.get("type")) for r in results if r.get("decision") == "skip"])


if __name__ == "__main__":
    for name in sys.argv[1:]:
        check(name)
