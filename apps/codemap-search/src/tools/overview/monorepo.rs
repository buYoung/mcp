pub(crate) fn is_root_alias(path: Option<&str>) -> bool {
    path.is_some_and(crate::codemap::is_all_workspace_scope_input)
}

pub(crate) fn resolve_path(
    raw_path: Option<&str>,
    catalog: &crate::codemap::WorkspaceCatalog,
) -> Option<String> {
    raw_path.and_then(|path| catalog.resolve_path(path))
}

pub(crate) fn root_view(catalog: &crate::codemap::WorkspaceCatalog) -> Option<String> {
    catalog.root_view()
}
