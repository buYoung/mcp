"""저장소 하나의 판정 집계를 관찰 기록 문서로 쓴다(express에서 쓴 형식과 같음, 판정 없음).

사용법: python3 write_judge_report.py <저장소 이름>
입력: <저장소>/judge-summary.json, <저장소>/judge-results.jsonl, <저장소>/units.json (summarize_judge.py 다음에 돌린다)
출력: <저장소>/judge-result.md
"""
import collections
import json
import sys
from word_eval_paths import work_dir

WORK = work_dir()


def main(repository):
    directory = WORK / repository
    summary = json.loads((directory / "judge-summary.json").read_text())
    rows = [json.loads(line) for line in open(directory / "judge-results.jsonl")]
    units = {unit["text"]: unit for unit in json.loads((directory / "units.json").read_text())}
    by_status = collections.defaultdict(collections.Counter)
    for row in rows:
        by_status[units[row["word"]]["dataset_status"]][row["sense_result"]] += 1
    results = summary["all"]["sense_result"]
    lines = [
        f"# {repository} 단어평가셋 판정 결과 (판정 없음, 관찰 기록)", "",
        "- 데이터셋: rts2-32469ce3ab670de2, 뜻을 주제별로 나눔. 판정자: gpt-6-luna medium, 지침 judge-brief-repo.md",
        f"- 단어 {summary['all']['words']}개: match {results.get('match', 0)}, partial {results.get('partial', 0)}, "
        f"none {results.get('none', 0)}, no_result {results.get('no_result', 0)}",
        f"- 한 단어({summary['single_words']['words']}): {summary['single_words']['sense_result']} / "
        f"문구({summary['phrases']['words']}): {summary['phrases']['sense_result']}",
        f"- 도움 단어 수 중앙값 {summary['all']['helpful_count']['median']}, "
        f"응답 관련어 수 중앙값 {summary['all']['returned_count']['median']}",
        f"- 기계 점검: 응답에 없는 도움 단어 {summary['checks']['helpful_words_not_in_output']}개(집계에서 뺌), "
        f"no_result 불일치 {summary['checks']['no_result_mismatch']}",
        "- 데이터셋 상태별: " + ", ".join(f"{status} {dict(counter)}" for status, counter in by_status.items()),
        "- 참고: gin에서 같은 입력 재판정 시 단어별 판정이 약 21% 바뀜(round-compare.md). 단어 하나의 판정보다 묶음 단위로 본다.", "",
        "## none / no_result",
    ]
    lines += [f"- {row['sense_result']} `{row['word']}` ({', '.join(row['code_fields'])}) {row['note'] or ''}"
              for row in rows if row["sense_result"] in ("none", "no_result")]
    (directory / "judge-result.md").write_text("\n".join(lines) + "\n")
    print(repository, summary["all"]["words"], "단어", dict(results), {k: dict(v) for k, v in by_status.items()})


if __name__ == "__main__":
    main(sys.argv[1])
