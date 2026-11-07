use crate::tools::ToolContext;

pub(crate) fn should_use(ctx: &ToolContext) -> bool {
    crate::tools::get_arg(ctx.arguments, "workspace_scope").is_some()
        || crate::tools::get_arg(ctx.arguments, "scope").is_some()
}

pub(crate) fn result_is_under_scope(result: &crate::index::SearchResult, scope: &str) -> bool {
    path_is_under_scope(&result.file_path, scope)
}

pub(crate) fn path_is_under_scope(path: &str, scope: &str) -> bool {
    path == scope || path.starts_with(&format!("{scope}/"))
}

pub(super) fn requested_workspace_scope(
    ctx: &ToolContext,
) -> Result<Option<String>, (i64, String)> {
    let explicit_scope = crate::tools::get_arg(ctx.arguments, "workspace_scope")
        .or_else(|| crate::tools::get_arg(ctx.arguments, "scope"))
        .and_then(|value| value.as_str());
    // A scope in another root names a directory of that root's index, or a file selecting its
    // parent; the root itself searches the whole index.
    if let Some(root) = crate::config::get().workspace_root.clone() {
        let start = crate::workspace::current_dir()?;
        let path =
            crate::workspace::resolve_path_lenient_from_cwd(&start, explicit_scope.unwrap_or("."));
        let directory = match path.parent() {
            Some(parent) if path.is_file() => parent.to_path_buf(),
            _ => path,
        };
        return Ok((directory != root).then(|| directory.to_string_lossy().replace('\\', "/")));
    }

    let snapshot = ctx.engine.published_snapshot();
    let catalog = snapshot.workspace_catalog();
    match explicit_scope {
        Some(raw_scope) if crate::codemap::is_all_workspace_scope_input(raw_scope) => Ok(None),
        Some(raw_scope)
            if catalog.is_ambiguous(raw_scope) =>
        {
            Err((
                -32602,
                "Ambiguous workspace scope. Use the canonical path listed by initial_instructions."
                    .to_string(),
            ))
        }
        Some(raw_scope) => catalog.search_scope_for_input(raw_scope)
            .map(Some)
            .ok_or_else(|| {
                (
                    -32602,
                    format!(
                        "Unknown workspace_scope '{raw_scope}'. Call initial_instructions to list workspace scopes, or pass workspace_scope=\"all\" for a repo-wide search."
                    ),
                )
            }),
        None => Ok(None),
    }
}

pub(crate) fn run_with_metadata(ctx: &ToolContext) -> Result<super::SearchOutput, (i64, String)> {
    let workspace_scope = requested_workspace_scope(ctx)?;
    match workspace_scope {
        Some(scope) => {
            super::run_inner_with_metadata(ctx, Some(&scope), super::DEFAULT_SEARCH_LIMIT)
        }
        None => super::run_inner_with_metadata(ctx, None, super::DEFAULT_SEARCH_LIMIT),
    }
}
