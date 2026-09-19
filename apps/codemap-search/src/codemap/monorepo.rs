use super::summary::{
    build_directory_summaries, significant_symbols, DirectorySummary, ExtractedFileSummary,
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
    let mut files_summary: Vec<ExtractedFileSummary<'_>> = files
        .iter()
        .map(|file| ExtractedFileSummary {
            file_path: super::normalize_path(&file.file_path).into_owned(),
            total_lines: file.total_lines,
            symbol_count: significant_symbols(&file.symbols).count(),
            symbols: Vec::new(),
            outline: None,
        })
        .collect();
    files_summary.sort_by(|left, right| left.file_path.cmp(&right.file_path));
    files_summary
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

/// Scope metadata belongs to one published index generation, not to an individual request.
#[derive(Debug, Clone)]
pub(crate) struct WorkspaceCatalog {
    total_files: usize,
    total_symbols: usize,
    scopes: Vec<DirectorySummary>,
    paths: Vec<String>,
    languages: std::collections::BTreeMap<String, Vec<(String, usize)>>,
}

impl WorkspaceCatalog {
    pub(crate) fn new(files: &[crate::parser::ExtractedFile]) -> Self {
        let summaries = file_summaries(files);
        let directories = build_directory_summaries(&summaries);
        let scopes: Vec<_> = selectable_scope_summaries(&directories)
            .into_iter()
            .cloned()
            .collect();
        let paths = if scopes.is_empty() {
            filesystem_workspace_scope_paths()
        } else {
            scopes.iter().map(|scope| scope.path.clone()).collect()
        };
        let mut counts: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, usize>,
        > = std::collections::BTreeMap::new();
        for file in files {
            let path = super::normalize_path(&file.file_path);
            let language = crate::lang::language_name_for_path(std::path::Path::new(path.as_ref()))
                .unwrap_or("other");
            for scope in &scopes {
                if path == scope.path || path.starts_with(&format!("{}/", scope.path)) {
                    *counts
                        .entry(scope.path.clone())
                        .or_default()
                        .entry(language.to_string())
                        .or_default() += 1;
                }
            }
        }
        let languages = counts
            .into_iter()
            .map(|(scope, counts)| {
                let mut languages: Vec<_> = counts.into_iter().collect();
                languages.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
                languages.truncate(3);
                (scope, languages)
            })
            .collect();
        Self {
            total_files: files.len(),
            total_symbols: summaries.iter().map(|file| file.symbol_count).sum(),
            scopes,
            paths,
            languages,
        }
    }

    pub(crate) fn is_ambiguous(&self, input: &str) -> bool {
        if is_all_workspace_scope_input(input) {
            return false;
        }
        let normalized = super::normalize_path(input);
        let scopes = &self.paths;
        if scopes.iter().any(|scope| {
            normalized == scope.as_str() || normalized.starts_with(&format!("{scope}/"))
        }) {
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

    pub(crate) fn resolve_path(&self, input: &str) -> Option<String> {
        if is_all_workspace_scope_input(input) {
            return None;
        }
        let normalized = super::normalize_path(input).into_owned();
        let scopes = &self.paths;
        if scopes.iter().any(|scope| {
            normalized == scope.as_str() || normalized.starts_with(&format!("{scope}/"))
        }) {
            return Some(normalized);
        }
        let (head, tail) = normalized
            .split_once('/')
            .map_or((normalized.as_str(), ""), |(head, tail)| (head, tail));
        let mut matches = scopes
            .iter()
            .filter(|scope| scope.rsplit('/').next() == Some(head))
            .collect::<Vec<_>>();
        if self.is_ambiguous(input) {
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

    pub(crate) fn scope_for_input(&self, input: &str) -> Option<String> {
        if is_all_workspace_scope_input(input) {
            return None;
        }
        let normalized = self.resolve_path(input)?;
        self.paths
            .iter()
            .filter(|scope| {
                normalized == scope.as_str() || normalized.starts_with(&format!("{scope}/"))
            })
            .max_by_key(|scope| scope.len())
            .cloned()
    }

    /// Whether `input` resolves to a selectable workspace root itself, rather than a
    /// subdirectory inside one. Root statistics use this to attach the same block to
    /// both the repository root and each monorepo project root.
    pub(crate) fn is_workspace_root(&self, input: &str) -> bool {
        if is_all_workspace_scope_input(input) {
            return false;
        }
        self.resolve_path(input)
            .is_some_and(|normalized| self.paths.contains(&normalized))
    }

    /// Search keeps the resolved subdirectory; workspace identity remains separate.
    /// A file selects its containing directory, matching directory-scope semantics.
    pub(crate) fn search_scope_for_input(&self, input: &str) -> Option<String> {
        let normalized = self.resolve_path(input)?;
        let path = std::path::Path::new(&normalized);
        if path.is_file() {
            path.parent()
                .map(|parent| parent.to_string_lossy().into_owned())
        } else {
            Some(normalized)
        }
    }

    pub(crate) fn root_view(&self) -> Option<String> {
        (!self.scopes.is_empty()).then(|| self.to_string())
    }
}

pub fn is_ambiguous_workspace_scope_input(
    files: &[crate::parser::ExtractedFile],
    input: &str,
) -> bool {
    WorkspaceCatalog::new(files).is_ambiguous(input)
}

pub fn resolve_workspace_path_input(
    files: &[crate::parser::ExtractedFile],
    input: &str,
) -> Option<String> {
    WorkspaceCatalog::new(files).resolve_path(input)
}

pub fn workspace_scope_for_input(
    files: &[crate::parser::ExtractedFile],
    input: &str,
) -> Option<String> {
    WorkspaceCatalog::new(files).scope_for_input(input)
}

pub fn looks_like_monorepo_workspace() -> bool {
    !filesystem_workspace_scope_paths().is_empty()
}

impl std::fmt::Display for WorkspaceCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(formatter, "# Root Codemap Overview")?;
        writeln!(formatter)?;
        writeln!(formatter, "- **Total Files**: {}", self.total_files)?;
        writeln!(formatter, "- **Total Symbols**: {}", self.total_symbols)?;
        writeln!(formatter)?;
        writeln!(formatter, "## Workspace Scopes")?;
        for scope in &self.scopes {
            let languages = self
                .languages
                .get(&scope.path)
                .map(|languages| {
                    languages
                        .iter()
                        .map(|(language, count)| format!("{language} {count}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            writeln!(
                formatter,
                "- {} ({} files, {} symbols) — languages: {}",
                scope.path, scope.file_count, scope.symbol_count, languages
            )?;
        }
        writeln!(formatter)?;
        writeln!(formatter, "## Next Step")?;
        writeln!(
            formatter,
            "- For broad changes, ask which workspace scope to use before acting. For read-only location discovery with no chosen scope, use a repo-wide search first, then narrow to the implementation path. Do not infer a scope from its name alone; check its languages."
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
    WorkspaceCatalog::new(files).root_view()
}
