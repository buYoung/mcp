use super::{
    rust_cfg::{self, Condition},
    text, Source, SourceResolver,
};
use std::path::Path;
use tree_sitter::Node;

impl SourceResolver<'_> {
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

    pub(super) fn rust_import_condition(
        &self,
        source: &Source,
        range: &crate::parser::CodeRange,
    ) -> Condition {
        super::node_at(source, range).map_or(Condition::Unknown, |node| {
            rust_cfg::condition(node, &source.text, self.target_os.as_deref())
        })
    }

    pub(super) fn has_unknown_rust_binding(
        &self,
        file: &str,
        source: &Source,
        lexical: Node<'_>,
        name: &str,
    ) -> bool {
        let mut cursor = lexical.walk();
        let has_unknown_binding = lexical.named_children(&mut cursor).any(|node| {
            node.child_by_field_name("name")
                .and_then(|name| text(name, source))
                == Some(name)
                && self.rust_node_condition(file, source, node) == Condition::Unknown
        });
        has_unknown_binding
    }
}
