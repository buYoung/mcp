//! Whether and when the MCP server indexes its workspace (`[index].mode`). In `auto` mode a
//! folder outside any repository (for example a parent of several repositories) is never
//! walked, and a repository is indexed only once an index-backed tool is used and its
//! indexable file count stays within `[index].auto_max_files`.

use std::path::{Path, PathBuf};

use crate::config::IndexMode;

/// Markers of a repository root.
const REPOSITORY_MARKERS: [&str; 3] = [".git", ".hg", ".jj"];
/// Repositories named in a refusal message; the count covers the rest.
const NAMED_REPOSITORIES_MAX: usize = 3;
/// Directory entries the nested-repository scan inspects at most.
const NESTED_SCAN_ENTRIES_MAX: usize = 2_000;

pub enum Admission {
    /// Index at server start.
    Eager,
    /// Index on the first index-backed tool call, after the size check.
    Deferred,
    /// Do not index in this server; the message says why and how to opt in.
    Unavailable(String),
}

/// The startup decision. Cheap: it reads repository markers, never walks the workspace.
pub fn decide(cwd: &Path) -> Admission {
    match crate::config::get().index_mode {
        IndexMode::On => Admission::Eager,
        IndexMode::Off => Admission::Unavailable(
            "Index unavailable: indexing is off ([index] mode = \"off\"). Use read/grep/find for live results.".into(),
        ),
        IndexMode::Auto => {
            let cwd = crate::workspace::path_resolved_from_cwd(cwd);
            if is_inside_repository(&cwd) {
                Admission::Deferred
            } else {
                Admission::Unavailable(outside_repository_message(&cwd))
            }
        }
    }
}

/// The deferred size check: `Some(message)` when more indexable files than
/// `[index].auto_max_files` exist. Counting stops right after the limit.
pub(super) fn size_refusal(cwd: &Path) -> Option<String> {
    let limit = crate::config::get().index_auto_max_files;
    let count = crate::workspace::build_walker(cwd, false)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_some_and(|kind| kind.is_file())
                && crate::workspace::is_supported_source_path(entry.path())
        })
        .take(limit.saturating_add(1))
        .count();
    (count > limit).then(|| {
        format!(
            "Index unavailable: {} has more than {limit} indexable files ([index] auto_max_files). Use read/grep/find for live results, or raise auto_max_files or set [index] mode = \"on\" and restart the server to index it.",
            crate::workspace::path_resolved_from_cwd(cwd).display()
        )
    })
}

fn is_repository_root(directory: &Path) -> bool {
    REPOSITORY_MARKERS
        .iter()
        .any(|marker| directory.join(marker).exists())
}

/// A repository at the home directory itself (for example tracked dotfiles) does not make
/// every folder below home a repository.
fn is_inside_repository(cwd: &Path) -> bool {
    let home = crate::workspace::user_home_dir()
        .map(|home| crate::workspace::path_resolved_from_cwd(&home));
    cwd.ancestors()
        .take_while(|directory| home.as_deref() != Some(*directory))
        .any(is_repository_root)
}

fn outside_repository_message(cwd: &Path) -> String {
    let repositories = nested_repositories(cwd);
    let holds = match repositories.len() {
        0 => String::new(),
        count => {
            let mut names: Vec<_> = repositories
                .iter()
                .take(NAMED_REPOSITORIES_MAX)
                .map(|path| path.display().to_string())
                .collect();
            if count > NAMED_REPOSITORIES_MAX {
                names.push("…".into());
            }
            format!(
                " and holds {count} {} ({})",
                if count == 1 {
                    "repository"
                } else {
                    "repositories"
                },
                names.join(", ")
            )
        }
    };
    format!(
        "Index unavailable: {} is not inside a repository{holds}. Use read/grep/find for live results, start the server inside a repository, or set [index] mode = \"on\" and restart the server to index this folder.",
        cwd.display()
    )
}

/// Repository roots among the visible child and grandchild directories, relative to `cwd`.
fn nested_repositories(cwd: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut inspected = 0;
    for child in visible_directories(cwd, &mut inspected) {
        if is_repository_root(&child) {
            found.push(child);
            continue;
        }
        for grandchild in visible_directories(&child, &mut inspected) {
            if is_repository_root(&grandchild) {
                found.push(grandchild);
            }
        }
    }
    found
        .into_iter()
        .filter_map(|path| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .collect()
}

fn visible_directories(directory: &Path, inspected: &mut usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut directories: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .take(NESTED_SCAN_ENTRIES_MAX.saturating_sub(*inspected))
        .inspect(|_| *inspected += 1)
        .filter(|entry| {
            !entry.file_name().to_string_lossy().starts_with('.')
                && entry.file_type().is_ok_and(|kind| kind.is_dir())
        })
        .map(|entry| entry.path())
        .collect();
    directories.sort();
    directories
}
