mod summary;
mod tree;

use summary::{build_directory_summaries, is_significant_symbol, summarize_file};
pub use summary::{DirectorySummary, ExtractedFileSummary, ExtractedSymbolSummary};
use tree::write_directory_tree;

pub trait CodemapView {
    fn to_markdown(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct RootCodemap<'a> {
    pub total_files: usize,
    pub total_symbols: usize,
    pub directories: Vec<DirectorySummary>,
    pub files: Vec<ExtractedFileSummary<'a>>,
}

/// Max directory rows the root view emits before truncating with a footer. Directory rows
/// can inline large child groups, so keep the root-first orientation bounded and let callers
/// drill into a subtree for the complete directory map.
const ROOT_DIRECTORY_LIMIT: usize = 60;
/// Max recursive file rows in the root view. Folder and file detail views remain the
/// continuation path for complete listings and line ranges.
const ROOT_FILE_SUMMARY_LIMIT: usize = 60;
/// Max symbol names shown per kind inside a compact root file row.
const ROOT_SYMBOLS_PER_KIND_LIMIT: usize = 4;
/// Max file links in llms-txt root output. The full file set remains discoverable through
/// folder overview and find; llms-txt is a first-call orientation surface, not an index dump.
const LLMS_TXT_FILE_LIMIT: usize = 200;

fn grouped_symbol_summary(file: &ExtractedFileSummary<'_>) -> Option<String> {
    let mut by_kind: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for symbol in &file.symbols {
        by_kind.entry(symbol.kind).or_default().push(symbol.name);
    }
    if by_kind.is_empty() {
        return None;
    }

    let mut groups = Vec::new();
    for (kind, mut names) in by_kind {
        names.sort_unstable();
        names.dedup();
        let total = names.len();
        let shown: Vec<&str> = names
            .into_iter()
            .take(ROOT_SYMBOLS_PER_KIND_LIMIT)
            .collect();
        let mut group = format!("{kind}: {}", shown.join(", "));
        if total > shown.len() {
            group.push_str(&format!(", +{} more", total - shown.len()));
        }
        groups.push(group);
    }
    Some(format!("{{{}}}", groups.join("; ")))
}

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
}

#[derive(Debug, Clone)]
pub struct DetailsCodemap<'a> {
    pub file_path: String,
    pub total_lines: usize,
    pub symbols: &'a [crate::parser::ExtractedSymbol],
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

        if !self.files.is_empty() {
            writeln!(
                f,
                "## Files (first {} by path; grouped significant symbols, no line ranges)",
                ROOT_FILE_SUMMARY_LIMIT.min(self.files.len())
            )?;
            for file in self.files.iter().take(ROOT_FILE_SUMMARY_LIMIT) {
                let symbols = grouped_symbol_summary(file)
                    .map(|summary| format!(" {summary}"))
                    .unwrap_or_default();
                writeln!(
                    f,
                    "- File: {} ({} lines, {} symbols){}",
                    file.file_path, file.total_lines, file.symbol_count, symbols
                )?;
            }
            if self.files.len() > ROOT_FILE_SUMMARY_LIMIT {
                writeln!(
                    f,
                    "- _Showing files 1-{} of {}; continue with `overview <dir>` for the relevant subtree or `find` for exact file enumeration._",
                    ROOT_FILE_SUMMARY_LIMIT,
                    self.files.len()
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
            write_directory_tree(
                f,
                &self.directories,
                &self.folder_path,
                ROOT_DIRECTORY_LIMIT,
            )?;
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

        let mut files_summary: Vec<ExtractedFileSummary<'a>> =
            files.iter().map(summarize_file).collect();
        files_summary.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        // Root counts significant symbols after per-file summarization, not the
        // raw extracted total; the later row caps only limit what is printed.
        let total_symbols = files_summary.iter().map(|f| f.symbol_count).sum();

        let directories = build_directory_summaries(&files_summary);

        RootCodemap {
            total_files,
            total_symbols,
            directories,
            files: files_summary,
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
        let mut all_summaries: Vec<ExtractedFileSummary<'a>> =
            files.iter().map(summarize_file).collect();
        all_summaries.sort_by(|a, b| a.file_path.cmp(&b.file_path));
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
        let mut sorted_files: Vec<&crate::parser::ExtractedFile> = files.iter().collect();
        sorted_files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        let shown = sorted_files.len().min(LLMS_TXT_FILE_LIMIT);
        let _ = writeln!(
            view,
            "> Showing the first {shown} of {} indexed files by path.",
            sorted_files.len()
        );
        let _ = writeln!(view);
        for file in sorted_files.into_iter().take(LLMS_TXT_FILE_LIMIT) {
            let _ = writeln!(view, "- [{}]({})", file.file_path, file.file_path);
        }
        if files.len() > LLMS_TXT_FILE_LIMIT {
            let _ = writeln!(
                view,
                "- _{} more files not shown; continue with `overview <dir>` or `find`._",
                files.len() - LLMS_TXT_FILE_LIMIT
            );
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
        // Root view keeps the directory skeleton and adds bounded compact file-symbol rows.
        assert!(formatted.contains("- src (2 files, 2 symbols)"));
        assert!(formatted.contains("- File: src/main.rs (0 lines, 1 symbols) {fn: main}"));
        assert!(formatted.contains("- File: src/lib.rs (0 lines, 1 symbols) {fn: init}"));
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
            formatted
                .contains("- top/junction/sub (1 files, 1 symbols): leafX (1 files, 1 symbols)"),
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
        assert!(
            formatted.contains("- mod/leafA (1 files, 1 symbols)"),
            "{formatted}"
        );

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
            literals: vec![crate::parser::ExtractedLiteral {
                text: "magic_value".to_string(),
                line: 1,
            }],
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
