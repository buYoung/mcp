//! Rust standard-path and selected-module resolution. An unresolved glob is a
//! missing-name boundary, never proof that the prelude is unshadowed.
use super::engine::Interpreter;
use super::syntax::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct RustScope {
    pub start: usize,
    pub end: usize,
    pub declared: BTreeSet<String>,
    pub visible: BTreeSet<String>,
    pub imports: BTreeMap<String, String>,
    pub has_unknown_glob: bool,
}

pub(crate) fn use_paths(
    source: &Source,
    id: Option<NodeId>,
    prefix: &str,
) -> Vec<(String, String)> {
    let Some(id) = id else {
        return Vec::new();
    };
    let node = &source.nodes[id];
    if node.kind == "scoped_use_list" {
        let path = source.text(source.child(id, &["path"]));
        let prefix = if prefix.is_empty() {
            path.into()
        } else {
            format!("{prefix}::{path}")
        };
        return use_paths(source, source.child(id, &["list"]), &prefix);
    }
    if node.kind == "use_list" {
        return node
            .children
            .iter()
            .flat_map(|part| use_paths(source, Some(*part), prefix))
            .collect();
    }
    if node.kind == "use_as_clause" {
        let path = source.text(source.child(id, &["path"]));
        return vec![(
            source.text(source.child(id, &["alias"])).into(),
            if prefix.is_empty() {
                path.into()
            } else {
                format!("{prefix}::{path}")
            },
        )];
    }
    if matches!(
        node.kind.as_str(),
        "identifier" | "scoped_identifier" | "self" | "super" | "crate" | "use_wildcard"
    ) {
        let text = source.text(Some(id));
        let path = if prefix.is_empty() {
            text.to_owned()
        } else {
            format!("{prefix}::{text}")
        };
        let path = path.trim_start_matches(':').to_owned();
        return vec![(path.rsplit("::").next().unwrap_or_default().into(), path)];
    }
    Vec::new()
}

impl Program {
    pub fn resolve_rust_names(&mut self) {
        let mut modules: BTreeMap<(String, String), (usize, NodeId)> = BTreeMap::new();
        for (index, source) in self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| s.language == "rust")
        {
            let path = std::path::Path::new(&source.path);
            let (root, relative) = if let Some((root, relative)) = source.path.rsplit_once("src/") {
                (format!("{root}src"), relative.to_owned())
            } else {
                (".".to_owned(), source.path.clone())
            };
            let stem = path
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or_default();
            let module = if relative.is_empty() {
                String::new()
            } else {
                let relative = std::path::Path::new(&relative);
                let dir = relative
                    .parent()
                    .map(|p| p.to_string_lossy().replace('/', "::"))
                    .unwrap_or_default();
                if matches!(stem, "lib" | "main" | "mod") {
                    dir
                } else if dir.is_empty() {
                    stem.into()
                } else {
                    format!("{dir}::{stem}")
                }
            };
            modules.insert((root.clone(), module.clone()), (index, 0));
            let mut pending = vec![(0, module)];
            while let Some((body, prefix)) = pending.pop() {
                for &node in &source.nodes[body].children {
                    if source.nodes[node].kind != "mod_item" {
                        continue;
                    }
                    let name = source.text(source.child(node, &["name"]));
                    let full = if prefix.is_empty() {
                        name.into()
                    } else {
                        format!("{prefix}::{name}")
                    };
                    if let Some(body) = source.child(node, &["body"]) {
                        modules.insert((root.clone(), full.clone()), (index, body));
                        pending.push((body, full));
                    }
                }
            }
        }
        // A filesystem layout alone cannot prove that lib.rs declares a file.
        // Walk declared modules from roots, then treat unselected files as their
        // own standalone sources.
        let mut reachable = BTreeSet::new();
        for (key, (index, _)) in &modules {
            let stem = std::path::Path::new(&self.sources[*index].path)
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or_default();
            if key.0.starts_with("@file:") || (key.1.is_empty() && matches!(stem, "lib" | "main")) {
                reachable.insert(key.clone());
            }
        }
        for _ in 0..16 {
            let before = reachable.len();
            for key in reachable.clone() {
                let (index, body) = modules[&key];
                let source = &self.sources[index];
                for &id in &source.nodes[body].children {
                    if source.nodes[id].kind == "mod_item" {
                        let name = source.text(source.child(id, &["name"]));
                        let module = if key.1.is_empty() {
                            name.into()
                        } else {
                            format!("{}::{name}", key.1)
                        };
                        if modules.contains_key(&(key.0.clone(), module.clone())) {
                            reachable.insert((key.0.clone(), module));
                        }
                    }
                }
            }
            if reachable.len() == before {
                break;
            }
        }
        let covered: BTreeSet<_> = reachable.iter().map(|k| modules[k].0).collect();
        for (index, source) in self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| s.language == "rust")
        {
            if !covered.contains(&index) {
                let key = (format!("@file:{}", source.path), String::new());
                modules.insert(key.clone(), (index, 0));
                reachable.insert(key);
            }
        }
        for _ in 0..16 {
            let mut added = Vec::new();
            for key in reachable.iter().filter(|k| k.0.starts_with("@file:")) {
                let (index, body) = modules[key];
                let source = &self.sources[index];
                for &id in &source.nodes[body].children {
                    if source.nodes[id].kind != "mod_item" {
                        continue;
                    }
                    if let Some(body) = source.child(id, &["body"]) {
                        let name = source.text(source.child(id, &["name"]));
                        let nested = (
                            key.0.clone(),
                            if key.1.is_empty() {
                                name.to_owned()
                            } else {
                                format!("{}::{name}", key.1)
                            },
                        );
                        if !reachable.contains(&nested) {
                            added.push((nested, (index, body)));
                        }
                    }
                }
            }
            if added.is_empty() {
                break;
            }
            for (key, value) in added {
                modules.insert(key.clone(), value);
                reachable.insert(key);
            }
        }
        for key in &reachable {
            let (index, body) = modules[key];
            let source = &self.sources[index];
            for &id in &source.nodes[body].children {
                if source.nodes[id].kind == "mod_item" {
                    let name = source.text(source.child(id, &["name"]));
                    let nested = (
                        key.0.clone(),
                        if key.1.is_empty() {
                            name.to_owned()
                        } else {
                            format!("{}::{name}", key.1)
                        },
                    );
                    if body == 0 {
                        if let Some((target, _)) =
                            modules.get(&nested).filter(|_| reachable.contains(&nested))
                        {
                            self.imports.insert(
                                (source.path.clone(), name.into()),
                                self.sources[*target].path.clone(),
                            );
                        }
                    }
                }
            }
        }
        let mut exports: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        for key in &reachable {
            let (index, body) = modules[key];
            let source = &self.sources[index];
            let declared = source.nodes[body]
                .children
                .iter()
                .filter(|id| {
                    matches!(
                        source.nodes[**id].kind.as_str(),
                        "struct_item"
                            | "enum_item"
                            | "trait_item"
                            | "type_item"
                            | "function_item"
                            | "const_item"
                            | "static_item"
                            | "mod_item"
                    )
                })
                .map(|id| source.text(source.child(*id, &["name"])).to_owned())
                .collect::<BTreeSet<_>>();
            let mut scope = RustScope {
                start: source.nodes[body].start,
                end: source.nodes[body].end,
                visible: declared.clone(),
                declared,
                imports: BTreeMap::new(),
                has_unknown_glob: false,
            };
            let mut public = BTreeSet::new();
            for &id in &source.nodes[body].children {
                if source.first(id, &["visibility_modifier"]).is_some() {
                    if let Some(name) = source.child(id, &["name"]) {
                        public.insert(source.text(Some(name)).to_owned());
                    }
                }
                if source.nodes[id].kind == "use_declaration" {
                    for (alias, path) in use_paths(source, source.child(id, &["argument"]), "") {
                        if alias != "*" {
                            scope.visible.insert(alias.clone());
                            scope.imports.insert(alias.clone(), path.clone());
                            if body == 0 {
                                self.import_paths.insert((source.path.clone(), alias), path);
                            }
                        }
                    }
                }
                if source.nodes[id].kind == "macro_invocation" {
                    scope.has_unknown_glob = true;
                }
            }
            self.rust_scopes.entry(index).or_default().push(scope);
            exports.insert(key.clone(), public);
        }
        for _ in 0..12 {
            let mut changed = false;
            for key in &reachable {
                let (index, body) = modules[key];
                let source = &self.sources[index];
                let scope_index = self.rust_scopes[&index]
                    .iter()
                    .position(|s| {
                        s.start == source.nodes[body].start && s.end == source.nodes[body].end
                    })
                    .unwrap();
                for &id in &source.nodes[body].children {
                    if source.nodes[id].kind != "use_declaration" {
                        continue;
                    }
                    for (alias, path) in use_paths(source, source.child(id, &["argument"]), "") {
                        let mut parts: Vec<_> = path.split("::").map(str::to_owned).collect();
                        let mut module: Vec<_> = key
                            .1
                            .split("::")
                            .filter(|p| !p.is_empty())
                            .map(str::to_owned)
                            .collect();
                        let mut target_root = key.0.clone();
                        if let Some(mapped) = parts
                            .first()
                            .and_then(|name| self.module_bindings.get(name))
                        {
                            if let Some((root, _)) =
                                modules.iter().find(|((_, module), (target, _))| {
                                    module.is_empty() && self.sources[*target].path == *mapped
                                })
                            {
                                target_root = root.0.clone();
                                module.clear();
                                parts.remove(0);
                            }
                        }
                        if parts.first().is_some_and(|p| p == "crate") {
                            module.clear();
                            parts.remove(0);
                        } else if parts.first().is_some_and(|p| p == "self") {
                            parts.remove(0);
                        } else {
                            while parts.first().is_some_and(|p| p == "super") {
                                parts.remove(0);
                                module.pop();
                            }
                        }
                        let Some(name) = parts.pop() else {
                            continue;
                        };
                        module.extend(parts);
                        let target_key = (target_root, module.join("::"));
                        let target = modules
                            .get(&target_key)
                            .filter(|_| reachable.contains(&target_key));
                        if alias == "*" {
                            if let Some((target, _)) = target {
                                let names = exports.get(&target_key).cloned().unwrap_or_default();
                                for name in names {
                                    if self.rust_scopes.get_mut(&index).unwrap()[scope_index]
                                        .visible
                                        .insert(name.clone())
                                    {
                                        changed = true;
                                    }
                                    if body == 0 {
                                        self.imports.insert(
                                            (source.path.clone(), name.clone()),
                                            format!("{}#{name}", self.sources[*target].path),
                                        );
                                    }
                                    if source.first(id, &["visibility_modifier"]).is_some() {
                                        changed |=
                                            exports.entry(key.clone()).or_default().insert(name);
                                    }
                                }
                            } else {
                                self.rust_scopes.get_mut(&index).unwrap()[scope_index]
                                    .has_unknown_glob = true;
                            }
                        } else if let Some((target, _)) = target {
                            if body == 0 {
                                self.imports.insert(
                                    (source.path.clone(), alias.clone()),
                                    format!("{}#{name}", self.sources[*target].path),
                                );
                            }
                            if source.first(id, &["visibility_modifier"]).is_some() {
                                changed |= exports.entry(key.clone()).or_default().insert(alias);
                            }
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }
}
impl Interpreter<'_, '_> {
    pub fn rust_standard_path(
        &self,
        path: &str,
        canonical: &[&str],
        prelude: Option<&str>,
    ) -> bool {
        let path = path.trim_start_matches(':');
        let mut parts = path.split("::");
        let root = parts.next().unwrap_or_default();
        if self.local_bindings.contains(root)
            || self.env.contains_key(root)
            || self.rust_type_parameters.contains_key(root)
        {
            return false;
        }
        let offset = self
            .function
            .map(|f| self.source.nodes[self.analyzer.program.functions[f].node].start)
            .unwrap_or(0);
        let scope = self
            .analyzer
            .program
            .rust_scopes
            .get(&self.source_index)
            .and_then(|scopes| {
                scopes
                    .iter()
                    .filter(|s| s.start <= offset && offset <= s.end)
                    .min_by_key(|s| s.end - s.start)
            });
        if scope.is_some_and(|s| s.declared.contains(root)) {
            return false;
        }
        let imported = scope.and_then(|s| s.imports.get(root)).or_else(|| {
            self.analyzer
                .program
                .import_paths
                .get(&(self.source.path.clone(), root.into()))
        });
        let suffix = parts.collect::<Vec<_>>().join("::");
        let resolved = if suffix.is_empty() {
            imported.map(String::as_str).unwrap_or(root).to_owned()
        } else {
            format!("{}::{suffix}", imported.map(String::as_str).unwrap_or(root))
        };
        if canonical.contains(&resolved.as_str()) {
            return true;
        }
        prelude == Some(path)
            && scope.is_some_and(|s| !s.visible.contains(root) && !s.has_unknown_glob)
    }
}
