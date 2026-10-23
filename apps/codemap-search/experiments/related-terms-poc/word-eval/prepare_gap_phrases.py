"""데이터셋 추가 1단계: 데이터셋에 없거나 관계 파일에만 있는 문구를 하나로 볼지 나눌지 판정할 묶음과 지침을 만든다.

사용법: python3 prepare_gap_phrases.py [묶음당 항목 수, 기본 200]
입력: dataset-coverage-v2.json, <저장소>/units(-v2).json, <저장소>/unit-input.json, tag-input.json, gap-phrase-brief.md
출력: gap/phrase-input.json(전체), <작업 폴더>/batches/gap/phrase-input/pNN.json, <작업 폴더>/batches/briefs/gap-phrase-pNN.md
판정 결과는 <작업 폴더>/batches/gap/phrase/pNN.json에 쓰게 한다.
"""
import json
import sys
from pathlib import Path
from word_eval_paths import CHECKOUTS, SCRIPTS, batches_dir, work_dir

from cross_repo_summary import REPOSITORIES

WORK = work_dir()
BATCHES = batches_dir()
BATCH_SIZE = 200


def code_usage_by_word():
    """단어별 저장소 코드 쓰임: 분석가가 적은 분야·뜻, 출처 식별자, 코드 줄 예시."""
    usage = {}
    for repository in REPOSITORIES:
        directory = WORK / repository
        units_file = "units-v2.json" if (directory / "judge-summary-v2.json").exists() else "units.json"
        examples = {}
        for name in ("unit-input.json", "tag-input.json"):
            if (directory / name).exists():
                for item in json.loads((directory / name).read_text()):
                    examples.setdefault(item["name"], item["examples"])
        for unit in json.loads((directory / units_file).read_text()):
            if not unit["is_included"]:
                continue
            entry = usage.setdefault(unit["text"], {"fields": [], "senses": [], "expansions": [], "identifiers": [], "code": []})
            entry["fields"] += [f for f in unit["fields"] if f not in entry["fields"]]
            entry["senses"] += [s for s in unit["senses"] if s not in entry["senses"]][:4 - len(entry["senses"])]
            entry["expansions"] += [e for e in unit["expansions"] if e not in entry["expansions"]]
            for name in unit["sources"][:4]:
                if len(entry["identifiers"]) < 6 and f"{repository}: {name}" not in entry["identifiers"]:
                    entry["identifiers"].append(f"{repository}: {name}")
                if examples.get(name) and len(entry["code"]) < 3:
                    entry["code"].append(f"{repository}/{examples[name][0]}")
    return usage


def main(batch_size=BATCH_SIZE):
    batch_size = int(batch_size)
    usage = code_usage_by_word()
    coverage = json.loads((WORK / "dataset-coverage-v2.json").read_text())
    items = [{"text": w["word"], "dataset_status": w["dataset_status"], "repositories": sorted(w["repositories"]),
              **usage[w["word"]]}
             for w in coverage if w["is_phrase"] and w["dataset_status"] in ("missing", "relation_only")]
    (WORK / "gap").mkdir(exist_ok=True)
    (WORK / "gap" / "phrase-input.json").write_text(json.dumps(items, ensure_ascii=False, indent=1))
    input_directory, result_directory = BATCHES / "gap/phrase-input", BATCHES / "gap/phrase"
    input_directory.mkdir(parents=True, exist_ok=True)
    result_directory.mkdir(parents=True, exist_ok=True)
    brief = (SCRIPTS / "gap-phrase-brief.md").read_text().replace("{checkouts}", str(CHECKOUTS))
    names = []
    for start in range(0, len(items), batch_size):
        name = f"p{start // batch_size + 1:02d}"
        (input_directory / f"{name}.json").write_text(json.dumps(items[start:start + batch_size], ensure_ascii=False, indent=1))
        Path(f"{BATCHES}/briefs/gap-phrase-{name}.md").write_text(
            brief + f"\n## 배정\n입력 파일: `{input_directory / name}.json`\n결과 파일: `{result_directory / name}.json`\n")
        names.append(name)
    print(len(items), "문구", names)


if __name__ == "__main__":
    main(*sys.argv[1:2])
