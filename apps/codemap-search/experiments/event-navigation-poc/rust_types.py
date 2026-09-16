"""Source-only Rust generic bindings and structurally distinct tuple map keys."""

import re

from model import MAP_ENTRIES, Value, slot, tuple_values
from syntax import child, element_type


def type_parameters(source, node):
    parameters = child(node, "type_parameters")
    return [source.text(child(part, "name")) for part in parameters.named_children
            if part.type == "type_parameter"] if parameters else []


def substitute_type_text(text, bindings):
    return re.sub(r"(?<![:\w])\w+", lambda match: bindings.get(match[0], match[0]), text)


def call_type_bindings(interpreter, function, callee_node, arguments):
    from rust_values import concrete_type_identity
    names = type_parameters(function.source, function.node)
    if not names:
        return {}
    bindings = {}
    explicit = child(callee_node, "type_arguments")
    supplied = [node for node in explicit.named_children if node.type != "lifetime"] if explicit else []
    if supplied and len(supplied) != len(names):
        return {}
    for name, node in zip(names, supplied):
        identity = concrete_type_identity(interpreter, node)
        if identity:
            bindings[name] = identity
    for (_, declaration), actual in zip(function.parameters, arguments):
        name = re.sub(r"^&(?:'\w+\s*)?(?:mut\s*)?", "", declaration.strip())
        if name not in names:
            continue
        identity = interpreter.analyzer.owner(actual, interpreter.source)
        if not identity:
            continue
        # The owner alone omits type/const arguments. Do not infer Holder<u32>
        # and Holder<String> as the same T, even when fields look identical.
        if identity in interpreter.program.generic_types or "<" in interpreter.analyzer.type_text(actual, interpreter.source):
            continue
        if name in bindings and bindings[name] != identity:
            interpreter.analyzer.notices.add(("rust_generic_argument_conflict", function.identifier))
            return {}
        bindings[name] = identity
    return bindings


def map_entry(interpreter, receiver, key):
    """Keep tuple arity and each key component, including concrete TypeId keys."""
    def append(base, value):
        if value.kind in {"tuple", "tuple_end"}:
            values = tuple_values(value)
            base = slot(base, Value("container", "tuple_key:" + str(len(values))))
            for item in values:
                base = append(base, item)
            return base
        return slot(base, value)
    target = append(slot(receiver, MAP_ENTRIES), key)
    declared = element_type(interpreter.analyzer.type_text(receiver, interpreter.source))
    if declared:
        interpreter.analyzer.value_types[target] = declared
    return target
