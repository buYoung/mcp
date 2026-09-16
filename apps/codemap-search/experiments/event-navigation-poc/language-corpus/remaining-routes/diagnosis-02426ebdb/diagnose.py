"""Bounded diagnosis only. No target execution or source-analyzer edits."""
import argparse
from collections import Counter, defaultdict
import gzip
import hashlib
import json
from pathlib import Path
import signal
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument("--poc", type=Path, required=True)
parser.add_argument("--sources", type=Path, required=True)
parser.add_argument("--variant", choices=["baseline", "global-budget", "function-budget"], required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
poc = args.poc.resolve()
sys.path.insert(0, str(poc))
import analyze
import engine
import model
from syntax import child
sys.path.insert(0, str(poc / "language-corpus"))
from manage import evaluate

manifest_path = poc / "language-corpus/remaining-routes/inputs.json"
spec = json.loads(manifest_path.read_bytes())["variants"]["bevy-modules"]
cases = [row for row in json.loads((poc / "language-corpus/cases.json").read_bytes())["cases"]
         if row["id"] in {"b_boxed_observer", "b_system_id", "b_gltf_handler"}]
caps = {"baseline": (192, 40000), "global-budget": (192, 160000), "function-budget": (768, 160000)}
engine.MAX_FACTS_PER_FUNCTION, engine.MAX_TOTAL_FACTS = caps[args.variant]
captured, samples, observations = {}, defaultdict(list), Counter()
seen, cap_stages = set(), []

watch_ranges = {
    "crates/bevy_ecs/src/system/system_registry.rs": [(33, 38), (456, 478), (682, 730)],
    "crates/bevy_ecs/src/observer/distributed_storage.rs": [(224, 241)],
    "crates/bevy_ecs/src/observer/runner.rs": [(35, 112)],
    "examples/gltf/gltf_extension_mesh_2d.rs": [(69, 85)],
    "crates/bevy_gltf/src/lib.rs": [(302, 310)],
    "crates/bevy_gltf/src/loader/mod.rs": [(259, 266), (1753, 1754)],
}
original_call = engine.Interpreter.call
original_cap = engine.Analyzer.cap_facts

def observed_call(self, node, level):
    returned = original_call(self, node, level)
    location = self.source.location(node)
    if any(begin <= location.line <= end for begin, end in watch_ranges.get(self.source.path, ())):
        key = f"{location.path}:{location.line}:{location.column}"
        observations[(key, returned.kind)] += 1
        record = {"location": vars(location), "expression": self.text(node), "function": self.identifier,
                  "depth": self.depth, "result": returned.display(), "rust_type_bindings": self.rust_type_bindings,
                  "locals": {name: self.env[name].display() for name in ("self", "system", "id", "state", "registered_system", "loader", "extensions") if name in self.env},
                  "some_shadow": {"local": "Some" in self.local_bindings, "module": "Some" in self.source.module_bindings}}
        encoded = json.dumps(record, sort_keys=True)
        if encoded not in seen:
            seen.add(encoded)
            if len(samples[key]) < 24:
                samples[key].append(record)
    return returned

def observed_cap(self, facts):
    result = original_cap(self, facts)
    cap_stages.append({"received": len(facts), "retained": len(result), "removed": len(facts) - len(result)})
    return result

def encode(data):
    return json.loads(json.dumps(data, default=analyze.encode))

def in_endpoint(fact, endpoint):
    return fact.location.path == endpoint[0] and endpoint[1] <= fact.location.line <= endpoint[2]

def diagnostic_connect(facts):
    normal, omitted = model.connect(facts)
    captured["endpoint_counts"] = {}
    captured["focused"] = {}
    for case in cases:
        storage = [fact for fact in facts if in_endpoint(fact, case["storage"])]
        invocation = [fact for fact in facts if in_endpoint(fact, case["invocation"])]
        focus = list(dict.fromkeys(storage + invocation))
        relations, skipped = model.connect(focus, max_relations=20000)
        captured["focused"][case["id"]] = {"relations": encode(relations), "omitted": skipped, "facts": len(focus)}
        captured["endpoint_counts"][case["id"]] = {
            "storage_fact_kinds": dict(Counter(f.kind for f in storage)),
            "invocation_fact_kinds": dict(Counter(f.kind for f in invocation)),
        }
    captured["default_omitted"] = omitted
    return normal, omitted

engine.Interpreter.call = observed_call
engine.Analyzer.cap_facts = observed_cap
analyze.connect = diagnostic_connect
started = time.monotonic()
def timeout(signum, frame):
    raise TimeoutError("diagnostic wall-time budget of 120 seconds exceeded")
signal.signal(signal.SIGALRM, timeout)
signal.alarm(120)
result = analyze.analyze(args.sources, spec["paths"], 4, spec["module_bindings"], spec.get("max_file_bytes", 524288))
signal.alarm(0)
encoded = encode(result)
assert encoded["scope"]["source_sha256"] == spec["source_sha256"]
normal = evaluate(cases, {("bevy", "rust"): encoded})
focused = {}
for case in cases:
    selected = captured["focused"][case["id"]]
    envelope = {**encoded, "relations": selected["relations"]}
    row = evaluate([case], {("bevy", "rust"): envelope})["cases"][0]
    focused[case["id"]] = {"status": row["status"], "matching_relations": row["matching_relations"],
                            "facts": selected["facts"], "relations": len(selected["relations"]), "omitted": selected["omitted"]}
base = {"baseline_commit": "02426ebdbd66e6f90833dbd9b50b42d51ea1fe47", "variant": args.variant,
        "fact_limit": engine.MAX_TOTAL_FACTS, "function_fact_limit": engine.MAX_FACTS_PER_FUNCTION,
        "summary_passes": 4, "source_callable_depth_limit": 6, "input_files": 62,
        "input_manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        "diagnostic_script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "implementation_sha256": {str(p.relative_to(poc)): hashlib.sha256(p.read_bytes()).hexdigest() for p in poc.glob("*.py")},
        "metrics": encoded["metrics"], "cap_stages": cap_stages,
        "default_relation_omitted": captured["default_omitted"], "endpoint_counts": captured["endpoint_counts"],
        "full_statuses": [{"id": row["id"], "status": row["status"], "matching_relations": row["matching_relations"]} for row in normal["cases"]],
        "focused_matching": focused, "notice_counts": dict(Counter(n["kind"] for n in encoded["notices"])),
        "target_program_executed": False, "production_defaults_changed": False,
        "case_labels_used_only_after_fact_extraction": True,
        "focused_matching_is_diagnostic_not_canonical_evaluation": True,
        "elapsed_seconds_including_observation": time.monotonic() - started}
args.output.mkdir(parents=True, exist_ok=True)
(args.output / (args.variant + ".json")).write_text(json.dumps(base, ensure_ascii=False, indent=2) + "\n")
details = {"samples": dict(samples), "call_observations": [{"location": key, "result_kind": kind, "count": count} for (key, kind), count in observations.items()],
           "sampling_limit_per_callsite": 24, "unique_observations": len(seen), "focused": captured["focused"]}
(args.output / (args.variant + ".details.json.gz")).write_bytes(gzip.compress(json.dumps(details, ensure_ascii=False).encode(), mtime=0))
print(json.dumps({key: base[key] for key in ["variant", "metrics", "cap_stages", "full_statuses", "focused_matching", "notice_counts"]}, ensure_ascii=False, indent=2))
