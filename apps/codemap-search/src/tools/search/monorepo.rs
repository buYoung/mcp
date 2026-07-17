use crate::tools::ToolContext;

const SCOPED_SEARCH_LIMIT: usize = 500;

pub(crate) fn should_use(ctx: &ToolContext) -> bool {
    ctx.active_workspace_scope.is_some()
        || crate::tools::get_arg(ctx.arguments, "workspace_scope").is_some()
        || crate::tools::get_arg(ctx.arguments, "scope").is_some()
}

pub(crate) fn result_is_under_scope(result: &crate::index::SearchResult, scope: &str) -> bool {
    path_is_under_scope(&result.file_path, scope)
}

pub(crate) fn path_is_under_scope(path: &str, scope: &str) -> bool {
    path == scope || path.starts_with(&format!("{scope}/"))
}

fn requested_workspace_scope(ctx: &ToolContext) -> Result<Option<String>, (i64, String)> {
    let explicit_scope = crate::tools::get_arg(ctx.arguments, "workspace_scope")
        .or_else(|| crate::tools::get_arg(ctx.arguments, "scope"))
        .and_then(|value| value.as_str());

    let snapshot = ctx.engine.codemap_snapshot();
    match explicit_scope {
        Some(raw_scope) if crate::codemap::is_all_workspace_scope_input(raw_scope) => Ok(None),
        Some(raw_scope) => crate::codemap::workspace_scope_for_input(&snapshot, raw_scope)
            .map(Some)
            .ok_or_else(|| {
                (
                    -32602,
                    format!(
                        "Unknown workspace_scope '{raw_scope}'. Run root overview to list workspace scopes, or pass workspace_scope=\"all\" for a repo-wide search."
                    ),
                )
            }),
        None => Ok(ctx.active_workspace_scope.map(ToString::to_string)),
    }
}

pub(crate) fn run_with_metadata(ctx: &ToolContext) -> Result<super::SearchOutput, (i64, String)> {
    let workspace_scope = requested_workspace_scope(ctx)?;
    match workspace_scope {
        Some(scope) => super::run_inner_with_metadata(ctx, Some(&scope), SCOPED_SEARCH_LIMIT),
        None => super::run_inner_with_metadata(ctx, None, super::DEFAULT_SEARCH_LIMIT),
    }
}
