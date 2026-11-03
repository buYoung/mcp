"""JSON-lines operator adapter; run is the only new-campaign launch command."""
from __future__ import annotations

import argparse
import json
import signal
import subprocess
import sys
import threading
from pathlib import Path

from . import execution
from .core import ContractError, require


def emit(value):
    print(json.dumps({"event": "progress", "message": value}, ensure_ascii=False), flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="bench:start JSON 어댑터; run은 명시적 승인 필수")
    parser.add_argument("action", choices=["catalog", "prepare", "run", "resume", "approve-review", "grade-sealed", "report", "inspect-operator"])
    parser.add_argument("--solver-model")
    args = parser.parse_args()
    cancel_event = threading.Event()
    previous = {}
    try:
        request = json.load(sys.stdin)
        if args.action in {"run", "resume", "grade-sealed"}:
            for sig in (signal.SIGINT, signal.SIGTERM):
                previous[sig] = signal.signal(sig, lambda *_: cancel_event.set())
        if args.action == "catalog":
            result = execution.catalog()
        elif args.action == "prepare":
            result = execution.prepare_plan(request["profile"], request["targets"], emit,
                                            solver_model=args.solver_model or request.get("solver_model"))
        elif args.action == "run":
            result = execution.run_new(request, cancel_event=cancel_event)
        elif args.action == "resume":
            require(request.get("confirmed") is True, "resume은 기존 승인 범위의 명시적 재개 확인이 필요합니다")
            result = execution.run_campaign(Path(request["run_root"]), resume=True,
                retry_unstarted=request.get("retry_unstarted") is True, cancel_event=cancel_event)
        elif args.action == "grade-sealed":
            require(request.get("confirmed") is True, "grade-sealed requires explicit confirmation within the existing authorization")
            result = execution.grade_sealed(Path(request["run_root"]), cancel_event=cancel_event)
        elif args.action == "approve-review":
            result = execution.approve_review(Path(request["run_root"]), request["slot_id"], request["approval"])
        elif args.action == "report":
            result = execution.report_campaign(Path(request["run_root"]))
        else:
            from .campaign_store import campaign_lease
            with campaign_lease():
                result = execution.operator_inspection(args.solver_model or request.get("solver_model"))
        print(json.dumps({"event": "result", "value": result}, ensure_ascii=False), flush=True)
        return 130 if cancel_event.is_set() else 1 if result.get("status") == "blocked" else 0
    except (ContractError, OSError, KeyError, TypeError, ValueError, subprocess.SubprocessError) as exc:
        print(json.dumps({"event": "error", "message": str(exc)}, ensure_ascii=False), flush=True)
        return 1
    except KeyboardInterrupt:
        emit("취소했습니다. 완료된 결과와 준비 로그를 보존합니다.")
        return 130
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == "__main__":
    raise SystemExit(main())
