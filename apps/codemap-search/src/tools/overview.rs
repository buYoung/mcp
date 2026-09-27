//! The `overview` tool body: reads the committed codemap snapshot the indexer publishes and
//! renders the root, folder, or single-file view. Orchestration only.
//!
//! The dispatch arm (`crate::mcp`) calls `EngineSupervisor::ensure_alive`/`trigger_refresh`
//! before delegating here; this body only reads the committed snapshot through `ctx.engine`.

mod monorepo;
mod stats;

use crate::codemap::CodemapView;
use crate::tools::ToolContext;

pub(crate) struct PreparedOverview {
    snapshot: std::sync::Arc<Vec<crate::parser::ExtractedFile>>,
    scope: Option<(String, bool)>,
    root_text: String,
    stats: String,
}

impl PreparedOverview {
    pub(crate) fn is_root(&self) -> bool {
        self.scope.is_none()
    }

    pub(crate) fn render(&self) -> String {
        let text = match &self.scope {
            Some((path, true)) => {
                let file = self
                    .snapshot
                    .iter()
                    .find(|file| &file.file_path == path)
                    .expect("file presence checked during preparation");
                crate::codemap::CodemapGenerator::generate_detail_view(file).to_markdown()
            }
            Some((path, false)) => {
                crate::codemap::CodemapGenerator::generate_folder_view(&self.snapshot, path)
                    .to_markdown()
            }
            None => self.root_text.clone(),
        };
        text + &self.stats
    }

    pub(crate) async fn filter(
        &self,
        ctx: &ToolContext<'_>,
        task: &crate::tools::task::RegisteredTask,
        evaluator: &dyn crate::jev::Evaluator,
        policy: &crate::tools::search::jev::FilterPolicy,
    ) -> (String, crate::tools::search::jev::FilterResult) {
        use crate::tools::{live_symbols::jev::overview, search::jev};
        let (path, is_file) = self.scope.as_ref().expect("root is never filtered");
        // Materialize signatures before awaiting Jev; neither applying decisions nor
        // rendering the response may read a later filesystem state.
        let mut folder = (!is_file)
            .then(|| crate::codemap::CodemapGenerator::generate_folder_view(&self.snapshot, path));
        let mut files: Vec<_> = self
            .snapshot
            .iter()
            .filter(|file| {
                if *is_file {
                    file.file_path == *path
                } else {
                    std::path::Path::new(&file.file_path)
                        .parent()
                        .is_some_and(|parent| parent == std::path::Path::new(path))
                }
            })
            .collect();
        files.sort_by(|left, right| left.file_path.cmp(&right.file_path));
        let base_text = folder.as_ref().map_or_else(
            || crate::codemap::CodemapGenerator::generate_detail_view(files[0]).to_markdown(),
            CodemapView::to_markdown,
        );
        let mut plan = overview::capture(&files, task, ctx.arguments, policy);
        plan.add_indexed_context(ctx.engine, policy);
        let mut result = jev::evaluate(&plan.input, evaluator, policy).await;
        let omitted = plan.omitted(&result);
        if omitted.is_empty() {
            return (base_text + &self.stats, result);
        }
        let mut text = if let Some(folder) = &mut folder {
            for file in &mut folder.files {
                file.symbols.retain(|symbol| {
                    !omitted.contains(&overview::DeclarationKey::new(
                        &file.file_path,
                        symbol.name,
                        symbol.kind,
                        symbol.start_line,
                        symbol.end_line,
                    ))
                });
                if let Some(outline) = &mut file.outline {
                    outline.retain(|symbol| {
                        !omitted.contains(&overview::DeclarationKey::from_symbol(
                            &file.file_path,
                            symbol,
                        ))
                    });
                }
            }
            folder.to_markdown()
        } else {
            let mut file = (*files[0]).clone();
            file.symbols = crate::codemap::significant_symbols(&file.symbols)
                .filter(|symbol| {
                    !omitted.contains(&overview::DeclarationKey::from_symbol(
                        &file.file_path,
                        symbol,
                    ))
                })
                .cloned()
                .collect();
            crate::codemap::CodemapGenerator::generate_detail_view(&file).to_markdown()
        };
        result.rendered_omissions = omitted.len();
        let note = format!("\n_Jev omitted {} declarations._\n", omitted.len());
        if text.len().saturating_add(note.len()) <= base_text.len() {
            text.push_str(&note);
        }
        text.push_str(&self.stats);
        (text, result)
    }
}

/// Run the `overview` tool and return the rendered codemap text (or a warming/dead notice).
/// The MCP dispatch arm wraps the returned string in the JSON-RPC `content` envelope.
pub fn run(ctx: &ToolContext) -> Result<String, (i64, String)> {
    Ok(prepare(ctx)?.render())
}

pub(crate) fn prepare(ctx: &ToolContext) -> Result<PreparedOverview, (i64, String)> {
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
        return Ok(PreparedOverview {
            snapshot,
            scope: None,
            root_text: text.into(),
            stats: String::new(),
        });
    }

    let scope = resolved_path.as_ref().and_then(|target| {
        let relative = crate::workspace::workspace_relative_key(target, &cwd);
        (!relative.is_empty()).then_some((relative, target.is_file()))
    });
    let root_text = if let Some(p) = path.filter(|_| scope.is_some()) {
        let target_path = resolved_path
            .as_ref()
            .ok_or_else(|| (-32603, format!("Failed to process path '{}'", p)))?;
        if target_path.is_file() {
            let rel_path_str = crate::workspace::workspace_relative_key(target_path, &cwd);
            if !extracted_files.iter().any(|f| f.file_path == rel_path_str) {
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
        }
        String::new()
    } else {
        if format == Some("llms-txt") {
            crate::codemap::CodemapGenerator::generate_llms_txt_view(extracted_files)
        } else if let Some(text) = monorepo::root_view(catalog) {
            text
        } else {
            crate::codemap::CodemapGenerator::generate_root_view(extracted_files).to_markdown()
        }
    };

    let mut stats_text = String::new();
    if let Some(stats_scope) = stats_scope {
        let workspace = cwd.to_string_lossy().into_owned();
        let snapshot_id = std::sync::Arc::as_ptr(&published).addr();
        stats_text.push_str("\n\n");
        stats_text.push_str(&stats::render(
            &cwd,
            &workspace,
            snapshot_id,
            &published,
            extracted_files,
            stats_scope,
        ));
    }

    Ok(PreparedOverview {
        snapshot,
        scope,
        root_text,
        stats: stats_text,
    })
}
