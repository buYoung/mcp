#!/usr/bin/env python3
"""Independent static PoC: source -> syntax -> summaries -> conditional relations."""

import argparse
from dataclasses import asdict, is_dataclass
import hashlib
import gzip
import json
from pathlib import Path
import platform
import resource
import subprocess
import time

from engine import Analyzer
from model import Fact, Value, connect
from syntax import load_program


def encode(value):
    if isinstance(value, Value):
        return value.display()
    if isinstance(value, Fact):
        result = {"kind": value.kind, "target": value.target.display(), "value": value.value.display(),
                "location": asdict(value.location), "function": value.function,
                "conditions": value.conditions, "via": [asdict(item) for item in value.via],
                "argument_index": value.argument_index}
        if value.consumer is not None:
            result["consumer"] = {"location": asdict(value.consumer.location), "target": value.consumer.target.display(),
                                  "kind": value.consumer.kind, "argument_index": value.consumer.argument_index}
        return result
    if is_dataclass(value):
        return asdict(value)
    raise TypeError(type(value).__name__)


def analyze(root: Path, paths: list[str], passes=None):
    started = time.perf_counter()
    program = load_program(root, paths)
    parsed = time.perf_counter()
    analyzer = Analyzer(program)
    if passes is not None:
        import engine
        engine.MAX_SUMMARY_PASSES = passes
    facts = analyzer.run()
    analyzed = time.perf_counter()
    relations, omitted = connect(facts)
    connected = time.perf_counter()
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    rss_bytes = rss if platform.system() == "Darwin" else rss * 1024
    source_hashes = {source.path: hashlib.sha256(source.data).hexdigest() for source in program.sources}
    implementation = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                      for path in sorted(Path(__file__).parent.glob("*.py"))}
    notices = program.notices + [{"kind": kind, "detail": detail} for kind, detail in sorted(analyzer.notices)]
    if omitted:
        notices.append({"kind": "relation_output_cap", "omitted": omitted})
    return {
        "contract": "conditional-storage-navigation-poc-v1",
        "method": "source_summaries_without_event_api_catalog",
        "summary_passes": passes or 4,
        "target_program_executed": False,
        "scope": {"paths": paths, "source_sha256": source_hashes},
        "implementation_sha256": implementation,
        "environment": {"python": platform.python_version(), "system": platform.system(), "machine": platform.machine()},
        "metrics": {"files": len(program.sources), "input_bytes": sum(len(source.data) for source in program.sources),
                    "functions": len(program.functions), "facts": len(facts), "relations": len(relations),
                    "parse_seconds": parsed - started, "analysis_seconds": analyzed - parsed,
                    "connect_seconds": connected - analyzed, "elapsed_seconds": connected - started,
                    "peak_rss_bytes_before_serialization": rss_bytes},
        "notices": notices,
        "relations": relations,
        "facts": facts,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--path", action="append", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--passes", type=int, choices=range(1, 5))
    options = parser.parse_args()
    result = analyze(options.root.resolve(), options.path, options.passes)
    result["revision"] = subprocess.check_output(["git", "-C", str(options.root), "rev-parse", "HEAD"], text=True).strip()
    options.output.parent.mkdir(parents=True, exist_ok=True)
    serialized = (json.dumps(result, default=encode, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    payload = gzip.compress(serialized, mtime=0) if options.output.suffix == ".gz" else serialized
    options.output.write_bytes(payload)
    navigation_bytes = len(json.dumps(result["relations"], default=encode, ensure_ascii=False, separators=(",", ":")).encode())
    print(json.dumps({"output": str(options.output), "output_bytes": len(serialized), "stored_bytes": len(payload),
                      "relation_output_bytes": navigation_bytes,
                      **result["metrics"], "notices": len(result["notices"])}, ensure_ascii=False))


if __name__ == "__main__":
    main()
