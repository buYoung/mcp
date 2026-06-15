//! `read` — single-file read with `cat -n` style line numbers, mirroring Claude
//! Code's Read tool so an agent can swap its built-in without surprises. Direct
//! filesystem I/O, no index/engine dependency.

use super::{get_arg, lenient_usize};
use crate::workspace::resolve_within_cwd;
use serde_json::Value;

/// When `limit` is omitted, refuse to read files larger than this so a single call
/// can't dump an unbounded blob (matches Claude Code's 256 KiB read cap).
const READ_FILE_BYTE_CAP: u64 = 262_144;

/// Extensions read refuses in v1 (binary / image / document / notebook). Searching
/// these is out of scope; the agent gets an explicit unsupported error.
const UNSUPPORTED_EXTENSIONS: &[&str] = &[
    // images
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff", "heic", // documents / notebooks
    "pdf", "ipynb", // archives / binaries
    "exe", "dll", "so", "dylib", "bin", "class", "o", "a", "obj", "wasm", "zip", "gz", "tar",
    "tgz", "bz2", "xz", "7z", "rar", "jar", "war", // media
    "mp3", "mp4", "mov", "avi", "mkv", "wav", "flac", "ogg",
    "webm", // office / design / fonts
    "doc", "docx", "xls", "xlsx", "ppt", "pptx", "woff", "woff2", "ttf", "otf", "eot", "psd",
    "sketch",
];

/// Format file content with right-justified, arrow-delimited line numbers
/// (`␠␠␠␠␠1→content`), matching Claude Code's `addLineNumbers`. `start_line` is
/// 1-indexed.
fn add_line_numbers(lines: &[&str], start_line: usize) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>6}\u{2192}{}", i + start_line, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read the file path, accepting the idiomatic aliases `path`, `file`, and `query` when the
/// canonical `file_path` is absent. Canonical wins when both are present; earlier aliases win
/// over later ones. Each lookup is alias-tolerant ([`get_arg`]) so a camel/kebab/Pascal variant
/// (`filePath`, `file-path`) resolves to its canonical parameter instead of being dropped — an
/// exact spelling still wins over a normalized variant. The error message names every accepted
/// spelling so a wrong-param call self-corrects.
fn resolve_file_path_arg(args: &Value) -> Result<&str, (i64, String)> {
    for key in ["file_path", "path", "file", "query"] {
        if let Some(value) = get_arg(args, key).and_then(|v| v.as_str()) {
            return Ok(value);
        }
    }
    Err((
        -32602,
        "Missing required 'file_path' parameter (aliases: 'path', 'file', 'query').".to_string(),
    ))
}

/// Resolve the read window from `offset`/`limit`, falling back to the 1-based inclusive
/// `start_line`/`end_line` (then `start`/`end`) aliases that Claude-family agents idiomatically
/// use. Priority: explicit `offset`/`limit` > `start_line`/`end_line` > `start`/`end`.
///
/// Mapping: the effective start drives offset (offset, else start_line/start, else — when only
/// an end is given — 1); limit is derived from `end - effective_start + 1` so a mixed window
/// like `{offset: 100, end_line: 120}` reads 100..=120, not 100..219 (the prior code derived
/// limit from the default start_line = 1 and over-read). An end alone implies start = 1; a start
/// alone sets offset only (no limit). A nonsensical `end < start` is clamped to a single line
/// (`limit = 1`) rather than dropped, so the agent gets exactly the line it anchored on instead
/// of an unbounded read. All values are coerced via [`lenient_usize`] so string-typed numerics
/// (e.g. `"2"`) are honored rather than silently dropped, and each key is resolved through
/// [`get_arg`] so camel/kebab/Pascal spellings (`startLine`, `end-line`) map to their canonical
/// snake_case window parameter rather than being silently ignored (observed live in benchmark
/// transcripts, where a camel `startLine` fell through and the whole file rendered).
fn resolve_window_args(args: &Value) -> (Option<usize>, Option<usize>) {
    let as_usize = |key: &str| get_arg(args, key).and_then(lenient_usize);

    let explicit_offset = as_usize("offset");
    let explicit_limit = as_usize("limit");

    // start/end aliases, in descending priority: start_line/end_line beat start/end.
    let start_alias = as_usize("start_line").or_else(|| as_usize("start"));
    let end_alias = as_usize("end_line").or_else(|| as_usize("end"));

    // offset: explicit wins; else start alias; else (end alias present) 1.
    let offset = explicit_offset
        .or(start_alias)
        .or_else(|| end_alias.map(|_| 1));

    // limit: explicit wins; else derive the span relative to the EFFECTIVE start (offset wins
    // over the start alias) so a mixed offset+end window reads the intended range.
    let limit = explicit_limit.or_else(|| {
        end_alias.map(|end| {
            let effective_start = explicit_offset.or(start_alias).unwrap_or(1);
            end.saturating_sub(effective_start).saturating_add(1).max(1)
        })
    });

    (offset, limit)
}

pub fn read_file(args: &Value) -> Result<String, (i64, String)> {
    let file_path = resolve_file_path_arg(args)?;
    let (offset, limit) = resolve_window_args(args);

    let resolved = resolve_within_cwd(file_path)?;

    let metadata = std::fs::metadata(&resolved)
        .map_err(|e| (-32602, format!("Cannot read '{file_path}': {e}")))?;

    if metadata.is_dir() {
        return Err((
            -32602,
            format!("'{file_path}' is a directory, not a file. Use 'find' to list its contents."),
        ));
    }

    if let Some(ext) = resolved.extension().and_then(|s| s.to_str()) {
        if UNSUPPORTED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
            return Err((
                -32602,
                format!("Reading '.{ext}' files is not supported (binary/image/document)."),
            ));
        }
    }

    // Without an explicit window, cap the read so we never emit an unbounded blob.
    if limit.is_none() && metadata.len() > READ_FILE_BYTE_CAP {
        return Err((
            -32602,
            format!(
                "File content ({} bytes) exceeds the maximum read size of {READ_FILE_BYTE_CAP} bytes. Continue with a narrower window such as offset=1, limit=200, then advance offset by the number of lines read.",
                metadata.len()
            ),
        ));
    }

    let bytes = std::fs::read(&resolved)
        .map_err(|e| (-32603, format!("Failed to read '{file_path}': {e}")))?;

    // Decode lossily (invalid bytes → U+FFFD), matching Claude Code's Node `utf8` decode.
    // Binary files are gated by extension above, not by a NUL/UTF-8 hard reject — so a
    // file with stray non-UTF-8 bytes still reads with replacement characters.
    let decoded = String::from_utf8_lossy(&bytes);
    // Strip a leading UTF-8 BOM so it never appears inside line 1.
    let content = decoded.strip_prefix('\u{feff}').unwrap_or(decoded.as_ref());

    if content.is_empty() {
        return Ok(
            "<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>"
                .to_string(),
        );
    }

    // Split on '\n' (stripping a trailing '\r' per line for CRLF) and KEEP the trailing
    // empty segment, so a newline-terminated file counts its final empty line like Claude
    // Code ("a\nb\n" → 3 lines). `str::lines()` drops that segment and is off-by-one here.
    let all_lines: Vec<&str> = content
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let total = all_lines.len();
    // offset is 1-indexed; 0 is treated as 1.
    let start_line = offset.unwrap_or(1).max(1);

    if start_line > total {
        return Ok(format!(
            "<system-reminder>Warning: the file exists but is shorter than the provided offset ({start_line}). The file has {total} lines.</system-reminder>"
        ));
    }

    let window: Vec<&str> = match limit {
        Some(n) => all_lines[start_line - 1..]
            .iter()
            .take(n)
            .copied()
            .collect(),
        None => all_lines[start_line - 1..].to_vec(),
    };

    let rendered = add_line_numbers(&window, start_line);
    // Always-applied output ceiling: even with `offset`/`limit` set, never emit an
    // unbounded blob (e.g. a multi-MB single line read with `limit: 1`). Throws rather than
    // truncating, matching Claude Code's token-cap behavior.
    let output_cap = crate::config::get().read_output_byte_cap;
    if rendered.len() > output_cap {
        let requested_line_count = window.len().max(1);
        let suggested_limit = requested_line_count
            .saturating_mul(output_cap)
            .checked_div(rendered.len())
            .unwrap_or(1)
            .saturating_sub(1)
            .max(1);
        return Err((
            -32602,
            format!(
                "Read output ({} bytes) exceeds the maximum of {output_cap} bytes. Continue with a narrower window such as offset={start_line}, limit={suggested_limit}. If limit=1 still exceeds the cap, use grep to locate narrower text first.",
                rendered.len(),
            ),
        ));
    }
    Ok(rendered)
}
