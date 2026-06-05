use std::collections::BTreeSet;

pub trait CodemapView {
    fn to_markdown(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct ExtractedSymbolSummary<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub is_exported: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractedFileSummary<'a> {
    pub file_path: String,
    pub symbol_count: usize,
    pub symbols: Vec<ExtractedSymbolSummary<'a>>,
}

#[derive(Debug, Clone)]
pub struct RootCodemap<'a> {
    pub total_files: usize,
    pub total_symbols: usize,
    pub directories: Vec<String>,
    pub files: Vec<ExtractedFileSummary<'a>>,
    // Store reference to original files if needed, or define with references
    pub original_files: &'a [crate::parser::ExtractedFile],
}

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
    pub symbols: &'a [crate::parser::ExtractedSymbol],
    pub literals: &'a [String],
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
            writeln!(f, "## Directories")?;
            for dir in &self.directories {
                writeln!(f, "- {}", dir)?;
            }
            writeln!(f)?;
        }

        if !self.files.is_empty() {
            writeln!(f, "## Files")?;
            for file in &self.files {
                writeln!(f, "- File: {}", file.file_path)?;
                for symbol in &file.symbols {
                    writeln!(f, "  - {} ({})", symbol.name, symbol.kind)?;
                }
            }
        }
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
                writeln!(f, "- File: {}", file.file_path)?;
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
        writeln!(f, "# Detailed Codemap: {}", self.file_path)?;
        writeln!(f)?;

        writeln!(f, "## Extracted Symbols")?;
        for symbol in self.symbols {
            writeln!(
                f,
                "- **Name**: {}\n  Kind: {}\n  Range: {}:{} - {}:{}",
                symbol.name,
                symbol.kind,
                symbol.range.start_line,
                symbol.range.start_col,
                symbol.range.end_line,
                symbol.range.end_col
            )?;
            if let Some(ref doc) = symbol.docstring {
                let mut lines = doc.lines();
                if let Some(first) = lines.next() {
                    writeln!(f, "  Docstring: {}", first)?;
                    for line in lines {
                        writeln!(f, "            {}", line)?;
                    }
                }
            }
            writeln!(
                f,
                "  Flags: hasTodo={}, hasFixme={}, isTest={}, isExported={}, isDeprecated={}",
                symbol.flags.has_todo,
                symbol.flags.has_fixme,
                symbol.flags.is_test,
                symbol.flags.is_exported,
                symbol.flags.is_deprecated,
            )?;
        }

        writeln!(f)?;
        writeln!(f, "## Literals")?;
        for lit in self.literals {
            writeln!(f, "- {}", lit)?;
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
        let total_symbols = files.iter().map(|f| f.symbols.len()).sum();
        
        let mut dirs_set = BTreeSet::new();
        for file in files {
            let normalized_file = normalize_path(&file.file_path);
            let parts: Vec<&str> = normalized_file.split('/').collect();
            for i in 1..parts.len() {
                let dir_path = parts[0..i].join("/");
                if !dir_path.is_empty() {
                    dirs_set.insert(dir_path);
                }
            }
        }
        let directories: Vec<String> = dirs_set.into_iter().collect();

        let files_summary: Vec<ExtractedFileSummary<'a>> = files.iter().map(|file| {
            ExtractedFileSummary {
                file_path: normalize_path(&file.file_path).into_owned(),
                symbol_count: file.symbols.len(),
                symbols: file.symbols.iter().map(|s| {
                    ExtractedSymbolSummary {
                        name: &s.name,
                        kind: &s.kind,
                        is_exported: s.flags.is_exported,
                    }
                }).collect(),
            }
        }).collect();

        RootCodemap {
            total_files,
            total_symbols,
            directories,
            files: files_summary,
            original_files: files,
        }
    }

    /// Folder level: Sub-directory specific view synthesized on the fly
    pub fn generate_folder_view<'a>(files: &'a [crate::parser::ExtractedFile], folder_path: &str) -> FolderCodemap<'a> {
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
                    files_summary.push(ExtractedFileSummary {
                        file_path: normalize_path(&file.file_path).into_owned(),
                        symbol_count: file.symbols.len(),
                        symbols: file.symbols.iter().map(|s| ExtractedSymbolSummary {
                            name: &s.name,
                            kind: &s.kind,
                            is_exported: s.flags.is_exported,
                        }).collect(),
                    });
                } else {
                    let rel = if normalized_folder.is_empty() {
                        normalized_file.as_ref()
                    } else {
                        &normalized_file[normalized_folder.len() + 1..]
                    };
 
                    let parts: Vec<&str> = rel.split('/').collect();
                    if parts.len() == 1 {
                        files_summary.push(ExtractedFileSummary {
                            file_path: normalize_path(&file.file_path).into_owned(),
                            symbol_count: file.symbols.len(),
                            symbols: file.symbols.iter().map(|s| ExtractedSymbolSummary {
                                name: &s.name,
                                kind: &s.kind,
                                is_exported: s.flags.is_exported,
                            }).collect(),
                        });
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
            symbols: &file.symbols,
            literals: &file.literals,
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
    use crate::parser::{CodeRange, SymbolFlags, ExtractedSymbol, ExtractedFile};

    fn make_mock_file(path: &str, symbol_names: &[(&str, &str)]) -> ExtractedFile {
        let symbols = symbol_names.iter().map(|(name, kind)| {
            ExtractedSymbol {
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
            }
        }).collect();

        ExtractedFile {
            file_path: path.to_string(),
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
        assert!(formatted.contains("- src"));
        assert!(formatted.contains("## Files"));
        assert!(formatted.contains("- File: src/main.rs"));
        assert!(formatted.contains("  - main (fn)"));
        assert!(formatted.contains("- File: src/lib.rs"));
        assert!(formatted.contains("  - init (fn)"));
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
        assert_eq!(root_folder.subdirectories, vec!["src".to_string(), "src_helper".to_string()]);

        // "src" folder view synthesis
        let src_folder = CodemapGenerator::generate_folder_view(&files, "src");
        assert_eq!(src_folder.files.len(), 1);
        assert_eq!(src_folder.files[0].file_path, "src/main.rs");
        assert_eq!(src_folder.subdirectories, vec!["src/core".to_string(), "src/utils".to_string()]);

        // Trailing slash path normalization
        let src_folder_slash = CodemapGenerator::generate_folder_view(&files, "src/");
        assert_eq!(src_folder_slash.files.len(), 1);
        assert_eq!(src_folder_slash.files[0].file_path, "src/main.rs");
        assert_eq!(src_folder_slash.subdirectories, vec!["src/core".to_string(), "src/utils".to_string()]);

        // Windows path backslash normalization
        let src_folder_win = CodemapGenerator::generate_folder_view(&files, "src\\");
        assert_eq!(src_folder_win.files.len(), 1);
        assert_eq!(src_folder_win.subdirectories, vec!["src/core".to_string(), "src/utils".to_string()]);
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
        assert!(formatted.contains("## Extracted Symbols"));
        assert!(formatted.contains("- **Name**: check"));
        assert!(formatted.contains("  Kind: fn"));
        assert!(formatted.contains("  Range: 5:1 - 10:2"));
        assert!(formatted.contains("  Docstring: A check function"));
        assert!(formatted.contains("            with multiple lines"));
        assert!(formatted.contains("  Flags: hasTodo=true, hasFixme=false, isTest=false, isExported=true, isDeprecated=true"));
        assert!(formatted.contains("## Literals"));
        assert!(formatted.contains("- magic_value"));
    }

    #[test]
    fn test_normalize_path_relative_navigation() {
        let files = vec![
            make_mock_file("src/../lib.rs", &[("init", "fn")]),
            make_mock_file("src/core/../utils/math.rs", &[("add", "fn")]),
        ];
        let root = CodemapGenerator::generate_root_view(&files);
        
        assert!(!root.directories.contains(&"src/..".to_string()));
        assert!(root.directories.contains(&"src/utils".to_string()));
        assert_eq!(root.files[0].file_path, "lib.rs");
    }
}
