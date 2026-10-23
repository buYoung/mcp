"""2단계: 단어평가셋의 단어를 하나씩 사전 조회 프로그램(sense_lookup.py)에 넣고 응답을 저장한다.

사용법: python3 lookup_words.py <수집 결과 디렉터리> [출력 파일 이름, 기본 lookups.jsonl]
입력: units.json 가운데 분류 가능한 단위(is_included)
출력: lookups.jsonl — 단어마다 CLI에 그 단어 하나만 넣었을 때와 같은 텍스트 응답(output_text)과 구조화 결과
색인은 한 번만 읽고, 단어마다 lookup()과 render_text([결과])를 따로 호출해 한 단어씩 조회한 것과 같게 만든다.
"""
import json
import sys
from pathlib import Path
from word_eval_paths import EXPERIMENTS

sys.path.insert(0, str(EXPERIMENTS))
import category_lookup as cl  # noqa: E402
import sense_lookup as sl  # noqa: E402
from term_canonical import canonical_surface  # noqa: E402


def main(collected_directory, output_name="lookups.jsonl"):
    collected_directory = Path(collected_directory)
    words = [unit["text"] for unit in json.loads((collected_directory / "units.json").read_text()) if unit["is_included"]]
    with open(collected_directory / output_name, "w") as handle:
        for row in lookup_all(words):
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")
    print(f"{len(words)}개 조회 완료 → {collected_directory / output_name}")


def lookup_all(words, sense_dataset=None):
    """단어마다 CLI에 그 단어 하나만 넣었을 때와 같은 응답 행을 입력 순서대로 돌려준다.
    sense_dataset을 주면 CLI의 --sense-dataset처럼 그 데이터셋으로 조회한다(기본: 기본 데이터셋)."""
    sense_dataset = Path(sense_dataset) if sense_dataset else sl.DEFAULT_SENSE_DATASET
    # CLI 기본값과 같게: 뜻을 주제별로 나눔(--split-senses-by-topic)
    index = sl.SenseIndex.load(sense_dataset, is_topic_split=True)
    stored = cl.run_rtpoc3(sl.spelling_queries(words), cl.DEFAULT_RTPOC3, cl.DEFAULT_DATASET)
    rows = []
    for word in words:
        # CLI 기본값과 같게: --exact-first, --evidence-inflections 켬, 역방향·구문 건너뛰기 끔
        result = sl.lookup(word, stored.get(canonical_surface(word), []), index, is_reverse_included=False,
                           is_syntax_skipped=False, is_exact_first=True, is_inflection_evidence_used=True,
                           inflection_dataset=cl.DEFAULT_DATASET, spelling_matches=stored)
        rows.append({
            "word": word, "output_text": sl.render_text([result]), "status": result["status"],
            "candidate_count": result["candidate_count"], "resolution": result["resolution"],
            "sense_fields": [sense["field"] for sense in result["senses"]],
            "sense_dataset": str(sense_dataset),
        })
    return rows


if __name__ == "__main__":
    main(*sys.argv[1:3])
