"""JSON-lines adapter for the interactive Node entry point."""
from __future__ import annotations

import argparse
import json
import subprocess
import sys

from . import execution
from .core import ContractError


def emit(value):
    print(json.dumps({"event": "progress", "message": value}, ensure_ascii=False), flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="bench:start 내부 JSON 어댑터")
    parser.add_argument("action", choices=["catalog", "prepare"])
    args = parser.parse_args()
    try:
        request = json.load(sys.stdin)
        if args.action == "catalog":
            result = execution.catalog()
        elif args.action == "prepare":
            result = execution.prepare_plan(request["profile"], request["targets"], emit)
        print(json.dumps({"event": "result", "value": result}, ensure_ascii=False), flush=True)
        return 0
    except (ContractError, OSError, KeyError, TypeError, ValueError, subprocess.SubprocessError) as exc:
        print(json.dumps({"event": "error", "message": str(exc)}, ensure_ascii=False), flush=True)
        return 1
    except KeyboardInterrupt:
        emit("취소했습니다. 완료된 결과와 준비 로그를 보존합니다.")
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
