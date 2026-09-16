"""Resolve selected Rust modules and explicitly bound dependency crate roots."""

from collections import defaultdict
from pathlib import PurePosixPath


def resolve_modules(program, crate_bindings=None):
    from syntax import child, rust_use_paths

    locations = defaultdict(list)
    rust_sources = [source for source in program.sources if source.language == 'rust']
    if not rust_sources:
        return
    for source in rust_sources:
        path = PurePosixPath(source.path)
        parts = path.parts
        index = max((i for i, part in enumerate(parts) if part == 'src'), default=-1)
        root = str(PurePosixPath(*parts[:index + 1])) if index >= 0 else '.'
        relative = parts[index + 1:] if index >= 0 else parts
        module = relative[:-1] + (() if path.stem in {'lib', 'main', 'mod'} else (path.stem,))
        locations[(root, module)].append(source)
    available = {key: values[0] for key, values in locations.items() if len(values) == 1}
    reachable = {key: source for key, source in available.items()
                 if not key[1] and PurePosixPath(source.path).stem in {'lib', 'main'}}
    bodies = {key: source.tree.root_node for key, source in reachable.items()}
    for _ in range(16):
        added = {}
        for (root, module), source in reachable.items():
            for node in bodies[(root, module)].named_children:
                if node.type != 'mod_item':
                    continue
                key = (root, module + (source.text(child(node, 'name')),))
                if key in reachable:
                    continue
                body = child(node, 'body')
                if body is not None:
                    added[key] = source
                    bodies[key] = body
                elif key in available:
                    added[key] = available[key]
                    bodies[key] = available[key].tree.root_node
        if not added:
            break
        reachable.update(added)
    # Standalone examples can import explicitly selected dependency crates;
    # their file layout does not establish a fictional `crate::` module tree.
    included_paths = {source.path for source in reachable.values()}
    for source in rust_sources:
        if source.path not in included_paths:
            key = ('@file:' + source.path, ())
            reachable[key] = source
            bodies[key] = source.tree.root_node
    crate_roots = {}
    for name, path in (crate_bindings or {}).items():
        roots = [key for key, source in reachable.items() if not key[1] and source.path == path and not key[0].startswith('@file:')]
        if len(roots) == 1:
            crate_roots[name] = roots[0]
        else:
            program.notices.append({'kind': 'rust_crate_binding_unavailable', 'name': name, 'path': path})
    symbols, imports = defaultdict(set), []
    module_values = {key: '@module:' + key[0] + '|' + '::'.join(key[1]) for key in reachable}
    modules_by_value = {value: key for key, value in module_values.items()}
    for key, source in reachable.items():
        for node in bodies[key].named_children:
            if node.type in {'struct_item', 'enum_item', 'trait_item', 'type_item', 'function_item', 'const_item', 'static_item'}:
                name = source.text(child(node, 'name'))
                symbols[(key, name)].add(source.path + '#' + name)
            if node.type == 'mod_item':
                name = source.text(child(node, 'name'))
                target = (key[0], key[1] + (name,))
                if target in reachable:
                    symbols[(key, name)].add(module_values[target])
            if node.type == 'use_declaration':
                is_public = any(part.type == 'visibility_modifier' for part in node.named_children)
                imports.extend((key, source, alias, path, is_public)
                               for alias, path in rust_use_paths(source, child(node, 'argument')))

    def resolve_path(key, path):
        root, current = key
        parts = path.split('::')
        if parts[0] in crate_roots:
            starts = [(crate_roots[parts.pop(0)], parts)]
        elif parts[0] == 'crate':
            starts = [((root, ()), parts[1:])]
        elif parts[0] == 'self':
            starts = [((root, current), parts[1:])]
        elif parts[0] == 'super':
            while parts and parts[0] == 'super':
                if not current:
                    return set()
                current, parts = current[:-1], parts[1:]
            starts = [((root, current), parts)]
        else:
            starts = [((root, current), parts), ((root, ()), parts)]
        found = set()
        for start, remaining in starts:
            values = {module_values[start]} if start in module_values else set()
            for part in remaining:
                following = set()
                for value in values:
                    module = modules_by_value.get(value)
                    if module is not None:
                        following.update(symbols.get((module, part), ()))
                values = following
            found.update(values)
        return found

    resolved = defaultdict(set)
    for _ in range(12):
        previous = sum(len(values) for values in symbols.values())
        for key, source, alias, path, is_public in imports:
            if alias == '*':
                modules = {modules_by_value[value] for value in resolve_path(key, path.removesuffix('::*')) if value in modules_by_value}
                for (module, name), values in list(symbols.items()):
                    if module in modules:
                        resolved[(source.path, name)].update(values)
                        if is_public:
                            symbols[(key, name)].update(values)
                continue
            values = resolve_path(key, path)
            resolved[(source.path, alias)].update(values)
            if is_public:
                symbols[(key, alias)].update(values)
        if sum(len(values) for values in symbols.values()) == previous:
            break
    for key, values in resolved.items():
        if len(values) == 1:
            value = next(iter(values))
            if value in modules_by_value:
                value = reachable[modules_by_value[value]].path
            program.imports[key] = value
        elif values:
            program.notices.append({'kind': 'ambiguous_rust_import', 'path': key[0], 'name': key[1]})
