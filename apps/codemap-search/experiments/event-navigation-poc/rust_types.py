"""Source-only Rust generic bindings and structurally distinct tuple map keys."""

import re
from dataclasses import dataclass

from model import MAP_ENTRIES, Value, slot, tuple_values
from syntax import child, element_type


@dataclass(frozen=True)
class RustType:
    kind: str
    name: str = ""
    arguments: tuple["RustType", ...] = ()
    scope: str = ""

    @property
    def is_concrete(self):
        return self.kind in {"nominal", "primitive", "application", "tuple", "reference", "pointer"} and all(part.is_concrete for part in self.arguments)

    def display(self):
        if self.kind == "variable":
            return self.name + "@{" + self.scope + "}"
        if self.kind == "application":
            return self.name + "<" + ",".join(part.display() for part in self.arguments) + ">"
        if self.kind == "tuple":
            return "(" + ",".join(part.display() for part in self.arguments) + ("," if len(self.arguments) == 1 else "") + ")"
        if self.kind in {"reference", "pointer", "projection"}:
            return self.name + "(" + ",".join(part.display() for part in self.arguments) + ")"
        return self.name


def scoped_parameters(source, node):
    """Nearest function/impl declaration wins; unrelated T names stay distinct."""
    result = {}
    while node is not None:
        parameters = child(node, "type_parameters")
        for parameter in parameters.named_children if parameters else ():
            if parameter.type not in {"type_parameter", "const_parameter"}:
                continue
            name = source.text(child(parameter, "name"))
            result.setdefault(name, RustType("variable", name, scope=f"{source.path}:{node.start_byte}-{node.end_byte}"))
        node = node.parent
    return result


def type_expression(interpreter, node):
    if node is None:
        return RustType("unresolved", "<missing>")
    text = interpreter.text(node)
    if node.type == "generic_type":
        base = type_expression(interpreter, child(node, "type"))
        args = child(node, "type_arguments")
        arguments = tuple(type_expression(interpreter, part) for part in args.named_children if part.type != "lifetime") if args else ()
        kind = "application" if base.kind == "nominal" else "unresolved_application"
        return RustType(kind, base.name, arguments, base.scope)
    if node.type in {"tuple_type", "unit_type"}:
        return RustType("tuple", arguments=tuple(type_expression(interpreter, part) for part in node.named_children))
    if node.type in {"reference_type", "pointer_type"}:
        if node.type == "reference_type":
            qualifier = "&mut" if any(part.type in {"mutable_specifier", "mut"} for part in node.children) else "&"
        else:
            qualifier = "*mut" if re.match(r"\*\s*mut\b", text) else "*const"
        return RustType("reference" if node.type == "reference_type" else "pointer",
                        qualifier,
                        (type_expression(interpreter, child(node, "type")),))
    if node.type not in {"primitive_type", "type_identifier", "scoped_type_identifier"}:
        return RustType("unresolved", text)
    bound = interpreter.rust_type_bindings.get(text)
    if bound:
        return bound if isinstance(bound, RustType) else RustType("nominal", bound)
    params = scoped_parameters(interpreter.source, interpreter.function.node if interpreter.function else None)
    if text in params:
        return params[text]
    if text == "Self" and interpreter.function:
        ancestor = interpreter.function.node.parent
        while ancestor is not None and ancestor.type != "impl_item":
            ancestor = ancestor.parent
        target = child(ancestor, "type")
        if target is not None and interpreter.text(target) != "Self":
            return type_expression(interpreter, target)
    if "::" in text and text.split("::", 1)[0] in params:
        prefix, member = text.split("::", 1)
        base = interpreter.rust_type_bindings.get(prefix, params[prefix])
        if isinstance(base, str):
            base = RustType("nominal", base)
        return RustType("projection", member, (base,))
    owner = interpreter.program.resolve_type(interpreter.source, text)
    if owner:
        return RustType("nominal", owner)
    primitive = text.rsplit("::", 1)[-1]
    if re.fullmatch(r"(?:[ui](?:8|16|32|64|128|size)|f(?:32|64)|bool|char|str)", primitive):
        if (text == primitive and text not in interpreter.source.module_bindings
                or interpreter.rust_standard_path(text, {f"{root}::primitive::{primitive}" for root in ("std", "core")})):
            return RustType("primitive", "core::primitive::" + primitive)
    return RustType("unresolved", text)


def type_parameters(source, node):
    parameters = child(node, "type_parameters")
    return [source.text(child(part, "name")) for part in parameters.named_children
            if part.type == "type_parameter"] if parameters else []


def substitute_type_text(text, bindings):
    def replacement(match):
        value = bindings.get(match[0], match[0])
        return value.display() if isinstance(value, RustType) else value
    return re.sub(r"(?<![:\w])\w+", replacement, text)


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
        if not identity or identity not in interpreter.program.types.values():
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
