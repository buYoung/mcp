use super::*;
use crate::parser::ImportKind;
use std::path::{Component, PathBuf};

impl Query<'_> {
    pub(super) fn allowed(&mut self, location: &Location) -> bool {
        if self.scope.is_some_and(|scope| {
            location.path != scope
                && !location
                    .path
                    .strip_prefix(scope)
                    .is_some_and(|tail| tail.starts_with('/'))
        }) {
            self.diagnostic(
                location,
                "dependency is outside the requested workspace scope",
            );
            return false;
        }
        if self.filter.is_file_excluded(&location.path)
            || self.filter.is_excluded(&location.path, &location.range)
        {
            return false;
        }
        if let Some(&allowed) = self.files.get(&location.path) {
            return allowed;
        }
        if self.files.len() >= FILES_PER_QUERY {
            self.diagnostic(location, "value-flow file budget reached");
            return false;
        }
        let path = self.root.join(&location.path);
        let expected = self
            .index
            .digest(&location.path)
            .map(str::to_string)
            .or_else(|| {
                self.index
                    .sources
                    .get(&location.path)
                    .map(|source| crate::implementations::digest(source.as_bytes()))
            });
        let reason = if !crate::workspace::walk_root_is_visible(&path, true) {
            Some("flow input is excluded by the current file policy")
        } else if let Some(expected) = expected {
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.len() > crate::config::get().max_file_size => {
                    Some("flow input exceeds max_file_size")
                }
                Ok(metadata) if self.source_bytes + metadata.len() as usize > 8 * 1024 * 1024 => {
                    Some("value-flow source byte budget reached")
                }
                Ok(_) => {
                    use std::io::Read;
                    let mut bytes = Vec::new();
                    let remaining = (8 * 1024 * 1024 - self.source_bytes)
                        .min(crate::config::get().max_file_size as usize);
                    match std::fs::File::open(&path)
                        .and_then(|file| file.take(remaining as u64 + 1).read_to_end(&mut bytes))
                    {
                        Ok(_) if bytes.len() > remaining => {
                            Some("value-flow source byte budget reached while reading")
                        }
                        Ok(_) => {
                            self.source_bytes += bytes.len();
                            if crate::implementations::digest(&bytes) == expected {
                                None
                            } else {
                                Some("flow input changed since indexing; stale relationships withheld")
                            }
                        }
                        Err(_) => Some("flow input could not be read"),
                    }
                }
                Err(_) => Some("flow input is missing or inaccessible"),
            }
        } else {
            Some("flow input is absent from this index generation")
        };
        let allowed = reason.is_none();
        self.files.insert(location.path.clone(), allowed);
        if let Some(reason) = reason {
            self.diagnostic(location, reason);
        }
        allowed
    }
    pub(super) fn binding_value(
        &mut self,
        key: BindingKey,
        frame: &mut Frame,
        location: &Location,
        depth: usize,
    ) -> ValueId {
        if let Some(&value) = frame.values.get(&key) {
            return self.transfer(value, location, "variable use", false);
        }
        if let Some(&value) = self.globals.get(&key) {
            self.touched.insert(key);
            return self.transfer(value, location, "shared binding use", false);
        }
        let Some(binding) = self
            .index
            .unit(&key.path, key.unit)
            .and_then(|unit| unit.bindings.get(key.binding).cloned())
        else {
            return self.unknown(location, "binding is absent from this flow snapshot");
        };
        let definition = Location {
            path: key.path.clone(),
            range: binding.range.clone(),
            name: binding.name.clone(),
        };
        if !self.allowed(&definition) {
            return self.unknown(location, "binding source is unavailable");
        }
        if !matches!(
            binding.kind,
            BindingKind::Local | BindingKind::Parameter { .. }
        ) {
            let ambiguous = self.index.unit(&key.path, key.unit).is_some_and(|unit| {
                unit.bindings.iter().enumerate().any(|(id, other)| {
                    id != key.binding && other.name == binding.name && other.scope == binding.scope
                })
            });
            if binding.is_mutated || ambiguous {
                return self.unknown(
                    location,
                    "reassigned/duplicate declaration has no unique callable or object value",
                );
            }
        }
        match binding.kind {
            BindingKind::Function(function) => {
                let function = FunctionKey {
                    path: key.path,
                    unit: key.unit,
                    function,
                };
                let closure = self.closure(function, frame.values.clone(), None);
                self.value(ValueKind::Function(closure), definition, Evidence::Source)
            }
            BindingKind::Class(class) => self.value(
                ValueKind::Class {
                    path: key.path,
                    unit: key.unit,
                    class,
                },
                definition,
                Evidence::Source,
            ),
            BindingKind::Parameter { .. } => {
                self.value(ValueKind::Parameter, definition, Evidence::Source)
            }
            BindingKind::Import(import) => {
                self.import_value(&key, import, frame, location, depth + 1)
            }
            BindingKind::Local => {
                if self
                    .index
                    .unit(&key.path, key.unit)
                    .is_some_and(|unit| !unit.functions[0].is_available)
                {
                    return self.unknown(location, "module name-resolution summary is incomplete");
                }
                let Some(initializer) = binding.initializer else {
                    return self.unknown(location, "binding initializer is unavailable");
                };
                let is_global = self
                    .index
                    .unit(&key.path, key.unit)
                    .and_then(|unit| unit.nodes.get(initializer).cloned())
                    .is_some_and(|node| node.function == 0);
                if !is_global {
                    return self.unknown(
                        location,
                        "local binding is uninitialized or outside this call context",
                    );
                }
                if binding.is_mutated {
                    return self.unknown(
                        location,
                        "mutable global binding has no unique value version",
                    );
                }
                if !self.loading.insert(key.clone()) {
                    return self.unknown(location, "cyclic initializer/import is unresolved");
                }
                let mut module = Frame {
                    key: FunctionKey {
                        path: key.path.clone(),
                        unit: key.unit,
                        function: 0,
                    },
                    values: BTreeMap::new(),
                    receiver: None,
                };
                let value = self.expression(initializer, &mut module, depth + 1);
                let value = self.transfer(value, &definition, "initializer → binding", false);
                self.globals.insert(key.clone(), value);
                self.loading.remove(&key);
                self.touched.insert(key);
                self.transfer(value, location, "shared binding use", false)
            }
        }
    }
    fn import_value(
        &mut self,
        key: &BindingKey,
        import: usize,
        frame: &mut Frame,
        location: &Location,
        depth: usize,
    ) -> ValueId {
        if depth > CALL_DEPTH {
            self.diagnostic(location, "import/re-export depth budget reached");
            return self.unknown(location, "import depth exceeded");
        }
        let Some(entry) = self
            .index
            .unit(&key.path, key.unit)
            .and_then(|unit| unit.imports.get(import).cloned())
        else {
            return self.unknown(location, "import metadata unavailable");
        };
        let language = self
            .index
            .unit(&key.path, key.unit)
            .unwrap()
            .language
            .clone();
        if language == "rust" {
            return self.rust_function(&key.path, &entry.range, &entry.local_name, frame, location);
        }
        let Some(source) = entry.source.as_deref() else {
            return self.unknown(location, "dynamic import source is unresolved");
        };
        let Some(path) = self.module_path(&key.path, source) else {
            return self.unknown(
                location,
                &format!(
                    "external/unindexed module {source:?}; implementation semantics unavailable"
                ),
            );
        };
        if entry.kind == ImportKind::Namespace {
            return self.value(
                ValueKind::Namespace { path, unit: 0 },
                location.clone(),
                Evidence::Source,
            );
        }
        let exported = if entry.kind == ImportKind::Default {
            "default"
        } else {
            entry.imported_name.as_deref().unwrap_or(&entry.local_name)
        };
        self.export_value(&path, 0, exported, frame, location, depth + 1)
    }
    pub(super) fn export_value(
        &mut self,
        path: &str,
        unit: usize,
        name: &str,
        frame: &mut Frame,
        location: &Location,
        depth: usize,
    ) -> ValueId {
        if depth > CALL_DEPTH {
            return self.unknown(location, "cyclic/deep re-export is unresolved");
        }
        let module_location = Location {
            path: path.into(),
            range: crate::parser::CodeRange {
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 2,
            },
            name: "module/re-export evidence".into(),
        };
        if !self.allowed(&module_location) {
            return self.unknown(
                location,
                "module/re-export evidence is stale or unavailable",
            );
        }
        let Some(export) = self
            .index
            .unit(path, unit)
            .and_then(|unit| unit.exports.get(name).cloned())
        else {
            return self.unknown(location, "export has no unique indexed value definition");
        };
        match export {
            Export::Local(name) => {
                let bindings = self
                    .index
                    .unit(path, unit)
                    .unwrap()
                    .bindings
                    .iter()
                    .enumerate()
                    .filter(|(_, binding)| {
                        binding.name == name
                            && !matches!(binding.kind, BindingKind::Parameter { .. })
                    })
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>();
                if bindings.len() != 1 {
                    return self.unknown(location, "export binding is ambiguous");
                }
                self.binding_value(
                    BindingKey {
                        path: path.into(),
                        unit,
                        binding: bindings[0],
                    },
                    frame,
                    location,
                    depth + 1,
                )
            }
            Export::Foreign { source, name } => {
                let Some(path) = self.module_path(path, &source) else {
                    return self.unknown(location, "re-export source unavailable");
                };
                self.export_value(&path, 0, &name, frame, location, depth + 1)
            }
            Export::Unknown => self.unknown(location, "export semantics unresolved"),
        }
    }
    fn module_path(&self, path: &str, module: &str) -> Option<String> {
        if !module.starts_with('.') {
            return None;
        }
        let joined = Path::new(path).parent()?.join(module);
        let mut normalized = PathBuf::new();
        for component in joined.components() {
            match component {
                Component::Normal(value) => normalized.push(value),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !normalized.pop() {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        let stem = normalized.to_string_lossy().replace('\\', "/");
        let mut candidates = Vec::new();
        if self.index.has_file(&stem) {
            candidates.push(stem.clone());
        }
        for extension in ["ts", "tsx", "js", "jsx", "mts", "mjs", "cts", "cjs"] {
            for candidate in [
                format!("{stem}.{extension}"),
                format!("{stem}/index.{extension}"),
            ] {
                if self.index.has_file(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
        (candidates.len() == 1).then(|| candidates.remove(0))
    }
    pub(super) fn unbound(
        &mut self,
        name: &str,
        frame: &mut Frame,
        location: &Location,
    ) -> ValueId {
        if matches!(name, "this" | "self" | "$this") {
            if let Some(receiver) = frame.receiver {
                return self.transfer(receiver, location, "receiver use", false);
            }
        }
        let language = self
            .index
            .unit(&frame.key.path, frame.key.unit)
            .unwrap()
            .language
            .clone();
        if matches!(language.as_str(), "typescript" | "javascript") && name == "Map" {
            if !self.should_model_collections
                || self
                    .index
                    .unit(&frame.key.path, frame.key.unit)
                    .unwrap()
                    .has_modified_map_builtin
            {
                return self.unknown(
                    location,
                    "Map built-in binding/prototype may be modified in this source",
                );
            }
            return self.value(ValueKind::MapConstructor, location.clone(), Evidence::Model);
        }
        if language == "rust" {
            return self.rust_function(
                &frame.key.path.clone(),
                &location.range.clone(),
                name,
                frame,
                location,
            );
        }
        if language == "go" {
            let parent = Path::new(&frame.key.path).parent();
            let namespace = self
                .index
                .unit(&frame.key.path, frame.key.unit)
                .unwrap()
                .namespace
                .clone();
            let candidates = self
                .index
                .codemap
                .iter()
                .filter(|file| {
                    file.file_path.ends_with(".go") && Path::new(&file.file_path).parent() == parent
                })
                .flat_map(|file| {
                    file.symbols
                        .iter()
                        .map(move |symbol| (&file.file_path, symbol))
                })
                .filter(|(_, symbol)| {
                    symbol.kind == "fn" && symbol.owner.is_none() && symbol.name == name
                })
                .map(|(path, symbol)| (path.clone(), symbol.range.clone()))
                .take(2)
                .collect::<Vec<_>>();
            if candidates.len() == 1 {
                let (path, range) = &candidates[0];
                if let Some(unit) = self
                    .index
                    .unit(path, 0)
                    .filter(|unit| unit.namespace == namespace)
                {
                    if let Some(function) = unit
                        .functions
                        .iter()
                        .position(|function| &function.range == range)
                    {
                        let is_global = unit.bindings.iter().any(|binding| {
                            matches!(binding.kind,BindingKind::Function(id) if id==function)
                                && binding.scope == unit.functions[0].range
                        });
                        let key = FunctionKey {
                            path: path.clone(),
                            unit: 0,
                            function,
                        };
                        let definition = self.index.location(&key).unwrap();
                        if is_global && self.allowed(&definition) {
                            let closure = self.closure(key, BTreeMap::new(), None);
                            return self.value(
                                ValueKind::Function(closure),
                                definition,
                                Evidence::Source,
                            );
                        }
                    }
                }
            }
        }
        self.unknown(
            location,
            &format!("{name} has no source-proven value/call target"),
        )
    }
    fn rust_function(
        &mut self,
        path: &str,
        range: &crate::parser::CodeRange,
        name: &str,
        frame: &Frame,
        location: &Location,
    ) -> ValueId {
        self.resolver().reset_dependency_tracking();
        let Some(file) = self
            .index
            .codemap
            .iter()
            .find(|file| file.file_path == path)
        else {
            return self.unknown(location, "Rust source unavailable");
        };
        let Some(target) = self.resolver().resolve_name(file, range, name, "call") else {
            return self.unknown(location, "Rust module/cfg/call target unresolved");
        };
        let target_path = target.file.file_path.clone();
        let target_range = target.symbol.range.clone();
        let dependencies = self.resolver().source_dependencies();
        for (path, range) in dependencies {
            if !self.allowed(&Location {
                path,
                range,
                name: "module evidence".into(),
            }) {
                return self.unknown(location, "Rust module evidence unavailable");
            }
        }
        let key = self.index.file(&target_path).and_then(|file| {
            file.units.iter().enumerate().find_map(|(unit, flow)| {
                flow.functions
                    .iter()
                    .enumerate()
                    .find(|(_, function)| function.range == target_range)
                    .map(|(function, _)| FunctionKey {
                        path: target_path.clone(),
                        unit,
                        function,
                    })
            })
        });
        let Some(key) = key else {
            return self.unknown(location, "Rust function summary unavailable");
        };
        let definition = self.index.location(&key).unwrap();
        let closure = self.closure(key, frame.values.clone(), frame.receiver);
        self.value(ValueKind::Function(closure), definition, Evidence::Source)
    }
}
