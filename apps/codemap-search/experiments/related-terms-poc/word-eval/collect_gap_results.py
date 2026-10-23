"""데이터셋 추가 판정 결과를 모은다(판정 없음, 기계 처리).

사용법:
  python3 collect_gap_results.py phrases   1단계 문구 판정(pNN) → gap/phrase-results.json, gap/phrase-summary.json
  python3 collect_gap_results.py words     2단계 단어 판정(wNN, xNN) → gap/word-results.json, gap/entries.json, gap/entries-report.json
entries.json은 build_sense_dataset.py add-entries의 입력이다.
  - derived·abbreviation_common: root·expansion이 데이터셋에 뜻(자기 뜻 또는 입력 뜻)이 있고 senses가 비어 있거나 inherit: true면 inherit,
    senses가 있으면 분석가가 고른 자리도 붙인다
  - 그 밖의 add·sense_gap add: 분석가가 고른 자리(기존 개념, 기존 주제의 새 개념, 새 주제, 새 분야)
  - 새 주제는 key가 같거나, 이름과 주 분야가 같으면 하나로 합친다(묶음마다 따로 만든 같은 주제)
  - 이어받기 다시 판정 결과(<작업 폴더>/batches/gap/inherit/iNN.json)가 있으면 (묶음, 단어)의 inherit·senses를 그것으로 바꾼다
  - gap/new-topics.json에 정리 전 새 주제 목록을 쓰고, 새 주제 정리 판정(<작업 폴더>/batches/gap/topic/rNN.json)이 있으면
    합치기(merge)·기존 주제로 옮기기(move)·기존 개념으로 넣기(concepts)를 반영한다
"""
import collections
import json
import re
import sqlite3
import sys
from word_eval_paths import EXPERIMENTS, batches_dir, work_dir

sys.path.insert(0, str(EXPERIMENTS))
import sense_lookup as sl  # noqa: E402
from term_canonical import word_key  # noqa: E402

WORK = work_dir()
BATCHES = batches_dir()
GAP = WORK / "gap"
TMP = BATCHES / "gap"


def read_batches(kind, prefixes):
    """입력 묶음과 결과 묶음을 맞춰 읽는다. 수·순서·text가 다르면 문제로 적는다."""
    rows, problems = [], []
    for prefix in prefixes:
        for input_path in sorted((TMP / f"{kind}-input").glob(f"{prefix}*.json")):
            items = json.loads(input_path.read_text())
            result_path = TMP / kind / input_path.name
            if not result_path.exists():
                problems.append(f"{input_path.stem}: 결과 없음")
                continue
            results = json.loads(result_path.read_text())
            if [i["text"] for i in items] != [r.get("text") for r in results]:
                problems.append(f"{input_path.stem}: 입력 {len(items)} 결과 {len(results)}, 순서·text 다름")
                continue
            for item, result in zip(items, results):
                rows.append({**result, "batch": input_path.stem, "input": item})
    return rows, problems


def collect_phrases():
    rows, problems = read_batches("phrase", ["p"])
    results = [{k: v for k, v in row.items() if k != "input"} for row in rows]
    (GAP / "phrase-results.json").write_text(json.dumps(results, ensure_ascii=False, indent=1))
    summary = {"phrases": len(results), "problems": problems,
               "by_type_decision": dict(collections.Counter(f"{r['type']}/{r['decision']}" for r in results).most_common()),
               "by_status_decision": dict(collections.Counter(f"{row['input']['dataset_status']}/{row['decision']}" for row in rows)),
               "components": len({c for r in results for c in r.get("components", [])})}
    (GAP / "phrase-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))
    print(json.dumps(summary, ensure_ascii=False, indent=1))


def slug(text):
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


class EntryBuilder:
    def __init__(self):
        connection = sqlite3.connect(f"file:{sl.DEFAULT_SENSE_DATASET}?mode=ro", uri=True)
        self.concepts = {c for (c,) in connection.execute("SELECT concept FROM concepts")}
        self.topics = {t for (t,) in connection.execute("SELECT topic FROM topics")}
        self.fields = {f for (f,) in connection.execute("SELECT field FROM fields")}
        self.with_senses = {k for (k,) in connection.execute("SELECT DISTINCT t.word_key FROM personas p JOIN terms t USING (term_id)")}
        self.with_senses |= {k for (k,) in connection.execute("SELECT DISTINCT word_key FROM input_senses")}
        connection.close()
        self.new_fields, self.new_topics, self.topic_alias, self.new_concepts = {}, {}, {}, {}
        self.personas, self.inherit, self.invalid = [], [], []
        self.meanings = collections.defaultdict(list)  # 새 개념 key → [(단어, 분석가가 적은 뜻)]

    def topic_key(self, topic):
        """새 주제를 key로, 아니면 (이름, 주 분야)로 합친다."""
        key = slug(topic.get("key") or topic.get("label", ""))
        if key in self.topic_alias:
            return self.topic_alias[key]
        by_label = next((k for k, t in self.new_topics.items()
                         if t["label"].lower() == topic.get("label", "").lower() and t["fields"][:1] == topic.get("fields", [])[:1]), None)
        if by_label:
            self.topic_alias[key] = by_label
            return by_label
        self.topic_alias[key] = key
        self.new_topics[key] = {"key": key, "label": topic.get("label", key), "scope": topic.get("scope", ""),
                                "fields": list(dict.fromkeys(topic.get("fields", [])))[:3]}
        return key

    def place(self, word, attach, meaning=""):
        """분석가가 고른 자리 하나를 페르소나로 바꾼다. 쓸 수 없는 자리는 invalid에 적는다."""
        if attach.get("new_field"):
            field = attach["new_field"]
            if field.get("field") and field["field"] not in self.fields:
                self.new_fields.setdefault(field["field"], {"field": field["field"], "label": field.get("label", field["field"]),
                                                            "scope": field.get("scope", "")})
        if "concept" in attach and not attach.get("new_concept"):
            concept = int(attach["concept"]) if str(attach["concept"]).isdigit() else attach["concept"]
            if concept not in self.concepts:
                self.invalid.append({"word": word, "attach": attach, "reason": "unknown concept"})
                return False
            self.personas.append({"word": word, "concept": concept})
            return True
        name = (attach.get("new_concept") or word).strip()
        if attach.get("new_topic"):
            topic = self.topic_key(attach["new_topic"])
            unknown = [f for f in self.new_topics[topic]["fields"] if f not in self.fields and f not in self.new_fields]
            if unknown or not self.new_topics[topic]["fields"]:
                self.invalid.append({"word": word, "attach": attach, "reason": f"unknown fields {unknown}"})
                return False
        elif attach.get("topic") in self.topics:
            topic = attach["topic"]
        else:
            self.invalid.append({"word": word, "attach": attach, "reason": "unknown topic"})
            return False
        key = f"{topic}::{name.lower()}"
        self.new_concepts.setdefault(key, {"key": key, "name": name, "topic": topic})
        self.personas.append({"word": word, "concept": key})
        self.meanings[key].append((word, meaning))
        return True

    def add(self, row):
        word = row["text"]
        source = row.get("root") if row.get("type") == "derived" else row.get("expansion") if row.get("type") == "abbreviation_common" else None
        # inherit가 적혀 있으면 그대로 따른다(true면 이어받고 적은 자리도 함께 붙인다).
        # 없으면 자리를 직접 적지 않았을 때만 이어받는다(기본형의 사전 뜻이 코드 쓰임과 다르면 분석가가 자리를 적는다).
        wants_inherit = row["inherit"] if "inherit" in row else not row.get("senses")
        is_inherited = bool(source) and word_key(source) in self.with_senses and bool(wants_inherit)
        if is_inherited:
            self.inherit.append({"word": word, "from": source})
        placed = [self.place(word, sense.get("attach") or {}, sense.get("meaning", "")) for sense in row.get("senses") or []]
        if is_inherited:
            return "inherit+placed" if any(placed) else "inherit"
        return "placed" if any(placed) else "unplaced"

    def new_topic_listing(self):
        """새 주제마다 개념과 그 단어·뜻(주제 정리 판정의 입력)."""
        listing = []
        for key, topic in self.new_topics.items():
            concepts = [{"name": c["name"], "words": [{"word": w, "meaning": m} for w, m in self.meanings[c["key"]]]}
                        for c in self.new_concepts.values() if c["topic"] == key]
            if concepts:
                listing.append({**topic, "concepts": concepts})
        return listing

    def apply_topic_review(self, decisions):
        """새 주제 정리 판정을 반영한다.
        merge: 다른 새 주제(into)로 합침, move: 기존 주제(into)의 새 개념으로 옮김,
        concepts: {개념 이름: {"concept": 기존 개념 id}}이면 그 개념의 단어를 기존 개념에 넣고 새 개념은 없앤다."""
        target, to_existing = {}, {}
        for decision in decisions:
            key, kind, into = decision.get("key"), decision.get("decision"), decision.get("into")
            if key not in self.new_topics:
                self.invalid.append({"topic_review": decision, "reason": "unknown new topic"})
                continue
            if kind == "merge" and into in self.new_topics and into != key:
                target[key] = into
            elif kind == "move" and into in self.topics:
                target[key] = into
            elif kind != "keep":
                self.invalid.append({"topic_review": decision, "reason": "unknown target"})
            for name, attach in (decision.get("concepts") or {}).items():
                concept = int(attach["concept"]) if str(attach.get("concept", "")).isdigit() else None
                if concept in self.concepts:
                    to_existing[f"{key}::{name.lower()}"] = concept
                else:
                    self.invalid.append({"topic_review": decision, "concept": name, "reason": "unknown concept"})

        def final(topic):
            seen = set()
            while topic in target and topic not in seen:
                seen.add(topic)
                topic = target[topic]
            return topic

        remap = {}
        for key, concept in list(self.new_concepts.items()):
            if key in to_existing:
                remap[key] = to_existing[key]
                del self.new_concepts[key]
                continue
            topic = final(concept["topic"])
            if topic == concept["topic"]:
                continue
            new_key = f"{topic}::{concept['name'].lower()}"
            del self.new_concepts[key]
            self.new_concepts.setdefault(new_key, {"key": new_key, "name": concept["name"], "topic": topic})
            remap[key] = new_key
        personas, seen = [], set()
        for persona in self.personas:
            concept = remap.get(persona["concept"], persona["concept"])
            if (persona["word"], concept) not in seen:
                seen.add((persona["word"], concept))
                personas.append({"word": persona["word"], "concept": concept})
        self.personas = personas
        return {"merged_or_moved_topics": len(target), "concepts_to_existing": len(to_existing), "remapped_concepts": len(remap)}

    def entries(self):
        used_topics = {c["topic"] for c in self.new_concepts.values()}
        return {"fields": list(self.new_fields.values()),
                "topics": [t for k, t in self.new_topics.items() if k in used_topics],
                "concepts": list(self.new_concepts.values()), "personas": self.personas, "inherit": self.inherit}


def apply_inherit_review(rows, problems):
    """이어받기 다시 판정(iNN, prepare_inherit_review.py)이 있으면 그 묶음·단어의 inherit·senses를 덮어쓴다."""
    reviews, review_problems = read_batches("inherit", ["i"])
    problems.extend(review_problems)
    by_key = {(review["input"]["batch"], review["text"]): review for review in reviews}
    applied = 0
    for row in rows:
        review = by_key.get((row["batch"], row["text"]))
        if review:
            row["inherit"], row["senses"] = bool(review.get("inherit")), review.get("senses") or []
            row["inherit_review_note"] = review.get("note", "")
            applied += 1
    return applied, len(by_key)


def collect_words():
    rows, problems = read_batches("word", ["w", "x"])
    reviewed, review_count = apply_inherit_review(rows, problems)
    print("inherit review applied", reviewed, "of", review_count)
    builder = EntryBuilder()
    outcome = collections.Counter()
    unplaced = []
    for row in rows:
        decision = row.get("decision")
        label = f"{row['input']['kind']}/{row.get('type')}/{decision}"
        if decision == "add":
            how = builder.add(row)
            outcome[f"{label}/{how}"] += 1
            if how == "unplaced":
                unplaced.append(row["text"])
        else:
            outcome[label] += 1
    (GAP / "word-results.json").write_text(json.dumps([{k: v for k, v in r.items() if k != "input"} for r in rows],
                                                      ensure_ascii=False, indent=1))
    # 새 주제 정리(prepare_topic_review.py의 입력)는 정리 판정을 반영하기 전 상태로 쓴다.
    (GAP / "new-topics.json").write_text(json.dumps(builder.new_topic_listing(), ensure_ascii=False, indent=1))
    topic_reviews = [r for path in sorted((TMP / "topic").glob("r*.json")) for r in json.loads(path.read_text())]
    if topic_reviews:
        print("topic review", builder.apply_topic_review(topic_reviews))
    entries = builder.entries()
    (GAP / "entries.json").write_text(json.dumps(entries, ensure_ascii=False, indent=1))
    report = {"items": len(rows), "problems": problems, "outcome": dict(sorted(outcome.items())),
              "entries": {k: len(v) for k, v in entries.items()}, "invalid": builder.invalid, "unplaced": unplaced}
    (GAP / "entries-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=1))
    print(json.dumps({k: v for k, v in report.items() if k not in ("invalid", "unplaced")}, ensure_ascii=False, indent=1))
    print("invalid", len(builder.invalid), "unplaced", len(unplaced))


if __name__ == "__main__":
    {"phrases": collect_phrases, "words": collect_words}[sys.argv[1]]()
