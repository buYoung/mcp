use super::summary::{
    build_directory_summaries, summarize_file, DirectorySummary, ExtractedFileSummary,
};

/// Conventional monorepo container directories. Monorepo-specific views treat their
/// immediate children as workspace scopes (`apps/api`, `packages/ui`, ...).
const WORKSPACE_CONTAINER_DIRS: &[&str] = &["apps", "packages", "crates", "libs", "services"];

fn immediate_child_under(path: &str, parent: &str) -> bool {
    let Some(remainder) = path.strip_prefix(parent).and_then(|p| p.strip_prefix('/')) else {
        return false;
    };
    !remainder.is_empty() && !remainder.contains('/')
}

fn conventional_workspace_scope_summaries(
    directories: &[DirectorySummary],
) -> Vec<&DirectorySummary> {
    directories
        .iter()
        .filter(|dir| {
            WORKSPACE_CONTAINER_DIRS
                .iter()
                .any(|container| immediate_child_under(&dir.path, container))
        })
        .collect()
}

fn workspace_scope_summaries(directories: &[DirectorySummary]) -> Vec<&DirectorySummary> {
    let scopes = conventional_workspace_scope_summaries(directories);
    if scopes.len() >= 2 {
        scopes
    } else {
        Vec::new()
    }
}

fn top_level_source_roots(directories: &[DirectorySummary]) -> Vec<&DirectorySummary> {
    directories
        .iter()
        .filter(|dir| !dir.path.contains('/'))
        .filter(|dir| !WORKSPACE_CONTAINER_DIRS.contains(&dir.path.as_str()))
        // Keep the documented repo-wide aliases unambiguous: a top-level directory
        // named after one is ordinary source content, but not a selectable scope.
        .filter(|dir| !is_all_workspace_scope_input(&dir.path))
        .collect()
}

/// The paths a monorepo root overview offers as selectable scopes. Conventional workspace
/// children establish the monorepo boundary; top-level source roots then participate in that
/// same selection contract rather than being informational-only output.
fn selectable_scope_summaries(directories: &[DirectorySummary]) -> Vec<&DirectorySummary> {
    let mut scopes = workspace_scope_summaries(directories);
    if scopes.is_empty() && looks_like_monorepo_workspace() {
        scopes = conventional_workspace_scope_summaries(directories);
    }
    if !scopes.is_empty() {
        scopes.extend(top_level_source_roots(directories));
    }
    scopes.sort_by(|left, right| left.path.cmp(&right.path));
    scopes.dedup_by(|left, right| left.path == right.path);
    scopes
}

fn file_summaries(files: &[crate::parser::ExtractedFile]) -> Vec<ExtractedFileSummary<'_>> {
    let mut files_summary: Vec<ExtractedFileSummary<'_>> =
        files.iter().map(summarize_file).collect();
    files_summary.sort_by(|left, right| left.file_path.cmp(&right.file_path));
    files_summary
}

fn directory_summaries(files: &[crate::parser::ExtractedFile]) -> Vec<DirectorySummary> {
    build_directory_summaries(&file_summaries(files))
}

fn filesystem_workspace_scope_paths() -> Vec<String> {
    let mut scopes = Vec::new();
    for container in WORKSPACE_CONTAINER_DIRS {
        let Ok(entries) = std::fs::read_dir(container) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
                continue;
            }
            scopes.push(format!(
                "{container}/{}",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    scopes.sort();
    if scopes.len() >= 2 {
        scopes
    } else {
        Vec::new()
    }
}

fn workspace_scope_paths_for_resolution(files: &[crate::parser::ExtractedFile]) -> Vec<String> {
    let scopes = workspace_scope_paths(files);
    if scopes.is_empty() {
        filesystem_workspace_scope_paths()
    } else {
        scopes
    }
}

pub fn is_all_workspace_scope_input(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("all")
        || trimmed.eq_ignore_ascii_case("root")
        || trimmed.eq_ignore_ascii_case("repo")
        || trimmed == "전체"
    {
        return true;
    }
    super::normalize_path(trimmed).is_empty()
}

pub fn workspace_scope_paths(files: &[crate::parser::ExtractedFile]) -> Vec<String> {
    let directories = directory_summaries(files);
    selectable_scope_summaries(&directories)
        .into_iter()
        .map(|scope| scope.path.clone())
        .collect()
}

pub fn is_ambiguous_workspace_scope_input(
    files: &[crate::parser::ExtractedFile],
    input: &str,
) -> bool {
    if is_all_workspace_scope_input(input) {
        return false;
    }
    let normalized = super::normalize_path(input);
    let scopes = workspace_scope_paths_for_resolution(files);
    if scopes
        .iter()
        .any(|scope| normalized == *scope || normalized.starts_with(&format!("{scope}/")))
    {
        return false;
    }
    let head = normalized.split('/').next().unwrap_or_default();
    scopes
        .iter()
        .filter(|scope| scope.rsplit('/').next() == Some(head))
        .take(2)
        .count()
        > 1
}

pub fn resolve_workspace_path_input(
    files: &[crate::parser::ExtractedFile],
    input: &str,
) -> Option<String> {
    if is_all_workspace_scope_input(input) {
        return None;
    }
    let normalized = super::normalize_path(input).into_owned();
    let scopes = workspace_scope_paths_for_resolution(files);
    if scopes
        .iter()
        .any(|scope| normalized == *scope || normalized.starts_with(&format!("{scope}/")))
    {
        return Some(normalized);
    }
    let (head, tail) = normalized
        .split_once('/')
        .map_or((normalized.as_str(), ""), |(head, tail)| (head, tail));
    let mut matches = scopes
        .iter()
        .filter(|scope| scope.rsplit('/').next() == Some(head))
        .collect::<Vec<_>>();
    if is_ambiguous_workspace_scope_input(files, input) {
        return None;
    }
    if matches.is_empty() {
        return None;
    }
    let scope = matches.remove(0);
    if tail.is_empty() {
        Some(scope.clone())
    } else {
        Some(format!("{scope}/{tail}"))
    }
}

pub fn workspace_scope_for_input(
    files: &[crate::parser::ExtractedFile],
    input: &str,
) -> Option<String> {
    if is_all_workspace_scope_input(input) {
        return None;
    }
    let normalized = resolve_workspace_path_input(files, input)?;
    workspace_scope_paths_for_resolution(files)
        .into_iter()
        .filter(|scope| normalized == *scope || normalized.starts_with(&format!("{scope}/")))
        .max_by_key(|scope| scope.len())
}

pub fn looks_like_monorepo_workspace() -> bool {
    !filesystem_workspace_scope_paths().is_empty()
}

pub struct MonorepoRootCodemap {
    total_files: usize,
    total_symbols: usize,
    scopes: Vec<DirectorySummary>,
}

impl std::fmt::Display for MonorepoRootCodemap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(formatter, "# Root Codemap Overview")?;
        writeln!(formatter)?;
        writeln!(formatter, "- **Total Files**: {}", self.total_files)?;
        writeln!(formatter, "- **Total Symbols**: {}", self.total_symbols)?;
        writeln!(formatter)?;
        writeln!(
            formatter,
            "## Workspace Scopes (choose one before reading or editing)"
        )?;
        for scope in &self.scopes {
            writeln!(
                formatter,
                "- {} ({} files, {} symbols)",
                scope.path, scope.file_count, scope.symbol_count
            )?;
        }
        writeln!(formatter)?;
        writeln!(formatter, "## Next Step")?;
        writeln!(
            formatter,
            "- For broad requests, ask which workspace scope to use before acting."
        )?;
        writeln!(
            formatter,
            "- If the user wants a repo-wide change, treat `all` / `전체` as an explicit whole-repo scope."
        )?;
        writeln!(
            formatter,
            "- `overview <scope>` sets the active scope for following `search` calls; `workspace_scope: \"all\"` searches the whole repo."
        )?;
        Ok(())
    }
}

pub fn generate_root_view(files: &[crate::parser::ExtractedFile]) -> Option<String> {
    let summaries = file_summaries(files);
    let total_files = files.len();
    let total_symbols = summaries.iter().map(|summary| summary.symbol_count).sum();
    let directories = build_directory_summaries(&summaries);
    let scopes = selectable_scope_summaries(&directories)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    if scopes.is_empty() {
        return None;
    }
    Some(
        MonorepoRootCodemap {
            total_files,
            total_symbols,
            scopes,
        }
        .to_string(),
    )
}
