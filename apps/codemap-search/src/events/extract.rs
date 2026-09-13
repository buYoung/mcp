use super::{
    javascript::{self, ApiIdentity, JsGraph, Value, ValueKind},
    *,
};
use crate::callers::resolution::SourceResolver;
use crate::parser::{CodeRange, ExtractedFile};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Node, Point, Tree};

pub(super) struct Extraction {
    pub endpoints: Vec<EventEndpoint>,
    pub omitted: usize,
    pub parse_failures: usize,
}

struct Context<'a> {
    files: &'a [ExtractedFile],
    sources: &'a HashMap<String, String>,
    js: JsGraph<'a>,
    rust: SourceResolver<'a>,
    rust_trees: HashMap<String, Tree>,
    rules: Vec<EventRule>,
}

fn node_at<'a>(tree: &'a Tree, range: &CodeRange) -> Option<Node<'a>> {
    let mut node = tree.root_node().descendant_for_point_range(
        Point::new(
            range.start_line.saturating_sub(1),
            range.start_col.saturating_sub(1),
        ),
        Point::new(
            range.end_line.saturating_sub(1),
            range.end_col.saturating_sub(1),
        ),
    )?;
    while javascript::range(node) != *range {
        node = node.parent()?;
    }
    Some(node)
}
fn language(path: &str) -> Option<&'static str> {
    match Path::new(path).extension()?.to_str()? {
        "rs" => Some("rust"),
        "ts" | "tsx" | "mts" | "cts" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        _ => None,
    }
}
fn arguments(node: Node<'_>) -> Vec<Node<'_>> {
    let Some(args) = node.child_by_field_name("arguments") else {
        return Vec::new();
    };
    let mut cursor = args.walk();
    args.named_children(&mut cursor)
        .filter(|n| !n.kind().contains("comment"))
        .take(16)
        .collect()
}
fn enclosing(file: &ExtractedFile, range: &CodeRange) -> Option<String> {
    file.symbols
        .iter()
        .filter(|s| {
            s.kind == "fn"
                && s.range.start_line <= range.start_line
                && range.end_line_inclusive() <= s.range.end_line_inclusive()
        })
        .min_by_key(|s| s.range.end_line.saturating_sub(s.range.start_line))
        .map(|s| s.name.clone())
}
fn conditions(mut node: Node<'_>, source: &str) -> (Vec<String>, bool) {
    let mut result = Vec::new();
    let mut inactive = false;
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "if_statement"
                | "if_expression"
                | "conditional_expression"
                | "while_statement"
                | "while_expression"
                | "for_statement"
        ) {
            if let Some(condition) = parent.child_by_field_name("condition") {
                let condition_text = javascript::text(condition, source)
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .trim();
                let is_alternative =
                    parent
                        .child_by_field_name("alternative")
                        .is_some_and(|alternative| {
                            alternative.start_byte() <= node.start_byte()
                                && node.end_byte() <= alternative.end_byte()
                        });
                inactive |= condition_text == "false" && !is_alternative
                    || condition_text == "true" && is_alternative;
                if result.len() < 8 {
                    result.push(format!(
                        "{}{}",
                        if is_alternative {
                            "else of "
                        } else {
                            "condition: "
                        },
                        condition_text.chars().take(120).collect::<String>()
                    ));
                }
            }
        }
        node = parent;
    }
    (result, inactive)
}

impl<'a> Context<'a> {
    fn new(
        files: &'a [ExtractedFile],
        sources: &'a HashMap<String, String>,
        root: &'a Path,
    ) -> Self {
        let mut rust_trees = HashMap::new();
        for (path, source) in sources.iter().filter(|(path, _)| path.ends_with(".rs")) {
            let mut parser = tree_sitter::Parser::new();
            if parser
                .set_language(&tree_sitter_rust::LANGUAGE.into())
                .is_ok()
            {
                if let Ok(tree) = crate::parser::parse_source(&mut parser, source.as_bytes()) {
                    rust_trees.insert(path.clone(), tree);
                }
            }
        }
        Self {
            files,
            sources,
            js: JsGraph::new(sources),
            rust: SourceResolver::from_stored_sources(files, root, sources),
            rust_trees,
            rules: super::rules::effective_rules(&crate::config::get().event_navigation),
        }
    }
    fn rust_type(
        &self,
        file: &ExtractedFile,
        range: &CodeRange,
        name: &str,
    ) -> Option<ApiIdentity> {
        if let Some(target) = self.rust.resolve_name(file, range, name, "type") {
            return Some(ApiIdentity {
                module: target.file.file_path.clone(),
                symbol: target.symbol.name.clone(),
                location: Some(EventLocation {
                    file_path: target.file.file_path.clone(),
                    range: target.symbol.range.clone(),
                    name: Some(target.symbol.name.clone()),
                }),
                is_callable: false,
            });
        }
        let imports = &file.navigation.as_ref()?.imports;
        let qualified = if name.starts_with("tauri::") {
            Some(name.to_string())
        } else {
            let imports: Vec<_> = imports
                .iter()
                .filter(|i| {
                    i.local_name == name
                        && self.rust.condition_at(&file.file_path, &i.range) == Some(true)
                })
                .collect();
            if imports.len() == 1 {
                imports[0].source.clone()
            } else {
                None
            }
        }?;
        // A local module/type spelling never masquerades as the external Tauri API.
        if !qualified.starts_with("tauri::")
            || self
                .files
                .iter()
                .flat_map(|f| &f.symbols)
                .any(|s| s.kind == "mod" && s.name == "tauri")
        {
            return None;
        }
        Some(ApiIdentity {
            module: "tauri".into(),
            symbol: qualified.rsplit("::").next()?.to_string(),
            location: None,
            is_callable: false,
        })
    }
    fn eval_rust(&self, file: &ExtractedFile, node: Node<'_>, depth: usize) -> Value {
        if depth > 16 {
            return Value::unknown("Rust value resolution depth exceeded");
        }
        let Some(source) = self.sources.get(&file.file_path) else {
            return Value::unknown("indexed source unavailable");
        };
        if let Some(value) = javascript::static_string(node, source) {
            return Value {
                kind: ValueKind::String(value),
                evidence: vec![javascript::location(&file.file_path, node, None)],
            };
        }
        if matches!(
            node.kind(),
            "reference_expression" | "parenthesized_expression"
        ) {
            return node
                .child_by_field_name("value")
                .or_else(|| node.named_child(0))
                .map_or_else(
                    || Value::unknown("wrapped Rust value unavailable"),
                    |node| self.eval_rust(file, node, depth + 1),
                );
        }
        if matches!(node.kind(), "closure_expression") {
            return Value {
                kind: ValueKind::Definition(ApiIdentity {
                    module: file.file_path.clone(),
                    symbol: "<inline handler>".into(),
                    location: Some(javascript::location(
                        &file.file_path,
                        node,
                        Some("<inline handler>".into()),
                    )),
                    is_callable: true,
                }),
                evidence: Vec::new(),
            };
        }
        if matches!(node.kind(), "identifier" | "scoped_identifier") {
            let range = javascript::range(node);
            let name = javascript::text(node, source);
            if let Some(target) = self.rust.resolve_name(file, &range, name, "const") {
                let Some(tree) = self.rust_trees.get(&target.file.file_path) else {
                    return Value::unknown("constant source unavailable");
                };
                if let Some(value) =
                    node_at(tree, &target.symbol.range).and_then(|n| n.child_by_field_name("value"))
                {
                    let mut result = self.eval_rust(target.file, value, depth + 1);
                    result.evidence.push(EventLocation {
                        file_path: target.file.file_path.clone(),
                        range: target.symbol.range.clone(),
                        name: Some(target.symbol.name.clone()),
                    });
                    return result;
                }
            }
            if let Some(target) = self.rust.resolve_name(file, &range, name, "call") {
                let location = EventLocation {
                    file_path: target.file.file_path.clone(),
                    range: target.symbol.range.clone(),
                    name: Some(target.symbol.name.clone()),
                };
                return Value {
                    kind: ValueKind::Definition(ApiIdentity {
                        module: target.file.file_path.clone(),
                        symbol: target.symbol.name.clone(),
                        location: Some(location.clone()),
                        is_callable: true,
                    }),
                    evidence: vec![location],
                };
            }
        }
        Value::unknown("Rust value is not a literal, static constant or resolved handler")
    }
    fn eval(&self, file: &ExtractedFile, node: Node<'_>) -> Value {
        if file.file_path.ends_with(".rs") {
            self.eval_rust(file, node, 0)
        } else {
            self.js.eval(&file.file_path, node, 0)
        }
    }
    fn api(
        &self,
        file: &ExtractedFile,
        call: Node<'_>,
        function: Node<'_>,
        method: Option<&str>,
        receiver: Option<Node<'_>>,
    ) -> Option<Value> {
        if !file.file_path.ends_with(".rs") {
            return Some(self.eval(file, receiver.unwrap_or(function)));
        }
        let site = file
            .navigation
            .as_ref()?
            .calls
            .iter()
            .find(|site| site.range == javascript::range(call))?;
        if let Some(receiver) = receiver {
            let source = self.sources.get(&file.file_path)?;
            let typ =
                self.rust
                    .declared_receiver_type(file, site, javascript::text(receiver, source))?;
            let identity = self.rust_type(file, &site.range, &typ)?;
            if identity.module == "tauri" {
                let needed = if matches!(method, Some("listen" | "once")) {
                    "tauri::Listener"
                } else {
                    "tauri::Emitter"
                };
                if !file.navigation.as_ref()?.imports.iter().any(|i| {
                    i.source.as_deref() == Some(needed)
                        && self.rust.condition_at(&file.file_path, &i.range) == Some(true)
                }) {
                    return None;
                }
            }
            Some(Value {
                kind: ValueKind::Opaque(
                    Some(identity),
                    "receiver type is known but its application/bus instance is opaque".into(),
                ),
                evidence: vec![javascript::location(&file.file_path, receiver, None)],
            })
        } else {
            let target = self.rust.resolve_call(file, site)?;
            let location = EventLocation {
                file_path: target.file.file_path.clone(),
                range: target.symbol.range.clone(),
                name: Some(target.symbol.name.clone()),
            };
            Some(Value {
                kind: ValueKind::Definition(ApiIdentity {
                    module: target.file.file_path.clone(),
                    symbol: target.symbol.name.clone(),
                    location: Some(location.clone()),
                    is_callable: true,
                }),
                evidence: vec![location],
            })
        }
    }
    fn framework_bus(&self, path: &str) -> Option<EventBusEvidence> {
        let mut directory = Path::new(path).parent();
        while let Some(dir) = directory {
            for candidate in [
                dir.join("src-tauri/tauri.conf.json"),
                dir.join("package.json"),
            ] {
                let candidate = candidate.to_string_lossy().replace('\\', "/");
                if self.sources.contains_key(&candidate) {
                    return Some(EventBusEvidence {identity:format!("framework:tauri:{candidate}"),description:"Tauri event API in this indexed application boundary; runtime delivery is not guaranteed".into(),is_configured_assumption:false,locations:vec![EventLocation {file_path:candidate,range:CodeRange {start_line:1,start_col:1,end_line:1,end_col:1},name:Some("application boundary".into())}]});
                }
            }
            directory = dir.parent();
        }
        None
    }
    fn bus_value(&self, value: Value) -> Option<EventBusEvidence> {
        if let ValueKind::Bus(api, allocation) = value.kind {
            let mut locations = value.evidence;
            locations.push(allocation.clone());
            Some(EventBusEvidence {
                identity: format!(
                    "allocation:{}:{}:{}",
                    allocation.file_path, allocation.range.start_line, allocation.range.start_col
                ),
                description: format!(
                    "immutable module allocation of {}::{}",
                    api.module, api.symbol
                ),
                is_configured_assumption: false,
                locations,
            })
        } else {
            None
        }
    }
    fn endpoint(&self, file: &ExtractedFile, call: Node<'_>) -> Option<EventEndpoint> {
        self.rust.reset_dependency_tracking();
        let language = language(&file.file_path)?;
        let source = self.sources.get(&file.file_path)?;
        if call.has_error() || call.is_missing() {
            return None;
        }
        let mut function = call.child_by_field_name("function")?;
        while matches!(
            function.kind(),
            "generic_function" | "parenthesized_expression"
        ) {
            function = function
                .child_by_field_name("function")
                .or_else(|| function.named_child(0))?;
        }
        let (method, receiver) =
            if matches!(function.kind(), "member_expression" | "field_expression") {
                let member = function
                    .child_by_field_name("property")
                    .or_else(|| function.child_by_field_name("field"))?;
                (
                    Some(javascript::text(member, source)),
                    function
                        .child_by_field_name("object")
                        .or_else(|| function.child_by_field_name("value")),
                )
            } else {
                (None, None)
            };
        // Method spelling only narrows work; canonical API evidence is still required below.
        if !self
            .rules
            .iter()
            .any(|rule| rule.language == language && rule.method.as_deref() == method)
        {
            return None;
        }
        let api_value = self.api(file, call, function, method, receiver)?;
        let api = api_value.api()?;
        let matching: Vec<_> = self
            .rules
            .iter()
            .filter(|r| {
                r.language == language
                    && r.module.trim_start_matches("./") == api.module
                    && r.symbol == api.symbol
                    && r.method.as_deref() == method
            })
            .collect();
        if matching.len() != 1 {
            return None;
        }
        let rule = matching[0];
        let args = arguments(call);
        let mut unresolved = Vec::new();
        let mut dependencies = api_value.evidence.clone();
        if let Some(location) = &api.location {
            dependencies.push(location.clone());
        }
        let bus = match rule.bus {
            EventBusRule::Receiver => self.bus_value(api_value.clone()),
            EventBusRule::Argument => rule
                .bus_arg
                .and_then(|i| args.get(i))
                .and_then(|n| self.bus_value(self.eval(file, *n))),
            EventBusRule::Fixed => Some(EventBusEvidence {
                identity: format!("configured:{}", rule.bus_identity.as_deref()?),
                description: format!("shared bus declared by rule {}", rule.id),
                is_configured_assumption: true,
                locations: Vec::new(),
            }),
            EventBusRule::Framework => self.framework_bus(&file.file_path),
        };
        if bus.is_none() {
            unresolved.push(match &api_value.kind {
                ValueKind::Opaque(_, reason) => reason.clone(),
                _ => {
                    "bus identity is not a shared immutable allocation or explicit configured scope"
                        .into()
                }
            });
        }
        if let Some(bus) = &bus {
            dependencies.extend(bus.locations.clone());
        }
        let key_value = if let Some(key) = &rule.event_key {
            Value {
                kind: ValueKind::String(key.clone()),
                evidence: Vec::new(),
            }
        } else {
            rule.event_arg.and_then(|i| args.get(i)).map_or_else(
                || Value::unknown("event key argument missing"),
                |n| self.eval(file, *n),
            )
        };
        let event_key = match &key_value.kind {
            ValueKind::String(key)
                if !key.is_empty() && key.len() <= 256 && !key.contains(['\n', '\r', '\0']) =>
            {
                Some(key.clone())
            }
            _ => {
                unresolved
                    .push("event key is not a supported literal/static immutable value".into());
                None
            }
        };
        dependencies.extend(key_value.evidence.clone());
        let handler_value = rule
            .handler_arg
            .and_then(|i| args.get(i))
            .map(|n| self.eval(file, *n));
        let handler = handler_value.as_ref().and_then(|value| match &value.kind {
            ValueKind::Definition(api) if api.is_callable => api.location.clone(),
            _ => None,
        });
        if let Some(value) = handler_value {
            dependencies.extend(value.evidence);
        }
        if let Some(handler) = &handler {
            dependencies.push(handler.clone());
        }
        let handler_reason = (rule.role == EventRole::Subscribe && handler.is_none()).then(|| {
            "handler definition is dynamic, opaque or outside the indexed static subset".into()
        });
        let mut target = rule.target.clone().or_else(|| {
            Some(
                if api.module == "tauri" || api.module == "@tauri-apps/api/event" {
                    "any"
                } else {
                    "unqualified"
                }
                .to_string(),
            )
        });
        if let Some(index) = rule.target_arg {
            target = args.get(index).and_then(|n| {
                let value = self.eval(file, *n);
                dependencies.extend(value.evidence);
                match value.kind {
                    ValueKind::String(value) => Some(format!("label:{value}")),
                    _ => None,
                }
            });
        } else if api.module == "@tauri-apps/api/event"
            && matches!(api.symbol.as_str(), "listen" | "once")
            && args.len() > 2
        {
            // A target options object cannot silently become an unrestricted listener.
            target = None;
            let options = args[2];
            if options.kind() == "object" {
                let mut cursor = options.walk();
                let members: Vec<_> = options.named_children(&mut cursor).collect();
                if members.is_empty() {
                    target = Some("any".into());
                }
                if members.len() == 1
                    && members[0].kind() == "pair"
                    && members[0]
                        .child_by_field_name("key")
                        .is_some_and(|n| javascript::text(n, source) == "target")
                {
                    target = members[0].child_by_field_name("value").and_then(|n| {
                        let value = self.eval(file, n);
                        dependencies.extend(value.evidence);
                        match value.kind {
                            ValueKind::String(value) => Some(format!("label:{value}")),
                            _ => None,
                        }
                    });
                }
            }
        }
        target = target.filter(|value| value.len() <= 256 && !value.contains(['\n', '\r', '\0']));
        if target.is_none() {
            unresolved.push("target qualifier is dynamic or unsupported".into());
        }
        let (mut conditions, mut is_inactive) = conditions(call, source);
        if language == "rust" {
            match self
                .rust
                .condition_at(&file.file_path, &javascript::range(call))
            {
                Some(true) => {}
                Some(false) => {
                    is_inactive = true;
                    conditions.push("inactive under the explicit Rust analysis target".into());
                }
                None => unresolved
                    .push("Rust cfg/module activation is unknown under the analysis target".into()),
            }
            let tracked = self.rust.source_dependencies();
            if tracked.len() > 64 {
                unresolved.push("source dependency proof reached its budget".into());
            }
            dependencies.extend(tracked.into_iter().take(64).map(|(file_path, range)| {
                EventLocation {
                    file_path,
                    range,
                    name: None,
                }
            }));
        }
        dependencies.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.range.start_line.cmp(&b.range.start_line))
                .then(a.range.start_col.cmp(&b.range.start_col))
        });
        dependencies.dedup_by(|a, b| a.file_path == b.file_path && a.range == b.range);
        Some(EventEndpoint {
            role: rule.role,
            location: javascript::location(&file.file_path, call, None),
            enclosing_symbol: enclosing(file, &javascript::range(call)),
            api_rule: rule.id.clone(),
            api_identity: format!(
                "{}::{}{}",
                api.module,
                api.symbol,
                method.map(|m| format!(".{m}")).unwrap_or_default()
            ),
            is_api_configured: !rule.is_builtin,
            api_definition: api.location.clone(),
            bus,
            event_key,
            is_key_configured: rule.event_key.is_some(),
            key_evidence: key_value.evidence,
            handler,
            handler_reason,
            target,
            is_target_configured: rule.target.is_some(),
            channel: rule.channel.clone().unwrap_or_else(|| "default".into()),
            conditions,
            unresolved_reasons: unresolved,
            is_once: rule.is_once,
            is_inactive,
            dependencies,
        })
    }
}

pub(super) fn extract(
    files: &[ExtractedFile],
    sources: &HashMap<String, String>,
    root: &Path,
) -> Extraction {
    let context = Context::new(files, sources, root);
    let mut result = Extraction {
        endpoints: Vec::new(),
        omitted: 0,
        parse_failures: 0,
    };
    for file in files {
        let Some(_) = language(&file.file_path) else {
            continue;
        };
        if !sources.contains_key(&file.file_path) {
            continue;
        }
        let tree = if file.file_path.ends_with(".rs") {
            context.rust_trees.get(&file.file_path)
        } else {
            context.js.files.get(&file.file_path).map(|f| &f.tree)
        };
        let Some(tree) = tree else {
            result.parse_failures += 1;
            continue;
        };
        let mut stack = vec![tree.root_node()];
        let mut count = 0;
        let mut inspected = 0;
        while let Some(node) = stack.pop() {
            if node.is_error() || node.is_missing() {
                continue;
            }
            let mut cursor = node.walk();
            let children: Vec<_> = node.named_children(&mut cursor).collect();
            stack.extend(children.into_iter().rev());
            if node.kind() != "call_expression" {
                continue;
            }
            inspected += 1;
            if inspected > 4096 {
                result.omitted += 1;
                continue;
            }
            if let Some(endpoint) = context.endpoint(file, node) {
                if count < super::ENDPOINTS_PER_FILE
                    && result.endpoints.len() < super::ENDPOINTS_PER_SNAPSHOT
                    && serde_json::to_vec(&endpoint).is_ok_and(|bytes| bytes.len() <= 8192)
                {
                    result.endpoints.push(endpoint);
                    count += 1;
                } else {
                    result.omitted += 1;
                }
            }
        }
    }
    result
}
