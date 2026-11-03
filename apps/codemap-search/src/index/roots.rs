//! The index of a workspace root other than the start folder, for requests that name a path
//! in it. Only `search` may create that index; `read` and `grep` use it when it exists. The
//! root's own settings files configure it, while the session's filesystem permissions decide
//! which roots can be used. One such root is open at a time.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::supervisor::EngineSupervisor;
use crate::config::RootConfig;

/// An open root that no request used for this long is closed with its watchers.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The root that owns `path` when it is not the start folder `start`, both canonical: inside
/// the start folder, the outermost linked worktree holding `path`; outside it, the nearest
/// repository holding `path` below the home directory.
pub(crate) fn owning_root(path: &Path, start: &Path) -> Option<PathBuf> {
    if let Ok(relative) = path.strip_prefix(start) {
        let mut directory = start.to_path_buf();
        for component in relative.components() {
            directory.push(component);
            if crate::workspace::is_linked_worktree_root(&directory) {
                return Some(directory);
            }
        }
        return None;
    }
    let home = crate::workspace::user_home_dir()
        .map(|home| crate::workspace::canonicalize_path_lenient(&home));
    path.ancestors()
        .take_while(|directory| home.as_deref() != Some(*directory))
        .find(|directory| super::admission::is_repository_root(directory))
        .map(Path::to_path_buf)
}

/// An open root: its settings and its index.
pub(crate) struct OpenRoot {
    pub(crate) config: RootConfig,
    pub(crate) engine: EngineSupervisor,
    last_used: Instant,
}

/// A root whose index-backed output reports `message`.
pub(crate) struct UnavailableRoot {
    pub(crate) config: RootConfig,
    pub(crate) message: String,
}

#[derive(Default)]
pub(crate) struct OtherRoots {
    open: Option<OpenRoot>,
}

impl OtherRoots {
    /// The index of `root` (canonical), opened unless it is the open one; `None` when it has no
    /// index and `should_create` is false. Opening another root closes the open one. Call it
    /// under the session's settings.
    pub(crate) fn open(
        &mut self,
        root: &Path,
        should_create: bool,
    ) -> Result<Option<&mut OpenRoot>, UnavailableRoot> {
        self.close_unused();
        if self
            .open
            .as_ref()
            .is_some_and(|open| open.config.root() != root)
        {
            self.open = None;
        }
        if self.open.is_none() {
            let config =
                RootConfig::load(root, crate::config::get().filesystem_permissions.clone());
            match open_engine(&config, should_create) {
                Ok(Some(engine)) => {
                    self.open = Some(OpenRoot {
                        config,
                        engine,
                        last_used: Instant::now(),
                    });
                }
                Ok(None) => return Ok(None),
                Err(message) => return Err(UnavailableRoot { config, message }),
            }
        }
        let open = self.open.as_mut().expect("the root was opened above");
        open.last_used = Instant::now();
        Ok(Some(open))
    }

    /// Closes the open root once no request used it for [`IDLE_TIMEOUT`] or its directory is
    /// gone.
    pub(crate) fn close_unused(&mut self) {
        if self.open.as_ref().is_some_and(|open| {
            open.last_used.elapsed() >= IDLE_TIMEOUT || !open.config.root().is_dir()
        }) {
            self.open = None;
        }
    }
}

/// The root's index under its own settings, started like a server started there starts it.
/// An index another format wrote stays unchanged and is reported unavailable.
fn open_engine(
    config: &RootConfig,
    should_create: bool,
) -> Result<Option<EngineSupervisor>, String> {
    let _root = config.bind();
    let _config = crate::config::pin(config.current());
    if let super::admission::Admission::Unavailable(message) =
        super::admission::decide(config.root())
    {
        return Err(message);
    }
    let index_path = crate::config::get().index_path.clone();
    let index = Path::new(&index_path);
    if index.join("meta.json").is_file() {
        if !super::engine::has_current_format(index) {
            return Err(format!(
                "Index unavailable: the index of {} was written in another format by a different codemap-search version and is left unchanged. Use read/grep/find for live results, or start codemap-search in that folder to rebuild it.",
                config.root().display()
            ));
        }
    } else if !should_create {
        return Ok(None);
    }
    let engine = super::TantivySearchEngine::new(&index_path)?;
    let mut supervisor = EngineSupervisor::deferred(engine);
    // A refused size check leaves the supervisor reporting its message.
    let _ = supervisor.activate();
    Ok(Some(supervisor))
}
