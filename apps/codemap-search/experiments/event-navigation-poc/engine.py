"""Bounded source-derived function summaries and symbolic heap access paths.

Only language/standard-container primitives have built-in behavior. User methods
are expanded from their bodies. Unknown callees remain explicit boundaries.
"""

from pathlib import Path
from collections import defaultdict, deque
import hashlib
import re

from model import ELEMENT, MAP_ENTRIES, MAP_KEYS, UNKNOWN, Consumer, Fact, Summary, Value, match_storage, merge_values, referenced_values, slot, split_path, substitute, tuple_value, tuple_values, value_options
from syntax import CLASSES, FUNCTIONS, IDENTIFIERS, Function, Program, child, container_kind, element_type, lexical_bindings, rust_pattern_bindings, scoped_type_name, walk
from rust_values import any_downcast_call, map_constructor_call, pointer_call, pointer_cast, pointer_dereference, type_id_call
from rust_types import call_type_bindings, map_entry as rust_map_entry, substitute_type_text
from rust_macros import expansion_identity, expansion_location, statement_expansion_definition
from js_generators import GENERATOR_NODES, create_generator, generator_call


MAX_FACTS_PER_FUNCTION = 192
MAX_EXPRESSION_DEPTH = 48
MAX_SUMMARY_PASSES = 4
MAX_PROVENANCE_HOPS = 8
MAX_TOTAL_FACTS = 40000
MAX_PROTOTYPE_INSTANCES = 256
MAX_PROTOTYPE_GENERATIONS = 6


def closure_captures(value):
    if value.base.kind != "capture_binding":
        return dict(zip((key.name for key in tuple_values(value.key)), tuple_values(value.base)))
    result = {}
    binding = value.base
    while binding.kind == "capture_binding":
        result[binding.name] = binding.base
        binding = binding.key
    return result


class Analyzer:
    def __init__(self, program: Program):
        self.program = program
        self.summaries: dict[str, Summary] = {}
        self.globals: dict[str, dict[str, Value]] = {}
        self.value_types: dict[Value, str] = {}
        self.value_owners: dict[Value, str] = {}
        self.value_conditions: dict[Value, tuple[str, ...]] = {}
        self.heap_values: dict[Value, list[Value]] = {}
        self.heap_read_cache: dict[Value, list[Value]] = {}
        self.allocation_owners: dict[Value, str] = {}
        self.allocation_origins: dict[Value, str] = {}
        self.allocation_contexts: dict[Value, tuple] = {}
        self.captures: dict[str, dict[str, Value]] = {}
        self.notices: set[tuple[str, str]] = set()
        self.module_facts: list[Fact] = []
        self.literal_fields = {}
        self.prototypes = {}
        self.argument_type_hints = {}
        self.generator_frames = {}
        self.sources = {source.path: source for source in program.sources}

    def require_value_conditions(self, value, conditions):
        existing = self.value_conditions.get(value)
        required = set(conditions) if existing is None else set(existing).intersection(conditions)
        self.value_conditions[value] = tuple(sorted(required))

    def cap_facts(self, facts):
        if len(facts) <= MAX_TOTAL_FACTS:
            return facts
        self.notices.add(("total_fact_cap", str(len(facts) - MAX_TOTAL_FACTS)))
        semantic_kinds = {"store", "remove", "invoke", "member_invoke", "argument", "return", "key_lookup",
                          "parameter_binding", "copy_properties", "function_type_conversion"}
        semantic = [fact for fact in facts if fact.kind in semantic_kinds]
        if len(semantic) > MAX_TOTAL_FACTS:
            buckets = defaultdict(deque)
            for fact in semantic:
                root, _ = split_path(fact.target)
                buckets[(fact.location.path, fact.function, fact.kind, root.kind)].append(fact)
            active = deque(buckets.values())
            selected = []
            while active and len(selected) < MAX_TOTAL_FACTS:
                bucket = active.popleft()
                selected.append(bucket.popleft())
                if bucket:
                    active.append(bucket)
        else:
            selected = semantic + [fact for fact in facts if fact.kind not in semantic_kinds][:MAX_TOTAL_FACTS - len(semantic)]
        retained = set(selected)
        # Preserve the input order for heap alternatives. Later projections get
        # the same budget opportunity as original facts, including typed returns.
        return [fact for fact in facts if fact in retained]

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
        prototype = self.prototypes.get(value.base)
        if prototype is not None:
            found.extend(self.read_heap_values(slot(prototype, value.key), depth + 1, seen, budget))
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
                if not declared and owner:
                    declared = self.value_types.get(slot(Value("receiver", owner), value.key), "")
                type_text = declared or type_text
            if not type_text:
                base_type = self.type_text(value.base, source, depth + 1)
                namespace = str(Path(source.path).parent) if source.language == "go" else source.path
                base_type = self.program.aliases.get((namespace, base_type), base_type)
                is_entry = value.base.kind == "slot" and value.base.key == MAP_ENTRIES
                if value.key.kind != "key" or is_entry or value.key.name.isdecimal():
                    type_text = element_type(base_type)
        return type_text.lstrip(": ")

    def owner(self, value: Value, source) -> str:
        if value in self.value_owners:
            return self.value_owners[value]
        if value.kind in {"receiver", "type"}:
            return value.name
        type_text = self.value_types.get(value, "")
        scope_owner = ""
        if value.kind == "slot" and value.key.kind == "key":
            base_owner = self.owner(value.base, source)
            scope_owner = base_owner
            type_text = self.program.field_types.get((base_owner, value.key.name), "") or type_text
            source = self.sources.get(base_owner.rsplit("::", 1)[0], source)
        if not type_text and value.kind == "slot":
            type_text = self.type_text(value, source)
        return self.program.resolve_type(source, type_text, scope_owner)

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
            self.generator_frames = {}
            previous = self.summaries
            previous_module_facts = self.module_facts
            self.module_facts = []
            # Imports may point to modules later in path order. Revisit module
            # initializers with the same bounded summary generation, not only once.
            for source in self.program.sources:
                interpreter = Interpreter(self, source)
                # A module contains independent initializers, not one function.
                # Give each declaration the same bounded summary budget so an
                # earlier declaration cannot evict a later constructor binding.
                for declaration in source.tree.root_node.named_children:
                    interpreter.facts = []
                    # Module bindings are live while initializers execute.
                    # Local factory captures still belong to each closure value.
                    self.globals[source.path] = interpreter.env
                    interpreter.statement(declaration)
                    self.module_facts.extend(interpreter.facts)
                self.globals[source.path] = dict(interpreter.env)
            next_summaries = {}
            for function in self.program.functions.values():
                interpreter = Interpreter(self, function.source, function)
                if function.body is not None and ((function.source.language == "rust" and function.node.type == "closure_expression" and function.body.type != "block")
                                                 or (function.source.language in {"typescript", "tsx"} and function.node.type == "arrow_function" and function.body.type != "statement_block")):
                    interpreter.record_return(interpreter.expression(function.body), function.body)
                else:
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
        facts = self.cap_facts(facts)
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
                    destination = split_path(store.target)[0]
                    source_context = self.allocation_contexts.get(store.value)
                    destination_context = self.allocation_contexts.get(destination)
                    # A factory's field location is reused by distinct calls.
                    # Only explicit, distinct allocation contexts justify
                    # crossing that repeated source span again.
                    has_distinct_calls = bool(source_context and destination_context and source_context != destination_context
                                              and self.allocation_origins.get(store.value) == self.allocation_origins.get(destination))
                    if ((store.location in field.via or field.location == store.location) and not has_distinct_calls) or len(field.via) >= 6:
                        continue
                    target = substitute(field.target, {store.value: store.target})
                    projected = Fact("store", target, field.value, field.location, field.function,
                                     tuple(sorted(set(field.conditions + store.conditions))), field.via + (store.location,), field.argument_index)
                    added.append(projected)
                    if field.value in self.value_types:
                        self.value_types[target] = self.value_types[field.value]
            facts = list(dict.fromkeys(facts + added))
            facts = self.cap_facts(facts)
        self.refresh_heap(facts)
        facts += self.project_typed_receiver_stores(facts)
        self.refresh_heap(facts)
        facts += self.project_object_copies(facts)
        self.refresh_heap(facts)
        facts += self.project_prototype_bodies(facts)
        facts += self.project_return_schemas(facts)
        facts += self.project_stored_bindings(facts)
        facts += self.specialize_callback_arguments(facts)
        combined = list(dict.fromkeys(facts + self.forward_callback_arguments(facts)))
        return self.cap_facts(combined)

    def specialize_callback_arguments(self, facts):
        """Bind explicit calls through formal callback parameters to their bodies.

        This specializes source schemas, not factory allocations or execution
        order. A generator's arguments are bound at creation; its body still
        requires a later resume, which remains an explicit condition.
        """
        bindings, calls = {}, {}
        for fact in facts:
            if fact.kind == "parameter_binding":
                bindings.setdefault(fact.target, []).append(fact)
            elif fact.kind == "argument" and fact.value.kind == "parameter" and fact.argument_index is not None:
                calls.setdefault((fact.value, fact.location, fact.function), []).append(fact)
        def targets(formal, seen=()):
            if formal in seen or len(seen) >= 6:
                return []
            result = []
            for binding in bindings.get(formal, ()):
                actuals = value_options(binding.value) + self.read_values(binding.value)
                for actual in actuals:
                    if actual.kind in {"function", "closure"}:
                        result.append((actual, (binding,)))
                    elif actual.kind == "parameter":
                        result.extend((value, (binding,) + trail) for value, trail in targets(actual, seen + (formal,)))
            return result[:16]
        added, visited, call_nodes = [], set(), {}
        for (formal, location, owner), arguments in calls.items():
            formal_owner = formal.name.rsplit(":", 1)[0]
            enclosing = set()
            function = self.program.functions.get(owner)
            while function is not None:
                enclosing.add(function.identifier)
                function = function.parent
            if formal_owner not in enclosing:
                continue
            roots = {split_path(fact.target)[0] for fact in arguments}
            if any(root.kind in {"allocation", "binding"} and
                   (self.allocation_contexts.get(root) or "~" in root.name
                    or self.allocation_owners.get(root) not in enclosing) for root in roots):
                continue
            arity = max(fact.argument_index for fact in arguments) + 1
            supplied = [merge_values([fact.target for fact in arguments if fact.argument_index == index]) for index in range(arity)]
            source = self.sources[location.path]
            for callback, trail in targets(formal):
                signature = (callback, tuple(supplied), location, owner)
                if signature in visited:
                    continue
                if len(visited) >= 256:
                    self.notices.add(("callback_binding_summary_bound", "256"))
                    return added
                visited.add(signature)
                # Locate the actual call so expansion retains its source path.
                # Several generated macro calls may share one original location;
                # this schema-only pass cannot choose one of those calls by span.
                if source.path not in call_nodes:
                    call_nodes[source.path] = {source.location(node): node for node in walk(source.tree.root_node)
                                               if node.type == "call_expression" and not expansion_identity(source, node)}
                call_node = call_nodes[source.path].get(location)
                if call_node is None:
                    continue
                conditions = {"source_callable_parameter_binding", "enclosing_callable_execution_unproven", "enclosing_function_schema_only"}
                for fact in arguments + list(trail):
                    conditions.update(fact.conditions)
                interpreter = Interpreter(self, source, conditions=tuple(sorted(conditions)))
                interpreter.apply_source_callable(callback.name, UNKNOWN, supplied, call_node,
                                                  closure_captures(callback) if callback.kind == "closure" else None)
                for fact in interpreter.facts:
                    if fact.kind == "key_lookup" or (fact.kind in {"invoke", "member_invoke", "argument", "return"}
                                                     and any(split_path(fact.target)[0] == split_path(value)[0] for value in supplied)):
                        added.append(Fact(fact.kind, fact.target, fact.value, fact.location, fact.function,
                                          fact.conditions, tuple(dict.fromkeys(fact.via + tuple(item.location for item in trail))),
                                          fact.argument_index, fact.consumer))
        return list(dict.fromkeys(added))

    def project_stored_bindings(self, facts):
        """Keep identity of a lexical value even when its initializer is opaque."""
        registrations = {}
        for fact in facts:
            if fact.kind == "store" and fact.value.kind == "binding":
                registrations.setdefault(fact.value, []).append(fact)
        result = []
        for fact in facts:
            if fact.kind not in {"store", "invoke", "member_invoke", "key_lookup", "return"}:
                continue
            root, keys = split_path(fact.target)
            if not keys:
                continue
            for registration in registrations.get(root, ()):
                if registration.location == fact.location:
                    continue
                result.append(Fact(fact.kind, substitute(fact.target, {root: registration.target}), fact.value,
                                   fact.location, fact.function,
                                   tuple(sorted(set(fact.conditions + registration.conditions + ("explicit_stored_value_binding", "initializer_value_unresolved")))),
                                   tuple(dict.fromkeys(fact.via + registration.via + (registration.location,))), fact.argument_index, fact.consumer))
                if len(result) >= 4096:
                    self.notices.add(("stored_binding_projection_bound", "4096"))
                    return result
        return result

    def project_prototype_bodies(self, facts):
        """Instantiate source prototype methods on their constructed receivers.

        Like a class method summary, this records a possible method execution;
        constructing a carrier never proves that the method actually ran.
        """
        added, visited = [], set()
        origin_cache = {}
        def callable_origin(value, depth=0):
            if value in origin_cache:
                return origin_cache[value]
            function = self.program.functions.get(value.name)
            if function is None:
                return ""
            path = function.source.path
            if value.kind != "closure" or depth >= 6:
                return path
            captures = closure_captures(value)
            targets = []
            for part in walk(function.body):
                if part.type != "call_expression":
                    continue
                callee = child(part, "function")
                if callee is not None and callee.type == "member_expression" and function.source.text(child(callee, "property")) in {"call", "apply"}:
                    callee = child(callee, "object")
                if callee is None or callee.type != "identifier":
                    continue
                actual = captures.get(function.source.text(callee))
                if actual is not None and actual.kind in {"function", "closure"} and actual != value:
                    targets.append(callable_origin(actual, depth + 1))
            paths = set(targets)
            result = next(iter(paths)) if len(paths) == 1 else path
            origin_cache[value] = result
            return result
        for generation in range(MAX_PROTOTYPE_GENERATIONS):
            initial_count = len(added)
            stores = [fact for fact in facts + added if fact.kind == "store"]
            by_parent = {}
            for fact in stores:
                if fact.target.kind == "slot":
                    by_parent.setdefault(fact.target.base, []).append(fact)
            def has_stored_callable(value, depth=0):
                if value.kind in {"function", "closure"}:
                    return True
                if depth >= 3:
                    return False
                values = self.read_values(value) + [fact.value for fact in by_parent.get(value, ())]
                return any(has_stored_callable(item, depth + 1) for item in values)
            def binding_priority(pair):
                instance, _ = pair
                refs = {item for fact in by_parent.get(instance, ()) for item in referenced_values(fact.value) if item.kind == "slot"}
                concrete = sum(split_path(item)[0].kind in {"allocation", "receiver", "global"} for item in refs)
                return concrete, len(refs), len(self.allocation_contexts.get(instance, ()))
            groups = {}
            for pair in sorted(list(self.prototypes.items()), key=binding_priority, reverse=True):
                # Preserve file fairness through wrappers that call a captured
                # function. Captured data callables are not execution targets.
                callbacks = [value for fact in by_parent.get(pair[0], ()) for value in value_options(fact.value)
                             if value.kind in {"closure", "function"} and value.name in self.program.functions]
                path = callable_origin(callbacks[0]) if callbacks else ""
                groups.setdefault(path, []).append(pair)
            ordered = []
            while any(groups.values()):
                for group in groups.values():
                    if group:
                        ordered.append(group.pop(0))
            remaining = max(0, (MAX_PROTOTYPE_INSTANCES - len(visited)) // (MAX_PROTOTYPE_GENERATIONS - generation))
            processed = 0
            for instance, prototype in ordered:
                if processed >= remaining:
                    self.notices.add(("prototype_instance_summary_bound", str(MAX_PROTOTYPE_INSTANCES)))
                    break
                keys = {fact.target.key for fact in by_parent.get(prototype, ())}
                for key in sorted(keys, key=lambda value: value.display()):
                    methods = [value for value in self.read_values(slot(prototype, key)) if value.kind in {"function", "closure"}]
                    if len(methods) != 1 or (instance, key, methods[0]) in visited:
                        continue
                    method = methods[0]
                    function = self.program.functions.get(method.name)
                    if function is None:
                        continue
                    callback_keys = []
                    for part in walk(function.body, stop_functions=True):
                        if part.type == "subscript_expression" and function.source.text(child(part, "object")) == "this":
                            callback_keys.append(child(part, "index"))
                    if not callback_keys:
                        continue
                    interpreter = Interpreter(self, function.source)
                    if method.kind == "closure":
                        interpreter.env.update(closure_captures(method))
                    has_callback = any(has_stored_callable(slot(instance, interpreter.expression(index)))
                                       for index in callback_keys if index is not None)
                    if not has_callback:
                        continue
                    if len(visited) >= MAX_PROTOTYPE_INSTANCES:
                        self.notices.add(("prototype_instance_summary_bound", str(MAX_PROTOTYPE_INSTANCES)))
                        return added
                    visited.add((instance, key, method))
                    processed += 1
                    interpreter.conditions += ("prototype_method_execution_required", "source_constructor_prototype_binding")
                    interpreter.apply_source_callable(method.name, instance, [], function.node,
                                                      closure_captures(method) if method.kind == "closure" else None)
                    added.extend(interpreter.facts)
            added = list(dict.fromkeys(added))
            self.refresh_heap(facts + added)
            if len(added) == initial_count:
                break
        return added

    def project_return_schemas(self, facts):
        """Compare a typed read with original receiver writes, one schema hop.

        No parameter writes or allocation-derived bindings become type-wide
        identities. Distinct parameter paths remain distinct in the raw facts.
        """
        result = []
        schema_roots = {split_path(fact.target)[0] for fact in facts if fact.kind == "store"
                        and split_path(fact.target)[0].kind == "receiver"}
        for fact in facts:
            if fact.kind != "return":
                continue
            root, _ = split_path(fact.target)
            if root.kind != "parameter":
                continue
            source = self.sources[fact.location.path]
            owner = self.owner(root, source)
            schema = Value("receiver", owner)
            if not owner or schema not in schema_roots:
                continue
            result.append(Fact("return", substitute(fact.target, {root: schema}), fact.value, fact.location, fact.function,
                               tuple(sorted(set(fact.conditions + ("typed_parameter_receiver_binding_required", "declared_receiver_schema_only")))), fact.via))
        return result

    def project_typed_receiver_stores(self, facts):
        """Project an original schema write onto one explicit opaque binding.

        Keep the consuming object's complete field path and the binding's
        provenance. Derived writes and allocations never become type aliases.
        """
        result = []
        sources = {source.path: source for source in self.program.sources}
        bindings = {}
        schemas = {}
        for fact in facts:
            if fact.kind != "store":
                continue
            bindings.setdefault(fact.target, []).append(fact)
            root, _ = split_path(fact.target)
            function = self.program.functions.get(fact.function)
            if root.kind == "receiver" and function and function.owner == root.name and not fact.via:
                schemas.setdefault(root.name, []).append(fact)
        for prefix, stores in bindings.items():
            root, _ = split_path(prefix)
            if root.kind != "receiver" or prefix.kind != "slot" or not all(store.value.kind in {"result", "unknown", "unresolved"} for store in stores):
                continue
            source = sources[stores[0].location.path]
            if source.language not in {"typescript", "tsx"}:
                continue
            owner = self.owner(prefix, source)
            for store in schemas.get(owner, ()):
                for binding in stores:
                    target = substitute(store.target, {Value("receiver", owner): prefix})
                    obligations = ("opaque_receiver_binding_required", "declared_receiver_schema_only", "method_dispatch_unproven")
                    result.append(Fact("store", target, store.value, store.location, store.function,
                                       tuple(sorted(set(store.conditions + binding.conditions + obligations))),
                                       tuple(dict.fromkeys(store.via + binding.via + (binding.location,)))))
        return result

    def explicit_object_keys(self, value):
        if value.kind != "allocation":
            return None
        origin = Value("allocation", self.allocation_origins.get(value, value.name))
        initial = self.literal_fields.get(origin)
        if initial is None:
            return None
        keys = set(initial)
        for target in self.heap_values:
            if target.kind == "slot" and target.base == value:
                if target.key.kind != "key":
                    return None
                keys.add(target.key.name)
        return keys

    def copied_property_paths(self, source, suffix, excluded):
        paths = [(source, ())]
        for index, requested in enumerate(suffix):
            next_paths = []
            for value, keys in paths:
                known = self.explicit_object_keys(value)
                choices = [requested]
                if known is not None and requested.kind not in {"key", "literal"}:
                    choices = [Value("key", name) for name in sorted(known)]
                for key in choices:
                    if index == 0 and key in excluded:
                        continue
                    if known is not None and key.kind == "key" and key.name not in known:
                        continue
                    target = slot(value, key)
                    actuals = self.read_values(target) or [target]
                    next_paths.extend((actual, keys + (key,)) for actual in actuals)
            paths = list(dict.fromkeys(next_paths))[:16]
        return list(dict.fromkeys(keys for _, keys in paths))

    def project_object_copies(self, facts):
        copies = {}
        for fact in facts:
            if fact.kind == "copy_properties":
                copies.setdefault(fact.target, []).append(fact)
        calls = {}
        for fact in facts:
            if fact.kind in {"invoke", "member_invoke", "argument"}:
                calls.setdefault(split_path(fact.target)[0], []).append(fact)
        result = []
        for store in facts:
            if store.kind != "store" or store.value not in copies:
                continue
            root, keys = split_path(store.target)
            for call in calls.get(root, []):
                _, called_keys = split_path(call.target)
                suffix = called_keys[len(keys):]
                if not suffix or len(suffix) > 3:
                    continue
                conditions = match_storage(store.target, call.target)
                if conditions is None:
                    continue
                for copy_index, copied in enumerate(copies[store.value]):
                    excluded = set(tuple_values(copied.value.key))
                    for later in copies[store.value][copy_index + 1:]:
                        later_keys = self.explicit_object_keys(later.value.base)
                        if later_keys is not None:
                            excluded.update(Value("key", name) for name in later_keys)
                    source = copied.value.base
                    for path in self.copied_property_paths(source, suffix, excluded):
                        target, value = store.target, source
                        for key in path:
                            target, value = slot(target, key), slot(value, key)
                        obligations = ("own_enumerable_property_required", "copied_property_not_overridden", "nested_member_presence_required", "object_copy_registration_projection")
                        result.append(Fact("store", target, value, store.location, store.function,
                                           tuple(sorted(set(store.conditions + copied.conditions + conditions + obligations))),
                                           tuple(dict.fromkeys(store.via + copied.via + (copied.location,))),
                                           consumer=Consumer(call.location, call.target, call.kind, call.argument_index)))
                        if len(result) + len(facts) >= MAX_TOTAL_FACTS:
                            self.notices.add(("object_copy_projection_cap", str(MAX_TOTAL_FACTS)))
                            return result
        return result


class Interpreter:
    def __init__(self, analyzer: Analyzer, source, function: Function | None = None,
                 env=None, conditions=(), depth=0, rust_type_bindings=None):
        self.analyzer = analyzer
        self.program = analyzer.program
        self.source = source
        self.function = function
        self.rust_type_bindings = dict(rust_type_bindings or {})
        self.identifier = function.identifier if function else "module:" + source.path
        self.env = dict(env if env is not None else analyzer.globals.get(source.path, {}))
        self.env.update(analyzer.captures.get(self.identifier, {}))
        self.conditions = tuple(conditions)
        if function and expansion_identity(source, function.node):
            self.conditions += ("source_declarative_impl_expansion", "macro_template_typing_unproven")
        if getattr(source, "has_type_projection", False):
            self.conditions += ("generic_type_constraints_unproven",)
        if function and function.parent:
            self.conditions += ("enclosing_callable_execution_unproven",)
        self.depth = depth
        self.instance_context = ""
        self.facts: list[Fact] = []
        self.returns: list[Value] = []
        self.match_returns = {}
        self.parameters = []
        self.local_bindings = lexical_bindings(source, function.body) if function else set()
        if function:
            if source.language in {"typescript", "tsx"} and function.node.type != "arrow_function":
                self.env["arguments"] = Value("arguments", function.identifier)
            parameter_node = child(function.node, "parameters") or child(function.node, "parameter")
            for name, type_text in function.parameters:
                if source.language == "rust":
                    type_text = substitute_type_text(type_text, self.rust_type_bindings)
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
                self.analyzer.value_owners[function.receiver] = function.owner
            if function.name == "constructor" and parameter_node:
                for parameter in parameter_node.named_children:
                    if any(part.type == "accessibility_modifier" for part in parameter.named_children):
                        pattern = child(parameter, "pattern") or child(parameter, "name")
                        if pattern is not None:
                            name = self.text(pattern)
                            self.emit("store", slot(function.receiver, name), self.env.get(name, UNKNOWN), parameter)

    def text(self, node):
        return self.source.text(node)

    def allocation_name(self, node):
        location = self.source.location(node)
        return f"{self.source.path}:{location.line}:{location.column}" + expansion_identity(self.source, node) + self.instance_context

    def emit(self, kind, target, value, node, extra=(), via=(), argument_index=None):
        macro_location = expansion_location(self.source, node)
        if macro_location is not None:
            via = tuple(via) + (macro_location,)
        definition = statement_expansion_definition(self.source, node)
        if definition is not None:
            via = tuple(via) + (definition,)
            extra = tuple(extra) + ("source_declarative_statement_expansion", "macro_template_typing_unproven")
        dependencies = {condition for item in (target, value) for referenced in referenced_values(item)
                        for condition in self.analyzer.value_conditions.get(referenced, ())}
        fact = Fact(kind, target, value, self.source.location(node), self.identifier,
                    tuple(sorted(set(self.conditions + tuple(extra)).union(dependencies))), tuple(via), argument_index)
        self.append_fact(fact)

    def append_fact(self, fact):
        if fact in self.facts:
            return
        if len(self.facts) >= MAX_FACTS_PER_FUNCTION:
            self.analyzer.notices.add(("function_fact_cap", self.identifier))
            priorities = {"store": 0, "invoke": 0, "member_invoke": 0, "remove": 1, "return": 2, "argument": 3}
            def priority(item):
                return (bool(item.via), priorities.get(item.kind, 4))
            victim = max(range(len(self.facts)), key=lambda index: priority(self.facts[index]))
            if priority(fact) >= priority(self.facts[victim]):
                return
            self.facts.pop(victim)
        self.facts.append(fact)

    def binding(self, name):
        if name in self.env:
            return self.env[name]
        if self.source.language == "rust" and "::" in name:
            prefix, method = name.rsplit("::", 1)
            receiver = self.env.get(prefix)
            owner = self.program.resolve_type(self.source, prefix)
            if receiver is not None and receiver.kind == "type":
                return slot(receiver, method)
            if owner:
                return slot(Value("type", owner), method)
            namespace = self.program.imports.get((self.source.path, prefix), "")
            if namespace and "#" not in namespace:
                candidates = self.program.methods.get((namespace, method), ())
                if len(candidates) == 1:
                    return Value("function", candidates[0])
        import_path = self.program.import_paths.get((self.source.path, name))
        if import_path and name not in self.local_bindings:
            if import_path == "reflect" and self.source.language == "go":
                return Value("import", "go:reflect")
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
                if self.source.language in {"typescript", "tsx"}:
                    value = Value("binding", self.identifier + ":" + str(pattern.start_byte) + ":" + self.text(pattern) + self.instance_context)
                    self.analyzer.allocation_owners[value] = self.identifier
                else:
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
        elif pattern.type == "tuple_struct_pattern" and self.rust_standard_path(
                self.text(child(pattern, "type")), {"std::option::Option::Some", "core::option::Option::Some"}, "Some"):
            payloads = [item.base for item in value_options(value) if item.kind == "wrapper" and item.name == "rust:option"]
            bindings = [part for part in pattern.named_children if part != child(pattern, "type")]
            if payloads and len(bindings) == 1:
                self.conditions += ("option_some_required",)
                self.bind(bindings[0], merge_values(payloads))
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
            if self.source.language in {"typescript", "tsx"}:
                name = scoped_type_name(self.source, node)
            receiver = Value("receiver", self.program.owner_key(self.source, name))
            body = child(node, "body")
            for field in body.named_children if body else []:
                if field.type == "public_field_definition":
                    base = Value("global", receiver.name + "::static") if any(part.type == "static" for part in field.children) else receiver
                    self.analyzer.value_owners[base] = receiver.name
                    target = slot(base, self.text(child(field, "name")))
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
            if statement_expansion_definition(self.source, node) is not None:
                self.analyzer.require_value_conditions(value, ("source_declarative_statement_expansion", "macro_template_typing_unproven"))
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
            for value in values:
                self.record_return(value, node)
            return
        if kind == "match_expression" and self.source.language == "rust":
            self.expression(node)
            return
        if kind == "if_expression" and self.source.language == "rust" and child(node, "condition") is not None and child(node, "condition").type in {"let_condition", "let_chain"}:
            self.rust_if_let(node)
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
            if tail.type not in {"let_declaration", "return_expression"} and (tail.type != "expression_statement" or not self.text(tail).rstrip().endswith(";")):
                value = self.expression(tail)
                if value != UNKNOWN:
                    self.record_return(value, tail)

    def rust_if_let(self, node):
        """Pattern bindings belong to the successful branch, including shadows."""
        base_env, old_conditions = dict(self.env), self.conditions
        condition = child(node, "condition")
        conditions = list(condition.named_children) if condition.type == "let_chain" else [condition]
        bound_names = set()
        self.conditions += (f"conditional_control:{self.source.path}:{self.source.location(node).line}", "rust_let_pattern_match_required")
        for part in conditions:
            if part.type == "let_condition":
                pattern = child(part, "pattern")
                value = self.expression(child(part, "value"))
                names = rust_pattern_bindings(self.source, pattern)
                bound_names.update(names)
                # A failed/unknown destructure cannot read an outer variable
                # with the same spelling through the new pattern binding.
                self.env.update({name: UNKNOWN for name in names})
                self.bind(pattern, value)
            else:
                self.expression(part)
        self.statement(child(node, "consequence"))
        success = dict(self.env)
        self.env = dict(base_env)
        self.conditions = old_conditions + (f"conditional_control:{self.source.path}:{self.source.location(node).line}", "rust_let_pattern_not_matched")
        self.statement(child(node, "alternative"))
        failure = dict(self.env)
        self.env = {name: value if (success.get(name, value) if name not in bound_names else value) == value
                    and failure.get(name, value) == value else UNKNOWN for name, value in base_env.items()}
        self.conditions = old_conditions

    def record_return(self, value, node):
        if value == UNKNOWN:
            return
        self.returns.append(value)
        if self.source.language not in {"rust", "typescript", "tsx"}:
            return
        if self.source.language in {"typescript", "tsx"}:
            if self.analyzer.kind(value, self.source) != "array":
                return
            if value.kind == "allocation":
                origins = [fact.value for fact in self.facts if fact.kind == "store"
                           and fact.target == slot(value, ELEMENT) and fact.value.kind == "slot"]
                for origin in origins:
                    self.emit("return", origin, UNKNOWN, node, ("returned_array_element_membership_required",))
                return
        def leaves(item):
            if item.kind == "tuple":
                return [leaf for part in tuple_values(item) for leaf in leaves(part)]
            if item.kind == "choice":
                return [leaf for part in value_options(item) for leaf in leaves(part)]
            if item.kind == "wrapper" and item.name == "rust:option":
                return leaves(item.base)
            return [item]
        expression = node.named_children[0] if node.type == "expression_statement" and node.named_children else node
        origins = self.match_returns.get(expression.start_byte, [(value, node)]) if expression.type == "match_expression" else [(value, node)]
        for returned, origin in origins:
            for item in leaves(returned):
                if item.kind == "slot":
                    self.emit("return", item, UNKNOWN, origin, ("match_arm_selection_required",) if expression.type == "match_expression" else ())

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

    def expression(self, node, level=0, is_read=True) -> Value:
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
            if kind == "template_string" and "${" in text:
                substitutions = list(re.finditer(r"\$\{([A-Za-z_$][\w$]*)\}", text))
                if substitutions and len(substitutions) == text.count("${"):
                    values = {match[1]: self.binding(match[1]) for match in substitutions}
                    if all(value.kind in {"key", "literal"} for value in values.values()):
                        return Value("key", re.sub(r"\$\{([A-Za-z_$][\w$]*)\}", lambda match: values[match[1]].name, text[1:-1]))
            if "${" in text or "\\" in text:
                return Value("dynamic_key", self.identifier + ":" + str(node.start_byte))
            return Value("key", text.strip("'\"`"))
        if kind in {"number", "integer_literal", "float_literal", "int_literal", "true", "false", "null", "nil"}:
            return Value("literal", self.text(node))
        if self.source.language == "rust" and kind == "range_expression":
            return Value("range", self.identifier + ":" + self.text(node))
        if self.source.language == "rust" and kind == "match_expression":
            return self.match_value(node, level)
        if kind in FUNCTIONS:
            function = self.program.nodes.get((self.source.path, node.start_byte))
            if function:
                if self.source.language in {"typescript", "tsx"} and function.parent:
                    return self.capture_callable(function)
                self.analyzer.captures[function.identifier] = ({} if self.source.language in {"typescript", "tsx"} else dict(self.env))
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
            if base.kind == "wrapper" and base.name == "javascript:iterator_result":
                if self.text(field_node) == "value":
                    return base.base
                if self.text(field_node) == "done":
                    return base.key
            if base.kind == "tuple" or base.kind == "tuple_end":
                if self.text(field_node) == "length":
                    return Value("literal", str(len(tuple_values(base))))
            if base.kind == "namespace" and self.source.language in {"typescript", "tsx"}:
                exported = self.program.exports.get((base.name, self.text(field_node)))
                if exported and "#" not in exported:
                    return Value("namespace", exported)
                if exported:
                    namespace, name = exported.rsplit("#", 1)
                    actual = self.analyzer.globals.get(namespace, {}).get(name)
                    if actual is not None:
                        return actual
                    functions = self.program.methods.get((namespace, name), [])
                    if len(functions) == 1:
                        return Value("function", functions[0])
                return slot(base, self.text(field_node))
            if base.kind == "type" and self.source.language in {"typescript", "tsx"}:
                base = Value("global", base.name + "::static")
                self.analyzer.value_owners[base] = self.text(base_node) and self.program.resolve_type(self.source, self.text(base_node))
            if is_read and self.source.language in {"typescript", "tsx"}:
                owner = self.analyzer.owner(base, self.source)
                getters = self.program.getters.get((owner, self.text(field_node)), [])
                if getters:
                    return self.apply_summary(getters[0], base, [], node) if len(getters) == 1 else UNKNOWN
            return slot(base, self.text(field_node))
        if kind in {"subscript_expression", "index_expression"}:
            base_node = child(node, "object") or child(node, "operand") or child(node, "value")
            index_node = child(node, "index")
            parts = node.named_children
            base = expr(base_node or (parts[0] if parts else None))
            index_value = expr(index_node or (parts[-1] if len(parts) > 1 else None))
            if base.kind in {"tuple", "tuple_end"} and index_value.kind == "literal" and index_value.name.isdecimal():
                values = tuple_values(base)
                return values[int(index_value.name)] if int(index_value.name) < len(values) else UNKNOWN
            if self.source.language == "rust":
                base = self.dereferenced_receiver(base, "index" if is_read else "index_mut", node)
            if self.source.language == "go" and self.analyzer.kind(base, self.source) == "map":
                base = slot(base, MAP_ENTRIES)
            return slot(base, index_value)
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
            if self.source.language == "rust" and kind == "try_expression":
                value = expr(part)
                if value.kind == "wrapper" and value.name == "rust:option":
                    self.conditions += ("option_some_required",)
                    return value.base
                return value
            if self.source.language == "rust" and kind == "unary_expression" and self.text(node).startswith("*"):
                value = expr(part)
                pointed = pointer_dereference(self, value)
                # References remain ordinary aliases; an unknown raw pointer
                # must not become the object denoted by its address expression.
                if pointed is not None:
                    return pointed
                if self.analyzer.type_text(value, self.source).strip().startswith("*"):
                    self.analyzer.notices.add(("pointer_origin_unresolved", self.identifier))
                    return UNKNOWN
                return value
            return expr(part)
        if self.source.language == "rust" and kind == "type_cast_expression":
            value = expr(child(node, "value"))
            pointed = pointer_cast(self, node, value)
            return pointed if pointed is not None else UNKNOWN
        if self.source.language == "rust" and kind == "unsafe_block":
            block = next((part for part in node.named_children if part.type == "block"), None)
            return expr(block)
        if kind in {"assignment_expression", "augmented_assignment_expression"}:
            left, right = child(node, "left"), child(node, "right")
            value = expr(right)
            if left and left.type in IDENTIFIERS:
                self.bind(left, value)
            else:
                target = self.expression(left, level + 1, is_read=False)
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
            value = Value("allocation", self.allocation_name(node))
            self.analyzer.allocation_owners[value] = self.identifier
            type_arguments = self.text(child(node, "type_arguments"))
            intrinsic = {"Map": "map", "Set": "set", "Array": "array"}.get(type_name)
            self.analyzer.value_types[value] = type_name + type_arguments if type_arguments else (intrinsic or type_name)
            owner = self.program.resolve_type(self.source, type_name)
            if owner:
                self.analyzer.value_owners[value] = owner
            arguments = [expr(argument) for argument in (child(node, "arguments").named_children if child(node, "arguments") else [])]
            if self.source.language in {"typescript", "tsx"}:
                callable_value = self.expression(type_node, level + 1)
                if callable_value.kind in {"function", "closure"}:
                    self.apply_source_callable(callable_value.name, value, arguments, node,
                                               closure_captures(callable_value) if callable_value.kind == "closure" else None)
                    prototypes = self.analyzer.read_values(slot(callable_value, "prototype"))
                    if len(prototypes) == 1:
                        self.analyzer.prototypes[value] = prototypes[0]
                    return value
            constructors = self.program.methods.get((owner, "constructor"), []) if owner else []
            if len(constructors) == 1 and constructors[0] in self.analyzer.summaries:
                returned = self.apply_summary(constructors[0], value, arguments, node)
                if returned != UNKNOWN and self.analyzer.owner(returned, self.source):
                    return returned
            return value
        if kind in {"call_expression", "macro_invocation"}:
            return self.call(node, level)
        if kind in {"conditional_expression", "ternary_expression"} and self.source.language in {"typescript", "tsx"}:
            condition = expr(child(node, "condition"))
            yes, no = child(node, "consequence"), child(node, "alternative")
            if condition == Value("literal", "true"):
                return expr(yes)
            if condition == Value("literal", "false"):
                return expr(no)
            return merge_values([expr(yes), expr(no)])
        if kind in {"binary_expression", "conditional_expression"}:
            values = [expr(part) for part in node.named_children]
            operator = self.text(child(node, "operator"))
            if operator in {"===", "!==", "==", "!="} and len(values) == 2 and all(value.kind == "literal" for value in values):
                equal = values[0] == values[1]
                return Value("literal", str(equal if operator in {"===", "=="} else not equal).lower())
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
            previous = len(self.returns)
            self.statement(node)
            return self.returns[-1] if len(self.returns) > previous else UNKNOWN
        if kind == "if_expression" and self.source.language == "rust" and child(node, "condition") is not None and child(node, "condition").type in {"let_condition", "let_chain"}:
            previous = len(self.returns)
            self.rust_if_let(node)
            return merge_values(self.returns[previous:])
        for part in node.named_children:
            expr(part)
        return UNKNOWN

    def capture_callable(self, function):
        free_names = {self.text(part) for part in walk(function.body) if part.type in {"identifier", "this"}}
        bound_names = lexical_bindings(self.source, function.body) | {name for name, _ in function.parameters}
        enclosing_names = {"this"}
        parent = function.parent
        while parent is not None:
            enclosing_names.update(name for name, _ in parent.parameters)
            enclosing_names.update(lexical_bindings(parent.source, parent.body))
            if parent.node.type != "arrow_function":
                enclosing_names.add("arguments")
            parent = parent.parent
        captures = {name: self.env[name] for name in sorted((free_names - bound_names) & enclosing_names) if name in self.env}
        self.analyzer.captures[function.identifier] = captures
        if len(captures) > 32:
            self.analyzer.notices.add(("closure_capture_cap", function.identifier))
            return UNKNOWN
        identity = Value("allocation", function.identifier + ":closure" + self.instance_context)
        self.analyzer.allocation_owners[identity] = self.identifier
        captures = {**captures, "__source_callable_identity__": identity}
        if len(captures) > 8:
            bindings = Value("capture_end")
            for name, value in reversed(list(captures.items())):
                bindings = Value("capture_binding", name, value, bindings)
            return Value("closure", function.identifier, bindings)
        return Value("closure", function.identifier, tuple_value(list(captures.values())),
                     tuple_value([Value("key", name) for name in captures]))

    def match_value(self, node, level):
        value = self.expression(child(node, "value"), level + 1)
        arms = child(node, "body")
        if arms is None:
            return UNKNOWN
        original_env, original_conditions = dict(self.env), self.conditions
        values = []
        origins = []
        for arm in arms.named_children:
            if arm.type != "match_arm":
                continue
            self.env = dict(original_env)
            self.conditions = original_conditions + ("match_arm_selection_required",)
            pattern = child(arm, "pattern")
            if pattern is not None and pattern.type == "match_pattern" and pattern.named_children:
                pattern = pattern.named_children[0]
            if pattern is not None and pattern.type == "tuple_struct_pattern" and self.text(child(pattern, "type")) == "Some":
                options = [option.base for option in value_options(value) if option.kind == "wrapper" and option.name == "rust:option"]
                if not options:
                    continue
                bindings = [part for part in pattern.named_children if part != child(pattern, "type")]
                if len(bindings) == 1:
                    self.bind(bindings[0], merge_values(options))
            elif self.text(pattern) == "None":
                values.append(Value("wrapper", "rust:option", UNKNOWN))
                continue
            returned = self.expression(child(arm, "value"), level + 1)
            values.append(returned)
            origins.append((returned, child(arm, "value")))
        self.env, self.conditions = original_env, original_conditions
        self.match_returns[node.start_byte] = origins
        return merge_values(values)

    def rust_standard_path(self, path, canonical_paths, prelude_name=None):
        """Require a canonical import/path, or an unshadowed explicit prelude name."""
        root, *rest = path.lstrip(":").split("::")
        if root in self.local_bindings or root in self.env:
            return False
        ancestor = self.function.node if self.function else None
        while ancestor is not None:
            parameters = child(ancestor, "type_parameters")
            if parameters is not None and any(self.text(child(item, "name")) == root for item in parameters.named_children):
                return False
            ancestor = ancestor.parent
        local_items = {self.text(child(item, "name")) for item in self.source.tree.root_node.named_children
                       if item.type in {"struct_item", "enum_item", "type_item", "trait_item", "mod_item", "function_item"}}
        if root in local_items:
            return False
        imported = self.program.import_paths.get((self.source.path, root))
        resolved = "::".join([imported or root] + rest)
        if resolved in canonical_paths:
            return True
        if path != prelude_name or root in self.source.module_bindings or (self.source.path, root) in self.program.imports:
            return False
        return not any("*" in self.text(item) for item in self.source.tree.root_node.named_children if item.type == "use_declaration")

    def rust_value_call(self, receiver, method, arguments, node):
        if not method:
            return None
        actuals = value_options(receiver) + [option for value in self.analyzer.read_values(receiver) for option in value_options(value)]
        boxes = [value for value in actuals if value.kind == "allocation" and self.analyzer.value_types.get(value) == "std::boxed::Box<_>"]
        if boxes and method in {"as_ref", "as_mut"} and not arguments:
            self.conditions += ("standard_box_reference_semantics",)
            return slot(receiver, "pointee")
        options = [value for value in actuals if value.kind == "wrapper" and value.name == "rust:option"]
        if options and method in {"unwrap", "expect", "unwrap_or_default", "map"}:
            value = merge_values([option.base for option in options])
            self.conditions += ("option_some_required",)
            if method == "map" and len(arguments) == 1 and arguments[0].kind == "function":
                returned = self.apply_summary(arguments[0].name, UNKNOWN, [value], node)
                return Value("wrapper", "rust:option", returned)
            if method in {"unwrap", "expect"}:
                return value
            if method == "unwrap_or_default" and all(item.kind == "wrapper" and item.name == "rust:slice_view" for item in value_options(value)):
                return value
        views = [value for value in actuals if value.kind == "wrapper" and value.name == "rust:slice_view"]
        if views and method in {"iter", "iter_mut"}:
            self.conditions += ("slice_range_membership_required",)
            return Value("iterator", "values", merge_values(views))
        iterators = [value for value in actuals if value.kind == "iterator"]
        if iterators and method == "chain" and len(arguments) == 1:
            others = [item for item in value_options(arguments[0]) if item.kind == "iterator"]
            if others:
                return Value("iterator", "values", merge_values([item.base for item in iterators + others]))
        if iterators and method == "next" and not arguments:
            elements = []
            for iterator in iterators:
                for base in value_options(iterator.base):
                    if base.kind == "wrapper" and base.name == "rust:slice_view":
                        base = base.base
                        self.conditions += ("slice_range_membership_required",)
                    elements.append(slot(base, ELEMENT))
            self.conditions += ("iterator_may_be_empty", "stored_iterator_binding_required")
            return Value("wrapper", "rust:option", merge_values(elements))
        return None

    def object_value(self, node, level):
        type_node = child(node, "type") or child(node, "name")
        type_text = self.text(type_node)
        value = Value("allocation", self.allocation_name(node))
        self.analyzer.allocation_owners[value] = self.identifier
        if node.type in {"array", "array_expression"}:
            self.analyzer.value_types[value] = "array"
            for part in node.named_children:
                self.emit("store", slot(value, ELEMENT), self.expression(part, level + 1), part)
            return value
        self.analyzer.value_types[value] = type_text
        owner = self.function.owner if type_text == "Self" and self.function else self.program.resolve_type(self.source, type_text)
        if owner:
            self.analyzer.value_owners[value] = owner
        body = child(node, "body")
        fields = body.named_children if body else node.named_children
        if node.type == "object" and all(field.type in {"shorthand_property_identifier", "comment"}
                                        or (field.type == "pair" and child(field, "key") is not None
                                            and child(field, "key").type in IDENTIFIERS | {"string", "number"}) for field in fields):
            self.analyzer.literal_fields[value] = {self.text(child(field, "key") or field).strip("'\"") for field in fields if field.type != "comment"}
        for field_index, field in enumerate(fields):
            if field.type == "method_definition" and self.source.language in {"typescript", "tsx"}:
                function = self.program.nodes.get((self.source.path, field.start_byte))
                named = child(field, "name")
                if function is not None and named is not None and named.type in IDENTIFIERS | {"computed_property_name"}:
                    callback = self.capture_callable(function)
                    key = self.expression(named.named_children[0], level + 1) if named.type == "computed_property_name" and named.named_children else self.text(named)
                    self.emit("store", slot(value, key), callback, field)
                continue
            if field.type == "spread_element" and self.source.language in {"typescript", "tsx"}:
                copied = self.expression(field, level + 1)
                excluded = []
                for later in fields[field_index + 1:]:
                    if later.type in {"pair", "shorthand_property_identifier"}:
                        key_node = child(later, "key") or later
                        if key_node.type in IDENTIFIERS | {"string", "number"}:
                            excluded.append(Value("key", self.text(key_node).strip("'\"")))
                self.emit("copy_properties", value, Value("object_copy", base=copied, key=tuple_value(excluded)), field)
                continue
            if field.type in {"pair", "field_initializer", "keyed_element"}:
                key_node = child(field, "key") or child(field, "field")
                field_value = child(field, "value")
                if key_node is None and field.named_children:
                    key_node = field.named_children[0]
                if field_value is None and len(field.named_children) > 1:
                    field_value = field.named_children[-1]
                if key_node is not None and key_node.type == "literal_element" and key_node.named_children:
                    key_node = key_node.named_children[0]
                key = (self.expression(key_node.named_children[0], level + 1) if key_node is not None and key_node.type == "computed_property_name" and key_node.named_children
                       else self.text(key_node).strip("'\""))
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
        bound_receiver = UNKNOWN
        if self.source.language == "rust" and callee_node is not None and callee_node.type == "generic_function":
            callee_text = self.text(child(callee_node, "function"))
        if self.source.language in {"typescript", "tsx"}:
            actuals = [callee] if callee.kind in {"function", "closure"} else self.analyzer.read_values(callee)
            known = list(dict.fromkeys(value for value in actuals if value.kind in {"function", "closure"}))
            if len(known) == 1 and known[0].kind == "closure":
                if callee.kind == "slot":
                    self.emit("invoke", callee, UNKNOWN, node)
                callback = known[0]
                captures = closure_captures(callback)
                receiver = callee.base if callee.kind == "slot" else UNKNOWN
                return self.apply_source_callable(callback.name, receiver=receiver, arguments=arguments, node=node, captures=captures)
            if len(known) == 1 and callee.kind != "function":
                if callee.kind == "slot":
                    self.emit("invoke", callee, UNKNOWN, node)
                    bound_receiver = callee.base
                callee = known[0]
        if self.source.language == "rust":
            type_id = type_id_call(self, callee_text, callee_node, arguments)
            if type_id is not None:
                return type_id
            constructed_map = map_constructor_call(self, callee_text, arguments, node)
            if constructed_map is not None:
                return constructed_map
            receiver = callee.base if callee.kind == "slot" else UNKNOWN
            method = callee.key.name if callee.kind == "slot" and callee.key.kind == "key" else ""
            pointed = pointer_call(self, callee_text, receiver, method, arguments, node)
            if pointed is not None:
                return pointed
            downcast = any_downcast_call(self, receiver, method, arguments, callee_node)
            if downcast is not None:
                return downcast
            if callee.kind == "type" and callee.name in self.program.tuple_fields:
                if len(arguments) != len(self.program.tuple_fields[callee.name]):
                    return UNKNOWN
                value = Value("allocation", self.allocation_name(node) + ":tuple_struct")
                self.analyzer.allocation_owners[value] = self.identifier
                self.analyzer.value_owners[value] = callee.name
                for index, argument in enumerate(arguments):
                    self.emit("store", slot(value, str(index)), argument, node)
                return value
            if len(arguments) == 1 and self.rust_standard_path(callee_text, {"std::option::Option::Some", "core::option::Option::Some"}, "Some"):
                return Value("wrapper", "rust:option", arguments[0])
            if len(arguments) == 1 and self.rust_standard_path(callee_text, {"std::boxed::Box::new", "alloc::boxed::Box::new"}):
                value = Value("allocation", self.allocation_name(node) + ":box")
                self.analyzer.allocation_owners[value] = self.identifier
                self.analyzer.value_types[value] = "std::boxed::Box<_>"
                self.emit("store", slot(value, "pointee"), arguments[0], node, ("standard_box_constructor",))
                return value
            if callee.kind == "allocation" and self.analyzer.value_types.get(callee) == "std::boxed::Box<_>":
                callee = slot(callee, "pointee")
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
        receiver = callee.base if callee.kind == "slot" else bound_receiver
        method = callee.key.name if callee.kind == "slot" and callee.key.kind == "key" else ""
        if self.source.language in {"typescript", "tsx"}:
            resumed = generator_call(self, receiver, method, arguments, node)
            if resumed is not None:
                return resumed
            if method in {"call", "apply"} and receiver.kind in {"function", "closure"} and arguments and not self.analyzer.heap_values.get(callee):
                supplied = arguments[1:] if method == "call" else (tuple_values(arguments[1]) if len(arguments) == 2 and arguments[1].kind in {"tuple", "tuple_end"} else None)
                if supplied is not None:
                    return self.apply_source_callable(receiver.name, arguments[0], supplied, node,
                                                      closure_captures(receiver) if receiver.kind == "closure" else None)
            if (method == "defineProperty" and receiver == Value("unresolved", self.source.path + ":Object")
                    and "Object" not in self.env and "Object" not in self.local_bindings and "Object" not in self.source.module_bindings
                    and len(arguments) == 3):
                self.emit("store", slot(arguments[0], arguments[1]), slot(arguments[2], "value"), node,
                          ("standard_object_property_definition",))
                return arguments[0]
        if self.source.language == "rust":
            standard = self.rust_value_call(receiver, method, arguments, node)
            if standard is not None:
                return standard
        if (self.source.language in {"typescript", "tsx"} and method == "get"
                and receiver == Value("unresolved", self.source.path + ":Reflect")
                and "Reflect" not in self.env and "Reflect" not in self.local_bindings
                and "Reflect" not in self.source.module_bindings
                and len(arguments) == 2 and arguments[1].kind == "key"):
            self.conditions += ("builtin_reflect_get_semantics_required",)
            return slot(arguments[0], arguments[1])
        if self.source.language == "go":
            if receiver == Value("import", "go:reflect") and method == "ValueOf" and len(arguments) == 1:
                return Value("wrapper", "go:reflect.Value", arguments[0])
            if receiver.kind == "wrapper" and receiver.name == "go:reflect.Value" and method == "Call" and len(arguments) == 1:
                self.emit("invoke", receiver.base, UNKNOWN, node, ("reflect_value_must_be_callable",))
                return UNKNOWN
        if self.source.language == "rust" and method == "as_ref" and not arguments:
            type_text = self.program.expand_type(self.source, self.analyzer.type_text(receiver, self.source))
            boxed = re.match(r"^(?:(::)?(std|alloc)::boxed::)?Box\s*<\s*dyn\s+(?:std::ops::)?Fn\s*\(", type_text)
            if boxed:
                qualifier = boxed.group(2)
                has_shadow = (qualifier or "Box") in self.source.module_bindings
                if not has_shadow and "*" not in " ".join(self.text(part) for part in self.source.tree.root_node.named_children if part.type == "use_declaration"):
                    self.conditions += ("standard_box_reference_semantics",)
                    return receiver
        if self.source.language == "rust":
            receiver = self.dereferenced_receiver(receiver, method, node)
            if method:
                callee = slot(receiver, method)
        kind = self.analyzer.kind(receiver, self.source)
        has_source_method = self.source.language == "rust" and bool(self.program.methods.get((self.analyzer.owner(receiver, self.source), method)))
        if kind and not has_source_method:
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
            target = self.program.functions.get(target_ids[0])
            if self.source.language == "rust" and target:
                type_bindings = call_type_bindings(self, target, callee_node, arguments)
                if type_bindings or any(value.kind == "tuple" for value in arguments):
                    return self.apply_source_callable(target_ids[0], receiver, arguments, node, rust_type_bindings=type_bindings)
            if self.source.language in {"typescript", "tsx"} and bound_receiver != UNKNOWN:
                return self.apply_source_callable(target_ids[0], bound_receiver, arguments, node)
            if self.source.language in {"typescript", "tsx"} and (target and target.node.type in GENERATOR_NODES
                    or any(value.kind == "wrapper" and value.name == "javascript:generator" for value in arguments)):
                return self.apply_source_callable(target_ids[0], receiver, arguments, node)
            if self.source.language == "rust" and (any(value.kind == "wrapper" and value.name in {"rust:nonnull", "rust:raw_pointer", "rust:maybeuninit_initialized"} for value in arguments)
                                                    or (receiver.kind == "allocation" and self.analyzer.owner(receiver, self.source) in self.program.tuple_fields)):
                return self.apply_source_callable(target_ids[0], receiver, arguments, node)
            if self.source.language in {"typescript", "tsx"} and any(value.kind in {"function", "closure", "binding"}
                    or any(stored.kind in {"function", "closure"} for stored in self.analyzer.read_values(value))
                    for value in arguments):
                return self.apply_source_callable(target_ids[0], receiver, arguments, node)
            returned = self.apply_summary(target_ids[0], UNKNOWN if receiver.kind == "type" else receiver, arguments, node)
            if returned != UNKNOWN:
                return returned
            return Value("result", f"{self.source.path}:{self.source.location(node).line}:{self.source.location(node).column}" + expansion_identity(self.source, node))
        if callee.kind in {"slot", "parameter"}:
            is_member = callee_node is not None and callee_node.type in {"member_expression", "field_expression", "selector_expression"}
            declared = self.program.expand_type(self.source, self.analyzer.type_text(callee, self.source))
            if declared.startswith(("func(", "unsafe fn", "fn(")) or "=>" in declared:
                is_member = False
            self.emit("member_invoke" if is_member else "invoke", callee, UNKNOWN, node)
        else:
            self.emit("external_boundary", callee, UNKNOWN, node)
        return Value("result", f"{self.source.path}:{self.source.location(node).line}:{self.source.location(node).column}" + expansion_identity(self.source, node))

    def apply_source_callable(self, target_id, receiver, arguments, node, captures=None, rust_type_bindings=None):
        """Bind actual callable arguments before interpreting a higher-order body.

        Captures belong to the returned value, not the last factory invocation.
        Ordinary first-order calls still use bounded reusable summaries.
        """
        function = self.program.functions.get(target_id)
        if function is None or target_id == self.identifier or self.depth >= 6:
            self.analyzer.notices.add(("callable_context_bound", target_id))
            return UNKNOWN
        nested = Interpreter(self.analyzer, function.source, function, conditions=self.conditions,
                             depth=self.depth + 1, rust_type_bindings=rust_type_bindings)
        if rust_type_bindings:
            nested.conditions += ("source_generic_type_argument_binding", "generic_trait_constraints_unproven")
        nested.env.update(captures or {})
        identity = (captures or {}).get("__source_callable_identity__")
        identity_parts = [identity.display()] if identity is not None else []
        if self.instance_context:
            identity_parts.append(self.instance_context)
        if receiver != UNKNOWN and function.node.type != "arrow_function":
            identity_parts.append(receiver.display())
        if identity_parts:
            nested.instance_context = "~closure:" + hashlib.sha256("\n".join(identity_parts).encode()).hexdigest()[:16]
        for (name, declared), actual in zip(function.parameters, arguments):
            nested.env[name] = actual
            if actual.kind in {"function", "closure", "parameter", "slot"}:
                formal = Value("parameter", function.identifier + ":" + name)
                self.emit("parameter_binding", formal, actual, node,
                          ("generator_execution_required",) if function.node.type in GENERATOR_NODES else ())
            if actual.kind == "binding" and declared:
                owner = self.program.resolve_type(function.source, declared.lstrip(": "))
                if owner:
                    hints = self.analyzer.argument_type_hints.setdefault(actual, set())
                    hints.add(owner)
                    if len(hints) == 1:
                        self.analyzer.value_types[actual] = declared.lstrip(": ")
                        self.analyzer.value_owners[actual] = owner
                        nested.conditions += ("declared_argument_type_required",)
                    else:
                        self.analyzer.value_types.pop(actual, None)
                        self.analyzer.value_owners.pop(actual, None)
                        self.analyzer.notices.add(("argument_type_hint_conflict", actual.display()))
        parameter_node = child(function.node, "parameters")
        for parameter in parameter_node.named_children if parameter_node else []:
            pattern = child(parameter, "pattern") or child(parameter, "name")
            if pattern is not None and pattern.type == "rest_pattern" and pattern.named_children:
                name = function.source.text(pattern.named_children[-1])
                index = next((index for index, item in enumerate(function.parameters) if item[0] == name), len(arguments))
                nested.env[name] = tuple_value(arguments[index:])
        if function.node.type != "arrow_function":
            nested.env["arguments"] = tuple_value(arguments)
        if receiver != UNKNOWN and function.node.type != "arrow_function":
            nested.env[function.receiver_name] = receiver
        if function.node.type in GENERATOR_NODES:
            return create_generator(self, nested, function, node)
        if function.body is not None and function.node.type == "arrow_function" and function.body.type != "statement_block":
            nested.record_return(nested.expression(function.body), function.body)
        else:
            nested.statement(function.body)
        facts = [Fact(fact.kind, fact.target, fact.value, fact.location, fact.function,
                      tuple(sorted(set(fact.conditions + ("source_callable_argument_binding",)))),
                      fact.via, fact.argument_index, fact.consumer) for fact in nested.facts]
        bound = Summary(function.name, function.source.path, function.owner, UNKNOWN, (), facts, nested.returns)
        return self.expand_summary(target_id, bound, UNKNOWN, [], node)

    def dereferenced_receiver(self, receiver, method, node):
        mutable = {"push", "push_back", "insert", "clear", "remove", "retain", "pop", "pop_front", "iter_mut", "get_mut", "index_mut"}
        readable = {"get", "iter", "len", "is_empty", "index"}
        if method not in mutable | readable:
            return receiver
        owner = self.analyzer.owner(receiver, self.source)
        if not owner or self.program.methods.get((owner, method)):
            return receiver
        mode = "mutable" if method in mutable else "shared"
        candidates = self.program.dereferences.get((owner, mode), ())
        if len(candidates) != 1:
            return receiver
        summary = self.analyzer.summaries.get(candidates[0])
        if summary is None or len(summary.returns) != 1:
            return receiver
        target = substitute(summary.returns[0], {summary.receiver: receiver})
        if target.kind != "slot" or split_path(target)[0] != split_path(receiver)[0]:
            return receiver
        # Only a source-returned, declared standard container is modeled here.
        # Arbitrary trait dispatch and a user's similarly named type stay opaque.
        declaration = self.program.functions[candidates[0]]
        if not self.analyzer.kind(target, declaration.source) or self.analyzer.owner(target, declaration.source):
            return receiver
        projected = self.apply_summary(candidates[0], receiver, [], node)
        if projected != target:
            return receiver
        self.conditions += ("source_deref_body_applied", "dereference_execution_unproven")
        self.emit("source_dereference", target, receiver, node, via=(declaration.source.location(declaration.node),))
        return target

    def apply_summary(self, target_id, receiver, arguments, node, capture_replacements=None):
        summary = self.analyzer.summaries.get(target_id)
        if summary is None or target_id == self.identifier:
            return UNKNOWN
        return self.expand_summary(target_id, summary, receiver, arguments, node, capture_replacements)

    def expand_summary(self, target_id, summary, receiver, arguments, node, capture_replacements=None):
        replacements = dict(zip(summary.parameters, arguments))
        replacements[Value("arguments", target_id)] = tuple_value(arguments)
        replacements.update(capture_replacements or {})
        if summary.receiver != UNKNOWN and receiver != UNKNOWN:
            replacements[summary.receiver] = receiver
        semantic_kinds = {"store", "remove", "invoke", "member_invoke", "argument", "return", "key_lookup", "function_type_conversion", "copy_properties", "parameter_binding"}
        selected = [fact for fact in summary.facts if fact.kind in semantic_kinds]
        values = set()
        for value in list(summary.returns) + [value for fact in selected for value in (fact.target, fact.value)]:
            values.update(referenced_values(value))
        location = self.source.location(node)
        context_key = (location, expansion_identity(self.source, node))
        for value in sorted(values, key=lambda item: item.display()):
            if value.kind not in {"allocation", "binding"} or self.analyzer.allocation_owners.get(value) != target_id:
                continue
            context = self.analyzer.allocation_contexts.get(value, ())
            if context_key in context or len(context) >= 6:
                replacements[value] = UNKNOWN
                self.analyzer.notices.add(("allocation_context_depth_bound", target_id))
                continue
            origin = self.analyzer.allocation_origins.get(value, value.name)
            context = context + (context_key,)
            suffix = ">".join(f"{item.path}:{item.line}:{item.column}{macro}" for item, macro in context)
            allocated = Value(value.kind, origin + "@" + suffix)
            replacements[value] = allocated
            self.analyzer.allocation_owners[allocated] = self.identifier
            self.analyzer.allocation_origins[allocated] = origin
            self.analyzer.allocation_contexts[allocated] = context
            if value in self.analyzer.value_types:
                self.analyzer.value_types[allocated] = self.analyzer.value_types[value]
            if value in self.analyzer.value_owners:
                self.analyzer.value_owners[allocated] = self.analyzer.value_owners[value]
            if value in self.analyzer.prototypes:
                self.analyzer.prototypes[allocated] = substitute(self.analyzer.prototypes[value], replacements)
        for value in values:
            if value in self.analyzer.value_conditions:
                self.analyzer.require_value_conditions(substitute(value, replacements), self.analyzer.value_conditions[value])
        for fact in selected:
            if len(fact.via) >= MAX_PROVENANCE_HOPS or location in fact.via:
                continue
            projected = Fact(fact.kind, substitute(fact.target, replacements), substitute(fact.value, replacements),
                             fact.location, fact.function, tuple(sorted(set(fact.conditions + self.conditions))),
                             fact.via + (location,), fact.argument_index)
            self.append_fact(projected)
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
        if self.source.language == "rust" and kind == "array" and method in {"get", "get_mut"} and arguments:
            index = arguments[0]
            if index.kind == "range":
                self.conditions += ("slice_range_membership_required",)
                return Value("wrapper", "rust:option", Value("wrapper", "rust:slice_view", receiver, index))
            return Value("wrapper", "rust:option", slot(receiver, index))
        if kind == "map" and method in {"get", "get_mut", "remove", "delete"} and arguments:
            target = (rust_map_entry(self, receiver, arguments[0]) if self.source.language == "rust"
                      else slot(slot(receiver, MAP_ENTRIES), arguments[0]))
            if method in {"remove", "delete"}:
                self.emit("remove", target, UNKNOWN, node)
                return UNKNOWN
            if arguments[0].kind in {"slot", "parameter"}:
                self.emit("key_lookup", arguments[0], receiver, node, ("map_entry_presence_required",))
            if self.source.language == "rust":
                return Value("wrapper", "rust:option", target)
            return target
        if kind == "map" and method in {"set", "insert"} and len(arguments) >= 2:
            target = (rust_map_entry(self, receiver, arguments[0]) if self.source.language == "rust"
                      else slot(slot(receiver, MAP_ENTRIES), arguments[0]))
            self.emit("store", target, arguments[1], node)
            self.emit("store", slot(slot(receiver, MAP_KEYS), ELEMENT), arguments[0], node)
            return receiver if method == "set" else UNKNOWN
        if kind in {"array", "set"} and method in {"push", "push_back", "add", "insert"} and arguments:
            for value in arguments:
                self.emit("store", slot(receiver, ELEMENT), value, node)
            return receiver if method == "add" and self.source.language in {"typescript", "tsx"} else UNKNOWN
        if kind == "array" and self.source.language in {"typescript", "tsx"} and method == "pop" and not arguments:
            self.conditions += ("array_nonempty_required", "stack_selection_and_order_required")
            self.emit("remove", slot(receiver, ELEMENT), UNKNOWN, node)
            return slot(receiver, ELEMENT)
        if method in {"clear", "delete", "remove", "splice", "retain", "pop", "pop_front"}:
            target = slot(receiver, MAP_ENTRIES) if kind == "map" else slot(receiver, ELEMENT)
            self.emit("remove", target, UNKNOWN, node)
            return UNKNOWN
        if method in {"values", "iter", "iter_mut", "into_iter"}:
            mode = "values" if method == "values" or kind != "map" else "pairs"
            return Value("iterator", mode, receiver)
        if method in {"forEach", "map", "filter", "find", "some", "every", "flatMap", "for_each"} and arguments:
            callback = arguments[0]
            if callback.kind in {"function", "closure"} and self.depth < 6:
                function = self.program.functions.get(callback.name)
                if function:
                    nested = Interpreter(self.analyzer, function.source, function, env=self.env,
                                         conditions=self.conditions + ("iteration_may_be_empty",), depth=self.depth + 1)
                    if callback.kind == "closure":
                        nested.env.update(closure_captures(callback))
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
