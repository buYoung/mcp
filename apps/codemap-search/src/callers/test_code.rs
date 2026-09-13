//! Runtime-only test-region filtering for automatic navigation context.
//! Original indexed files and live read/grep results are never rewritten.

use crate::config::TestCodeRules;
use crate::parser::{CodeRange, ExtractedFile};
use globset::{GlobBuilder, GlobMatcher};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
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
struct RegionCache {
    entries: HashMap<PathBuf, CacheEntry>,
    capacity: usize,
}

impl Default for RegionCache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            capacity: 1024,
        }
    }
}

impl RegionCache {
    fn retain_snapshot(&mut self, root: &Path, snapshot: &[ExtractedFile]) {
        let paths: HashSet<_> = snapshot
            .iter()
            .map(|file| root.join(&file.file_path))
            .collect();
        self.entries.retain(|path, _| paths.contains(path));
        // A whole-snapshot scan must fit, or every read/grep reparses the same
        // large repository. Leave bounded room for live files outside the index.
        self.capacity = paths.len().saturating_add(1024);
    }

    fn insert(&mut self, path: PathBuf, entry: CacheEntry) {
        if !self.entries.contains_key(&path) && self.entries.len() >= self.capacity {
            if let Some(evicted) = self.entries.keys().next().cloned() {
                self.entries.remove(&evicted);
            }
        }
        self.entries.insert(path, entry);
    }
}

static CACHE: OnceLock<Mutex<RegionCache>> = OnceLock::new();

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
    marker_prefilters: BTreeMap<String, Option<regex::bytes::Regex>>,
}

fn marker_prefilter(patterns: &[&str], language: &str) -> Option<regex::bytes::Regex> {
    let mut alternatives = Vec::new();
    for &pattern in patterns {
        // A wildcard/alternative may match names with no common literal. Leave
        // these rules to the syntax filter instead of risking a false negative.
        if pattern.is_empty() || pattern.contains(['*', '?', '[', ']', '{', '}']) {
            return None;
        }
        let literal = if language == "rust" && pattern == "cfg(test)" {
            "cfg"
        } else {
            pattern
        };
        let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
        let start = if literal.bytes().next().is_some_and(is_word) {
            r"(?-u:\b)"
        } else {
            ""
        };
        let end = if literal.bytes().last().is_some_and(is_word) {
            r"(?-u:\b)"
        } else {
            ""
        };
        alternatives.push(format!("{start}{}{end}", regex::escape(literal)));
    }
    regex::bytes::Regex::new(&alternatives.join("|")).ok()
}

fn compile_marker_prefilters(
    rules: &TestCodeRules,
) -> BTreeMap<String, Option<regex::bytes::Regex>> {
    let mut languages: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for map in [&rules.attributes, &rules.decorators, &rules.calls] {
        for (language, patterns) in map {
            languages
                .entry(language)
                .or_default()
                .extend(patterns.iter().map(String::as_str));
        }
    }
    languages
        .into_iter()
        .map(|(language, patterns)| (language.to_string(), marker_prefilter(&patterns, language)))
        .collect()
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
            marker_prefilters: compile_marker_prefilters(&rules),
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

    pub(crate) fn is_file_excluded(&self, file_path: &str) -> bool {
        !self.should_include && self.is_test_file(file_path)
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
        if self.attributes.get(language).is_none_or(Vec::is_empty)
            && self.decorators.get(language).is_none_or(Vec::is_empty)
            && self.calls.get(language).is_none_or(Vec::is_empty)
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
        let cache = CACHE.get_or_init(|| Mutex::new(RegionCache::default()));
        if let Ok(cache) = cache.lock() {
            if let Some(entry) = cache.entries.get(&path) {
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
        let may_have_marker = self
            .marker_prefilters
            .get(language)
            .and_then(Option::as_ref)
            .is_none_or(|pattern| pattern.is_match(&source));
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let regions = if may_have_marker {
            self.parse_regions(&source, language, spec.grammar(ext))
        } else {
            Vec::new()
        };
        let regions = Arc::new(regions);
        if let Ok(mut cache) = cache.lock() {
            cache.insert(
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

    fn parse_regions(
        &self,
        source: &[u8],
        language: &str,
        grammar: tree_sitter::Language,
    ) -> Vec<TestRegion> {
        let mut parser = Parser::new();
        if parser.set_language(&grammar).is_err() {
            return Vec::new();
        }
        let Ok(tree) = crate::parser::parse_source(&mut parser, source) else {
            // A possible test marker was found but its extent cannot be proven.
            // Keep the entire file out of automatic context on parse failure.
            return vec![TestRegion {
                start_byte: 0,
                end_byte: usize::MAX,
                range: CodeRange {
                    start_line: 1,
                    start_col: 1,
                    end_line: usize::MAX,
                    end_col: usize::MAX,
                },
            }];
        };
        let mut regions = Vec::new();
        let mut nodes = vec![tree.root_node()];
        while let Some(node) = nodes.pop() {
            if let Some(target) = self.test_scope(node, language, source) {
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

    pub(crate) fn retain_snapshot_cache(&self, snapshot: &[ExtractedFile]) {
        if let Ok(mut cache) = CACHE
            .get_or_init(|| Mutex::new(RegionCache::default()))
            .lock()
        {
            cache.retain_snapshot(&self.root, snapshot);
        }
    }

    pub(crate) fn filter_snapshot<'a>(
        &self,
        snapshot: &'a [ExtractedFile],
        should_include_file: impl Fn(&ExtractedFile) -> bool,
    ) -> Cow<'a, [ExtractedFile]> {
        if self.should_include && snapshot.iter().all(&should_include_file) {
            return Cow::Borrowed(snapshot);
        }
        self.retain_snapshot_cache(snapshot);
        // Annotation consumers need declarations/navigation, not potentially large literals.
        let filtered = snapshot
            .iter()
            .filter(|file| should_include_file(file))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_prefilter_preserves_custom_rules_and_syntax_filtering() {
        let powershell = marker_prefilter(&["Describe", "Context", "It"], "powershell").unwrap();
        assert!(!powershell.is_match(b"function Write-Output { Get-Item . }"));
        assert!(powershell.is_match(b"Describe 'case' { It 'works' {} }"));
        assert!(marker_prefilter(&["*Suite", "Test"], "java").is_none());
        assert!(marker_prefilter(&["{Fact,Theory}"], "csharp").is_none());
        assert!(marker_prefilter(&["cfg(test)"], "rust")
            .unwrap()
            .is_match(b"#[cfg(all(unix, test))]"));
        assert!(marker_prefilter(&["Fact"], "csharp")
            .unwrap()
            .is_match(b"[Xunit.Fact]"));

        let root = tempfile::tempdir().unwrap();
        let mut filter = TestCodeFilter::from_config(root.path());
        filter.should_include = false;
        filter.file_patterns.clear();
        std::fs::write(root.path().join("commands.ps1"), b"$text = 'Describe demo {}'\n# It 'comment' {}\nfunction Visible {}\nDescribe 'real' { function Hidden {} }\n").unwrap();
        let regions = filter.regions("commands.ps1");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].range.start_line, 4);
        assert_eq!(regions[0].range.end_line_inclusive(), 4);
    }

    #[test]
    fn large_snapshot_keeps_test_regions_between_reads_and_drops_removed_files() {
        let root = Path::new("validation-repository");
        let mut snapshot: Vec<_> = (0..1025)
            .map(|index| ExtractedFile {
                file_path: format!("source-{index}.ps1"),
                total_lines: 1,
                symbols: Vec::new(),
                literals: Vec::new(),
                docstrings: Vec::new(),
                navigation: None,
            })
            .collect();
        let mut cache = RegionCache::default();
        cache.retain_snapshot(root, &snapshot);
        let regions = Arc::new(Vec::new());
        let entry = || CacheEntry {
            modified: SystemTime::UNIX_EPOCH,
            size_bytes: 7,
            rules_hash: 1,
            regions: Arc::clone(&regions),
        };
        for file in &snapshot {
            cache.insert(root.join(&file.file_path), entry());
        }
        cache.retain_snapshot(root, &snapshot);
        assert_eq!(cache.entries.len(), snapshot.len());
        assert!(snapshot.iter().all(|file| {
            Arc::ptr_eq(
                &cache.entries[&root.join(&file.file_path)].regions,
                &regions,
            )
        }));

        let removed = snapshot.pop().unwrap();
        cache.retain_snapshot(root, &snapshot);
        assert!(!cache.entries.contains_key(&root.join(removed.file_path)));
        for index in 0..1026 {
            cache.insert(root.join(format!("live-{index}.ps1")), entry());
        }
        assert_eq!(cache.entries.len(), cache.capacity);
    }
}
