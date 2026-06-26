pub(crate) fn is_root_alias(path: Option<&str>) -> bool {
    path.is_some_and(crate::codemap::is_all_workspace_scope_input)
}

pub(crate) fn resolve_path(
    raw_path: Option<&str>,
    files: &[crate::parser::ExtractedFile],
) -> Option<String> {
    raw_path.and_then(|path| crate::codemap::resolve_workspace_path_input(files, path))
}

pub(crate) fn root_view(files: &[crate::parser::ExtractedFile]) -> Option<String> {
    crate::codemap::generate_monorepo_root_view(files)
}
