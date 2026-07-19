#!/usr/bin/env python3
"""Bind parser status and required raw artifacts into wrapper and run manifest."""
from __future__ import annotations

import pathlib
import sys

from common import load_json, write_json


REQUIRED = ("raw/events.jsonl", "raw/stderr.log", "wrapper.json", "normalized.json")


def main() -> int:
    if len(sys.argv) != 4:
        raise SystemExit("usage: record_postprocess.py RUN_DIR SUPERVISOR_STATUS PARSER_STATUS")
    run_dir = pathlib.Path(sys.argv[1]).resolve(); supervisor_status = int(sys.argv[2]); parser_status = int(sys.argv[3])
    status = {
        "supervisor_status": supervisor_status,
        "parser_status": parser_status,
        "parser_ok": parser_status == 0,
        "required_files": list(REQUIRED),
        "required_files_present": all((run_dir / relative).is_file() for relative in REQUIRED),
    }
    write_json(run_dir / "postprocess-status.json", status)
    wrapper = load_json(run_dir / "wrapper.json"); wrapper["postprocess"] = status; write_json(run_dir / "wrapper.json", wrapper)
    manifest = load_json(run_dir / "run.manifest.json"); manifest["postprocess"] = status; write_json(run_dir / "run.manifest.json", manifest)
    return 0


if __name__ == "__main__": raise SystemExit(main())
