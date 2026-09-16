"""Observe Bevy source values without adding any rules to the analyzer."""
import argparse
from dataclasses import asdict
import hashlib
import json
from pathlib import Path
import signal
import sys

DIRECTORY = Path(__file__).resolve().parent
POC = DIRECTORY.parent.parent
sys.path.insert(0, str(POC))
import analyze
from engine import Analyzer, Interpreter
from model import split_path
from rust_types import call_type_bindings
from rust_values import concrete_type_identity
from syntax import child, load_program, walk
sys.path.insert(0, str(DIRECTORY.parent))
from manage import evaluate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    spec = json.loads((DIRECTORY / "inputs.json").read_bytes())["variants"]["bevy-modules"]
    observed = {}
    original = Interpreter.call

    def observe(interpreter, node, level):
        value = original(interpreter, node, level)
        location = interpreter.source.location(node)
        if interpreter.depth == 0 and (
                location.path.endswith("/system/system_registry.rs") and location.line in {476, 477, 682, 684, 689, 704, 725}
                or location.path.endswith("/observer/distributed_storage.rs") and location.line == 227):
            observed[(location.path, node.start_byte, node.end_byte)] = {
                "location": asdict(location), "expression": interpreter.text(node),
                "result": value.display(), "function": interpreter.identifier,
                "root_kind": split_path(value)[0].kind,
            }
        return value

    def timeout(signum, frame):
        raise TimeoutError("Bevy frontier diagnosis exceeded 180 seconds")

    signal.signal(signal.SIGALRM, timeout)
    signal.alarm(180)
    Interpreter.call = observe
    try:
        result = analyze.analyze(args.sources, spec["paths"], module_bindings=spec["module_bindings"])
    finally:
        Interpreter.call = original
        signal.alarm(0)
    assert result["scope"]["source_sha256"] == spec["source_sha256"]
    rows = list(observed.values())
    id_result = next(row for row in rows if row["expression"] == "self.spawn(RegisteredSystem::new(system)).id()")
    assert id_result["result"].endswith("[key:entity]") and id_result["root_kind"] == "allocation"
    assert any(row["expression"] == "Some(system)" and row["result"].startswith("wrapper:rust:option(allocation:") for row in rows)

    program = load_program(args.sources, spec["paths"], module_bindings=spec["module_bindings"])
    analyzer = Analyzer(program)
    constructor = next(f for f in program.functions.values() if f.name == "new" and f.source.path.endswith("/observer/distributed_storage.rs") and f.source.location(f.node).line == 224)
    conversion = next(f for f in program.functions.values() if f.name == "into_observer" and f.source.path == constructor.source.path and f.source.location(f.node).line == 570)
    interpreter = Interpreter(analyzer, conversion.source, conversion)
    call = next(node for node in walk(conversion.body) if node.type == "call_expression" and conversion.source.text(node) == "Observer::new(self)")
    bindings = call_type_bindings(interpreter, constructor, child(call, "function"), [conversion.receiver])
    bound = Interpreter(analyzer, constructor.source, constructor, rust_type_bindings=bindings)
    parameter = next(node for node in child(constructor.node, "parameters").named_children if constructor.source.text(child(node, "pattern")) == "system")
    assert concrete_type_identity(bound, child(parameter, "type")) is None
    labels = json.loads((DIRECTORY.parent / "cases.json").read_bytes())["cases"]
    selected = [case for case in labels if case["repository"] == "bevy" and case["kind"] == "positive"]
    encoded = json.loads(json.dumps(result, default=analyze.encode))
    evaluation = evaluate(selected, {("bevy", "rust"): encoded})
    public = [{"id": row["id"], "status": row["status"], "matching_relations": row["matching_relations"]} for row in evaluation["cases"]]
    missing = [call for call in encoded["unresolved_calls"] if not call["via"] and call["location"]["path"].endswith("/system/system_registry.rs") and 673 <= call["location"]["line"] <= 738]
    output = {"frontend_checks_passed": True, "public_pairs": public,
              "observed_calls": rows, "remaining_registry_calls": missing,
              "generic_impl_variable_is_concrete": False, "metrics": result["metrics"],
              "source_sha256": spec["source_sha256"], "revision": spec["revision"],
              "implementation_sha256": result["implementation_sha256"],
              "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "target_program_executed": False, "compiler_auxiliary_input_used": False}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"frontend_checks_passed": True, "public_pairs": public, "observed_calls": len(rows)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
