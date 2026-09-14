use super::*;
use crate::parser::{ImportEntry, ImportKind};

pub(super) fn scope(mut node: Node<'_>) -> Node<'_> {
    while let Some(parent) = node.parent() {
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
            "java" | "kotlin" | "scala" | "groovy" | "csharp"
        ) && matches!(
            node.kind(),
            "import_declaration" | "import_header" | "using_directive"
        ) {
            if modifier(node, "static", source) {
                continue;
            }
            let value = text(node, source)
                .trim()
                .trim_start_matches("import")
                .trim_start_matches("using")
                .trim()
                .trim_end_matches(';')
                .trim();
            if value.contains(['*', '{', '}', '=']) {
                continue;
            }
            let (original, alias) = value
                .split_once(" as ")
                .map_or((value, None), |(a, b)| (a, Some(b)));
            let original = compact(original);
            if !original
                .chars()
                .all(|c| c.is_alphanumeric() || "_.$".contains(c))
            {
                continue;
            }
            let local = alias
                .map(str::to_string)
                .unwrap_or_else(|| original.rsplit('.').next().unwrap_or("").into());
            unit.imports.push(ScopedImport {
                entry: ImportEntry {
                    local_name: local,
                    imported_name: Some(original.clone()),
                    source: Some(original),
                    kind: ImportKind::Named,
                    range: range(node),
                },
                scope: range(root),
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
    unit.imports.dedup_by(|a, b| a == b);
}
