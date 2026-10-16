"""태그 값 기준 2판에서 바뀐 단어를 관찰 기록으로 쓴다(판정 없음).

사용법: python3 write_tag_report.py   (merge_tag_units.py combine 다음에 돌린다)
입력: <저장소>/tag-input-summary.json, tag-merge.json, judge-results-tags.jsonl, judge-results(-r2).jsonl, units-v2.json, gin/tag-test-only.json
출력: tag-values-result.md
"""
import collections
import json
from pathlib import Path
from word_eval_paths import work_dir

WORK = work_dir()
TAG_REPOSITORIES = ["flask", "keycloak", "tokio", "vector", "bitwarden-server", "sentry"]
RESULTS = ("match", "partial", "none", "no_result")


def read_json(path):
    return json.loads(Path(path).read_text())


def read_rows(path):
    return [json.loads(line) for line in open(path)] if Path(path).exists() else []


def main():
    lines = ["# 태그 값 기준 2판에서 바뀐 단어 (판정 없음, 관찰 기록)", "",
             "- 기준(사용자 확정, 2026-10-11): 태그 표기(속성·애너테이션·데코레이터) 안의 값을 더함. 공백이 들어간 문자열과 테스트 파일의 값은 넣지 않음",
             "- 1판 판정은 그대로 두고, 새로 분류 가능이 된 단어만 같은 지침으로 luna 판정(merge_tag_units.py)",
             "- gin은 struct 태그 항목이 모두 테스트 파일에만 있어 빼고, 그 태그에서만 분류 가능으로 판정된 단어를 2판에서 뺌", "",
             "## 저장소별", "",
             "| 저장소 | 태그 값 항목 | 뺀 문자열(공백 / 테스트 파일) | 새 단어 | match | partial | none | no_result | 빠진 단어 |",
             "|---|---|---|---|---|---|---|---|---|"]
    totals = collections.Counter()
    sections = []
    for repository in ["gin"] + TAG_REPOSITORIES:
        directory = WORK / repository
        merge = read_json(directory / "tag-merge.json")
        status = {row["text"]: row["dataset_status"] for row in read_json(directory / "units-v2.json")}
        new_rows = read_rows(directory / "judge-results-tags.jsonl")
        counter = collections.Counter(row["sense_result"] for row in new_rows)
        totals.update(counter)
        if repository == "gin":
            test_only = read_json(directory / "tag-test-only.json")
            items, skipped = f"struct 태그 {test_only['struct_tag_items']}(모두 테스트 파일)", "-"
            old_rows = {row["word"]: row for row in read_rows(directory / "judge-results-r2.jsonl")}
            old_status = {row["text"]: row["dataset_status"] for row in read_json(directory / "units.json")}
            removed = [old_rows[word] for word in merge["removed_words"]]
            sections += ["", f"## gin — 2판에서 빠진 단어 {len(removed)}개 (1판 판정)", ""]
            sections += [f"- `{row['word']}` ({old_status[row['word']]}) — 1판 {row['sense_result']}" for row in removed]
            removed_text = str(len(removed))
        else:
            summary = read_json(directory / "tag-input-summary.json")
            items = str(summary["items"])
            skipped = f"{summary['skipped_strings'].get('has_whitespace', 0)} / {summary['skipped_strings'].get('test_file', 0)}"
            removed_text = str(len(merge["removed_words"]))
            if new_rows:
                sections += ["", f"## {repository} — 새 단어 {len(new_rows)}개", ""]
                sections += [f"- `{row['word']}` ({status[row['word']]}) — {row['sense_result']}, 응답 관련어 {row['returned_count']}개"
                             for row in new_rows]
        lines.append(f"| {repository} | {items} | {skipped} | {len(merge['new_words'])} | "
                     + " | ".join(str(counter.get(k, 0)) for k in RESULTS) + f" | {removed_text} |")
    lines += ["", "- 새 단어 합계: " + ", ".join(f"{k} {totals.get(k, 0)}" for k in RESULTS)
              + f" (모두 {sum(totals.values())}개)"]
    (WORK / "tag-values-result.md").write_text("\n".join(lines + sections) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
