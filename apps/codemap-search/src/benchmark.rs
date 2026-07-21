use crate::index::SearchEngine;
use crate::parser::CodeExtractor;
use std::path::Path;

pub struct BenchmarkEngine;

impl BenchmarkEngine {
    pub fn run_benchmark(
        queries_path: &str,
        extractor: &impl CodeExtractor,
        search_engine: &mut impl SearchEngine,
        target_dir: &str,
    ) -> Result<(), String> {
        let path = Path::new(queries_path);
        if !path.exists() {
            return Err("Query file not found".to_string());
        }

        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let queries: Vec<serde_json::Value> =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;

        if queries.is_empty() {
            println!("No queries");
            return Ok(());
        }

        search_engine.index_files(&[target_dir])?;

        let mut source_files = Vec::new();
        for entry in walkdir::WalkDir::new(target_dir)
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
            let p = entry.path();
            if p.is_file() {
                if crate::workspace::is_explicitly_excluded_file(p) {
                    continue;
                }
                if crate::workspace::is_supported_source_path(p) {
                    source_files.push(p.to_path_buf());
                }
            }
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let abs_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        let mut precomputed_source_files = Vec::new();
        for file_path in source_files {
            let abs_path = file_path
                .canonicalize()
                .unwrap_or_else(|_| file_path.clone());
            let rel = abs_path.strip_prefix(&abs_cwd).unwrap_or(&file_path);
            let rel_path = crate::workspace::normalize_workspace_key(&rel.to_string_lossy());
            precomputed_source_files.push((file_path, rel_path));
        }

        let mut total_baseline_latency = 0.0;
        let mut total_index_latency = 0.0;
        let mut total_baseline_recall = 0.0;
        let mut total_index_recall = 0.0;
        let mut all_identical = true;
        let mut diff_queries_count = 0;
        let mut valid_queries = 0;

        for item in &queries {
            // E2E handles malformed queries by skipping or exiting cleanly.
            // If the query is missing or not a string, let's skip.
            let query = match item.get("query").and_then(|q| q.as_str()) {
                Some(q) => q.to_string(),
                None => {
                    continue;
                }
            };
            valid_queries += 1;

            let mut expected_normalized = Vec::new();
            if let Some(expected_val) = item.get("expected") {
                if let Some(expected_arr) = expected_val.as_array() {
                    for val in expected_arr {
                        if let Some(s) = val.as_str() {
                            expected_normalized.push(crate::workspace::normalize_workspace_key(s));
                        }
                    }
                } else {
                    return Err(
                        "Malformed expected schema: 'expected' field must be an array of strings"
                            .to_string(),
                    );
                }
            }

            let query_lower = query.to_lowercase();
            let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

            // 1. Baseline Search
            let start_baseline = std::time::Instant::now();
            let mut baseline_matched = Vec::new();
            for (file_path, rel_path) in &precomputed_source_files {
                if let Ok(file_content) = std::fs::read_to_string(file_path) {
                    if let Ok(extracted) = extractor.extract(&file_content, rel_path) {
                        let mut matched = false;
                        for sym in &extracted.symbols {
                            if !query_terms.is_empty()
                                && query_terms.iter().all(|&term| {
                                    sym.name.to_lowercase().contains(term)
                                        || sym
                                            .docstring
                                            .as_ref()
                                            .is_some_and(|d| d.to_lowercase().contains(term))
                                        || crate::parser::split_identifier(&sym.name)
                                            .iter()
                                            .any(|t| t.to_lowercase().contains(term))
                                })
                            {
                                matched = true;
                                break;
                            }
                        }
                        if !matched {
                            for lit in &extracted.literals {
                                if !query_terms.is_empty()
                                    && query_terms
                                        .iter()
                                        .all(|&term| lit.text.to_lowercase().contains(term))
                                {
                                    matched = true;
                                    break;
                                }
                            }
                        }
                        if matched {
                            baseline_matched.push(rel_path.clone());
                        }
                    }
                }
            }
            let baseline_latency = start_baseline.elapsed().as_secs_f64() * 1000.0;
            let baseline_set: std::collections::HashSet<String> =
                baseline_matched.into_iter().collect();

            let baseline_recall = if expected_normalized.is_empty() {
                1.0
            } else {
                let matched = expected_normalized
                    .iter()
                    .filter(|p| baseline_set.contains(p.as_str()))
                    .count();
                matched as f64 / expected_normalized.len() as f64
            };

            // 2. Index Search
            let start_index = std::time::Instant::now();
            let index_results = match search_engine.search(&query, 100) {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("Index search failed for query '{}': {}", query, e);
                    valid_queries -= 1;
                    continue;
                }
            };
            let index_latency = start_index.elapsed().as_secs_f64() * 1000.0;

            let index_set: std::collections::HashSet<String> = index_results
                .iter()
                .map(|r| crate::workspace::normalize_workspace_key(&r.file_path))
                .collect();

            let index_recall = if expected_normalized.is_empty() {
                1.0
            } else {
                let matched = expected_normalized
                    .iter()
                    .filter(|p| index_set.contains(p.as_str()))
                    .count();
                matched as f64 / expected_normalized.len() as f64
            };

            // Accumulate metrics after successful completion of both baseline and index search
            total_baseline_latency += baseline_latency;
            total_baseline_recall += baseline_recall;
            total_index_latency += index_latency;
            total_index_recall += index_recall;

            if baseline_set != index_set {
                all_identical = false;
                diff_queries_count += 1;
            }
        }

        if valid_queries == 0 {
            println!("No queries");
            return Ok(());
        }

        let avg_baseline_latency = total_baseline_latency / valid_queries as f64;
        let avg_index_latency = total_index_latency / valid_queries as f64;
        let avg_baseline_recall = total_baseline_recall / valid_queries as f64;
        let avg_index_recall = total_index_recall / valid_queries as f64;

        println!("| Metric | Baseline | Index |");
        println!(
            "| Latency | {:.2}ms | {:.2}ms |",
            avg_baseline_latency, avg_index_latency
        );

        let baseline_recall_pct = avg_baseline_recall * 100.0;
        let index_recall_pct = avg_index_recall * 100.0;

        let baseline_recall_str = if (baseline_recall_pct - 100.0).abs() < 1e-5 {
            "100%".to_string()
        } else if baseline_recall_pct.abs() < 1e-5 {
            "0%".to_string()
        } else {
            format!("{:.1}%", baseline_recall_pct)
        };

        let index_recall_str = if (index_recall_pct - 100.0).abs() < 1e-5 {
            "100%".to_string()
        } else if index_recall_pct.abs() < 1e-5 {
            "0%".to_string()
        } else {
            format!("{:.1}%", index_recall_pct)
        };

        println!(
            "| Recall | {} | {} |",
            baseline_recall_str, index_recall_str
        );

        if all_identical {
            println!("Results: Identical (0% diff)");
        } else {
            let diff_percent = (diff_queries_count as f64 / valid_queries as f64) * 100.0;
            println!("Results: {:.1}% diff", diff_percent);
        }

        Ok(())
    }
}
