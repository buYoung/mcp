//! Filesystem watcher driving autonomous incremental index refresh. A recursive `notify`
//! watch of the repo root feeds a hand-written fixed-window debounce loop on a dedicated
//! OS thread; each closed window becomes one [`IndexCommand`] to the indexer thread —
//! path-scoped ([`IndexCommand::RefreshPaths`]) for ordinary edits, a full walk
//! ([`IndexCommand::Refresh`]) for bulk events (git HEAD change, event overflow/rescan,
//! oversized batches). While the watcher is healthy the server suppresses its
//! request-triggered fallback entirely (see [`super::EngineSupervisor::trigger_refresh`]),
//! which is what removes the per-request O(repo) walk; when the watch can't start (e.g. the
//! Linux inotify limit) or dies, [`WatcherStatus`] flips unhealthy and the fallback takes
//! over.
//!
//! Feedback-loop prevention: events under `.codemap` / `.codemap-index` / `.git` (except
//! the HEAD-hint files) / the configured excluded directories are dropped before they
//! reach the batch, so tantivy's own commits never re-trigger indexing. Access-kind
//! events (reads) are dropped too — the indexer reading source files must not look like
//! a change.
//!
//! The git layer is a hint only: events touching `.git/HEAD` / `packed-refs` /
//! `ORIG_HEAD` mark the batch, and at window close `git rev-parse HEAD` (the `git` CLI,
//! no `git2`) is compared against the remembered hash — a change forces the full-walk
//! pass, which is what makes branch switch / pull / rebase land exactly (including
//! deletions, via the full walk's set-difference detection). The hash never decides
//! *what* to reindex; the working-tree walk stays the source of truth, so uncommitted
//! edits are never missed. In a git-less workspace the initial `rev-parse` fails and the
//! layer disables itself. In a linked git worktree (`.git` is a file, the real HEAD
//! lives outside the watch root) the hint never fires — correctness is preserved because
//! a checkout's tree changes still arrive as ordinary file events, with the oversized
//! batch threshold escalating bulk switches to a full walk.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};

use super::IndexCommand;

/// Above this many distinct paths in one debounce window, a single full walk is cheaper
/// and safer than per-path passes (and bulk events of this size usually mean a branch
/// switch or generator ran, where full-walk delete detection is wanted anyway).
pub(super) const FULL_WALK_PATH_THRESHOLD: usize = 1024;

/// Git files watched as HEAD-change hints. Everything else under `.git` is dropped.
const GIT_REF_HINT_PATHS: &[&str] = &[".git/HEAD", ".git/packed-refs", ".git/ORIG_HEAD"];

/// Shared watcher health flag. The server reads it to gate the request-triggered
/// fallback: healthy → suppressed, unhealthy (watch off / failed / died) → active.
/// Created by the caller (default unhealthy) so the server can hold it before — and
/// independently of — a watcher actually starting.
#[derive(Default)]
pub struct WatcherStatus {
    healthy: AtomicBool,
}

impl WatcherStatus {
    /// True while the watcher thread is running and the notify watch is registered.
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
    }

    fn set_healthy(&self, value: bool) {
        self.healthy.store(value, Ordering::Release);
    }
}

enum WatcherMessage {
    Event(Result<notify::Event, notify::Error>),
    Stop,
}

/// Owner of the notify watcher and the debounce thread. Drop shuts both down: signaling
/// stop wakes the debounce loop immediately, which drops the `RecommendedWatcher` and its
/// `IndexCommand` sender clone before exiting, then the thread is joined. The `IndexerHandle`
/// must therefore drop AFTER this handle — guaranteed by [`super::EngineSupervisor`]'s field
/// declaration order (`watcher` before `indexer`).
pub struct WatcherHandle {
    stop_requested: Arc<AtomicBool>,
    stop_sender: Option<std::sync::mpsc::Sender<WatcherMessage>>,
    status: Arc<WatcherStatus>,
    join_handle: Option<JoinHandle<()>>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop_requested.store(true, Ordering::Release);
        self.status.set_healthy(false);
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(WatcherMessage::Stop);
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Start the debounce thread and register a recursive watch of `root` on that thread so
/// startup and activation never block on OS watch registration or `git rev-parse`. Flips
/// `status` healthy once the watch is registered; if registration fails (e.g. Linux inotify
/// watch limit), `status` stays unhealthy and the server keeps the request-triggered fallback.
pub fn spawn_watcher(
    root: &Path,
    command_sender: SyncSender<IndexCommand>,
    status: Arc<WatcherStatus>,
) -> Option<WatcherHandle> {
    // Canonicalize so event paths (which the OS reports against the resolved root) strip
    // cleanly in the path filter, e.g. macOS `/var` → `/private/var`.
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let debounce = Duration::from_millis(crate::config::DEFAULT_WATCH_DEBOUNCE_MS);

    let (msg_sender, msg_receiver) = channel::<WatcherMessage>();
    let stop_sender = msg_sender.clone();
    let stop_requested = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop_requested);
    let thread_status = Arc::clone(&status);
    let root_config = crate::config::thread_root_config();
    let join_handle = match std::thread::Builder::new()
        .name("codemap-watcher".to_string())
        .spawn(move || {
            let _root = root_config.as_ref().map(crate::config::RootConfig::bind);
            // Drop guard, not a trailing statement: the health flag must flip off on ANY
            // exit — normal shutdown, indexer gone, or a panic mid-loop — or the request
            // fallback would stay suppressed against a watcher that no longer refreshes.
            struct HealthOffGuard(Arc<WatcherStatus>);
            impl Drop for HealthOffGuard {
                fn drop(&mut self) {
                    self.0.set_healthy(false);
                }
            }
            let _guard = HealthOffGuard(Arc::clone(&thread_status));
            if thread_stop.load(Ordering::Acquire) {
                return;
            }
            let event_sender = msg_sender;
            let mut watcher = match notify::recommended_watcher(move |result| {
                let _ = event_sender.send(WatcherMessage::Event(result));
            }) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!(
                        "watcher creation failed: {e} — keeping request-triggered refresh"
                    );
                    return;
                }
            };
            if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
                tracing::warn!(
                    "watch registration failed: {e} — keeping request-triggered refresh"
                );
                return;
            }
            if thread_stop.load(Ordering::Acquire) {
                return;
            }
            // Initial HEAD hash; `None` (no git, no commits yet, no `git` binary) disables the
            // git hint layer while file events keep working.
            let initial_head = git_head_hash(&root);
            if thread_stop.load(Ordering::Acquire) {
                return;
            }
            thread_status.set_healthy(true);
            run_debounce_loop(msg_receiver, command_sender, root, debounce, initial_head);
            drop(watcher);
        }) {
        Ok(handle) => handle,
        Err(e) => {
            status.set_healthy(false);
            tracing::warn!("watcher thread spawn failed: {e} — keeping request-triggered refresh");
            return None;
        }
    };

    Some(WatcherHandle {
        stop_requested,
        stop_sender: Some(stop_sender),
        status,
        join_handle: Some(join_handle),
    })
}

/// One debounce window's accumulated work.
#[derive(Default)]
struct PendingBatch {
    /// Distinct event paths to refresh incrementally (BTreeSet: dedup + stable order).
    paths: BTreeSet<PathBuf>,
    /// Escalate to a full-walk pass (rescan/overflow, watch error, oversized batch,
    /// HEAD change — resolved at window close).
    needs_full_walk: bool,
    /// A git ref hint file was touched; compare HEAD at window close.
    git_ref_touched: bool,
}

/// The debounce loop: block for the first event, collect everything arriving within the
/// debounce window, then hand the indexer exactly one command. Exits when signaled to stop,
/// when the event channel disconnects, or when the indexer is gone.
fn run_debounce_loop(
    events: Receiver<WatcherMessage>,
    commands: SyncSender<IndexCommand>,
    root: PathBuf,
    debounce: Duration,
    mut last_head: Option<String>,
) {
    loop {
        let first = match events.recv() {
            Ok(WatcherMessage::Event(event)) => event,
            Ok(WatcherMessage::Stop) | Err(_) => return, // shutdown
        };
        let mut batch = PendingBatch::default();
        accumulate(&mut batch, first, &root);

        let deadline = Instant::now() + debounce;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match events.recv_timeout(remaining) {
                Ok(WatcherMessage::Event(event)) => accumulate(&mut batch, event, &root),
                Ok(WatcherMessage::Stop) | Err(RecvTimeoutError::Disconnected) => return, // shutdown — drop the batch
                Err(RecvTimeoutError::Timeout) => break,
            }
        }

        // Resolve the git hint only at window close, so a half-written tree mid-checkout
        // is not walked twice: the HEAD change and the tree churn share one window.
        if batch.git_ref_touched {
            if let Some(stored_head) = last_head.as_deref() {
                match git_head_hash(&root) {
                    Some(current) if current == stored_head => {}
                    Some(current) => {
                        batch.needs_full_walk = true;
                        last_head = Some(current);
                    }
                    // Transient rev-parse failure (e.g. mid-checkout): walk to be safe,
                    // keep the stored hash so the next hint re-compares.
                    None => batch.needs_full_walk = true,
                }
            }
        }

        // Ignore files decide what the walk indexes, so a change to one reconciles fully.
        // Other paths hidden by ignore rules are dropped like excluded directories, except
        // native inputs while macro expansion is on: an ignored generated header or
        // compilation database still changes how the indexed files expand.
        if !batch.needs_full_walk {
            if batch.paths.iter().any(|path| is_ignore_file(path)) {
                batch.needs_full_walk = true;
            } else {
                let is_expanding = crate::config::get().macro_expansion.is_enabled;
                let mut probe = crate::workspace::IgnoreProbe::default();
                batch.paths.retain(|path| {
                    (is_expanding && super::preprocess::is_native_input(path))
                        || !probe.is_ignored(path)
                });
            }
        }

        let command = if batch.needs_full_walk {
            IndexCommand::Refresh
        } else if !batch.paths.is_empty() {
            IndexCommand::RefreshPaths(batch.paths.into_iter().collect())
        } else {
            continue; // everything filtered out — nothing to do
        };

        // Blocking send, deliberately: a `try_send` drop would silently lose this batch's
        // paths (a full-walk command may coalesce, a path batch may not). While blocked,
        // events keep buffering in the unbounded notify channel above.
        if commands.send(command).is_err() {
            return; // indexer gone — the thread exit flips the health flag off
        }
    }
}

/// Fold one notify event into the batch: errors and rescans escalate to a full walk,
/// access-kind events are dropped, and each path is classified by the filter below.
fn accumulate(batch: &mut PendingBatch, event: Result<notify::Event, notify::Error>, root: &Path) {
    let event = match event {
        Ok(event) => event,
        Err(e) => {
            // Backend error (e.g. inotify queue overflow surfaces here on some
            // platforms): events may have been lost; a full walk recovers them all.
            tracing::warn!("watcher backend error: {e} — scheduling a full-walk refresh");
            batch.needs_full_walk = true;
            return;
        }
    };
    if event.need_rescan() {
        batch.needs_full_walk = true;
    }
    // Reads must not look like changes: the indexer itself reads source files (and walks
    // directories) during a pass, which surfaces as access events on some backends —
    // acting on them would loop watcher → index → events → watcher forever.
    if matches!(event.kind, EventKind::Access(_)) {
        return;
    }
    for path in &event.paths {
        match classify_event_path(path, root) {
            EventPathKind::Ignored => {}
            EventPathKind::GitRefHint => batch.git_ref_touched = true,
            EventPathKind::WatchRoot => batch.needs_full_walk = true,
            EventPathKind::Candidate => {
                batch.paths.insert(path.clone());
                if batch.paths.len() > FULL_WALK_PATH_THRESHOLD {
                    batch.needs_full_walk = true;
                }
            }
        }
    }
}

enum EventPathKind {
    /// Outside the root, under an excluded directory, or `.git` internals — dropped.
    Ignored,
    /// One of [`GIT_REF_HINT_PATHS`] — marks the batch for the HEAD comparison.
    GitRefHint,
    /// The watch root itself — semantically "anything may have changed", full walk.
    WatchRoot,
    /// A workspace path worth a path-scoped refresh.
    Candidate,
}

/// The watcher-side path filter, mirroring the index walk's exclusions
/// ([`crate::workspace::ALWAYS_EXCLUDED_DIRS`] + configured `excluded_directories` + linked
/// worktree roots) so tantivy's commits to `.codemap/index` and churn in
/// `node_modules`/`target`/… never re-trigger indexing. `.git` paths are dropped except the HEAD-hint files.
fn classify_event_path(path: &Path, root: &Path) -> EventPathKind {
    let rel = match path.strip_prefix(root) {
        Ok(rel) => rel,
        Err(_) => return EventPathKind::Ignored,
    };
    if rel.as_os_str().is_empty() {
        return EventPathKind::WatchRoot;
    }
    if GIT_REF_HINT_PATHS.iter().any(|hint| rel == Path::new(hint)) {
        return EventPathKind::GitRefHint;
    }
    let config = crate::config::get();
    // The tantivy index location is configurable: the default `.codemap/index` is caught
    // by the excluded-dir names below, but a custom `index_path` outside those names must
    // be dropped explicitly or every commit's own writes would schedule a wasteful
    // (though convergent — index files are never indexable) refresh pass.
    let index_path = Path::new(&config.index_path);
    if rel.starts_with(index_path) || path.starts_with(index_path) {
        return EventPathKind::Ignored;
    }
    // Match directory ancestors only: a file called `build` is not a directory rule.
    let mut directory = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };
    while let Some(parent) = directory {
        if parent == root {
            break;
        }
        if crate::workspace::directory_is_excluded(parent, root, &config, true)
            || crate::workspace::is_linked_worktree_root(parent)
        {
            return EventPathKind::Ignored;
        }
        directory = parent.parent();
    }
    EventPathKind::Candidate
}

fn is_ignore_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        name == ".gitignore"
            || name == ".ignore"
            || name == crate::workspace::CODEMAP_IGNORE_FILENAME
    })
}

/// Current HEAD commit hash via the `git` CLI, or `None` when unavailable (not a git
/// repo, no commits yet, or no `git` binary) — consistent with `build_walker`'s
/// `require_git(false)` posture: git is an optional hint, never a requirement.
fn git_head_hash(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() {
        None
    } else {
        Some(hash)
    }
}
