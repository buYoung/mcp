#!/usr/bin/env python3
"""Analyze one language/repository in an isolated process without case labels."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import platform
import resource
import sys
import time

DIRECTORY = Path(__file__).resolve().parent
sys.path.insert(0, str(DIRECTORY.parent))
from analyze import analyze as analyze_legacy, encode
from model import connect
from polyglot import Analyzer, Program, load_sources
from assembly import analyze_assembly

LEGACY = {"javascript", "typescript", "go", "rust"}


def analyze(root, paths, language, passes=4):
    if language in LEGACY:
        result = analyze_legacy(root, paths, passes)
        result["language"] = language
        result["engine"] = "legacy"
        return result
    started = time.perf_counter()
    sources, notices = load_sources(root, paths, language)
    program = Program(sources)
    notices.extend(program.notices)
    parsed = time.perf_counter()
    analyzer = Analyzer(program, passes)
    facts = analyze_assembly(sources) if language == "assembly" else analyzer.run()
    analyzed = time.perf_counter()
    relations, omitted = connect(facts, max_relations=4096)
    finished = time.perf_counter()
    if omitted:
        notices.append({"kind": "relation_output_cap", "omitted": omitted})
    notices.extend({"kind": kind, "detail": detail} for kind, detail in sorted(analyzer.notices))
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return {"contract": "conditional-storage-navigation-poc-v1", "language": language, "engine": "polyglot",
            "method": "bounded_source_summaries_without_event_api_catalog", "summary_passes": passes,
            "target_program_executed": False,
            "scope": {"paths": paths, "source_sha256": {source.path: hashlib.sha256(source.data).hexdigest() for source in sources}},
            "metrics": {"files": len(sources), "input_bytes": sum(len(source.data) for source in sources),
                        "functions": len(program.functions), "facts": len(facts), "relations": len(relations),
                        "parse_seconds": parsed - started, "analysis_seconds": analyzed - parsed,
                        "connect_seconds": finished - analyzed, "elapsed_seconds": finished - started,
                        "peak_rss_bytes_before_serialization": rss if platform.system() == "Darwin" else rss * 1024},
            "notices": notices, "facts": facts, "relations": relations,
            "semantic_limits": ["receiver schemas are conditional, not concrete instances",
                                "bounded summaries; full control flow, lifetime, dispatch and copy semantics are unproven",
                                "unmodeled expressions remain unresolved"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--path", action="append", required=True)
    parser.add_argument("--language", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--passes", type=int, choices=range(1, 5), default=4)
    options = parser.parse_args()
    result = analyze(options.root.resolve(), options.path, options.language, options.passes)
    implementation = [*DIRECTORY.glob("*.py"), *DIRECTORY.parent.glob("*.py")]
    result["implementation_sha256"] = {str(path.relative_to(DIRECTORY.parent)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(implementation)}
    result["environment"] = {"python": platform.python_version(), "system": platform.system(), "machine": platform.machine()}
    options.output.parent.mkdir(parents=True, exist_ok=True)
    payload = (json.dumps(result, default=encode, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    options.output.write_bytes(gzip.compress(payload, mtime=0) if options.output.suffix == ".gz" else payload)
    print(json.dumps({"language": options.language, **result["metrics"]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
