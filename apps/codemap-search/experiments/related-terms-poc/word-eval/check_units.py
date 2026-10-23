"""단위 분류 결과 묶음 하나가 입력과 같은 수·순서인지, 단위가 식별자를 덮는지 확인한다.

사용법: python3 check_units.py <저장소> <묶음 이름 ...>
"""
import collections
import json
import re
import sys
from word_eval_paths import batches_dir

BATCHES = batches_dir()


def compact(text):
    return re.sub(r"[^a-z0-9]", "", text.lower())


def main(repository, *batches):
    for batch in batches:
        items = json.load(open(f"{BATCHES}/units-input/{repository}/{batch}.json"))
        try:
            judged = json.load(open(f"{BATCHES}/units/{repository}/{batch}.json"))
        except FileNotFoundError:
            print(repository, batch, "결과 없음")
            continue
        is_ordered = [(i["name"], i["where"]) for i in items] == [(j["name"], j.get("where")) for j in judged]
        uncovered = [j["name"] for j in judged if compact("".join(u["text"] for u in j["units"])) != compact(j["name"])]
        counts = collections.Counter(u.get("categorizable") for j in judged for u in j["units"])
        print(repository, batch, "입력", len(items), "결과", len(judged), "순서 일치" if is_ordered else "순서 다름",
              "분류 가능", counts[True], "불가", counts[False], "덮지 못함", len(uncovered), uncovered[:5])


if __name__ == "__main__":
    main(*sys.argv[1:])
