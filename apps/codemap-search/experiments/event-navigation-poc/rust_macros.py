"""Expand same-scope, single-ident declarative item macros from their source.

Literal impl templates and restricted identifier-shadowing statement templates
are accepted. Repetitions, general hygiene and procedural execution stay bounds.
"""

from bisect import bisect_right
from copy import copy
import re


def expand_ident_impl_macros(source, parser, max_file_bytes):
    from syntax import child, walk
    definitions = {}
    root = source.tree.root_node
    for node in root.named_children:
        if node.type == "macro_definition":
            definitions.setdefault(source.text(child(node, "name")), []).append(node)
    edits = []
    for statement in root.named_children:
        invocation = statement
        if statement.type == "expression_statement" and len(statement.named_children) == 1:
            invocation = statement.named_children[0]
        if invocation.type != "macro_invocation":
            continue
        macro = source.text(child(invocation, "macro"))
        candidates = [node for node in definitions.get(macro, ()) if node.start_byte < invocation.start_byte]
        if len(candidates) != 1:
            continue
        definition = candidates[0]
        rules = [node for node in definition.named_children if node.type == "macro_rule"]
        if len(rules) != 1:
            continue
        pattern, body = child(rules[0], "left"), child(rules[0], "right")
        match = re.fullmatch(r"\(\s*\$(\w+)\s*:\s*ident\s*\)", source.text(pattern))
        arguments = next((node for node in invocation.named_children if node.type == "token_tree"), None)
        values = [node for node in arguments.named_children if node.type not in {"line_comment", "block_comment"}] if arguments else []
        if not match or len(values) != 1 or values[0].type not in {"identifier", "type_identifier"}:
            continue
        variable, argument = "$" + match[1], values[0]
        replacements = []
        supported = True
        for node in walk(body):
            if "repetition" in node.type:
                supported = False
            if node.type == "metavariable":
                if source.text(node) != variable:
                    supported = False
                replacements.append(node)
        if not supported or not replacements:
            continue
        start, end = body.start_byte + 1, body.end_byte - 1
        chunks = []
        cursor = start
        for variable_node in replacements:
            chunks.append((source.data[cursor:variable_node.start_byte], cursor))
            chunks.append((source.data[argument.start_byte:argument.end_byte], argument.start_byte))
            cursor = variable_node.end_byte
        chunks.append((source.data[cursor:end], cursor))
        expanded = b"".join(data for data, _ in chunks)
        tree = parser.parse(expanded)
        allowed = {"impl_item", "line_comment", "block_comment", "attribute_item"}
        if tree.root_node.has_error or not tree.root_node.named_children or any(node.type not in allowed for node in tree.root_node.named_children):
            continue
        edits.append((statement.start_byte, statement.end_byte, invocation.start_byte, chunks))
    if not edits:
        return 0
    expected_bytes = len(source.data) + sum(sum(len(data) for data, _ in chunks) - (end - start) for start, end, _, chunks in edits)
    if len(edits) > 256 or expected_bytes > max_file_bytes:
        return 0
    chunks, segments, expansions = [], [], []
    length, cursor = 0, 0
    def append(data, origin):
        nonlocal length
        if data:
            segments.append((length, length + len(data), origin))
            chunks.append(data)
            length += len(data)
    for start, end, invocation, expanded_chunks in edits:
        append(source.data[cursor:start], cursor)
        expansion_start = length
        for data, origin in expanded_chunks:
            append(data, origin)
        expansions.append((expansion_start, length, invocation))
        cursor = end
    append(source.data[cursor:], cursor)
    parsed_data = b"".join(chunks)
    tree = parser.parse(parsed_data)
    if tree.root_node.has_error:
        return 0
    source.parsed_data = parsed_data
    source.parsed_segments = segments
    source.parsed_segment_starts = [segment[0] for segment in segments]
    source.macro_expansions = expansions
    source.tree = tree
    return len(edits)


def original_offset(source, offset):
    segments = getattr(source, "parsed_segments", ())
    if not segments:
        return offset
    index = bisect_right(source.parsed_segment_starts, offset) - 1
    if index < 0 or offset >= segments[index][1]:
        return offset
    segment = segments[index]
    start, _, origin = segment[:3]
    return origin + offset - start if len(segment) == 3 or segment[3] else origin


def expand_ident_statement_macros(sources, parser, crate_bindings, max_file_bytes):
    """Expand explicit single-ident imports without guessing macro API names.

    The body may only shadow its identifier argument. Other identifiers must be
    fully qualified paths or member names; fresh locals/repetitions are rejected.
    `$crate` receives a collision-free namespace bound to its definition source.
    """
    from syntax import child, rust_use_paths, walk
    by_path = {source.path: source for source in sources if source.language == "rust"}
    templates = {path: copy(source) for path, source in by_path.items()}
    definitions, exported = {}, {}
    for source in by_path.values():
        local, attributes = {}, []
        for node in source.tree.root_node.named_children:
            if node.type in {"line_comment", "block_comment"}:
                continue
            if node.type == "attribute_item":
                attributes.append(source.text(node))
                continue
            if node.type == "macro_definition":
                name = source.text(child(node, "name"))
                local.setdefault(name, []).append(node)
                if any(re.fullmatch(r"#\[\s*macro_export\s*\]", text) for text in attributes):
                    exported.setdefault((source.path, name), []).append(node)
            attributes = []
        definitions[source.path] = local
    notices = []
    for source in by_path.values():
        imports = {}
        for node in source.tree.root_node.named_children:
            if node.type == "use_declaration":
                for alias, path in rust_use_paths(source, child(node, "argument")):
                    imports.setdefault(alias, set()).add(path)
        nested_names = {source.text(child(node, "name")) for node in walk(source.tree.root_node)
                        if node.type == "macro_definition" and node.parent != source.tree.root_node}
        edits, generated_imports, metadata = [], {}, {}
        for invocation in walk(source.tree.root_node):
            statement = invocation.parent
            if (invocation.type != "macro_invocation" or statement is None
                    or statement.type != "expression_statement" or statement.parent.type != "block"
                    or expansion_identity(source, invocation)):
                continue
            # Local scopes can shadow imported macros. Reject rather than merge.
            name = source.text(child(invocation, "macro"))
            if name in nested_names:
                continue
            candidates = [(templates[source.path], node) for node in definitions[source.path].get(name, ())
                          if node.start_byte < invocation.start_byte]
            if not candidates:
                paths = imports.get(name, {name} if "::" in name else set())
                if len(paths) != 1:
                    continue
                for path in paths:
                    parts = path.split("::")
                    if len(parts) != 2 or parts[0] not in crate_bindings:
                        continue
                    target = templates.get(crate_bindings[parts[0]])
                    if target is not None:
                        candidates.extend((target, node) for node in exported.get((target.path, parts[1]), ()))
            if len(candidates) != 1:
                continue
            definition_source, definition = candidates[0]
            rules = [node for node in definition.named_children if node.type == "macro_rule"]
            if len(rules) != 1:
                continue
            pattern, body = child(rules[0], "left"), child(rules[0], "right")
            match = re.fullmatch(r"\(\s*\$(\w+)\s*:\s*ident\s*\)", definition_source.text(pattern))
            tokens = next((node for node in invocation.named_children if node.type == "token_tree"), None)
            arguments = [node for node in tokens.named_children if node.type not in {"line_comment", "block_comment"}] if tokens else []
            if not match or len(arguments) != 1 or arguments[0].type != "identifier":
                continue
            variable, argument = "$" + match[1], source.text(arguments[0])
            parts = list(walk(body))
            variables = [node for node in parts if node.type == "metavariable"]
            if any("repetition" in node.type for node in parts) or any(definition_source.text(node) not in {variable, "$crate"} for node in variables):
                continue
            alias = "__poc_macro_crate_" + str(len(edits))
            if alias in source.text(source.tree.root_node):
                continue
            if any(definition_source.text(node) == "$crate" for node in variables) and definition_source.path not in crate_bindings.values():
                continue
            data = getattr(definition_source, "parsed_data", definition_source.data)
            cursor, chunks = body.start_byte + 1, []
            for node in variables:
                chunks.extend([data[cursor:node.start_byte], (alias if definition_source.text(node) == "$crate" else argument).encode()])
                cursor = node.end_byte
            chunks.append(data[cursor:body.end_byte - 1])
            expanded = b"".join(chunks)
            if not statement_template_is_safe(expanded, argument, alias, parser):
                continue
            origin = original_offset(source, invocation.start_byte)
            edits.append((statement.start_byte, statement.end_byte, origin, expanded))
            generated_imports[alias] = definition_source.path
            metadata[origin] = definition_source.location(definition)
        if edits and apply_statement_edits(source, parser, edits, max_file_bytes):
            source.generated_imports = generated_imports
            source.statement_macro_definitions = metadata
            notices.append({"kind": "rust_declarative_statement_expansion", "path": source.path, "expansions": len(edits)})
    return notices


def statement_template_is_safe(expanded, argument, alias, parser):
    from syntax import child, walk
    data = b"fn __poc_template() {" + expanded + b"}"
    tree = parser.parse(data)
    if tree.root_node.has_error or len(tree.root_node.named_children) != 1:
        return False
    body = child(tree.root_node.named_children[0], "body")
    text = lambda node: data[node.start_byte:node.end_byte].decode()
    if body is None or not body.named_children:
        return False
    for node in body.named_children:
        if node.type in {"line_comment", "block_comment"}:
            continue
        pattern = child(node, "pattern")
        if node.type != "let_declaration" or pattern is None or pattern.type != "identifier" or text(pattern) != argument:
            return False
    for node in walk(body):
        if node.type in {"macro_invocation", "macro_definition", "closure_expression", "function_item", "mod_item", "use_declaration"}:
            return False
        if node.type not in {"identifier", "type_identifier"} or text(node) == argument:
            continue
        ancestor = node.parent
        while ancestor is not None and ancestor.type in {"scoped_identifier", "scoped_type_identifier", "generic_type", "generic_function"}:
            if text(ancestor).startswith(("::core::", "::std::", "::alloc::", alias + "::")):
                break
            ancestor = ancestor.parent
        else:
            return False
    return True


def apply_statement_edits(source, parser, edits, max_file_bytes):
    """Compose parser offsets while keeping original bytes, hashes and locations."""
    data = getattr(source, "parsed_data", source.data)
    if len(edits) > 256 or len(data) + sum(len(text) - (end - start) for start, end, _, text in edits) > max_file_bytes:
        return False
    old_segments = getattr(source, "parsed_segments", [(0, len(data), 0)])
    chunks, segments, expansions = [], [], []
    length, cursor = 0, 0
    def append(text, origin, linear=True):
        nonlocal length
        if text:
            segments.append((length, length + len(text), origin, linear))
            chunks.append(text)
            length += len(text)
    def unchanged(start, end):
        for segment in old_segments:
            left, right = max(start, segment[0]), min(end, segment[1])
            if left < right:
                linear = len(segment) == 3 or segment[3]
                append(data[left:right], segment[2] + left - segment[0] if linear else segment[2], linear)
    for start, end, origin, expanded in edits:
        unchanged(cursor, start)
        begin = length
        append(expanded, origin, False)
        expansions.append((begin, length, origin))
        cursor = end
    unchanged(cursor, len(data))
    parsed_data = b"".join(chunks)
    tree = parser.parse(parsed_data)
    if tree.root_node.has_error:
        return False
    def shifted(offset):
        return offset + sum(len(text) - (end - start) for start, end, _, text in edits if end <= offset)
    expansions.extend((shifted(start), shifted(end), origin) for start, end, origin in getattr(source, "macro_expansions", ()))
    source.parsed_data, source.parsed_segments = parsed_data, segments
    source.parsed_segment_starts = [segment[0] for segment in segments]
    source.macro_expansions, source.tree = sorted(expansions), tree
    return True


def statement_expansion_definition(source, node):
    for start, end, origin in getattr(source, "macro_expansions", ()):
        if start <= node.start_byte < end:
            return getattr(source, "statement_macro_definitions", {}).get(origin)
    return None


def expansion_identity(source, node):
    for start, end, origin in getattr(source, "macro_expansions", ()):
        if start <= node.start_byte < end:
            if origin in getattr(source, "statement_macro_definitions", {}):
                return f":macro@{origin}:node@{node.start_byte - start}"
            return f":macro@{origin}"
    return ""


def expansion_location(source, node):
    from model import Location
    for start, end, origin in getattr(source, "macro_expansions", ()):
        if start <= node.start_byte < end:
            row = bisect_right(source.line_starts, origin) - 1
            return Location(source.path, row + 1, origin - source.line_starts[row] + 1)
    return None
