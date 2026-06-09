use std::collections::BTreeSet;

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
    pub subdirectories: Vec<String>,
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
fn range_strictly_contains(
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
                "## Directories (files / symbols beneath each — drill in with `overview <dir>`)"
            )?;
            for dir in self.directories.iter().take(ROOT_DIRECTORY_LIMIT) {
                writeln!(
                    f,
                    "- {} ({} files, {} symbols)",
                    dir.path, dir.file_count, dir.symbol_count
                )?;
            }
            if self.directories.len() > ROOT_DIRECTORY_LIMIT {
                writeln!(
                    f,
                    "_… {} more directories not shown; narrow with `overview <dir>`._",
                    self.directories.len() - ROOT_DIRECTORY_LIMIT
                )?;
            }
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
        writeln!(f, "# Folder Codemap: {}", self.folder_path)?;
        writeln!(f)?;

        if self.subdirectories.is_empty() && self.files.is_empty() {
            writeln!(f, "No files in this folder.")?;
            return Ok(());
        }

        if !self.subdirectories.is_empty() {
            writeln!(f, "## Sub-directories")?;
            for dir in &self.subdirectories {
                writeln!(f, "- {}", dir)?;
            }
            writeln!(f)?;
        }

        if !self.files.is_empty() {
            writeln!(f, "## Files")?;
            for file in &self.files {
                writeln!(f, "- File: {} ({} lines)", file.file_path, file.total_lines)?;
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

        // Aggregate each ancestor directory's recursive file + significant-symbol
        // counts in one pass over the summaries. Every prefix of a file's path is an
        // ancestor, so a file in `src/a/b.ts` credits `src` and `src/a`. The counts
        // let the agent pick the relevant subtree from the root view without the
        // per-file dump (BTreeMap keeps the output path-sorted, matching the prior
        // directory ordering).
        let mut dir_counts: std::collections::BTreeMap<String, (usize, usize)> =
            std::collections::BTreeMap::new();
        for file in &files_summary {
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
        let directories: Vec<DirectorySummary> = dir_counts
            .into_iter()
            .map(|(path, (file_count, symbol_count))| DirectorySummary {
                path,
                file_count,
                symbol_count,
            })
            .collect();

        RootCodemap {
            total_files,
            total_symbols,
            directories,
            files: files_summary,
            original_files: files,
        }
    }

    /// Folder level: Sub-directory specific view synthesized on the fly
    pub fn generate_folder_view<'a>(
        files: &'a [crate::parser::ExtractedFile],
        folder_path: &str,
    ) -> FolderCodemap<'a> {
        let normalized_folder = normalize_path(folder_path);
        let mut sub_dirs = BTreeSet::new();
        let mut files_summary = Vec::new();

        let prefix = format!("{}/", normalized_folder);

        for file in files {
            let normalized_file = normalize_path(&file.file_path);
            let is_match = if normalized_folder.is_empty() {
                true
            } else {
                normalized_file == normalized_folder || normalized_file.starts_with(&prefix)
            };

            if is_match {
                if normalized_file == normalized_folder {
                    files_summary.push(summarize_file(file));
                } else {
                    let rel = if normalized_folder.is_empty() {
                        normalized_file.as_ref()
                    } else {
                        &normalized_file[normalized_folder.len() + 1..]
                    };

                    let parts: Vec<&str> = rel.split('/').collect();
                    if parts.len() == 1 {
                        files_summary.push(summarize_file(file));
                    } else if parts.len() > 1 {
                        let first_component = parts[0];
                        let sub_dir = if normalized_folder.is_empty() {
                            first_component.to_string()
                        } else {
                            format!("{}/{}", normalized_folder, first_component)
                        };
                        sub_dirs.insert(sub_dir);
                    }
                }
            }
        }

        FolderCodemap {
            folder_path: folder_path.to_string(),
            subdirectories: sub_dirs.into_iter().collect(),
            files: files_summary,
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

        // Root folder view synthesis
        let root_folder = CodemapGenerator::generate_folder_view(&files, "");
        assert_eq!(root_folder.files.len(), 1);
        assert_eq!(root_folder.files[0].file_path, "Cargo.toml");
        assert_eq!(
            root_folder.subdirectories,
            vec!["src".to_string(), "src_helper".to_string()]
        );

        // "src" folder view synthesis
        let src_folder = CodemapGenerator::generate_folder_view(&files, "src");
        assert_eq!(src_folder.files.len(), 1);
        assert_eq!(src_folder.files[0].file_path, "src/main.rs");
        assert_eq!(
            src_folder.subdirectories,
            vec!["src/core".to_string(), "src/utils".to_string()]
        );

        // Trailing slash path normalization
        let src_folder_slash = CodemapGenerator::generate_folder_view(&files, "src/");
        assert_eq!(src_folder_slash.files.len(), 1);
        assert_eq!(src_folder_slash.files[0].file_path, "src/main.rs");
        assert_eq!(
            src_folder_slash.subdirectories,
            vec!["src/core".to_string(), "src/utils".to_string()]
        );

        // Windows path backslash normalization
        let src_folder_win = CodemapGenerator::generate_folder_view(&files, "src\\");
        assert_eq!(src_folder_win.files.len(), 1);
        assert_eq!(
            src_folder_win.subdirectories,
            vec!["src/core".to_string(), "src/utils".to_string()]
        );
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
            }],
            literals: vec!["magic_value".to_string()],
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
