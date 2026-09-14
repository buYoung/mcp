use super::LiveAnchor;
use crate::parser::ExtractedFile;
use std::path::Path;

pub(super) fn read_hint(path: &str, line: usize) -> String {
    format!(
        "read {}",
        serde_json::json!({"file_path":path,"offset":line.max(1),"limit":12,"view":"source"})
    )
}

/// Diagnose only a requested, permission-checked path. Never enumerate excluded
/// source while trying to explain a failed query.
pub(super) fn unavailable(
    file: Option<&ExtractedFile>,
    anchor: &LiveAnchor,
    root: &Path,
    flow_digest: Option<&str>,
) -> Option<String> {
    let path = root.join(&anchor.file_path);
    let line = anchor.start_line.unwrap_or(1);
    let cfg = crate::config::get();
    let reason = if !crate::workspace::walk_root_is_visible(&path, true) {
        "excluded from the current index by path/Git rules".to_string()
    } else if crate::lang::spec_for_path(&path).is_none() {
        "file format is unsupported or disabled for indexing".to_string()
    } else if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > cfg.max_file_size) {
        format!(
            "file exceeds index.max_file_size ({} bytes)",
            cfg.max_file_size
        )
    } else if file.is_none() {
        "file is not in this index generation; it may be newly created or previously excluded"
            .into()
    } else {
        let expected = file
            .and_then(|file| file.navigation.as_ref())
            .and_then(|navigation| navigation.implementations.as_ref())
            .map(|facts| facts.source_digest.as_str())
            .or(flow_digest)?;
        if std::fs::read(&path)
            .is_ok_and(|source| crate::implementations::digest(&source) == expected)
        {
            return None;
        }
        "source changed since indexing; declaration and relation locations are withheld until refresh".into()
    };
    Some(format!(
        "[Navigation unavailable — {}:{line}: {reason}. Next: {}]\n",
        anchor.file_path,
        read_hint(&anchor.file_path, line)
    ))
}

pub(super) fn no_declaration(file: &ExtractedFile, line: usize, relations_only: bool) -> String {
    let reason = if relations_only {
        "no callable declaration overlaps the returned lines"
    } else if file.symbols.is_empty() {
        "the indexed parser result contains no declarations; expressions/comments/literals can still be present"
    } else {
        "returned lines do not overlap an indexed declaration"
    };
    let nearest = file
        .symbols
        .iter()
        .filter(|symbol| !relations_only || super::structure::callable(symbol))
        .min_by_key(|symbol| symbol.range.start_line.abs_diff(line));
    let next = nearest.map_or_else(
        || read_hint(&file.file_path, line),
        |symbol| read_hint(&file.file_path, symbol.range.start_line),
    );
    format!(
        "[No declaration context — {}:{line}: {reason}. Next: {next}]\n",
        file.file_path
    )
}
