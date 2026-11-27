use super::{
    node_at, node_range,
    rust_cfg::{self, Condition},
    scope, text, Source, SourceResolver,
};
use crate::parser::{CodeRange, ExtractedFile, ExtractedSymbol, ImportKind};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use tree_sitter::Node;

pub(super) type ScopeKey = (usize, usize);

#[inline]
pub(super) fn scope_key(node: Node<'_>) -> ScopeKey {
    (node.start_byte(), node.end_byte())
}

#[derive(Default)]
pub(super) struct RustScopeEntry {
    pub(super) unknown_bindings: HashSet<String>,
    pub(super) mod_items: HashMap<String, Vec<CodeRange>>,
    pub(super) named_imports: HashMap<String, Vec<(usize, Condition)>>,
    pub(super) glob_imports: Vec<(usize, Condition)>,
    pub(super) type_names_in_scope: HashSet<String>,
    pub(super) local_defs: HashMap<String, Vec<(&'static str, usize)>>,
}

pub(super) struct RustFileIndex {
    pub(super) scopes: HashMap<ScopeKey, RustScopeEntry>,
}

fn rust_symbol_matches_node(node: Node<'_>, source: &Source, symbol: &ExtractedSymbol) -> bool {
    let mut node = node;
    if matches!(
        node.kind(),
        "type_declaration" | "var_declaration" | "const_declaration"
    ) {
        let mut cursor = node.walk();
        let declarations: Vec<_> = node
            .named_children(&mut cursor)
            .filter(|child| child.child_by_field_name("name").is_some())
            .collect();
        if declarations.len() != 1 {
            return false;
        }
        node = declarations[0];
    }
    while node.child_by_field_name("name").is_none() {
        let Some(parent) = node
            .parent()
            .filter(|parent| parent.byte_range() == node.byte_range())
        else {
            return false;
        };
        node = parent;
    }
    if node
        .child_by_field_name("name")
        .and_then(|name| text(name, source))
        != Some(&symbol.name)
    {
        return false;
    }
    match symbol.kind.as_str() {
        "fn" => matches!(
            node.kind(),
            "function_item"
                | "function_declaration"
                | "method_declaration"
                | "method_elem"
                | "macro_definition"
        ),
        "const" | "static" => {
            matches!(node.kind(), "const_item" | "static_item" | "const_spec")
        }
        _ => matches!(
            node.kind(),
            "struct_item"
                | "enum_item"
                | "type_item"
                | "trait_item"
                | "union_item"
                | "type_spec"
                | "type_alias"
        ),
    }
}

impl SourceResolver<'_> {
    pub(super) fn rust_file_index(
        &self,
        file: &ExtractedFile,
        source: &Source,
    ) -> Rc<RustFileIndex> {
        if let Some(cached) = self.rust_file_indexes.borrow().get(&file.file_path) {
            return Rc::clone(cached);
        }
        let file_cond = self.rust_file_condition(&file.file_path, 0);
        let target_os = self.target_os.as_deref();
        let mut scopes: HashMap<ScopeKey, RustScopeEntry> = HashMap::new();

        // Single pass over the syntax tree to index scope-local unknown bindings and `mod_item`s.
        let mut stack = vec![source.syntax.tree.root_node()];
        while let Some(node) = stack.pop() {
            let is_scope = matches!(node.kind(), "source_file" | "block" | "declaration_list");
            let mut cursor = node.walk();
            if is_scope {
                let entry = scopes.entry(scope_key(node)).or_default();
                for child in node.named_children(&mut cursor) {
                    if let Some(name) = child
                        .child_by_field_name("name")
                        .and_then(|n| text(n, source))
                    {
                        let cond =
                            file_cond.and(rust_cfg::condition(child, &source.text, target_os));
                        if cond == Condition::Unknown {
                            entry.unknown_bindings.insert(name.to_string());
                        }
                        if child.kind() == "mod_item" && cond != Condition::False {
                            entry
                                .mod_items
                                .entry(name.to_string())
                                .or_default()
                                .push(node_range(child));
                        }
                    }
                    stack.push(child);
                }
            } else {
                for child in node.named_children(&mut cursor) {
                    stack.push(child);
                }
            }
        }

        // Single pass over imports to index named and glob imports by enclosing lexical scope.
        if let Some(navigation) = source.extracted.navigation.as_ref() {
            for (idx, import) in navigation.imports.iter().enumerate() {
                let Some(imp_node) = node_at(source, &import.range) else {
                    continue;
                };
                let mut use_decl = imp_node;
                let mut found_use = false;
                loop {
                    if use_decl.kind() == "use_declaration" {
                        found_use = true;
                        break;
                    }
                    let Some(parent) = use_decl.parent() else {
                        break;
                    };
                    use_decl = parent;
                }
                if !found_use
                    || rust_cfg::condition(use_decl, &source.text, target_os) == Condition::False
                {
                    continue;
                }
                let imp_cond = rust_cfg::condition(imp_node, &source.text, target_os);
                let entry = scopes.entry(scope_key(scope(use_decl))).or_default();
                if import.kind == ImportKind::Glob {
                    entry.glob_imports.push((idx, imp_cond));
                }
                entry
                    .named_imports
                    .entry(import.local_name.clone())
                    .or_default()
                    .push((idx, imp_cond));
            }
        }

        // Single pass over file-local symbols so Rust lookups never scan all workspace candidates.
        for (sym_idx, symbol) in file.symbols.iter().enumerate() {
            let Some(node) = node_at(source, &symbol.range) else {
                continue;
            };
            let sc = scope_key(scope(node));
            let is_type_kind = matches!(
                symbol.kind.as_str(),
                "struct" | "enum" | "type" | "trait" | "interface" | "union"
            );
            if is_type_kind {
                scopes
                    .entry(sc)
                    .or_default()
                    .type_names_in_scope
                    .insert(symbol.name.clone());
            }
            let bucket: Option<&'static str> = if is_type_kind {
                Some("type")
            } else if matches!(symbol.kind.as_str(), "const" | "static") {
                Some("const")
            } else if symbol.kind == "fn" && symbol.owner.is_none() {
                if node.kind() == "macro_definition" {
                    Some("macro")
                } else {
                    Some("call")
                }
            } else {
                None
            };
            let Some(bucket) = bucket else {
                continue;
            };
            let cond = file_cond.and(rust_cfg::condition(node, &source.text, target_os));
            if cond == Condition::True && rust_symbol_matches_node(node, source, symbol) {
                scopes
                    .entry(sc)
                    .or_default()
                    .local_defs
                    .entry(symbol.name.clone())
                    .or_default()
                    .push((bucket, sym_idx));
            }
        }

        let built = Rc::new(RustFileIndex { scopes });
        self.rust_file_indexes
            .borrow_mut()
            .insert(file.file_path.clone(), Rc::clone(&built));
        built
    }
    pub(super) fn rust_node_condition(
        &self,
        file: &str,
        source: &Source,
        node: Node<'_>,
    ) -> Condition {
        if !file.ends_with(".rs") {
            return Condition::True;
        }
        self.rust_file_condition(file, 0).and(rust_cfg::condition(
            node,
            &source.text,
            self.target_os.as_deref(),
        ))
    }

    fn rust_file_condition(&self, file: &str, depth: usize) -> Condition {
        if let Some(state) = self.file_conditions.borrow().get(file) {
            return *state;
        }
        if depth >= 32 {
            return Condition::Unknown;
        }
        let current = self.root.join(file);
        let Some(root) = self.rust_root(file) else {
            return Condition::Unknown;
        };
        let Some(source) = self.source(file) else {
            return Condition::Unknown;
        };
        let own = rust_cfg::condition(
            source.syntax.tree.root_node(),
            &source.text,
            self.target_os.as_deref(),
        );
        let state = if current == root {
            own
        } else {
            let mut directory = if current.file_name().and_then(|s| s.to_str()) == Some("mod.rs") {
                current.parent().and_then(Path::parent)
            } else {
                current.parent()
            };
            let mut parent_state = None;
            while let Some(dir) = directory.filter(|dir| dir.starts_with(self.root)) {
                let parent = self.module_file(dir).filter(|p| *p != current).or_else(|| {
                    (root.parent() == Some(dir) && root != current).then(|| root.clone())
                });
                if let Some(parent) = parent.and_then(|path| self.file(&path)) {
                    if let Some(parent_source) = self.source(&parent.file_path) {
                        let mut stack = vec![parent_source.syntax.tree.root_node()];
                        let mut matches = Vec::new();
                        while let Some(node) = stack.pop() {
                            let mut cursor = node.walk();
                            for module in node
                                .named_children(&mut cursor)
                                .filter(|n| n.kind() == "mod_item")
                            {
                                if let Some(body) = module.child_by_field_name("body") {
                                    stack.push(body);
                                } else if self
                                    .module_destination(parent, &parent_source, module)
                                    .as_ref()
                                    == Some(&current)
                                {
                                    self.record_dependency(
                                        &parent.file_path,
                                        &super::node_range(module),
                                    );
                                    matches.push(rust_cfg::condition(
                                        module,
                                        &parent_source.text,
                                        self.target_os.as_deref(),
                                    ));
                                }
                            }
                        }
                        if !matches.is_empty() {
                            let active: Vec<_> = matches
                                .into_iter()
                                .filter(|s| *s != Condition::False)
                                .collect();
                            parent_state = Some(if active.is_empty() {
                                Condition::False
                            } else if active.len() == 1 {
                                active[0]
                                    .and(self.rust_file_condition(&parent.file_path, depth + 1))
                            } else {
                                Condition::Unknown
                            });
                            break;
                        }
                    }
                }
                if root.parent() == Some(dir) {
                    break;
                }
                directory = dir.parent();
            }
            own.and(parent_state.unwrap_or(Condition::Unknown))
        };
        self.file_conditions
            .borrow_mut()
            .insert(file.to_string(), state);
        state
    }
}
