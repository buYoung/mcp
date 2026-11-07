//! Start a new worktree's index from the index of another worktree of the same repository.
//!
//! Worktrees of one repository mostly hold the same files, and the refresh pass compares
//! content hashes, so a copy of a neighbor's committed index re-extracts only the files whose
//! content differs. The initial pass reconciles the copy with this tree before the indexer
//! publishes anything from it.

use std::path::{Path, PathBuf};

use tantivy::directory::{Directory, MmapDirectory, META_LOCK};
use tantivy::Index;

/// Index-directory files besides the segments and `meta.json` that a copy keeps: the format
/// stamp and the event settings stamp compared by refresh passes. Paths of indexed roots,
/// content mtimes, locks and the event-route cache, whose key includes the root, belong to one
/// tree. So does the macro-expansion settings stamp: recorded expansion inputs name the
/// neighbor's files, and without the stamp the first pass treats the settings as changed and
/// re-extracts every native file.
const COPIED_FILES: [&str; 2] = ["codemap.format", "event-navigation-config"];

/// Fill the absent index at `index` of the worktree at `root` from the most recently committed
/// current-format index of another worktree of the same repository. Returns that index.
pub(super) fn copy_into(index: &Path, root: &Path) -> Option<PathBuf> {
    let relative = index.strip_prefix(root).ok()?;
    let source = newest_neighbor_index(root, relative)?;
    let name = index.file_name()?.to_string_lossy().into_owned();
    let staging = index.with_file_name(format!(".{name}.copy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let copied = std::fs::create_dir_all(&staging)
        .map_err(|error| error.to_string())
        .and_then(|()| copy_committed(&source, &staging));
    // An empty index directory is replaced; any other content means another process started
    // this index meanwhile, and its index is kept.
    let placed =
        copied.and_then(|()| std::fs::rename(&staging, index).map_err(|error| error.to_string()));
    match placed {
        Ok(()) => Some(source),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            tracing::debug!("index not started from {}: {error}", source.display());
            None
        }
    }
}

/// Copy the segments that `source`'s `meta.json` lists, with that metadata, while holding the
/// lock under which readers open segments and garbage collection chooses files to delete, so
/// none of them is removed during the copy even while another process writes `source`.
fn copy_committed(source: &Path, target: &Path) -> Result<(), String> {
    let directory = MmapDirectory::open(source).map_err(|error| error.to_string())?;
    let _meta_lock = directory
        .acquire_lock(&META_LOCK)
        .map_err(|error| error.to_string())?;
    if !super::engine::has_current_format(source) {
        return Err("the index has another extraction format".into());
    }
    let metas = Index::open(directory.clone())
        .and_then(|index| index.load_metas())
        .map_err(|error| error.to_string())?;
    let copy = |name: &Path| {
        std::fs::copy(source.join(name), target.join(name)).map_err(|error| error.to_string())
    };
    let mut managed = vec!["meta.json".to_string()];
    for file in metas.list_segment_files() {
        if source.join(&file).exists() {
            copy(&file)?;
            managed.push(file.to_string_lossy().into_owned());
        }
    }
    managed.sort();
    // The same encodings tantivy writes, so it manages and later collects the copied files.
    let mut meta = serde_json::to_vec_pretty(&metas).map_err(|error| error.to_string())?;
    meta.push(b'\n');
    let mut listed = serde_json::to_vec(&managed).map_err(|error| error.to_string())?;
    listed.push(b'\n');
    std::fs::write(target.join("meta.json"), meta)
        .and_then(|()| std::fs::write(target.join(".managed.json"), listed))
        .map_err(|error| error.to_string())?;
    for name in COPIED_FILES {
        if source.join(name).exists() {
            copy(Path::new(name))?;
        }
    }
    Ok(())
}

/// The current-format index at `relative` in another worktree of `root`'s repository whose
/// metadata was committed last.
fn newest_neighbor_index(root: &Path, relative: &Path) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    worktree_roots(&git_common_dir(&root)?)
        .into_iter()
        .filter(|worktree| worktree.canonicalize().ok().as_ref() != Some(&root))
        .map(|worktree| worktree.join(relative))
        .filter(|index| super::engine::has_current_format(index))
        .filter_map(|index| {
            let committed = std::fs::metadata(index.join("meta.json"))
                .and_then(|metadata| metadata.modified())
                .ok()?;
            Some((committed, index))
        })
        .max_by_key(|(committed, _)| *committed)
        .map(|(_, index)| index)
}

/// The git directory shared by all worktrees of the repository checked out at `root`: its
/// `.git` directory, or the directory a linked worktree's `.git` file names through
/// `commondir`.
fn git_common_dir(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir = root.join(text.lines().next()?.strip_prefix("gitdir:")?.trim());
    let common = std::fs::read_to_string(gitdir.join("commondir")).ok()?;
    gitdir.join(common.trim()).canonicalize().ok()
}

/// The checked-out worktrees of the repository whose common git directory is `common`: the
/// main worktree when the repository is not bare, and every linked worktree it records.
fn worktree_roots(common: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if common.file_name().is_some_and(|name| name == ".git") {
        roots.extend(common.parent().map(Path::to_path_buf));
    }
    let Ok(entries) = std::fs::read_dir(common.join("worktrees")) else {
        return roots;
    };
    for entry in entries.flatten() {
        let Ok(gitdir) = std::fs::read_to_string(entry.path().join("gitdir")) else {
            continue;
        };
        // `gitdir` names the worktree's `.git` file.
        roots.extend(Path::new(gitdir.trim()).parent().map(Path::to_path_buf));
    }
    roots
}
