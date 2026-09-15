//! Source-scoped Rust/Go lookup. A unique repository-wide spelling is not a target.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use tree_sitter::{Node, Point};

#[cfg(test)]
use crate::parser::CodeExtractor;
use crate::parser::{CallSite, CodeRange, ExtractedFile, ExtractedSymbol, ImportKind};

use super::source::SourceSyntax;

mod local;
mod rust_cfg;
mod rust_files;
use rust_cfg::Condition;

#[derive(Clone, Copy)]
pub(crate) struct Target<'a> {
    pub file: &'a ExtractedFile,
    pub symbol: &'a ExtractedSymbol,
    pub is_precise: bool,
}

struct Source {
    text: String,
    syntax: SourceSyntax,
    extracted: ExtractedFile,
}

pub(crate) struct SourceResolver<'a> {
    files: &'a [ExtractedFile],
    files_by_path: HashMap<&'a str, &'a ExtractedFile>,
    root: &'a Path,
    names: HashMap<&'a str, Vec<Target<'a>>>,
    sources: RefCell<HashMap<String, Option<Rc<Source>>>>,
    manifests: RefCell<HashMap<PathBuf, Option<toml::Value>>>,
    test_filter: super::test_code::TestCodeFilter,
    target_os: Option<String>,
    file_conditions: RefCell<HashMap<String, Condition>>,
    stored_sources: Option<&'a HashMap<String, String>>,
    dependency_ranges: RefCell<Vec<(String, CodeRange)>>,
}

pub(crate) fn supports(path: &str) -> bool {
    supports_modules(path) || local::supports(path)
}

fn supports_modules(path: &str) -> bool {
    matches!(
        Path::new(path).extension().and_then(|ext| ext.to_str()),
        Some("rs" | "go")
    )
}

fn node_at<'a>(source: &'a Source, range: &CodeRange) -> Option<Node<'a>> {
    let start = Point::new(
        range.start_line.checked_sub(1)?,
        range.start_col.checked_sub(1)?,
    );
    let end = Point::new(
        range.end_line.checked_sub(1)?,
        range.end_col.checked_sub(1)?,
    );
    let mut node = source
        .syntax
        .tree_for_range(range)?
        .root_node()
        .descendant_for_point_range(start, end)?;
    while !crate::parser::node_matches_source_range(node, source.text.as_bytes(), range) {
        node = node.parent()?;
    }
    Some(node)
}

fn contains(outer: Node, inner: Node) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

fn text<'a>(node: Node, source: &'a Source) -> Option<&'a str> {
    node.utf8_text(source.text.as_bytes()).ok()
}

fn scope(mut node: Node) -> Node {
    while let Some(parent) = node.parent() {
        node = parent;
        if matches!(node.kind(), "source_file" | "block" | "declaration_list") {
            break;
        }
    }
    node
}

fn module_names(mut node: Node, source: &Source) -> Vec<String> {
    let mut names = Vec::new();
    while let Some(parent) = node.parent() {
        node = parent;
        if node.kind() == "mod_item" {
            if let Some(name) = node
                .child_by_field_name("name")
                .and_then(|name| text(name, source))
            {
                names.push(name.to_string());
            }
        }
    }
    names.reverse();
    names
}

fn node_range(node: Node) -> CodeRange {
    CodeRange {
        start_line: node.start_position().row + 1,
        start_col: node.start_position().column + 1,
        end_line: node.end_position().row + 1,
        end_col: node.end_position().column + 1,
    }
}

fn module_body(mut node: Node) -> Node {
    while let Some(parent) = node.parent() {
        node = parent;
        if node.kind() == "mod_item" {
            return node.child_by_field_name("body").unwrap_or(node);
        }
    }
    node
}

fn lexical_scopes(mut node: Node) -> Vec<Node> {
    let mut scopes = Vec::new();
    loop {
        if matches!(node.kind(), "block" | "declaration_list" | "source_file") {
            scopes.push(node);
            if node
                .parent()
                .is_some_and(|parent| parent.kind() == "mod_item")
            {
                break;
            }
        }
        let Some(parent) = node.parent() else { break };
        node = parent;
    }
    scopes
}

fn pattern_has_name(node: Node, source: &Source, name: &str) -> bool {
    if matches!(node.kind(), "identifier" | "type_identifier" | "self")
        && text(node, source) == Some(name)
    {
        return true;
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| pattern_has_name(child, source, name));
    found
}

fn parameter_has_name(parameter: Node, source: &Source, name: &str) -> bool {
    let mut cursor = parameter.walk();
    matches!(parameter.kind(), "identifier" | "self") && text(parameter, source) == Some(name)
        || parameter
            .child_by_field_name("pattern")
            .is_some_and(|pattern| pattern_has_name(pattern, source, name))
        || parameter
            .children_by_field_name("name", &mut cursor)
            .any(|pattern| pattern_has_name(pattern, source, name))
}

fn binding_scope(mut node: Node) -> Node {
    while let Some(parent) = node.parent() {
        node = parent;
        match node.kind() {
            "for_expression" | "while_expression" => {
                return node.child_by_field_name("body").unwrap_or(node)
            }
            "if_expression" => return node.child_by_field_name("consequence").unwrap_or(node),
            "block" | "source_file" | "match_arm" => return node,
            _ => {}
        }
    }
    node
}

fn has_generic_parameter(mut node: Node, source: &Source, name: &str) -> bool {
    loop {
        if let Some(parameters) = node.child_by_field_name("type_parameters") {
            let mut cursor = parameters.walk();
            if parameters
                .named_children(&mut cursor)
                .any(|parameter| parameter_has_name(parameter, source, name))
            {
                return true;
            }
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        node = parent;
    }
}

fn unique<'a>(targets: Vec<Target<'a>>) -> Option<Target<'a>> {
    let mut seen = HashSet::new();
    let targets: Vec<_> = targets
        .into_iter()
        .filter(|target| {
            seen.insert((
                &target.file.file_path,
                target.symbol.range.start_line,
                target.symbol.range.start_col,
            ))
        })
        .collect();
    (targets.len() == 1).then(|| targets[0])
}

impl<'a> SourceResolver<'a> {
    pub(crate) fn new(files: &'a [ExtractedFile], root: &'a Path) -> Self {
        let mut names: HashMap<_, Vec<_>> = HashMap::new();
        for file in files
            .iter()
            .filter(|file| supports_modules(&file.file_path))
        {
            for symbol in &file.symbols {
                names.entry(symbol.name.as_str()).or_default().push(Target {
                    file,
                    symbol,
                    is_precise: true,
                });
            }
        }
        Self {
            files,
            files_by_path: files
                .iter()
                .filter(|file| supports(&file.file_path))
                .map(|file| (file.file_path.as_str(), file))
                .collect(),
            root,
            names,
            sources: RefCell::new(HashMap::new()),
            manifests: RefCell::new(HashMap::new()),
            test_filter: super::test_code::TestCodeFilter::from_config(root),
            target_os: crate::config::get().analysis_target_os.clone(),
            file_conditions: RefCell::new(HashMap::new()),
            stored_sources: None,
            dependency_ranges: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn from_stored_sources(
        files: &'a [ExtractedFile],
        root: &'a Path,
        sources: &'a HashMap<String, String>,
    ) -> Self {
        let mut resolver = Self::new(files, root);
        resolver.stored_sources = Some(sources);
        resolver.test_filter = super::test_code::TestCodeFilter::including_tests(root);
        resolver
    }

    fn read_source(&self, file: &str) -> Option<String> {
        match self.stored_sources {
            Some(sources) => sources.get(file).cloned(),
            None => super::read_workspace_file(file, self.root),
        }
    }

    pub(crate) fn reset_dependency_tracking(&self) {
        self.dependency_ranges.borrow_mut().clear();
    }
    pub(crate) fn source_dependencies(&self) -> Vec<(String, CodeRange)> {
        self.dependency_ranges.borrow().clone()
    }
    fn record_dependency(&self, file: &str, range: &CodeRange) {
        if !self
            .stored_sources
            .is_some_and(|sources| sources.contains_key(file))
        {
            return;
        }
        let mut dependencies = self.dependency_ranges.borrow_mut();
        if dependencies.len() < 65
            && !dependencies
                .iter()
                .any(|(path, existing)| path == file && existing == range)
        {
            dependencies.push((file.to_string(), range.clone()));
        }
    }

    /// Borrow the already parsed Rust source for additional source-grounded facts.
    pub(crate) fn inspect_rust<T>(
        &self,
        path: &str,
        inspect: impl FnOnce(&str, &tree_sitter::Tree) -> T,
    ) -> Option<T> {
        if !path.ends_with(".rs") {
            return None;
        }
        let source = self.source(path)?;
        Some(inspect(&source.text, &source.syntax.tree))
    }

    pub(crate) fn condition_at(&self, file: &str, range: &CodeRange) -> Option<bool> {
        self.record_dependency(file, range);
        let source = self.source(file)?;
        let node = node_at(&source, range)?;
        match self.rust_node_condition(file, &source, node) {
            Condition::True => Some(true),
            Condition::False => Some(false),
            Condition::Unknown => None,
        }
    }

    pub(crate) fn declared_receiver_type(
        &self,
        file: &ExtractedFile,
        call: &CallSite,
        receiver: &str,
    ) -> Option<String> {
        let source = self.source(&file.file_path)?;
        let node = node_at(&source, &call.range)?;
        self.local_binding(file, node, receiver)?
            .as_type()
            .and_then(|(name, is_explicit)| is_explicit.then_some(name))
    }

    fn source(&self, file: &str) -> Option<Rc<Source>> {
        self.record_dependency(
            file,
            &CodeRange {
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 1,
            },
        );
        if let Some(source) = self.sources.borrow().get(file) {
            return source.clone();
        }
        let source = self.read_source(file).and_then(|text| {
            let mut bytes = text.into_bytes();
            self.test_filter.mask_source(file, &mut bytes);
            let text = String::from_utf8(bytes).ok()?;
            let extracted = crate::parser::TreeSitterExtractor::new()
                .extract_for_resolution(&text, file)
                .ok()?;
            SourceSyntax::parse(file, text.as_bytes()).map(|syntax| {
                Rc::new(Source {
                    text,
                    syntax,
                    extracted,
                })
            })
        });
        self.sources
            .borrow_mut()
            .insert(file.to_string(), source.clone());
        source
    }

    fn file(&self, path: &Path) -> Option<&'a ExtractedFile> {
        let key = crate::workspace::workspace_relative_key(path, self.root);
        self.files_by_path.get(key.as_str()).copied()
    }

    fn nearest(&self, path: &str, name: &str) -> Option<PathBuf> {
        let mut directory = self.root.join(path).parent()?.to_path_buf();
        loop {
            let candidate = directory.join(name);
            let exists = self.stored_sources.map_or_else(
                || candidate.is_file(),
                |sources| {
                    sources.contains_key(&crate::workspace::workspace_relative_key(
                        &candidate, self.root,
                    ))
                },
            );
            if exists {
                return Some(directory.join(name));
            }
            if directory == self.root || !directory.starts_with(self.root) || !directory.pop() {
                return None;
            }
        }
    }

    fn manifest(&self, path: &Path) -> Option<toml::Value> {
        self.record_dependency(
            &crate::workspace::workspace_relative_key(path, self.root),
            &CodeRange {
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 1,
            },
        );
        if let Some(value) = self.manifests.borrow().get(path) {
            return value.clone();
        }
        let content = match self.stored_sources {
            Some(sources) => sources
                .get(&crate::workspace::workspace_relative_key(path, self.root))
                .cloned(),
            None => std::fs::read_to_string(path).ok(),
        };
        let value = content.and_then(|text| toml::from_str(&text).ok());
        self.manifests
            .borrow_mut()
            .insert(path.to_path_buf(), value.clone());
        value
    }

    fn rust_root(&self, file: &str) -> Option<PathBuf> {
        if let Some(manifest) = self.nearest(file, "Cargo.toml") {
            let directory = manifest.parent()?;
            let source_path = self.root.join(file);
            if source_path == directory.join("src/main.rs") {
                return Some(source_path);
            }
            if let Some(path) = self
                .manifest(&manifest)
                .and_then(|value| value.get("lib")?.get("path")?.as_str().map(str::to_string))
            {
                return Some(directory.join(path));
            }
            for name in ["src/lib.rs", "src/main.rs"] {
                let candidate = directory.join(name);
                if self.file(&candidate).is_some() {
                    if !source_path.starts_with(directory.join("src"))
                        || source_path.starts_with(directory.join("src/bin"))
                    {
                        return Some(source_path);
                    }
                    return Some(candidate);
                }
            }
        }
        let directory = self.root.join(file).parent()?.to_path_buf();
        for name in ["lib.rs", "main.rs", "mod.rs"] {
            let candidate = directory.join(name);
            if self.file(&candidate).is_some() {
                return Some(candidate);
            }
        }
        Some(self.root.join(file))
    }

    fn rust_dependency(&self, file: &str, name: &str) -> Option<PathBuf> {
        let manifest = self.nearest(file, "Cargo.toml")?;
        let value = self.manifest(&manifest)?;
        let dependency = ["dependencies", "dev-dependencies"]
            .iter()
            .find_map(|section| {
                value
                    .get(section)?
                    .as_table()?
                    .iter()
                    .find(|(key, _)| key.replace('-', "_") == name)
                    .map(|(_, value)| value.clone())
            })?;
        let (directory, dependency) = if dependency.get("workspace").and_then(toml::Value::as_bool)
            == Some(true)
        {
            let mut directory = manifest.parent()?.to_path_buf();
            loop {
                let candidate = directory.join("Cargo.toml");
                if let Some(dependency) = self.manifest(&candidate).and_then(|value| {
                    value
                        .get("workspace")?
                        .get("dependencies")?
                        .as_table()?
                        .iter()
                        .find(|(key, _)| key.replace('-', "_") == name)
                        .map(|(_, value)| value.clone())
                }) {
                    break (directory, dependency);
                }
                if directory == self.root || !directory.pop() || !directory.starts_with(self.root) {
                    return None;
                }
            }
        } else {
            (manifest.parent()?.to_path_buf(), dependency)
        };
        let path = directory.join(dependency.get("path")?.as_str()?);
        let path = crate::workspace::canonicalize_path_lenient(&path);
        if !path.starts_with(self.root) {
            return None;
        }
        let lib = self
            .manifest(&path.join("Cargo.toml"))
            .and_then(|value| value.get("lib")?.get("path")?.as_str().map(str::to_string))
            .unwrap_or_else(|| "src/lib.rs".to_string());
        Some(path.join(lib))
    }

    fn go_package(&self, file: &str) -> Option<String> {
        let source = self.source(file)?;
        let root = source.syntax.tree.root_node();
        let mut cursor = root.walk();
        let package = root
            .named_children(&mut cursor)
            .find(|node| node.kind() == "package_clause")
            .and_then(|node| node.named_child(0))
            .and_then(|node| text(node, &source).map(str::to_string));
        package
    }

    fn go_import_directory(&self, file: &str, import: &str) -> Option<PathBuf> {
        let manifest = self.nearest(file, "go.mod")?;
        let content = std::fs::read_to_string(&manifest).ok()?;
        let module = content
            .lines()
            .find_map(|line| line.trim().strip_prefix("module "))?
            .trim();
        if let Some(suffix) = import
            .strip_prefix(module)
            .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
        {
            return Some(manifest.parent()?.join(suffix.trim_start_matches('/')));
        }
        // Workspace replacements are explicit source mappings, unlike matching a path suffix.
        let mut directory = manifest.parent()?.to_path_buf();
        loop {
            if let Ok(content) = std::fs::read_to_string(directory.join("go.mod")) {
                for line in content.lines() {
                    let line = line.trim().strip_prefix("replace ").unwrap_or(line.trim());
                    if let Some((module, replacement)) = line.split_once("=>") {
                        let module = module.split_whitespace().next()?;
                        let replacement = replacement.split_whitespace().next()?;
                        if replacement.starts_with('.') {
                            if let Some(suffix) = import
                                .strip_prefix(module)
                                .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
                            {
                                let path = crate::workspace::canonicalize_path_lenient(
                                    &directory
                                        .join(replacement)
                                        .join(suffix.trim_start_matches('/')),
                                );
                                if path.starts_with(self.root) {
                                    return Some(path);
                                }
                            }
                        }
                    }
                }
            }
            if directory == self.root || !directory.pop() || !directory.starts_with(self.root) {
                return None;
            }
        }
    }

    fn candidates(&self, name: &str, kind: &str) -> Vec<Target<'a>> {
        self.names
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|target| supports_modules(&target.file.file_path))
            .filter(|target| match kind {
                "call" => target.symbol.kind == "fn" && target.symbol.owner.is_none(),
                "type" => matches!(
                    target.symbol.kind.as_str(),
                    "struct" | "enum" | "type" | "trait" | "interface" | "union"
                ),
                "const" => matches!(target.symbol.kind.as_str(), "const" | "static"),
                "macro" => target.symbol.kind == "fn" && target.symbol.owner.is_none(),
                _ => false,
            })
            .collect()
    }

    fn is_current_target(&self, target: Target<'_>) -> bool {
        let Some(source) = self.source(&target.file.file_path) else {
            return false;
        };
        let Some(mut node) = node_at(&source, &target.symbol.range) else {
            return false;
        };
        if self.rust_node_condition(&target.file.file_path, &source, node) != Condition::True {
            return false;
        }
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
            .and_then(|name| text(name, &source))
            != Some(&target.symbol.name)
        {
            return false;
        }
        match target.symbol.kind.as_str() {
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

    pub(crate) fn resolve_name(
        &self,
        file: &ExtractedFile,
        range: &CodeRange,
        name: &str,
        kind: &str,
    ) -> Option<Target<'a>> {
        if !supports_modules(&file.file_path) {
            return None;
        }
        let file = self
            .files
            .iter()
            .find(|candidate| candidate.file_path == file.file_path)?;
        self.lookup(file, range, name, kind, 0)
    }

    pub(crate) fn calls(&self, file: &ExtractedFile) -> Vec<CallSite> {
        self.source(&file.file_path)
            .and_then(|source| {
                source.extracted.navigation.as_ref().map(|navigation| {
                    navigation
                        .calls
                        .iter()
                        .filter(|call| {
                            node_at(&source, &call.range).is_some_and(|node| {
                                self.rust_node_condition(&file.file_path, &source, node)
                                    != Condition::False
                            })
                        })
                        .cloned()
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    pub(super) fn caller_name(&self, file: &ExtractedFile, line: usize) -> Option<String> {
        let source = self.source(&file.file_path)?;
        super::symbols::enclosing_fn(&source.extracted, line)
            .map(|symbol| super::qualified_name(symbol, &file.file_path))
    }

    pub(super) fn matches_caller(
        &self,
        file: &ExtractedFile,
        name: &str,
        line: usize,
        target_file: &str,
        target: &ExtractedSymbol,
    ) -> bool {
        self.calls(file)
            .iter()
            .filter(|call| {
                call.name == name && call.range.start_line <= line && line <= call.range.end_line
            })
            .any(|call| {
                self.resolve_call(file, call).is_some_and(|resolved| {
                    resolved.file.file_path == target_file
                        && resolved.symbol.range == target.range
                        && resolved.symbol.name == target.name
                })
            })
    }

    fn lookup(
        &self,
        file: &'a ExtractedFile,
        range: &CodeRange,
        path: &str,
        kind: &str,
        depth: usize,
    ) -> Option<Target<'a>> {
        self.record_dependency(&file.file_path, range);
        if depth > 16 {
            return None;
        }
        let source = self.source(&file.file_path)?;
        let at = node_at(&source, range)?;
        if self.rust_node_condition(&file.file_path, &source, at) != Condition::True {
            return None;
        }
        if has_generic_parameter(at, &source, path.split([':', '.']).next()?) {
            return None;
        }
        if file.file_path.ends_with(".go") {
            return self.go_lookup(file, range, path, kind, depth);
        }
        self.rust_lookup(file, range, path, kind, depth)
    }

    fn go_lookup(
        &self,
        file: &'a ExtractedFile,
        range: &CodeRange,
        path: &str,
        kind: &str,
        depth: usize,
    ) -> Option<Target<'a>> {
        let source = self.source(&file.file_path)?;
        let (directory, name, imported) = if let Some((alias, name)) = path.split_once('.') {
            if name.contains('.') {
                return None;
            }
            let imports = &source.extracted.navigation.as_ref()?.imports;
            let import = imports.iter().find(|import| import.local_name == alias)?;
            (
                self.go_import_directory(&file.file_path, import.source.as_deref()?)?,
                name,
                true,
            )
        } else {
            if source
                .extracted
                .navigation
                .as_ref()
                .is_some_and(|navigation| {
                    navigation
                        .imports
                        .iter()
                        .any(|import| import.local_name == path)
                })
            {
                // Imports belong to the file block and shadow package-level declarations.
                return None;
            }
            (
                self.root.join(&file.file_path).parent()?.to_path_buf(),
                path,
                false,
            )
        };
        let package = if imported {
            let packages: HashSet<_> = self
                .files
                .iter()
                .filter(|file| {
                    file.file_path.ends_with(".go")
                        && !file.file_path.ends_with("_test.go")
                        && self.root.join(&file.file_path).parent() == Some(directory.as_path())
                })
                .filter_map(|file| self.go_package(&file.file_path))
                .collect();
            if packages.len() != 1 {
                return None;
            }
            packages.into_iter().next()
        } else {
            self.go_package(&file.file_path)
        };
        let candidates: Vec<_> = self
            .candidates(name, kind)
            .into_iter()
            .filter(|target| {
                target.file.file_path.ends_with(".go")
                    && self.root.join(&target.file.file_path).parent() == Some(directory.as_path())
                    && self.go_package(&target.file.file_path) == package
                    && (!imported || target.symbol.flags.is_exported)
                    && self.is_current_target(*target)
            })
            .collect();
        let at = node_at(&source, range)?;
        let mut visible = Vec::new();
        let mut nearest_scope = usize::MAX;
        for target in candidates {
            let target_source = self.source(&target.file.file_path)?;
            let declaration = node_at(&target_source, &target.symbol.range)?;
            let lexical = scope(declaration);
            let scope_size = if lexical.kind() == "source_file" {
                usize::MAX
            } else if !imported
                && target.file.file_path == file.file_path
                && contains(lexical, at)
                && declaration.end_byte() <= at.start_byte()
            {
                lexical.end_byte() - lexical.start_byte()
            } else {
                continue;
            };
            if scope_size < nearest_scope {
                visible.clear();
                nearest_scope = scope_size;
            }
            if scope_size == nearest_scope {
                visible.push(target);
            }
        }
        let _ = depth;
        unique(visible)
    }

    fn rust_lookup(
        &self,
        file: &'a ExtractedFile,
        range: &CodeRange,
        path: &str,
        kind: &str,
        depth: usize,
    ) -> Option<Target<'a>> {
        let source = self.source(&file.file_path)?;
        let at = node_at(&source, range)?;
        if let Some((head, tail)) = path.split_once("::") {
            if head == "crate" {
                return self.lookup_at_root(
                    self.file(&self.rust_root(&file.file_path)?)?,
                    tail,
                    kind,
                    depth + 1,
                );
            }
            if head == "self" {
                return self.lookup(file, &node_range(module_body(at)), tail, kind, depth + 1);
            }
            if head == "super" {
                if !module_names(at, &source).is_empty() {
                    return self.lookup(
                        file,
                        &node_range(module_body(module_body(at).parent()?)),
                        tail,
                        kind,
                        depth + 1,
                    );
                }
                let current = self.root.join(&file.file_path);
                let parent = if current.file_name()?.to_str()? == "mod.rs" {
                    current.parent()?.parent()?
                } else {
                    current.parent()?
                };
                let destination = self
                    .module_file(parent)
                    .or_else(|| self.rust_root(&file.file_path))?;
                return self.lookup_at_root(self.file(&destination)?, tail, kind, depth + 1);
            }
            for lexical in lexical_scopes(at) {
                if self.has_unknown_rust_binding(&file.file_path, &source, lexical, head) {
                    return None;
                }
                let imports = self.imports_in_scope(&source, lexical, Some(head));
                if imports.len() > 1
                    || imports.iter().any(|import| {
                        self.rust_import_condition(&source, &import.range) != Condition::True
                    })
                {
                    return None;
                }
                let mut cursor = lexical.walk();
                let modules: Vec<_> = lexical
                    .named_children(&mut cursor)
                    .filter(|node| {
                        node.kind() == "mod_item"
                            && self.rust_node_condition(&file.file_path, &source, *node)
                                != Condition::False
                            && node
                                .child_by_field_name("name")
                                .and_then(|name| text(name, &source))
                                == Some(head)
                    })
                    .collect();
                let types = self.candidates(head, "type").into_iter().any(|target| {
                    target.file.file_path == file.file_path
                        && node_at(&source, &target.symbol.range)
                            .is_some_and(|node| scope(node) == lexical)
                });
                if imports.is_empty() && modules.is_empty() && !types {
                    continue;
                }
                let mut targets: Vec<_> = imports
                    .iter()
                    .filter_map(|import| {
                        self.lookup(
                            file,
                            &import.range,
                            &format!("{}::{tail}", import.source.as_deref()?),
                            kind,
                            depth + 1,
                        )
                    })
                    .collect();
                for module in modules {
                    let target = if let Some(body) = module.child_by_field_name("body") {
                        self.lookup(file, &node_range(body), tail, kind, depth + 1)
                    } else {
                        self.module_destination(file, &source, module)
                            .and_then(|path| self.file(&path))
                            .and_then(|file| self.lookup_at_root(file, tail, kind, depth + 1))
                    };
                    targets.extend(target);
                }
                // A local type/import/module shadows a crate with the same spelling,
                // including when its target is absent from the filtered snapshot.
                return unique(targets);
            }
            let dependency = self.rust_dependency(&file.file_path, head)?;
            return self.lookup_at_root(self.file(&dependency)?, tail, kind, depth + 1);
        }
        for lexical in lexical_scopes(at) {
            if self.has_unknown_rust_binding(&file.file_path, &source, lexical, path) {
                return None;
            }
            let mut targets: Vec<_> = self
                .candidates(path, kind)
                .into_iter()
                .filter(|target| {
                    target.file.file_path == file.file_path && self.is_current_target(*target)
                })
                .filter(|target| {
                    node_at(&source, &target.symbol.range).is_some_and(|node| {
                        scope(node) == lexical
                            && ((kind == "macro") == (node.kind() == "macro_definition"))
                    })
                })
                .collect();
            let imports = self.imports_in_scope(&source, lexical, Some(path));
            if imports.len() > 1
                || imports.iter().any(|import| {
                    self.rust_import_condition(&source, &import.range) != Condition::True
                })
            {
                return None;
            }
            let has_local_name = !targets.is_empty() || !imports.is_empty();
            for import in imports {
                targets.extend(self.lookup(
                    file,
                    &import.range,
                    import.source.as_deref()?,
                    kind,
                    depth + 1,
                ));
            }
            if has_local_name {
                return unique(targets);
            }
            let globs = self.imports_in_scope(&source, lexical, None);
            if globs
                .iter()
                .any(|import| self.rust_import_condition(&source, &import.range) != Condition::True)
            {
                return None;
            }
            if !globs.is_empty() {
                return unique(
                    globs
                        .iter()
                        .filter_map(|import| {
                            self.lookup(
                                file,
                                &import.range,
                                &format!("{}::{path}", import.source.as_deref()?),
                                kind,
                                depth + 1,
                            )
                        })
                        .collect(),
                );
            }
        }
        None
    }

    fn imports_in_scope<'f>(
        &self,
        source: &'f Source,
        lexical: Node,
        name: Option<&str>,
    ) -> Vec<&'f crate::parser::ImportEntry> {
        source
            .extracted
            .navigation
            .as_ref()
            .into_iter()
            .flat_map(|navigation| &navigation.imports)
            .filter(|import| {
                if !name
                    .map(|name| import.local_name == name)
                    .unwrap_or(import.kind == ImportKind::Glob)
                {
                    return false;
                }
                let Some(mut node) = node_at(source, &import.range) else {
                    return false;
                };
                while node.kind() != "use_declaration" {
                    let Some(parent) = node.parent() else {
                        return false;
                    };
                    node = parent;
                }
                scope(node) == lexical
                    && rust_cfg::condition(node, &source.text, self.target_os.as_deref())
                        != Condition::False
            })
            .collect()
    }

    fn module_destination(
        &self,
        file: &ExtractedFile,
        source: &Source,
        module: Node,
    ) -> Option<PathBuf> {
        let mut directory = self.rust_module_directory(&file.file_path)?;
        for name in module_names(module, source) {
            directory.push(name);
        }
        let mut previous = module.prev_named_sibling();
        while let Some(attribute) = previous {
            if attribute.kind().contains("comment") {
                previous = attribute.prev_named_sibling();
                continue;
            }
            if attribute.kind() != "attribute_item" {
                break;
            }
            let attribute_text = text(attribute, source)?;
            if attribute_text.trim_start().starts_with("#[path") {
                let path = attribute_text.split('"').nth(1)?;
                if path.contains('\\') {
                    return None;
                }
                return Some(crate::workspace::canonicalize_path_lenient(
                    &directory.join(path),
                ));
            }
            previous = attribute.prev_named_sibling();
        }
        self.module_file(&directory.join(text(module.child_by_field_name("name")?, source)?))
    }

    fn rust_module_directory(&self, file: &str) -> Option<PathBuf> {
        let path = self.root.join(file);
        if self.rust_root(file).as_deref() == Some(path.as_path())
            || matches!(path.file_name()?.to_str()?, "lib.rs" | "main.rs" | "mod.rs")
        {
            Some(path.parent()?.to_path_buf())
        } else {
            Some(path.with_extension(""))
        }
    }

    fn module_file(&self, path: &Path) -> Option<PathBuf> {
        let candidates: Vec<_> = [path.with_extension("rs"), path.join("mod.rs")]
            .into_iter()
            .filter(|path| self.file(path).is_some())
            .collect();
        (candidates.len() == 1).then(|| candidates[0].clone())
    }

    fn lookup_at_root(
        &self,
        file: &'a ExtractedFile,
        path: &str,
        kind: &str,
        depth: usize,
    ) -> Option<Target<'a>> {
        let source = self.source(&file.file_path)?;
        let root = source.syntax.tree.root_node();
        self.lookup(
            file,
            &CodeRange {
                start_line: 1,
                start_col: 1,
                end_line: root.end_position().row + 1,
                end_col: root.end_position().column + 1,
            },
            path,
            kind,
            depth,
        )
    }

    pub(crate) fn resolve_call(&self, file: &ExtractedFile, call: &CallSite) -> Option<Target<'a>> {
        let file = self.files_by_path.get(file.file_path.as_str()).copied()?;
        if local::supports(&file.file_path) {
            return self.resolve_local_call(file, call);
        }
        let source = self.source(&file.file_path)?;
        let node = node_at(&source, &call.range)?;
        if self.rust_node_condition(&file.file_path, &source, node) != Condition::True {
            return None;
        }
        if node.kind() == "macro_invocation" {
            let path = node
                .child_by_field_name("macro")
                .and_then(|path| text(path, &source))?;
            return self.lookup(file, &call.range, path, "macro", 0);
        }
        if node.kind() != "call_expression" {
            return None;
        }
        let mut function = node.child_by_field_name("function")?;
        while matches!(
            function.kind(),
            "generic_function" | "parenthesized_expression"
        ) {
            function = function
                .child_by_field_name("function")
                .or_else(|| function.named_child(0))?;
        }
        if matches!(function.kind(), "identifier" | "scoped_identifier") {
            let path = text(function, &source)?;
            if self.local_binding(file, node, path).is_some() {
                return None;
            }
            if let Some(target) = self.lookup(file, &call.range, path, "call", 0) {
                return Some(target);
            }
            if let Some((owner, name)) = path.rsplit_once("::") {
                let owner = if owner == "Self" {
                    self.self_type(node, &source)?
                } else {
                    owner.to_string()
                };
                let owner = self.lookup(file, &call.range, &owner, "type", 0)?;
                return self.method(owner, name, true);
            }
            return None;
        }
        if !matches!(function.kind(), "field_expression" | "selector_expression") {
            return None;
        }
        if function
            .child_by_field_name("field")
            .and_then(|field| text(field, &source))
            != Some(&call.name)
        {
            return None;
        }
        let receiver = function
            .child_by_field_name("value")
            .or_else(|| function.child_by_field_name("operand"))?;
        let receiver_text = text(receiver, &source)?;
        if file.file_path.ends_with(".go")
            && self.local_binding(file, node, receiver_text).is_none()
        {
            if let Some(target) = self.lookup(
                file,
                &call.range,
                &format!("{receiver_text}.{}", call.name),
                "call",
                0,
            ) {
                return Some(target);
            }
        }
        let (typ, is_precise) = self.local_binding(file, node, receiver_text)?.as_type()?;
        if !is_precise {
            return None;
        }
        let owner = self.lookup(file, &call.range, &typ, "type", 0)?;
        self.method(owner, &call.name, is_precise)
    }

    fn method(&self, owner: Target<'a>, name: &str, is_precise: bool) -> Option<Target<'a>> {
        unique(
            self.names
                .get(name)
                .into_iter()
                .flatten()
                .copied()
                .filter(|target| {
                    target.symbol.kind == "fn"
                        && target.symbol.owner.as_deref() == Some(&owner.symbol.name)
                        && (target.file.file_path == owner.file.file_path
                            || owner.file.file_path.ends_with(".go")
                                && Path::new(&target.file.file_path).parent()
                                    == Path::new(&owner.file.file_path).parent()
                                && self.go_package(&target.file.file_path)
                                    == self.go_package(&owner.file.file_path))
                        && self.is_current_target(*target)
                        && self.is_inherent_method(*target, owner)
                })
                .map(|mut target| {
                    target.is_precise = is_precise;
                    target
                })
                .collect(),
        )
    }

    fn is_inherent_method(&self, target: Target<'a>, owner: Target<'a>) -> bool {
        let Some(source) = self.source(&target.file.file_path) else {
            return false;
        };
        let Some(mut node) = node_at(&source, &target.symbol.range) else {
            return false;
        };
        if target.file.file_path.ends_with(".go") {
            let typ = node
                .child_by_field_name("receiver")
                .and_then(|receiver| receiver.named_child(0))
                .and_then(|parameter| parameter.child_by_field_name("type"));
            return node.kind() == "method_declaration"
                && typ
                    .and_then(|typ| text(typ, &source))
                    .and_then(|typ| {
                        Binding {
                            typ: Some(typ.to_string()),
                            is_precise: true,
                        }
                        .as_type()
                    })
                    .and_then(|(typ, _)| {
                        self.resolve_name(target.file, &target.symbol.range, &typ, "type")
                    })
                    .is_some_and(|resolved| {
                        resolved.file.file_path == owner.file.file_path
                            && resolved.symbol.range == owner.symbol.range
                    });
        }
        while let Some(parent) = node.parent() {
            node = parent;
            if node.kind() == "impl_item" {
                if node.child_by_field_name("trait").is_some() {
                    return false;
                }
                let Some(typ) = node.child_by_field_name("type") else {
                    return false;
                };
                let Some(name) =
                    text(typ, &source).map(|text| text.split('<').next().unwrap_or(text).trim())
                else {
                    return false;
                };
                let range = CodeRange {
                    start_line: typ.start_position().row + 1,
                    start_col: typ.start_position().column + 1,
                    end_line: typ.end_position().row + 1,
                    end_col: typ.end_position().column + 1,
                };
                return self
                    .resolve_name(target.file, &range, name, "type")
                    .is_some_and(|resolved| {
                        resolved.file.file_path == owner.file.file_path
                            && resolved.symbol.range == owner.symbol.range
                    });
            }
        }
        false
    }

    fn self_type(&self, mut node: Node, source: &Source) -> Option<String> {
        while let Some(parent) = node.parent() {
            node = parent;
            if node.kind() == "impl_item" {
                return node
                    .child_by_field_name("type")
                    .and_then(|typ| text(typ, source))
                    .map(str::to_string);
            }
        }
        None
    }

    fn local_binding(&self, file: &ExtractedFile, call: Node, name: &str) -> Option<Binding> {
        let source = self.source(&file.file_path)?;
        if name == "self" && file.file_path.ends_with(".rs") {
            return self.self_type(call, &source).map(|typ| Binding {
                typ: Some(typ),
                is_precise: true,
            });
        }
        let mut bindings = Vec::new();
        if let Some(navigation) = &source.extracted.navigation {
            for binding in navigation
                .local_bindings
                .iter()
                .filter(|binding| binding.name == name)
            {
                let Some(node) = node_at(&source, &binding.range) else {
                    continue;
                };
                let lexical = binding_scope(node);
                if node.end_byte() <= call.start_byte() && contains(lexical, call) {
                    let explicit_type = node
                        .child_by_field_name("type")
                        .and_then(|typ| text(typ, &source))
                        .map(str::to_string);
                    let inferred = binding.type_name.clone().or_else(|| {
                        binding
                            .value_type
                            .clone()
                            .filter(|typ| typ.chars().next().is_some_and(char::is_uppercase))
                    });
                    bindings.push((
                        lexical.end_byte() - lexical.start_byte(),
                        node.end_byte(),
                        Binding {
                            typ: explicit_type.clone().or(inferred),
                            is_precise: explicit_type.is_some(),
                        },
                    ));
                }
            }
        }
        let mut ancestor = call.parent();
        while let Some(node) = ancestor {
            if matches!(
                node.kind(),
                "function_item"
                    | "function_declaration"
                    | "method_declaration"
                    | "closure_expression"
                    | "func_literal"
            ) {
                for field in ["parameters", "receiver"] {
                    if let Some(parameters) = node.child_by_field_name(field) {
                        let mut cursor = parameters.walk();
                        for parameter in parameters.named_children(&mut cursor) {
                            if parameter_has_name(parameter, &source, name) {
                                let typ = parameter
                                    .child_by_field_name("type")
                                    .and_then(|typ| text(typ, &source))
                                    .map(str::to_string);
                                let lexical = node.child_by_field_name("body").unwrap_or(node);
                                bindings.push((
                                    lexical.end_byte() - lexical.start_byte(),
                                    parameter.end_byte(),
                                    Binding {
                                        typ,
                                        is_precise: true,
                                    },
                                ));
                            }
                        }
                    }
                }
            }
            ancestor = node.parent();
        }
        bindings
            .into_iter()
            .max_by_key(|(size, position, _)| (std::cmp::Reverse(*size), *position))
            .map(|(_, _, binding)| binding)
    }
}

struct Binding {
    typ: Option<String>,
    is_precise: bool,
}

impl Binding {
    fn as_type(&self) -> Option<(String, bool)> {
        let typ = self
            .typ
            .as_deref()?
            .trim()
            .trim_start_matches(['&', '*'])
            .trim()
            .strip_prefix("mut ")
            .unwrap_or_else(|| {
                self.typ
                    .as_deref()
                    .unwrap()
                    .trim()
                    .trim_start_matches(['&', '*'])
                    .trim()
            });
        let typ = typ.split('<').next()?.trim();
        (!typ.is_empty()
            && typ
                .chars()
                .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ':' | '.')))
        .then(|| (typ.to_string(), self.is_precise))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<ExtractedFile>) {
        let root = tempfile::tempdir().unwrap();
        let extractor = crate::parser::TreeSitterExtractor::new();
        let mut snapshot = Vec::new();
        for (path, source) in files {
            let full = root.path().join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, source).unwrap();
            if supports(path) {
                snapshot.push(extractor.extract(source, path).unwrap());
            }
        }
        (root, snapshot)
    }

    fn call<'a>(file: &'a ExtractedFile, name: &str) -> &'a CallSite {
        file.navigation
            .as_ref()
            .unwrap()
            .calls
            .iter()
            .find(|call| call.name == name)
            .unwrap()
    }

    #[test]
    fn rust_explicit_os_selects_conditional_modules_reexports_and_calls() {
        let (root, files) = project(&[
            ("Cargo.toml", "[package]\nname='example'\nversion='0.1.0'\n"),
            ("src/lib.rs", "mod platform; mod client;\n"),
            ("src/platform/mod.rs", "#[cfg(target_os=\"macos\")]\npub mod macos;\n#[cfg(not(target_os=\"macos\"))]\npub mod windows;\n#[cfg(target_os=\"macos\")]\npub use self::macos::run;\n#[cfg(not(target_os=\"macos\"))]\npub use self::windows::run;\n"),
            ("src/platform/macos.rs", "pub fn run() {}\n"),
            ("src/platform/windows.rs", "pub fn run() {}\n"),
            ("src/client.rs", "use crate::platform::run as renamed;\npub fn caller() {\n #[cfg(target_os=\"macos\")] { renamed(); }\n #[cfg(not(target_os=\"macos\"))] { renamed(); }\n}\n"),
        ]);
        let client = files
            .iter()
            .find(|f| f.file_path == "src/client.rs")
            .unwrap();
        assert_eq!(
            client
                .navigation
                .as_ref()
                .unwrap()
                .calls
                .iter()
                .filter(|c| c.name == "renamed")
                .count(),
            2
        );
        for target in [None, Some("macos"), Some("windows"), Some("linux")] {
            let mut resolver = SourceResolver::new(&files, root.path());
            resolver.target_os = target.map(str::to_string);
            let calls = resolver.calls(client);
            let calls: Vec<_> = calls.iter().filter(|c| c.name == "renamed").collect();
            if target.is_none() {
                assert_eq!(calls.len(), 2);
                assert!(calls
                    .iter()
                    .all(|c| resolver.resolve_call(client, c).is_none()));
            } else {
                assert_eq!(calls.len(), 1);
                let resolved = resolver.resolve_call(client, calls[0]).unwrap();
                let expected = if target == Some("macos") {
                    "src/platform/macos.rs"
                } else {
                    "src/platform/windows.rs"
                };
                assert_eq!(resolved.file.file_path, expected);
            }
        }
    }

    #[test]
    fn rust_unknown_conditions_and_conflicting_aliases_do_not_create_links() {
        let (root,files)=project(&[
            ("src/lib.rs","mod a; mod b;\n#[cfg(feature=\"ui\")] use crate::a::run as uncertain;\nuse crate::a::run as conflict;\nuse crate::b::run as conflict;\nfn entry() { uncertain(); conflict(); }\n"),
            ("src/a.rs","pub fn run() {}\n"),("src/b.rs","pub fn run() {}\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        assert_eq!(
            files
                .iter()
                .flat_map(|f| &f.symbols)
                .filter(|s| s.name == "run")
                .count(),
            2
        );
        let calls = resolver.calls(&files[0]);
        assert_eq!(calls.len(), 2);
        assert!(calls
            .iter()
            .all(|call| resolver.resolve_call(&files[0], call).is_none()));
    }

    #[test]
    fn rust_group_alias_and_inline_module_use_the_declared_source() {
        let (root, files) = project(&[
            ("Cargo.toml", "[package]\nname='example'\nversion='0.1.0'\n"),
            ("src/lib.rs", "mod a; mod b; use crate::a::{run as renamed, nested::{VALUE}}; pub fn entry() { renamed(); }\n"),
            ("src/a.rs", "pub fn run() {} pub mod nested { pub const VALUE: u32 = 1; }\n"),
            ("src/b.rs", "pub fn run() {}\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        let entry = &files[0];
        let target = resolver
            .resolve_call(entry, call(entry, "renamed"))
            .unwrap();
        assert_eq!(target.file.file_path, "src/a.rs");
        assert_eq!(target.symbol.name, "run");
        assert!(target.is_precise);
        let imported = resolver
            .resolve_name(entry, &call(entry, "renamed").range, "VALUE", "const")
            .unwrap();
        assert_eq!(imported.file.file_path, "src/a.rs");
        assert_eq!(imported.symbol.name, "VALUE");
        assert!(!entry
            .navigation
            .as_ref()
            .unwrap()
            .imports
            .iter()
            .any(|import| import.local_name.contains('}')));
    }

    #[test]
    fn rust_methods_do_not_cross_same_named_inline_types_or_local_shadowing() {
        let (root, files) = project(&[(
            "lib.rs",
            r#"
mod left { pub struct Worker; impl Worker { pub fn run(&self) {} } }
mod right { pub struct Worker; impl Worker { pub fn run(&self) {} } }
use left::Worker;
pub fn entry(worker: &Worker) { worker.run(); }
pub fn shadow(worker: &Worker) { let worker = "string"; worker.run(); }
"#,
        )]);
        let resolver = SourceResolver::new(&files, root.path());
        let calls: Vec<_> = files[0]
            .navigation
            .as_ref()
            .unwrap()
            .calls
            .iter()
            .filter(|call| call.name == "run")
            .collect();
        let target = resolver.resolve_call(&files[0], calls[0]).unwrap();
        assert_eq!(target.symbol.range.start_line, 2);
        assert!(resolver.resolve_call(&files[0], calls[1]).is_none());
    }

    #[test]
    fn go_imports_and_receivers_do_not_join_foreign_packages_or_test_packages() {
        let (root, files) = project(&[
            ("go.mod", "module example.local/app\n"),
            ("main.go", "package main\nimport alias \"example.local/app/worker\"\nfunc entry(value *alias.Worker) { alias.Run(); value.Work(); }\n"),
            ("worker/worker.go", "package worker\ntype Worker struct {}\nfunc Run() {}\nfunc (*Worker) Work() {}\n"),
            ("worker/external_test.go", "package worker_test\nfunc Run() {}\n"),
            ("other/worker.go", "package other\ntype Worker struct {}\nfunc Run() {}\nfunc (*Worker) Work() {}\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        for name in ["Run", "Work"] {
            let target = resolver
                .resolve_call(&files[0], call(&files[0], name))
                .unwrap_or_else(|| panic!("expected source target for {name}"));
            assert_eq!(target.file.file_path, "worker/worker.go");
            assert!(target.is_precise);
        }
    }

    #[test]
    fn go_constants_respect_package_and_lexical_scope() {
        let (root, files) = project(&[
            ("main.go", "package main\nfunc other() { const VALUE = 1; _ = VALUE }\nfunc caller() int { return VALUE }\nfunc local() int { const VALUE = 3; return VALUE }\n"),
            ("constants.go", "package main\nconst VALUE = 2\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        for (name, path) in [("caller", "constants.go"), ("local", "main.go")] {
            let symbol = files[0]
                .symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .unwrap();
            let source = resolver.source("main.go").unwrap();
            let function = node_at(&source, &symbol.range).unwrap();
            let body = function.child_by_field_name("body").unwrap();
            let mut cursor = body.walk();
            let statement = body
                .named_children(&mut cursor)
                .find(|node| node.kind() == "statement_list")
                .unwrap();
            let reference = statement
                .named_child((statement.named_child_count() - 1).try_into().unwrap())
                .unwrap();
            let target = resolver
                .resolve_name(&files[0], &node_range(reference), "VALUE", "const")
                .unwrap();
            assert_eq!(target.file.file_path, path);
        }
    }

    #[test]
    fn rust_binary_crate_paths_do_not_resolve_into_the_library() {
        let (root, files) = project(&[
            ("Cargo.toml", "[package]\nname='example'\nversion='0.1.0'\n"),
            ("src/lib.rs", "pub fn run() {}\n"),
            ("src/main.rs", "fn main() { crate::run(); }\nfn run() {}\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        let target = resolver
            .resolve_call(&files[1], call(&files[1], "run"))
            .unwrap();
        assert_eq!(target.file.file_path, "src/main.rs");
    }

    #[test]
    fn go_import_name_shadows_a_constant_in_another_file() {
        let (root, files) = project(&[
            ("main.go", "package main\nimport VALUE \"strings\"\nfunc caller() { _ = VALUE.Clone(\"x\") }\n"),
            ("constants.go", "package main\nconst VALUE = 2\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        assert!(resolver
            .resolve_name(&files[0], &call(&files[0], "Clone").range, "VALUE", "const")
            .is_none());
    }

    #[test]
    fn inferred_receivers_remain_unresolved_while_explicit_types_keep_locations() {
        let (root, files) = project(&[(
            "lib.rs",
            concat!(
                "struct Worker; impl Worker { fn new() -> Self { Self } fn run(&self) {} }\n",
                "fn entry() {\n  let inferred = Worker::new(); inferred.run();\n",
                "  let explicit: Worker = Worker::new(); explicit.run();\n}\n",
            ),
        )]);
        let resolver = SourceResolver::new(&files, root.path());
        let calls: Vec<_> = resolver
            .calls(&files[0])
            .into_iter()
            .filter(|call| call.name == "run")
            .collect();
        assert_eq!(calls.len(), 2);
        assert!(resolver.resolve_call(&files[0], &calls[0]).is_none());
        let target = resolver.resolve_call(&files[0], &calls[1]).unwrap();
        assert_eq!(target.symbol.owner.as_deref(), Some("Worker"));
        assert!(target.is_precise);
    }

    #[test]
    fn callback_parameters_and_generic_types_do_not_resolve_as_global_definitions() {
        let (root, files) = project(&[("lib.rs", concat!(
            "struct Worker; impl Worker { fn trim(&self) {} }\n",
            "fn target() {}\n",
            "fn callbacks(value: Worker) { let callback = |target: fn(), value: &str| { target(); value.trim(); }; }\n",
            "trait Runner { fn act(&self); }\n",
            "struct T; impl T { fn act(&self) {} }\n",
            "fn generic<T: Runner>(value: T) { value.act(); }\n",
        ))]);
        let resolver = SourceResolver::new(&files, root.path());
        for name in ["target", "trim", "act"] {
            assert!(
                resolver
                    .resolve_call(&files[0], call(&files[0], name))
                    .is_none(),
                "unexpected global target for {name}"
            );
        }
        let (root, files) = project(&[(
            "main.go",
            concat!(
                "package main\nfunc Target() {}\n",
                "func callback() { _ = func(Target func()) { Target() } }\n",
                "type T struct{}\nfunc (T) Act() {}\n",
                "type Runner interface { Act() }\n",
                "func generic[T Runner](value T) { value.Act() }\n",
            ),
        )]);
        let resolver = SourceResolver::new(&files, root.path());
        for name in ["Target", "Act"] {
            assert!(
                resolver
                    .resolve_call(&files[0], call(&files[0], name))
                    .is_none(),
                "unexpected global Go target for {name}"
            );
        }
    }

    #[test]
    fn inner_local_bindings_override_closure_parameters_without_leaking_loop_bindings() {
        let (root, files) = project(&[(
            "lib.rs",
            concat!(
                "struct Worker; impl Worker { fn trim(&self) {} }\n",
                "fn target() {}\n",
                "fn entry() {\n",
                "  let callback = |value: &str| { let value: Worker = Worker; value.trim(); };\n",
                "  for target in [1, 2] {}\n",
                "  target();\n",
                "}\n",
            ),
        )]);
        let resolver = SourceResolver::new(&files, root.path());
        for name in ["trim", "target"] {
            let target = resolver
                .resolve_call(&files[0], call(&files[0], name))
                .unwrap();
            assert_eq!(target.symbol.name, name);
        }
    }

    #[test]
    fn stale_import_and_local_binding_are_checked_against_current_source() {
        let original = "mod left1; mod right; use crate::left1::run; fn entry() { run(); }\n";
        let (root, files) = project(&[
            ("Cargo.toml", "[package]\nname='example'\nversion='0.1.0'\n"),
            ("src/lib.rs", original),
            ("src/left1.rs", "pub fn run() {}\n"),
            ("src/right.rs", "pub fn run() {}\n"),
        ]);
        std::fs::write(
            root.path().join("src/lib.rs"),
            original.replace("use crate::left1", "use crate::right"),
        )
        .unwrap();
        let resolver = SourceResolver::new(&files, root.path());
        let target = resolver
            .resolve_call(&files[0], call(&files[0], "run"))
            .unwrap();
        assert_eq!(target.file.file_path, "src/right.rs");

        let original = "fn target() {}\nfn entry() {\n                        \n    target();\n}\n";
        let (root, files) = project(&[("lib.rs", original)]);
        std::fs::write(
            root.path().join("lib.rs"),
            original.replace("                        ", "    let target = || {};  "),
        )
        .unwrap();
        let resolver = SourceResolver::new(&files, root.path());
        assert!(resolver
            .resolve_call(&files[0], call(&files[0], "target"))
            .is_none());
    }

    #[test]
    fn stale_symbol_spelling_is_not_confirmed_by_matching_ranges() {
        let (root, files) = project(&[("lib.rs", "fn before() {}\nfn caller() { before(); }\n")]);
        std::fs::write(
            root.path().join("lib.rs"),
            "fn afterx() {}\nfn caller() { before(); }\n",
        )
        .unwrap();
        let resolver = SourceResolver::new(&files, root.path());
        assert!(resolver
            .resolve_call(&files[0], call(&files[0], "before"))
            .is_none());
    }
}
