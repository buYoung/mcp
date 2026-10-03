"""Frozen route rubric, blind packets and three-valued judgments. No model calls."""
from __future__ import annotations

import copy
import itertools
import re
import urllib.parse
from pathlib import Path

from . import grading_support as support, route_contract as routes
from .core import canonical, digest, file_digest, read_json, require, write_json

INSTRUCTIONS = """고정 Grafana 커밋에 대한 답변을 제공된 rubric대로 채점한다.
core와 extended 각 fact의 correct와 supported를 독립적으로 true, false, indeterminate로 판정한다.
인용 누락·잘못된 인용·불충분한 인용은 supported=false다. 평가자의 원문 접근 불능만 해당 차원의 보류 사유다.
answer_units와 citation_index는 실제 답변에서 추출했다. private_evidence는 정답 검증용이며 답변 인용을 대신하지 않는다.
인용 파싱과 source access 표시는 보조 정보다. 파싱 성공·실패나 특정 인용 형식만으로 supported를 확정하지 않는다.
답변 원문에 표현된 근거와 제공된 고정 소스를 대조해 LLM이 최종 판단한다. 파서가 놓친 인용은 실제 answer_unit_ids로 가리킬 수 있다.
citation_ids는 식별된 보조 항목이 있을 때만 사용한다. 원문을 확인할 수 없으면 해당 차원의 사유와 함께 indeterminate로 판단한다.
실제 답변에 등장하는 unit ID와 citation ID만 사용하고 해당 문구의 의미를 보존한다.
동등한 대체 인용은 허용한다. context_notes와 claim_review_notes를 새 필수 사실로 채점하지 않는다.
확장 오류는 해당 확장에 귀속하고 실제 핵심 모순은 영향받는 핵심 사실에도 개별 반영한다.
모든 false 또는 indeterminate에는 그 차원의 구체적 사유를 작성한다. 인용 문맥은 인용 범위를 확대하지 않는다.
외부 도구나 모델은 호출하지 않고 지정한 JSON schema로만 응답한다.
"""


def dimension(value) -> bool:
    return type(value) is bool or value == "indeterminate"


def fact_state(correct, supported) -> str:
    require(dimension(correct) and dimension(supported), "잘못된 route 판정 차원")
    if correct is False or supported is False:
        return "not_fulfilled"
    if correct == "indeterminate" or supported == "indeterminate":
        return "indeterminate"
    return "fulfilled"


def core_complete(states: list[str]):
    require(bool(states), "core facts 누락")
    if "not_fulfilled" in states:
        return False
    return "indeterminate" if "indeterminate" in states else True


def judgment_schema(question: dict, units: list[str] | None = None, citations: list[str] | None = None) -> dict:
    def obj(properties):
        return {"type": "object", "properties": properties, "required": list(properties), "additionalProperties": False}
    dim = obj({"value": {"anyOf": [{"type": "boolean"}, {"type": "string", "enum": ["indeterminate"]}]},
               "reason": {"type": "string"}})
    unit = {"type": "string", **({"enum": units} if units else {})}
    citation = {"type": "string", **({"enum": citations} if citations else {})}
    fact = obj({"fact_id": {"type": "string", "enum": [f["id"] for f in question["facts"]]},
                "correct": dim, "supported": copy.deepcopy(dim),
                "answer_unit_ids": {"type": "array", "items": unit},
                "citation_ids": {"type": "array", "items": citation}})
    return obj({"answer_id": {"type": "string"}, "question_id": {"type": "string", "enum": [question["id"]]},
                "facts": {"type": "array", "items": fact, "minItems": len(question["facts"]), "maxItems": len(question["facts"])},
                "extra_claim_diagnostics": {"type": "array", "items": obj({"answer_unit_ids": {"type": "array", "items": unit},
                                                                          "reason": {"type": "string"}})}})


REFERENCE = re.compile(r"(?P<path>[^\s`<>\[\]()\"',;]+?\.[A-Za-z0-9]+)(?::L?|#L)(?P<start>\d+)(?:[-–—]L?(?P<end>\d+))?")
IDENTITY_MENTION = re.compile(
    r"(?i)(?<![A-Za-z0-9_])(?:mcp[_:.-]+)?(?:codemap[-_\s]*search(?:_[A-Za-z0-9_]+)?|"
    r"codegraph(?:_[A-Za-z0-9_]+)?|zvec[-_\s]*grep(?:_[A-Za-z0-9_]+)?|"
    r"graphify(?:_[A-Za-z0-9_]+)?|codebase[-_\s]*memory(?:[-_\s]*mcp)?(?:_[A-Za-z0-9_]+)?|plain[-_\s]+rg|ripgrep|rg|"
    r"gpt[-_\s]*\d[A-Za-z0-9_.\-]*)(?![A-Za-z0-9_])")


def citation_index(answer: str) -> list[dict]:
    """Recognize explicit ranges only, including link labels with a wider stated range."""
    records, link_spans = [], []
    for link in re.finditer(r"\[([^\]\n]+)\]\(([^)]+)\)", answer):
        label, target = link.groups()
        hinted = REFERENCE.search(label)
        label_span = re.fullmatch(r"\s*L?(\d+)\s*[-–—]\s*L?(\d+)\s*", label.strip("` "))
        match = REFERENCE.search(target)
        if not match:
            target = target.strip("<>")
            if not (hinted or label_span) or not re.fullmatch(r"[^\s]+\.[A-Za-z0-9]+", target):
                continue
            row = {"path": target, "start": hinted["start"] if hinted else label_span[1],
                   "end": hinted["end"] if hinted else label_span[2]}
        else:
            row = match.groupdict()
        if hinted and (row["path"] == hinted["path"] or row["path"].endswith("/" + hinted["path"])):
            row.update(start=hinted["start"], end=hinted["end"])
        elif label_span:
            row.update(start=label_span[1], end=label_span[2])
        records.append({**row, "raw": link.group()})
        link_spans.append(link.span())
    for match in REFERENCE.finditer(answer):
        if not any(start <= match.start() < end for start, end in link_spans):
            records.append({**match.groupdict(), "raw": match.group()})
    result = []
    for row in records:
        path = urllib.parse.unquote(row["path"])
        prefix = f"https://github.com/grafana/grafana/blob/{routes.SOURCE_COMMIT}/"
        if path.startswith(prefix):
            path = path[len(prefix):]
        path = re.sub(r"^/frozen-grafana/source-\d+/", "", path).removeprefix("./")
        result.append({"id": f"c{len(result) + 1:03d}", "path": path, "start_line": int(row["start"]),
                       "end_line": int(row["end"] or row["start"]), "raw": row["raw"]})
    return result


def citation_sources(citations: list[dict], source: Path) -> list[dict]:
    corpus = read_json(routes.PREPARATION / "corpus.json")
    result = []
    for citation in citations:
        row = {**citation, "context_does_not_expand_answer_citation": True}
        relative = citation["path"]
        start, end = citation["start_line"], citation["end_line"]
        if relative not in corpus["files"] or not 1 <= start <= end:
            result.append({**row, "access": "invalid", "reason": "Citation path or range is not in the fixed source."})
            continue
        try:
            path = source / relative
            require(path.resolve().is_relative_to(source.resolve()), "citation symlink escapes fixed source")
            require(file_digest(path) == corpus["files"][relative]["sha256"], "fixed source bytes changed")
            lines = path.read_text().splitlines()
            if end > len(lines):
                result.append({**row, "access": "invalid", "reason": "Citation exceeds source line count."})
                continue
            low, high = max(1, start - 6), min(len(lines), end + 6)
            result.append({**row, "access": "available", "source_sha256": corpus["files"][relative]["sha256"],
                           "context_start_line": low, "context_end_line": high,
                           "numbered_source": "\n".join(f"{i}: {lines[i - 1]}" for i in range(low, high + 1))})
        except (OSError, UnicodeError, ValueError) as exc:
            # Exception text can contain an arm-specific absolute source path.
            result.append({**row, "access": "unavailable", "error_type": type(exc).__name__,
                           "reason": "The fixed-source excerpt could not be read or verified."})
    return result


def packet(question: dict, runs: list[dict], source: Path, experiment: Path, seed: int) -> tuple[dict, dict, list]:
    evidence = read_json(routes.DATA / "evidence.json")
    registry, answers, warnings = {}, [], []
    for run in runs:
        ident, masked, replacements, positions, flags = support.mask_answer(run, source, experiment, seed=seed)
        units = support.units(masked)
        citations = citation_index(masked)
        sources = citation_sources(citations, source)
        # Keep possible self-identification out of an automatically sealed blind packet.
        mentions = list(IDENTITY_MENTION.finditer(masked))
        if mentions:
            flags.append({"kind": "tool_or_model_identity", "spans": [[m.start(), m.end()] for m in mentions],
                          "mentions": [m.group() for m in mentions],
                          "policy": "Do not seal this answer until these exact spans are anonymized or verified as legitimate source identifiers; preserve the original answer."})
        require(ident not in registry, "중복 익명 답변")
        registry[ident] = {"run_id": run["id"], "question_id": question["id"], "answer": run["answer"],
                           "blinded_answer": masked, "blinding": replacements, "positions": positions,
                           "units": units, "citation_index": sources}
        answers.append({"answer_id": ident, "answer": masked,
                        "answer_units": [{"id": u["id"], "text": u["text"]} for u in units],
                        "citation_index": sources})
        if flags:
            warnings.append({"run_id": run["id"], "answer_sha256": digest(run["answer"]), "flags": flags})
    value = {"schema_version": 1, "question_id": question["id"], "prompt": question["prompt"],
             "source_commit": routes.SOURCE_COMMIT, "facts": question["facts"],
             "rubric": read_json(routes.DATA / "grading.json"),
             "private_evidence": {key: evidence["evidence"][key] for key in question["evidence_ids"]},
             "non_scoring_notes": {key: question.get(key, []) for key in ("context_notes", "claim_review_notes")},
             "wrong_claim_examples": question["wrong_claims"], "answers": answers,
             "citation_index_role": "advisory source navigation; the LLM judges the original answer against fixed-source evidence"}
    return value, registry, warnings


def validate_judgment(question: dict, judgment: dict, answer_id: str, record: dict) -> dict:
    require(set(judgment) == {"answer_id", "question_id", "facts", "extra_claim_diagnostics"}, "route judgment 필드 오류")
    require(judgment["answer_id"] == answer_id and judgment["question_id"] == question["id"], "다른 답변의 판정")
    expected = {f["id"]: f for f in question["facts"]}
    require(len(judgment["facts"]) == len(expected) and {f["fact_id"] for f in judgment["facts"]} == expected.keys(), "fact 판정 누락/중복")
    unit_ids = {u["id"] for u in record["units"]}
    citations = {c["id"]: c for c in record["citation_index"]}
    facts = []
    for fact in judgment["facts"]:
        require(set(fact) == {"fact_id", "correct", "supported", "answer_unit_ids", "citation_ids"}, "fact 필드 오류")
        require(set(fact["answer_unit_ids"]) <= unit_ids and set(fact["citation_ids"]) <= citations.keys(), "답변에 없는 unit/citation")
        for name in ("correct", "supported"):
            value = fact[name]
            require(set(value) == {"value", "reason"} and dimension(value["value"]) and isinstance(value["reason"], str), "차원 판정 형식 오류")
            require(value["value"] is True or bool(value["reason"].strip()), "false/indeterminate 차원 사유 누락")
        if fact["correct"]["value"] is True:
            require(bool(fact["answer_unit_ids"]), "correct=true에는 실제 답변 단위가 필요합니다")
        if fact["supported"]["value"] is True:
            require(bool(fact["answer_unit_ids"]), "supported=true에는 실제 답변 단위가 필요합니다")
        facts.append({**copy.deepcopy(fact), "level": expected[fact["fact_id"]]["level"],
                      "state": fact_state(fact["correct"]["value"], fact["supported"]["value"])})
    for diagnostic in judgment["extra_claim_diagnostics"]:
        require(set(diagnostic) == {"answer_unit_ids", "reason"} and set(diagnostic["answer_unit_ids"]) <= unit_ids
                and isinstance(diagnostic["reason"], str), "추가 주장 진단 형식 오류")
    return {**copy.deepcopy(judgment), "facts": facts,
            "core_complete": core_complete([f["state"] for f in facts if f["level"] == "core"])}


def prepare_packets(experiment: Path, dataset: dict, runs: list[dict], source: Path, spec: dict) -> list[Path]:
    """Prepare actual answers only; no calibration answers or fabricated scores."""
    batches = []
    for run in runs:
        require(run["status"] != "planned", "실행하지 않은 계획을 채점할 수 없습니다")
        if not run.get("conditions_valid") or run["status"] not in {"completed", "timeout", "call_limit", "token_limit"}:
            continue
        question = next(q for q in dataset["questions"] if q["id"] == run["question_id"])
        value, registry, flags = packet(question, [run], source, experiment, spec["seed"])
        if flags:
            write_json(experiment / "grading/review-required.json", flags)
            raise ValueError("비교군 식별 정보 검토가 필요합니다. grading/review-required.json의 정확한 구간을 확인하고 원답변을 보존하세요")
        prompt = INSTRUCTIONS + canonical(value)
        require(len(prompt.encode()) <= spec.get("batch_limits", {"bytes": 160 * 1024})["bytes"],
                "채점 패킷이 160 KiB를 초과합니다. rubric/근거를 자동 절단하지 않습니다")
        ident, record = next(iter(registry.items()))
        folder = experiment / "grading" / ident
        folder.mkdir(parents=True, exist_ok=True)
        write_json(folder / "packet.json", value)
        write_json(folder / "registry.json", registry)
        write_json(folder / "schema.json", judgment_schema(question, [u["id"] for u in record["units"]],
                                                           [c["id"] for c in record["citation_index"]]))
        (folder / "prompt.txt").write_text(prompt)
        batches.append(folder)
    return batches


def publish_grading() -> dict:
    dataset = routes.load_dataset()
    root = routes.PREPARATION / "grading-contract"
    schemas = {}
    packets = {}
    for question in dataset["questions"]:
        path = root / question["id"] / "schema.json"
        write_json(path, judgment_schema(question))
        schemas[question["id"]] = {"path": str(path), "sha256": file_digest(path)}
        value, registry, flags = packet(question, [], Path("."), root, routes.load_profile()["seed"])
        packet_path = path.with_name("packet-contract.json")
        write_json(packet_path, value)
        require(not registry and not flags and not value["answers"], "미실행 답변이 생성되었습니다")
        packets[question["id"]] = {"path": str(packet_path), "sha256": file_digest(packet_path), "answers": 0}
    table = [{"correct": c, "supported": s, "state": fact_state(c, s)}
             for c, s in itertools.product((True, False, "indeterminate"), repeat=2)]
    return routes.publish_handoff("05-grading.json", {
        "status": "verified_offline", "rubric_hash": file_digest(routes.DATA / "grading.json"),
        "judgment_schema": schemas, "packet_contract": packets, "fact_counts": {"core": 21, "extended": 8},
        "truth_table_evidence": table, "grader": routes.load_profile()["grader"],
        "citation_policy": "Parser and source-access labels are advisory; supported is the LLM's judgment of the original answer and fixed source.",
        "blinding_policy": "Tool/model self-identification blocks packet sealing; review-required retains exact spans outside grader input.",
        "calibration_runs": 0, "grader_runs": 0, "calibration_status": "not_executed",
        "limitations": ["No judge calibration or real answer grading has run.", "Alternative citations require the fixed full source inventory."]},
        implementation=["benchmark/route_grading.py", "benchmark/grading_support.py"],
        dependencies=["01-contract.json"])


if __name__ == "__main__":
    print(publish_grading()["status"])
