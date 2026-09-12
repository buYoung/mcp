//! Runtime-only test-region filtering for automatic navigation context.
//! Original indexed files and live read/grep results are never rewritten.

use crate::config::TestCodeRules;
use crate::parser::{CodeRange, ExtractedFile};
use globset::{GlobBuilder, GlobMatcher};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;
use tree_sitter::{Node, Parser};

#[derive(Clone)]
struct TestRegion {
    start_byte: usize,
    end_byte: usize,
    range: CodeRange,
}

type Regions = Arc<Vec<TestRegion>>;
struct CacheEntry {
    modified: SystemTime,
    size_bytes: u64,
    rules_hash: u64,
    regions: Regions,
}
static CACHE: OnceLock<Mutex<HashMap<PathBuf, CacheEntry>>> = OnceLock::new();

pub(crate) struct TestCodeFilter {
    root: PathBuf,
    should_include: bool,
    max_file_size_bytes: u64,
    rules: TestCodeRules,
    rules_hash: u64,
    file_patterns: Vec<(GlobMatcher, bool)>,
    attributes: BTreeMap<String, Vec<GlobMatcher>>,
    decorators: BTreeMap<String, Vec<GlobMatcher>>,
    calls: BTreeMap<String, Vec<GlobMatcher>>,
}

fn compile_patterns(patterns: &[String], is_file: bool) -> Vec<GlobMatcher> {
    patterns
        .iter()
        .filter_map(|pattern| {
            GlobBuilder::new(pattern)
                .literal_separator(is_file)
                .backslash_escape(false)
                .build()
                .ok()
                .map(|glob| glob.compile_matcher())
        })
        .collect()
}

fn compile_languages(
    patterns: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<GlobMatcher>> {
    patterns
        .iter()
        .map(|(language, patterns)| (language.clone(), compile_patterns(patterns, false)))
        .collect()
}

impl TestCodeFilter {
    pub(crate) fn from_config(root: &Path) -> Self {
        let cfg = crate::config::get();
        let rules = cfg.test_code_rules.clone();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        rules.hash(&mut hasher);
        Self {
            root: root.to_path_buf(),
            should_include: cfg.should_include_test_code,
            max_file_size_bytes: cfg.max_file_size,
            rules_hash: hasher.finish(),
            file_patterns: rules
                .file_patterns
                .iter()
                .filter_map(|pattern| {
                    Some((
                        GlobBuilder::new(pattern)
                            .literal_separator(true)
                            .build()
                            .ok()?
                            .compile_matcher(),
                        !pattern.contains('/'),
                    ))
                })
                .collect(),
            attributes: compile_languages(&rules.attributes),
            decorators: compile_languages(&rules.decorators),
            calls: compile_languages(&rules.calls),
            rules,
        }
    }

    fn is_test_file(&self, file_path: &str) -> bool {
        let normalized = file_path.replace('\\', "/");
        let name = normalized.rsplit('/').next().unwrap_or(&normalized);
        self.file_patterns.iter().any(|(pattern, is_basename)| {
            pattern.is_match(if *is_basename { name } else { &normalized })
        })
    }

    fn regions(&self, file_path: &str) -> Regions {
        if self.should_include {
            return Arc::new(Vec::new());
        }
        if self.is_test_file(file_path) {
            return Arc::new(vec![TestRegion {
                start_byte: 0,
                end_byte: usize::MAX,
                range: CodeRange {
                    start_line: 1,
                    start_col: 1,
                    end_line: usize::MAX,
                    end_col: usize::MAX,
                },
            }]);
        }
        let path = self.root.join(file_path);
        let Some(spec) = crate::lang::spec_for_path(&path) else {
            return Arc::new(Vec::new());
        };
        let language = spec.language_name();
        if !self.attributes.contains_key(language)
            && !self.decorators.contains_key(language)
            && !self.calls.contains_key(language)
        {
            return Arc::new(Vec::new());
        }
        let Ok(metadata) = std::fs::metadata(&path) else {
            return Arc::new(Vec::new());
        };
        if metadata.len() > self.max_file_size_bytes {
            return Arc::new(Vec::new());
        }
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(entries) = cache.lock() {
            if let Some(entry) = entries.get(&path) {
                if entry.modified == modified
                    && entry.size_bytes == metadata.len()
                    && entry.rules_hash == self.rules_hash
                {
                    return Arc::clone(&entry.regions);
                }
            }
        }
        let Ok(source) = std::fs::read(&path) else {
            return Arc::new(Vec::new());
        };
        let mut parser = Parser::new();
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if parser.set_language(&spec.grammar(ext)).is_err() {
            return Arc::new(Vec::new());
        }
        let Some(tree) = parser.parse(&source, None) else {
            return Arc::new(Vec::new());
        };
        let mut regions = Vec::new();
        let mut nodes = vec![tree.root_node()];
        while let Some(node) = nodes.pop() {
            if let Some(target) = self.test_scope(node, language, &source) {
                let mut beginning = if node.start_byte() < target.start_byte() {
                    node
                } else {
                    target
                };
                if language == "rust" && node.kind() == "attribute_item" {
                    while let Some(previous) = beginning.prev_named_sibling() {
                        if !matches!(
                            previous.kind(),
                            "attribute_item" | "line_comment" | "block_comment" | "comment"
                        ) {
                            break;
                        }
                        beginning = previous;
                    }
                }
                let start = beginning.start_position();
                let end = target.end_position();
                regions.push(TestRegion {
                    start_byte: beginning.start_byte(),
                    end_byte: target.end_byte(),
                    range: CodeRange {
                        start_line: start.row + 1,
                        start_col: start.column + 1,
                        end_line: end.row + 1,
                        end_col: end.column + 1,
                    },
                });
            }
            let mut cursor = node.walk();
            nodes.extend(node.named_children(&mut cursor));
        }
        regions.sort_by_key(|region| region.start_byte);
        let regions = Arc::new(regions);
        if let Ok(mut entries) = cache.lock() {
            if entries.len() >= 1024 {
                entries.clear();
            }
            entries.insert(
                path,
                CacheEntry {
                    modified,
                    size_bytes: metadata.len(),
                    rules_hash: self.rules_hash,
                    regions: Arc::clone(&regions),
                },
            );
        }
        regions
    }

    fn test_scope<'a>(&self, node: Node<'a>, language: &str, source: &[u8]) -> Option<Node<'a>> {
        let kind = node.kind();
        if matches!(kind, "attribute_item" | "inner_attribute_item") && language == "rust" {
            let text = node.utf8_text(source).ok()?;
            let attribute = text
                .trim()
                .strip_prefix(if kind == "inner_attribute_item" {
                    "#!["
                } else {
                    "#["
                })?
                .strip_suffix(']')?
                .trim();
            let is_cfg_test = self
                .rules
                .attributes
                .get(language)
                .is_some_and(|rules| rules.iter().any(|rule| rule == "cfg(test)"))
                && is_test_only_cfg(attribute);
            let name = marker_name(attribute);
            if is_cfg_test
                || (name != "cfg" && has_matching_test_marker(&self.attributes, language, name))
            {
                if kind == "inner_attribute_item" {
                    return declaration_parent(node).or_else(|| node.parent());
                }
                let mut next = node.next_named_sibling();
                while let Some(candidate) = next {
                    if !matches!(
                        candidate.kind(),
                        "attribute_item" | "line_comment" | "block_comment" | "comment"
                    ) {
                        return Some(candidate);
                    }
                    next = candidate.next_named_sibling();
                }
                return declaration_parent(node);
            }
        } else if kind == "decorator" {
            let text = node.utf8_text(source).ok()?.trim().trim_start_matches('@');
            if has_matching_test_marker(&self.decorators, language, marker_name(text)) {
                return declaration_parent(node);
            }
        } else if matches!(
            kind,
            "annotation"
                | "marker_annotation"
                | "normal_annotation"
                | "annotation_entry"
                | "attribute"
        ) {
            let text = node.utf8_text(source).ok()?.trim().trim_start_matches('@');
            if has_matching_test_marker(&self.attributes, language, marker_name(text)) {
                return declaration_parent(node);
            }
        } else if matches!(kind, "call_expression" | "call" | "method_call" | "command") {
            let function = node
                .child_by_field_name("function")
                .or_else(|| node.child_by_field_name("method"))
                .or_else(|| node.child_by_field_name("name"))
                .or_else(|| node.named_child(0))?;
            let name = marker_name(function.utf8_text(source).ok()?.trim());
            if has_matching_test_name(&self.calls, language, name) {
                return Some(node);
            }
        }
        None
    }

    pub(crate) fn is_excluded(&self, file_path: &str, range: &CodeRange) -> bool {
        is_in_test_region(&self.regions(file_path), range)
    }

    pub(crate) fn filter_snapshot<'a>(
        &self,
        snapshot: &'a [ExtractedFile],
    ) -> Cow<'a, [ExtractedFile]> {
        if self.should_include {
            return Cow::Borrowed(snapshot);
        }
        // Annotation consumers need declarations/navigation, not potentially large literals.
        let filtered = snapshot
            .iter()
            .map(|file| {
                let regions = self.regions(&file.file_path);
                let mut navigation = file.navigation.clone();
                if let Some(navigation) = &mut navigation {
                    navigation
                        .calls
                        .retain(|site| !is_in_test_region(&regions, &site.range));
                    navigation
                        .references
                        .retain(|site| !is_in_test_region(&regions, &site.range));
                    navigation
                        .local_bindings
                        .retain(|site| !is_in_test_region(&regions, &site.range));
                    navigation
                        .imports
                        .retain(|site| !is_in_test_region(&regions, &site.range));
                }
                ExtractedFile {
                    file_path: file.file_path.clone(),
                    total_lines: file.total_lines,
                    symbols: file
                        .symbols
                        .iter()
                        .filter(|symbol| !is_in_test_region(&regions, &symbol.range))
                        .cloned()
                        .collect(),
                    literals: Vec::new(),
                    docstrings: Vec::new(),
                    navigation,
                }
            })
            .collect();
        Cow::Owned(filtered)
    }

    pub(crate) fn mask_source(&self, file_path: &str, source: &mut [u8]) {
        for region in self.regions(file_path).iter() {
            let start = region.start_byte.min(source.len());
            let end = region.end_byte.min(source.len());
            for byte in &mut source[start..end] {
                if *byte != b'\n' && *byte != b'\r' {
                    *byte = b' ';
                }
            }
        }
    }
}

fn is_in_test_region(regions: &[TestRegion], range: &CodeRange) -> bool {
    regions.iter().any(|region| {
        (region.range.start_line, region.range.start_col) <= (range.start_line, range.start_col)
            && (range.start_line, range.start_col) < (region.range.end_line, region.range.end_col)
    })
}

fn marker_name(text: &str) -> &str {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, '(' | '[' | ']'))
        .next()
        .unwrap_or("")
}

fn has_matching_test_name(
    rules: &BTreeMap<String, Vec<GlobMatcher>>,
    language: &str,
    name: &str,
) -> bool {
    rules
        .get(language)
        .is_some_and(|patterns| patterns.iter().any(|pattern| pattern.is_match(name)))
}

fn has_matching_test_marker(
    rules: &BTreeMap<String, Vec<GlobMatcher>>,
    language: &str,
    name: &str,
) -> bool {
    has_matching_test_name(rules, language, name)
        || has_matching_test_name(
            rules,
            language,
            name.rsplit(['.', '\\']).next().unwrap_or(name),
        )
}

fn declaration_parent(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = node.parent() {
        node = parent;
        if matches!(
            node.kind(),
            "decorated_definition"
                | "function_item"
                | "function_definition"
                | "function_declaration"
                | "method_declaration"
                | "method_definition"
                | "class_declaration"
                | "class_definition"
                | "struct_item"
                | "mod_item"
                | "module"
                | "property_declaration"
        ) {
            return Some(node);
        }
        if node.kind().contains("parameter") {
            return None;
        }
    }
    None
}

// Evaluate a cfg predicate with `test=false`; other options remain unknown.
// Only a predicate proven false without `test` identifies a test-only region.
fn is_test_only_cfg(attribute: &str) -> bool {
    fn evaluate(expression: &str, depth: usize) -> (Option<bool>, bool) {
        if depth > 64 {
            return (None, false);
        }
        let expression = expression.trim();
        if expression == "test" {
            return (Some(false), true);
        }
        if expression == "true" {
            return (Some(true), false);
        }
        if expression == "false" {
            return (Some(false), false);
        }
        let Some((operator, rest)) = expression.split_once('(') else {
            return (None, false);
        };
        let Some(inner) = rest.trim().strip_suffix(')') else {
            return (None, false);
        };
        let mut parts = Vec::new();
        let mut start = 0;
        let mut nesting_depth = 0;
        let mut is_quoted = false;
        let mut is_escaped = false;
        for (index, ch) in inner.char_indices() {
            if is_escaped {
                is_escaped = false;
                continue;
            }
            if is_quoted && ch == '\\' {
                is_escaped = true;
                continue;
            }
            if ch == '"' {
                is_quoted = !is_quoted;
                continue;
            }
            if is_quoted {
                continue;
            }
            match ch {
                '(' => nesting_depth += 1,
                ')' => nesting_depth -= 1,
                ',' if nesting_depth == 0 => {
                    parts.push(inner[start..index].trim());
                    start = index + 1;
                }
                _ => {}
            }
        }
        if !inner[start..].trim().is_empty() {
            parts.push(inner[start..].trim());
        }
        let values: Vec<_> = parts.iter().map(|part| evaluate(part, depth + 1)).collect();
        let has_test = values.iter().any(|(_, has_test)| *has_test);
        let value = match operator.trim() {
            "not" if values.len() == 1 => values[0].0.map(|value| !value),
            "all" if values.iter().any(|(value, _)| *value == Some(false)) => Some(false),
            "all" if values.iter().all(|(value, _)| *value == Some(true)) => Some(true),
            "any" if values.iter().any(|(value, _)| *value == Some(true)) => Some(true),
            "any" if values.iter().all(|(value, _)| *value == Some(false)) => Some(false),
            _ => None,
        };
        (value, has_test)
    }
    let Some((name, inner)) = attribute.split_once('(') else {
        return false;
    };
    name.trim() == "cfg"
        && inner
            .strip_suffix(')')
            .is_some_and(|expression| evaluate(expression, 0) == (Some(false), true))
}
