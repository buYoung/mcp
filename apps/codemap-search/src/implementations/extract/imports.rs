use super::*;
use crate::parser::{ImportEntry, ImportKind};

pub(super) fn scope(mut node: Node<'_>) -> Node<'_> {
    for parent in crate::parser::ancestors(node) {
        node = parent;
        if matches!(
            node.kind(),
            "program"
                | "module"
                | "source_file"
                | "compilation_unit"
                | "translation_unit"
                | "block"
                | "statement_block"
                | "class_body"
                | "declaration_list"
                | "template_body"
                | "script_block"
        ) {
            return node;
        }
    }
    node
}

fn name_node(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("name").or_else(|| {
        child(
            node,
            &[
                "dotted_name",
                "qualified_name",
                "qualified_identifier",
                "scoped_identifier",
                "identifier",
                "namespace_name",
                "package_identifier",
            ],
        )
    })
}

pub(super) fn collect(
    root: Node<'_>,
    source: &[u8],
    unit: &mut ImplementationUnit,
    navigation: &NavigationFile,
) {
    for entry in &navigation.imports {
        let node = root.named_descendant_for_point_range(
            Point::new(entry.range.start_line - 1, entry.range.start_col - 1),
            Point::new(entry.range.end_line - 1, entry.range.end_col - 1),
        );
        // Java `import static` imports static members, not types.
        if unit.language == "java" && node.is_some_and(|node| modifier(node, "static", source)) {
            continue;
        }
        // `lang::scala` strips `{...}` selectors into a single Namespace entry; let the Scala
        // parser below expand each selector without treating `{X}` as a wildcard import.
        if unit.language == "scala" && entry.kind == ImportKind::Namespace {
            if let Some(node) = node {
                let raw = text(node, source);
                if raw.contains('{')
                    || (!raw.trim_end().ends_with("._") && !raw.trim_end().ends_with(".*"))
                {
                    continue;
                }
            }
        }
        unit.imports.push(ScopedImport {
            entry: entry.clone(),
            scope: range(node.map(scope).unwrap_or(root)),
        });
    }
    for node in children(root) {
        if matches!(
            node.kind(),
            "package_declaration" | "package_clause" | "package_header"
        ) {
            if let Some(name) = name_node(node) {
                unit.namespace = compact(text(name, source));
            }
        }
        if node.kind() == "file_scoped_namespace_declaration"
            || node.kind() == "namespace_definition" && node.child_by_field_name("body").is_none()
        {
            if let Some(name) = name_node(node) {
                unit.namespace = compact(text(name, source)).replace('\\', ".");
            }
        }
        if matches!(
            unit.language.as_str(),
            "java" | "kotlin" | "scala" | "groovy"
        ) && matches!(
            node.kind(),
            "import_declaration" | "import_header" | "import"
        ) {
            if modifier(node, "static", source) {
                continue;
            }
            let value = text(node, source)
                .trim()
                .trim_start_matches("import")
                .trim()
                .trim_end_matches(';')
                .trim();
            let import_scope = range(scope(node));
            let is_valid_dotted = |path: &str| {
                !path.is_empty()
                    && path
                        .chars()
                        .all(|c| c.is_alphanumeric() || "_.$".contains(c))
            };
            // Scala brace imports: `import a.b.{X, Y => Z, _}`.
            if unit.language == "scala" && value.contains('{') && value.ends_with('}') {
                if let Some((prefix, body)) = value.split_once('{') {
                    let prefix = compact(prefix.trim().trim_end_matches('.'));
                    if is_valid_dotted(&prefix) {
                        let inner = body.trim_end_matches('}');
                        for selector in inner.split(',') {
                            let selector = selector.trim();
                            if selector.is_empty() {
                                continue;
                            }
                            if matches!(selector, "_" | "*") {
                                unit.imports.push(ScopedImport {
                                    entry: ImportEntry {
                                        local_name: "*".into(),
                                        imported_name: None,
                                        source: Some(prefix.clone()),
                                        kind: ImportKind::Namespace,
                                        range: range(node),
                                    },
                                    scope: import_scope.clone(),
                                });
                                continue;
                            }
                            let (orig, alias) = selector
                                .split_once("=>")
                                .or_else(|| selector.split_once(" as "))
                                .map_or((selector, None), |(a, b)| (a.trim(), Some(b.trim())));
                            if alias == Some("_") || !is_valid_dotted(orig) {
                                continue;
                            }
                            let local = alias
                                .map(str::to_string)
                                .unwrap_or_else(|| orig.rsplit('.').next().unwrap_or("").into());
                            if local.is_empty()
                                || !local
                                    .chars()
                                    .all(|c| c.is_alphanumeric() || "_$".contains(c))
                            {
                                continue;
                            }
                            let full = format!("{prefix}.{orig}");
                            unit.imports.push(ScopedImport {
                                entry: ImportEntry {
                                    local_name: local,
                                    imported_name: Some(full.clone()),
                                    source: Some(full),
                                    kind: ImportKind::Named,
                                    range: range(node),
                                },
                                scope: import_scope.clone(),
                            });
                        }
                    }
                }
                continue;
            }
            // Wildcard package imports: `import a.b.*` (Java/Kotlin/Groovy/Scala) and `import a.b._` (Scala).
            if let Some(pkg) = value.strip_suffix(".*").or_else(|| {
                (unit.language == "scala")
                    .then(|| value.strip_suffix("._"))
                    .flatten()
            }) {
                let pkg = compact(pkg);
                if is_valid_dotted(&pkg) {
                    unit.imports.push(ScopedImport {
                        entry: ImportEntry {
                            local_name: "*".into(),
                            imported_name: None,
                            source: Some(pkg),
                            kind: ImportKind::Namespace,
                            range: range(node),
                        },
                        scope: import_scope,
                    });
                }
                continue;
            }
            if value.contains(['*', '{', '}', '=']) {
                continue;
            }
            let (original, alias) = value
                .split_once(" as ")
                .map_or((value, None), |(a, b)| (a, Some(b)));
            let original = compact(original);
            if !is_valid_dotted(&original) {
                continue;
            }
            let local = alias
                .map(compact)
                .unwrap_or_else(|| original.rsplit('.').next().unwrap_or("").into());
            unit.imports.push(ScopedImport {
                entry: ImportEntry {
                    local_name: local,
                    imported_name: Some(original.clone()),
                    source: Some(original),
                    kind: ImportKind::Named,
                    range: range(node),
                },
                scope: import_scope,
            });
        }
        if unit.language == "python" && node.kind() == "import_from_statement" {
            let Some(module) = node.child_by_field_name("module_name") else {
                continue;
            };
            for imported in children(node)
                .into_iter()
                .filter(|child| child.id() != module.id())
            {
                if !matches!(imported.kind(), "dotted_name" | "aliased_import") {
                    continue;
                }
                let original = imported.child_by_field_name("name").unwrap_or(imported);
                let alias = imported.child_by_field_name("alias").unwrap_or(original);
                unit.imports.push(ScopedImport {
                    entry: ImportEntry {
                        local_name: compact(text(alias, source)),
                        imported_name: Some(compact(text(original, source))),
                        source: Some(compact(text(module, source))),
                        kind: ImportKind::Named,
                        range: range(node),
                    },
                    scope: range(root),
                });
            }
        }
    }
    if unit.language == "cpp" {
        collect_cpp_using_directives(root, source, unit);
    }
    if unit.language == "dart" {
        for import in dart_library_imports(root, source) {
            let entries = if import.shown.is_empty() {
                vec![("*".to_string(), None, ImportKind::Glob)]
            } else {
                import
                    .shown
                    .into_iter()
                    .map(|name| (name.clone(), Some(name), ImportKind::Named))
                    .collect()
            };
            for (local_name, imported_name, kind) in entries {
                unit.imports.push(ScopedImport {
                    entry: ImportEntry {
                        local_name,
                        imported_name,
                        source: Some(import.uri.clone()),
                        kind,
                        range: import.range.clone(),
                    },
                    scope: range(root),
                });
            }
        }
    }
    unit.imports.dedup_by(|a, b| a == b);
}

/// Collects C++ `using namespace ns;` directives and `using ns::Type;` declarations across the
/// translation unit and nested namespace blocks.
fn collect_cpp_using_directives(root: Node<'_>, source: &[u8], unit: &mut ImplementationUnit) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "using_directive" | "using_declaration" => {
                let raw = text(node, source).trim();
                let after_using = raw
                    .strip_prefix("using")
                    .map(str::trim)
                    .unwrap_or("")
                    .trim_end_matches(';')
                    .trim();
                if let Some(after_ns) = after_using.strip_prefix("namespace") {
                    if after_ns.starts_with(char::is_whitespace) {
                        // `using namespace foo::bar;`
                        let ns = compact(after_ns.trim())
                            .trim_start_matches("::")
                            .replace("::", ".");
                        if !ns.is_empty()
                            && ns.chars().all(|c| c.is_alphanumeric() || "_.".contains(c))
                        {
                            unit.using_namespaces.push(ScopedName {
                                name: ns,
                                scope: range(scope(node)),
                                range: range(node),
                            });
                        }
                        continue;
                    }
                }
                // `using foo::Bar;` (not inside a class body, where `using Base::f;` inherits methods)
                if node.parent().is_some_and(|p| {
                    matches!(
                        p.kind(),
                        "field_declaration_list" | "class_specifier" | "struct_specifier"
                    )
                }) {
                    continue;
                }
                let raw_decl = after_using.trim_start_matches("typename").trim();
                let qualified = compact(raw_decl)
                    .trim_start_matches("::")
                    .replace("::", ".");
                if let Some((_prefix, local)) = qualified.rsplit_once('.') {
                    if !local.is_empty()
                        && qualified
                            .chars()
                            .all(|c| c.is_alphanumeric() || "_.".contains(c))
                    {
                        unit.imports.push(ScopedImport {
                            entry: ImportEntry {
                                local_name: local.to_string(),
                                imported_name: Some(qualified.clone()),
                                source: Some(qualified),
                                kind: ImportKind::Named,
                                range: range(node),
                            },
                            scope: range(scope(node)),
                        });
                    }
                }
            }
            "translation_unit"
            | "namespace_definition"
            | "declaration_list"
            | "preproc_ifdef"
            | "preproc_if"
            | "preproc_else"
            | "preproc_elif" => {
                for child in children(node) {
                    stack.push(child);
                }
            }
            _ => {}
        }
    }
}

/// A Dart library directive without a prefix: the library URI and the names its `show` and
/// `hide` lists name. An empty `shown` list exposes every public name not hidden.
pub(crate) struct DartLibraryDirective {
    pub(crate) uri: String,
    pub(crate) shown: Vec<String>,
    pub(crate) hidden: Vec<String>,
    pub(crate) range: CodeRange,
}

/// The library imports of a Dart file whose names are reachable unqualified. Prefixed imports
/// and imports with a `hide` list are left out.
pub(crate) fn dart_library_imports(root: Node<'_>, source: &[u8]) -> Vec<DartLibraryDirective> {
    dart_directives(root, source, "library_import")
        .into_iter()
        .filter(|import| import.hidden.is_empty())
        .collect()
}

/// The `export` directives of a Dart library.
pub(crate) fn dart_library_exports(root: Node<'_>, source: &[u8]) -> Vec<DartLibraryDirective> {
    dart_directives(root, source, "library_export")
}

fn dart_directives(root: Node<'_>, source: &[u8], kind: &str) -> Vec<DartLibraryDirective> {
    let mut directives = Vec::new();
    for node in children(root)
        .into_iter()
        .filter(|node| node.kind() == "import_or_export")
    {
        // An import keeps its URI and combinators in a specification; an export holds them.
        for specification in children(node)
            .into_iter()
            .filter(|node| node.kind() == kind)
            .flat_map(|directive| match kind {
                "library_import" => children(directive)
                    .into_iter()
                    .filter(|node| node.kind() == "import_specification")
                    .collect(),
                _ => vec![directive],
            })
        {
            let Some(uri) = specification
                .child_by_field_name("uri")
                .filter(|_| specification.child_by_field_name("alias").is_none())
            else {
                continue;
            };
            let mut shown = Vec::new();
            let mut hidden = Vec::new();
            for combinator in children(specification)
                .into_iter()
                .filter(|node| node.kind() == "combinator")
            {
                let names = if text(combinator, source).trim_start().starts_with("hide") {
                    &mut hidden
                } else {
                    &mut shown
                };
                names.extend(
                    children(combinator)
                        .into_iter()
                        .filter(|node| node.kind() == "identifier")
                        .map(|node| text(node, source).to_string()),
                );
            }
            directives.push(DartLibraryDirective {
                uri: text(uri, source)
                    .trim()
                    .trim_matches(['\'', '"'])
                    .to_string(),
                shown,
                hidden,
                range: range(node),
            });
        }
    }
    directives
}
