"""Parse source and expose syntax facts, without framework-specific API rules."""

from dataclasses import dataclass, field
from bisect import bisect_right
from functools import cached_property
from pathlib import Path
import re

from tree_sitter import Language, Parser
import tree_sitter_go
import tree_sitter_rust
import tree_sitter_typescript

from model import Location, Value


FUNCTIONS = {"function_declaration", "function_expression", "generator_function", "generator_function_declaration",
             "method_definition", "arrow_function", "function_item", "method_declaration",
             "func_literal", "closure_expression"}
CLASSES = {"class_declaration", "class", "impl_item"}
IDENTIFIERS = {"identifier", "property_identifier", "private_property_identifier", "field_identifier", "type_identifier",
               "shorthand_property_identifier", "shorthand_property_identifier_pattern"}


def walk(node, stop_functions=False):
    yield node
    for child in node.named_children:
        if stop_functions and child.type in FUNCTIONS:
            continue
        yield from walk(child, stop_functions)


def child(node, name, fallback=None):
    return node.child_by_field_name(name) if node is not None else fallback


def container_kind(type_text: str) -> str:
    value = re.sub(r"\s+", "", type_text).lstrip(":")
    if re.match(r"(?:Map|ReadonlyMap|HashMap|BTreeMap)<", value) or value.startswith("map["):
        return "map"
    if re.match(r"(?:Set|HashSet|BTreeSet)<", value):
        return "set"
    if value.startswith("[]") or (value.endswith("[]") and not value.startswith("{")) or re.match(r"(?:Array|ReadonlyArray|Vec|VecDeque)<", value):
        return "array"
    return ""


def element_type(type_text: str) -> str:
    value = type_text.strip().lstrip(": ")
    if value.startswith("[]"):
        return value[2:]
    if value.endswith("[]"):
        return value[:-2]
    if value.startswith("map["):
        return value[value.index("]") + 1:]
    mapped = re.match(r"\{\s*\[[^]]+\]\s*:\s*(.+)\s*\}", value, re.S)
    if mapped:
        return mapped.group(1).strip()
    if "<" in value and value.endswith(">"):
        arguments = value[value.index("<") + 1:-1]
        depth = 0
        parts = []
        start = 0
        for index, character in enumerate(arguments):
            if character in "<([{":
                depth += 1
            elif character in ">)]}":
                depth -= 1
            elif character == "," and depth == 0:
                parts.append(arguments[start:index].strip())
                start = index + 1
        parts.append(arguments[start:].strip())
        if container_kind(value) == "map" and len(parts) > 1:
            return parts[1]
        if container_kind(value) in {"array", "set"}:
            return parts[0]
    return ""


@dataclass
class Source:
    path: str
    language: str
    data: bytes
    tree: object

    @cached_property
    def line_starts(self):
        return [0] + [index + 1 for index, byte in enumerate(self.data) if byte == 10]

    def text(self, node) -> str:
        return self.data[node.start_byte:node.end_byte].decode("utf8", errors="replace") if node else ""

    def location(self, node) -> Location:
        # Byte offsets avoid the native Point.column accessor implicated in
        # py-tree-sitter issue #487 and preserve byte-accurate source coordinates.
        offset = node.start_byte
        row = bisect_right(self.line_starts, offset) - 1
        return Location(self.path, row + 1, offset - self.line_starts[row] + 1)


@dataclass
class Function:
    identifier: str
    name: str
    owner: str
    source: Source
    node: object
    body: object
    parameters: list[tuple[str, str]]
    receiver_name: str
    parent: "Function | None" = None

    @property
    def receiver(self) -> Value:
        return Value("receiver", self.owner) if self.owner else Value("unknown")


@dataclass
class Program:
    sources: list[Source] = field(default_factory=list)
    functions: dict[str, Function] = field(default_factory=dict)
    nodes: dict[tuple[str, int], Function] = field(default_factory=dict)
    methods: dict[tuple[str, str], list[str]] = field(default_factory=dict)
    field_types: dict[tuple[str, str], str] = field(default_factory=dict)
    types: dict[tuple[str, str], str] = field(default_factory=dict)
    aliases: dict[tuple[str, str], str] = field(default_factory=dict)
    imports: dict[tuple[str, str], str] = field(default_factory=dict)
    embedded_fields: dict[str, list[str]] = field(default_factory=dict)
    interface_methods: dict[str, set[str]] = field(default_factory=dict)
    notices: list[dict] = field(default_factory=list)

    def owner_key(self, source: Source, name: str) -> str:
        namespace = str(Path(source.path).parent) if source.language == "go" else source.path
        return namespace + "::" + name

    def type_name(self, source: Source, name: str) -> tuple[str, str]:
        namespace = str(Path(source.path).parent) if source.language == "go" else source.path
        if source.language == "go" and "." in name:
            prefix, name = name.split(".", 1)
            return self.imports.get((source.path, prefix), ""), name
        imported = self.imports.get((source.path, name), "")
        if imported and "#" in imported:
            return tuple(imported.rsplit("#", 1))
        return namespace, name

    def expand_type(self, source: Source, type_text: str) -> str:
        value = type_text.strip().lstrip(": ")
        namespace, name = self.type_name(source, value)
        seen = set()
        for _ in range(12):
            key = (namespace, name)
            if key in seen or key not in self.aliases:
                break
            seen.add(key)
            value = self.aliases[key].strip()
            if not re.fullmatch(r"[A-Za-z_$][\w$]*", value):
                break
            name = value
        return value

    def resolve_type(self, source: Source, name: str) -> str:
        clean = re.sub(r"[&*]|\bmut\b|\bconst\b|\s", "", name).split("<")[0]
        namespace, clean = self.type_name(source, clean)
        seen = set()
        while (namespace, clean) in self.aliases and (namespace, clean) not in seen:
            if (namespace, clean) in self.types:
                break
            seen.add((namespace, clean))
            target = self.aliases[(namespace, clean)].strip()
            if not re.fullmatch(r"[A-Za-z_$][\w$]*", target):
                break
            clean = target
        if not re.fullmatch(r"[A-Za-z_$][\w$]*", clean):
            return ""
        return self.types.get((namespace, clean), "")


def parameter_names(source: Source, params) -> list[tuple[str, str]]:
    if params is None:
        return []
    if params.type == "identifier":
        return [(source.text(params), "")]
    result = []
    for item in params.named_children:
        if item.type in {"comment", "type_parameter", "self_parameter"}:
            continue
        pattern = child(item, "pattern") or child(item, "name")
        type_node = child(item, "type")
        if pattern is None and item.type in IDENTIFIERS:
            pattern = item
        if pattern is None:
            pattern = next((part for part in item.named_children if part.type == "identifier"), None)
        if pattern is not None and pattern.type == "identifier":
            result.append((source.text(pattern), source.text(type_node)))
        elif pattern is not None:
            # Destructuring is explicit in the body interpreter; do not invent aliases here.
            result.append((source.text(pattern), source.text(type_node)))
    return result


def load_program(root: Path, paths: list[str], max_files=4096, max_bytes=64 * 1024 * 1024) -> Program:
    root = root.resolve()
    program = Program()
    parsers = {
        "typescript": Parser(Language(tree_sitter_typescript.language_typescript())),
        "tsx": Parser(Language(tree_sitter_typescript.language_tsx())),
        "rust": Parser(Language(tree_sitter_rust.language())),
        "go": Parser(Language(tree_sitter_go.language())),
    }
    extensions = {".ts": "typescript", ".js": "typescript", ".mjs": "typescript", ".cjs": "typescript",
                  ".tsx": "tsx", ".jsx": "tsx", ".rs": "rust", ".go": "go"}
    candidates = set()
    for item in paths:
        target = (root / item).resolve()
        if not target.is_relative_to(root.resolve()):
            raise ValueError(f"Source path escapes repository: {item}")
        if not target.exists():
            program.notices.append({"kind": "missing_input", "path": item})
            continue
        if target.is_file():
            candidates.add(target)
        else:
            candidates.update(file for file in target.rglob("*") if file.is_file())
    total_bytes = 0
    for path in sorted(candidates):
        if path.suffix not in extensions or any(part in {".git", "node_modules", "target", "dist", "vendor"} for part in path.parts):
            continue
        if path.name.endswith((".test.ts", ".spec.ts", "_test.go")):
            continue
        if path.is_symlink():
            program.notices.append({"kind": "symlink_not_followed", "path": path.relative_to(root).as_posix()})
            continue
        size = path.stat().st_size
        relative = path.relative_to(root).as_posix()
        if size > 512 * 1024 or total_bytes + size > max_bytes or len(program.sources) >= max_files:
            program.notices.append({"kind": "input_cap", "path": relative, "bytes": size})
            continue
        data = path.read_bytes()
        language = extensions[path.suffix]
        tree = parsers[language].parse(data)
        source = Source(relative, language, data, tree)
        program.sources.append(source)
        total_bytes += size
        if tree.root_node.has_error:
            errors = [source.location(node) for node in walk(tree.root_node) if node.type == "ERROR" or node.is_missing]
            program.notices.append({"kind": "parse_error", "path": relative, "locations": errors[:12], "count": len(errors)})

    for source in program.sources:
        for node in walk(source.tree.root_node):
            if node.type == "type_alias_declaration":
                program.aliases[(source.path, source.text(child(node, "name")))] = source.text(child(node, "value"))
            elif node.type == "type_item":
                program.aliases[(source.path, source.text(child(node, "name")))] = source.text(child(node, "type"))
            if node.type in {"class_declaration", "interface_declaration", "struct_item", "type_spec"}:
                name = source.text(child(node, "name"))
                owner = program.owner_key(source, name)
                namespace = str(Path(source.path).parent) if source.language == "go" else source.path
                program.types[(namespace, name)] = owner
                if node.type == "type_spec":
                    program.aliases[(namespace, name)] = source.text(child(node, "type"))
                body = child(node, "body") or child(node, "type")
                if body:
                    for field_node in walk(body, stop_functions=True):
                        if field_node.type in {"public_field_definition", "field_declaration", "property_signature"}:
                            field_name = source.text(child(field_node, "name"))
                            if not field_name and source.language == "go" and field_node.type == "field_declaration":
                                field_type = source.text(child(field_node, "type")).lstrip("*")
                                if re.fullmatch(r"[A-Za-z_][\w.]*", field_type):
                                    field_name = field_type.rsplit(".", 1)[-1]
                                    program.embedded_fields.setdefault(owner, []).append(field_name)
                            if field_name:
                                program.field_types[(owner, field_name)] = source.text(child(field_node, "type"))
                        if body.type == "interface_type" and field_node.type in {"method_elem", "method_spec"}:
                            method_name = source.text(child(field_node, "name"))
                            if method_name:
                                program.interface_methods.setdefault(owner, set()).add(method_name)

    source_paths = {source.path for source in program.sources}
    namespaces = {namespace for namespace, _ in program.types}
    for source in program.sources:
        for node in walk(source.tree.root_node, stop_functions=True):
            if source.language == "go" and node.type == "import_spec":
                import_path = source.text(child(node, "path")).strip('"`')
                alias = source.text(child(node, "name")) or import_path.rsplit("/", 1)[-1]
                candidates = [namespace for namespace in namespaces
                              if import_path == namespace or import_path.endswith("/" + namespace)]
                if len(candidates) == 1:
                    program.imports[(source.path, alias)] = candidates[0]
            if source.language in {"typescript", "tsx"} and node.type == "import_statement":
                specifier = source.text(child(node, "source")).strip("'\"")
                if not specifier.startswith("."):
                    continue
                target = (root / Path(source.path).parent / specifier).resolve()
                candidates = [target, target.with_suffix(".ts"), target.with_suffix(".tsx"), target / "index.ts"]
                available = [path.relative_to(root).as_posix() for path in candidates
                             if path.is_relative_to(root) and path.relative_to(root).as_posix() in source_paths]
                available = list(dict.fromkeys(available))
                if len(available) != 1:
                    continue
                for part in walk(node):
                    if part.type == "import_specifier":
                        exported = source.text(child(part, "name"))
                        alias = source.text(child(part, "alias")) or exported
                        program.imports[(source.path, alias)] = available[0] + "#" + exported

    for source in program.sources:
        def collect(node, owner="", parent=None):
            if node.type in CLASSES:
                name = source.text(child(node, "name") or child(node, "type"))
                if name:
                    owner = program.owner_key(source, name)
            if node.type in FUNCTIONS:
                if parent and source.language in {"typescript", "tsx"} and node.type not in {"arrow_function", "method_definition"}:
                    owner = ""
                receiver_name = "self" if source.language == "rust" else "this"
                if node.type == "method_declaration":
                    receiver = child(node, "receiver")
                    receiver_params = parameter_names(source, receiver)
                    if receiver_params:
                        receiver_name, receiver_type = receiver_params[0]
                        owner = program.resolve_type(source, receiver_type)
                name = source.text(child(node, "name"))
                if not name and node.parent and node.parent.type in {"variable_declarator", "pair", "public_field_definition"}:
                    name = source.text(child(node.parent, "name") or child(node.parent, "key"))
                location = source.location(node)
                identifier = f"{source.path}:{location.line}:{location.column}"
                params = child(node, "parameters") or child(node, "parameter")
                if name == "constructor" and owner and params:
                    for parameter in params.named_children:
                        if any(part.type == "accessibility_modifier" for part in parameter.named_children):
                            parameter_name = source.text(child(parameter, "pattern") or child(parameter, "name"))
                            program.field_types[(owner, parameter_name)] = source.text(child(parameter, "type"))
                function = Function(identifier, name or f"closure@{location.line}", owner, source,
                                    node, child(node, "body"), parameter_names(source, params), receiver_name, parent)
                program.functions[identifier] = function
                program.nodes[(source.path, node.start_byte)] = function
                namespace = str(Path(source.path).parent) if source.language == "go" else source.path
                program.methods.setdefault((owner or namespace, function.name), []).append(identifier)
                parent = function
            for nested in node.named_children:
                collect(nested, owner, parent)
        collect(source.tree.root_node)
    return program
