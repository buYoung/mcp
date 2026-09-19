//! `find` — file-path search by glob or basename regex, with fd-style entry/depth
//! selection. Mirrors Claude Code's Glob / scout's `find_files`, except it respects
//! `.gitignore` + `.codemapignore` by default (the decided divergence) with an
//! `include_ignored` override. Glob matching uses the shared gitignore-style engine
//! in `mod.rs` (`rg --glob` semantics: a slash-less pattern matches the basename at
//! any depth).

use super::{arg_bool, arg_required_str, build_glob_matcher, get_arg};
use crate::workspace::{build_walker, current_dir, resolve_for_filesystem_tool, FilesystemTool};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

/// Max files returned; on overflow keep the NEWEST [`FIND_FILES_RESULT_LIMIT`] by
/// mtime, so the files most likely just created/edited are never the ones dropped.
/// This is codemap's decided divergence from Claude Code's Glob `--sort=modified`
/// (which keeps the OLDEST 100): newest-first is more useful for an editing agent.
const FIND_FILES_RESULT_LIMIT: usize = 100;
const FIND_FILES_TRUNCATION_MESSAGE: &str =
    "(Results are truncated. Consider using a more specific path or pattern.)";

#[derive(Default)]
struct FindResults {
    hits: Vec<(String, SystemTime)>,
    is_truncated: bool,
}

impl FindResults {
    fn record(&mut self, path: String, mtime: SystemTime) {
        let position = self
            .hits
            .partition_point(|(existing_path, existing_mtime)| {
                *existing_mtime > mtime || (*existing_mtime == mtime && existing_path < &path)
            });
        if self.hits.len() == FIND_FILES_RESULT_LIMIT {
            self.is_truncated = true;
        }
        if position < FIND_FILES_RESULT_LIMIT {
            self.hits.insert(position, (path, mtime));
            self.hits.truncate(FIND_FILES_RESULT_LIMIT);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryType {
    File,
    Directory,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PatternType {
    Glob,
    Regex,
}

impl EntryType {
    fn parse(value: Option<&Value>) -> Result<Self, (i64, String)> {
        match value {
            None => Ok(Self::File),
            Some(value) => value
                .as_str()
                .ok_or_else(|| {
                    (
                        -32602,
                        "Parameter 'entry_type' must be a string".to_string(),
                    )
                })
                .and_then(|value| match value {
                    "file" => Ok(Self::File),
                    "directory" => Ok(Self::Directory),
                    "all" => Ok(Self::All),
                    _ => Err((
                        -32602,
                        format!(
                    "Invalid 'entry_type' value: {value}. Expected one of: file, directory, all"
                ),
                    )),
                }),
        }
    }
}

impl PatternType {
    fn parse(value: Option<&Value>) -> Result<Self, (i64, String)> {
        match value {
            None => Ok(Self::Glob),
            Some(value) => value
                .as_str()
                .ok_or_else(|| {
                    (
                        -32602,
                        "Parameter 'pattern_type' must be a string".to_string(),
                    )
                })
                .and_then(|value| match value {
                    "glob" => Ok(Self::Glob),
                    "regex" => Ok(Self::Regex),
                    _ => Err((
                        -32602,
                        format!(
                            "Invalid 'pattern_type' value: {value}. Expected one of: glob, regex"
                        ),
                    )),
                }),
        }
    }
}

/// Split an absolute glob pattern into a search base directory + a remainder pattern
/// mirroring Claude Code's `extractGlobBaseDirectory`: the static prefix up to the
/// first glob metacharacter becomes the base, the rest the pattern. A fully-literal
/// absolute path (no metacharacter) splits into dirname (base) + basename (pattern).
/// The split point is the last `/` at or before the first metacharacter.
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
/// The base is checked through the shared filesystem permission model.
fn resolve_absolute_pattern(pattern: &str) -> Result<(PathBuf, String), (i64, String)> {
    let (base_str, remainder) = split_static_prefix(pattern);
    // A dir-only absolute pattern (trailing `/`, no file part) leaves nothing to match;
    // reject rather than silently matching every file under the base.
    if remainder.is_empty() {
        return Err((
            -32602,
            format!("Absolute path pattern has no file component to match: {pattern}"),
        ));
    }
    let base_canonical = resolve_for_filesystem_tool(&base_str, FilesystemTool::Find)?;
    Ok((base_canonical, remainder))
}

pub fn find_files(args: &Value) -> Result<String, (i64, String)> {
    let pattern = arg_required_str(args, "pattern")?;
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let include_ignored = arg_bool(args, "include_ignored", false);
    let entry_type = EntryType::parse(get_arg(args, "entry_type"))?;
    let pattern_type = PatternType::parse(get_arg(args, "pattern_type"))?;
    let max_depth = match get_arg(args, "max_depth") {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|depth| usize::try_from(depth).ok())
            .map(Some)
            .ok_or((
                -32602,
                "Parameter 'max_depth' must be a non-negative integer".to_string(),
            )),
    }?;

    // Resolve the search base and the pattern relative to it. Absolute glob patterns
    // split their static prefix into the base (Claude Code parity); relative patterns
    // search the `path` arg and reject `..` escapes. Regex patterns are not paths:
    // preserve their text verbatim and use `path` as the search root.
    let pattern_path = crate::workspace::path_from_workspace_input(pattern);
    let (base, relative_pattern) = if pattern_type == PatternType::Regex {
        let base = resolve_for_filesystem_tool(path, FilesystemTool::Find)?;
        (base, pattern.to_string())
    } else if pattern_path.is_absolute() {
        resolve_absolute_pattern(pattern)?
    } else {
        if pattern.split(['/', '\\']).any(|seg| seg == "..") {
            return Err((
                -32602,
                format!("Parent-directory ('..') patterns are not allowed: {pattern}"),
            ));
        }
        let base = resolve_for_filesystem_tool(path, FilesystemTool::Find)?;
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
    // Regex mode is deliberately basename-only and case-sensitive.
    let glob_matcher = if pattern_type == PatternType::Glob {
        Some(build_glob_matcher(
            &base,
            std::slice::from_ref(&relative_pattern),
        )?)
    } else {
        None
    };
    let regex_matcher = if pattern_type == PatternType::Regex {
        Some(
            regex::Regex::new(relative_pattern.as_str()).map_err(|error| {
                (
                    -32602,
                    format!("Invalid regex pattern '{pattern}': {error}"),
                )
            })?,
        )
    } else {
        None
    };
    let cwd = current_dir()?;
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);

    // Visit every candidate, retaining only the deterministic newest 100 in memory.
    let results = Mutex::new(FindResults::default());
    let mut walker = build_walker(&base, include_ignored);
    if let Some(max_depth) = max_depth {
        walker.max_depth(Some(max_depth));
    }
    walker.threads(4).build_parallel().run(|| {
        Box::new(|result| {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => return ignore::WalkState::Continue,
            };
            if entry.depth() == 0 {
                return ignore::WalkState::Continue;
            }
            let file_type = entry.file_type();
            let is_file = file_type.is_some_and(|file_type| file_type.is_file());
            let is_directory = file_type.is_some_and(|file_type| file_type.is_dir());
            let matches_entry_type = match entry_type {
                EntryType::File => is_file,
                EntryType::Directory => is_directory,
                EntryType::All => is_file || is_directory,
            };
            if !matches_entry_type {
                return ignore::WalkState::Continue;
            }
            if is_file
                && !include_ignored
                && crate::workspace::is_default_live_tool_excluded_file(entry.path())
            {
                return ignore::WalkState::Continue;
            }

            let rel_to_base = entry.path().strip_prefix(&base).unwrap_or(entry.path());
            let matches_pattern = match pattern_type {
                PatternType::Glob => glob_matcher
                    .as_ref()
                    .is_some_and(|matcher| matcher.is_match_entry(rel_to_base, is_directory)),
                PatternType::Regex => entry
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| regex_matcher.as_ref().map(|regex| regex.is_match(name)))
                    .unwrap_or(false),
            };
            if !matches_pattern {
                return ignore::WalkState::Continue;
            }

            // Output paths stay cwd-relative; an out-of-root base (opt-in) falls back to
            // the absolute path since it cannot be relativized to cwd.
            let mut display = entry
                .path()
                .strip_prefix(&cwd_canonical)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            if is_directory {
                display.push('/');
            }
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            results
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .record(display, mtime);
            ignore::WalkState::Continue
        })
    });

    let results = results.into_inner().map_err(|_| {
        (
            -32603,
            "Find result collection failed because a worker panicked".to_string(),
        )
    })?;

    if results.hits.is_empty() {
        return Ok("No files found".to_string());
    }

    let mut out = results
        .hits
        .into_iter()
        .map(|(path, _)| path)
        .collect::<Vec<_>>()
        .join("\n");
    if results.is_truncated {
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
