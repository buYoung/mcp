//! `rtpoc2 search`: the product's ranked search over the index an `rtpoc2 index` run wrote.
//!
//! This is the `codemap-search search` CLI call path (`TantivySearchEngine::new` +
//! `search_with_context` with an empty `SearchQueryContext`). Only the index location
//! differs: the CLI opens `config.index_path` (default `.codemap/index` under the cwd), this
//! opens `<state>/index`. Ranking reads no configuration, so none is loaded here.
//! `results` is the ranking; `displayed` is the order the MCP search tool renders it in.

use std::path::Path;

use codemap_search::index::{SearchQueryContext, SearchResult, TantivySearchEngine};

use crate::stats::INDEX_DIR;

/// MCP search display defaults: `config.rs` `result_threshold` (detail files) and the
/// `tools/search` constants `DETAIL_DIR_CAP` / `DIVERSITY_MIN_SCORE_RATIO`.
const RESULT_THRESHOLD: usize = 24;
const DETAIL_DIR_CAP: usize = 3;
const DIVERSITY_MIN_SCORE_RATIO: f32 = 0.6;

fn parent_dir(file_path: &str) -> &str {
    file_path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Port of the private `tools::search::diversified_order`: the order in which the MCP search
/// tool renders files (a directory-diversified detail head, then the rest in score order).
fn diversified_order(results: &[SearchResult], head_len: usize, per_dir_cap: usize) -> Vec<usize> {
    if results.len() <= 1 || head_len == 0 {
        return (0..results.len()).collect();
    }
    let mut head: Vec<usize> = Vec::with_capacity(head_len);
    let mut deferred: Vec<usize> = Vec::new();
    let mut dir_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (index, result) in results.iter().enumerate() {
        if head.len() >= head_len {
            deferred.push(index);
            continue;
        }
        let dir = parent_dir(&result.file_path);
        let dir_count = dir_counts.get(dir).copied().unwrap_or(0);
        let head_room_left = head_len - head.len();
        let diverse_ahead: Vec<&SearchResult> = results[index + 1..]
            .iter()
            .filter(|other| parent_dir(&other.file_path) != dir)
            .collect();
        let is_score_gap_small = match diverse_ahead.first().map(|other| other.score) {
            Some(diverse_score) if result.score > 0.0 => diverse_score >= result.score * DIVERSITY_MIN_SCORE_RATIO,
            Some(_) => true,
            None => false,
        };
        if dir_count >= per_dir_cap && diverse_ahead.len() >= head_room_left && is_score_gap_small {
            deferred.push(index);
            continue;
        }
        head.push(index);
        *dir_counts.entry(dir).or_insert(0) += 1;
    }
    head.extend(deferred);
    head
}

/// The product writes this stamp next to the index; see `TantivySearchEngine::new`.
const EXTRACTION_FORMAT_FILE: &str = "codemap.format";

fn read_format_stamp(index_dir: &Path) -> Option<String> {
    std::fs::read_to_string(index_dir.join(EXTRACTION_FORMAT_FILE))
        .ok()
        .map(|stamp| stamp.trim().to_string())
}

/// `TantivySearchEngine::new` wipes and recreates an index whose format stamp differs from
/// the linked library. Search must stay read-only, so the linked library's stamp is read from
/// a throwaway empty index first and a differing state index is refused.
fn ensure_format_matches_library(index_dir: &Path) -> Result<(), String> {
    let probe = std::env::temp_dir().join(format!("rtpoc2-format-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&probe);
    let library_stamp = TantivySearchEngine::new(&probe.to_string_lossy())
        .map(drop)
        .map(|()| read_format_stamp(&probe));
    let _ = std::fs::remove_dir_all(&probe);
    let library_stamp = library_stamp?.ok_or("linked library wrote no index format stamp")?;
    match read_format_stamp(index_dir) {
        Some(stamp) if stamp == library_stamp => Ok(()),
        stamp => Err(format!(
            "index format {stamp:?} differs from the linked library's {library_stamp:?}; re-run rtpoc2 index"
        )),
    }
}

pub fn search(state_dir: &Path, query: &str, limit: usize) -> Result<serde_json::Value, String> {
    let index_dir = state_dir.join(INDEX_DIR);
    if !index_dir.join("meta.json").is_file() {
        return Err(format!("no committed index at {}", index_dir.display()));
    }
    ensure_format_matches_library(&index_dir)?;
    let engine = TantivySearchEngine::new(&index_dir.to_string_lossy())?;
    let results = engine.search_with_context(query, limit, &SearchQueryContext::default())?;
    let displayed: Vec<&str> = diversified_order(&results, RESULT_THRESHOLD, DETAIL_DIR_CAP)
        .into_iter()
        .map(|index| results[index].file_path.as_str())
        .collect();
    let paths: Vec<&str> = results.iter().map(|result| result.file_path.as_str()).collect();
    Ok(serde_json::json!({
        "query": query,
        "limit": limit,
        "results": paths,
        "displayed": displayed,
        "display": {"result_threshold": RESULT_THRESHOLD, "detail_dir_cap": DETAIL_DIR_CAP,
                    "diversity_min_score_ratio": DIVERSITY_MIN_SCORE_RATIO},
    }))
}
