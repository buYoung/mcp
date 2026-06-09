//! `read` — single-file read with `cat -n` style line numbers, mirroring Claude
//! Code's Read tool so an agent can swap its built-in without surprises. Direct
//! filesystem I/O, no index/engine dependency.

use super::{arg_required_str, resolve_within_cwd};
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

pub fn read_file(args: &Value) -> Result<String, (i64, String)> {
    let file_path = arg_required_str(args, "file_path")?;
    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

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
                "File content ({} bytes) exceeds the maximum read size of {READ_FILE_BYTE_CAP} bytes. Use the offset and limit parameters to read it in smaller chunks.",
                metadata.len()
            ),
        ));
    }

    let bytes = std::fs::read(&resolved)
        .map_err(|e| (-32603, format!("Failed to read '{file_path}': {e}")))?;

    // Reject binary content (NUL byte or invalid UTF-8) even without a known extension.
    if bytes.contains(&0) {
        return Err((
            -32602,
            format!("'{file_path}' appears to be a binary file."),
        ));
    }
    let content = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return Err((-32602, format!("'{file_path}' is not valid UTF-8 text."))),
    };

    if content.is_empty() {
        return Ok(
            "<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>"
                .to_string(),
        );
    }

    let all_lines: Vec<&str> = content.lines().collect();
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

    Ok(add_line_numbers(&window, start_line))
}
