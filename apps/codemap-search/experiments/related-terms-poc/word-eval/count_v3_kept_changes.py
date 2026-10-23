"""3판에서 이전 판정을 그대로 썼지만 응답이 바뀐 단어(10,695개)를 관련어 변경 유형별로 나눈다(판정 없음, 수만 센다).

사용법: python3 count_v3_kept_changes.py
대상: 저장소별 3판 단어 가운데 다시 판정하지 않았고(judge-results-v3new.jsonl에 없음), 이전 판정이 있으며,
      이전·새 데이터셋 응답 원문이 다른 단어
유형: 관련어 목록이 늘기만 함 / 줄기만 함 / 늘고 줄어듦 / 목록 같음(순서·표기만 다름)
크기: 관련어 수 변화율 ±10% 이내 / 10~50% / 50% 초과
도움어 빠짐: 이전 판정의 helpful_words 가운데 새 응답에 없는 것이 있음
출력: v3-kept-changes.json(요약과 단어별 목록)
"""
import collections
import json
from word_eval_paths import work_dir

import prepare_v3
import summarize_judge

WORK = work_dir()
REPOSITORIES = ["bitwarden-server", "express", "flask", "gin", "gutenberg", "keycloak", "redis", "sentry",
                "terminal", "tokio", "vector", "woocommerce"]


def change_kind(added, removed):
    if added and removed:
        return "늘고 줄어듦"
    if added:
        return "늘기만"
    if removed:
        return "줄기만"
    return "목록 같음"


def size_band(old_count, new_count):
    change = abs(new_count - old_count) / max(old_count, 1)
    return "±10% 이내" if change <= 0.1 else "10~50%" if change <= 0.5 else "50% 초과"


def main():
    words = []
    for repository in REPOSITORIES:
        directory = WORK / repository
        rejudged = {row["word"] for row in prepare_v3.read_jsonl(directory / "judge-results-v3new.jsonl")}
        _, v2_rows, _ = prepare_v3.v2_state(repository)
        previous = {row["word"]: row for row in v2_rows}
        old = prepare_v3.read_jsonl(directory / "lookups-v3-old.jsonl")
        new = prepare_v3.read_jsonl(directory / "lookups-v3-new.jsonl")
        for o, n in zip(old, new):
            word = o["word"]
            if word in rejudged or word not in previous or o["output_text"] == n["output_text"]:
                continue
            old_words = set(summarize_judge.listed_words(o["output_text"]))
            new_words = set(summarize_judge.listed_words(n["output_text"]))
            helpful_lost = sorted(set(previous[word].get("helpful_words") or []) - new_words)
            words.append({"repository": repository, "word": word, "sense_result": previous[word]["sense_result"],
                          "kind": change_kind(new_words - old_words, old_words - new_words),
                          "size": size_band(len(old_words), len(new_words)),
                          "old_related": len(old_words), "new_related": len(new_words),
                          "helpful_lost": helpful_lost})
    summary = {
        "words": len(words),
        "by_kind": dict(collections.Counter(w["kind"] for w in words)),
        "by_kind_size": {f"{k} / {s}": c for (k, s), c in sorted(collections.Counter((w["kind"], w["size"]) for w in words).items())},
        "helpful_lost": dict(collections.Counter(w["kind"] for w in words if w["helpful_lost"])),
        "by_kind_result": {f"{k} / {r}": c for (k, r), c in sorted(collections.Counter((w["kind"], w["sense_result"]) for w in words).items())},
    }
    (WORK / "v3-kept-changes.json").write_text(json.dumps({"summary": summary, "words": words}, ensure_ascii=False, indent=1))
    print(json.dumps(summary, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
