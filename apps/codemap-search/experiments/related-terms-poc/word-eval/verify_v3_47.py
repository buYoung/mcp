"""3판에서 관련어가 크게 줄어든 한 단어 47개를 단어별 원자료로 다시 확인한다(판정 없음, 기계 확인).

사용법: python3 verify_v3_47.py
대상: 12개 저장소 3판 평가셋의 한 단어(공백 없음) 가운데, 이전 데이터셋 응답의 관련어가 20개 이상이고
      새 데이터셋 응답의 관련어가 그 1/5 이하인 단어(저장소가 여럿이면 단어 하나로 센다).
단어마다 모으는 것
  - 이전·새 조회 응답: 관련어 수, 입력 해석(seeds), 뜻 묶음 제목
  - 이전(rts2-32469ce3ab670de2)·새(rts2-309e9bbaa19f7eb5) 데이터셋 상태: 표제어 여부, input_senses 수, 다른 표제어 관계에 나온 수
  - 단어 판단 입력의 dataset_status, 단어 판단 결과(gap/word-results.json)의 kind·type·decision·batch
  - 이어받기 검토(<작업 폴더>/batches/gap/inherit/) 대상 여부와 결과, entries.json의 이어받기·페르소나
  - 새 데이터셋에서 그 단어가 든 개념·주제와 주제 소속 단어 수
  - 저장소별 2판(없으면 1판)·3판 판정
출력: v3-47-words.json, v3-47-words.md
"""
import collections
import glob
import json
import sqlite3
import sys
from pathlib import Path
from word_eval_paths import DATASETS, EXPERIMENTS, batches_dir, work_dir

sys.path.insert(0, str(EXPERIMENTS))
from term_canonical import word_key  # noqa: E402

import prepare_v3  # noqa: E402
import summarize_judge  # noqa: E402

WORK = work_dir()
BATCHES = batches_dir()
OLD_DATASET = DATASETS / "rts2-32469ce3ab670de2/dataset.sqlite3"
NEW_DATASET = DATASETS / "rts2-309e9bbaa19f7eb5/dataset.sqlite3"
REVIEWED_BATCHES = [f"w{n:02d}" for n in range(1, 44)]
REPOSITORIES = ["bitwarden-server", "express", "flask", "gin", "gutenberg", "keycloak", "redis", "sentry",
                "terminal", "tokio", "vector", "woocommerce"]


def related_count(output_text):
    return len(summarize_judge.listed_words(output_text))


def headings(output_text):
    return [line[3:].strip() for line in output_text.splitlines() if line.startswith("## ")]


def dataset_state(connection, key):
    is_headword = connection.execute("select count(*) from terms where word_key = ?", (key,)).fetchone()[0] > 0
    input_senses = connection.execute("select count(*) from input_senses where word_key = ?", (key,)).fetchone()[0]
    relation_mentions = connection.execute("select count(*) from relations where word_key = ?", (key,)).fetchone()[0]
    return {"is_headword": is_headword, "input_senses": input_senses, "relation_mentions": relation_mentions}


def new_concepts(connection, key):
    rows = connection.execute(
        "select c.concept, c.name, c.topic, tp.label, "
        "(select count(distinct term_id) from topic_members tm where tm.topic = c.topic) "
        "from personas p join terms t using (term_id) join concepts c using (concept) join topics tp on tp.topic = c.topic "
        "where t.word_key = ? order by c.concept", (key,)).fetchall()
    return [{"concept": r[0], "name": r[1], "topic": r[2], "topic_label": r[3], "topic_terms": r[4]} for r in rows]


def find_words():
    found = {}
    for repository in REPOSITORIES:
        directory = WORK / repository
        old = prepare_v3.read_jsonl(directory / "lookups-v3-old.jsonl")
        new = prepare_v3.read_jsonl(directory / "lookups-v3-new.jsonl")
        for o, n in zip(old, new):
            word = o["word"]
            if " " in word:
                continue
            old_count, new_count = related_count(o["output_text"]), related_count(n["output_text"])
            if old_count >= 20 and new_count <= old_count * 0.2:
                entry = found.setdefault(word, {"word": word, "repositories": [],
                                                "old_related": old_count, "new_related": new_count,
                                                "old_seeds": o["resolution"].get("seeds"),
                                                "new_seeds": n["resolution"].get("seeds"),
                                                "old_headings": headings(o["output_text"]),
                                                "new_headings": headings(n["output_text"])})
                entry["repositories"].append(repository)
    return found


def judgements(word, repositories):
    rows = []
    for repository in repositories:
        directory = WORK / repository
        _, v2_rows, _ = prepare_v3.v2_state(repository)
        before = {row["word"]: row for row in v2_rows}.get(word)
        rejudged = {row["word"]: row for row in prepare_v3.read_jsonl(directory / "judge-results-v3new.jsonl")}.get(word)
        after = rejudged or before
        rows.append({"repository": repository, "before": before["sense_result"] if before else None,
                     "after": after["sense_result"] if after else None, "rejudged": rejudged is not None})
    return rows


def review_exclusion(entry, judged, old_db):
    """이어받기 검토 대상 선정 조건(prepare_inherit_review.py) 가운데 어느 것에서 빠졌는지."""
    result = entry["word_result"]
    if entry["inherit_review"] is not None or result["kind"] != "word":
        return None
    if result["type"] not in ("derived", "abbreviation_common"):
        return f"type={result['type']}(검토 대상 유형 아님)"
    if result["batch"] not in REVIEWED_BATCHES:
        return f"{result['batch']}(이어받기 규칙이 단어 판단 지침에 들어간 뒤 판단한 묶음)"
    source = judged.get("root") if result["type"] == "derived" else judged.get("expansion")
    state = dataset_state(old_db, word_key(source)) if source else None
    if not source or not (state["is_headword"] or state["input_senses"]):
        return f"원래 말 `{source}`에 이전 데이터셋 뜻 없음"
    return "조건상 대상인데 빠짐(확인 필요)"


def group_of(entry):
    result = entry["word_result"]
    if result["kind"] == "sense_gap":
        return "A. 뜻 보충(kind=sense_gap): 이전 데이터셋에 input 뜻이 있던 단어에 새 뜻을 붙임"
    if result["type"] == "derived" and entry["inherit_review"] is not None:
        return "B. 새 표제어, 파생·활용형: 이어받기 검토에서 이어받지 않기로 함"
    if result["type"] == "derived":
        return "C. 새 표제어, 파생·활용형: 이어받기 검토 대상이 아니었음"
    return "D. 새 표제어, 파생·활용형 아님"


def main():
    words = find_words()
    inputs = {}
    for name in ["gap/word-input-base.json", "gap/word-input-phrases.json"]:
        for item in json.loads((WORK / name).read_text()):
            inputs.setdefault(item["text"].lower(), item)
    results = {}
    for row in json.loads((WORK / "gap/word-results.json").read_text()):
        results.setdefault(row["text"].lower(), row)
    judged_rows = {(Path(path).stem, row["text"].lower()): row for path in sorted(glob.glob(f"{BATCHES}/gap/word/*.json"))
                   for row in json.loads(Path(path).read_text())}
    reviews = {row["text"].lower(): row for path in sorted(glob.glob(f"{BATCHES}/gap/inherit/i*.json"))
               for row in json.loads(Path(path).read_text())}
    entries = json.loads((WORK / "gap/entries.json").read_text())
    inherited = {word_key(i["word"]): i["from"] for i in entries["inherit"]}
    persona_concepts = collections.defaultdict(list)
    for persona in entries["personas"]:
        persona_concepts[word_key(persona["word"])].append(persona["concept"])

    old_db, new_db = sqlite3.connect(OLD_DATASET), sqlite3.connect(NEW_DATASET)
    for word, entry in words.items():
        key = word_key(word)
        item = inputs.get(word.lower(), {})
        result = results.get(word.lower(), {})
        review = reviews.get(word.lower())
        entry.update({
            "dataset_status": item.get("dataset_status"),
            "relation_evidence": item.get("relation_evidence"),
            "old_dataset": dataset_state(old_db, key),
            "new_dataset": dataset_state(new_db, key),
            "word_result": {k: result.get(k) for k in ["kind", "type", "decision", "batch", "note"]},
            "inherit_review": ({"inherit": review.get("inherit"), "note": review.get("note")} if review else None),
            "inherited_from": inherited.get(key),
            "entry_concepts": persona_concepts.get(key, []),
            "new_concepts": new_concepts(new_db, key),
            "judgements": judgements(word, entry["repositories"]),
        })
        judged = judged_rows.get((entry["word_result"]["batch"], word.lower()), {})
        entry["word_result"].update({"root": judged.get("root"), "expansion": judged.get("expansion"),
                                     "note": judged.get("note") or entry["word_result"]["note"]})
        entry["review_exclusion"] = review_exclusion(entry, judged, old_db)
        entry["group"] = group_of(entry)
    rows = sorted(words.values(), key=lambda e: (e["group"], e["word"]))
    (WORK / "v3-47-words.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1))

    groups = collections.defaultdict(list)
    for row in rows:
        groups[row["group"]].append(row)
    status_by_group = {g[0]: dict(collections.Counter(r["dataset_status"] for r in m)) for g, m in groups.items()}
    group_a = next(m for g, m in groups.items() if g.startswith("A"))
    old_paths = collections.defaultdict(list)
    for row in rows:
        seeds = row["old_seeds"] or []
        if all(s["match"] == "lemma" for s in seeds):
            old_paths["기본형(lemma)만"].append(row["word"])
        elif any(s["span"] != row["word"] for s in seeds):
            old_paths["부분 단어 정확 일치"].append(row["word"])
        else:
            old_paths["자기 input 뜻(정확 일치)과 기본형"].append(row["word"])
    instances = [j for r in rows for j in r["judgements"]]
    only_exact = sum(1 for r in rows if [(s["span"], s["match"]) for s in (r["new_seeds"] or [])] == [(r["word"], "exact")])
    lines = ["# 관련어가 크게 줄어든 한 단어 47개 — 단어별 원자료", "",
             "`verify_v3_47.py`로 만들었다(판정 없음, 기계 확인). 원자료는 `v3-47-words.json`.", "",
             "## 대상과 출처", "",
             "- 대상: 12개 저장소 3판 평가셋의 한 단어(공백 없음) 가운데 이전 데이터셋(`rts2-32469ce3ab670de2`) 응답의 관련어가 "
             "20개 이상이고 새 데이터셋(`rts2-309e9bbaa19f7eb5`) 응답의 관련어가 그 1/5 이하인 단어. 여러 저장소에 있으면 한 단어로 센다.",
             "- 조회 응답: `<저장소>/lookups-v3-old.jsonl`·`lookups-v3-new.jsonl`",
             "- 데이터셋 상태: 두 데이터셋의 `terms`(표제어), `input_senses`(다른 표제어 뜻의 관련어로 분류됨), `relations.word_key`(관련어로 나옴)",
             "- `dataset_status`: 단어 판단 입력(`gap/word-input-base.json`)의 값. 정의는 `aggregate_units.py`·`prepare_gap_words.py`"
             "(head 자기 뜻 있음 / input 다른 표제어 뜻의 관련어로 분류됨 / relation_only 관련어로만 있고 분류 없음(관계 파일에만 있음) / missing 없음)",
             "- 단어 판단: `gap/word-results.json`(kind·type·batch). 이어받기 검토: `<작업 폴더>/batches/gap/inherit/i*.json`"
             "(대상 선정은 `prepare_inherit_review.py`: w01–w43의 `derived`·`abbreviation_common` 추가 중 원래 말에 뜻이 있는 것)",
             "- 이어받기 결과: `gap/entries.json`의 `inherit`. 판정: 저장소별 2판(없으면 1판)과 3판",
             "- 정확 일치 우선: `sense_lookup.py` `lookup()` 설명. 입력이 자기 뜻을 가진 표제어면 기본형으로 넓히지 않고, "
             "뜻이 `input_senses`에서만 오면 기본형도 함께 찾는다.", "",
             "## 묶음 규칙과 요약", "",
             "- A: 단어 판단 kind=`sense_gap`. B: kind=`word`, type=`derived`, 이어받기 검토 대상. "
             "C: kind=`word`, type=`derived`, 검토 대상 아님. D: 그 밖의 kind=`word`.",
             "- 묶음별 `dataset_status`: " + "; ".join(f"{g} {s}" for g, s in status_by_group.items()),
             f"- 이전 데이터셋에서 표제어였던 단어 {sum(r['old_dataset']['is_headword'] for r in rows)}개, "
             f"새 데이터셋에서 표제어인 단어 {sum(r['new_dataset']['is_headword'] for r in rows)}개, "
             f"기본형 뜻을 이어받은 단어 {sum(1 for r in rows if r['inherited_from'])}개.",
             "- 새 데이터셋에서 든 개념 수: " + ", ".join(
                 f"{n}개 {c}단어" for n, c in sorted(collections.Counter(len(r["new_concepts"]) for r in rows).items()))
             + f" (2개 이상: {', '.join(r['word'] for r in rows if len(r['new_concepts']) > 1)})",
             f"- A {len(group_a)}개 중 새 데이터셋에서 이전과 같은 수의 `input_senses`를 가진 단어: "
             f"{sum(1 for r in group_a if r['new_dataset']['input_senses'] == r['old_dataset']['input_senses'])}개",
             "- 이전 조회 경로: " + "; ".join(f"{path} {len(words)}개" for path, words in old_paths.items())
             + f". 새 조회에서 표제어 자신만 정확 일치로 찾는 단어: {only_exact}개",
             f"- 저장소별 {len(instances)}건 중 3판에서 다시 판정한 건: {sum(j['rejudged'] for j in instances)}건", "",
             "열: 단어 · 저장소 · 관련어(이전→새) · 이전 입력 해석(첫 seed: 일치 방식/찾은 표제어) · "
             "dataset_status(단어 판단 입력) · 이전 데이터셋(표제어/input 뜻 수/관계 언급 수) · "
             "단어 판단(kind/type/batch) · 이어받기 검토 · "
             "새 개념(주제, 주제 소속 단어 수) · 판정(저장소: 전→후, *는 다시 판정)", ""]
    for group, members in groups.items():
        lines += [f"## {group} — {len(members)}개", "",
                  "| 단어 | 저장소 | 관련어 | 이전 입력 해석 | dataset_status | 이전 데이터셋 | 단어 판단 | 이어받기 검토 | 새 개념 | 판정 |",
                  "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
        for row in members:
            seed = (row["old_seeds"] or [{}])[0]
            od = row["old_dataset"]
            result = row["word_result"]
            review = row["inherit_review"]
            review_text = "대상 아님" if review is None else f"inherit={review['inherit']}"
            concepts = "; ".join(f"{c['name']} ({c['topic_label']}, {c['topic_terms']})" for c in row["new_concepts"])
            judged = ", ".join(f"{j['repository']}: {j['before']}→{j['after']}{'*' if j['rejudged'] else ''}"
                               for j in row["judgements"])
            lines.append(
                f"| `{row['word']}` | {', '.join(row['repositories'])} | {row['old_related']}→{row['new_related']} | "
                f"{seed.get('match')}/{', '.join(seed.get('terms') or [])} | {row['dataset_status']} | "
                f"{'표제어' if od['is_headword'] else '표제어 아님'}/{od['input_senses']}/{od['relation_mentions']} | "
                f"{result['kind']}/{result['type']}/{result['batch']} | {review_text} | {concepts} | {judged} |")
        lines.append("")
    lines += ["## 이어받기 검토 대상이 아니었던 kind=`word` 단어와 이유", "",
              "| 단어 | type | 원래 말 | 이유 | 단어 판단 메모 |", "| --- | --- | --- | --- | --- |"]
    for row in rows:
        if row["review_exclusion"]:
            result = row["word_result"]
            lines.append(f"| `{row['word']}` | {result['type']} | {result['root'] or result['expansion'] or ''} | "
                         f"{row['review_exclusion']} | {(result['note'] or '').replace('|', '/')} |")
    lines += ["", "## B 묶음 이어받기 검토 메모", "", "| 단어 | 원래 말 | 검토 메모 |", "| --- | --- | --- |"]
    for row in rows:
        if row["inherit_review"] is not None:
            lines.append(f"| `{row['word']}` | {row['word_result']['root'] or ''} | "
                         f"{(row['inherit_review']['note'] or '').replace('|', '/')} |")
    lines.append("")
    (WORK / "v3-47-words.md").write_text("\n".join(lines))

    print("단어", len(rows))
    for group, members in groups.items():
        print(len(members), group)
    print("이전 데이터셋 상태:", collections.Counter(
        ("표제어" if r["old_dataset"]["is_headword"] else "표제어 아님",
         "input 뜻 있음" if r["old_dataset"]["input_senses"] else "input 뜻 없음",
         "관계 언급 있음" if r["old_dataset"]["relation_mentions"] else "관계 언급 없음") for r in rows))
    print("dataset_status:", collections.Counter(r["dataset_status"] for r in rows))
    print("묶음별 dataset_status:", {g: dict(collections.Counter(r["dataset_status"] for r in m)) for g, m in groups.items()})
    print("이전 일치 방식:", collections.Counter((r["group"][0], (r["old_seeds"] or [{}])[0].get("match")) for r in rows))
    print("이어받기:", sum(1 for r in rows if r["inherited_from"]))
    print("검토 제외 이유:", collections.Counter((r["review_exclusion"] or "-").split("(")[0] for r in rows))


if __name__ == "__main__":
    main()
