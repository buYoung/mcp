"""Source-selected Scala companion implicits, anonymous bodies and quote templates.

This is a bounded subset, not compiler implicit search. Ambiguous providers and
unavailable context-bound evidence never select a concrete implementation.
"""

import re
from model import UNKNOWN, Value


def children(node):
    return list(node.named_children) if node is not None else []


def field(node, name):
    return node.child_by_field_name(name) if node is not None else None


def nodes(node):
    if node is not None:
        yield node
        for part in children(node):
            yield from nodes(part)


def type_parts(text):
    text = text.strip()
    if "[" not in text or not text.endswith("]"):
        return text, []
    name, rest = text.split("[", 1)
    depth, start, arguments = 0, 0, []
    for index, char in enumerate(rest[:-1]):
        depth += (char == "[") - (char == "]")
        if char == "," and depth == 0:
            arguments.append(rest[start:index].strip())
            start = index + 1
    arguments.append(rest[start:-1].strip())
    return name, arguments


class ScalaSemantics:
    def __init__(self, program):
        self.program = program
        self.bases = {}
        self.anonymous = {}
        self.singletons = set()
        self.implicit_names = {}
        self.providers = []
        self.declarations = {}
        self.package_members = {}
        class_kinds = {"class_definition", "object_definition", "trait_definition"}
        for source in program.sources:
            if source.language != "scala":
                continue
            for node in nodes(source.tree.root_node):
                if node.type in class_kinds:
                    name = source.text(field(node, "name"))
                    owner = program.owner(source, name)
                    self.declarations[owner] = (source, node)
                    parents = []
                    ancestor = node.parent
                    while ancestor is not None:
                        if ancestor.type in class_kinds:
                            parents.append(source.text(field(ancestor, "name")))
                        ancestor = ancestor.parent
                    if parents:
                        program.types[(source.path, ".".join(reversed(parents)) + "." + name)] = owner
                    if node.type == "object_definition":
                        self.singletons.add(owner)
                if node.type == "instance_expression":
                    body = next((part for part in children(node) if part.type == "template_body"), None)
                    if body is not None:
                        owner = program.owner(source, "anonymous@" + str(node.start_byte))
                        self.anonymous[(source.path, node.start_byte)] = owner
                        program.owner_sources[owner] = source
                        program.class_parameters[owner] = []
                        program.collect_body(source, body, owner)
        for owner, (source, node) in self.declarations.items():
            extend = field(node, "extend")
            if extend is not None:
                names = [source.text(part) for part in children(extend)
                         if part.type in {"type_identifier", "stable_type_identifier", "generic_type"}]
                self.bases[owner] = [base for name in names if (base := program.declared_owner(source, name))]
        for function in program.functions:
            if function.source.language != "scala":
                continue
            source = function.source
            ancestor = function.node.parent
            while ancestor is not None:
                if ancestor.type == "package_object":
                    namespace = program.namespaces[source.path] + "." + source.text(field(ancestor, "name"))
                    self.package_members[(namespace, function.name)] = function
                    break
                ancestor = ancestor.parent
            implicit_names = set()
            for group in children(function.node):
                if group.type == "parameters" and re.match(r"\(\s*(?:implicit|using)\b", source.text(group)):
                    implicit_names.update(source.text(field(parameter, "name")) for parameter in children(group))
            self.implicit_names[function.identifier] = implicit_names
            modifiers = " ".join(source.text(part) for part in children(function.node) if part.type == "modifiers")
            if "implicit" in modifiers.split() and not function.parameters:
                self.providers.append(function)

    def ancestors(self, owner):
        result, pending = [], [owner]
        while pending and len(result) < 32:
            item = pending.pop(0)
            if item in result:
                continue
            result.append(item)
            pending.extend(self.bases.get(item, ()))
        return result

    def methods(self, owner, name):
        for ancestor in self.ancestors(owner):
            methods = self.program.methods.get((ancestor, name), [])
            if methods:
                return methods
        return []

    def bind_implicit_arguments(self, interpreter, function, arguments, call_node):
        implicit = self.implicit_names.get(function.identifier, set())
        if not implicit or len(arguments) == len(function.parameters):
            return arguments
        explicit = [parameter for parameter in function.parameters if parameter[0] not in implicit]
        if len(explicit) != len(arguments):
            return None
        bindings = {}
        for (_, declared, _), actual in zip(explicit, arguments):
            owner = interpreter.analyzer.owner(actual)
            if owner:
                bindings[declared] = owner.split(":", 1)[-1].replace("::", ".")
            elif actual.kind == "literal" and actual.name.isdecimal():
                bindings[declared] = "Int"
        result, index = [], 0
        for name, declared, _ in function.parameters:
            if name not in implicit:
                result.append(arguments[index])
                index += 1
                continue
            required = re.sub(r"\b[A-Za-z_]\w*\b", lambda match: bindings.get(match[0], match[0]), declared)
            provider = self.select_provider(interpreter, function.source, required)
            if provider is None:
                interpreter.analyzer.notices.add(("implicit_resolution_unproven", function.identifier + ":" + name))
                return None
            value = interpreter.apply_callable(provider, Value("type", provider.owner), [], call_node)
            if value == UNKNOWN:
                return None
            interpreter.analyzer.value_conditions[value].add("source_companion_implicit_selection_required")
            result.append(value)
        return result

    def select_provider(self, interpreter, source, required):
        constructor, arguments = type_parts(required)
        companion = self.program.declared_owner(source, constructor)
        if not companion or not arguments:
            return None
        candidates = []
        for function in self.providers:
            if function.owner not in self.ancestors(companion):
                continue
            type_parameters = next((part for part in children(function.node) if part.type == "type_parameters"), None)
            parameter_text = function.source.text(type_parameters)
            # A context bound requires independent evidence. This subset does
            # not synthesize Numeric/Ordering/etc. instances for arbitrary A.
            if re.search(r"(?<![<:]):(?!:)", parameter_text):
                continue
            offered, offered_args = type_parts(function.source.text(field(function.node, "return_type")))
            if self.program.declared_owner(function.source, offered) != companion or len(offered_args) != len(arguments):
                continue
            variables = set(re.findall(r"(?:\[|,)\s*([A-Za-z_]\w*)", parameter_text))
            first = offered_args[0]
            if first in variables:
                if "AnyRef" in parameter_text and not self.program.declared_owner(source, arguments[0]):
                    continue
            elif first != arguments[0]:
                continue
            candidates.append(function)
        return candidates[0] if len(candidates) == 1 else None

    def macro_template(self, interpreter, function):
        """Expose a returned reify template; never execute the macro itself."""
        text = function.source.text(function.body).lstrip()
        match = re.match(r"macro\s+([\w.]+)", text)
        if not match:
            return None
        *prefix, name = match[1].split(".")
        owner = self.program.declared_owner(function.source, ".".join(prefix))
        if not owner and prefix:
            owner = self.program.declared_owner(function.source, prefix[-1])
        candidates = self.program.methods.get((owner, name), ())
        if len(candidates) != 1:
            return None
        implementation = candidates[0]
        source = implementation.source
        declarations = [node for node in children(implementation.body) if node.type == "val_definition"]
        templates = []
        for declaration in declarations:
            value = field(declaration, "value")
            if value is None or value.type != "call_expression" or source.text(field(value, "function")) != "reify":
                continue
            body = next((part for part in children(value) if part.type == "block"), None)
            bound = source.text(field(declaration, "pattern") or field(declaration, "name"))
            tails = [part for part in children(implementation.body) if "comment" not in part.type]
            if body is None or not tails or not bound:
                continue
            # Only the returned tree's explicit dataflow is projected. Helper
            # transformations and compiler typing remain named obligations.
            if re.search(r"\b" + re.escape(bound) + r"\.tree\b", source.text(tails[-1])):
                templates.append((implementation, body))
        return templates[0] if len(templates) == 1 else None
