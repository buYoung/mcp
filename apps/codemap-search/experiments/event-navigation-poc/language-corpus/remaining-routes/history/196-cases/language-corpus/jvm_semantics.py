"""Source Java hierarchy and qualified VarHandle field access for mixed inputs."""

from model import UNKNOWN, Value, slot
from scala_semantics import children, field, nodes


class JvmSemantics:
    def __init__(self, program):
        self.program = program
        self.parents = {}
        self.interfaces = {}
        self.handles = {}
        self.enum_constants = {}
        self.field_modifiers = {}
        declarations = []
        for source in program.sources:
            if source.language != "java":
                continue
            for node in nodes(source.tree.root_node):
                if node.type not in {"class_declaration", "interface_declaration", "enum_declaration"}:
                    continue
                name = source.text(field(node, "name"))
                owner = program.owner(source, name)
                program.types[(source.path, name)] = owner
                program.owner_sources[owner] = source
                declarations.append((source, node, owner))
                for declaration in children(field(node, "body")):
                    if declaration.type == "field_declaration":
                        modifiers = next((part for part in children(declaration) if part.type == "modifiers"), None)
                        for variable in children(declaration):
                            if variable.type == "variable_declarator":
                                self.field_modifiers[(owner, source.text(field(variable, "name")))] = set(source.text(modifiers).split())
                if node.type == "enum_declaration":
                    self.enum_constants[owner] = {source.text(field(part, "name")) for part in children(field(node, "body")) if part.type == "enum_constant"}
        for source, node, owner in declarations:
            superclass = field(node, "superclass")
            if superclass is not None and children(superclass):
                self.parents[owner] = program.declared_owner(source, source.text(children(superclass)[0]))
            interfaces = field(node, "interfaces") or next((part for part in children(node) if part.type == "extends_interfaces"), None)
            if interfaces is not None:
                self.interfaces[owner] = [resolved for part in nodes(interfaces) if part.type == "type_identifier"
                                          if (resolved := program.declared_owner(source, source.text(part)))]
            for part in nodes(field(node, "body")):
                if part.type != "assignment_expression":
                    continue
                left, right = field(part, "left"), field(part, "right")
                if left is None or left.type != "identifier" or right is None or right.type != "method_invocation":
                    continue
                if source.text(field(right, "name")) != "findVarHandle":
                    continue
                lookup = field(right, "object")
                if lookup is None or lookup.type != "method_invocation" or source.text(field(lookup, "name")) != "lookup":
                    continue
                handle_api = source.text(field(lookup, "object"))
                if (program.imports.get((source.path, handle_api), handle_api) != "java.lang.invoke.MethodHandles"
                        or program.declared_owner(source, handle_api)):
                    continue
                if any(item.type == "variable_declarator" and source.text(field(item, "name")) == handle_api
                       for item in nodes(field(node, "body"))):
                    continue
                arguments = children(field(right, "arguments"))
                if len(arguments) != 3 or arguments[0].type != "class_literal" or arguments[1].type != "string_literal" or arguments[2].type != "class_literal":
                    continue
                target_type = children(arguments[0])
                target_owner = program.declared_owner(source, source.text(target_type[0])) if target_type else ""
                key = source.text(arguments[1]).strip('"')
                declared_handle = program.fields.get(owner, {}).get(source.text(left), "")
                handle_modifiers = self.field_modifiers.get((owner, source.text(left)), set())
                actual_type = program.fields.get(target_owner, {}).get(key, "").removeprefix("java.lang.")
                requested_type = source.text(children(arguments[2])[0]).removeprefix("java.lang.") if children(arguments[2]) else ""
                if (target_owner and key in program.fields.get(target_owner, {}) and
                        actual_type == requested_type and {"static", "final"}.issubset(handle_modifiers) and
                        "static" not in self.field_modifiers.get((target_owner, key), set()) and
                        program.imports.get((source.path, declared_handle), declared_handle) == "java.lang.invoke.VarHandle"):
                    self.handles[(owner, source.text(left))] = Value("wrapper", "java:VarHandle", Value("type", target_owner), Value("key", key))

    def lineage(self, owner):
        result = []
        while owner and owner not in result and len(result) < 16:
            result.append(owner)
            owner = self.parents.get(owner, "")
        return result

    def methods(self, owner, name):
        lineage = self.lineage(owner)
        for parent in lineage:
            methods = self.program.methods.get((parent, name), ())
            if methods:
                return methods
        candidates, pending, seen = [], [item for parent in lineage for item in self.interfaces.get(parent, ())], set()
        while pending and len(seen) < 32:
            interface = pending.pop(0)
            if interface in seen:
                continue
            seen.add(interface)
            candidates.extend(self.program.methods.get((interface, name), ()))
            pending.extend(self.interfaces.get(interface, ()))
        return candidates

    def binding(self, owner, name):
        for parent in self.lineage(owner):
            if (parent, name) in self.handles:
                return self.handles[(parent, name)]
        return None

    def invoke_handle(self, interpreter, handle, method, arguments, node):
        if handle.kind != "wrapper" or handle.name != "java:VarHandle" or not arguments:
            return None
        owner = interpreter.analyzer.owner(arguments[0])
        if handle.base.name not in self.lineage(owner):
            interpreter.analyzer.notices.add(("var_handle_receiver_unproven", interpreter.identifier))
            return UNKNOWN
        target = slot(arguments[0], handle.key)
        if method in {"get", "getVolatile", "getAcquire", "getOpaque"} and len(arguments) == 1:
            interpreter.analyzer.value_conditions[target].add("qualified_jdk_varhandle_access")
            return target
        if "final" in self.field_modifiers.get((handle.base.name, handle.key.name), set()):
            interpreter.analyzer.notices.add(("var_handle_read_only_field", handle.base.name + ":" + handle.key.name))
            return UNKNOWN
        if method == "compareAndSet" and len(arguments) == 3:
            interpreter.store_value(target, arguments[2], node, ("compare_and_set_success_required", "qualified_jdk_varhandle_access"))
            return UNKNOWN
        if method in {"set", "setVolatile", "setRelease", "setOpaque"} and len(arguments) == 2:
            interpreter.store_value(target, arguments[1], node, ("qualified_jdk_varhandle_access",))
            return UNKNOWN
        return None
