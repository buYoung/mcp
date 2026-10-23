"""3판 다시 판정(vNN) 묶음 하나를 확인하고, 띄운 것으로 적은 뒤 다음에 띄울 묶음을 알려 준다(판정 없음, 기계 확인).

사용법:
  python3 check_v3_judge.py done <저장소> <묶음>   결과 확인(수·순서·id·sense_result 값) 후 <작업 폴더>/batches/v3-done.txt에 적기
  python3 check_v3_judge.py launched <저장소> <묶음>   <작업 폴더>/batches/v3-launched.txt에 적기
  python3 check_v3_judge.py next [개수]            대기열에서 아직 띄우지 않은 묶음
  python3 check_v3_judge.py status                띄운 수·끝난 수·남은 수
"""
import json
import sys
from pathlib import Path
from word_eval_paths import batches_dir

BATCHES = batches_dir()

QUEUE = BATCHES / "v3-queue.txt"
LAUNCHED = BATCHES / "v3-launched.txt"
DONE = BATCHES / "v3-done.txt"
RESULTS = {"match", "partial", "none", "no_result"}


def lines(path):
    return [line.strip() for line in path.read_text().splitlines() if line.strip()] if path.exists() else []


def done(repository, batch):
    items = json.loads(Path(f"{BATCHES}/repo-judge-input/{repository}/{batch}.json").read_text())
    result_path = Path(f"{BATCHES}/repo-judge/{repository}/{batch}.json")
    if not result_path.exists():
        print(repository, batch, "결과 없음")
        return
    results = json.loads(result_path.read_text())
    same_order = [(i["id"], i["word"]) for i in items] == [(r.get("id"), r.get("word")) for r in results]
    bad = [r.get("id") for r in results if r.get("sense_result") not in RESULTS]
    print(repository, batch, "입력", len(items), "결과", len(results), "id·단어 순서 일치", same_order, "잘못된 sense_result", bad)
    if same_order and not bad:
        with open(DONE, "a") as handle:
            handle.write(f"{repository} {batch}\n")


def main():
    command, *arguments = sys.argv[1:]
    if command == "done":
        done(*arguments)
    elif command == "launched":
        with open(LAUNCHED, "a") as handle:
            handle.write(" ".join(arguments) + "\n")
    elif command == "next":
        count = int(arguments[0]) if arguments else 1
        launched = set(lines(LAUNCHED))
        print("\n".join([entry for entry in lines(QUEUE) if entry not in launched][:count]))
    elif command == "status":
        queue, launched, finished = lines(QUEUE), set(lines(LAUNCHED)), set(lines(DONE))
        print("전체", len(queue), "띄움", len(launched), "끝남", len(finished), "돌고 있음", len(launched - finished),
              "남음", len([e for e in queue if e not in launched]))


if __name__ == "__main__":
    main()
