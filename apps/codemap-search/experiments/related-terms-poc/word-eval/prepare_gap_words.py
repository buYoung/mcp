"""데이터셋 추가 2단계: 단어를 넣을지와 넣을 자리를 판정할 묶음과 지침을 만든다.

사용법:
  python3 prepare_gap_words.py base      데이터셋에 없거나 관계 파일에만 있는 한 단어 + 뜻 보강 후보 → wNN
  python3 prepare_gap_words.py phrases   1단계에서 유지한 문구 + 나눈 문구의 구성 단어 가운데 데이터셋에 없거나 관계 파일에만 있는 것 → xNN
뜻 보강 후보: 표제어·입력 뜻 단어 가운데 (코드 분야가 데이터셋 분야에 없거나 일부만 있고 match가 아닌 저장소가 있음) 또는 none 판정이 있는 것
입력: dataset-coverage-v2.json, gap/phrase-results.json(phrases), 저장소별 units·unit-input·tag-input, 기본 데이터셋, gap-word-brief.md
출력: gap/word-input-<모드>.json, <작업 폴더>/batches/gap/word-input/<묶음>.json, <작업 폴더>/batches/briefs/gap-word-<묶음>.md
판정 결과는 <작업 폴더>/batches/gap/word/<묶음>.json에 쓰게 한다.
"""
import collections
import json
import sqlite3
import sys
from pathlib import Path
from word_eval_paths import CHECKOUTS, EXPERIMENTS, SCRIPTS, batches_dir, work_dir

sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402
from term_canonical import word_key  # noqa: E402

import aggregate_units  # noqa: E402
from prepare_gap_phrases import code_usage_by_word  # noqa: E402

WORK = work_dir()
BATCHES = batches_dir()
BATCH_SIZE = 80


class Dataset:
    """현재 사전의 뜻·관련어를 판정 입력에 붙일 만큼만 읽는다."""

    def __init__(self):
        self.connection = sqlite3.connect(f"file:{sl.DEFAULT_SENSE_DATASET}?mode=ro", uri=True)
        self.head_fields, self.input_fields, self.related = aggregate_units.load_dataset(sl.DEFAULT_SENSE_DATASET)
        surfaces = dict(self.connection.execute("SELECT word_key, surface FROM terms"))
        # 뜻 자리 = (주제, 개념). rt-sense/4에서 주제에만 연결된 단어는 개념이 None인 자리에 든다.
        self.places, self.word_places, self.members = {}, collections.defaultdict(list), collections.defaultdict(list)
        for persona, key, _, concept, topic, _ in sorted(sl.persona_rows(self.connection)):
            self.places[persona] = (topic, concept)
            self.word_places[key].append((topic, concept))
            if surfaces[key] not in self.members[(topic, concept)]:
                self.members[(topic, concept)].append(surfaces[key])
        self.concept_names = dict(self.connection.execute("SELECT concept, name FROM concepts"))
        self.topic_info = {topic: (label, field) for topic, label, field in self.connection.execute(
            "SELECT tp.topic, tp.label, tf.field FROM topics tp JOIN topic_fields tf ON tf.topic = tp.topic AND tf.rank = 0")}

    def status(self, text):
        key = word_key(text)
        return ("head" if key in self.head_fields else "input" if key in self.input_fields
                else "relation_only" if key in self.related else "missing")

    def sense_rows(self, places):
        rows = []
        for topic, concept in sorted(set(places), key=lambda place: (place[1] is None, place[1] or 0, place[0]))[:12]:
            label, field = self.topic_info[topic]
            rows.append({"field": field, "topic": topic, "topic_label": label, "concept": concept,
                         "concept_name": self.concept_names.get(concept), "words": self.members[(topic, concept)][:6]})
        return rows

    def current_senses(self, text):
        key = word_key(text)
        own = self.word_places.get(key, [])
        through = [self.places[p] for (p,) in self.connection.execute("SELECT persona FROM input_senses WHERE word_key = ?", (key,))]
        return self.sense_rows(own + [place for place in through if place not in own])

    def relation_evidence(self, text):
        rows = self.connection.execute(
            "SELECT t.surface, r.persona FROM relations r JOIN terms t USING (term_id) WHERE r.word_key = ? LIMIT 40",
            (word_key(text),)).fetchall()
        evidence = []
        for head, persona in rows:
            if len(evidence) >= 6:
                break
            entry = {"head": head}
            if persona is not None:
                topic, concept = self.places[persona]
                label, field = self.topic_info[topic]
                entry.update({"concept": concept, "concept_name": self.concept_names.get(concept), "topic_label": label,
                              "field": field})
            evidence.append(entry)
        return evidence


def item_for(text, status, repositories, usage, dataset, kind):
    item = {"text": text, "kind": kind, "dataset_status": status, "repositories": repositories, **usage}
    if kind == "sense_gap" or status == "input":
        item["current_senses"] = dataset.current_senses(text)
    if status == "relation_only":
        item["relation_evidence"] = dataset.relation_evidence(text)
    return item


def is_sense_gap(word):
    results = {v for v in word["repositories"].values() if v}
    return word["dataset_status"] in ("head", "input") and (
        (word["field_coverage"] in ("none", "some") and results - {"match"}) or "none" in results)


def base_items(coverage, usage, dataset):
    items = []
    for word in coverage:
        if not word["is_phrase"] and word["dataset_status"] in ("missing", "relation_only"):
            items.append(item_for(word["word"], word["dataset_status"], word["repositories"], usage[word["word"]], dataset, "word"))
        elif is_sense_gap(word):
            items.append(item_for(word["word"], word["dataset_status"], word["repositories"], usage[word["word"]], dataset, "sense_gap"))
    return items


def phrase_items(coverage, usage, dataset):
    """유지한 문구, 그리고 나눈 문구의 구성 단어 가운데 평가셋에 없던 단어(데이터셋에 없거나 관계 파일에만 있는 것)."""
    by_word = {word["word"]: word for word in coverage}
    results = json.loads((WORK / "gap" / "phrase-results.json").read_text())
    items, components = [], collections.defaultdict(lambda: {"repositories": {}, "usage": None, "phrases": []})
    for result in results:
        word = by_word[result["text"]]
        if result["decision"] == "keep":
            items.append(item_for(word["word"], word["dataset_status"], word["repositories"], usage[word["word"]], dataset, "word"))
            continue
        for component in result["components"]:
            if component in by_word:  # 이미 평가셋 단어면 base 묶음이나 기존 판정이 맡는다
                continue
            entry = components[component]
            entry["repositories"].update({repository: None for repository in word["repositories"]})
            entry["phrases"].append(word["word"])
            if entry["usage"] is None:
                entry["usage"] = dict(usage[word["word"]], senses=[], expansions=[])
    for component, entry in sorted(components.items()):
        status = dataset.status(component)
        if status in ("missing", "relation_only"):
            usage_entry = dict(entry["usage"], from_phrases=entry["phrases"][:6])
            items.append(item_for(component, status, entry["repositories"], usage_entry, dataset, "word"))
    return items


def main(mode):
    dataset = Dataset()
    usage = code_usage_by_word()
    coverage = json.loads((WORK / "dataset-coverage-v2.json").read_text())
    items = base_items(coverage, usage, dataset) if mode == "base" else phrase_items(coverage, usage, dataset)
    # 같은 분야의 단어가 한 묶음에 모이게 해서 새 개념·주제가 묶음 안에서 겹쳐 만들어지지 않게 한다.
    items.sort(key=lambda item: ((item.get("fields") or ["~"])[0], item["kind"], item["text"]))
    (WORK / "gap" / f"word-input-{mode}.json").write_text(json.dumps(items, ensure_ascii=False, indent=1))
    input_directory, result_directory = BATCHES / "gap/word-input", BATCHES / "gap/word"
    input_directory.mkdir(parents=True, exist_ok=True)
    result_directory.mkdir(parents=True, exist_ok=True)
    brief = ((SCRIPTS / "gap-word-brief.md").read_text().replace("{catalog}", str(BATCHES / "gap/catalog"))
             .replace("{checkouts}", str(CHECKOUTS)))
    prefix, names = ("w" if mode == "base" else "x"), []
    for start in range(0, len(items), BATCH_SIZE):
        name = f"{prefix}{start // BATCH_SIZE + 1:02d}"
        (input_directory / f"{name}.json").write_text(json.dumps(items[start:start + BATCH_SIZE], ensure_ascii=False, indent=1))
        Path(f"{BATCHES}/briefs/gap-word-{name}.md").write_text(
            brief + f"\n## 배정\n입력 파일: `{input_directory / name}.json`\n결과 파일: `{result_directory / name}.json`\n")
        names.append(name)
    kinds = collections.Counter(item["kind"] for item in items)
    print(mode, len(items), dict(kinds), len(names), "묶음", names[0], "~", names[-1])


if __name__ == "__main__":
    main(sys.argv[1])
