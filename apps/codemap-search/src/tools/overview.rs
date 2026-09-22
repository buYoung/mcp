//! The `overview` tool body: reads the committed codemap snapshot the indexer publishes and
//! renders the root, folder, or single-file view. Orchestration only.
//!
//! The dispatch arm (`crate::mcp`) calls `EngineSupervisor::ensure_alive`/`trigger_refresh`
//! before delegating here; this body only reads the committed snapshot through `ctx.engine`.

mod monorepo;
mod stats;

use crate::tools::ToolContext;

/// Run the `overview` tool and return the rendered codemap text (or a warming/dead notice).
/// The MCP dispatch arm wraps the returned string in the JSON-RPC `content` envelope.
pub mod jev;

/// A root-only, same-generation input for optional asynchronous recommendations.
pub struct PreparedOverview {
    pub text: String,
    pub files: Vec<crate::parser::ExtractedFile>,
    pub snapshot_id: usize,
    pub is_eligible: bool,
}

pub fn run(ctx: &ToolContext) -> Result<String, (i64, String)> {
    prepare(ctx).map(|prepared| prepared.text)
}

pub fn prepare(ctx: &ToolContext) -> Result<PreparedOverview, (i64, String)> {
    // Accept the same path aliases as `read` ('file_path'/'file'/'query'):
    // an unknown param (e.g. `{"query": "file.cpp"}`) used to silently fall
    // back to the ROOT overview, wasting agent turns. Earlier aliases win.
    // An empty or "." path means the repo root overview, not a folder
    // named "" — normalize so it renders the root view (Child 03).
    let raw_path = ["path", "file_path", "file", "query"]
        .iter()
        .find_map(|key| ctx.arguments.get(*key).and_then(|v| v.as_str()))
        .filter(|p| !p.is_empty() && *p != ".");
    let format = ctx.arguments.get("format").and_then(|v| v.as_str());

    let cwd = std::env::current_dir()
        .map_err(|e| (-32603, format!("Error getting current dir: {}", e)))?;

    // Keep the readiness observed before this snapshot so a concurrent initial publish
    // cannot make an empty pre-index snapshot lose its warm-up notice.
    let is_warming = ctx.engine.is_warming();
    let published = ctx.engine.published_snapshot();
    let catalog = published.workspace_catalog();
    let snapshot = published.codemap();
    let extracted_files: &[crate::parser::ExtractedFile] = &snapshot;

    if raw_path.is_some_and(|path| catalog.is_ambiguous(path)) {
        return Err((
            -32602,
            "Ambiguous workspace scope. Use the canonical path shown by root overview.".to_string(),
        ));
    }

    let workspace_resolved_path = monorepo::resolve_path(raw_path, catalog);
    let path = if monorepo::is_root_alias(raw_path) {
        None
    } else {
        workspace_resolved_path.as_deref().or(raw_path)
    };

    let resolved_path = match path {
        Some(p) => Some(
            crate::workspace::resolve_within_cwd(p)
                .map_err(|_| (-32602, "Path traversal detected".to_string()))?,
        ),
        None => None,
    };

    let stats_scope = if crate::config::get().is_overview_stats_enabled {
        match resolved_path.as_deref() {
            None => Some(stats::StatsScope::Root),
            Some(target_path) if target_path.is_dir() => {
                let relative_path = crate::workspace::workspace_relative_key(target_path, &cwd);
                if relative_path.is_empty() {
                    Some(stats::StatsScope::Root)
                } else if catalog.is_workspace_root(&relative_path) {
                    Some(stats::StatsScope::WorkspaceRoot(relative_path))
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    };

    // Nothing to show yet because the initial index is still building (or
    // the indexer thread died before it finished): say so rather than
    // render an empty codemap.
    if extracted_files.is_empty() && (is_warming || ctx.engine.is_dead()) {
        let text = if ctx.engine.is_dead() {
            "Background indexer stopped before the codemap was built; restart the server. Use find/grep/read for live results."
        } else {
            "Codemap is warming up (initial background indexing in progress). Retry shortly, or use find/grep/read for live results."
        };
        return Ok(PreparedOverview { text: text.to_string(), files: Vec::new(), snapshot_id: 0, is_eligible: false });
    }

    use crate::codemap::CodemapView;
    let mut codemap_text = if let Some(p) = path {
        let target_path = resolved_path
            .as_ref()
            .ok_or_else(|| (-32603, format!("Failed to process path '{}'", p)))?;
        if target_path.is_file() {
            let rel_path_str = crate::workspace::workspace_relative_key(target_path, &cwd);
            if let Some(file) = extracted_files.iter().find(|f| f.file_path == rel_path_str) {
                crate::codemap::CodemapGenerator::generate_detail_view(file).to_markdown()
            } else {
                // On disk but absent from the codemap: skipped, not
                // broken — non-source extension, over the size cap, or
                // unparseable. Say so rather than imply a failure, and name
                // the fallback: agents were observed retrying `overview` on
                // the same script/doc file across sessions before switching
                // to `read` on their own. Keep the "is not in the codemap"
                // prefix verbatim — the e2e helpers key their index-warmup
                // retry on it.
                let index_size_cap = crate::config::get().max_file_size;
                let fallback_hint = if crate::tools::read::is_unsupported_extension(target_path) {
                    "This extension is binary/document, so `read` cannot open it either."
                        .to_string()
                } else if std::fs::metadata(target_path)
                    .map(|m| m.len() > index_size_cap)
                    .unwrap_or(false)
                {
                    format!(
                        "It exceeds the index size cap ({index_size_cap} bytes) but exists on disk; use `read` with offset/limit windows for its content."
                    )
                } else if let Some(reason) =
                    crate::workspace::source_encoding_exclusion(target_path)
                {
                    reason
                } else {
                    "The file exists on disk; use `read` for its raw content (offset/limit for large files)."
                        .to_string()
                };
                return Err((-32602, format!(
                    "File '{}' is not in the codemap (not a supported source file, exceeds the size cap, or could not be parsed). {fallback_hint}",
                    p
                )));
            }
        } else {
            crate::codemap::CodemapGenerator::generate_folder_view(extracted_files, p).to_markdown()
        }
    } else {
        if format == Some("llms-txt") {
            crate::codemap::CodemapGenerator::generate_llms_txt_view(extracted_files)
        } else if let Some(text) = monorepo::root_view(catalog) {
            text
        } else {
            crate::codemap::CodemapGenerator::generate_root_view(extracted_files).to_markdown()
        }
    };

    if let Some(stats_scope) = stats_scope {
        let workspace = cwd.to_string_lossy().into_owned();
        let snapshot_id = std::sync::Arc::as_ptr(&published).addr();
        codemap_text.push_str("\n\n");
        codemap_text.push_str(&stats::render(
            &cwd,
            &workspace,
            snapshot_id,
            &published,
            extracted_files,
            stats_scope,
        ));
    }

    let is_eligible = path.is_none() && format != Some("llms-txt")
        && !extracted_files.is_empty() && !ctx.engine.is_dead()
        && !is_warming && ctx.engine.last_error().is_none();
    Ok(PreparedOverview {
        text: codemap_text,
        files: if is_eligible { extracted_files.to_vec() } else { Vec::new() },
        snapshot_id: std::sync::Arc::as_ptr(&published).addr(),
        is_eligible,
    })
}
