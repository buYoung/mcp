"""Bounded source-derived function summaries and symbolic heap access paths.

Only language/standard-container primitives have built-in behavior. User methods
are expanded from their bodies. Unknown callees remain explicit boundaries.
"""

from pathlib import Path
import re

from model import ELEMENT, MAP_ENTRIES, MAP_KEYS, UNKNOWN, Fact, Summary, Value, merge_values, referenced_values, slot, split_path, substitute, tuple_value, tuple_values, value_options
from syntax import CLASSES, FUNCTIONS, IDENTIFIERS, Function, Program, child, container_kind, element_type, walk


MAX_FACTS_PER_FUNCTION = 192
MAX_EXPRESSION_DEPTH = 48
MAX_SUMMARY_PASSES = 4
MAX_TOTAL_FACTS = 40000


class Analyzer:
    def __init__(self, program: Program):
        self.program = program
        self.summaries: dict[str, Summary] = {}
        self.globals: dict[str, dict[str, Value]] = {}
        self.value_types: dict[Value, str] = {}
        self.value_owners: dict[Value, str] = {}
        self.heap_values: dict[Value, list[Value]] = {}
        self.heap_read_cache: dict[Value, list[Value]] = {}
        self.allocation_owners: dict[Value, str] = {}
        self.allocation_origins: dict[Value, str] = {}
        self.allocation_contexts: dict[Value, tuple] = {}
        self.captures: dict[str, dict[str, Value]] = {}
        self.notices: set[tuple[str, str]] = set()
        self.module_facts: list[Fact] = []

    def kind(self, value: Value, source) -> str:
        type_text = self.program.expand_type(source, self.type_text(value, source))
        known = container_kind(type_text) or (type_text if type_text in {"map", "set", "array"} else "")
        if known:
            return known
        inferred = set()
        for stored in self.read_values(value):
            text = self.program.expand_type(source, self.type_text(stored, source))
            candidate = container_kind(text) or (text if text in {"map", "set", "array"} else "")
            if candidate:
                inferred.add(candidate)
        return next(iter(inferred)) if len(inferred) == 1 else ""

    def read_values(self, value: Value) -> list[Value]:
        if value in self.heap_read_cache:
            return self.heap_read_cache[value]
        budget = [96]
        result = self.read_heap_values(value, 0, set(), budget)
        if budget[0] == 0:
            self.notices.add(("heap_read_bound", value.display()))
        self.heap_read_cache[value] = result
        return result

    def read_heap_values(self, value: Value, depth, seen, budget) -> list[Value]:
        if budget[0] == 0:
            return []
        budget[0] -= 1
        if depth >= 6 or value.kind != "slot":
            return []
        seen = set(seen)
        if value in seen:
            return []
        seen.add(value)
        found = [option for item in self.heap_values.get(value, ()) for option in value_options(item)]
        for base in self.read_heap_values(value.base, depth + 1, seen, budget):
            found.extend(option for item in self.heap_values.get(slot(base, value.key), ()) for option in value_options(item))
        expanded = list(found)
        for item in found:
            expanded.extend(self.read_heap_values(item, depth + 1, seen, budget))
        return list(dict.fromkeys(expanded))[:16]

    def refresh_heap(self, facts):
        heap = {}
        for fact in facts:
            if fact.kind == "store" and fact.target != fact.value:
                values = heap.setdefault(fact.target, [])
                if fact.value not in values and len(values) < 16:
                    values.append(fact.value)
        self.heap_values = heap
        self.heap_read_cache.clear()

    def type_text(self, value: Value, source, depth=0) -> str:
        if depth > 12:
            return ""
        type_text = self.value_types.get(value, "")
        if value.kind == "slot":
            if value.key == MAP_ENTRIES:
                return self.type_text(value.base, source, depth + 1)
            if value.key.kind == "key":
                owner = self.owner(value.base, source)
                declared = self.program.field_types.get((owner, value.key.name), "")
                type_text = declared or type_text
            if not type_text:
                base_type = self.type_text(value.base, source, depth + 1)
                namespace = str(Path(source.path).parent) if source.language == "go" else source.path
                base_type = self.program.aliases.get((namespace, base_type), base_type)
                type_text = element_type(base_type)
        return type_text.lstrip(": ")

    def owner(self, value: Value, source) -> str:
        if value in self.value_owners:
            return self.value_owners[value]
        if value.kind == "receiver":
            return value.name
        type_text = self.value_types.get(value, "")
        if value.kind == "slot" and value.key.kind == "key":
            base_owner = self.owner(value.base, source)
            type_text = self.program.field_types.get((base_owner, value.key.name), "") or type_text
        return self.program.resolve_type(source, type_text)

    def promoted_receiver(self, receiver, method, source):
        if source.language != "go":
            return receiver
        owner = self.owner(receiver, source)
        if self.program.methods.get((owner, method)):
            return receiver
        matches = []
        for field_name in self.program.embedded_fields.get(owner, ()):
            embedded = slot(receiver, field_name)
            embedded_owner = self.owner(embedded, source)
            if method in self.program.interface_methods.get(embedded_owner, set()) or self.program.methods.get((embedded_owner, method)):
                matches.append(embedded)
        return matches[0] if len(matches) == 1 else receiver

    def forward_callback_arguments(self, facts):
        sources = {source.path: source for source in self.program.sources}
        forwarded = []
        for argument in facts:
            callee = argument.value
            if argument.kind != "argument" or argument.argument_index is None or callee.kind != "slot" or callee.key.kind != "key":
                continue
            source = sources[argument.location.path]
            receiver = self.promoted_receiver(callee.base, callee.key.name, source)
            # Only concrete types reached through source-backed stored values are
            # considered. A method name or interface alone never selects a body.
            for actual in self.read_values(receiver):
                owner = self.owner(actual, source)
                if actual.kind != "allocation" or not owner:
                    continue
                methods = self.program.methods.get((owner, callee.key.name), ())
                if len(methods) != 1:
                    continue
                summary = self.summaries.get(methods[0])
                if summary is None or argument.argument_index >= len(summary.parameters):
                    continue
                formal = summary.parameters[argument.argument_index]
                for fact in summary.facts:
                    if fact.kind not in {"invoke", "member_invoke"} or split_path(fact.target)[0] != formal:
                        continue
                    conditions = tuple(sorted(set(fact.conditions + argument.conditions +
                                                  ("runtime_branch_selection_required", "dispatch_instance_unproven", f"source_receiver_type:{owner}"))))
                    target = substitute(fact.target, {formal: argument.target})
                    forwarded.append(Fact(fact.kind, target, fact.value, fact.location, fact.function,
                                          conditions, fact.via + (argument.location,)))
        return list(dict.fromkeys(forwarded))

    def run(self) -> list[Fact]:
        stable = False
        for iteration in range(MAX_SUMMARY_PASSES):
            previous = self.summaries
            previous_module_facts = self.module_facts
            self.module_facts = []
            # Imports may point to modules later in path order. Revisit module
            # initializers with the same bounded summary generation, not only once.
            for source in self.program.sources:
                interpreter = Interpreter(self, source)
                interpreter.statement(source.tree.root_node)
                self.globals[source.path] = dict(interpreter.env)
                self.module_facts.extend(interpreter.facts)
            next_summaries = {}
            for function in self.program.functions.values():
                interpreter = Interpreter(self, function.source, function)
                interpreter.statement(function.body)
                summary = Summary(function.name, function.source.path, function.owner, function.receiver,
                                  tuple(interpreter.parameters), interpreter.facts, interpreter.returns)
                next_summaries[function.identifier] = summary
            self.summaries = next_summaries
            self.refresh_heap(self.module_facts + [fact for summary in self.summaries.values() for fact in summary.facts])
            if next_summaries == previous and self.module_facts == previous_module_facts:
                stable = True
                break
        if not stable:
            self.notices.add(("summary_depth_bound", str(MAX_SUMMARY_PASSES)))
        facts = self.module_facts + [fact for summary in self.summaries.values() for fact in summary.facts]
        facts = list(dict.fromkeys(facts))
        if len(facts) > MAX_TOTAL_FACTS:
            self.notices.add(("total_fact_cap", str(len(facts) - MAX_TOTAL_FACTS)))
            facts = facts[:MAX_TOTAL_FACTS]
        # Project explicit object fields through stored aliases, without inventing
        # an object for an opaque external return or a dependency-injected type.
        for _ in range(2):
            fields = {}
            for fact in facts:
                root, _ = split_path(fact.target)
                if fact.kind == "store" and root.kind == "allocation":
                    fields.setdefault(root, []).append(fact)
            added = []
            for store in facts:
                if store.kind != "store" or store.value.kind != "allocation":
                    continue
                for field in fields.get(store.value, ()):
                    if store.location in field.via or field.location == store.location or len(field.via) >= 6:
                        continue
                    target = substitute(field.target, {store.value: store.target})
                    projected = Fact("store", target, field.value, field.location, field.function,
                                     tuple(sorted(set(field.conditions + store.conditions))), field.via + (store.location,), field.argument_index)
                    added.append(projected)
                    if field.value in self.value_types:
                        self.value_types[target] = self.value_types[field.value]
            facts = list(dict.fromkeys(facts + added))
            if len(facts) > MAX_TOTAL_FACTS:
                self.notices.add(("total_fact_cap", str(len(facts) - MAX_TOTAL_FACTS)))
                facts = facts[:MAX_TOTAL_FACTS]
        self.refresh_heap(facts)
        return list(dict.fromkeys(facts + self.forward_callback_arguments(facts)))


class Interpreter:
    def __init__(self, analyzer: Analyzer, source, function: Function | None = None,
                 env=None, conditions=(), depth=0):
        self.analyzer = analyzer
        self.program = analyzer.program
        self.source = source
        self.function = function
        self.identifier = function.identifier if function else "module:" + source.path
        self.env = dict(env if env is not None else analyzer.globals.get(source.path, {}))
        self.env.update(analyzer.captures.get(self.identifier, {}))
        self.conditions = tuple(conditions)
        if function and function.parent:
            self.conditions += ("enclosing_callable_execution_unproven",)
        self.depth = depth
        self.facts: list[Fact] = []
        self.returns: list[Value] = []
        self.parameters = []
        if function:
            parameter_node = child(function.node, "parameters") or child(function.node, "parameter")
            for name, type_text in function.parameters:
                value = Value("parameter", function.identifier + ":" + name)
                self.parameters.append(value)
                self.env[name] = value
                if name.startswith("{") and parameter_node:
                    for parameter in parameter_node.named_children:
                        pattern = child(parameter, "pattern") or child(parameter, "name")
                        if pattern is not None and self.text(pattern) == name:
                            self.bind(pattern, value, type_text)
                if type_text:
                    analyzer.value_types[value] = type_text.lstrip(": ")
                    owner = self.program.resolve_type(source, type_text.lstrip(": "))
                    if owner:
                        analyzer.value_owners[value] = owner
            self.env[function.receiver_name] = function.receiver
            if function.owner:
                self.env["Self"] = Value("type", function.owner)
            if function.name == "constructor" and parameter_node:
                for parameter in parameter_node.named_children:
                    if any(part.type == "accessibility_modifier" for part in parameter.named_children):
                        pattern = child(parameter, "pattern") or child(parameter, "name")
                        if pattern is not None:
                            name = self.text(pattern)
                            self.emit("store", slot(function.receiver, name), self.env.get(name, UNKNOWN), parameter)

    def text(self, node):
        return self.source.text(node)

    def emit(self, kind, target, value, node, extra=(), via=(), argument_index=None):
        if len(self.facts) >= MAX_FACTS_PER_FUNCTION:
            self.analyzer.notices.add(("function_fact_cap", self.identifier))
            return
        fact = Fact(kind, target, value, self.source.location(node), self.identifier,
                    tuple(sorted(set(self.conditions + tuple(extra)))), tuple(via), argument_index)
        if fact not in self.facts:
            self.facts.append(fact)

    def binding(self, name):
        if name in self.env:
            return self.env[name]
        namespace = str(Path(self.source.path).parent) if self.source.language == "go" else self.source.path
        imported = self.program.imports.get((self.source.path, name), "")
        if imported and "#" not in imported:
            return Value("namespace", imported)
        if imported:
            namespace, name = imported.rsplit("#", 1)
            exported = self.analyzer.globals.get(namespace, {}).get(name)
            if exported is not None:
                return exported
        candidates = self.program.methods.get((namespace, name), [])
        if len(candidates) == 1:
            return Value("function", candidates[0])
        if (namespace, name) in self.program.aliases or (namespace, name) in self.program.types:
            return Value("type", namespace + "::" + name)
        if name == "globalThis" and self.source.language in {"typescript", "tsx"}:
            return Value("global", "javascript:globalThis")
        return Value("unresolved", self.source.path + ":" + name)

    def bind(self, pattern, value, type_text=""):
        if pattern is None:
            return
        if pattern.type in IDENTIFIERS:
            if value == UNKNOWN:
                value = Value("unresolved", self.identifier + ":binding:" + self.text(pattern))
            self.env[self.text(pattern)] = value
            if type_text:
                self.analyzer.value_types[value] = type_text.lstrip(": ")
                owner = self.program.resolve_type(self.source, type_text.lstrip(": "))
                if owner:
                    self.analyzer.value_owners[value] = owner
        elif pattern.type in {"object_pattern", "object_assignment_pattern"}:
            for part in pattern.named_children:
                if part.type in IDENTIFIERS:
                    self.bind(part, slot(value, self.text(part)))
                elif part.type in {"pair_pattern", "pair"}:
                    self.bind(child(part, "value"), slot(value, self.text(child(part, "key"))))
        elif pattern.type in {"tuple_pattern", "array_pattern", "expression_list"}:
            parts = tuple_values(value)
            for index, part in enumerate(pattern.named_children):
                self.bind(part, parts[index] if index < len(parts) else slot(value, Value("key", str(index))))
        elif pattern.type in {"mut_pattern", "ref_pattern", "reference_pattern"} and pattern.named_children:
            self.bind(pattern.named_children[-1], value, type_text)
        elif pattern.type == "struct_pattern":
            for field in pattern.named_children:
                if field.type == "field_pattern":
                    field_name = child(field, "name") or child(field, "field")
                    target = child(field, "pattern") or field_name
                    if field_name is not None:
                        self.bind(target, slot(value, self.text(field_name)))
        else:
            self.analyzer.notices.add(("binding_pattern_unsupported", pattern.type))

    def statement(self, node):
        if node is None:
            return
        kind = node.type
        if kind in FUNCTIONS:
            self.expression(node)
            return
        if kind in CLASSES:
            name = self.text(child(node, "name") or child(node, "type"))
            receiver = Value("receiver", self.program.owner_key(self.source, name))
            body = child(node, "body")
            for field in body.named_children if body else []:
                if field.type == "public_field_definition":
                    target = slot(receiver, self.text(child(field, "name")))
                    initial = self.expression(child(field, "value"))
                    if initial != UNKNOWN:
                        self.emit("store", target, initial, field, ("field_initializer_schema",))
                    type_text = self.text(child(field, "type"))
                    if type_text:
                        self.analyzer.value_types[target] = type_text.lstrip(": ")
                    elif initial in self.analyzer.value_types:
                        self.analyzer.value_types[target] = self.analyzer.value_types[initial]
                elif field.type in FUNCTIONS:
                    self.expression(field)
            return
        if kind in {"variable_declarator", "let_declaration", "var_spec", "const_spec"}:
            pattern = child(node, "name") or child(node, "pattern")
            value_node = child(node, "value")
            value = self.expression(value_node)
            self.bind(pattern, value, self.text(child(node, "type")))
            return
        if kind in {"short_var_declaration", "assignment_statement"}:
            left = child(node, "left")
            right = child(node, "right")
            left_nodes = left.named_children if left and left.type == "expression_list" else [left]
            right_nodes = right.named_children if right and right.type == "expression_list" else [right]
            values = [self.expression(item) for item in right_nodes]
            if len(values) == 1 and len(left_nodes) > 1:
                options = value_options(values[0])
                alternatives = [tuple_values(option) for option in options]
                is_multi_call = len(right_nodes) == 1 and right_nodes[0] is not None and right_nodes[0].type == "call_expression"
                if is_multi_call or any(alternatives):
                    alternatives = [items if len(items) == len(left_nodes) else
                                    [slot(option, f"return_{index}") for index in range(len(left_nodes))]
                                    for option, items in zip(options, alternatives)]
                    values = [merge_values([items[index] for items in alternatives]) for index in range(len(left_nodes))]
            for index, pattern in enumerate(left_nodes):
                value = values[index] if index < len(values) else UNKNOWN
                if pattern is not None and pattern.type in IDENTIFIERS:
                    self.bind(pattern, value)
                elif pattern is not None:
                    target = self.expression(pattern)
                    self.emit("store", target, value, node)
                    if target.kind == "slot" and self.analyzer.kind(target.base, self.source) == "map":
                        base = target.base.base if target.base.kind == "slot" and target.base.key == MAP_ENTRIES else target.base
                        self.emit("store", slot(slot(base, MAP_KEYS), ELEMENT), target.key, node)
            return
        if kind in {"return_statement", "return_expression"}:
            parts = list(node.named_children)
            if self.source.language == "go" and len(parts) == 1 and parts[0].type == "expression_list":
                parts = list(parts[0].named_children)
            values = [self.expression(part) for part in parts]
            if self.source.language == "go" and len(values) > 1:
                values = [tuple_value(values)]
            self.returns.extend(value for value in values if value != UNKNOWN)
            return
        if kind in {"if_statement", "if_expression", "switch_statement", "expression_switch_statement",
                    "type_switch_statement", "match_expression", "conditional_expression"}:
            base_env = dict(self.env)
            branch_envs = [base_env]
            old_conditions = self.conditions
            self.conditions += (f"conditional_control:{self.source.path}:{self.source.location(node).line}",)
            for part in node.named_children:
                self.env = dict(base_env)
                self.statement(part)
                branch_envs.append(dict(self.env))
            self.env = {name: value if all(branch.get(name, value) == value for branch in branch_envs) else UNKNOWN
                        for name, value in base_env.items()}
            self.conditions = old_conditions
            return
        if kind in {"for_in_statement", "for_expression", "for_statement"}:
            self.loop(node)
            return
        if kind in {"expression_statement", "assignment_expression", "augmented_assignment_expression", "call_expression",
                    "new_expression", "await_expression", "send_statement"}:
            self.expression(node)
            return
        if kind in {"type_declaration", "type_alias_declaration", "interface_declaration", "struct_item",
                    "enum_item", "import_statement", "import_declaration", "use_declaration", "comment",
                    "line_comment", "block_comment", "attribute_item"}:
            return
        for part in node.named_children:
            self.statement(part)
        if kind == "block" and self.source.language == "rust" and node.named_children:
            tail = node.named_children[-1]
            if tail.type not in {"expression_statement", "let_declaration", "return_expression"}:
                value = self.expression(tail)
                if value != UNKNOWN:
                    self.returns.append(value)

    def loop(self, node):
        left = child(node, "left") or child(node, "pattern")
        right = child(node, "right") or child(node, "value")
        range_node = next((part for part in node.named_children if part.type == "range_clause"), None)
        if range_node:
            left, right = child(range_node, "left"), child(range_node, "right")
        body = child(node, "body")
        if right is None:
            for part in node.named_children:
                if part != body:
                    self.statement(part)
            self.statement(body)
            return
        container = self.expression(right)
        iterator_mode = container.name if container.kind == "iterator" else ""
        if iterator_mode:
            container = container.base
        kind = self.analyzer.kind(container, self.source)
        old_env = dict(self.env)
        old_conditions = self.conditions
        self.conditions += ("iteration_may_be_empty",)
        if left is not None:
            if left.type in {"lexical_declaration", "variable_declaration"}:
                declaration = next((part for part in left.named_children if part.type == "variable_declarator"), None)
                left = child(declaration, "name") or left
            parts = left.named_children if left.type in {"expression_list", "tuple_pattern", "array_pattern"} else [left]
            if self.source.language == "go" and kind == "map":
                if parts:
                    self.bind(parts[0], slot(slot(container, MAP_KEYS), ELEMENT))
                if len(parts) > 1:
                    self.bind(parts[1], slot(slot(container, MAP_ENTRIES), ELEMENT))
            elif len(parts) > 1 and kind == "map":
                self.bind(parts[0], slot(slot(container, MAP_KEYS), ELEMENT))
                self.bind(parts[1], slot(slot(container, MAP_ENTRIES), ELEMENT))
            else:
                for part in parts:
                    if kind == "map":
                        if iterator_mode in {"values", "keys"}:
                            base = slot(container, MAP_KEYS if iterator_mode == "keys" else MAP_ENTRIES)
                            self.bind(part, slot(base, ELEMENT))
                        else:
                            self.bind(part, UNKNOWN)
                    else:
                        self.bind(part, slot(container, ELEMENT))
        self.statement(body)
        self.env = old_env
        self.conditions = old_conditions

    def expression(self, node, level=0) -> Value:
        if node is None:
            return UNKNOWN
        if level > MAX_EXPRESSION_DEPTH:
            self.analyzer.notices.add(("expression_depth_cap", self.identifier))
            return UNKNOWN
        expr = lambda part: self.expression(part, level + 1)
        kind = node.type
        if kind in IDENTIFIERS or kind in {"this", "self"}:
            return self.binding(self.text(node))
        if kind in {"string", "string_literal", "interpreted_string_literal", "raw_string_literal", "template_string"}:
            text = self.text(node)
            if "${" in text or "\\" in text:
                return Value("dynamic_key", self.identifier + ":" + str(node.start_byte))
            return Value("key", text.strip("'\"`"))
        if kind in {"number", "integer_literal", "float_literal", "int_literal", "true", "false", "null", "nil"}:
            return Value("literal", self.text(node))
        if kind in FUNCTIONS:
            function = self.program.nodes.get((self.source.path, node.start_byte))
            if function:
                self.analyzer.captures[function.identifier] = dict(self.env)
                return Value("function", function.identifier)
            return UNKNOWN
        if kind in {"member_expression", "field_expression", "selector_expression"}:
            base_node = child(node, "object") or child(node, "value") or child(node, "operand")
            field_node = child(node, "property") or child(node, "field")
            if base_node is None and node.named_children:
                base_node = node.named_children[0]
            if field_node is None and len(node.named_children) > 1:
                field_node = node.named_children[-1]
            base = expr(base_node)
            if base.kind == "type" and self.source.language in {"typescript", "tsx"}:
                base = Value("global", base.name + "::static")
                self.analyzer.value_owners[base] = self.text(base_node) and self.program.resolve_type(self.source, self.text(base_node))
            return slot(base, self.text(field_node))
        if kind in {"subscript_expression", "index_expression"}:
            base_node = child(node, "object") or child(node, "operand") or child(node, "value")
            index_node = child(node, "index")
            parts = node.named_children
            base = expr(base_node or (parts[0] if parts else None))
            if self.source.language == "go" and self.analyzer.kind(base, self.source) == "map":
                base = slot(base, MAP_ENTRIES)
            return slot(base, expr(index_node or (parts[-1] if len(parts) > 1 else None)))
        if kind in {"parenthesized_expression", "await_expression", "reference_expression", "unary_expression",
                    "as_expression", "non_null_expression", "type_assertion_expression", "try_expression",
                    "generic_function", "instantiation_expression", "spread_element", "expression_statement", "literal_element"}:
            if kind == "unary_expression" and self.text(node).startswith("delete "):
                target = expr(node.named_children[-1])
                self.emit("remove", target, UNKNOWN, node)
                return UNKNOWN
            part = child(node, "argument") or child(node, "value") or child(node, "expression") or child(node, "function")
            if part is None and node.named_children:
                part = node.named_children[0]
            return expr(part)
        if kind in {"assignment_expression", "augmented_assignment_expression"}:
            left, right = child(node, "left"), child(node, "right")
            value = expr(right)
            if left and left.type in IDENTIFIERS:
                self.bind(left, value)
            else:
                target = expr(left)
                self.emit("store", target, value, node)
                if target.kind == "slot" and self.analyzer.kind(target.base, self.source) == "map":
                    base = target.base.base if target.base.kind == "slot" and target.base.key == MAP_ENTRIES else target.base
                    self.emit("store", slot(slot(base, MAP_KEYS), ELEMENT), target.key, node)
            return value
        if kind in {"object", "object_pattern", "array", "array_expression", "struct_expression", "composite_literal"}:
            return self.object_value(node, level)
        if kind == "tuple_expression":
            return tuple_value([expr(part) for part in node.named_children])
        if kind == "new_expression":
            type_node = child(node, "constructor")
            type_name = self.text(type_node)
            value = Value("allocation", f"{self.source.path}:{self.source.location(node).line}:{self.source.location(node).column}")
            self.analyzer.allocation_owners[value] = self.identifier
            type_arguments = self.text(child(node, "type_arguments"))
            intrinsic = {"Map": "map", "Set": "set", "Array": "array"}.get(type_name)
            self.analyzer.value_types[value] = type_name + type_arguments if type_arguments else (intrinsic or type_name)
            owner = self.program.resolve_type(self.source, type_name)
            if owner:
                self.analyzer.value_owners[value] = owner
            arguments = [expr(argument) for argument in (child(node, "arguments").named_children if child(node, "arguments") else [])]
            constructors = self.program.methods.get((owner, "constructor"), []) if owner else []
            if len(constructors) == 1 and constructors[0] in self.analyzer.summaries:
                returned = self.apply_summary(constructors[0], value, arguments, node)
                if returned != UNKNOWN and self.analyzer.owner(returned, self.source):
                    return returned
            return value
        if kind in {"call_expression", "macro_invocation"}:
            return self.call(node, level)
        if kind in {"binary_expression", "conditional_expression"}:
            values = [expr(part) for part in node.named_children]
            operator = self.text(child(node, "operator"))
            if operator in {"||", "??"}:
                self.conditions += (f"fallback_path_unresolved:{self.source.location(node).line}",)
                return next((value for value in values if value.kind in {"slot", "receiver", "allocation", "parameter"}), UNKNOWN)
            return UNKNOWN
        if kind in {"expression_list", "arguments", "argument_list"}:
            values = [expr(part) for part in node.named_children]
            return values[0] if len(values) == 1 else UNKNOWN
        if kind in {"scoped_identifier", "scoped_type_identifier"}:
            return self.binding(self.text(node))
        if kind == "send_statement":
            self.emit("transport_boundary", expr(child(node, "channel")), expr(child(node, "value")), node)
            return UNKNOWN
        if kind in {"statement_block", "block"}:
            self.statement(node)
            return self.returns[-1] if self.returns else UNKNOWN
        for part in node.named_children:
            expr(part)
        return UNKNOWN

    def object_value(self, node, level):
        type_node = child(node, "type") or child(node, "name")
        type_text = self.text(type_node)
        value = Value("allocation", f"{self.source.path}:{self.source.location(node).line}:{self.source.location(node).column}")
        self.analyzer.allocation_owners[value] = self.identifier
        if node.type in {"array", "array_expression"}:
            self.analyzer.value_types[value] = "array"
            for part in node.named_children:
                self.emit("store", slot(value, ELEMENT), self.expression(part, level + 1), part)
            return value
        self.analyzer.value_types[value] = type_text
        owner = self.program.resolve_type(self.source, type_text)
        if owner:
            self.analyzer.value_owners[value] = owner
        body = child(node, "body")
        fields = body.named_children if body else node.named_children
        for field in fields:
            if field.type in {"pair", "field_initializer", "keyed_element"}:
                key_node = child(field, "key") or child(field, "field")
                field_value = child(field, "value")
                if key_node is None and field.named_children:
                    key_node = field.named_children[0]
                if field_value is None and len(field.named_children) > 1:
                    field_value = field.named_children[-1]
                if key_node is not None and key_node.type == "literal_element" and key_node.named_children:
                    key_node = key_node.named_children[0]
                key = self.text(key_node).strip("'\"")
                stored = self.expression(field_value, level + 1)
                self.emit("store", slot(value, key), stored, field)
                if owner:
                    self.emit("store", slot(Value("receiver", owner), key), stored, field, ("constructor_schema_only",))
            elif field.type in {"shorthand_property_identifier", "shorthand_field_initializer"}:
                key = self.text(field)
                self.emit("store", slot(value, key), self.binding(key), field)
                if owner:
                    self.emit("store", slot(Value("receiver", owner), key), self.binding(key), field, ("constructor_schema_only",))
        return value

    def call(self, node, level):
        if node.type == "macro_invocation":
            self.emit("macro_boundary", UNKNOWN, UNKNOWN, node)
            return UNKNOWN
        callee_node = child(node, "function")
        callee = self.expression(callee_node, level + 1)
        arguments_node = child(node, "arguments")
        argument_nodes = list(arguments_node.named_children) if arguments_node else []
        arguments = [self.expression(part, level + 1) for part in argument_nodes]
        callee_text = self.text(callee_node)
        if self.source.language == "go" and callee.kind == "type" and len(arguments) == 1:
            namespace, name = callee.name.rsplit("::", 1)
            underlying = self.program.aliases.get((namespace, name), "").strip()
            if underlying.startswith("func(") or underlying.startswith("func ("):
                self.emit("function_type_conversion", arguments[0], callee, node)
                return arguments[0]
        if self.source.language == "go" and callee_text == "append" and arguments:
            for value in arguments[1:]:
                self.emit("store", slot(arguments[0], ELEMENT), value, node)
            self.analyzer.value_types[arguments[0]] = "array"
            return arguments[0]
        if self.source.language == "go" and callee_text == "delete" and len(arguments) == 2:
            self.emit("remove", slot(slot(arguments[0], MAP_ENTRIES), arguments[1]), UNKNOWN, node)
            return UNKNOWN
        receiver = callee.base if callee.kind == "slot" else UNKNOWN
        method = callee.key.name if callee.kind == "slot" and callee.key.kind == "key" else ""
        kind = self.analyzer.kind(receiver, self.source)
        if kind:
            value = self.container_call(kind, receiver, method, arguments, argument_nodes, node)
            if value is not None:
                return value
        for index, argument in enumerate(arguments):
            # A stored value passed to a callee is observable even when that
            # callee's body/interface target cannot be resolved. It is not a call
            # of the value and must retain the argument position.
            declared = self.program.expand_type(self.source, self.analyzer.type_text(argument, self.source))
            if argument.kind == "slot" or (argument.kind == "parameter" and (declared.startswith("func") or "=>" in declared)):
                self.emit("argument", argument, callee, node, argument_index=index)
        target_ids = []
        if callee.kind == "function":
            target_ids = [callee.name]
        elif method:
            promoted = self.analyzer.promoted_receiver(receiver, method, self.source)
            if promoted != receiver:
                receiver = promoted
                callee = slot(receiver, method)
            owner = self.analyzer.owner(receiver, self.source)
            if owner:
                target_ids = self.program.methods.get((owner, method), [])
            elif receiver.kind == "namespace":
                target_ids = self.program.methods.get((receiver.name, method), [])
        if len(target_ids) == 1:
            returned = self.apply_summary(target_ids[0], receiver, arguments, node)
            if returned != UNKNOWN:
                return returned
            return Value("result", f"{self.source.path}:{self.source.location(node).line}:{self.source.location(node).column}")
        if callee.kind in {"slot", "parameter"}:
            is_member = callee_node is not None and callee_node.type in {"member_expression", "field_expression", "selector_expression"}
            declared = self.program.expand_type(self.source, self.analyzer.type_text(callee, self.source))
            if declared.startswith(("func(", "unsafe fn", "fn(")) or "=>" in declared:
                is_member = False
            self.emit("member_invoke" if is_member else "invoke", callee, UNKNOWN, node)
        else:
            self.emit("external_boundary", callee, UNKNOWN, node)
        return Value("result", f"{self.source.path}:{self.source.location(node).line}:{self.source.location(node).column}")

    def apply_summary(self, target_id, receiver, arguments, node):
        summary = self.analyzer.summaries.get(target_id)
        if summary is None or target_id == self.identifier:
            return UNKNOWN
        replacements = dict(zip(summary.parameters, arguments))
        if summary.receiver != UNKNOWN and receiver != UNKNOWN:
            replacements[summary.receiver] = receiver
        semantic_kinds = {"store", "remove", "invoke", "member_invoke", "argument", "function_type_conversion"}
        selected = [fact for fact in summary.facts if fact.kind in semantic_kinds]
        values = set()
        for value in list(summary.returns) + [value for fact in selected for value in (fact.target, fact.value)]:
            values.update(referenced_values(value))
        location = self.source.location(node)
        for value in sorted(values, key=lambda item: item.display()):
            if value.kind != "allocation" or self.analyzer.allocation_owners.get(value) != target_id:
                continue
            context = self.analyzer.allocation_contexts.get(value, ())
            if location in context or len(context) >= 3:
                replacements[value] = UNKNOWN
                self.analyzer.notices.add(("allocation_context_depth_bound", target_id))
                continue
            origin = self.analyzer.allocation_origins.get(value, value.name)
            context = context + (location,)
            suffix = ">".join(f"{item.path}:{item.line}:{item.column}" for item in context)
            allocated = Value("allocation", origin + "@" + suffix)
            replacements[value] = allocated
            self.analyzer.allocation_owners[allocated] = self.identifier
            self.analyzer.allocation_origins[allocated] = origin
            self.analyzer.allocation_contexts[allocated] = context
            if value in self.analyzer.value_types:
                self.analyzer.value_types[allocated] = self.analyzer.value_types[value]
            if value in self.analyzer.value_owners:
                self.analyzer.value_owners[allocated] = self.analyzer.value_owners[value]
        for fact in selected:
            if len(fact.via) >= MAX_SUMMARY_PASSES or location in fact.via:
                continue
            projected = Fact(fact.kind, substitute(fact.target, replacements), substitute(fact.value, replacements),
                             fact.location, fact.function, tuple(sorted(set(fact.conditions + self.conditions))),
                             fact.via + (location,), fact.argument_index)
            if projected in self.facts:
                continue
            if len(self.facts) >= MAX_FACTS_PER_FUNCTION:
                self.analyzer.notices.add(("function_fact_cap", self.identifier))
                break
            self.facts.append(projected)
        returns = list(dict.fromkeys(substitute(value, replacements) for value in summary.returns))
        if len(returns) == 1:
            if returns[0].kind == "function":
                returned_function = self.program.functions.get(returns[0].name)
                if returned_function and returned_function.parent:
                    self.analyzer.notices.add(("returned_closure_environment_unresolved", returned_function.identifier))
                    return UNKNOWN
            return returns[0]
        if returns:
            merged = merge_values(returns)
            if merged.kind == "unknown":
                self.analyzer.notices.add(("value_alternative_bound", target_id))
            return merged
        return UNKNOWN

    def container_call(self, kind, receiver, method, arguments, argument_nodes, node):
        if kind == "map" and method in {"get", "get_mut", "remove", "delete"} and arguments:
            target = slot(slot(receiver, MAP_ENTRIES), arguments[0])
            if method in {"remove", "delete"}:
                self.emit("remove", target, UNKNOWN, node)
                return UNKNOWN
            return target
        if kind == "map" and method in {"set", "insert"} and len(arguments) >= 2:
            self.emit("store", slot(slot(receiver, MAP_ENTRIES), arguments[0]), arguments[1], node)
            self.emit("store", slot(slot(receiver, MAP_KEYS), ELEMENT), arguments[0], node)
            return receiver if method == "set" else UNKNOWN
        if kind in {"array", "set"} and method in {"push", "push_back", "add", "insert"} and arguments:
            for value in arguments:
                self.emit("store", slot(receiver, ELEMENT), value, node)
            return receiver if method == "add" and self.source.language in {"typescript", "tsx"} else UNKNOWN
        if method in {"clear", "delete", "remove", "splice", "retain", "pop", "pop_front"}:
            target = slot(receiver, MAP_ENTRIES) if kind == "map" else slot(receiver, ELEMENT)
            self.emit("remove", target, UNKNOWN, node)
            return UNKNOWN
        if method in {"values", "iter", "iter_mut", "into_iter"}:
            mode = "values" if method == "values" or kind != "map" else "pairs"
            return Value("iterator", mode, receiver)
        if method in {"forEach", "map", "filter", "find", "some", "every", "flatMap", "for_each"} and arguments:
            callback = arguments[0]
            if callback.kind == "function" and self.depth < 6:
                function = self.program.functions.get(callback.name)
                if function:
                    nested = Interpreter(self.analyzer, function.source, function, env=self.env,
                                         conditions=self.conditions + ("iteration_may_be_empty",), depth=self.depth + 1)
                    if function.parameters:
                        base = slot(receiver, MAP_ENTRIES) if kind == "map" else receiver
                        nested.env[function.parameters[0][0]] = slot(base, ELEMENT)
                    if kind == "map" and len(function.parameters) > 1:
                        nested.env[function.parameters[1][0]] = slot(slot(receiver, MAP_KEYS), ELEMENT)
                    nested.statement(function.body)
                    self.facts.extend(nested.facts[:max(0, MAX_FACTS_PER_FUNCTION - len(self.facts))])
                    if method in {"map", "flatMap"}:
                        result = Value("allocation", self.identifier + ":mapped:" + str(node.start_byte))
                        self.analyzer.value_types[result] = "array"
                        return result
            return receiver if method == "filter" else UNKNOWN
        if method in {"sort", "sort_by", "reverse"}:
            return receiver
        if method in {"has", "contains", "contains_key", "indexOf", "len", "is_empty"}:
            return UNKNOWN
        return None
