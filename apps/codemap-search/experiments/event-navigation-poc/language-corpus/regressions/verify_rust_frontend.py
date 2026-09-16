"""Check observable operand/type preservation in the Rust frontier fixture."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import sys

POC = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(POC))
from engine import Analyzer, Interpreter
from syntax import load_program


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--analysis", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    data = json.loads(gzip.decompress(args.analysis.read_bytes()))
    calls = data["unresolved_calls"]
    direct = [call for call in calls if not call["via"]]
    factories = [call for call in direct if call["callee"].endswith(":factory")]
    assert len(factories) == 1
    factory = factories[0]
    assert len(factory["arguments"]) == 10
    assert [value.split(":", 1)[0] for value in factory["arguments"][:4]] == ["allocation", "wrapper", "tuple(parameter", "parameter"]
    ids = [call for call in direct if call["callee"] == factory["result"] + "[key:id]"]
    assert len(ids) == 1 and ids[0]["result"] != factory["result"]
    assert ids[0]["location"] == factory["location"]
    variables = [call["generic_types"][0] for call in direct if call["callee"].endswith(":opaque")]
    assert len(variables) == 2 and all(value["kind"] == "variable" and value["name"] == "T" for value in variables)
    assert len({value["scope"] for value in variables}) == 2
    inherited = [call for call in calls if call["callee"].endswith(":missing") and call["via"]]
    assert len({call["result"] for call in inherited}) == 2
    assert {call["via"][-1]["line"] for call in inherited} == {58, 59}
    assert all(call["arguments"] for call in inherited)
    program = load_program(Path(__file__).parent / "examples/rust/source-frontier", ["routes.rs"])
    analyzer = Analyzer(program)
    generic = next(function for function in program.functions.values() if function.name == "distinct_generic")
    owner = program.types[("routes.rs", "Holder")]
    specialized = Interpreter(analyzer, generic.source, generic, rust_type_bindings={"T": owner})
    assert analyzer.owner(specialized.env["value"], generic.source) == ""
    unbound = Interpreter(analyzer, generic.source, generic)
    assert analyzer.owner(unbound.env["value"], generic.source) == ""
    generic_impl = next(function for function in program.functions.values() if function.name == "convert")
    assert generic_impl.owner != program.types[("routes.rs", "T")]
    result = {"checks": ["all_ten_operands_preserved", "nested_calls_same_start_distinct_results",
                         "impl_and_function_type_variables_have_distinct_scopes", "source_call_contexts_remain_distinct",
                         "generic_formal_types_do_not_leak_between_calls"],
              "passed": True, "target_program_executed": False,
              "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "analysis_sha256": hashlib.sha256(args.analysis.read_bytes()).hexdigest(),
              "implementation_sha256": data["implementation_sha256"]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
