"""Shared JSON, identity and command primitives for the 1.0.0 benchmark."""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path
from typing import Any


class ContractError(ValueError):
    """A saved input or observation cannot satisfy the declared contract."""



def canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)



def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value).encode()).hexdigest()



def file_digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()



def read_json(path: Path) -> Any:
    def reject_duplicates(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ContractError(f"duplicate JSON key: {key} ({path})")
            result[key] = value
        return result
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicates,
                      parse_constant=lambda value: (_ for _ in ()).throw(ContractError(value)))



def write_json(path: Path, value: Any, *, exclusive: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False) + "\n"
    if exclusive:
        with path.open("x", encoding="utf-8") as stream:
            stream.write(text)
    else:
        temporary = path.with_name(path.name + f".{os.getpid()}.tmp")
        temporary.write_text(text, encoding="utf-8")
        temporary.replace(path)



def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    rows = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if line.strip():
            try:
                rows.append(json.loads(line))
            except ValueError as exc:
                raise ContractError(f"invalid JSONL {path}:{number}") from exc
    return rows



def append_jsonl(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as stream:
        stream.write(canonical(value) + "\n")
        stream.flush()



def metric(value: float | int | None, unit: str, numerator=None, denominator=None,
           reason: str | None = None, evidence: list[str] | None = None) -> dict:
    if value is None and not reason:
        raise ContractError("null metric requires a reason")
    if numerator is None and denominator is None and value is not None:
        numerator, denominator = value, 1
    return {"value": value, "unit": unit, "numerator": numerator, "denominator": denominator,
            "validity": "valid" if value is not None else "unavailable",
            "missing_reason": reason, "evidence": evidence or []}



def ratio(numerator, denominator, unit="ratio", evidence=None) -> dict:
    if numerator is None or denominator is None:
        return metric(None, unit, numerator, denominator, "missing operand", evidence)
    if denominator == 0:
        return metric(None, unit, numerator, denominator, "zero denominator", evidence)
    return metric(numerator / denominator, unit, numerator, denominator, evidence=evidence)



def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)



def command(args: list[str], cwd: Path | None = None, timeout_seconds=120) -> str:
    result = subprocess.run(args, cwd=cwd, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=timeout_seconds)
    require(result.returncode == 0, f"command failed ({args[0]}): {result.stderr[-2000:]}")
    return result.stdout

