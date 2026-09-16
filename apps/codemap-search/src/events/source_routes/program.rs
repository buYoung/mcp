use super::syntax::*;
use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::sync::Arc;

impl Program {
    pub fn new(
        sources: Vec<Arc<Source>>,
        module_bindings: std::collections::BTreeMap<String, String>,
    ) -> Self {
        let mut program = Self {
            sources,
            module_bindings,
            ..Self::default()
        };
        for source in &program.sources {
            if source.nodes.is_empty() {
                continue;
            }
            let mut namespace = String::new();
            for &id in &source.nodes[0].children {
                let kind = source.nodes[id].kind.as_str();
                if matches!(
                    kind,
                    "package_declaration"
                        | "package_header"
                        | "package_clause"
                        | "namespace_definition"
                        | "namespace_declaration"
                        | "file_scoped_namespace_declaration"
                ) {
                    namespace = source.text(source.child(id, &["name"])).to_owned();
                    if namespace.is_empty() {
                        namespace = source
                            .text(Some(id))
                            .trim_start_matches("package ")
                            .trim_start_matches("namespace ")
                            .split([';', '{'])
                            .next()
                            .unwrap_or_default()
                            .trim()
                            .to_owned();
                    }
                }
                if matches!(kind, "import_declaration" | "import_header") {
                    let text = source
                        .text(Some(id))
                        .trim_start_matches("import ")
                        .trim()
                        .trim_end_matches(';')
                        .trim();
                    if let Some(prefix) =
                        text.strip_suffix("._").or_else(|| text.strip_suffix(".*"))
                    {
                        program
                            .namespace_imports
                            .entry(source.path.clone())
                            .or_default()
                            .insert(prefix.into());
                    } else if let Some((prefix, selectors)) = text.split_once(".{") {
                        for name in selectors.trim_end_matches('}').split(',').map(str::trim) {
                            if is_name(name) {
                                program.imports.insert(
                                    (source.path.clone(), name.into()),
                                    format!("{prefix}.{name}"),
                                );
                            }
                        }
                    } else {
                        program.imports.insert(
                            (
                                source.path.clone(),
                                text.rsplit('.').next().unwrap_or(text).into(),
                            ),
                            text.into(),
                        );
                    }
                }
                if kind == "using_directive" {
                    let text = source
                        .text(Some(id))
                        .trim_start_matches("using ")
                        .trim()
                        .trim_end_matches(';')
                        .trim();
                    if !text.contains('=') && !text.starts_with("static ") {
                        program
                            .namespace_imports
                            .entry(source.path.clone())
                            .or_default()
                            .insert(text.into());
                    }
                }
            }
            if source.language == "go" {
                namespace = Path::new(&source.path)
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_string_lossy()
                    .into();
            }
            program.namespaces.insert(source.path.clone(), namespace);
        }
        // Collect every type before resolving receivers or imports from another
        // file. Path iteration order must not decide whether a method exists.
        for index in 0..program.sources.len() {
            let source = program.sources[index].clone();
            if source.nodes.is_empty() {
                continue;
            }
            for id in source.walk(0, false) {
                if is_class(&source.nodes[id].kind) && source.nodes[id].kind != "impl_item" {
                    let named = source.child(id, &["name"]).or_else(|| {
                        source.first(id, &["type_identifier", "identifier", "constant"])
                    });
                    let name = source.text(named);
                    if !name.is_empty() {
                        let name = scoped_name(&source, id, name);
                        let owner = program.owner_key(index, &name);
                        if source.language == "scala" {
                            let mut names = vec![name.clone()];
                            let mut parent = source.nodes[id].parent;
                            while let Some(node) = parent {
                                if matches!(
                                    source.nodes[node].kind.as_str(),
                                    "class_definition" | "object_definition" | "trait_definition"
                                ) {
                                    names.push(source.text(source.child(node, &["name"])).into());
                                }
                                parent = source.nodes[node].parent;
                            }
                            if names.len() > 1 {
                                names.reverse();
                                program
                                    .types
                                    .insert((source.path.clone(), names.join(".")), owner.clone());
                            }
                        }
                        let namespace = program.type_namespace(index);
                        program.types.insert((namespace, name), owner.clone());
                        if source.language == "rust"
                            && source.child(id, &["type_parameters"]).is_some_and(|p| {
                                source.nodes[p]
                                    .children
                                    .iter()
                                    .any(|n| source.nodes[*n].kind != "lifetime_parameter")
                            })
                        {
                            program.generic_types.insert(owner.clone());
                        }
                        program.owner_sources.insert(owner, index);
                    }
                }
                if matches!(
                    source.nodes[id].kind.as_str(),
                    "type_alias_declaration"
                        | "type_item"
                        | "type_spec"
                        | "alias_declaration"
                        | "typealias_declaration"
                ) {
                    let name = source.text(source.child(id, &["name"]));
                    let target = source.text(source.child(id, &["value", "type"]));
                    if !name.is_empty() && !target.is_empty() {
                        program
                            .aliases
                            .insert((program.type_namespace(index), name.into()), target.into());
                    }
                }
            }
        }
        program.resolve_imports();
        program.resolve_esm();
        program.resolve_rust_names();
        for source in &program.sources {
            for (name, path) in &source.generated_imports {
                program
                    .imports
                    .insert((source.path.clone(), name.clone()), path.clone());
            }
        }
        for index in 0..program.sources.len() {
            if !program.sources[index].nodes.is_empty() {
                program.collect(index, 0, "", None);
            }
        }
        program.collect_jvm();
        program.collect_scala();
        program
    }
    pub fn type_namespace(&self, source: usize) -> String {
        let source = &self.sources[source];
        if source.language == "go" {
            self.namespaces
                .get(&source.path)
                .cloned()
                .unwrap_or_default()
        } else {
            source.path.clone()
        }
    }
    pub fn owner_key(&self, source_index: usize, name: &str) -> String {
        let source = &self.sources[source_index];
        let name = if source.language == "rust" {
            name.split('<').next().unwrap_or(name).trim()
        } else {
            name
        };
        let namespace = self
            .namespaces
            .get(&source.path)
            .filter(|v| !v.is_empty())
            .map(String::as_str)
            .unwrap_or(&source.path);
        if matches!(
            source.language.as_str(),
            "typescript" | "javascript" | "go" | "rust"
        ) {
            format!("{}::{name}", self.type_namespace(source_index))
        } else {
            format!("{}:{namespace}::{name}", source.language)
        }
    }
    pub fn resolve_type(&self, source: usize, text: &str, scope: &str) -> String {
        self.resolve_type_at(source, text, scope, 0)
    }
    fn resolve_type_at(
        &self,
        source_index: usize,
        text: &str,
        scope: &str,
        depth: usize,
    ) -> String {
        if depth > 8 {
            return String::new();
        }
        let source = &self.sources[source_index];
        if self.types.values().any(|v| v == text) {
            return text.into();
        }
        let unions = split_top_level(text.trim_start_matches([':', ' ']), '|');
        let nonnull: Vec<_> = unions
            .iter()
            .filter(|v| !matches!(v.as_str(), "undefined" | "null" | "never"))
            .collect();
        if nonnull.len() != 1 {
            return String::new();
        }
        let mut clean = nonnull[0].as_str().trim().to_owned();
        for modifier in [
            "const ",
            "mut ",
            "struct ",
            "class ",
            "volatile ",
            "ref ",
            "out ",
            "in ",
            "final ",
            "typename ",
        ] {
            clean = clean.replace(modifier, "");
        }
        clean = clean.trim_matches(['&', '*', '?', ' ']).to_owned();
        if clean.starts_with('\'') {
            clean = clean
                .split_once(' ')
                .map(|(_, r)| r.trim_start_matches("mut ").to_owned())
                .unwrap_or_default();
        }
        if let Some(alias) = self
            .aliases
            .get(&(scope.into(), clean.clone()))
            .filter(|alias| !scope.is_empty() && *alias != &clean)
        {
            return self.resolve_type_at(source_index, alias, scope, depth + 1);
        }
        if source.language == "swift" {
            if let Some((base, member)) = clean.rsplit_once('.') {
                let parent = self.resolve_type_at(source_index, base, scope, depth + 1);
                if let Some(alias) = self.aliases.get(&(parent.clone(), member.into())) {
                    return self.resolve_type_at(
                        self.owner_sources
                            .get(&parent)
                            .copied()
                            .unwrap_or(source_index),
                        alias,
                        &parent,
                        depth + 1,
                    );
                }
            }
        }
        clean = clean
            .split(['<', '['])
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if clean.is_empty() {
            return String::new();
        }
        let namespace = self.type_namespace(source_index);
        if let Some(found) = self.types.get(&(namespace.clone(), clean.clone())) {
            return found.clone();
        }
        if let Some(target) = self.aliases.get(&(namespace.clone(), clean.clone())) {
            if target != &clean {
                return self.resolve_type_at(source_index, target, scope, depth + 1);
            }
        }
        if let Some(import) = self.imports.get(&(source.path.clone(), clean.clone())) {
            if let Some((path, name)) = import.rsplit_once('#') {
                if let Some(index) = self.sources.iter().position(|s| s.path == path) {
                    return self.resolve_type_at(index, name, "", depth + 1);
                }
            }
        }
        for separator in ["::", "."] {
            if let Some((prefix, member)) = clean.split_once(separator) {
                if let Some(path) = self.imports.get(&(source.path.clone(), prefix.into())) {
                    if source.language == "go" {
                        if let Some(owner) = self.types.get(&(path.clone(), member.into())) {
                            return owner.clone();
                        }
                    }
                    if let Some(index) = self.sources.iter().position(|s| &s.path == path) {
                        return self.resolve_type_at(index, member, "", depth + 1);
                    }
                }
            }
        }
        if matches!(source.language.as_str(), "typescript" | "javascript") {
            let name = scope.rsplit("::").next().unwrap_or_default();
            let mut prefix = name.rsplit_once('.').map(|p| p.0);
            while let Some(part) = prefix {
                if let Some(owner) = self
                    .types
                    .get(&(namespace.clone(), format!("{part}.{clean}")))
                {
                    return owner.clone();
                }
                prefix = part.rsplit_once('.').map(|p| p.0);
            }
        }
        let namespace = self
            .namespaces
            .get(&source.path)
            .map(String::as_str)
            .unwrap_or_default();
        let imported = self.imports.get(&(source.path.clone(), clean.clone()));
        let candidates: BTreeSet<_> = self
            .types
            .iter()
            .filter_map(|((path, name), owner)| {
                let ns = self
                    .namespaces
                    .get(path)
                    .map(String::as_str)
                    .unwrap_or_default();
                let qualified = format!("{ns}.{name}");
                let visible = imported == Some(&qualified)
                    || (!namespace.is_empty() && ns == namespace && name == &clean)
                    || clean == qualified
                    || self
                        .namespace_imports
                        .get(&source.path)
                        .is_some_and(|prefixes| {
                            prefixes.iter().any(|p| format!("{p}.{clean}") == qualified)
                        });
                visible.then_some(owner.clone())
            })
            .collect();
        if candidates.len() == 1 {
            return candidates.into_iter().next().unwrap_or_default();
        }
        if source.language == "swift" {
            let candidates: BTreeSet<_> = self
                .types
                .iter()
                .filter(|((_, name), owner)| name == &clean && owner.starts_with("swift:"))
                .map(|(_, owner)| owner.clone())
                .collect();
            if candidates.len() == 1 {
                return candidates.into_iter().next().unwrap();
            }
        }
        // An explicit C pointer type provides a translation-unit schema, not
        // alias or object identity. It never searches arbitrary other files.
        if matches!(source.language.as_str(), "c" | "cpp")
            && is_name(&clean)
            && !matches!(
                clean.as_str(),
                "int" | "char" | "void" | "size_t" | "bool" | "double" | "float" | "long"
            )
        {
            return self.owner_key(source_index, &clean);
        }
        String::new()
    }
    pub fn expanded_type(&self, source: usize, text: &str) -> String {
        let mut text = text.trim().trim_start_matches([':', ' ']).to_owned();
        let mut seen = BTreeSet::new();
        for _ in 0..12 {
            if !seen.insert(text.clone()) {
                break;
            }
            let Some(alias) = self
                .aliases
                .get(&(self.type_namespace(source), text.clone()))
            else {
                break;
            };
            text = alias.trim().into();
        }
        text
    }
    fn collect(
        &mut self,
        source_index: usize,
        id: NodeId,
        current_owner: &str,
        parent: Option<usize>,
    ) {
        let source = self.sources[source_index].clone();
        let node = &source.nodes[id];
        let mut owner = current_owner.to_owned();
        if source.language == "scala"
            && node.kind == "instance_expression"
            && source.first(id, &["template_body"]).is_some()
        {
            owner = self.owner_key(source_index, &format!("anonymous@{}", node.start));
            self.owner_sources.insert(owner.clone(), source_index);
            self.scala
                .anonymous
                .insert((source_index, id), owner.clone());
            self.scala
                .class_parameters
                .insert(owner.clone(), Vec::new());
        }
        if is_class(&node.kind) {
            let name_node = source
                .child(id, &["name", "type"])
                .or_else(|| source.first(id, &["type_identifier", "identifier", "constant"]));
            let name = source.text(name_node);
            if !name.is_empty() {
                let name = scoped_name(&source, id, name);
                owner = self.resolve_type(source_index, &name, "");
                if owner.is_empty() {
                    owner = self.owner_key(source_index, &name);
                }
                if node.kind == "impl_item" {
                    if let Some(params) = source.child(id, &["type_parameters"]) {
                        if source.nodes[params]
                            .children
                            .iter()
                            .any(|p| source.text(source.child(*p, &["name"])) == name)
                        {
                            owner =
                                format!("rust:type_variable:{}:{}:{name}", source.path, node.start);
                        }
                    }
                }
                self.owner_sources.insert(owner.clone(), source_index);
                self.collect_fields(source_index, id, &owner);
            }
        }
        if source.language == "c"
            && current_owner.is_empty()
            && node.kind == "declaration"
            && source.nodes[id].children.iter().any(|n| {
                source.nodes[*n].kind == "storage_class_specifier"
                    && source.text(Some(*n)) == "static"
            })
        {
            for declarator in node.fields.get("declarator").into_iter().flatten() {
                let target = if source.nodes[*declarator].kind == "init_declarator" {
                    source.child(*declarator, &["declarator"])
                } else {
                    Some(*declarator)
                };
                if target.is_some_and(|n| {
                    source.nodes[n].kind == "function_declarator"
                        && source
                            .child(n, &["declarator"])
                            .is_some_and(|d| source.nodes[d].kind == "identifier")
                }) {
                    continue;
                }
                if let Some(named) = source.identifier(target) {
                    let name = source.text(Some(named));
                    let value =
                        super::model::Value::new("global", format!("{}::static", source.path))
                            .field(name);
                    self.global_values
                        .insert((source.path.clone(), name.into()), value.clone());
                    if let Some(initial) = source.child(*declarator, &["value"]) {
                        self.global_initializers
                            .push((source_index, value, initial, id));
                    }
                }
            }
        }
        let mut enclosing = parent;
        if matches!(
            node.kind.as_str(),
            "alias_declaration" | "type_alias_declaration" | "typealias_declaration"
        ) {
            let mut named = source.child(id, &["name"]);
            let mut value = source.child(id, &["type", "value"]);
            if source.language == "swift" {
                if let Some(names) = node.fields.get("name").filter(|v| v.len() == 2) {
                    named = Some(names[0]);
                    value = Some(names[1]);
                }
            }
            if let (Some(named), Some(value)) = (named, value) {
                self.aliases.insert(
                    (owner.clone(), source.text(Some(named)).into()),
                    source.text(Some(value)).into(),
                );
            }
        }
        if is_function(&node.kind)
            || matches!(
                node.kind.as_str(),
                "method_signature" | "function_signature" | "getter"
            )
            || (source.language == "csharp"
                && node.kind == "property_declaration"
                && source
                    .child(id, &["value"])
                    .is_some_and(|v| source.nodes[v].kind == "arrow_expression_clause"))
        {
            let signature = source
                .first(id, &["function_signature", "constructor_signature"])
                .unwrap_or(id);
            let declarator = source.child(signature, &["declarator"]);
            let named = source
                .child(signature, &["name"])
                .or_else(|| source.identifier(declarator))
                .or_else(|| source.first(signature, &["identifier", "simple_identifier", "name"]));
            let mut name = source.text(named).to_owned();
            if name.is_empty() {
                if let Some(ancestor) = node.parent {
                    if matches!(
                        source.nodes[ancestor].kind.as_str(),
                        "variable_declarator" | "pair" | "public_field_definition"
                    ) {
                        name = source.text(source.child(ancestor, &["name", "key"])).into();
                    }
                }
            }
            if name.is_empty() {
                name = format!("closure@{}", node.line);
            }
            if source.language == "swift" && node.kind == "init_declaration" {
                name = "init".into();
            }
            let mut receiver_name = if matches!(
                source.language.as_str(),
                "rust" | "python" | "ruby" | "lua" | "swift"
            ) {
                "self"
            } else {
                "this"
            }
            .to_owned();
            if source.language == "php" {
                receiver_name = "this".into();
            }
            if source.language == "lua" {
                if let Some(named) = named.filter(|n| {
                    matches!(
                        source.nodes[*n].kind.as_str(),
                        "method_index_expression" | "dot_index_expression"
                    )
                }) {
                    owner =
                        self.owner_key(source_index, source.text(source.child(named, &["table"])));
                    name = source
                        .text(source.child(named, &["method", "field"]))
                        .into();
                }
            }
            if source.language == "go" && node.kind == "method_declaration" {
                if let Some(receiver) = source.child(id, &["receiver"]) {
                    if let Some((name, ty)) = parameter_names(&source, Some(receiver)).first() {
                        receiver_name = name.clone();
                        owner = self.resolve_type(source_index, ty, "");
                    }
                }
            }
            if parent.is_some()
                && matches!(source.language.as_str(), "typescript" | "javascript")
                && !matches!(node.kind.as_str(), "arrow_function" | "method_definition")
            {
                owner.clear();
            }
            let parameters = source
                .child(signature, &["parameters", "parameter"])
                .or_else(|| declarator.and_then(|d| source.child(d, &["parameters"])))
                .or_else(|| {
                    source.first(
                        signature,
                        &[
                            "formal_parameters",
                            "formal_parameter_list",
                            "function_value_parameters",
                            "parameter_list",
                            "parameters",
                            "method_parameters",
                        ],
                    )
                });
            let mut params = parameter_names(&source, parameters);
            if name == "constructor" && !owner.is_empty() {
                if let Some(parameters) = parameters {
                    for &param in &source.nodes[parameters].children {
                        if source.first(param, &["accessibility_modifier"]).is_some() {
                            let name = source.text(source.child(param, &["pattern", "name"]));
                            let ty = source.text(source.child(param, &["type"]));
                            self.fields.insert((owner.clone(), name.into()), ty.into());
                        }
                    }
                }
            }
            if params.is_empty() && source.language == "swift" {
                params = source.nodes[signature]
                    .children
                    .iter()
                    .filter(|id| source.nodes[**id].kind == "parameter")
                    .flat_map(|id| parameter_names(&source, Some(*id)))
                    .collect();
            }
            if source.language == "scala" {
                params = source.nodes[signature]
                    .children
                    .iter()
                    .filter(|id| source.nodes[**id].kind == "parameters")
                    .flat_map(|id| parameter_names(&source, Some(*id)))
                    .collect();
            }
            let mut body = source
                .child(id, &["body"])
                .or_else(|| source.first(id, &["function_body", "block", "body_statement"]));
            if source.language == "dart"
                && matches!(
                    node.kind.as_str(),
                    "method_signature" | "function_signature"
                )
            {
                if let Some(parent) = node.parent {
                    let siblings = &source.nodes[parent].children;
                    if let Some(pos) = siblings.iter().position(|n| *n == id) {
                        body = siblings
                            .get(pos + 1)
                            .copied()
                            .filter(|n| source.nodes[*n].kind == "function_body");
                    }
                }
            }
            if source.language == "csharp" && node.kind == "property_declaration" {
                body = source.child(id, &["value"]);
            }
            if source.language == "kotlin" && node.kind == "getter" {
                if let Some(parent) = node.parent {
                    let property = if source.nodes[parent].kind == "property_declaration" {
                        Some(parent)
                    } else {
                        let siblings = &source.nodes[parent].children;
                        siblings
                            .iter()
                            .position(|n| *n == id)
                            .and_then(|pos| pos.checked_sub(1))
                            .and_then(|pos| siblings.get(pos))
                            .copied()
                    };
                    if let Some(property) = property {
                        let named = source.child(property, &["name"]).or_else(|| {
                            source
                                .first(property, &["variable_declaration"])
                                .and_then(|n| source.identifier(Some(n)))
                        });
                        name = source.text(named).into();
                    }
                }
            }
            let index = self.functions.len();
            let is_getter = node.tokens.contains("get")
                || node.kind == "getter"
                || (source.language == "csharp" && node.kind == "property_declaration");
            let function = Function {
                identifier: format!(
                    "{}:{}:{}{}",
                    source.path,
                    node.line,
                    node.column,
                    source.expansion_identity(id)
                ),
                name: name.clone(),
                owner: owner.clone(),
                source: source_index,
                node: id,
                body,
                parameters: params,
                receiver_name,
                parent,
                is_static: node.tokens.contains("static"),
            };
            self.function_nodes.insert((source_index, id), index);
            let namespace = if owner.is_empty() {
                self.type_namespace(source_index)
            } else {
                owner.clone()
            };
            if is_getter {
                self.getters
                    .entry((namespace, name))
                    .or_default()
                    .push(index);
            } else if !node.tokens.contains("set") {
                self.methods
                    .entry((namespace, name))
                    .or_default()
                    .push(index);
            }
            if source.language == "rust" {
                let mut ancestor = node.parent;
                while let Some(parent) = ancestor {
                    if source.nodes[parent].kind == "impl_item" {
                        let trait_name = source.text(source.child(parent, &["trait"]));
                        let resolved = self
                            .import_paths
                            .get(&(source.path.clone(), trait_name.into()))
                            .map(String::as_str)
                            .unwrap_or(trait_name);
                        let mode = if matches!(resolved, "std::ops::Deref" | "core::ops::Deref")
                            && function.name == "deref"
                        {
                            Some("shared")
                        } else if matches!(resolved, "std::ops::DerefMut" | "core::ops::DerefMut")
                            && function.name == "deref_mut"
                        {
                            Some("mutable")
                        } else {
                            None
                        };
                        if let Some(mode) = mode {
                            self.dereferences
                                .entry((owner.clone(), mode.into()))
                                .or_default()
                                .push(index);
                        }
                        break;
                    }
                    ancestor = source.nodes[parent].parent;
                }
            }
            self.functions.push(function);
            enclosing = Some(index);
        }
        for &child in &node.children {
            if source.language == "dart"
                && node.kind == "method_signature"
                && source.nodes[child].kind == "function_signature"
            {
                continue;
            }
            self.collect(source_index, child, &owner, enclosing);
        }
    }
    fn collect_fields(&mut self, source_index: usize, id: NodeId, owner: &str) {
        let source = self.sources[source_index].clone();
        let body = source.child(id, &["body", "type"]).or_else(|| {
            source.first(
                id,
                &[
                    "class_body",
                    "template_body",
                    "declaration_list",
                    "field_declaration_list",
                ],
            )
        });
        let Some(body) = body else {
            return;
        };
        if source.language == "java"
            && source.nodes[id].kind == "interface_declaration"
            && source.child(id, &["interfaces"]).is_none()
        {
            let methods: Vec<_> = source.nodes[body]
                .children
                .iter()
                .filter(|n| {
                    source.nodes[**n].kind == "method_declaration"
                        && source.child(**n, &["body"]).is_none()
                        && !source.text(Some(**n)).contains("static ")
                        && !source.text(Some(**n)).contains("default ")
                })
                .map(|n| source.text(source.child(*n, &["name"])).to_owned())
                .collect();
            if methods.len() == 1 {
                self.sam_methods.insert(owner.into(), methods[0].clone());
            }
        }
        if source.nodes[body].kind == "ordered_field_declaration_list" {
            let fields = source.nodes[body]
                .fields
                .get("type")
                .cloned()
                .unwrap_or_default();
            self.tuple_fields.insert(
                owner.into(),
                fields
                    .iter()
                    .map(|id| source.text(Some(*id)).into())
                    .collect(),
            );
            for (index, field) in fields.into_iter().enumerate() {
                self.fields.insert(
                    (owner.into(), index.to_string()),
                    source.text(Some(field)).into(),
                );
            }
        }
        for field in source.walk(body, true) {
            let kind = source.nodes[field].kind.as_str();
            if source.language == "go"
                && source.nodes[body].kind == "interface_type"
                && matches!(kind, "method_elem" | "method_spec")
            {
                let name = source.text(source.child(field, &["name"]));
                if !name.is_empty() {
                    self.interface_methods
                        .entry(owner.into())
                        .or_default()
                        .insert(name.into());
                }
            }
            if !matches!(
                kind,
                "public_field_definition"
                    | "field_definition"
                    | "field_declaration"
                    | "property_signature"
                    | "property_declaration"
                    | "declaration"
                    | "var_definition"
                    | "val_definition"
                    | "event_field_declaration"
            ) {
                continue;
            }
            // Nested class fields belong to the nested owner; never hoist them.
            let mut ancestor = source.nodes[field].parent;
            let mut nested_owner = false;
            while let Some(parent) = ancestor {
                if parent == body || parent == id {
                    break;
                }
                if is_class(&source.nodes[parent].kind) {
                    nested_owner = true;
                    break;
                }
                ancestor = source.nodes[parent].parent;
            }
            if nested_owner {
                continue;
            }
            let type_node = source
                .child(field, &["type"])
                .or_else(|| source.first(field, &["type_annotation", "function_type", "user_type"]))
                .or_else(|| {
                    source
                        .first(field, &["variable_declaration"])
                        .and_then(|n| source.child(n, &["type"]))
                });
            let type_text = source.text(type_node).to_owned();
            if source.language == "go"
                && kind == "field_declaration"
                && source.child(field, &["name"]).is_none()
            {
                let clean = type_text.trim_start_matches('*');
                if clean
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
                    && !clean.is_empty()
                {
                    let name = clean.rsplit('.').next().unwrap();
                    self.embedded_fields
                        .entry(owner.into())
                        .or_default()
                        .push(name.into());
                    self.fields
                        .insert((owner.into(), name.into()), type_text.clone());
                }
            }
            let mut candidates = Vec::new();
            if let Some(named) = source
                .child(field, &["name", "pattern", "property"])
                .and_then(|n| source.identifier(Some(n)))
            {
                candidates.push((named, source.child(field, &["value"])));
            }
            for part in source.walk(field, true) {
                if matches!(
                    source.nodes[part].kind.as_str(),
                    "variable_declarator"
                        | "property_element"
                        | "initialized_identifier"
                        | "variable_declaration"
                        | "init_declarator"
                ) {
                    if source.language == "csharp"
                        && source.nodes[part].kind == "variable_declaration"
                    {
                        continue;
                    }
                    if let Some(named) = source.identifier(
                        source.child(part, &["name", "declarator"]).or_else(|| {
                            source
                                .first(part, &["identifier", "simple_identifier", "variable_name"])
                        }),
                    ) {
                        let mut value = source.child(part, &["value"]);
                        if value.is_none()
                            && source.nodes[part].kind == "initialized_identifier"
                            && source.nodes[part].children.len() > 1
                        {
                            value = source.nodes[part].children.last().copied();
                        }
                        if value.is_none()
                            && source.language == "kotlin"
                            && source.nodes[part].kind == "variable_declaration"
                        {
                            value = source.nodes[field]
                                .children
                                .last()
                                .copied()
                                .filter(|last| *last != part);
                        }
                        candidates.push((named, value));
                    }
                }
            }
            if let Some(declarators) = source.nodes[field].fields.get("declarator") {
                for &part in declarators {
                    if let Some(named) = source.identifier(Some(part)) {
                        candidates.push((named, source.child(part, &["value"])));
                    }
                }
            }
            for (named, value) in candidates {
                let name = source.text(Some(named)).trim_start_matches('$').to_owned();
                if name.is_empty() {
                    continue;
                }
                self.fields
                    .insert((owner.into(), name.clone()), type_text.clone());
                if kind == "event_field_declaration" {
                    self.events.insert((owner.into(), name.clone()));
                }
                if let Some(value) = value.filter(|v| *v != named) {
                    self.initializers
                        .push((source_index, owner.into(), name, value, field));
                }
            }
        }
    }
    fn resolve_imports(&mut self) {
        for source in &self.sources {
            if source.nodes.is_empty() {
                continue;
            }
            for id in source.walk(0, true) {
                if source.nodes[id].kind == "import_statement" {
                    let specifier = source
                        .text(source.child(id, &["source"]))
                        .trim_matches(['\'', '"']);
                    let Some(target) = self.resolve_relative(&source.path, specifier) else {
                        continue;
                    };
                    for part in source.walk(id, false) {
                        if source.nodes[part].kind == "import_specifier" {
                            let name = source.text(source.child(part, &["name"]));
                            let alias = source.text(source.child(part, &["alias"]));
                            self.imports.insert(
                                (
                                    source.path.clone(),
                                    if alias.is_empty() { name } else { alias }.into(),
                                ),
                                format!("{target}#{name}"),
                            );
                        } else if source.nodes[part].kind == "namespace_import" {
                            if let Some(name) = source.first(part, &["identifier"]) {
                                self.imports.insert(
                                    (source.path.clone(), source.text(Some(name)).into()),
                                    target.clone(),
                                );
                            }
                        }
                    }
                }
                if source.nodes[id].kind == "import_spec" && source.language == "go" {
                    let import = source
                        .text(source.child(id, &["path"]))
                        .trim_matches(['"', '`']);
                    let name = source.text(source.child(id, &["name"]));
                    let name = if name.is_empty() {
                        import.rsplit('/').next().unwrap_or(import)
                    } else {
                        name
                    };
                    self.import_paths
                        .insert((source.path.clone(), name.into()), import.into());
                    let namespaces: BTreeSet<_> = self
                        .namespaces
                        .values()
                        .filter(|ns| {
                            !ns.is_empty()
                                && (import == ns.as_str() || import.ends_with(&format!("/{ns}")))
                        })
                        .collect();
                    if namespaces.len() == 1 {
                        self.imports.insert(
                            (source.path.clone(), name.into()),
                            (*namespaces.iter().next().unwrap()).clone(),
                        );
                    }
                }
            }
        }
    }
    pub fn resolve_relative(&self, path: &str, specifier: &str) -> Option<String> {
        if !specifier.starts_with('.') {
            return self
                .module_bindings
                .get(specifier)
                .filter(|path| self.sources.iter().any(|s| &s.path == *path))
                .cloned();
        }
        let joined = Path::new(path).parent()?.join(specifier);
        let mut normalized = std::path::PathBuf::new();
        for part in joined.components() {
            match part {
                Component::ParentDir => {
                    if !normalized.pop() {
                        return None;
                    }
                }
                Component::CurDir => {}
                Component::Normal(name) => normalized.push(name),
                _ => return None,
            }
        }
        let base = normalized.to_string_lossy().replace('\\', "/");
        let mut choices = vec![base.clone()];
        for ext in ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"] {
            choices.push(format!("{base}.{ext}"));
            choices.push(format!("{base}/index.{ext}"));
        }
        if matches!(
            normalized.extension().and_then(|v| v.to_str()),
            Some("js" | "jsx")
        ) {
            for ext in ["ts", "tsx"] {
                choices.push(
                    normalized
                        .with_extension(ext)
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
        let available: BTreeSet<_> = choices
            .into_iter()
            .filter(|p| self.sources.iter().any(|s| &s.path == p))
            .collect();
        (available.len() == 1).then(|| available.into_iter().next().unwrap())
    }
}

pub(crate) fn parameter_names(source: &Source, params: Option<NodeId>) -> Vec<(String, String)> {
    let Some(params) = params else {
        return Vec::new();
    };
    if is_identifier(&source.nodes[params].kind) {
        return vec![(source.text(Some(params)).into(), String::new())];
    }
    let parts = if matches!(
        source.nodes[params].kind.as_str(),
        "parameter" | "parameter_declaration"
    ) {
        vec![params]
    } else {
        source.nodes[params].children.clone()
    };
    let mut result = Vec::new();
    for part in parts {
        if matches!(
            source.nodes[part].kind.as_str(),
            "comment" | "line_comment" | "type_parameter" | "self_parameter"
        ) {
            continue;
        }
        let pattern = source
            .child(part, &["pattern", "name", "declarator"])
            .or_else(|| is_identifier(&source.nodes[part].kind).then_some(part))
            .or_else(|| {
                source.first(
                    part,
                    &["identifier", "simple_identifier", "name", "variable_name"],
                )
            });
        if let Some(pattern) = pattern {
            let pattern = if matches!(source.language.as_str(), "c" | "cpp") {
                source.identifier(Some(pattern)).unwrap_or(pattern)
            } else {
                pattern
            };
            let name = source.text(Some(pattern));
            if name == "this" && matches!(source.language.as_str(), "typescript" | "javascript") {
                continue;
            }
            let name = if source.nodes[pattern].kind == "rest_pattern" {
                source.text(source.nodes[pattern].children.last().copied())
            } else {
                name
            };
            let ty = source.child(part, &["type"]).or_else(|| {
                source.nodes[part]
                    .children
                    .iter()
                    .copied()
                    .find(|id| source.nodes[*id].kind.contains("type"))
            });
            let mut type_text = source.text(ty).to_owned();
            if matches!(source.language.as_str(), "c" | "cpp")
                && source.text(Some(part)).contains('*')
            {
                type_text.push('*');
            }
            result.push((name.trim_start_matches('$').to_owned(), type_text));
        }
    }
    result
}
fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && !text.as_bytes()[0].is_ascii_digit()
}
fn scoped_name(source: &Source, id: NodeId, name: &str) -> String {
    if !matches!(source.language.as_str(), "typescript" | "javascript") {
        return name.into();
    }
    let mut names = vec![name.to_owned()];
    let mut ancestor = source.nodes[id].parent;
    while let Some(parent) = ancestor {
        if source.nodes[parent].kind == "internal_module" {
            names.push(source.text(source.child(parent, &["name"])).to_owned());
        }
        ancestor = source.nodes[parent].parent;
    }
    names.reverse();
    names.join(".")
}
