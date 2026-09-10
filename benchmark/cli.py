"""JSON-lines adapter for the interactive Node entry point."""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

from . import execution, settings
from .v2.core import ContractError, require


def emit(value):
    print(json.dumps({"event": "progress", "message": value}, ensure_ascii=False), flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="bench:start 내부 JSON 어댑터")
    parser.add_argument("action", choices=["catalog", "prepare", "start", "resume"])
    args = parser.parse_args()
    try:
        request = json.load(sys.stdin)
        if args.action == "catalog":
            result = execution.catalog()
        elif args.action == "prepare":
            result = execution.prepare_plan(request["profile"], request["targets"], emit)
        else:
            require(request.get("confirmed") is True, "실행 확인이 필요합니다")
            if args.action == "start":
                root = execution.create_run(request["plan_id"], request.get("reuse"))
            else:
                ident = request["run_id"]
                require(isinstance(ident, str) and re.fullmatch(r"[a-zA-Z0-9_-]+", ident), "실행 ID 오류")
                root = settings.RUNS / ident
                require((root / "manifest.json").is_file(), "저장된 실행이 없습니다")
            emit(f"실행 원자료: {root}")
            result = execution.execute(root, emit)
        print(json.dumps({"event": "result", "value": result}, ensure_ascii=False), flush=True)
        return 130 if result.get("status") == "interrupted" else 1 if result.get("status") == "finished_with_errors" else 0
    except (ContractError, OSError, KeyError, TypeError, ValueError, subprocess.SubprocessError) as exc:
        print(json.dumps({"event": "error", "message": str(exc)}, ensure_ascii=False), flush=True)
        return 1
    except KeyboardInterrupt:
        emit("취소했습니다. 완료된 결과와 준비 로그를 보존합니다.")
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
