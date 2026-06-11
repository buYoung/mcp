pub trait CodemapView {
    fn to_markdown(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct ExtractedSymbolSummary<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub is_exported: bool,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractedFileSummary<'a> {
    pub file_path: String,
    pub total_lines: usize,
    pub symbol_count: usize,
    pub symbols: Vec<ExtractedSymbolSummary<'a>>,
}

/// One directory node in the root overview: its path plus the number of source
/// files and significant symbols anywhere beneath it (recursive aggregate). The
/// root view shows this skeleton instead of a per-file dump so a large repo stays
/// cheap to orient in — the agent reads the tree, spots the relevant subtree by
/// its counts, then drills with `overview <dir>`.
#[derive(Debug, Clone)]
pub struct DirectorySummary {
    pub path: String,
    pub file_count: usize,
    pub symbol_count: usize,
}

#[derive(Debug, Clone)]
pub struct RootCodemap<'a> {
    pub total_files: usize,
    pub total_symbols: usize,
    pub directories: Vec<DirectorySummary>,
    pub files: Vec<ExtractedFileSummary<'a>>,
    // Store reference to original files if needed, or define with references
    pub original_files: &'a [crate::parser::ExtractedFile],
}

/// Max directory rows the root view emits before truncating with a footer. The
/// per-file dump is already gone (Design B); this only bounds pathologically wide
/// monorepos so a tree with thousands of directories can't reintroduce the bloat.
/// Set high enough that an ordinary large repo (hundreds of directories) renders in
/// full — at ~50 bytes/row even 400 rows is ~5 KB, an order below the old per-file dump.
const ROOT_DIRECTORY_LIMIT: usize = 400;

#[derive(Debug, Clone)]
pub struct FolderCodemap<'a> {
    pub folder_path: String,
    /// Recursive file/symbol counts for the whole tree (repo-wide). The folder view
    /// renders only the part reachable from `folder_path`, but counts are repo-wide so a
    /// directory reads identically here and in the root view.
    pub directories: Vec<DirectorySummary>,
    /// Recursive totals for `folder_path` itself (its scope header).
    pub total_files: usize,
    pub total_symbols: usize,
    pub files: Vec<ExtractedFileSummary<'a>>,
    pub original_files: &'a [crate::parser::ExtractedFile],
}

#[derive(Debug, Clone)]
pub struct DetailsCodemap<'a> {
    pub file_path: String,
    pub total_lines: usize,
    pub symbols: &'a [crate::parser::ExtractedSymbol],
}

/// Kinds whose body opens a function scope. A symbol strictly contained within
/// one of these is function-local (a local variable, nested fn, or closure) and
/// is dropped from significant views. The tree-sitter layer normalizes both free
/// functions and methods to "fn"; "method"/"function" are listed for resilience
/// if that normalization ever changes.
const FUNCTION_SCOPE_KINDS: &[&str] = &["fn", "method", "function"];

/// `outer` strictly contains `inner` when `inner`'s line span sits inside
/// `outer`'s and the two spans are not identical — so a symbol never contains
/// itself and two symbols sharing a range never drop each other.
pub(crate) fn range_strictly_contains(
    outer: &crate::parser::CodeRange,
    inner: &crate::parser::CodeRange,
) -> bool {
    outer.start_line <= inner.start_line
        && inner.end_line <= outer.end_line
        && (outer.start_line < inner.start_line || inner.end_line < outer.end_line)
}

/// A symbol is "significant" when it is exported, or when it is not nested inside
/// a function scope in the same file. Type members (class/struct/impl methods —
/// contained by a type symbol, not a function) stay significant; function-local
/// symbols are dropped. Operates on the flat per-file symbol list, since the
/// extractor records no parent links.
fn is_significant_symbol(
    symbol: &crate::parser::ExtractedSymbol,
    file_symbols: &[crate::parser::ExtractedSymbol],
) -> bool {
    if symbol.flags.is_exported {
        return true;
    }
    !file_symbols.iter().any(|parent| {
        FUNCTION_SCOPE_KINDS.contains(&parent.kind.as_str())
            && range_strictly_contains(&parent.range, &symbol.range)
    })
}

/// Build a per-file summary carrying only significant symbols, with their count.
/// Shared by the root and folder views so both apply the same filter.
fn summarize_file(file: &crate::parser::ExtractedFile) -> ExtractedFileSummary<'_> {
    let symbols: Vec<ExtractedSymbolSummary<'_>> = file
        .symbols
        .iter()
        .filter(|s| is_significant_symbol(s, &file.symbols))
        .map(|s| ExtractedSymbolSummary {
            name: &s.name,
            kind: &s.kind,
            is_exported: s.flags.is_exported,
            start_line: s.range.start_line,
            end_line: s.range.end_line,
        })
        .collect();
    ExtractedFileSummary {
        file_path: normalize_path(&file.file_path).into_owned(),
        total_lines: file.total_lines,
        symbol_count: symbols.len(),
        symbols,
    }
}

/// Aggregate every ancestor directory's recursive file + significant-symbol counts in
/// one pass over the file summaries: each prefix of a file's path is an ancestor, so a
/// file in `src/a/b.ts` credits both `src` and `src/a`. Shared by the root and folder
/// views so a directory's counts are identical regardless of where you view it from. The
/// BTreeMap keeps the result path-sorted.
fn build_directory_summaries(files: &[ExtractedFileSummary]) -> Vec<DirectorySummary> {
    let mut dir_counts: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new();
    for file in files {
        let parts: Vec<&str> = file.file_path.split('/').collect();
        for i in 1..parts.len() {
            let dir_path = parts[0..i].join("/");
            if dir_path.is_empty() {
                continue;
            }
            let entry = dir_counts.entry(dir_path).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += file.symbol_count;
        }
    }
    dir_counts
        .into_iter()
        .map(|(path, (file_count, symbol_count))| DirectorySummary {
            path,
            file_count,
            symbol_count,
        })
        .collect()
}

type ChildMap<'a> = std::collections::HashMap<&'a str, Vec<&'a DirectorySummary>>;

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A directory is a leaf when it has no subdirectories.
fn dir_is_leaf(children: &ChildMap, path: &str) -> bool {
    children.get(path).map_or(true, |kids| kids.is_empty())
}

/// "Deep" = has at least one non-leaf subdirectory (its subtree is ≥2 levels).
fn dir_is_deep(children: &ChildMap, path: &str) -> bool {
    children
        .get(path)
        .is_some_and(|kids| kids.iter().any(|c| !dir_is_leaf(children, &c.path)))
}

/// True when at least one immediate subdirectory is itself a leaf.
fn dir_has_leaf_child(children: &ChildMap, path: &str) -> bool {
    children
        .get(path)
        .is_some_and(|kids| kids.iter().any(|c| dir_is_leaf(children, &c.path)))
}

/// How a child renders *inline* on its parent's line:
/// - leaf → `name (counts)`
/// - terminal-group (all of its children are leaves) → `name (counts): leafA (counts), …`
/// - deep (has a non-leaf child) → `name (counts)` (bare; its subtree continues on its
///   own anchor line(s))
fn render_inline_child(children: &ChildMap, child: &DirectorySummary) -> String {
    let base = basename(&child.path);
    let head = format!(
        "{} ({} files, {} symbols)",
        base, child.file_count, child.symbol_count
    );
    if dir_is_leaf(children, &child.path) || dir_is_deep(children, &child.path) {
        head
    } else {
        // terminal-group: inline its leaf children one level deep, wrapped in braces so
        // the nested group is unambiguous against the parent's own sibling list
        // (`auth: {a, b}, common` — `common` is clearly the parent's child, not auth's).
        let leaves = children[child.path.as_str()]
            .iter()
            .map(|g| {
                format!(
                    "{} ({} files, {} symbols)",
                    basename(&g.path),
                    g.file_count,
                    g.symbol_count
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}: {{{}}}", head, leaves)
    }
}

/// Render a directory tree with Rust-`use`-style inlining. Each *anchor* directory
/// occupies one line that inlines all its immediate children (see [`render_inline_child`]);
/// the repeated parent-path prefix — the dominant redundancy on a deep tree — is written
/// once. Which directories get their own line:
/// - every immediate child of `scope` is an anchor (`scope` is `""` for the repo root, or
///   a folder path like `src/modules` for a folder view);
/// - a `deep` child of an anchor that *has a leaf child of its own* becomes an anchor
///   (it has direct files/leaf-dirs worth breaking out, e.g. `src/common`);
/// - a `deep` child with *no* leaf child (a pure junction, e.g. `src/common/modules`)
///   gets no line — it is only bare-mentioned on its parent, and its subdirectories are
///   promoted to anchors instead.
/// The full repo `directories` slice is passed regardless of scope; only directories
/// reachable from `scope` are seeded, so the rest never render. Output is bounded by
/// `limit` emitted anchor lines so a pathologically wide tree can't blow the budget.
fn write_directory_tree(
    f: &mut std::fmt::Formatter<'_>,
    directories: &[DirectorySummary],
    scope: &str,
    limit: usize,
) -> std::fmt::Result {
    use std::collections::{BTreeSet, HashMap};

    // Immediate children keyed by parent path (top-level directories sit under "").
    // `directories` is path-sorted, so each child list stays alphabetical.
    let mut children: ChildMap = HashMap::new();
    for dir in directories {
        let parent = match dir.path.rfind('/') {
            Some(slash) => &dir.path[..slash],
            None => "",
        };
        children.entry(parent).or_default().push(dir);
    }

    // Decide the anchor set (directories that get their own line). A small worklist
    // walks down from `scope`'s immediate children: an anchor exposes its deep children,
    // and a deep child either becomes an anchor (it has a leaf child) or is skipped so its
    // own children are considered in turn.
    enum Task<'a> {
        Anchor(&'a str),
        Consider(&'a str),
    }
    let mut anchors: BTreeSet<&str> = BTreeSet::new();
    let mut stack: Vec<Task> = Vec::new();
    if let Some(tops) = children.get(scope) {
        for dir in tops {
            stack.push(Task::Anchor(dir.path.as_str()));
        }
    }
    while let Some(task) = stack.pop() {
        match task {
            Task::Anchor(path) => {
                if !anchors.insert(path) {
                    continue;
                }
                if let Some(kids) = children.get(path) {
                    for child in kids {
                        if dir_is_deep(&children, &child.path) {
                            stack.push(Task::Consider(child.path.as_str()));
                        }
                    }
                }
            }
            Task::Consider(path) => {
                if dir_has_leaf_child(&children, path) {
                    stack.push(Task::Anchor(path));
                } else if let Some(kids) = children.get(path) {
                    for child in kids {
                        stack.push(Task::Consider(child.path.as_str()));
                    }
                }
            }
        }
    }

    let by_path: HashMap<&str, &DirectorySummary> =
        directories.iter().map(|d| (d.path.as_str(), d)).collect();
    let total = anchors.len();
    for path in anchors.iter().take(limit) {
        let dir = by_path[path];
        match children.get(path) {
            Some(kids) if !kids.is_empty() => {
                let inlined = kids
                    .iter()
                    .map(|c| render_inline_child(&children, c))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    f,
                    "- {} ({} files, {} symbols): {}",
                    dir.path, dir.file_count, dir.symbol_count, inlined
                )?;
            }
            _ => writeln!(
                f,
                "- {} ({} files, {} symbols)",
                dir.path, dir.file_count, dir.symbol_count
            )?,
        }
    }
    if total > limit {
        writeln!(
            f,
            "_… {} more directory groups not shown; narrow with `overview <dir>`._",
            total - limit
        )?;
    }
    Ok(())
}

impl<'a> std::fmt::Display for RootCodemap<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.files.is_empty() && self.directories.is_empty() {
            return write!(f, "No files found.");
        }
        writeln!(f, "# Root Codemap Overview")?;
        writeln!(f)?;
        writeln!(f, "- **Total Files**: {}", self.total_files)?;
        writeln!(f, "- **Total Symbols**: {}", self.total_symbols)?;
        writeln!(f)?;

        if !self.directories.is_empty() {
            writeln!(
                f,
                "## Directories (recursive file/symbol counts; children inlined as `parent: childA, childB`; drill in with `overview <dir>`)"
            )?;
            write_directory_tree(f, &self.directories, "", ROOT_DIRECTORY_LIMIT)?;
            writeln!(f)?;
        }

        // Only repo-root-level files (no parent directory) are listed individually —
        // every nested file is reachable by drilling its directory. This is what keeps
        // the root view bounded on a large tree (the per-file dump was the bulk).
        let root_level_files: Vec<&ExtractedFileSummary<'a>> = self
            .files
            .iter()
            .filter(|file| !file.file_path.contains('/'))
            .collect();
        if !root_level_files.is_empty() {
            writeln!(f, "## Files (repo root)")?;
            for file in root_level_files {
                writeln!(
                    f,
                    "- File: {} ({} lines, {} symbols)",
                    file.file_path, file.total_lines, file.symbol_count
                )?;
            }
            writeln!(f)?;
        }

        writeln!(
            f,
            "_Tip: `overview <dir>` lists a folder's files; `search <keyword>` jumps straight to code by name._"
        )?;
        Ok(())
    }
}

impl<'a> CodemapView for RootCodemap<'a> {
    fn to_markdown(&self) -> String {
        self.to_string()
    }
}

impl<'a> std::fmt::Display for FolderCodemap<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "# Codemap: {}", self.folder_path)?;
        writeln!(f)?;
        writeln!(f, "- **Total Files**: {}", self.total_files)?;
        writeln!(f, "- **Total Symbols**: {}", self.total_symbols)?;
        writeln!(f)?;

        // Does any directory sit directly under this folder? (Drives the Sub-directories
        // section, which reuses the root view's folded, counted renderer scoped here.)
        let has_subdirs = self.directories.iter().any(|dir| {
            let parent = match dir.path.rfind('/') {
                Some(slash) => &dir.path[..slash],
                None => "",
            };
            parent == self.folder_path
        });

        if !has_subdirs && self.files.is_empty() {
            writeln!(f, "No files in this folder.")?;
            return Ok(());
        }

        if has_subdirs {
            writeln!(
                f,
                "## Sub-directories (recursive file/symbol counts; children inlined as `parent: childA, childB`; drill in with `overview <dir>`)"
            )?;
            write_directory_tree(f, &self.directories, &self.folder_path, ROOT_DIRECTORY_LIMIT)?;
            writeln!(f)?;
        }

        if !self.files.is_empty() {
            writeln!(f, "## Files")?;
            for file in &self.files {
                writeln!(
                    f,
                    "- File: {} ({} lines, {} symbols)",
                    file.file_path, file.total_lines, file.symbol_count
                )?;
                // Significant symbols only, names/kinds without line ranges: the folder
                // view orients within a known folder but is not a locator — exact
                // positions come from search/grep (overview must not substitute for search).
                for symbol in &file.symbols {
                    writeln!(f, "  - {} ({})", symbol.name, symbol.kind)?;
                }
            }
        }

        Ok(())
    }
}

impl<'a> CodemapView for FolderCodemap<'a> {
    fn to_markdown(&self) -> String {
        self.to_string()
    }
}

impl<'a> std::fmt::Display for DetailsCodemap<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "# Detailed Codemap: {} ({} lines)",
            self.file_path, self.total_lines
        )?;
        writeln!(f)?;

        writeln!(f, "## Symbols")?;
        for symbol in self
            .symbols
            .iter()
            .filter(|s| is_significant_symbol(s, self.symbols))
        {
            writeln!(
                f,
                "- {} ({}) [L{}-{}]",
                symbol.name, symbol.kind, symbol.range.start_line, symbol.range.end_line
            )?;
        }

        Ok(())
    }
}

impl<'a> CodemapView for DetailsCodemap<'a> {
    fn to_markdown(&self) -> String {
        self.to_string()
    }
}

fn normalize_path(path: &str) -> std::borrow::Cow<'_, str> {
    let needs_normalization = path.contains('\\')
        || path.starts_with("./")
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains("..")
        || path == "."
        || path.is_empty();

    if !needs_normalization {
        return std::borrow::Cow::Borrowed(path);
    }

    let replaced = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in replaced.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.pop();
        } else {
            parts.push(part);
        }
    }
    let normalized = parts.join("/");
    std::borrow::Cow::Owned(normalized)
}

pub struct CodemapGenerator;

impl CodemapGenerator {
    /// Root level: Overview of files and top-level definitions
    pub fn generate_root_view<'a>(files: &'a [crate::parser::ExtractedFile]) -> RootCodemap<'a> {
        let total_files = files.len();

        let files_summary: Vec<ExtractedFileSummary<'a>> =
            files.iter().map(summarize_file).collect();
        // Root counts the significant symbols actually surfaced downstream, not
        // the raw extracted total — so the headline matches the per-file sums.
        let total_symbols = files_summary.iter().map(|f| f.symbol_count).sum();

        let directories = build_directory_summaries(&files_summary);

        RootCodemap {
            total_files,
            total_symbols,
            directories,
            files: files_summary,
            original_files: files,
        }
    }

    /// Folder level: the same folded directory tree as the root view, scoped to
    /// `folder_path`, plus the files that live directly in that folder (with their
    /// symbols) — the files-with-symbols payload is what this view adds over the root.
    pub fn generate_folder_view<'a>(
        files: &'a [crate::parser::ExtractedFile],
        folder_path: &str,
    ) -> FolderCodemap<'a> {
        let normalized_folder = normalize_path(folder_path).into_owned();

        // Repo-wide summaries → repo-wide directory counts (so a directory reads the same
        // here as in the root view); the renderer scopes the output to this folder.
        let all_summaries: Vec<ExtractedFileSummary<'a>> =
            files.iter().map(summarize_file).collect();
        let directories = build_directory_summaries(&all_summaries);

        // Walk the summaries once: accumulate this folder's recursive totals and collect
        // the files that sit directly in it (parent directory == the folder).
        let prefix = format!("{}/", normalized_folder);
        let mut total_files = 0usize;
        let mut total_symbols = 0usize;
        let mut files_in_folder = Vec::new();
        for summary in &all_summaries {
            let under_folder = normalized_folder.is_empty()
                || summary.file_path == normalized_folder
                || summary.file_path.starts_with(&prefix);
            if !under_folder {
                continue;
            }
            total_files += 1;
            total_symbols += summary.symbol_count;

            let parent = match summary.file_path.rfind('/') {
                Some(slash) => &summary.file_path[..slash],
                None => "",
            };
            if parent == normalized_folder {
                files_in_folder.push(summary.clone());
            }
        }

        FolderCodemap {
            folder_path: normalized_folder,
            directories,
            total_files,
            total_symbols,
            files: files_in_folder,
            original_files: files,
        }
    }

    /// Detail level: Detailed symbols, literals, and docstrings for a specific file
    pub fn generate_detail_view<'a>(file: &'a crate::parser::ExtractedFile) -> DetailsCodemap<'a> {
        DetailsCodemap {
            file_path: file.file_path.clone(),
            total_lines: file.total_lines,
            symbols: &file.symbols,
        }
    }

    /// LLMS.txt format view
    pub fn generate_llms_txt_view(files: &[crate::parser::ExtractedFile]) -> String {
        use std::fmt::Write;
        let mut view = String::new();
        let _ = writeln!(view, "# llms.txt");
        let _ = writeln!(view);
        for file in files {
            let _ = writeln!(view, "- [{}]({})", file.file_path, file.file_path);
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{CodeRange, ExtractedFile, ExtractedSymbol, SymbolFlags};

    fn make_mock_file(path: &str, symbol_names: &[(&str, &str)]) -> ExtractedFile {
        let symbols = symbol_names
            .iter()
            .map(|(name, kind)| ExtractedSymbol {
                name: name.to_string(),
                kind: kind.to_string(),
                range: CodeRange {
                    start_line: 1,
                    start_col: 1,
                    end_line: 1,
                    end_col: 1,
                },
                docstring: None,
                flags: SymbolFlags {
                    has_todo: false,
                    has_fixme: false,
                    is_test: false,
                    is_exported: true,
                    is_deprecated: false,
                },
                owner: None,
            })
            .collect();

        ExtractedFile {
            file_path: path.to_string(),
            total_lines: 0,
            symbols,
            literals: vec![],
            docstrings: vec![],
        }
    }

    #[test]
    fn test_path_normalization() {
        assert_eq!(normalize_path(""), "");
        assert_eq!(normalize_path("."), "");
        assert_eq!(normalize_path("./"), "");
        assert_eq!(normalize_path("/"), "");
        assert_eq!(normalize_path("src/"), "src");
        assert_eq!(normalize_path("src\\core\\"), "src/core");
        assert_eq!(normalize_path(".\\src\\"), "src");
        assert_eq!(normalize_path("/src/core/"), "src/core");
    }

    #[test]
    fn test_root_codemap_empty() {
        let files = vec![];
        let root = CodemapGenerator::generate_root_view(&files);
        assert_eq!(root.to_markdown(), "No files found.");
        assert_eq!(format!("{}", root), "No files found.");
    }

    #[test]
    fn test_root_codemap_formatting() {
        let files = vec![
            make_mock_file("src/main.rs", &[("main", "fn")]),
            make_mock_file("src/lib.rs", &[("init", "fn")]),
        ];
        let root = CodemapGenerator::generate_root_view(&files);
        let formatted = format!("{}", root);
        assert!(formatted.contains("# Root Codemap Overview"));
        assert!(formatted.contains("- **Total Files**: 2"));
        assert!(formatted.contains("- **Total Symbols**: 2"));
        assert!(formatted.contains("## Directories"));
        // Root view is a directory skeleton with recursive counts, not a per-file dump:
        // both files live under `src`, so it rolls up to one directory row.
        assert!(formatted.contains("- src (2 files, 2 symbols)"));
        // Nested files are reachable by drilling their directory — they are not listed
        // individually at root (that per-file dump was the bloat we removed).
        assert!(!formatted.contains("- File: src/main.rs"));
        assert!(!formatted.contains("  - main (fn)"));
        assert!(!formatted.contains("[L"));
    }

    /// Pins the recursive directory inlining of the root view across all three child
    /// shapes on one tree:
    ///  - `leafA`        — a leaf directory → inlined bare on its parent's line.
    ///  - `group`        — a terminal-group (all children leaves) → inlined with its
    ///                     children in braces: `group (..): {leaf1 (..), leaf2 (..)}`.
    ///  - `junction`     — deep with NO leaf child of its own → gets no line; it is only
    ///                     bare-mentioned on `top`, and its grandchild `sub` (which has a
    ///                     leaf child) is promoted to its own anchor line.
    #[test]
    fn test_root_codemap_recursive_inlining() {
        let files = vec![
            make_mock_file("top/leafA/a.rs", &[("a", "fn")]),
            make_mock_file("top/group/leaf1/b.rs", &[("b", "fn")]),
            make_mock_file("top/group/leaf2/c.rs", &[("c", "fn")]),
            make_mock_file("top/junction/sub/leafX/d.rs", &[("d", "fn")]),
        ];
        let formatted = format!("{}", CodemapGenerator::generate_root_view(&files));

        // `top` is the only top-level anchor; it inlines all three children in path
        // (alphabetical) order — group, junction, leafA — with the terminal-group `group`
        // brace-wrapped so its members can't be confused with `top`'s own siblings.
        assert!(
            formatted.contains(
                "- top (4 files, 4 symbols): \
                 group (2 files, 2 symbols): {leaf1 (1 files, 1 symbols), leaf2 (1 files, 1 symbols)}, \
                 junction (1 files, 1 symbols), \
                 leafA (1 files, 1 symbols)"
            ),
            "top line did not inline children as expected:\n{formatted}"
        );
        // `junction` has no leaf child → it gets no anchor line of its own...
        assert!(
            !formatted.contains("- top/junction ("),
            "deep junction with no leaf child must not get its own line:\n{formatted}"
        );
        // ...its grandchild `sub` is promoted instead.
        assert!(
            formatted.contains("- top/junction/sub (1 files, 1 symbols): leafX (1 files, 1 symbols)"),
            "sub should be promoted to an anchor line:\n{formatted}"
        );
        // The terminal-group `group` is inlined into `top`, never a standalone anchor.
        assert!(
            !formatted.contains("- top/group ("),
            "terminal-group child must be inlined, not anchored:\n{formatted}"
        );
    }

    #[test]
    fn test_folder_codemap_synthesis() {
        let files = vec![
            make_mock_file("src/main.rs", &[("main", "fn")]),
            make_mock_file("src/utils/mod.rs", &[("utils", "fn")]),
            make_mock_file("src/utils/math.rs", &[("add", "fn")]),
            make_mock_file("src/core/engine/mod.rs", &[("run", "fn")]),
            make_mock_file("Cargo.toml", &[]),
            make_mock_file("src_helper/lib.rs", &[("helper", "fn")]),
        ];

        // Immediate subdirectories of `scope`, derived from the repo-wide directory list
        // the folder view now carries (the old `subdirectories` field is gone).
        fn immediate_children(dirs: &[DirectorySummary], scope: &str) -> Vec<String> {
            let mut v: Vec<String> = dirs
                .iter()
                .filter(|d| {
                    let parent = match d.path.rfind('/') {
                        Some(slash) => &d.path[..slash],
                        None => "",
                    };
                    parent == scope
                })
                .map(|d| d.path.clone())
                .collect();
            v.sort();
            v
        }

        // Root folder view synthesis
        let root_folder = CodemapGenerator::generate_folder_view(&files, "");
        assert_eq!(root_folder.files.len(), 1);
        assert_eq!(root_folder.files[0].file_path, "Cargo.toml");
        assert_eq!(
            immediate_children(&root_folder.directories, ""),
            vec!["src".to_string(), "src_helper".to_string()]
        );

        // "src" folder view synthesis
        let src_folder = CodemapGenerator::generate_folder_view(&files, "src");
        assert_eq!(src_folder.files.len(), 1);
        assert_eq!(src_folder.files[0].file_path, "src/main.rs");
        assert_eq!(
            immediate_children(&src_folder.directories, "src"),
            vec!["src/core".to_string(), "src/utils".to_string()]
        );

        // Trailing slash path normalization (folder_path is stored normalized).
        let src_folder_slash = CodemapGenerator::generate_folder_view(&files, "src/");
        assert_eq!(src_folder_slash.folder_path, "src");
        assert_eq!(src_folder_slash.files.len(), 1);
        assert_eq!(src_folder_slash.files[0].file_path, "src/main.rs");
        assert_eq!(
            immediate_children(&src_folder_slash.directories, "src"),
            vec!["src/core".to_string(), "src/utils".to_string()]
        );

        // Windows path backslash normalization
        let src_folder_win = CodemapGenerator::generate_folder_view(&files, "src\\");
        assert_eq!(src_folder_win.folder_path, "src");
        assert_eq!(src_folder_win.files.len(), 1);
        assert_eq!(
            immediate_children(&src_folder_win.directories, "src"),
            vec!["src/core".to_string(), "src/utils".to_string()]
        );
    }

    /// The folder view reuses the root view's folded, counted directory renderer scoped
    /// to the folder, and adds a Files section for files directly in it. Pins: the
    /// scope+totals header, the same leaf / terminal-group(braces) / deep-no-leaf-promotion
    /// rendering as the root, and the file line carrying its symbol count + bullets.
    #[test]
    fn test_folder_codemap_recursive_inlining() {
        let files = vec![
            make_mock_file("mod/direct.rs", &[("direct", "fn")]),
            make_mock_file("mod/leafA/a.rs", &[("a", "fn")]),
            make_mock_file("mod/group/leaf1/b.rs", &[("b", "fn")]),
            make_mock_file("mod/group/leaf2/c.rs", &[("c", "fn")]),
            make_mock_file("mod/junction/sub/leafX/d.rs", &[("d", "fn")]),
        ];
        let folder = CodemapGenerator::generate_folder_view(&files, "mod");
        let formatted = format!("{}", folder);

        // Scope header + recursive totals (5 files / 5 symbols under `mod`).
        assert!(formatted.contains("# Codemap: mod"), "{formatted}");
        assert!(formatted.contains("- **Total Files**: 5"), "{formatted}");
        assert!(formatted.contains("- **Total Symbols**: 5"), "{formatted}");

        // Sub-directories: same folding as root, scoped under `mod`. Immediate children of
        // the scope are always anchors (like top-level dirs in the root view), so each of
        // group / junction / leafA gets its own line; a *nested* terminal-group (`sub`
        // under `junction`) is the one that gets brace-wrapped.
        assert!(
            formatted
                .contains("- mod/group (2 files, 2 symbols): leaf1 (1 files, 1 symbols), leaf2 (1 files, 1 symbols)"),
            "scope-immediate anchor lists its children directly:\n{formatted}"
        );
        assert!(
            formatted.contains(
                "- mod/junction (1 files, 1 symbols): sub (1 files, 1 symbols): {leafX (1 files, 1 symbols)}"
            ),
            "a nested terminal-group should fold with braces:\n{formatted}"
        );
        assert!(formatted.contains("- mod/leafA (1 files, 1 symbols)"), "{formatted}");

        // Files section: files directly in the folder, with symbol count + symbol bullets
        // (no line ranges — that stays search/grep's job).
        assert!(
            formatted.contains("## Files")
                && formatted.contains("- File: mod/direct.rs (0 lines, 1 symbols)")
                && formatted.contains("  - direct (fn)"),
            "files section missing or mis-formatted:\n{formatted}"
        );
        // A file deep in a subdir is NOT relisted in this folder's Files section.
        assert!(!formatted.contains("- File: mod/leafA/a.rs"), "{formatted}");
    }

    #[test]
    fn test_folder_codemap_empty() {
        let files = vec![];
        let folder = CodemapGenerator::generate_folder_view(&files, "src");
        let formatted = format!("{}", folder);
        assert!(formatted.contains("No files in this folder."));
    }

    #[test]
    fn test_details_codemap_formatting() {
        let file = ExtractedFile {
            file_path: "src/lib.rs".to_string(),
            total_lines: 12,
            symbols: vec![ExtractedSymbol {
                name: "check".to_string(),
                kind: "fn".to_string(),
                range: CodeRange {
                    start_line: 5,
                    start_col: 1,
                    end_line: 10,
                    end_col: 2,
                },
                docstring: Some("A check function\nwith multiple lines".to_string()),
                flags: SymbolFlags {
                    has_todo: true,
                    has_fixme: false,
                    is_test: false,
                    is_exported: true,
                    is_deprecated: true,
                },
                owner: None,
            }],
            literals: vec![crate::parser::ExtractedLiteral { text: "magic_value".to_string(), line: 1 }],
            docstrings: vec!["A check function\nwith multiple lines".to_string()],
        };

        let details = CodemapGenerator::generate_detail_view(&file);
        let formatted = format!("{}", details);
        assert!(formatted.contains("# Detailed Codemap: src/lib.rs"));
        assert!(formatted.contains("## Symbols"));
        // File view is a trimmed outline: name, kind, line range only.
        assert!(formatted.contains("- check (fn) [L5-10]"));
        // Verbose details (flags, docstrings, literals) belong to read/grep, not overview.
        assert!(!formatted.contains("Docstring"));
        assert!(!formatted.contains("Flags:"));
        assert!(!formatted.contains("## Literals"));
        assert!(!formatted.contains("magic_value"));
    }

    #[test]
    fn test_normalize_path_relative_navigation() {
        let files = vec![
            make_mock_file("src/../lib.rs", &[("init", "fn")]),
            make_mock_file("src/core/../utils/math.rs", &[("add", "fn")]),
        ];
        let root = CodemapGenerator::generate_root_view(&files);

        assert!(!root.directories.iter().any(|d| d.path == "src/.."));
        assert!(root.directories.iter().any(|d| d.path == "src/utils"));
        assert_eq!(root.files[0].file_path, "lib.rs");
    }
}
