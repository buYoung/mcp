//! `find` — file-path search by glob, ripgrep-equivalent file walking. Mirrors
//! Claude Code's Glob / scout's `find_files`, except it respects `.gitignore` +
//! `.codemapignore` by default (the decided divergence) with an `include_ignored`
//! override. Glob matching uses the shared gitignore-style engine in `mod.rs`
//! (`rg --glob` semantics: a slash-less pattern matches the basename at any depth).

use super::{arg_bool, arg_required_str, build_glob_matcher};
use crate::workspace::{build_walker, current_dir, resolve_within_cwd};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Max files returned; on overflow keep the NEWEST [`FIND_FILES_RESULT_LIMIT`] by
/// mtime, so the files most likely just created/edited are never the ones dropped.
/// This is codemap's decided divergence from Claude Code's Glob `--sort=modified`
/// (which keeps the OLDEST 100): newest-first is more useful for an editing agent.
const FIND_FILES_RESULT_LIMIT: usize = 100;
const FIND_FILES_TRUNCATION_MESSAGE: &str =
    "(Results are truncated. Consider using a more specific path or pattern.)";

/// Split an absolute glob pattern into a search base directory + a remainder pattern
/// matched relative to it, mirroring Claude Code's `extractGlobBaseDirectory`: the static
/// prefix up to the first glob metacharacter becomes the base, the rest the pattern. A
/// fully-literal absolute path (no metacharacter) splits into dirname (base) + basename
/// (pattern). The split point is the last `/` at or before the first metacharacter.
fn split_static_prefix(pattern: &str) -> (String, String) {
    let normalized = pattern.replace('\\', "/");
    let meta_index = normalized.find(['*', '?', '[', '{']);
    let split_at = match meta_index {
        Some(index) => normalized[..index].rfind('/').map(|s| s + 1).unwrap_or(0),
        None => normalized.rfind('/').map(|s| s + 1).unwrap_or(0),
    };
    (
        normalized[..split_at].to_string(),
        normalized[split_at..].to_string(),
    )
}

/// Resolve an absolute glob pattern into a (canonicalized search base, relative remainder).
/// The base must stay within the workspace root unless `allow_absolute_path_outside_root`
/// is enabled; an escaping base (including via `..`) is rejected by default.
fn resolve_absolute_pattern(
    pattern: &str,
    cwd_canonical: &Path,
) -> Result<(PathBuf, String), (i64, String)> {
    let (base_str, remainder) = split_static_prefix(pattern);
    // A dir-only absolute pattern (trailing `/`, no file part) leaves nothing to match;
    // reject rather than silently matching every file under the base.
    if remainder.is_empty() {
        return Err((
            -32602,
            format!("Absolute path pattern has no file component to match: {pattern}"),
        ));
    }
    let base_canonical = crate::workspace::canonicalize_path_lenient(
        &crate::workspace::path_from_workspace_input(&base_str),
    );
    let allow_outside = crate::config::get().allow_absolute_path_outside_root;
    if !allow_outside && !base_canonical.starts_with(cwd_canonical) {
        return Err((
            -32602,
            format!("Absolute path pattern escapes the workspace root: {pattern}"),
        ));
    }
    Ok((base_canonical, remainder))
}

pub fn find_files(args: &Value) -> Result<String, (i64, String)> {
    let pattern = arg_required_str(args, "pattern")?;
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let include_ignored = arg_bool(args, "include_ignored", false);

    let cwd = current_dir()?;
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);

    // Resolve the search base and the glob matched relative to it. Absolute patterns split
    // their static prefix into the base (Claude Code parity); relative patterns search the
    // `path` arg and reject `..` escapes.
    let pattern_path = crate::workspace::path_from_workspace_input(pattern);
    let (base, relative_pattern) = if pattern_path.is_absolute() {
        resolve_absolute_pattern(pattern, &cwd_canonical)?
    } else {
        if pattern.split(['/', '\\']).any(|seg| seg == "..") {
            return Err((
                -32602,
                format!("Parent-directory ('..') patterns are not allowed: {pattern}"),
            ));
        }
        // Honor `allow_absolute_path_outside_root` for an absolute `path` arg too (decision
        // 4), so the opt-in escape hatch is symmetric with absolute patterns. Without the
        // flag, `resolve_within_cwd` still sandboxes the path to the workspace root.
        let path_arg = crate::workspace::path_from_workspace_input(path);
        let base =
            if path_arg.is_absolute() && crate::config::get().allow_absolute_path_outside_root {
                crate::workspace::canonicalize_path_lenient(&path_arg)
            } else {
                resolve_within_cwd(path)?
            };
        (base, pattern.to_string())
    };

    if !base.is_dir() {
        return Err((
            -32602,
            format!("Search path is not a directory: {}", base.display()),
        ));
    }

    // Shared gitignore-style matcher: `*.rs` matches at any depth, `**` crosses dirs,
    // `{a,b}` brace-expands, leading `!` negates — identical to `rg --glob`.
    let matcher = build_glob_matcher(&base, &[relative_pattern])?;

    let mut hits: Vec<(String, SystemTime)> = Vec::new();
    for result in build_walker(&base, include_ignored).build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        // Glob is matched relative to the search base (gitignore semantics).
        let rel_to_base = entry.path().strip_prefix(&base).unwrap_or(entry.path());
        if !matcher.is_match(rel_to_base) {
            continue;
        }
        // Output paths stay cwd-relative; an out-of-root base (opt-in) falls back to the
        // absolute path since it cannot be relativized to cwd.
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

    // Newest first (decision 2); ties broken by path name ascending so truncation at the
    // result cap is deterministic across repeated calls.
    hits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_static_prefix_normalizes_windows_separators() {
        assert_eq!(
            split_static_prefix("G:\\repo\\src\\*.rs"),
            ("G:/repo/src/".to_string(), "*.rs".to_string())
        );
        assert_eq!(
            split_static_prefix("G:\\repo\\src\\lib.rs"),
            ("G:/repo/src/".to_string(), "lib.rs".to_string())
        );
    }
}
