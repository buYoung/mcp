"""12개 저장소의 판정 집계를 한 표로 모은다(판정 없음, 관찰 기록).

사용법: python3 cross_repo_summary.py [v2]
입력: <저장소>/judge-summary.json(gin은 같은 데이터셋으로 다시 판정한 judge-summary-r2.json), <저장소>/units-summary.json, <저장소>/summary.json
      v2를 주면 judge-summary-v2.json·units-summary-v2.json이 있는 저장소는 그것을 쓴다(태그 값 기준 반영, merge_tag_units.py)
출력: cross-repo-summary.json, cross-repo-summary.md(표까지만 씀, 관찰 메모는 따로 덧붙임). v2는 cross-repo-summary-v2.*
"""
import collections
import json
import sys
from word_eval_paths import work_dir

WORK = work_dir()
REPOSITORIES = ["gin", "express", "flask", "keycloak", "tokio", "redis", "terminal", "vector", "bitwarden-server",
                "gutenberg", "woocommerce", "sentry"]
RESULTS = ("match", "partial", "none", "no_result")


def rate(counter, keys):
    total = sum(counter.get(k, 0) for k in RESULTS)
    return round(sum(counter.get(k, 0) for k in keys) / total, 3) if total else None


def load(repository, version=None):
    directory = WORK / repository
    # gin 1회차는 이전 데이터셋으로 판정했으므로 다른 저장소와 같은 데이터셋인 2회차를 쓴다.
    judge_file = "judge-summary-r2.json" if repository == "gin" else "judge-summary.json"
    units_file = "units-summary.json"
    if version == "v2" and (directory / "judge-summary-v2.json").exists():
        judge_file, units_file = "judge-summary-v2.json", "units-summary-v2.json"
    judge = json.loads((directory / judge_file).read_text())
    units = json.loads((directory / units_file).read_text())
    collected = json.loads((directory / "summary.json").read_text())
    if "languages" not in collected:
        # gin은 표본 없이 저장소 전체를 collect_words.py로 모았다(전체 Go 23,672줄이 다른 저장소의 표본 줄 수 기준).
        collected = {"languages": {collected["language"]: collected["file_count"]}, "sampled_lines": 23672}
    groups = {}
    for group in ("all", "single_words", "phrases"):
        counter = judge[group]["sense_result"]
        groups[group] = {"words": judge[group]["words"], "sense_result": counter,
                         "match_rate": rate(counter, ("match",)), "match_or_partial_rate": rate(counter, ("match", "partial"))}
    return {
        "repository": repository, "judge_file": judge_file,
        "languages": collected["languages"], "sampled_lines": collected["sampled_lines"],
        "unit_count": units["unit_count"], "included_count": units["included_count"], "excluded_count": units["excluded_count"],
        "included_missing": units["included_by_status"].get("missing", 0),
        "groups": groups,
        "helpful_median": judge["all"]["helpful_count"]["median"], "returned_median": judge["all"]["returned_count"]["median"],
        "no_result_mismatch": judge["checks"]["no_result_mismatch"],
        "excluded_groups": {group: judge[group]["words"] for group in ("local_abbreviation", "fixture_or_error")},
    }


def main(version=None):
    suffix = f"-{version}" if version else ""
    rows = [load(repository, version) for repository in REPOSITORIES]
    totals = {group: collections.Counter() for group in ("all", "single_words", "phrases")}
    for row in rows:
        for group in totals:
            totals[group].update(row["groups"][group]["sense_result"])
    summary = {
        "repositories": rows,
        "totals": {group: {"words": sum(counter.values()), "sense_result": dict(counter),
                           "match_rate": rate(counter, ("match",)), "match_or_partial_rate": rate(counter, ("match", "partial"))}
                   for group, counter in totals.items()},
    }
    (WORK / f"cross-repo-summary{suffix}.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1))

    percent = lambda value: f"{value * 100:.1f}%"
    gin = rows[REPOSITORIES.index("gin")]
    lines = [
        "# 12개 저장소 단어평가셋 판정 모음 (판정 없음, 관찰 기록)", "",
        "- 데이터셋: rts2-32469ce3ab670de2, 뜻을 주제별로 나눔. 판정자: gpt-6-luna medium",
        "- gin은 같은 데이터셋으로 다시 판정한 2회차 값(응답이 바뀌지 않은 243단어는 1회차 판정을 그대로 씀)",
        f"- gin의 한 단어·문구 값은 지역 약어 {gin['excluded_groups']['local_abbreviation']}개와 테스트 값 "
        f"{gin['excluded_groups']['fixture_or_error']}개를 뺀 {gin['groups']['single_words']['words'] + gin['groups']['phrases']['words']}개 기준"
        f"(전체 값에는 포함). 그래서 합계의 한 단어+문구가 전체보다 {sum(gin['excluded_groups'].values())}개 적음",
        "- 같은 입력 재판정 시 단어별 판정이 약 21% 바뀜(gin/round-compare.md). 저장소 사이 몇 %p 차이는 이 흔들림 안에 있을 수 있음", "",
    ]
    if version == "v2":
        lines += ["- 2판: 태그 표기 안의 값(테스트 파일 제외, 공백 없는 값만)을 더하고 gin의 테스트 파일 struct 태그를 뺌. "
                  "1판 판정은 그대로 두고 새 단어만 판정함(merge_tag_units.py). 판정 파일 열: 어느 판을 썼는지", ""]
    lines += [
        "## 저장소별",
        "| 저장소 | 언어 | 단위 | 포함 | 그중 데이터셋에 없음 | match | partial | none | no_result | match 비율 | match+partial | 한 단어 match / +partial | 문구 match / +partial | 도움 단어 중앙값 | 응답 관련어 중앙값 | 판정 파일 |",
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for row in rows:
        counter, single, phrase = (row["groups"][g] for g in ("all", "single_words", "phrases"))
        languages = ", ".join(f"{name} {count}" for name, count in sorted(row["languages"].items(), key=lambda item: -item[1]))
        lines.append(
            f"| {row['repository']} | {languages} | {row['unit_count']} | {row['included_count']} | {row['included_missing']} | "
            + " | ".join(str(counter["sense_result"].get(k, 0)) for k in RESULTS)
            + f" | {percent(counter['match_rate'])} | {percent(counter['match_or_partial_rate'])}"
            f" | {percent(single['match_rate'])} / {percent(single['match_or_partial_rate'])}"
            f" | {percent(phrase['match_rate'])} / {percent(phrase['match_or_partial_rate'])}"
            f" | {row['helpful_median']:g} | {row['returned_median']:g} | {row['judge_file']} |")
    lines += ["", "## 합계"]
    for group, label in (("all", "전체"), ("single_words", "한 단어"), ("phrases", "문구")):
        total = summary["totals"][group]
        lines.append(f"- {label} {total['words']}개: " + ", ".join(f"{k} {total['sense_result'].get(k, 0)}" for k in RESULTS)
                     + f" — match {percent(total['match_rate'])}, match+partial {percent(total['match_or_partial_rate'])}")
    (WORK / f"cross-repo-summary{suffix}.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main(*sys.argv[1:2])
