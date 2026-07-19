#!/usr/bin/env python3
"""Substitute only validated absolute paths into the fixed sandbox profile."""
from __future__ import annotations

import pathlib
import re
import sys

from common import HARNESS_ROOT


def safe_path(value: str) -> str:
    path = pathlib.Path(value).resolve()
    if not path.is_absolute() or '"' in str(path):
        raise ValueError("unsafe sandbox path")
    return str(path)


def main() -> int:
    if len(sys.argv) != 15:
        raise SystemExit(
            "usage: materialize_sandbox.py OUTPUT BENCHMARK LEGACY PRODUCT B2_SOURCE "
            "SOURCE WORKING_CODEMAP HOST_CONFIG HOST_DATA QUALITY_ROOT RUN_ROOT "
            "OPENCODE_BINARY B2_BINARY AUTH_RUNTIME"
        )
    (
        output, benchmark, legacy, product, b2_source, source, codemap, host_config, host_data,
        quality_root, run_root, opencode_binary, b2_binary, auth_runtime,
    ) = sys.argv[1:]
    replacements = {
        "__BENCHMARK_ROOT__": safe_path(benchmark),
        "__LEGACY_ROOT__": safe_path(legacy),
        "__PRODUCT_REPOSITORY_ROOT__": safe_path(product),
        "__B2_SOURCE_ROOT__": safe_path(b2_source),
        "__SOURCE_ROOT__": safe_path(source),
        "__WORKING_CODEMAP__": safe_path(codemap),
        "__HOST_OPENCODE_CONFIG__": safe_path(host_config),
        "__HOST_OPENCODE_DATA__": safe_path(host_data),
        "__QUALITY_ROOT__": safe_path(quality_root),
        "__RUN_ROOT__": safe_path(run_root),
        "__OPENCODE_BINARY__": safe_path(opencode_binary),
        "__B2_BINARY__": safe_path(b2_binary),
        "__AUTH_RUNTIME__": safe_path(auth_runtime),
    }
    profile = (HARNESS_ROOT / "templates/sandbox-profile.sb").read_text(encoding="utf-8")
    for marker, value in replacements.items():
        if profile.count(marker) < 1:
            raise SystemExit(f"sandbox marker is absent: {marker}")
        profile = profile.replace(marker, value)
    if re.search(r"__[A-Z][A-Z0-9_]+__", profile):
        raise SystemExit("unresolved sandbox marker")
    pathlib.Path(output).write_text(profile, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
