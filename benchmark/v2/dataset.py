"""Public PR provenance and code-anchored, independently adjudicable facts."""
from __future__ import annotations

import concurrent.futures
import re
import shutil
from pathlib import Path

from .core import (DIFFICULTIES, LANGUAGES, SPEC, canonical, command, digest, file_digest,
                   read_json, require, safe_path, write_json)


def github(endpoint: str, accept="application/vnd.github+json"):
    import json
    return json.loads(command(["gh", "api", "-H", f"Accept: {accept}", endpoint]))


def collect(output: Path, max_candidates: int = 400) -> dict:
    require(0 < max_candidates <= SPEC["max_candidates"], "candidate limit must be 1..400")
    output.mkdir(parents=True, exist_ok=True)
    commit_path = output / "commit.json"
    if not commit_path.exists():
        write_json(commit_path, github(f"repos/{SPEC['repository']}/commits/{SPEC['source_commit']}"))
    commit = read_json(commit_path)
    require(commit["sha"] == SPEC["source_commit"], "source commit mismatch")
    until = commit["commit"]["committer"]["date"]
    query = (f"repo:{SPEC['repository']} is:pr is:merged "
             f"merged:{SPEC['collect_since']}..{until} sort:updated-desc")
    from urllib.parse import urlencode
    candidates = []
    for page in range(1, (max_candidates + 99) // 100 + 1):
        search_path = output / f"search-{page}.json"
        if not search_path.exists():
            write_json(search_path, github("search/issues?" + urlencode({"q": query, "per_page": 100, "page": page})))
        candidates.extend(read_json(search_path)["items"])
    candidates = candidates[:max_candidates]
    require(len({p["number"] for p in candidates}) == len(candidates), "duplicate PR in search pages")

    def fetch(candidate):
        number = candidate["number"]
        folder = output / "prs" / str(number)
        folder.mkdir(parents=True, exist_ok=True)
        detail_path = folder / "pr.json"
        if not detail_path.exists():
            write_json(detail_path, github(f"repos/{SPEC['repository']}/pulls/{number}"))
        detail = read_json(detail_path)
        files_path = folder / "files.json"
        if not files_path.exists():
            files = []
            for page in range(1, (detail["changed_files"] + 99) // 100 + 1):
                files.extend(github(f"repos/{SPEC['repository']}/pulls/{number}/files?per_page=100&page={page}"))
            write_json(files_path, files)
        files = read_json(files_path)
        source_files = [f["filename"] for f in files if eligible_file(f["filename"])]
        reason = None
        if not detail["merged_at"] or not (SPEC["collect_since"] <= detail["merged_at"] <= until):
            reason = "outside merged time window"
        elif re.search(r"\b(backport|cherry.pick)\b", detail["title"], re.I):
            reason = "backport candidate"
        elif not source_files:
            reason = "documentation/dependency/generated-only"
        elif len(files) != detail["changed_files"]:
            reason = "incomplete file provenance"
        if not reason:
            ancestor_path = folder / "ancestry.json"
            if not ancestor_path.exists():
                comparison = github(f"repos/{SPEC['repository']}/compare/{detail['merge_commit_sha']}...{SPEC['source_commit']}")
                write_json(ancestor_path, {"status": comparison["status"],
                    "merge_commit": detail["merge_commit_sha"], "fixed_commit": SPEC["source_commit"],
                    "merge_base": comparison["merge_base_commit"]["sha"]})
            ancestry = read_json(ancestor_path)
            if ancestry["merge_base"] != detail["merge_commit_sha"]:
                reason = "merge is not ancestor of fixed commit"
        if not reason and not (folder / "change.patch").exists():
            patch = command(["gh", "api", "-H", "Accept: application/vnd.github.diff",
                             f"repos/{SPEC['repository']}/pulls/{number}"], timeout_seconds=120)
            (folder / "change.patch").write_text(patch, encoding="utf-8")
        result = {"pr": number, "url": detail["html_url"], "title": detail["title"],
                  "merge_commit": detail["merge_commit_sha"], "merged_at": detail["merged_at"],
                  "source_files": source_files, "eligible": reason is None, "exclusion_reason": reason,
                  "provenance": str(folder.relative_to(output)),
                  "artifacts": {p.name: file_digest(p) for p in sorted(folder.iterdir()) if p.is_file()}}
        print(f"PR {number}: {reason or 'candidate; fixed-code review required'}", flush=True)
        return result

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        records = list(executor.map(fetch, candidates))
    result = {"spec_sha256": digest(SPEC), "query": query, "until": until,
              "reviewed_candidates": len(records), "candidates": records}
    write_json(output / "candidates.json", result)
    return result


def eligible_file(path: str) -> bool:
    return (path.endswith((".ts", ".tsx", ".go"))
            and not re.search(r"(^|/)(vendor|node_modules|generated|docs?)/|\.gen\.|\.pb\.go$|_generated\.|lock", path))


def source_text(source: Path, path: str, start_line: int, end_line: int) -> str:
    require(type(start_line) is int and type(end_line) is int and 1 <= start_line <= end_line,
            "invalid evidence line range")
    lines = safe_path(source, path).read_text(encoding="utf-8").splitlines()
    require(end_line <= len(lines), f"evidence range outside file: {path}")
    return "\n".join(lines[start_line - 1:end_line])


def validate_dataset(dataset: dict, source: Path | None = None, complete=True) -> None:
    require(dataset.get("spec_sha256") == digest(SPEC), "dataset specification drift")
    require(dataset.get("source_commit") == SPEC["source_commit"], "dataset source drift")
    questions = dataset.get("questions", [])
    require(questions and len({q["id"] for q in questions}) == len(questions), "missing or duplicate question IDs")
    changes = set()
    prs = set()
    issues_by_phase = {"preparation": set(), "main": set()}
    for q in questions:
        require(re.fullmatch(r"[a-zA-Z0-9_-]+", q["id"]) is not None, "unsafe question ID")
        require(q["phase"] in SPEC["phases"] and q["difficulty"] in DIFFICULTIES
                and q["language"] in LANGUAGES, "invalid question stratum")
        require(q["prompt"].strip() and q.get("difficulty_rationale"), "question needs prompt and prospective difficulty rationale")
        require(q["source"]["pr"] not in prs and q["source"]["change_key"] not in changes, "duplicate PR/change")
        prs.add(q["source"]["pr"])
        changes.add(q["source"]["change_key"])
        issues_by_phase[q["phase"]].update(q["source"]["issue_keys"])
        require(q["source"].get("code_reviewed") is True, "fixed code must be reviewed before freezing")
        require(q["source"].get("ancestor_verified") is True, "missing ancestor verification")
        expected_suffix = (".ts", ".tsx") if q["language"] == "typescript" else (".go",)
        require(q["entrypoint"].endswith(expected_suffix), "entrypoint language mismatch")
        evidence = {e["id"]: e for e in q["evidence"]}
        require(len(evidence) == len(q["evidence"]) and evidence, "duplicate or missing evidence")
        for e in evidence.values():
            require(e["text"].strip() and e["text_sha256"] == digest(e["text"]), "evidence content/hash missing")
            require(e["end_line"] - e["start_line"] + 1 == len(e["text"].splitlines()), "evidence text/line count mismatch")
            if source:
                require(source_text(source, e["path"], e["start_line"], e["end_line"]) == e["text"],
                        f"fixed source evidence mismatch: {q['id']}/{e['id']}")
        fact_ids = {f["id"] for f in q["facts"]}
        require(fact_ids and len(fact_ids) == len(q["facts"]), "duplicate or missing facts")
        for fact in q["facts"]:
            require(fact["requirement"] and fact["requirement"] in q["prompt"], "fact not traceable to question requirement")
            require(fact["correct"].strip() and isinstance(fact["conditions"], list), "missing correct fact/conditions")
            require(fact["wrong_claims"] and isinstance(fact["confusions"], list), "missing incorrect/confusable claims")
            require(fact["evidence_sets"] and all(group and set(group) <= evidence.keys()
                    for group in fact["evidence_sets"]), "invalid evidence bundle or alternative")
        from .grading import validate_judgment, classify
        require({e["kind"] for e in q["examples"]} >= {"minimal", "alternative", "partial", "wrong"}, "missing grading controls")
        for example in q["examples"]:
            require(example["answer"].strip(), "empty grading control")
            judgment = dict(example["expected"], answer_id=example["id"], question_id=q["id"])
            validate_judgment(q, judgment, example["id"], example["answer"])
            expected = {"minimal": "correct", "alternative": "correct", "partial": "partial", "wrong": "incorrect"}[example["kind"]]
            require(classify(q, judgment) == expected, "control does not match expected classification")
    require(not (issues_by_phase["preparation"] & issues_by_phase["main"]), "preparation/main issue leakage")
    if complete:
        for phase, settings in SPEC["phases"].items():
            for difficulty in DIFFICULTIES:
                rows = [q for q in questions if q["phase"] == phase and q["difficulty"] == difficulty]
                require(len(rows) == settings["per_difficulty"], f"incomplete stratum {phase}/{difficulty}")
                if phase == "main":
                    for language in LANGUAGES:
                        require(sum(q["language"] == language for q in rows) == 5, "main stratum requires 5 TS + 5 Go")


def build_dataset(selection: Path, collection: Path, source: Path, output: Path) -> dict:
    require(command(["git", "rev-parse", "HEAD"], source).strip() == SPEC["source_commit"], "wrong source HEAD")
    require(not command(["git", "status", "--porcelain", "--untracked-files=no"], source).strip(), "modified source checkout")
    data = read_json(selection)
    candidates = {p["pr"]: p for p in read_json(collection / "candidates.json")["candidates"]}
    for q in data["questions"]:
        candidate = candidates.get(q["source"]["pr"])
        require(candidate is not None and candidate["eligible"], "question PR not eligible")
        folder = collection / candidate["provenance"]
        for name, checksum in candidate["artifacts"].items():
            require(file_digest(folder / name) == checksum, "PR provenance changed")
        provenance = output.parent / "provenance" / str(candidate["pr"])
        provenance.mkdir(parents=True, exist_ok=True)
        for name, checksum in candidate["artifacts"].items():
            target = provenance / name
            if target.exists():
                require(file_digest(target) == checksum, "saved selected-PR provenance drift")
            else:
                shutil.copyfile(folder / name, target)
        pr = read_json(folder / "pr.json")
        closing_issues = re.findall(r"(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s*:?\s*(?:https://github.com/grafana/grafana/(?:issues|pull)/|#)(\d+)",
                                   pr.get("body") or "", re.I)
        q["source"]["issue_keys"] = sorted(set(q["source"]["issue_keys"]) | {"grafana/grafana#" + key for key in closing_issues})
        q["source"].update({"url": candidate["url"], "merge_commit": candidate["merge_commit"],
                            "ancestor_verified": True, "artifacts": candidate["artifacts"],
                            "provenance": str(provenance.relative_to(output.parent))})
        for e in q["evidence"]:
            text = source_text(source, e["path"], e["start_line"], e["end_line"])
            if "text" in e:
                require(text == e["text"], "authored evidence differs from fixed code")
            e.update(text=text, text_sha256=digest(text))
    data.update(spec_sha256=digest(SPEC), source_commit=SPEC["source_commit"])
    validate_dataset(data, source)
    write_json(output, data, exclusive=True)
    return data
