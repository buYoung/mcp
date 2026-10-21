"""3판에서 다시 판정해 판정이 내린 단어를 이전 근거 뜻 묶음이 새 응답에 남았는지로 나눈다(판정 없음, 수만 센다).

사용법: python3 count_v3_downgrades.py
대상: 저장소별 judge-results-v3new.jsonl의 단어 가운데 이전 판정(2판, 없으면 1판)이 있고
      판정 순위(match > partial > none > no_result)가 내려간 단어
근거 묶음: 이전 판정의 matched_headings. 새 데이터셋 응답(lookups-v3-new.jsonl)의 뜻 묶음 제목(`## `)과 비교한다.
          모두 남음 / 일부 남음 / 모두 사라짐(이전 근거 묶음이 없던 단어는 '이전 근거 없음')
도움어: 이전 helpful_words 가운데 새 응답에 없는 것
출력: v3-downgrades.json(요약과 단어별 목록)
"""
import collections
import json
from word_eval_paths import work_dir

import prepare_v3
import summarize_judge

WORK = work_dir()
REPOSITORIES = ["bitwarden-server", "express", "flask", "gin", "gutenberg", "keycloak", "redis", "sentry",
                "terminal", "tokio", "vector", "woocommerce"]
RANK = {"match": 3, "partial": 2, "none": 1, "no_result": 0}


def headings(output_text):
    return {line[3:].strip() for line in output_text.splitlines() if line.startswith("## ")}


def evidence_state(previous_headings, new_headings):
    if not previous_headings:
        return "이전 근거 없음"
    kept = [h for h in previous_headings if h in new_headings]
    if len(kept) == len(previous_headings):
        return "모두 남음"
    return "일부 남음" if kept else "모두 사라짐"


def main():
    words = []
    for repository in REPOSITORIES:
        directory = WORK / repository
        _, v2_rows, _ = prepare_v3.v2_state(repository)
        previous = {row["word"]: row for row in v2_rows}
        new_outputs = {row["word"]: row["output_text"] for row in prepare_v3.read_jsonl(directory / "lookups-v3-new.jsonl")}
        for row in prepare_v3.read_jsonl(directory / "judge-results-v3new.jsonl"):
            before = previous.get(row["word"])
            if not before or RANK[row["sense_result"]] >= RANK[before["sense_result"]]:
                continue
            output_text = new_outputs[row["word"]]
            new_words = set(summarize_judge.listed_words(output_text))
            words.append({"repository": repository, "word": row["word"],
                          "before": before["sense_result"], "after": row["sense_result"],
                          "evidence": evidence_state(before.get("matched_headings") or [], headings(output_text)),
                          "previous_headings": before.get("matched_headings") or [],
                          "new_headings": row.get("matched_headings") or [],
                          "helpful_lost": sorted(set(before.get("helpful_words") or []) - new_words)})
    summary = {
        "words": len(words),
        "by_evidence": dict(collections.Counter(w["evidence"] for w in words)),
        "by_evidence_transition": {f"{e} / {t}": c for (e, t), c in sorted(collections.Counter(
            (w["evidence"], f"{w['before']}->{w['after']}") for w in words).items())},
        "with_helpful_lost": dict(collections.Counter(w["evidence"] for w in words if w["helpful_lost"])),
    }
    (WORK / "v3-downgrades.json").write_text(json.dumps({"summary": summary, "words": words}, ensure_ascii=False, indent=1))
    print(json.dumps(summary, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
