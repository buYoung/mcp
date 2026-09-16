"""Resolve Rust imports using input crate roots and explicit module declarations."""

from collections import defaultdict
from pathlib import PurePosixPath


def resolve_modules(program):
    from syntax import child, rust_use_paths

    locations = defaultdict(list)
    for source in program.sources:
        if source.language != "rust":
            continue
        path = PurePosixPath(source.path)
        parts = path.parts
        index = max((i for i, part in enumerate(parts) if part == "src"), default=-1)
        root = str(PurePosixPath(*parts[:index + 1])) if index >= 0 else "."
        relative = parts[index + 1:] if index >= 0 else parts
        module = relative[:-1] + (() if path.stem in {"lib", "main", "mod"} else (path.stem,))
        locations[(root, module)].append(source)
    available = {key: sources[0] for key, sources in locations.items() if len(sources) == 1}
    reachable = {key: source for key, source in available.items() if not key[1] and PurePosixPath(source.path).stem in {"lib", "main"}}
    for _ in range(12):
        added = {}
        for (root, module), source in reachable.items():
            for node in source.tree.root_node.named_children:
                if node.type == "mod_item" and child(node, "body") is None:
                    key = (root, module + (source.text(child(node, "name")),))
                    if key in available and key not in reachable:
                        added[key] = available[key]
        if not added:
            break
        reachable.update(added)
    symbols = defaultdict(set)
    imports = []
    for key, source in reachable.items():
        for node in source.tree.root_node.named_children:
            if node.type in {"struct_item", "enum_item", "trait_item", "type_item", "function_item", "const_item", "static_item"}:
                name = source.text(child(node, "name"))
                symbols[(key, name)].add(source.path + "#" + name)
            if node.type == "use_declaration":
                is_public = any(part.type == "visibility_modifier" for part in node.named_children)
                imports.extend((key, source, alias, path, is_public) for alias, path in rust_use_paths(source, child(node, "argument")))

    def module_candidates(key, path):
        root, current = key
        parts = path.split("::")
        if parts[0] == "crate":
            return [(root, tuple(parts[1:]))]
        if parts[0] == "self":
            return [(root, current + tuple(parts[1:]))]
        if parts[0] == "super":
            while parts and parts[0] == "super":
                if not current:
                    return []
                current, parts = current[:-1], parts[1:]
            return [(root, current + tuple(parts))]
        # Bare re-exports may name an explicitly declared child module. External
        # crate names never resolve merely by a type spelling in another crate.
        return list(dict.fromkeys([(root, current + tuple(parts)), (root, tuple(parts))]))

    resolved = defaultdict(set)
    for _ in range(8):
        previous = sum(len(values) for values in symbols.values())
        for key, source, alias, path, is_public in imports:
            if alias == "*":
                for module in module_candidates(key, path.removesuffix("::*")):
                    if module not in reachable:
                        continue
                    for (owner, name), values in list(symbols.items()):
                        if owner == module:
                            resolved[(source.path, name)].update(values)
                            if is_public:
                                symbols[(key, name)].update(values)
                continue
            values = set()
            for full in module_candidates(key, path):
                if full in reachable:
                    values.add(reachable[full].path)
                if full[1]:
                    values.update(symbols.get(((full[0], full[1][:-1]), full[1][-1]), ()))
            resolved[(source.path, alias)].update(values)
            if is_public:
                symbols[(key, alias)].update(values)
        if sum(len(values) for values in symbols.values()) == previous:
            break
    for key, values in resolved.items():
        if len(values) == 1:
            program.imports[key] = next(iter(values))
        elif values:
            program.notices.append({"kind": "ambiguous_rust_import", "path": key[0], "name": key[1]})
