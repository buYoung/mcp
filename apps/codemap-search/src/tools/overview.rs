//! The `overview` tool body: reads the committed codemap snapshot the indexer publishes and
//! renders the root, folder, or single-file view. Orchestration only.
//!
//! The dispatch arm (`crate::mcp`) calls `EngineSupervisor::ensure_alive`/`trigger_refresh`
//! before delegating here; this body only reads the committed snapshot through `ctx.engine`.

mod monorepo;

use crate::tools::ToolContext;

/// Run the `overview` tool and return the rendered codemap text (or a warming/dead notice).
/// The MCP dispatch arm wraps the returned string in the JSON-RPC `content` envelope.
pub fn run(ctx: &ToolContext) -> Result<String, (i64, String)> {
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

    let snapshot = ctx.engine.codemap_snapshot();
    let extracted_files: &[crate::parser::ExtractedFile] = &snapshot;

    if raw_path.is_some_and(|path| {
        crate::codemap::is_ambiguous_workspace_scope_input(extracted_files, path)
    }) {
        return Err((
            -32602,
            "Ambiguous workspace scope. Use the canonical path shown by root overview.".to_string(),
        ));
    }

    let workspace_resolved_path = monorepo::resolve_path(raw_path, extracted_files);
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

    // Nothing to show yet because the initial index is still building (or
    // the indexer thread died before it finished): say so rather than
    // render an empty codemap.
    if extracted_files.is_empty() && (ctx.engine.is_warming() || ctx.engine.is_dead()) {
        let text = if ctx.engine.is_dead() {
            "Background indexer stopped before the codemap was built; restart the server. Use find/grep/read for live results."
        } else {
            "Codemap is warming up (initial background indexing in progress). Retry shortly, or use find/grep/read for live results."
        };
        return Ok(text.to_string());
    }

    use crate::codemap::CodemapView;
    let codemap_text = if let Some(p) = path {
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
                // unparseable. Say so rather than imply a failure.
                return Err((-32602, format!(
                    "File '{}' is not in the codemap (not a supported source file, exceeds the size cap, or could not be parsed)",
                    p
                )));
            }
        } else {
            crate::codemap::CodemapGenerator::generate_folder_view(extracted_files, p).to_markdown()
        }
    } else {
        if format == Some("llms-txt") {
            crate::codemap::CodemapGenerator::generate_llms_txt_view(extracted_files)
        } else if let Some(text) = monorepo::root_view(extracted_files) {
            text
        } else {
            crate::codemap::CodemapGenerator::generate_root_view(extracted_files).to_markdown()
        }
    };

    Ok(codemap_text)
}
