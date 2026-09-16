"""Bounded language adapters into the existing storage/navigation fact model.

No repository names, event API names, expected endpoints, or fixture labels are
inputs to this module. Receivers are type schemas, never proven object identity.
"""

from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from model import (ELEMENT, MAP_ENTRIES, MAP_KEYS, UNKNOWN, Fact, Value,
                   match_storage, merge_values, referenced_values, slot, split_path, substitute, tuple_value, tuple_values, value_options)
from syntax import Source
from grammars import parser_for

IDENTIFIERS = {"identifier", "simple_identifier", "field_identifier", "property_identifier",
               "name", "variable_name", "constant", "type_identifier"}
CLASSES = {"class_declaration", "class_definition", "class", "module", "struct_specifier",
           "class_specifier", "object_definition", "trait_definition", "interface_declaration", "package_object"}
FUNCTIONS = {"function_definition", "function_declaration", "method_declaration", "method",
             "constructor_declaration", "singleton_method", "function_item", "init_declaration"}
LAMBDAS = {"lambda", "lambda_expression", "lambda_literal", "anonymous_function",
           "anonymous_function_creation_expression", "arrow_function", "function_expression",
           "lambda_function", "closure", "lambda_literal", "anonymous_method_expression"}
CALLS = {"call", "call_expression", "function_call", "function_call_expression",
         "method_invocation", "member_call_expression", "nullsafe_member_call_expression",
         "invocation_expression", "scoped_call_expression"}
MEMBERS = {"attribute", "field_access", "field_expression", "member_access_expression",
           "nullsafe_member_access_expression", "navigation_expression", "dot_index_expression",
           "method_index_expression", "conditional_access_expression", "member_binding_expression"}
ASSIGNMENTS = {"assignment", "assignment_expression", "assignment_statement",
               "augmented_assignment", "operator_assignment", "reference_assignment_expression"}
DECLARATIONS = {"variable_declarator", "init_declarator", "var_definition", "val_definition",
                "property_declaration", "initialized_identifier", "local_variable_declaration",
                "initialized_variable_definition", "pattern_variable_declaration"}
PARAMS = {"parameter", "parameter_declaration", "formal_parameter", "simple_parameter",
          "typed_parameter", "default_parameter", "typed_default_parameter", "optional_parameter"}
LOOPS = {"for_statement", "for_in_statement", "foreach_statement", "enhanced_for_statement",
         "for_generic_clause", "for_expression"}
BLOCKS = {"block", "compound_statement", "statement_block", "body_statement", "function_body",
          "statements", "program", "module", "source_file", "compilation_unit", "translation_unit", "chunk", "indented_block"}
MAX_FACTS = 256
MAX_TOTAL_FACTS = 40000


def parts(node):
    return list(node.named_children) if node is not None else []


def child(node, *names):
    if node is not None:
        for name in names:
            result = node.child_by_field_name(name)
            if result is not None:
                return result
    return None


def walk(node, should_stop_functions=False):
    if node is None:
        return
    yield node
    for item in parts(node):
        if not should_stop_functions or item.type not in FUNCTIONS | LAMBDAS:
            yield from walk(item, should_stop_functions)


def first(node, kinds):
    return next((item for item in parts(node) if item.type in kinds), None)


def syntax_errors(source):
    # Missing punctuation can be anonymous; named_children alone silently
    # reports zero errors for a tree whose root.has_error is true.
    pending = [source.tree.root_node]
    errors = []
    while pending:
        node = pending.pop()
        if node.type == "ERROR" or node.is_missing:
            errors.append(source.location(node))
        pending.extend(reversed(node.children))
    return errors


def identifier(node):
    if node is None:
        return None
    if node.type in IDENTIFIERS:
        return node
    target = child(node, "declarator", "bound_identifier", "pattern", "name")
    if target is not None and target != node:
        found = identifier(target)
        if found is not None:
            return found
    return next((item for item in walk(node) if item.type in IDENTIFIERS), None)


def scope_bindings(source, node):
    """Conservative lexical bindings, including assignments after a use."""
    names = set()
    for item in parts(node):
        if item.type in FUNCTIONS | CLASSES:
            named = child(item, "name")
            if named is not None:
                names.add(source.text(named))
            continue
        if item.type in LAMBDAS:
            continue
        if "import" in item.type:
            # Extra names only disable an intrinsic; they never invent one.
            names.update(re.findall(r"[A-Za-z_]\w*", source.text(item)))
            continue
        if item.type in ASSIGNMENTS | DECLARATIONS:
            target = child(item, "left", "target", "name", "pattern")
            if target is not None and target.type not in MEMBERS:
                names.update(source.text(part) for part in walk(target) if part.type in IDENTIFIERS)
        names.update(scope_bindings(source, item))
    return names


def type_kind(text):
    clean = text.replace(" ", "")
    if clean.rstrip("?").startswith("[") and clean.rstrip("?").endswith("]"):
        return "map" if ":" in clean else "array"
    if re.search(r"\b(?:Map|HashMap|Dictionary|dict|Hash|TreeMap|ConcurrentHashMap|MutableMap)\b", clean):
        return "map"
    if re.search(r"\b(?:Set|HashSet|MutableSet)\b", clean):
        return "set"
    if re.search(r"\b(?:List|Array|ArrayList|MutableList|vector|Vec|CopyOnWriteArrayList|array)\b", clean) or "[]" in clean:
        return "array"
    return ""


@dataclass
class Callable:
    source: Source
    node: object
    body: object
    name: str
    owner: str
    parameters: list
    identifier: str

    @property
    def receiver(self):
        return Value("receiver", self.owner) if self.owner else UNKNOWN


@dataclass
class Summary:
    parameters: list = field(default_factory=list)
    facts: list = field(default_factory=list)
    returns: list = field(default_factory=list)


class Program:
    def __init__(self, sources):
        self.sources = sources
        self.functions = []
        self.methods = defaultdict(list)
        self.getters = defaultdict(list)
        self.fields = defaultdict(dict)
        self.events = set()
        self.globals = {}
        self.global_initializers = []
        self.sam_methods = {}
        self.initializers = []
        self.types = {}
        self.owner_sources = {}
        self.unproven_type_sources = set()
        self.aliases = {}
        self.namespaces = {}
        self.imports = {}
        self.namespace_imports = defaultdict(set)
        self.notices = []
        self.class_parameters = {}
        self.case_classes = set()
        self.module_bindings = {source.path: scope_bindings(source, source.tree.root_node) for source in sources}
        for source in sources:
            namespace = ""
            for node in parts(source.tree.root_node):
                if node.type in {"package_declaration", "package_header", "package_clause", "namespace_definition",
                                 "namespace_declaration", "file_scoped_namespace_declaration"}:
                    name = child(node, "name")
                    namespace = source.text(name) or re.sub(r"^(?:package|namespace)\s+|[;{].*$", "", source.text(node)).strip()
                if node.type in {"import_declaration", "import_header"}:
                    text = source.text(node).removeprefix("import ").strip().rstrip(";").strip()
                    selector = re.fullmatch(r"([\w.]+)\.\{([^}]+)\}", text)
                    if source.language == "scala" and selector:
                        for name in selector[2].split(","):
                            name = name.strip()
                            if re.fullmatch(r"[A-Za-z_]\w*", name):
                                self.imports[(source.path, name)] = selector[1] + "." + name
                    elif source.language == "scala" and text.endswith("._"):
                        self.namespace_imports[source.path].add(text[:-2])
                    else:
                        self.imports[(source.path, text.rsplit(".", 1)[-1])] = text
                if source.language == "csharp" and node.type == "using_directive":
                    text = source.text(node).removeprefix("using ").strip().rstrip(";").strip()
                    if "=" not in text and not text.startswith("static "):
                        self.namespace_imports[source.path].add(text)
            self.namespaces[source.path] = namespace
            self.collect(source, source.tree.root_node)
        from scala_semantics import ScalaSemantics
        self.scala = ScalaSemantics(self)
        from jvm_semantics import JvmSemantics
        self.jvm = JvmSemantics(self)

    def owner(self, source, name):
        namespace = self.namespaces.get(source.path, "") or source.path
        return source.language + ":" + namespace + "::" + name

    def declared_owner(self, source, type_text, scope="", depth=0):
        if depth > 6:
            return ""
        clean = re.sub(r"\b(?:const|struct|class|volatile|ref|out|in|final|typename)\b|[?*&]", "", type_text).strip().lstrip(": ")
        exact = {owner for (path, name), owner in self.types.items()
                 if path == source.path and re.sub(r"\s+", "", name) == re.sub(r"\s+", "", clean)}
        if len(exact) == 1:
            return exact.pop()
        qualified_alias = re.fullmatch(r"(.+)(?:::|\.)([A-Za-z_]\w*)", clean)
        if qualified_alias:
            prefix, member = qualified_alias.groups()
            parent = self.declared_owner(source, prefix, scope, depth + 1)
            if (parent, member) in self.aliases:
                return self.declared_owner(self.owner_sources.get(parent, source), self.aliases[(parent, member)], parent, depth + 1)
        clean = clean.split("<", 1)[0].split("[", 1)[0].strip()
        alias = self.aliases.get((scope, clean))
        if alias:
            return self.declared_owner(source, alias, scope, depth + 1)
        if not re.fullmatch(r"[\w.]+", clean):
            return ""
        if (source.path, clean) in self.types:
            return self.types[(source.path, clean)]
        namespace = self.namespaces.get(source.path, "")
        imported = self.imports.get((source.path, clean))
        candidates = {owner for (path, name), owner in self.types.items()
                      if (imported and self.namespaces.get(path, "") + "." + name == imported)
                      or (namespace and self.namespaces.get(path) == namespace and name == clean)
                      or ("." in clean and self.namespaces.get(path, "") + "." + name == clean)
                      or any(prefix + "." + clean == self.namespaces.get(path, "") + "." + name
                             for prefix in self.namespace_imports[source.path])}
        if len(candidates) == 1:
            return candidates.pop()
        if source.language == "swift":
            candidates = {owner for (path, name), owner in self.types.items()
                          if name == clean and owner.startswith("swift:")}
            if len(candidates) == 1:
                owner = candidates.pop()
                self.unproven_type_sources.update((source.path, self.owner_sources[owner].path))
                return owner
        if source.language in {"c", "cpp"} and clean not in {"int", "char", "void", "size_t", "bool", "double", "float", "long"}:
            # A declared C pointer type supplies a translation-unit receiver
            # schema even when macros hide its fields. This is not alias proof.
            return self.owner(source, clean)
        return ""

    def expanded_type(self, source, type_text, scope=""):
        value = type_text.strip().lstrip(": ")
        seen = set()
        for _ in range(8):
            name = re.sub(r"\b(?:const|struct|class|volatile|ref|out|in|final|typename)\b|[?*&]", "", value).strip()
            key = (scope, name)
            if key in seen or key not in self.aliases:
                break
            seen.add(key)
            value = self.aliases[key]
        return value

    def collect(self, source, node, owner=""):
        if node.type in CLASSES:
            name_node = child(node, "name") or first(node, {"type_identifier", "identifier", "constant"})
            if name_node is not None:
                name = source.text(name_node)
                owner = self.owner(source, name)
                self.owner_sources[owner] = source
                self.types[(source.path, name)] = owner
                if source.language == "scala" and node.type == "class_definition":
                    parameters = child(node, "class_parameters")
                    self.class_parameters[owner] = [(source.text(child(parameter, "name")), source.text(child(parameter, "type")),
                                                     child(parameter, "default_value"), parameter) for parameter in parts(parameters)]
                    if re.search(r"\bcase\s+class\b", source.text(node)[:name_node.start_byte - node.start_byte]):
                        self.case_classes.add(owner)
                    for parameter_name, type_text, _, parameter in self.class_parameters[owner]:
                        if owner in self.case_classes or re.search(r"\b(?:val|var)\b", source.text(parameter)):
                            self.fields[owner][parameter_name] = type_text
                if "<" in name:
                    self.types.setdefault((source.path, name.split("<", 1)[0]), owner)
            body = child(node, "body") or first(node, {"class_body", "template_body", "declaration_list", "field_declaration_list"})
            if body is not None:
                if source.language == "java" and node.type == "interface_declaration" and child(node, "interfaces") is None:
                    methods = [source.text(child(item, "name")) for item in parts(body)
                               if item.type == "method_declaration" and child(item, "body") is None
                               and not re.search(r"\b(?:static|default)\b", source.text(item))]
                    if len(methods) == 1:
                        self.sam_methods[owner] = methods[0]
                self.collect_body(source, body, owner)
                return
        if node.type in FUNCTIONS:
            self.add_function(source, node, child(node, "body") or first(node, {"function_body", "block"}), owner)
            return
        if source.language == "c" and not owner and node.type == "declaration":
            if any(part.type == "storage_class_specifier" and source.text(part) == "static" for part in parts(node)):
                for index, item in enumerate(node.children):
                    if node.field_name_for_child(index) != "declarator":
                        continue
                    declarator = child(item, "declarator") if item.type == "init_declarator" else item
                    if declarator.type == "function_declarator" and child(declarator, "declarator").type == "identifier":
                        continue
                    named = identifier(declarator)
                    if named is None:
                        continue
                    target = slot(Value("global", source.path + "::static"), source.text(named))
                    self.globals[(source.path, source.text(named))] = target
                    if child(item, "value") is not None:
                        self.global_initializers.append((source, target, child(item, "value"), node))
        if node.type in {"alias_declaration", "type_alias_declaration", "typealias_declaration"}:
            named = child(node, "name")
            value = child(node, "type", "value")
            if source.language == "swift" and node.type == "typealias_declaration":
                names = [item for index, item in enumerate(node.children) if node.field_name_for_child(index) == "name"]
                if len(names) == 2:
                    named, value = names
            if named is not None and value is not None:
                self.aliases[(owner, source.text(named))] = source.text(value)
        for item in parts(node):
            self.collect(source, item, owner)

    def collect_body(self, source, body, owner):
        children = parts(body)
        for index, node in enumerate(children):
            if source.language == "csharp" and node.type == "property_declaration":
                value = child(node, "value")
                named = child(node, "name")
                if value is not None and value.type == "arrow_expression_clause" and named is not None:
                    name = source.text(named)
                    self.fields[owner][name] = source.text(child(node, "type"))
                    self.add_function(source, node, value, owner, name, True)
                    continue
            if source.language == "kotlin" and node.type == "getter" and index and children[index - 1].type == "property_declaration":
                named = identifier(first(children[index - 1], {"variable_declaration"}))
                if named is not None:
                    self.add_function(source, node, first(node, {"function_body"}), owner, source.text(named), True)
                continue
            # Dart exposes a method signature and body as adjacent siblings.
            if node.type in {"method_signature", "function_signature"} and index + 1 < len(children) and children[index + 1].type == "function_body":
                self.add_function(source, node, children[index + 1], owner)
                continue
            if node.type == "function_body" and index and children[index - 1].type in {"method_signature", "function_signature"}:
                continue
            if node.type in {"field_declaration", "property_declaration", "declaration", "var_definition", "val_definition", "event_field_declaration"}:
                is_event = node.type == "event_field_declaration"
                if is_event and any(source.text(part) == "static" for part in parts(node)):
                    self.notices.append({"kind": "static_event_storage_unsupported", "path": source.path})
                    continue
                type_node = child(node, "type") or first(node, {"type_annotation", "function_type", "user_type"})
                if type_node is None:
                    type_node = child(first(node, {"variable_declaration"}), "type")
                type_text = source.text(type_node)
                candidates = []
                for item in walk(node, should_stop_functions=True):
                    if item.type in {"variable_declarator", "property_element", "initialized_identifier", "variable_declaration"}:
                        if source.language == "csharp" and item.type == "variable_declaration":
                            continue
                        named = child(item, "name") or first(item, IDENTIFIERS)
                        if named is not None:
                            value = child(item, "value")
                            if item.type == "initialized_identifier" and len(parts(item)) > 1:
                                value = parts(item)[-1]
                            elif item.type == "variable_declaration" and source.language == "kotlin" and parts(node) and parts(node)[-1] != item:
                                value = parts(node)[-1]
                            if value is not None and value.type == "getter":
                                value = None
                            candidates.append((named, value))
                named = child(node, "name", "pattern")
                if named is not None:
                    candidates.append((identifier(named), child(node, "value")))
                for index, item in enumerate(node.children):
                    if node.field_name_for_child(index) == "declarator":
                        candidates.append((identifier(item), child(item, "value")))
                for named, value in candidates:
                    if named is None:
                        continue
                    name = source.text(named).lstrip("$")
                    self.fields[owner][name] = type_text
                    if is_event:
                        self.events.add((owner, name))
                    if value is not None and value != named:
                        self.initializers.append((source, owner, name, value, node))
                    getter = first(node, {"getter"})
                    if getter is not None:
                        self.add_function(source, getter, first(getter, {"function_body"}), owner, name, True)
            self.collect(source, node, owner)

    def add_function(self, source, node, body, owner, name_override=None, is_getter=False):
        if body is None:
            return
        signature = first(node, {"function_signature", "constructor_signature"}) or node
        declarator = child(signature, "declarator")
        name_node = child(signature, "name") or identifier(declarator) or first(signature, IDENTIFIERS)
        name = name_override or source.text(name_node)
        if source.language == "swift" and node.type == "init_declaration":
            name = "init"
        if source.language == "lua" and name_node is not None and name_node.type in {"method_index_expression", "dot_index_expression"}:
            owner = self.owner(source, source.text(child(name_node, "table")))
            name = source.text(child(name_node, "method", "field"))
        if not name:
            name = "anonymous@" + str(node.start_byte)
        parameter_node = child(signature, "parameters") or child(declarator, "parameters") or first(signature, {"formal_parameters", "formal_parameter_list", "function_value_parameters", "parameter_list", "parameters", "method_parameters"})
        parameter_parts = parts(parameter_node) if parameter_node is not None else [item for item in parts(signature) if item.type == "parameter"]
        if source.language == "scala":
            parameter_parts = [parameter for group in parts(signature) if group.type == "parameters" for parameter in parts(group)]
        parameters = []
        for item in parameter_parts:
            if item.type in {"comment", "line_comment"}:
                continue
            named = child(item, "name", "pattern", "declarator")
            if named is None:
                named = item if item.type in IDENTIFIERS else first(item, IDENTIFIERS)
            named = identifier(named)
            if named is not None:
                type_node = child(item, "type")
                if type_node is None:
                    type_node = next((part for part in parts(item) if "type" in part.type), None)
                parameters.append((source.text(named), source.text(type_node), "*" in source.text(item)))
        key = source.path + ":" + str(node.start_byte)
        function = Callable(source, node, body, name, owner, parameters, key)
        self.functions.append(function)
        (self.getters if is_getter else self.methods)[(owner or source.path, name)].append(function)


class Analyzer:
    def __init__(self, program, passes=4):
        self.program = program
        self.passes = passes
        self.summaries = {}
        self.kinds = {}
        self.owners = {}
        self.value_types = {}
        self.value_conditions = defaultdict(set)
        self.alias_cache = {}
        self.notices = set()
        self.heap = defaultdict(list)
        self.heap_facts = defaultdict(list)
        self.heap_by_root = defaultdict(list)
        self.closures = {}

    def kind(self, value, depth=0):
        if value in self.kinds:
            return self.kinds[value]
        info = self.type_info(value)
        kind = type_kind(self.program.expanded_type(*info)) if info else ""
        if kind or depth >= 3 or not any(source.language == "scala" for source in self.program.sources):
            return kind
        candidates = {self.kinds.get(alias, "") for alias in self.resolved_aliases(value) if alias != value}
        candidates.discard("")
        return candidates.pop() if len(candidates) == 1 else ""

    def type_info(self, value, depth=0):
        if depth > 8:
            return None
        if value in self.value_types:
            return self.value_types[value]
        if value.kind == "slot" and value.key.kind == "key":
            owner = self.owner(value.base, depth + 1)
            declared = self.program.fields.get(owner, {}).get(value.key.name)
            source = self.program.owner_sources.get(owner)
            if source is not None and declared:
                return source, declared, owner
        if value.kind == "slot" and (value.key == ELEMENT or value.key.kind in {"literal", "dynamic_key", "parameter"}):
            info = self.type_info(value.base, depth + 1)
            if info:
                source, declared, scope = info
                declared = self.program.expanded_type(source, declared, scope).strip()
                if declared.endswith("[]"):
                    return source, declared[:-2], scope
        return None

    def owner(self, value, depth=0):
        if value.kind in {"receiver", "type"}:
            return value.name
        if value in self.owners:
            return self.owners[value]
        info = self.type_info(value, depth + 1) if depth <= 8 else None
        owner = self.program.declared_owner(*info) if info else ""
        if owner or depth >= 5 or not any(source.language == "scala" for source in self.program.sources):
            return owner
        candidates = {self.owners.get(alias, "") for alias in self.resolved_aliases(value) if alias != value}
        candidates.discard("")
        return candidates.pop() if len(candidates) == 1 else ""

    def record_type(self, value, source, type_text, scope=""):
        if value == UNKNOWN or not type_text:
            return
        self.value_types[value] = source, type_text, scope
        owner = self.program.declared_owner(source, type_text, scope)
        if owner:
            self.owners[value] = owner

    def run(self):
        facts = []
        for _ in range(self.passes):
            initial = []
            for source, target, value_node, location in self.program.global_initializers:
                interpreter = Interpreter(self, source)
                interpreter.store_value(target, interpreter.expression(value_node), location)
                initial.extend(interpreter.facts)
            for source, owner, name, value_node, location in self.program.initializers:
                interpreter = Interpreter(self, source, owner=owner)
                target = slot(Value("receiver", owner), name)
                value = interpreter.expression(value_node)
                if self.kind(value):
                    self.kinds[target] = self.kind(value)
                if self.owner(value):
                    self.owners[target] = self.owner(value)
                if value in self.value_types:
                    self.value_types[target] = self.value_types[value]
                interpreter.store_value(target, value, location, ("field_initializer_schema",))
                initial.extend(interpreter.facts)
            next_summaries = {}
            facts = initial
            for function in self.program.functions:
                interpreter = Interpreter(self, function.source, function)
                initializers = first(function.node, {"field_initializer_list"})
                for initializer in parts(initializers):
                    named = first(initializer, IDENTIFIERS)
                    value_node = first(initializer, {"initializer_list", "argument_list"})
                    if named is not None and value_node is not None and len(parts(value_node)) == 1:
                        interpreter.store_value(slot(interpreter.receiver, function.source.text(named)),
                                                interpreter.expression(parts(value_node)[0]), initializer)
                if function.source.language == "scala" and function.body.type not in BLOCKS:
                    interpreter.returns.append(interpreter.expression(function.body))
                else:
                    interpreter.statement(function.body)
                summary = Summary(interpreter.parameters, interpreter.facts, interpreter.returns)
                next_summaries[function.identifier] = summary
                facts.extend(interpreter.facts)
            self.summaries = next_summaries
            self.heap.clear()
            self.alias_cache.clear()
            self.heap_facts.clear()
            self.heap_by_root.clear()
            for fact in facts:
                if fact.kind == "store" and fact.value != UNKNOWN:
                    self.heap[fact.target].append(fact.value)
                    self.heap_facts[fact.target].append(fact)
                    self.heap_by_root[split_path(fact.target)[0]].append(fact)
            if len(facts) > MAX_TOTAL_FACTS:
                self.notices.add(("total_fact_cap", str(MAX_TOTAL_FACTS)))
                facts = facts[:MAX_TOTAL_FACTS]
                break
        self.notices.add(("bounded_summary_passes", str(self.passes)))
        facts = list(dict.fromkeys(facts))
        facts += self.project_stored_fields(facts)
        combined = list(dict.fromkeys(facts + self.project_calls(facts)))
        if len(combined) > MAX_TOTAL_FACTS:
            self.notices.add(("total_fact_cap", str(len(combined) - MAX_TOTAL_FACTS)))
        return combined[:MAX_TOTAL_FACTS]

    def required_conditions(self, value):
        """Retain the source conditions on a receiver and its explicit aliases."""
        if value == UNKNOWN:
            return set()
        result = set()
        for item in referenced_values(value):
            if item != UNKNOWN:
                result.update(self.value_conditions.get(item, ()))
        for alias, conditions, _ in self.target_aliases(value):
            result.update(conditions)
            for item in referenced_values(alias):
                if item != UNKNOWN:
                    result.update(self.value_conditions.get(item, ()))
        return result

    def project_stored_fields(self, facts):
        """An explicit object store carries fields backed by source stores.

        Preserve the registration location; retain the original field writes as
        provenance. This does not relax the connector's path-depth check.
        """
        stores = [fact for fact in facts if fact.kind == "store"]
        fields = defaultdict(list)
        for fact in stores:
            if fact.target.kind == "slot" and fact.target.key.kind == "key":
                fields[fact.target.base].append(fact)
        result = []
        for store in stores:
            value = store.value
            candidates = fields.get(value, [])
            if not candidates and value.kind == "parameter":
                owner = self.owner(value)
                if owner:
                    candidates = fields.get(Value("receiver", owner), [])
            for member in candidates[:16]:
                if member.target.base == store.target or member.target.key.kind != "key":
                    continue
                target = slot(store.target, member.target.key)
                field_value = member.value if member.target.base == value else slot(value, member.target.key)
                derived = Fact("store", target, field_value, store.location, store.function,
                               tuple(sorted(set(store.conditions + member.conditions + ("stored_object_field_projection",)))),
                               store.via + (member.location,))
                result.append(derived)
                self.heap[target].append(field_value)
                self.heap_facts[target].append(derived)
                self.heap_by_root[split_path(target)[0]].append(derived)
                if len(facts) + len(result) >= MAX_TOTAL_FACTS:
                    self.notices.add(("stored_field_projection_cap", str(MAX_TOTAL_FACTS)))
                    return result
        return result

    def target_aliases(self, target, depth=0):
        if depth >= 6:
            return []
        candidates = [(fact, ()) for fact in self.heap_facts.get(target, ())]
        if not candidates and target.kind == "slot":
            root, keys = split_path(target)
            for fact in self.heap_by_root.get(root, ()):
                if len(split_path(fact.target)[1]) != len(keys):
                    continue
                conditions = match_storage(fact.target, target)
                if conditions is not None:
                    candidates.append((fact, conditions))
        if len(candidates) > 8:
            self.notices.add(("heap_alias_alternative_cap", target.display()))
        values = [(option, tuple(sorted(set(fact.conditions + conditions))), fact.via + (fact.location,))
                  for fact, conditions in candidates[:8] for option in value_options(fact.value)
                  if option.kind in {"slot", "receiver", "parameter", "allocation"}]
        if target.kind == "slot":
            values.extend((slot(value, target.key), conditions, via)
                          for value, conditions, via in self.target_aliases(target.base, depth + 1))
        return list(dict.fromkeys(values))[:32]

    def resolved_aliases(self, target):
        if target in self.alias_cache:
            return self.alias_cache[target]
        seen, pending, values = {target}, [target], []
        for _ in range(6):
            following = []
            for current in pending:
                for alias, _, _ in self.target_aliases(current):
                    if alias in seen:
                        continue
                    seen.add(alias)
                    values.append(alias)
                    following.append(alias)
                    if len(seen) >= 96:
                        self.notices.add(("source_alias_search_bound", self.identifier if hasattr(self, "identifier") else target.display()))
                        self.alias_cache[target] = values
                        return values
            pending = following
            if not pending:
                break
        self.alias_cache[target] = values
        return values

    def project_calls(self, facts):
        """Follow explicit constructor stores and declared receiver types.

        A constructor allocation is merely an alternative for a type-relative
        receiver. The emitted obligation prevents treating it as actual dispatch.
        """
        result = []
        allocations = defaultdict(list)
        for allocation, owner in self.owners.items():
            if allocation.kind == "allocation":
                allocations[owner].append(allocation)
        for fact in facts:
            if fact.kind not in {"store", "invoke", "member_invoke", "argument"}:
                continue
            root, _ = split_path(fact.target)
            targets = [fact.target]
            if fact.kind != "store" and root in self.owners:
                targets.append(substitute(fact.target, {root: Value("receiver", self.owners[root])}))
            if fact.kind != "store" and root.kind == "receiver":
                if len(allocations[root.name]) > 8:
                    self.notices.add(("receiver_allocation_alternative_cap", root.name))
                for allocation in allocations[root.name][:8]:
                    targets.append(substitute(fact.target, {root: allocation}))
            targets = [(target, fact.conditions, fact.via) for target in targets]
            visited = set()
            for _ in range(4):
                next_targets = []
                for target, path_conditions, path_via in targets:
                    if target in visited:
                        continue
                    visited.add(target)
                    if target != fact.target:
                        if len(result) >= MAX_TOTAL_FACTS:
                            self.notices.add(("projection_fact_cap", str(MAX_TOTAL_FACTS)))
                            return result
                        result.append(Fact(fact.kind, target, fact.value, fact.location, fact.function,
                                           tuple(sorted(set(path_conditions + ("receiver_dispatch_instance_unproven", "explicit_store_projection")))),
                                           path_via, fact.argument_index))
                    # Rebinding a reference is not a write through its old
                    # value. Only strict prefixes of a store target may alias.
                    aliases = ([(slot(value, target.key), conditions, via) for value, conditions, via in self.target_aliases(target.base)]
                               if fact.kind == "store" and target.kind == "slot" else
                               ([] if fact.kind == "store" else self.target_aliases(target)))
                    next_targets.extend((value, tuple(sorted(set(path_conditions + conditions))),
                                         tuple(dict.fromkeys(path_via + via))) for value, conditions, via in aliases)
                    if target != fact.target:
                        alias_root, _ = split_path(target)
                        if alias_root.kind == "parameter" and alias_root in self.owners:
                            next_targets.append((substitute(target, {alias_root: Value("receiver", self.owners[alias_root])}), path_conditions, path_via))
                if len(next_targets) > 32:
                    self.notices.add(("projection_alternative_cap", fact.function))
                targets = next_targets[:32]
        return result


class Interpreter:
    def __init__(self, analyzer, source, function=None, owner="", env=None):
        self.analyzer = analyzer
        self.program = analyzer.program
        self.source = source
        self.function = function
        self.owner = function.owner if function else owner
        self.receiver = Value("receiver", self.owner) if self.owner else UNKNOWN
        self.identifier = function.identifier if function else "module:" + source.path
        self.env = dict(env or {})
        self.parameters = []
        self.declared_receivers = {}
        self.references = {}
        self.facts = []
        self.returns = []
        self.local_bindings = scope_bindings(source, function.body) if function else set()
        self.active_closures = ()
        self.active_functions = ()
        self.call_context = ()
        self.is_macro_template = False
        self.conditions = ()
        for name, type_text, is_pointer in function.parameters if function else []:
            value = Value("parameter", self.identifier + ":" + name)
            self.parameters.append(value)
            self.env[name] = value
            self.analyzer.record_type(value, source, type_text, self.owner)
            declared = self.program.declared_owner(source, type_text, self.owner)
            if declared:
                self.analyzer.owners[value] = declared
                if is_pointer and source.language in {"c", "cpp"}:
                    self.env[name] = Value("receiver", declared)
                    self.declared_receivers[name] = declared
            kind = type_kind(self.program.expanded_type(source, type_text, self.owner))
            if kind:
                self.analyzer.kinds[value] = kind
        for name in ("self", "this", "$this"):
            if self.owner:
                self.env[name] = self.receiver
        for name, type_text in self.program.fields.get(self.owner, {}).items():
            target = slot(self.receiver, name)
            self.analyzer.record_type(target, source, type_text, self.owner)
            kind = type_kind(self.program.expanded_type(source, type_text, self.owner))
            if kind:
                self.analyzer.kinds[target] = kind
            declared = self.program.declared_owner(source, type_text, self.owner)
            if declared:
                self.analyzer.owners[target] = declared

    def text(self, node):
        return self.source.text(node)

    def allocation_name(self, node, label=""):
        origin = self.source.path + ":" + label + str(node.start_byte)
        return origin + ("@" + ">".join(self.call_context) if self.call_context else "")

    def emit(self, kind, target, value, node, conditions=(), via=(), argument_index=None):
        if node is None or target == UNKNOWN:
            return
        if len(self.facts) >= MAX_FACTS:
            self.analyzer.notices.add(("function_fact_cap", self.identifier))
            return
        extra = ("preprocessor_configuration_unproven",) if getattr(self.source, "has_branch_projection", False) else ()
        if getattr(self.source, "has_conditional_projection", False):
            extra += ("conditional_compilation_selection_unproven",)
        if self.source.path in self.program.unproven_type_sources:
            extra += ("source_module_binding_required",)
        extra += tuple(self.analyzer.value_conditions.get(value, ()))
        extra += tuple(self.analyzer.value_conditions.get(target, ()))
        for alternative in value_options(target):
            if len(self.facts) >= MAX_FACTS:
                self.analyzer.notices.add(("function_fact_cap", self.identifier))
                break
            fact = Fact(kind, alternative, value, self.source.location(node), self.identifier,
                        tuple(sorted(set(("control_flow_unproven",) + self.conditions + tuple(conditions) + extra))), tuple(via), argument_index)
            if fact not in self.facts:
                self.facts.append(fact)

    def binding(self, name):
        if name in self.env:
            return self.env[name]
        if self.source.language == "java":
            known = self.program.jvm.binding(self.owner, name)
            if known is not None:
                return known
        if self.source.language == "scala" and name == "copy" and self.owner in self.program.case_classes:
            return slot(self.receiver, name)
        if name.startswith("@") and self.owner:
            return slot(self.receiver, name.lstrip("@"))
        if self.is_implicit_field(name):
            return slot(self.receiver, name.lstrip("$"))
        if (self.source.path, name) in self.program.globals:
            return self.program.globals[(self.source.path, name)]
        functions = self.program.methods.get((self.owner or self.source.path, name), [])
        if len(functions) == 1:
            return Value("function", functions[0].identifier)
        if self.source.language == "scala":
            function = self.program.scala.package_members.get((self.program.namespaces[self.source.path], name))
            if function:
                return Value("function", function.identifier)
        owner = self.program.declared_owner(self.source, name, self.owner)
        if owner in self.program.owner_sources:
            return Value("type", owner)
        if self.source.language == "swift":
            functions = [function for function in self.program.functions
                         if not function.owner and function.name == name and function.source.language == "swift"]
            if len(functions) == 1:
                self.program.unproven_type_sources.update((self.source.path, functions[0].source.path))
                return Value("function", functions[0].identifier)
        return Value("unresolved", self.identifier + ":" + name)

    def is_implicit_field(self, name):
        return (self.source.language in {"cpp", "csharp", "dart", "groovy", "java", "kotlin", "scala", "swift"}
                and name.lstrip("$") in self.program.fields.get(self.owner, {}))

    def bind(self, node, value, type_text=""):
        if node is None:
            return
        if self.text(node) == "_":
            return
        if value.kind == "bound_method" and type_text:
            method = self.java_sam_method(type_text)
            value = Value("bound_method", method, value.base, value.key)
        if node.type in {"tuple_pattern", "record_pattern"}:
            patterns = [part for part in parts(node) if "comment" not in part.type]
            if len(patterns) > 8:
                self.analyzer.notices.add(("tuple_arity_cap", self.identifier))
                return
            values = tuple_values(value)
            if values and len(values) != len(patterns):
                self.analyzer.notices.add(("tuple_arity_mismatch", self.identifier))
                return
            for index, pattern in enumerate(patterns):
                if pattern.type in {"constant_pattern", "variable_pattern"} and len(parts(pattern)) == 1:
                    pattern = parts(pattern)[0]
                projected = values[index] if values else slot(value, Value("tuple_index", str(index)))
                self.bind(pattern, projected)
            return
        named = identifier(node)
        if named is not None and node.type in IDENTIFIERS | {"pattern", "variable_declaration", "parameter", "directly_assignable_expression"}:
            name = self.text(named)
            if self.is_implicit_field(name) and name not in self.env:
                self.store_value(slot(self.receiver, name.lstrip("$")), value, node)
            else:
                self.env[name] = value
                self.analyzer.record_type(value, self.source, type_text, self.owner)
                declared = self.program.declared_owner(self.source, type_text)
                if declared and "*" in type_text and self.source.language in {"c", "cpp"}:
                    self.env[name] = Value("receiver", declared)
                elif declared:
                    self.analyzer.owners[value] = declared
            return
        target = self.expression(node)
        if target.kind == "slot":
            self.store_value(target, value, node)

    def store_value(self, target, value, node, conditions=()):
        if value.kind == "tuple":
            # Only leaves are callable storage. Treating the whole record as a
            # callable object would also connect the wrong positional field.
            for index, item in enumerate(tuple_values(value)):
                self.store_value(slot(target, Value("tuple_index", str(index))), item, node, conditions)
        else:
            self.emit("store", target, value, node, conditions)

    def tuple_parts(self, node):
        if self.source.language == "swift":
            if any(node.field_name_for_child(index) == "name" for index in range(len(node.children))):
                self.analyzer.notices.add(("named_tuple_projection_unsupported", self.identifier))
                return None
            return [part for index, part in enumerate(node.children) if node.field_name_for_child(index) == "value"]
        children = [part for part in parts(node) if "comment" not in part.type]
        if node.type == "record_literal":
            if any(part.type != "record_field" or len(parts(part)) != 1 for part in children):
                self.analyzer.notices.add(("named_record_projection_unsupported", self.identifier))
                return None
            return [parts(part)[0] for part in children]
        return children

    def assignment(self, node):
        left = child(node, "left", "target")
        right = child(node, "right", "result", "value")
        children = parts(node)
        if left is None and children:
            left = children[0]
        if right is None and len(children) > 1:
            right = children[-1]
        if left is not None and left.type == "variable_list":
            left = parts(left)[0] if parts(left) else left
        if left is not None and left.type == "directly_assignable_expression" and len(parts(left)) == 1:
            left = parts(left)[0]
        if right is not None and right.type == "expression_list" and len(parts(right)) == 1:
            right = parts(right)[0]
        if self.source.language == "php" and node.type == "reference_assignment_expression" and left is not None:
            value = self.expression(right, is_read=False)
            name = self.text(left)
            self.references[name] = value
            self.env[name] = value
            return value
        value = self.expression(right)
        if left is None:
            return value
        if left.type == "tuple_expression":
            targets = self.tuple_parts(left)
            values = tuple_values(value)
            if targets is None or len(targets) != len(values) or len(values) > 8:
                self.analyzer.notices.add(("tuple_assignment_shape_unresolved", self.identifier))
                return value
            for target_node, item in zip(targets, values):
                if self.text(target_node) == "_":
                    continue
                if target_node.type in IDENTIFIERS:
                    self.bind(target_node, item)
                else:
                    self.store_value(self.expression(target_node, is_read=False), item, node)
            return value
        operator = next((item.type for item in node.children if item.type in {"+=", "-=", "<<", "||="}), "=")
        target = self.expression(left, is_read=False)
        if self.source.language == "php" and self.text(left) in self.references:
            self.store_value(self.references[self.text(left)], value, node, ("reference_binding_required",))
            return value
        if target.kind == "slot" and target.base.kind == "receiver" and (target.base.name, target.key.name) in self.program.events:
            if operator in {"+=", "-="}:
                self.emit("store" if operator == "+=" else "remove", target, value, node, ("multicast_event_membership_required",))
                return target
        if operator in {"+=", "-=", "<<"} and self.analyzer.kind(target) in {"array", "set"}:
            self.emit("remove" if operator == "-=" else "store", slot(target, ELEMENT), value, node, ("collection_operator",))
            return target
        if left.type in IDENTIFIERS and self.text(left) not in self.env and target.kind != "slot":
            self.env[self.text(left)] = value
        elif left.type in IDENTIFIERS and self.text(left) in self.env:
            declared = self.declared_receivers.get(self.text(left))
            self.env[self.text(left)] = Value("receiver", declared) if declared and value.kind in {"unknown", "result", "unresolved"} else value
        else:
            if target.kind == "slot":
                self.store_value(target, value, node)
                if value.kind == "literal" and value.name in {"nil", "null", "None"}:
                    self.emit("remove", target, UNKNOWN, node)
                if self.analyzer.kind(value):
                    self.analyzer.kinds[target] = self.analyzer.kind(value)
                if target.base is not None and target.base.kind == "slot" and target.base.key == MAP_ENTRIES:
                    self.emit("store", slot(slot(target.base.base, MAP_KEYS), ELEMENT), target.key, node, ("map_key_role",))
        return value

    def statement(self, node):
        if node is None:
            return
        kind = node.type
        if kind in FUNCTIONS | CLASSES | LAMBDAS or "comment" in kind:
            return
        if kind in ASSIGNMENTS:
            self.assignment(node)
            return
        if self.source.language == "java" and kind == "explicit_constructor_invocation":
            if self.text(child(node, "constructor")) == "super":
                parent = self.program.jvm.parents.get(self.owner, "")
                values = [self.expression(part) for part in parts(child(node, "arguments"))]
                if parent:
                    self.apply(parent, parent.rsplit("::", 1)[-1], self.receiver, values, node)
            return
        if self.source.language in {"scala", "java"} and kind in {"match_expression", "switch_expression"}:
            value = self.expression(node)
            if value != UNKNOWN:
                self.returns.append(value)
            return
        if self.source.language in {"scala", "java"} and kind in {"if_expression", "if_statement"}:
            condition = self.expression(child(node, "condition"))
            consequence = child(node, "consequence", "body")
            alternative = child(node, "alternative")
            if consequence is not None:
                old = self.conditions
                self.conditions += (f"source_branch_condition_required:{self.source.path}:{self.source.location(node).line}",)
                selected = [consequence] if condition == Value("literal", "true") else ([alternative] if condition == Value("literal", "false") else [consequence, alternative])
                for part in selected:
                    self.statement(part)
                self.conditions = old
                return
        if kind in {"return_statement", "return_expression", "yield", "yield_expression", "yield_statement"}:
            for item in parts(node):
                value = self.expression(item)
                if value != UNKNOWN:
                    self.returns.append(Value("iterator", "yield", value) if kind.startswith("yield") else value)
            return
        if kind == "arrow_expression_clause" and parts(node):
            self.returns.append(self.expression(parts(node)[-1]))
            return
        if self.source.language == "swift" and kind == "control_transfer_statement" and any(item.type == "return" for item in node.children):
            value = child(node, "result") or (parts(node)[-1] if parts(node) else None)
            self.returns.append(self.expression(value))
            return
        if kind in LOOPS:
            self.loop(node)
            return
        if kind in DECLARATIONS:
            if kind == "local_variable_declaration" and self.source.language == "java":
                for declarator in parts(node):
                    if declarator.type == "variable_declarator":
                        self.bind(child(declarator, "name"), self.expression(child(declarator, "value")), self.text(child(node, "type")))
                return
            if kind == "local_variable_declaration" and first(node, {"pattern_variable_declaration"}) is not None:
                self.statement(first(node, {"pattern_variable_declaration"}))
                return
            pattern = child(node, "name", "pattern") or first(node, {"variable_declaration", "identifier", "simple_identifier"})
            if kind == "pattern_variable_declaration":
                pattern = first(node, {"record_pattern"})
            value_node = child(node, "value")
            if value_node is None and pattern is not None:
                value_node = parts(node)[-1] if parts(node) and parts(node)[-1] != pattern else None
            if value_node is not None:
                value = self.expression(value_node)
                self.bind(pattern, value, self.text(child(node, "type")))
            else:
                for item in parts(node):
                    if item != pattern:
                        self.statement(item)
            return
        if kind == "declaration" and self.source.language in {"c", "cpp"}:
            type_text = self.text(child(node, "type"))
            for item in parts(node):
                if item.type in {"init_declarator", "pointer_declarator", "identifier"}:
                    named = identifier(child(item, "declarator") or item)
                    value = self.expression(child(item, "value"))
                    name = self.text(named)
                    declared = self.program.declared_owner(self.source, type_text)
                    if declared and "*" in self.text(item):
                        value = Value("receiver", declared)
                        self.declared_receivers[name] = declared
                    if name:
                        self.env[name] = value
            return
        if kind in CALLS or kind in {"expression_statement", "expression_list"}:
            self.expression(node)
            return
        for item in parts(node):
            self.statement(item)
        if kind in BLOCKS and self.source.language in {"scala", "swift", "kotlin", "ruby"} and parts(node):
            expressions = [part for part in parts(node) if "comment" not in part.type]
            if not expressions:
                return
            tail = expressions[-1]
            if tail.type not in ASSIGNMENTS | FUNCTIONS and "comment" not in tail.type:
                value = self.expression(tail)
                if value != UNKNOWN:
                    self.returns.append(value)

    def read_property(self, base, name, node):
        owner = self.analyzer.owner(base)
        if base.kind == "type" and name in self.program.jvm.enum_constants.get(owner, ()):
            return Value("enum", owner + "." + name)
        if self.source.language == "scala":
            if self.is_macro_template and name == "splice":
                return base
            methods = self.program.scala.methods(owner, name)
            if len(methods) == 1 and methods[0].source.language == "scala" and not any(part.type == "parameters" for part in parts(methods[0].node)):
                return self.apply_callable(methods[0], base, [], node)
            if name == "toArray" and self.analyzer.kind(base) in {"array", "set"}:
                return self.invoke(slot(base, name), [], node)
        getters = self.program.getters.get((owner, name), [])
        if self.source.language == "groovy" and ".@" not in self.text(node):
            getter_name = "get" + name[:1].upper() + name[1:]
            getters = self.program.methods.get((owner, getter_name), [])
        if getters:
            if len(getters) == 1 and not getters[0].parameters:
                return self.apply_callable(getters[0], base, [], node)
            return UNKNOWN
        if self.source.language == "swift" and name.isdecimal():
            return slot(base, Value("tuple_index", name))
        if self.source.language == "scala" and re.fullmatch(r"_[1-8]", name):
            return slot(base, Value("tuple_index", str(int(name[1:]) - 1)))
        return slot(base, name)

    def member(self, node, is_read=True):
        base_node = child(node, "object", "argument", "value", "table", "target", "receiver", "expression")
        field_node = child(node, "attribute", "field", "property", "method", "suffix", "name")
        children = parts(node)
        if base_node is None and len(children) > 1:
            base_node = children[0]
        if field_node is None and children:
            field_node = children[-1]
        base = self.expression(base_node) if base_node is not None else (self.receiver if self.text(node).startswith("this.") else UNKNOWN)
        if field_node is not None and field_node.type in {"navigation_suffix", "member_binding_expression"}:
            field_node = parts(field_node)[-1] if parts(field_node) else field_node
        if self.source.language == "php" and field_node is not None and field_node.type == "variable_name":
            key = self.expression(field_node)
            # A runtime property name is not the literal spelling of its variable.
            return slot(base, key) if base != UNKNOWN and key.kind == "key" else UNKNOWN
        name = self.text(field_node).lstrip(".$?->")
        if self.source.language == "lua" and self.analyzer.kind(base) == "map":
            base = slot(base, MAP_ENTRIES)
        if base == UNKNOWN or not name:
            return UNKNOWN
        return self.read_property(base, name, node) if is_read else (
            slot(base, Value("tuple_index", name)) if self.source.language == "swift" and name.isdecimal() else slot(base, name))

    def expression(self, node, depth=0, is_read=True):
        if node is None or depth > 32:
            return UNKNOWN
        kind = node.type
        children = parts(node)
        if "comment" in kind:
            return UNKNOWN
        if kind in IDENTIFIERS or kind in {"self", "this", "self_expression", "this_expression", "instance_variable"}:
            name = self.text(node)
            if is_read and self.owner and name not in self.env and self.source.language in {"kotlin", "groovy"}:
                value = self.read_property(self.receiver, name, node)
                if value != slot(self.receiver, name):
                    return value
            return self.binding(name)
        if kind in {"string", "string_literal", "string_literal_fragment", "simple_symbol", "symbol", "encapsed_string"}:
            return Value("key", self.text(node).strip("'\"`:"))
        if kind in {"integer", "integer_literal", "decimal_integer_literal", "number", "null", "null_literal", "nil", "none", "true", "false", "boolean_literal"}:
            return Value("literal", self.text(node))
        if kind in ASSIGNMENTS:
            return self.assignment(node)
        if kind in MEMBERS:
            return self.member(node, is_read)
        if self.source.language == "java" and kind == "class_literal" and children:
            owner = self.program.declared_owner(self.source, self.text(children[0]))
            return Value("type", owner) if owner else UNKNOWN
        if self.source.language in {"scala", "java"} and kind in {"match_expression", "switch_expression"}:
            compared = self.expression(child(node, "value", "condition"))
            body = child(node, "body")
            matches = []
            for case in parts(body):
                if case.type == "case_clause":
                    pattern = child(case, "pattern")
                    candidate = self.expression(pattern)
                    if candidate == compared and compared.kind in {"type", "enum", "literal", "key"}:
                        matches.append(child(case, "body"))
                elif case.type == "switch_block_statement_group":
                    label = first(case, {"switch_label"})
                    labels = parts(label)
                    if compared.kind == "enum" and any(self.text(item) == compared.name.rsplit(".", 1)[-1] for item in labels):
                        matches.extend(part for part in parts(case) if part != label)
            if not matches:
                self.analyzer.notices.add(("case_selection_unproven", self.identifier))
                return UNKNOWN
            before = len(self.returns)
            values = []
            for part in matches:
                if part is not None and part.type == "return_statement":
                    self.statement(part)
                else:
                    values.append(self.expression(part, depth + 1))
            values.extend(self.returns[before:])
            return merge_values([value for value in values if value != UNKNOWN])
        if self.source.language == "scala" and kind == "generic_function":
            target = child(node, "function")
            if target is not None and target.type == "field_expression" and self.text(child(target, "field")) == "asInstanceOf":
                value = self.expression(child(target, "value"), depth + 1)
                self.analyzer.value_conditions[value].add("scala_cast_compatibility_required")
                return value
            return self.expression(child(node, "function"), depth + 1)
        if self.source.language == "scala" and kind == "instance_expression":
            type_node = child(node, "type") or first(node, {"type_identifier", "generic_type"})
            owner = self.program.declared_owner(self.source, self.text(type_node), self.owner)
            arguments = [part for part in parts(child(node, "arguments")) if "comment" not in part.type]
            anonymous = self.program.scala.anonymous.get((self.source.path, node.start_byte))
            if anonymous:
                value = Value("allocation", self.allocation_name(node, "anonymous:"))
                self.analyzer.owners[value] = anonymous
                return value
            if owner and not first(node, {"template_body"}):
                if owner.startswith("java:"):
                    value = Value("allocation", self.allocation_name(node))
                    self.analyzer.owners[value] = owner
                    self.apply(owner, owner.rsplit("::", 1)[-1], value,
                               [self.expression(item, depth + 1) for item in arguments], node)
                    return value
                return self.construct_scala(owner, [self.expression(item, depth + 1) for item in arguments], arguments, node)
            self.analyzer.notices.add(("anonymous_or_unresolved_constructor", self.identifier))
            return UNKNOWN
        if kind == "directly_assignable_expression" and len(children) > 1:
            return self.member(node, is_read)
        if kind in {"subscript", "subscript_expression", "element_access_expression", "element_reference", "array_access", "bracket_index_expression", "index_expression"}:
            base = self.expression(child(node, "object", "value", "argument", "table", "array") or (children[0] if children else None), depth + 1)
            index_node = child(node, "index", "subscript", "indices") or (children[-1] if len(children) > 1 else None)
            if index_node is not None and index_node.type in {"argument", "argument_list", "bracketed_argument_list"} and parts(index_node):
                index_node = parts(index_node)[0]
            index = self.expression(index_node, depth + 1)
            if self.source.language == "php" and index_node is None and not is_read:
                index = ELEMENT
            if index == UNKNOWN:
                index = Value("dynamic_key", self.identifier + ":" + str(node.start_byte))
            if self.analyzer.kind(base) == "map" or (self.source.language == "lua" and index.kind != "key"):
                base = slot(base, MAP_ENTRIES)
            return slot(base, index)
        if kind in LAMBDAS:
            if self.source.language == "cpp" and kind == "lambda_expression":
                return self.capture_closure(node)
            return Value("function", self.identifier + ":lambda:" + str(node.start_byte))
        if self.source.language == "java" and kind == "method_reference" and len(children) == 2:
            receiver = self.expression(children[0], depth + 1)
            if receiver.kind in {"slot", "parameter", "receiver", "allocation"}:
                return Value("bound_method", "", receiver, Value("key", self.text(children[-1])))
            return UNKNOWN
        if kind in {"tuple_expression", "record_literal"}:
            items = self.tuple_parts(node)
            return tuple_value([self.expression(item, depth + 1) for item in items]) if items is not None else UNKNOWN
        if kind in {"array", "list", "set", "dictionary", "hash", "table_constructor", "array_creation_expression", "array_creation", "list_literal", "set_or_map_literal"}:
            value = Value("allocation", self.allocation_name(node))
            container = "map" if kind in {"dictionary", "hash", "table_constructor", "set_or_map_literal"} else "array"
            self.analyzer.kinds[value] = container
            for item in children:
                val = self.expression(item, depth + 1)
                self.emit("store", slot(value, ELEMENT), val, item)
            return value
        if kind in {"object_creation_expression", "new_expression", "instance_creation_expression"} or (self.source.language == "swift" and kind == "constructor_expression"):
            value = Value("allocation", self.allocation_name(node))
            type_text = self.text(child(node, "type", "constructor", "constructed_type"))
            owner = self.program.declared_owner(self.source, type_text, self.owner)
            self.analyzer.record_type(value, self.source, type_text, self.owner)
            if owner:
                self.analyzer.owners[value] = owner
            container = type_kind(type_text)
            if container:
                self.analyzer.kinds[value] = container
            argument_node = child(node, "arguments")
            if self.source.language == "swift" and kind == "constructor_expression":
                argument_node = first(node, {"value_arguments"})
            arguments = [part for part in parts(argument_node) if "comment" not in part.type]
            values = [self.expression(item, depth + 1) for item in arguments]
            if owner:
                name = "init" if self.source.language == "swift" else owner.rsplit("::", 1)[-1]
                self.apply(owner, name, value, values, node)
            return value
        if kind in CALLS:
            return self.call(node)
        if kind in {"yield", "yield_expression", "yield_statement"}:
            self.statement(node)
            return UNKNOWN
        if self.source.language == "dart" and kind in {"expression_statement", "assignable_expression", "argument", "initializer_list_entry"}:
            return self.dart_chain(node)
        if kind in {"expression_statement", "argument", "value_argument", "parenthesized_expression", "parenthesized_lvalue",
                    "directly_assignable_expression", "await_expression", "try_expression", "postfix_expression", "prefix_expression",
                    "cast_expression", "as_expression", "argument_value", "unary_expression", "pointer_expression",
                    "argument_list", "expression_list", "call_suffix", "callable_expression"}:
            target = child(node, "value", "argument", "expression", "body")
            if target is None:
                target = children[-1] if children else None
            return self.expression(target, depth + 1)
        if kind in {"binary_expression", "binary_operator", "infix_expression", "infix_function_call", "elvis_expression"}:
            if self.source.language == "scala" and self.text(child(node, "operator")) == "+":
                left = self.expression(child(node, "left"), depth + 1)
                if self.analyzer.kind(left) == "set":
                    right = self.expression(child(node, "right"), depth + 1)
                    value = Value("allocation", self.allocation_name(node, "set_copy:"))
                    self.analyzer.kinds[value] = "set"
                    self.store_value(slot(value, ELEMENT), slot(left, ELEMENT), node, ("immutable_collection_copy",))
                    self.store_value(slot(value, ELEMENT), right, node, ("immutable_collection_copy",))
                    return value
            values = [self.expression(item, depth + 1) for item in children]
            return merge_values([value for value in values if value.kind in {"slot", "allocation", "receiver", "parameter"}])
        if kind == "conditional_expression":
            self.expression(child(node, "condition"), depth + 1)
            consequence = child(node, "consequence", "body")
            alternative = child(node, "alternative")
            if consequence is not None and alternative is not None:
                return merge_values([self.expression(consequence, depth + 1), self.expression(alternative, depth + 1)])
        if kind in BLOCKS:
            self.statement(node)
            return self.returns[-1] if self.returns else UNKNOWN
        for item in children:
            self.expression(item, depth + 1)
        return UNKNOWN

    def dart_chain(self, node):
        value = UNKNOWN
        for item in parts(node):
            if item.type in {"selector", "unconditional_assignable_selector", "conditional_assignable_selector"}:
                selector = parts(item)[0] if item.type == "selector" and parts(item) else item
                if not parts(selector):
                    # Dart's postfix ! has no named children and does not
                    # introduce a member or change the value's identity.
                    continue
                argument_part = selector if selector.type == "argument_part" else first(selector, {"argument_part", "arguments"})
                if argument_part is not None:
                    arguments = first(argument_part, {"arguments"}) or argument_part
                    value = self.invoke(value, [self.expression(arg) for arg in parts(arguments)], node)
                elif "[" in self.text(selector):
                    index = first(selector, {"index_selector"}) or selector
                    index = parts(index)[0] if parts(index) else None
                    value = slot(value, self.expression(index))
                else:
                    named = identifier(selector)
                    value = slot(value, self.text(named))
            else:
                value = self.expression(item)
        return value

    def call(self, node):
        kind = node.type
        method_node = None
        receiver_node = None
        if kind in {"method_invocation", "member_call_expression", "nullsafe_member_call_expression", "scoped_call_expression"}:
            receiver_node = child(node, "object", "scope")
            method_node = child(node, "name")
        elif kind == "call" and self.source.language == "ruby":
            receiver_node = child(node, "receiver")
            method_node = child(node, "method")
        function_node = child(node, "function", "name")
        if method_node is not None:
            receiver = self.expression(receiver_node) if receiver_node is not None else self.receiver
            callee = slot(receiver, self.text(method_node)) if receiver != UNKNOWN else self.binding(self.text(method_node))
        else:
            if function_node is None and parts(node):
                function_node = parts(node)[0]
            callee = self.expression(function_node)
        arguments_node = child(node, "arguments")
        if arguments_node is None:
            suffix = first(node, {"call_suffix"})
            arguments_node = first(suffix, {"value_arguments", "arguments"})
        argument_nodes = [part for part in parts(arguments_node) if "comment" not in part.type]
        if self.source.language == "php" and len(argument_nodes) == 1 and argument_nodes[0].type == "variadic_placeholder":
            self.analyzer.value_conditions[callee].add("first_class_callable_adapter")
            self.emit("callable_adapter", callee, UNKNOWN, node)
            return callee
        arguments = [self.expression(child(item, "right") if self.source.language == "scala" and item.type == "assignment_expression" else item) for item in argument_nodes]
        block = child(node, "block") or first(node, {"lambda_literal", "do_block", "block"})
        if block is None:
            suffix = first(node, {"call_suffix"})
            annotated = first(suffix, {"annotated_lambda"})
            block = first(annotated, {"lambda_literal"})
        return self.invoke(callee, arguments, node, argument_nodes, block)

    def apply(self, owner, method, receiver, arguments, node):
        candidates = (self.program.jvm.methods(owner, method) if owner.startswith("java:") else
                      self.program.scala.methods(owner, method) if self.source.language == "scala" else self.program.methods.get((owner, method), []))
        if len(candidates) > 1 and self.source.language == "scala":
            matching = [candidate for candidate in candidates
                        if len(candidate.parameters) - len(self.program.scala.implicit_names.get(candidate.identifier, ())) <= len(arguments) <= len(candidate.parameters)]
            if len(matching) == 1:
                candidates = matching
        if len(candidates) != 1:
            if candidates:
                self.analyzer.notices.add(("overload_resolution_unproven", owner + ":" + method))
            return UNKNOWN
        function = candidates[0]
        return self.apply_callable(function, receiver, arguments, node)

    def apply_callable(self, function, receiver, arguments, node):
        if function.source.language == "scala" or (function.source.language == "java" and any(source.language == "scala" for source in self.program.sources)):
            return self.apply_scala_callable(function, receiver, arguments, node)
        summary = self.analyzer.summaries.get(function.identifier)
        if summary is None or function.identifier == self.identifier:
            return UNKNOWN
        replacements = dict(zip(summary.parameters, arguments))
        if function.owner:
            replacements[function.receiver] = receiver
        for fact in summary.facts:
            if len(fact.via) >= 4:
                continue
            target = substitute(fact.target, replacements)
            value = substitute(fact.value, replacements)
            if len(self.facts) < MAX_FACTS:
                mapped = Fact(fact.kind, target, value, fact.location, self.identifier,
                              tuple(sorted(set(fact.conditions + ("callee_summary_applied",)))),
                              fact.via + (self.source.location(node),), fact.argument_index)
                self.append_mapped_fact(mapped)
                if fact.kind == "store" and set(summary.parameters).intersection(referenced_values(fact.value)):
                    registered = Fact("store", target, value, self.source.location(node), self.identifier,
                                      tuple(sorted(set(mapped.conditions + ("argument_store_through_summary",)))),
                                      tuple(dict.fromkeys(mapped.via + (fact.location,))))
                    self.append_mapped_fact(registered)
        return merge_values([substitute(value, replacements) for value in summary.returns])

    def apply_scala_callable(self, function, receiver, arguments, node):
        if function.identifier in self.active_functions or function.identifier == self.identifier or len(self.active_functions) >= 10:
            self.analyzer.notices.add(("source_call_depth_bound", function.identifier))
            return UNKNOWN
        actuals = self.program.scala.bind_implicit_arguments(self, function, arguments, node)
        if actuals is None or len(actuals) != len(function.parameters):
            return UNKNOWN
        nested = Interpreter(self.analyzer, function.source, function)
        nested.active_functions = self.active_functions + (self.identifier,)
        requirements = set(self.conditions)
        for value in [receiver, *actuals]:
            requirements.update(self.analyzer.required_conditions(value))
        nested.conditions = tuple(sorted(requirements))
        call_location = self.source.location(node)
        nested.call_context = self.call_context + (f"{call_location.path}:{call_location.line}:{call_location.column}",)
        nested.receiver = receiver if receiver != UNKNOWN else function.receiver
        nested.env.update({"this": nested.receiver, "self": nested.receiver})
        for (name, _, _), actual in zip(function.parameters, actuals):
            nested.env[name] = actual
        template = self.program.scala.macro_template(self, function)
        if template:
            implementation, body = template
            nested.source = implementation.source
            nested.is_macro_template = True
            nested.statement(body)
            extra = ("source_macro_template_preservation_required", "compiler_macro_typing_unproven")
        else:
            body = function.body
            if function.source.language == "scala" and body.type not in BLOCKS:
                nested.returns.append(nested.expression(body))
            else:
                nested.statement(body)
            extra = ()
        location = self.source.location(node)
        returned = merge_values(nested.returns)
        for fact in nested.facts:
            mapped = Fact(fact.kind, fact.target, fact.value, fact.location, fact.function,
                          tuple(sorted(set(fact.conditions + extra + ("source_callable_argument_binding",)))),
                          tuple(dict.fromkeys(fact.via + (location,))), fact.argument_index, fact.consumer)
            self.append_mapped_fact(mapped)
        if returned != UNKNOWN:
            self.analyzer.value_conditions[returned].update(extra + nested.conditions)
        return returned

    def append_mapped_fact(self, fact):
        for target in value_options(fact.target):
            if len(self.facts) >= MAX_FACTS:
                self.analyzer.notices.add(("function_fact_cap", self.identifier))
                return
            mapped = Fact(fact.kind, target, fact.value, fact.location, fact.function,
                          fact.conditions, fact.via, fact.argument_index)
            if mapped not in self.facts:
                self.facts.append(mapped)

    def construct_scala(self, owner, arguments, argument_nodes, node, original=None):
        parameters = self.program.class_parameters.get(owner)
        if parameters is None:
            return UNKNOWN
        value = Value("allocation", self.allocation_name(node, "scala_instance:"))
        self.analyzer.owners[value] = owner
        source = self.program.owner_sources[owner]
        nested = Interpreter(self.analyzer, source, owner=owner)
        nested.active_functions = self.active_functions
        nested.call_context = self.call_context
        nested.receiver = value
        nested.env.update({"this": value, "self": value})
        supplied = {}
        for index, argument in enumerate(arguments):
            named = argument_nodes[index] if index < len(argument_nodes) else None
            name = self.text(child(named, "left")) if named is not None and named.type == "assignment_expression" else (parameters[index][0] if index < len(parameters) else "")
            if name:
                supplied[name] = argument
        for name, type_text, default, parameter_node in parameters:
            actual = supplied[name] if name in supplied else (slot(original, name) if original is not None else nested.expression(default))
            nested.env[name] = actual
            target = slot(value, name)
            self.analyzer.record_type(target, source, type_text, owner)
            if name in self.program.fields[owner]:
                self.store_value(target, actual, node, ("case_class_copy" if original is not None else "source_constructor_parameter",))
        for initial_source, initial_owner, name, value_node, location in self.program.initializers:
            if initial_owner == owner:
                nested.store_value(slot(value, name), nested.expression(value_node), location, ("constructor_initializer",))
        for fact in nested.facts:
            self.append_mapped_fact(fact)
        return value

    def invoke(self, callee, arguments, node, argument_nodes=(), block=None):
        if callee.kind == "choice":
            return merge_values([self.invoke(option, arguments, node, argument_nodes, block) for option in value_options(callee)])
        if self.source.language == "scala" and self.analyzer.kind(callee) == "array" and len(arguments) == 1:
            return slot(callee, arguments[0])
        if callee.kind == "closure":
            return self.invoke_closure(callee, arguments, node)
        if callee.kind == "type" and self.source.language in {"swift", "kotlin", "scala", "groovy"}:
            if self.source.language == "scala":
                if self.program.methods.get((callee.name, "apply")):
                    return self.apply(callee.name, "apply", callee, arguments, node)
                return self.construct_scala(callee.name, arguments, argument_nodes, node)
            value = Value("allocation", self.source.path + ":" + str(node.start_byte))
            self.analyzer.owners[value] = callee.name
            constructor = "init" if self.source.language == "swift" else callee.name.rsplit("::", 1)[-1]
            self.apply(callee.name, constructor, value, arguments, node)
            return value
        if callee.kind == "slot":
            receiver, method = callee.base, callee.key.name
            if self.source.language == "java":
                value = self.program.jvm.invoke_handle(self, receiver, method, arguments, node)
                if value is not None:
                    return value
            if self.source.language == "scala" and method == "copy" and self.analyzer.owner(receiver) in self.program.case_classes:
                return self.construct_scala(self.analyzer.owner(receiver), arguments, argument_nodes, node, receiver)
            if receiver.kind == "bound_method":
                if method == receiver.name and method:
                    return self.invoke(slot(receiver.base, receiver.key), arguments, node)
                return UNKNOWN
            kind = self.analyzer.kind(receiver)
            if self.source.language == "csharp":
                has_ref = bool(argument_nodes and self.text(argument_nodes[0]).lstrip().startswith("ref "))
                if has_ref and self.is_standard_static(receiver, "System.Threading.Volatile") and method == "Read" and len(arguments) == 1:
                    self.analyzer.value_conditions[arguments[0]].add("snapshot_read_time_unproven")
                    return arguments[0]
                if has_ref and self.is_standard_static(receiver, "System.Threading.Interlocked") and method == "CompareExchange" and len(arguments) == 3:
                    self.store_value(arguments[0], arguments[1], node, ("compare_exchange_success_required",))
                    # The returned snapshot precedes this write. Do not alias
                    # it to the mutable destination after the exchange.
                    self.analyzer.notices.add(("atomic_previous_value_snapshot_unresolved", self.identifier))
                    return Value("result", self.source.path + ":atomic_previous:" + str(node.start_byte))
            if self.source.language == "java" and method == "invoke" and self.has_standard_type(receiver, "java.lang.reflect.Method"):
                self.emit("invoke", receiver, arguments[0] if arguments else UNKNOWN, node,
                          ("reflective_method_target_unproven", "reflective_dispatch_compatibility_required"))
                for index, argument in enumerate(arguments):
                    self.emit("argument", argument, receiver, node, ("reflective_argument_role",), argument_index=index)
                return UNKNOWN
            if (self.source.language == "ruby" and method == "instance_variable_get"
                    and receiver == self.receiver and len(arguments) == 1
                    and arguments[0].kind == "key" and re.fullmatch(r"@[A-Za-z_]\w*", arguments[0].name)
                    and not any(function.name == method for function in self.program.functions)):
                return slot(receiver, arguments[0].name[1:])
            if method in {"call", "invoke", "Invoke"} and receiver.kind in {"slot", "parameter"}:
                self.emit("invoke", receiver, UNKNOWN, node, ("callable_receiver_required",))
                return UNKNOWN
            if kind in {"map", "array", "set"}:
                entries = slot(receiver, MAP_ENTRIES) if kind == "map" else receiver
                if method in {"get", "GetValueOrDefault", "getOrDefault"} and arguments:
                    return slot(entries, arguments[0])
                if method in {"put", "set", "Set", "Add"} and kind == "map" and len(arguments) > 1:
                    self.emit("store", slot(entries, arguments[0]), arguments[1], node)
                    self.emit("store", slot(slot(receiver, MAP_KEYS), ELEMENT), arguments[0], node)
                    return receiver
                if method in {"append", "push", "push_back", "add", "Add", "insert", "Insert"} and arguments:
                    self.emit("store", slot(entries, ELEMENT), arguments[-1], node)
                    return receiver
                if method in {"remove", "Remove", "delete", "discard", "clear", "Clear", "removeAll"}:
                    self.emit("remove", slot(entries, arguments[0] if kind == "map" and arguments else ELEMENT), UNKNOWN, node)
                    return UNKNOWN
                if method in {"values", "Values", "items", "entrySet", "keys", "Keys", "iterator", "toArray"}:
                    base = slot(receiver, MAP_KEYS) if method.lower() == "keys" else entries
                    if self.source.language == "scala" and method == "toArray":
                        value = Value("allocation", self.allocation_name(node, "array_copy:"))
                        self.analyzer.kinds[value] = "array"
                        self.store_value(slot(value, ELEMENT), slot(base, ELEMENT), node, ("collection_snapshot_time_unproven",))
                        return value
                    return Value("iterator", method, slot(base, ELEMENT))
                if method in {"forEach", "foreach", "each", "map", "collect"}:
                    callback = next((item for item in argument_nodes if item.type in LAMBDAS), None) or block
                    if callback is not None:
                        if kind == "map" and method == "each" and self.source.language == "ruby":
                            self.inline_lambda(callback, slot(slot(receiver, MAP_KEYS), ELEMENT), slot(entries, ELEMENT))
                        else:
                            self.inline_lambda(callback, slot(entries, ELEMENT))
                    return UNKNOWN
            if method in {"__send__", "send", "public_send"} and self.source.language == "ruby" and arguments:
                self.emit("member_invoke", slot(receiver, arguments[0]), UNKNOWN, node, ("dynamic_method_resolution_required",))
            else:
                self.emit("invoke", callee, UNKNOWN, node)
                if callee.key.kind == "key":
                    self.emit("member_invoke", callee, UNKNOWN, node, ("method_dispatch_unproven",))
            owner = self.analyzer.owner(receiver)
            if owner.startswith("java:") and not self.program.jvm.methods(owner, method):
                actuals = list(dict.fromkeys(alias for alias in self.analyzer.resolved_aliases(receiver)
                                             if alias.kind == "allocation" and self.program.jvm.methods(self.analyzer.owner(alias), method)))
                if len(actuals) == 1:
                    receiver = actuals[0]
                    owner = self.analyzer.owner(receiver)
            returned = self.apply(owner, method, receiver, arguments, node) if owner else UNKNOWN
            if returned != UNKNOWN:
                return returned
        elif callee.kind == "function":
            function = next((function for function in self.program.functions if function.identifier == callee.name), None)
            if function:
                result = self.apply(function.owner or function.source.path, function.name, self.receiver, arguments, node)
                if result != UNKNOWN:
                    return result
        else:
            name = callee.name.rsplit(":", 1)[-1]
            if (self.source.language == "python" and name == "getattr" and callee.kind == "unresolved"
                    and name not in self.env and name not in self.local_bindings
                    and name not in self.program.module_bindings.get(self.source.path, set())
                    and len(arguments) == 2 and arguments[1].kind == "key"):
                return slot(arguments[0], arguments[1])
            if name in {"pairs", "ipairs", "iter", "enumerate"} and arguments:
                root = arguments[0]
                is_keys = name == "pairs"
                base = slot(root, MAP_KEYS) if is_keys else root
                return Value("iterator", "keys" if is_keys else "values", slot(base, ELEMENT))
            container = type_kind(name)
            if container or name in {"list", "dict", "set", "defaultdict", "mutableListOf", "mutableSetOf"}:
                value = Value("allocation", self.source.path + ":" + str(node.start_byte))
                self.analyzer.kinds[value] = container or ("map" if name in {"dict", "defaultdict"} else "array")
                if self.source.language == "scala" and name == "Array" and not self.program.declared_owner(self.source, name, self.owner):
                    for argument in arguments:
                        self.store_value(slot(value, ELEMENT), argument, node)
                return value
            self.emit("invoke", callee, UNKNOWN, node)
        for index, argument in enumerate(arguments):
            if argument.kind == "slot":
                self.emit("argument", argument, callee, node, argument_index=index)
        return Value("result", self.source.path + ":" + str(node.start_byte))

    def is_standard_static(self, receiver, qualified_name):
        root, keys = split_path(receiver)
        if root.kind != "unresolved" or any(key.kind != "key" for key in keys):
            return False
        name = root.name.rsplit(":", 1)[-1]
        if name in self.env or name in self.local_bindings:
            return False
        namespace, short_name = qualified_name.rsplit(".", 1)
        if any(type_name == short_name and (path == self.source.path or self.program.namespaces.get(path) == namespace)
               for path, type_name in self.program.types):
            return False
        spelling = ".".join([name, *(key.name for key in keys)])
        return spelling == qualified_name or (spelling == short_name and namespace in self.program.namespace_imports[self.source.path])

    def has_standard_type(self, value, qualified_name):
        info = self.analyzer.type_info(value)
        if not info:
            return False
        source, type_text, scope = info
        spelling = self.program.expanded_type(source, type_text, scope).strip().rstrip("?")
        if self.program.declared_owner(source, spelling, scope) in self.program.owner_sources:
            return False
        return spelling == qualified_name or self.program.imports.get((source.path, spelling)) == qualified_name

    def java_sam_method(self, type_text):
        name = re.sub(r"<.*>", "", type_text).strip()
        owner = self.program.declared_owner(self.source, name)
        if owner:
            return self.program.sam_methods.get(owner, "")
        imported = self.program.imports.get((self.source.path, name))
        if name == "java.lang.Runnable" or (name == "Runnable" and imported in {None, "java.lang.Runnable"}):
            return "run"
        return ""

    def capture_closure(self, node):
        captures = child(node, "captures")
        captured = {}
        for item in parts(captures):
            name = self.text(item)
            if item.type == "this":
                captured["this"] = self.receiver
            elif item.type == "identifier":
                captured[name] = self.binding(name)
            else:
                self.analyzer.notices.add(("closure_capture_form_unsupported", self.identifier + ":" + str(node.start_byte)))
                return UNKNOWN
        # Reference/default/object-copy captures need lifetime and alias rules;
        # do not silently give them the semantics of a by-value capture.
        if any(token in self.text(captures) for token in ("&", "=", "*")) or len(captured) > 8:
            self.analyzer.notices.add(("closure_capture_form_unsupported", self.identifier + ":" + str(node.start_byte)))
            return UNKNOWN
        identifier = self.source.path + ":closure:" + str(node.start_byte)
        owner = self.owner if "this" in captured else ""
        self.analyzer.closures[identifier] = (self.source, node, owner, tuple(captured))
        return Value("closure", identifier, tuple_value(list(captured.values())))

    def invoke_closure(self, callee, arguments, call_node):
        definition = self.analyzer.closures.get(callee.name)
        if definition is None or callee.name in self.active_closures or len(self.active_closures) >= 4:
            self.analyzer.notices.add(("closure_expansion_bound", callee.name))
            return UNKNOWN
        source, node, owner, names = definition
        captures = dict(zip(names, tuple_values(callee.base)))
        nested = Interpreter(self.analyzer, source, owner=owner, env=captures)
        nested.identifier = callee.name
        nested.active_closures = self.active_closures + (callee.name,)
        if "this" in captures:
            nested.receiver = captures["this"]
            nested.env["this"] = captures["this"]
        parameters = child(child(node, "declarator"), "parameters")
        for index, parameter in enumerate(parts(parameters)):
            named = identifier(child(parameter, "declarator"))
            if named is not None:
                nested.env[source.text(named)] = arguments[index] if index < len(arguments) else UNKNOWN
        nested.statement(child(node, "body"))
        for fact in nested.facts:
            if len(self.facts) >= MAX_FACTS:
                self.analyzer.notices.add(("function_fact_cap", self.identifier))
                break
            self.facts.append(Fact(fact.kind, fact.target, fact.value, fact.location, self.identifier,
                                   tuple(sorted(set(fact.conditions + ("closure_capture_binding_required",)))),
                                   fact.via + (self.source.location(call_node),), fact.argument_index))
        return merge_values(nested.returns)

    def inline_lambda(self, node, value, second=UNKNOWN):
        params = child(node, "parameters") or first(node, {"lambda_parameters", "block_parameters"})
        names = [item for item in walk(params) if item.type in IDENTIFIERS]
        previous = dict(self.env)
        if names:
            self.env[self.text(names[0])] = value
            if len(names) > 1:
                self.env[self.text(names[1])] = second
        else:
            self.env["it"] = value
            self.env["$0"] = value
        body = child(node, "body") or first(node, {"statements", "block", "body_statement"})
        self.statement(body)
        self.env = previous

    def loop(self, node):
        right = child(node, "right", "value", "collection", "iterable")
        pattern = child(node, "left", "name", "pattern")
        body = child(node, "body") or first(node, {"block", "compound_statement", "statements"})
        is_reference = False
        if self.source.language == "php" and node.type == "foreach_statement":
            children = parts(node)
            if len(children) >= 3:
                right, pattern, body = children[0], children[1], children[-1]
                if pattern.type == "pair":
                    pattern = parts(pattern)[-1]
                if pattern.type == "by_ref" and parts(pattern):
                    is_reference = True
                    pattern = parts(pattern)[-1]
        if self.source.language == "lua":
            clause = first(node, {"for_generic_clause"}) or node
            names = first(clause, {"variable_list"})
            expressions = first(clause, {"expression_list"})
            pattern = parts(names)[0] if parts(names) else None
            right = parts(expressions)[0] if parts(expressions) else None
        if right is not None and pattern is not None:
            source = self.expression(right)
            alternatives = []
            for option in value_options(source):
                if option.kind == "iterator":
                    alternatives.append(option.base)
                else:
                    base = slot(option, MAP_KEYS) if self.analyzer.kind(option) == "map" else option
                    alternatives.append(slot(base, ELEMENT))
            value = merge_values(alternatives)
            self.bind(pattern, value)
            if is_reference:
                self.references[self.text(pattern)] = value
        for item in parts(node):
            if item not in {right, pattern, body}:
                self.statement(item)
        self.statement(body)


def load_sources(root, paths, language, max_file_bytes=512 * 1024):
    parser = parser_for(language)
    sources = []
    notices = []
    total = 0
    for path in sorted(set(paths)):
        target = root / path
        if not target.resolve().is_relative_to(root.resolve()) or target.is_symlink():
            raise ValueError("Input path escapes repository: " + path)
        if not target.is_file():
            notices.append({"kind": "missing_input", "path": path})
            continue
        size = target.stat().st_size
        if size > max_file_bytes or total + size > 64 * 1024 * 1024 or len(sources) >= 4096:
            notices.append({"kind": "input_cap", "path": path})
            continue
        data = target.read_bytes()
        total += len(data)
        tree = parser.parse(data)
        has_projection = False
        has_conditional_projection = False
        if language in {"c", "cpp"} and tree.root_node.has_error:
            projected = project_preprocessor(data)
            alternative = parser.parse(projected)
            if not alternative.root_node.has_error and len(alternative.root_node.named_children) > 0:
                tree = alternative
                has_projection = True
                notices.append({"kind": "preprocessor_branch_projection", "path": path,
                                "detail": "one conditional source view with include guards retained; macro values and other builds remain unproven"})
        if language == "swift" and tree.root_node.has_error:
            projected = b"".join(bytes(byte if byte in {10, 13} else 32 for byte in line)
                                 if re.match(rb"\s*#(?:if|elseif|else|endif)\b", line) else line
                                 for line in data.splitlines(keepends=True))
            alternative = parser.parse(projected)
            if projected != data and not alternative.root_node.has_error:
                tree = alternative
                has_conditional_projection = True
                notices.append({"kind": "conditional_compilation_projection", "path": path,
                                "detail": "byte-aligned directive masking; branch selection and module binding remain unproven"})
        source = Source(path, language, data, tree)
        source.has_branch_projection = has_projection
        source.has_conditional_projection = has_conditional_projection
        sources.append(source)
        if tree.root_node.has_error:
            errors = syntax_errors(source)
            notices.append({"kind": "parse_error", "path": path, "locations": errors[:8], "count": len(errors)})
    return sources, notices


def project_preprocessor(data):
    """One explicitly conditional, byte-aligned view; never a build claim."""
    frames = []
    is_active = True
    output = []
    lines = data.splitlines(keepends=True)
    for index, line in enumerate(lines):
        match = re.match(rb"\s*#\s*(if|ifdef|ifndef|elif|else|endif)\b", line)
        if match:
            directive = match.group(1)
            if directive in {b"if", b"ifdef", b"ifndef"}:
                guard = re.match(rb"\s*#\s*ifndef\s+(\w+)", line)
                is_guard = bool(guard and re.search(rb"#\s*define\s+" + re.escape(guard.group(1)) + rb"\b", b"".join(lines[index + 1:index + 5])))
                frames.append((is_active, is_guard))
                is_active = is_active and is_guard
            elif directive in {b"else", b"elif"} and frames:
                is_parent_active, is_first_selected = frames[-1]
                is_active = is_parent_active and not is_first_selected if directive == b"else" else False
            elif directive == b"endif" and frames:
                is_active = frames.pop()[0]
        output.append(line if is_active and not match else bytes(byte if byte in {10, 13} else 32 for byte in line))
    return b"".join(output)
