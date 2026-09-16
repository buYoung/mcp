"""Read-only probes of existing front-end behavior; no implementation patches."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

parser = argparse.ArgumentParser()
parser.add_argument("--poc", type=Path, required=True)
parser.add_argument("--sources", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
poc = args.poc.resolve()
sys.path.insert(0, str(poc))
from syntax import load_program, child, walk
from engine import Analyzer, Interpreter
from rust_types import call_type_bindings
from rust_values import concrete_type_identity

spec = json.loads((poc / "language-corpus/remaining-routes/inputs.json").read_bytes())["variants"]["bevy-modules"]
program = load_program(args.sources, spec["paths"], module_bindings=spec["module_bindings"])
analyzer = Analyzer(program)

def function(path, name, line):
    return next(f for f in program.functions.values() if f.source.path == path and f.name == name and f.source.location(f.node).line == line)

observer = function("crates/bevy_ecs/src/observer/distributed_storage.rs", "new", 224)
example = function("examples/gltf/gltf_extension_mesh_2d.rs", "build", 70)
observer_interpreter = Interpreter(analyzer, observer.source, observer)
example_interpreter = Interpreter(analyzer, example.source, example)
some_paths = {"core::option::Option::Some", "std::option::Option::Some"}
box_paths = {"alloc::boxed::Box::new", "std::boxed::Box::new"}
prelude = {
    "observer_some_unqualified": observer_interpreter.rust_standard_path("Some", some_paths, "Some"),
    "observer_some_qualified": observer_interpreter.rust_standard_path("core::option::Option::Some", some_paths, "Some"),
    "observer_some_local_shadow": "Some" in observer_interpreter.local_bindings,
    "observer_some_module_shadow": "Some" in observer.source.module_bindings,
    "observer_some_import": program.import_paths.get((observer.source.path, "Some")),
    "observer_has_wildcard_use": any("*" in observer.source.text(n) for n in observer.source.tree.root_node.named_children if n.type == "use_declaration"),
    "example_box_unqualified": example_interpreter.rust_standard_path("Box::new", box_paths),
    "example_box_qualified": example_interpreter.rust_standard_path("alloc::boxed::Box::new", box_paths),
    "example_box_import": program.import_paths.get((example.source.path, "Box")),
}
generic = function(observer.source.path, "into_observer", 570)
generic_interpreter = Interpreter(analyzer, generic.source, generic)
call = next(n for n in walk(generic.body) if n.type == "call_expression" and generic.source.text(n) == "Observer::new(self)")
bindings = call_type_bindings(generic_interpreter, observer, child(call, "function"), [generic.receiver])
generic_owner = {"impl_receiver_owner": generic.owner, "declared_nominal_type": generic.owner in program.types.values(),
                 "listed_as_generic_nominal_type": generic.owner in program.generic_types, "inferred_callee_bindings": bindings,
                 "source_impl": generic.source.text(generic.node.parent.parent).split("{", 1)[0].strip()}
bound = Interpreter(analyzer, observer.source, observer, rust_type_bindings=bindings)
parameter = next(n for n in child(observer.node, "parameters").named_children if observer.source.text(child(n, "pattern")) == "system")
generic_owner["concrete_type_identity_accepts_formal_I_as"] = concrete_type_identity(bound, child(parameter, "type"))
registry = next(s for s in program.sources if s.path.endswith("/system/system_registry.rs"))
chained = [{"expression": registry.text(n), "start_byte": n.start_byte, "end_byte": n.end_byte,
            "location": vars(registry.location(n))}
           for n in walk(registry.tree.root_node) if n.type == "call_expression" and registry.location(n).line == 476]
missing_paths = ["crates/bevy_ecs/src/entity/mod.rs", "crates/bevy_ecs/src/entity/hash_map.rs", "crates/bevy_ecs/src/archetype.rs",
                 "crates/bevy_platform/src/collections/hash_map.rs", "crates/bevy_asset/src/server/loaders.rs"]
result = {"baseline_commit": "02426ebdbd66e6f90833dbd9b50b42d51ea1fe47", "prelude_resolution": prelude,
          "generic_owner_probe": generic_owner, "chained_call_nodes": chained,
          "input_membership": {path: path in spec["paths"] for path in missing_paths},
          "input_has_async_lock_sources": any("async-lock" in path or "async_lock" in path for path in spec["paths"]),
          "source_sha256": {s.path: hashlib.sha256(s.data).hexdigest() for s in program.sources},
          "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          "implementation_changed": False, "target_program_executed": False}
assert result["source_sha256"] == spec["source_sha256"]
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
print(json.dumps({key: value for key, value in result.items() if key != "source_sha256"}, ensure_ascii=False, indent=2))
