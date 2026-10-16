"""데이터셋 추가 2단계용 목록: 현재 기본 데이터셋의 분야·주제·개념을 분석가가 rg로 찾아볼 수 있는 JSONL로 쓴다.

사용법: python3 build_gap_catalog.py
출력(<작업 폴더>/batches/gap/catalog/):
  fields.json         분야 id·이름·범위
  topics.jsonl        주제 하나당 한 줄(id, 이름, 범위, 분야, 개념 id·이름)
  concepts.jsonl      개념 하나당 한 줄(id, 이름, 주제 id·이름, 분야, 구성 단어 최대 20개)
"""
import collections
import json
import sqlite3
import sys
from word_eval_paths import EXPERIMENTS, batches_dir

BATCHES = batches_dir()

sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402

CATALOG = BATCHES / "gap/catalog"


def main():
    CATALOG.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(f"file:{sl.DEFAULT_SENSE_DATASET}?mode=ro", uri=True)
    fields = [{"field": f, "label": label, "scope": scope} for f, label, scope in connection.execute("SELECT field, label, scope FROM fields ORDER BY field")]
    topic_fields = collections.defaultdict(list)
    for topic, field in connection.execute("SELECT topic, field FROM topic_fields ORDER BY topic, rank"):
        topic_fields[topic].append(field)
    members = collections.defaultdict(list)
    for concept, surface in connection.execute(
            "SELECT DISTINCT p.concept, t.surface FROM personas p JOIN terms t USING (term_id) ORDER BY p.concept, p.persona"):
        members[concept].append(surface)
    topics = {topic: {"topic": topic, "label": label, "scope": scope, "fields": topic_fields[topic], "concepts": []}
              for topic, label, scope in connection.execute("SELECT topic, label, scope FROM topics ORDER BY topic")}
    concepts = []
    for concept, name, topic in connection.execute("SELECT concept, name, topic FROM concepts ORDER BY concept"):
        topics[topic]["concepts"].append({"concept": concept, "name": name})
        concepts.append({"concept": concept, "name": name, "topic": topic, "topic_label": topics[topic]["label"],
                         "fields": topic_fields[topic], "words": members[concept][:20]})
    connection.close()
    (CATALOG / "fields.json").write_text(json.dumps(fields, ensure_ascii=False, indent=1))
    with open(CATALOG / "topics.jsonl", "w") as handle:
        for topic in topics.values():
            handle.write(json.dumps(topic, ensure_ascii=False) + "\n")
    with open(CATALOG / "concepts.jsonl", "w") as handle:
        for concept in concepts:
            handle.write(json.dumps(concept, ensure_ascii=False) + "\n")
    print(f"{sl.DEFAULT_SENSE_DATASET.parent.name}: 분야 {len(fields)}, 주제 {len(topics)}, 개념 {len(concepts)} → {CATALOG}")


if __name__ == "__main__":
    main()
