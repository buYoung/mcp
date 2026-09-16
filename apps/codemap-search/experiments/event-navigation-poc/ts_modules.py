"""Bounded ESM imports and re-exports over explicitly selected source modules."""

from collections import defaultdict
from pathlib import Path


def resolve_modules(program, root, module_bindings):
    from syntax import child, walk

    sources = {source.path: source for source in program.sources if source.language in {"typescript", "tsx"}}

    def module(source, specifier):
        mapped = module_bindings.get(specifier)
        if mapped is not None:
            target = (root / mapped).resolve()
        elif specifier.startswith("."):
            target = (root / Path(source.path).parent / specifier).resolve()
        else:
            return None
        candidates = [target, Path(str(target) + ".ts"), Path(str(target) + ".tsx"), target / "index.ts"]
        if target.suffix in {".js", ".jsx"}:
            candidates.extend([target.with_suffix(".ts"), target.with_suffix(".tsx")])
        available = {path.relative_to(root).as_posix() for path in candidates
                     if path.is_relative_to(root) and path.relative_to(root).as_posix() in sources}
        return next(iter(available)) if len(available) == 1 else None

    definitions = {}
    exports = defaultdict(set)
    imports = []
    forwards = []
    for source in sources.values():
        for node in source.tree.root_node.named_children:
            declaration = child(node, "declaration") if node.type == "export_statement" else node
            names = []
            if declaration is not None:
                if declaration.type in {"function_declaration", "class_declaration", "interface_declaration", "type_alias_declaration"}:
                    names = [source.text(child(declaration, "name"))]
                elif declaration.type in {"lexical_declaration", "variable_declaration"}:
                    names = [source.text(child(part, "name")) for part in declaration.named_children
                             if part.type == "variable_declarator" and child(part, "name").type == "identifier"]
            for name in names:
                definitions[(source.path, name)] = source.path + "#" + name
                if node.type == "export_statement":
                    public = "default" if any(part.type == "default" for part in node.children) else name
                    exports[(source.path, public)].add(source.path + "#" + name)
            if node.type not in {"export_statement", "import_statement"}:
                continue
            specifier = source.text(child(node, "source")).strip("'\"")
            target = module(source, specifier) if specifier else source.path
            if target is None:
                continue
            if node.type == "import_statement":
                for part in walk(node):
                    if part.type == "import_specifier":
                        name = source.text(child(part, "name"))
                        alias = source.text(child(part, "alias")) or name
                        imports.append((source.path, alias, target, name))
                    elif part.type == "namespace_import" and part.named_children:
                        program.imports[(source.path, source.text(part.named_children[-1]))] = target
                    elif part.type == "import_clause":
                        for value in part.named_children:
                            if value.type == "identifier":
                                imports.append((source.path, source.text(value), target, "default"))
            else:
                specific = False
                for part in walk(node):
                    if part.type == "export_specifier":
                        name = source.text(child(part, "name"))
                        alias = source.text(child(part, "alias")) or name
                        forwards.append((source.path, alias, target, name, bool(specifier)))
                        specific = True
                    elif part.type == "namespace_export" and part.named_children:
                        exports[(source.path, source.text(part.named_children[-1]))].add(target)
                        specific = True
                if specifier and not specific and any(part.type == "*" for part in node.children):
                    forwards.append((source.path, "*", target, "*", True))
    for _ in range(12):
        before = sum(len(values) for values in exports.values())
        for path, alias, target, name in imports:
            candidates = exports.get((target, name), ())
            if len(candidates) == 1:
                program.imports[(path, alias)] = next(iter(candidates))
        for path, alias, target, name, is_remote in forwards:
            if name == "*":
                for (owner, member), values in list(exports.items()):
                    if owner == target and member != "default":
                        exports[(path, member)].update(values)
            elif is_remote:
                exports[(path, alias)].update(exports.get((target, name), ()))
            else:
                value = definitions.get((path, name)) or program.imports.get((path, name))
                if value:
                    exports[(path, alias)].add(value)
        if sum(len(values) for values in exports.values()) == before:
            break
    program.exports = {key: next(iter(values)) for key, values in exports.items() if len(values) == 1}
    for path, alias, target, name in imports:
        if (target, name) in program.exports:
            program.imports[(path, alias)] = program.exports[(target, name)]
        else:
            program.imports.pop((path, alias), None)
