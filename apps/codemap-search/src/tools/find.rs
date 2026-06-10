//! `find` — file-path search by glob, ripgrep-equivalent file walking. Mirrors
//! Claude Code's Glob / scout's `find_files`, except it respects `.gitignore` +
//! `.codemapignore` by default (the decided divergence) with an `include_ignored`
//! override.

use super::{arg_bool, arg_required_str, build_walker, resolve_within_cwd};
use serde_json::Value;
use std::time::SystemTime;

/// Max files returned; on overflow keep the NEWEST [`FIND_FILES_RESULT_LIMIT`] by
/// mtime, so the files most likely just created/edited are never the ones dropped.
/// Matches Claude Code's Glob `--sort=modified`, which surfaces newest first.
const FIND_FILES_RESULT_LIMIT: usize = 100;
const FIND_FILES_TRUNCATION_MESSAGE: &str =
    "(Results are truncated. Consider using a more specific path or pattern.)";

pub fn find_files(args: &Value) -> Result<String, (i64, String)> {
    let pattern = arg_required_str(args, "pattern")?;
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let include_ignored = arg_bool(args, "include_ignored", false);

    // Pattern guardrails: absolute and parent-directory patterns can escape the root.
    if std::path::Path::new(pattern).is_absolute() {
        return Err((
            -32602,
            format!("Absolute path patterns are not allowed: {pattern}"),
        ));
    }
    if pattern.split(['/', '\\']).any(|seg| seg == "..") {
        return Err((
            -32602,
            format!("Parent-directory ('..') patterns are not allowed: {pattern}"),
        ));
    }

    let base = resolve_within_cwd(path)?;
    if !base.is_dir() {
        return Err((-32602, format!("Search path is not a directory: {path}")));
    }
    let cwd = super::current_dir()?;
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);

    // Shell-glob semantics: `*`/`?` do not cross `/`, `**` does (literal_separator).
    let matcher = globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|e| (-32602, format!("Invalid glob pattern '{pattern}': {e}")))?
        .compile_matcher();

    let mut hits: Vec<(String, SystemTime)> = Vec::new();
    for result in build_walker(&base, include_ignored).build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        // Glob is matched relative to the search base (globby cwd semantics).
        let rel_to_base = entry.path().strip_prefix(&base).unwrap_or(entry.path());
        if !matcher.is_match(rel_to_base) {
            continue;
        }
        let display = entry
            .path()
            .strip_prefix(&cwd_canonical)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        hits.push((display, mtime));
    }

    if hits.is_empty() {
        return Ok("No files found".to_string());
    }

    hits.sort_by(|a, b| b.1.cmp(&a.1)); // newest first, so truncation drops the oldest
    let truncated = hits.len() > FIND_FILES_RESULT_LIMIT;
    hits.truncate(FIND_FILES_RESULT_LIMIT);

    let mut out = hits
        .into_iter()
        .map(|(p, _)| p)
        .collect::<Vec<_>>()
        .join("\n");
    if truncated {
        out.push('\n');
        out.push_str(FIND_FILES_TRUNCATION_MESSAGE);
    }
    Ok(out)
}
