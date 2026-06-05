use crate::parser::{ExtractedSymbol, ExtractedFile, CodeExtractor};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, ReloadPolicy, Term, TantivyDocument, IndexSettings};
use tantivy::query::{AllQuery, QueryParser};
use tantivy::collector::TopDocs;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub score: f32,
    pub matched_symbols: Vec<ExtractedSymbol>,
    pub matched_literals: Vec<String>,
}

pub trait SearchEngine {
    fn index_files(&mut self, paths: &[&str]) -> Result<(), String>;
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String>;
}

pub struct TantivySearchEngine {
    pub index_path: String,
    pub schema: Schema,
    pub index: Index,
    pub reader: IndexReader,
    
    // Schema field references
    pub file_path_field: Field,
    pub file_path_parts_field: Field,
    pub symbol_field: Field,
    pub docstring_field: Field,
    pub literal_field: Field,
    pub extracted_json_field: Field,
    pub mtime_field: Field,
}

impl TantivySearchEngine {
    pub fn new(index_path: &str) -> Result<Self, String> {
        let mut schema_builder = Schema::builder();
        let file_path_field = schema_builder.add_text_field("file_path", STRING | STORED);
        let file_path_parts_field = schema_builder.add_text_field("file_path_parts", TEXT);
        let symbol_field = schema_builder.add_text_field("symbol", TEXT | STORED);
        let docstring_field = schema_builder.add_text_field("docstring", TEXT | STORED);
        let literal_field = schema_builder.add_text_field("literal", TEXT | STORED);
        let extracted_json_field = schema_builder.add_text_field("extracted_json", STORED);
        let mtime_field = schema_builder.add_u64_field("mtime", STORED);
        let schema = schema_builder.build();

        let path = Path::new(index_path);
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        }

        // Try to open or create index directory. Auto-rebuild if metadata is corrupted.
        let index = match tantivy::directory::MmapDirectory::open(path)
            .map_err(|e| e.to_string())
            .and_then(|dir| Index::open_or_create(dir, schema.clone()).map_err(|e| e.to_string()))
        {
            Ok(idx) => idx,
            Err(_) => {
                let _ = std::fs::remove_dir_all(path);
                std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
                let directory = tantivy::directory::MmapDirectory::open(path).map_err(|e| e.to_string())?;
                Index::create(directory, schema.clone(), IndexSettings::default()).map_err(|e| e.to_string())?
            }
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            index_path: index_path.to_string(),
            schema,
            index,
            reader,
            file_path_field,
            file_path_parts_field,
            symbol_field,
            docstring_field,
            literal_field,
            extracted_json_field,
            mtime_field,
        })
    }

    fn get_indexed_mtimes(&self) -> HashMap<String, u64> {
        let searcher = self.reader.searcher();
        let mut map = HashMap::new();
        
        if let Ok(top_docs) = searcher.search(&AllQuery, &TopDocs::with_limit(100_000)) {
            for (_score, doc_address) in top_docs {
                if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                    let path_val = doc.get_first(self.file_path_field);
                    let mtime_val = doc.get_first(self.mtime_field);
                    if let (Some(p_val), Some(m_val)) = (path_val, mtime_val) {
                        let path = p_val.as_str().unwrap_or("").to_string();
                        let mtime = m_val.as_u64().unwrap_or(0);
                        if !path.is_empty() {
                            map.insert(path, mtime);
                        }
                    }
                }
            }
        }
        map
    }

    #[allow(dead_code)]
    fn check_mtime(&self, _file_path: &str) -> bool {
        true
    }
}

fn tokenize_path(file_path: &str) -> String {
    let mut tokens = Vec::new();
    for part in file_path.split(|c| c == '/' || c == '\\') {
        if part.is_empty() {
            continue;
        }
        if part.contains('.') {
            let subparts: Vec<&str> = part.split('.').filter(|s| !s.is_empty()).collect();
            for sp in subparts {
                tokens.push(sp.to_string());
            }
        } else {
            tokens.push(part.to_string());
        }
    }
    tokens.join(" ")
}

fn normalize_relative_path(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    let replaced = s.replace('\\', "/");
    let mut trimmed = replaced.as_str();
    while trimmed.starts_with("./") {
        trimmed = &trimmed[2..];
    }
    while trimmed.starts_with('/') {
        trimmed = &trimmed[1..];
    }
    trimmed.to_string()
}

impl SearchEngine for TantivySearchEngine {
    fn index_files(&mut self, paths: &[&str]) -> Result<(), String> {
        let mut files_to_process = Vec::new();
        
        for path_str in paths {
            let path = Path::new(path_str);
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if matches!(ext, "rs" | "py" | "ts" | "tsx" | "js" | "jsx") {
                        if let Ok(metadata) = std::fs::metadata(path) {
                            if let Ok(modified) = metadata.modified() {
                                if let Ok(duration) = modified.duration_since(std::time::SystemTime::UNIX_EPOCH) {
                                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                                    let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                                    let abs_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
                                    let rel = abs_path.strip_prefix(&abs_cwd).unwrap_or(path);
                                    let rel_path = normalize_relative_path(rel);
                                    files_to_process.push((rel_path, path.to_path_buf(), duration.as_secs()));
                                }
                            }
                        }
                    }
                }
            } else if path.is_dir() {
                for entry in walkdir::WalkDir::new(path)
                    .into_iter()
                    .filter_entry(|e| {
                        if e.depth() == 0 {
                            true
                        } else {
                            let name = e.file_name().to_string_lossy();
                            if e.file_type().is_dir() {
                                !name.starts_with('.')
                            } else {
                                true
                            }
                        }
                    })
                    .filter_map(|e| e.ok())
                {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(ext) = entry_path.extension().and_then(|s| s.to_str()) {
                            if matches!(ext, "rs" | "py" | "ts" | "tsx" | "js" | "jsx") {
                                if let Ok(metadata) = std::fs::metadata(entry_path) {
                                    if let Ok(modified) = metadata.modified() {
                                        if let Ok(duration) = modified.duration_since(std::time::SystemTime::UNIX_EPOCH) {
                                            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                                            let abs_path = entry_path.canonicalize().unwrap_or_else(|_| entry_path.to_path_buf());
                                            let abs_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
                                            let rel = abs_path.strip_prefix(&abs_cwd).unwrap_or(entry_path);
                                            let rel_path = normalize_relative_path(rel);
                                            files_to_process.push((rel_path, entry_path.to_path_buf(), duration.as_secs()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let indexed_mtimes = self.get_indexed_mtimes();
        let disk_file_paths: std::collections::HashSet<String> = files_to_process.iter()
            .map(|(rel_path, _, _)| rel_path.clone())
            .collect();

        let mut to_delete = Vec::new();
        for indexed_path in indexed_mtimes.keys() {
            if !disk_file_paths.contains(indexed_path) {
                to_delete.push(indexed_path.clone());
            }
        }

        let files_to_process_len = files_to_process.len();
        let mut to_index = Vec::new();
        for (rel_path, disk_path, mtime) in files_to_process {
            match indexed_mtimes.get(&rel_path) {
                Some(&indexed_mtime) if indexed_mtime == mtime => {
                    // Skip indexing: mtime hasn't changed
                }
                _ => {
                    to_index.push((rel_path, disk_path, mtime));
                }
            }
        }

        println!("DEBUG index_files: files_to_process={}, to_index={}, to_delete={}", files_to_process_len, to_index.len(), to_delete.len());

        // Return early if no updates (adds or deletes) to avoid touching index and triggering modification
        if to_index.is_empty() && to_delete.is_empty() {
            return Ok(());
        }

        let extractor = crate::parser::TreeSitterExtractor::new();
        let mut writer = match self.index.writer(50_000_000) {
            Ok(w) => w,
            Err(tantivy::TantivyError::LockFailure(e, _)) => {
                println!("DEBUG index_files LockFailure: {:?}", e);
                return Ok(());
            }
            Err(e) => return Err(e.to_string()),
        };

        for rel_path in to_delete {
            let term = Term::from_field_text(self.file_path_field, &rel_path);
            writer.delete_term(term);
        }

        for (rel_path, disk_path, mtime) in to_index {
            let content = match std::fs::read_to_string(&disk_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Warning: Failed to read file {}: {}", disk_path.display(), e);
                    continue;
                }
            };
            let extracted = match extractor.extract(&content, &rel_path) {
                Ok(ext) => ext,
                Err(e) => {
                    eprintln!("Warning: Failed to parse file {}: {}", disk_path.display(), e);
                    continue;
                }
            };

            let term = Term::from_field_text(self.file_path_field, &rel_path);
            writer.delete_term(term);

            let mut doc = TantivyDocument::default();
            doc.add_text(self.file_path_field, &rel_path);
            
            let path_parts = tokenize_path(&rel_path);
            doc.add_text(self.file_path_parts_field, &path_parts);

            let json_str = match serde_json::to_string(&extracted) {
                Ok(js) => js,
                Err(e) => {
                    eprintln!("Warning: Failed to serialize extracted symbols for {}: {}", disk_path.display(), e);
                    continue;
                }
            };
            doc.add_text(self.extracted_json_field, &json_str);
            doc.add_u64(self.mtime_field, mtime);

            for sym in &extracted.symbols {
                doc.add_text(self.symbol_field, &sym.name);
                let sub_tokens = crate::parser::split_identifier(&sym.name);
                for token in sub_tokens {
                    doc.add_text(self.symbol_field, &token);
                }
                if let Some(ref docstring) = sym.docstring {
                    doc.add_text(self.docstring_field, docstring);
                }
            }

            for lit in &extracted.literals {
                doc.add_text(self.literal_field, lit);
            }

            if let Err(e) = writer.add_document(doc) {
                eprintln!("Warning: Failed to add document to index for {}: {}", disk_path.display(), e);
                continue;
            }
        }

        writer.commit().map_err(|e| e.to_string())?;
        self.reader.reload().map_err(|e| e.to_string())?;

        Ok(())
    }

    fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        if query_str.len() > 10000 {
            return Err("Query too long".to_string());
        }
        let searcher = self.reader.searcher();
        
        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.symbol_field,
                self.docstring_field,
                self.literal_field,
                self.file_path_parts_field,
            ]
        );
        
        query_parser.set_field_boost(self.symbol_field, 4.0);
        query_parser.set_field_boost(self.docstring_field, 2.0);
        query_parser.set_field_boost(self.literal_field, 1.0);
        query_parser.set_field_boost(self.file_path_parts_field, 1.0);

        if query_str.trim().is_empty() {
            return Ok(Vec::new());
        }

        let query = match query_parser.parse_query(query_str) {
            Ok(q) => q,
            Err(_) => {
                let escaped: String = query_str
                    .to_lowercase()
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c.is_whitespace() {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect();
                if escaped.trim().is_empty() {
                    return Ok(Vec::new());
                }
                match query_parser.parse_query(&escaped) {
                    Ok(q) => q,
                    Err(e) => return Err(e.to_string()),
                }
            }
        };

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        let query_lower = query_str.to_lowercase();

        for (score, doc_address) in top_docs {
            let doc = searcher.doc::<TantivyDocument>(doc_address).map_err(|e| e.to_string())?;
            
            let file_path = doc.get_first(self.file_path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let extracted_json = doc.get_first(self.extracted_json_field)
                .and_then(|v| v.as_str())
                .unwrap_or("{}");

            let extracted_file: ExtractedFile = serde_json::from_str(extracted_json)
                .unwrap_or_else(|_| ExtractedFile {
                    file_path: file_path.clone(),
                    symbols: Vec::new(),
                    literals: Vec::new(),
                    docstrings: Vec::new(),
                });

            let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

            let matched_symbols: Vec<ExtractedSymbol> = extracted_file.symbols
                .into_iter()
                .filter(|sym| {
                    !query_terms.is_empty() && query_terms.iter().all(|&term| {
                        sym.name.to_lowercase().contains(term) ||
                        sym.docstring.as_ref().map_or(false, |d| d.to_lowercase().contains(term)) ||
                        crate::parser::split_identifier(&sym.name).iter().any(|t| t.to_lowercase().contains(term))
                    })
                })
                .collect();

            let matched_literals: Vec<String> = extracted_file.literals
                .into_iter()
                .filter(|lit| {
                    !query_terms.is_empty() && query_terms.iter().all(|&term| lit.to_lowercase().contains(term))
                })
                .collect();

            results.push(SearchResult {
                file_path,
                score: score as f32,
                matched_symbols,
                matched_literals,
            });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_tokenize_path_helper() {
        assert_eq!(tokenize_path("src/lib.rs"), "src lib rs");
        assert_eq!(tokenize_path("a\\b\\c.js"), "a b c js");
        assert_eq!(tokenize_path("main.rs"), "main rs");
    }

    #[test]
    fn test_normalize_relative_path_helper() {
        assert_eq!(normalize_relative_path(Path::new("./src/lib.rs")), "src/lib.rs");
        assert_eq!(normalize_relative_path(Path::new("src\\lib.rs")), "src/lib.rs");
    }

    #[test]
    fn test_engine_basic_indexing_and_search() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        let file1 = src_dir.join("lib.rs");
        fs::write(&file1, "pub fn calculate_prime_numbers() {}").unwrap();

        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        
        // Initially search should be empty
        let res = engine.search("calculate_prime_numbers", 10).unwrap();
        assert_eq!(res.len(), 0);

        // Index the files
        if let Err(e) = engine.index_files(&[&temp.path().to_string_lossy()]) {
            println!("test_engine_basic_indexing_and_search index error: {}", e);
        }

        // Search again
        let res = engine.search("calculate_prime_numbers", 10).unwrap();
        println!("basic search results len: {}", res.len());
        if res.is_empty() {
            // Let's print out what files were registered or if indexing was skipped
            println!("Indexed files mtimes map: {:?}", engine.get_indexed_mtimes());
        }
        assert_eq!(res.len(), 1);
        assert!(res[0].file_path.contains("lib.rs"));
        assert_eq!(res[0].matched_symbols[0].name, "calculate_prime_numbers");
    }

    #[test]
    fn test_engine_ranking_weights() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // File A: QueryTerm in literal (weight = 1.0)
        let file_a = src_dir.join("file_a.rs");
        fs::write(&file_a, "pub fn test() { let x = \"QueryTerm\"; }").unwrap();

        // File B: QueryTerm in symbol name (weight = 4.0)
        let file_b = src_dir.join("file_b.rs");
        fs::write(&file_b, "pub fn QueryTerm() {}").unwrap();

        // File C: QueryTerm in docstring (weight = 2.0)
        let file_c = src_dir.join("file_c.rs");
        fs::write(&file_c, "/// QueryTerm\npub fn hello() {}").unwrap();

        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        engine.index_files(&[&temp.path().to_string_lossy()]).unwrap();

        let res = engine.search("QueryTerm", 10).unwrap();
        assert_eq!(res.len(), 3);

        // Best match should be File B (symbol, weight 4)
        assert!(res[0].file_path.contains("file_b.rs"), "Expected file_b.rs first, got: {:?}", res);
        // Second best should be File C (docstring, weight 2)
        assert!(res[1].file_path.contains("file_c.rs"), "Expected file_c.rs second, got: {:?}", res);
        // Third should be File A (literal, weight 1)
        assert!(res[2].file_path.contains("file_a.rs"), "Expected file_a.rs third, got: {:?}", res);
    }

    #[test]
    fn test_engine_incremental_indexing() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        let file1 = src_dir.join("lib.rs");
        fs::write(&file1, "pub fn first_func() {}").unwrap();

        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        engine.index_files(&[&temp.path().to_string_lossy()]).unwrap();

        // Read initial modification time of index directory
        let initial_mtime = fs::metadata(&index_dir).unwrap().modified().unwrap();

        // Index again immediately with no changes
        std::thread::sleep(std::time::Duration::from_millis(50));
        engine.index_files(&[&temp.path().to_string_lossy()]).unwrap();

        let final_mtime = fs::metadata(&index_dir).unwrap().modified().unwrap();
        assert_eq!(initial_mtime, final_mtime);

        // Now modify a file
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&file1, "pub fn first_func_modified() {}").unwrap();
        // Force update the mtime of the file
        let new_mtime = filetime::FileTime::from_system_time(std::time::SystemTime::now() + std::time::Duration::from_secs(10));
        filetime::set_file_mtime(&file1, new_mtime).unwrap();

        engine.index_files(&[&temp.path().to_string_lossy()]).unwrap();
        let after_modify_mtime = fs::metadata(&index_dir).unwrap().modified().unwrap();
        assert_ne!(initial_mtime, after_modify_mtime);

        // Search for new symbol
        let res = engine.search("first_func_modified", 10).unwrap();
        assert_eq!(res.len(), 1);
    }

    #[test]
    fn test_engine_corrupt_recovery() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        fs::create_dir_all(&index_dir).unwrap();

        // Corrupt the index directory with invalid meta.json
        fs::write(index_dir.join("meta.json"), "{invalid json}").unwrap();

        // Instantiating the engine should auto-recover
        let engine = TantivySearchEngine::new(&index_dir.to_string_lossy());
        assert!(engine.is_ok());
    }

    #[test]
    fn test_query_error_handling() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        
        let engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        
        // Search with query containing syntax errors / special characters should not panic
        let res = engine.search("AND OR NOT * : ()", 10);
        if let Err(ref e) = res {
            println!("test_query_error_handling failed with error: {}", e);
        }
        assert!(res.is_ok());
    }
}
