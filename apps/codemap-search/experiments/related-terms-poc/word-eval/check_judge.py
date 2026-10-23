"""판정 결과 묶음이 입력과 같은 수·id 순서인지 확인한다.

사용법: python3 check_judge.py <저장소> <묶음 이름 ...>
"""
import collections
import json
import os
import sys
from word_eval_paths import batches_dir

BATCHES = batches_dir()


def main(repository, *batches):
    for batch in batches:
        items = json.load(open(f"{BATCHES}/repo-judge-input/{repository}/{batch}.json"))
        try:
            judged = json.load(open(f"{BATCHES}/repo-judge/{repository}/{batch}.json"))
        except FileNotFoundError:
            # 판정자가 gin 때 쓰던 폴더에 잘못 쓴 적이 있어 그 자리도 알려 준다(옮기지는 않는다).
            misplaced = f"{BATCHES}/judge/{repository}/{batch}.json"
            print(repository, batch, "결과 없음", f"(잘못된 경로에 있음: {misplaced})" if os.path.exists(misplaced) else "")
            continue
        is_ordered = [i["id"] for i in items] == [j["id"] for j in judged]
        results = collections.Counter(j["sense_result"] for j in judged)
        print(repository, batch, "입력", len(items), "결과", len(judged), "순서 일치" if is_ordered else "순서 다름", dict(results))


if __name__ == "__main__":
    main(*sys.argv[1:])
