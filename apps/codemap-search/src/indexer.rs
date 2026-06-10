//! Background indexing thread for the MCP server. The stdio request loop never blocks on
//! indexing: it owns a read-only [`crate::index::SearcherHandle`] and a fire-and-forget
//! [`IndexerHandle`], while a dedicated OS thread owns the [`TantivySearchEngine`] (the
//! single tantivy writer) and refreshes the index on request. Commits become visible to
//! the server's reader clone via the reader's reload policy, so search/overview serve the
//! latest committed snapshot without any lock on the hot path.
//!
//! A plain `std::thread` + capacity-1 `std::sync::mpsc::sync_channel` is deliberate: the
//! server is a sequential stdio loop with nothing to `await` on, so all it needs is a
//! send-and-forget trigger. Moving the engine into the thread removes every lock on the
//! search path and satisfies tantivy's single-writer rule for free; no extra dependency
//! (e.g. crossbeam) earns its keep for one producer / one consumer at this message rate.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::index::TantivySearchEngine;
use crate::parser::ExtractedFile;

/// Message to the indexer thread.
pub enum IndexCommand {
    /// Full working-tree walk + mtime diff (set-difference delete detection included).
    /// Sent by the request fallback and by the watcher on bulk events (git HEAD change,
    /// event overflow, rescan).
    Refresh,
    /// Path-scoped incremental refresh: reindex the event paths that exist on disk,
    /// delete the ones that don't. Sent by the watcher for ordinary edits so the common
    /// case never pays the O(repo) walk.
    RefreshPaths(Vec<std::path::PathBuf>),
}

/// Shared, lock-light status the server reads to annotate responses.
#[derive(Default)]
pub struct IndexerStatus {
    /// Set once the initial background indexing pass finishes. Until then search/overview
    /// may return empty/partial results and say so.
    pub initial_index_done: AtomicBool,
    /// Last background refresh error, if any, so a failing refresh surfaces as a note
    /// instead of silently serving stale results.
    pub last_error: Mutex<Option<String>>,
    /// Set once the indexer thread is gone (panicked or exited): the receiver is dropped, so
    /// `trigger_refresh` observes `Disconnected`. Results then stay frozen at the last
    /// commit, so responses warn instead of implying freshness.
    pub thread_died: AtomicBool,
}

/// Immutable codemap snapshot published by the indexer and read by `overview`. Swapped as
/// a whole `Arc`, so readers never observe a partially-updated tree.
pub type CodemapSnapshot = Arc<Vec<ExtractedFile>>;

/// Server-side handle to the background indexer: a fire-and-forget refresh trigger, shared
/// status, the current codemap snapshot, and the thread join handle (joined on drop).
pub struct IndexerHandle {
    sender: Option<SyncSender<IndexCommand>>,
    pub status: Arc<IndexerStatus>,
    snapshot: Arc<Mutex<CodemapSnapshot>>,
    join_handle: Option<JoinHandle<()>>,
}

impl IndexerHandle {
    /// Enqueue a background refresh; never blocks. A queued request coalesces concurrent
    /// triggers (capacity-1 channel), and a gone indexer thread degrades to stale results.
    pub fn trigger_refresh(&self) {
        if let Some(sender) = &self.sender {
            match sender.try_send(IndexCommand::Refresh) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {} // a refresh is already queued — coalesced
                Err(TrySendError::Disconnected(_)) => {
                    self.status.thread_died.store(true, Ordering::Release);
                    tracing::warn!("indexer thread is gone; serving stale results");
                }
            }
        }
    }

    /// The current codemap snapshot (cheap `Arc` clone; the lock is held only for the clone).
    pub fn codemap_snapshot(&self) -> CodemapSnapshot {
        self.snapshot.lock().unwrap().clone()
    }

    /// True until the initial background indexing pass completes.
    pub fn is_warming(&self) -> bool {
        !self.status.initial_index_done.load(Ordering::Acquire)
    }

    /// True if the background indexer thread has stopped (panicked or exited). Results are
    /// then frozen at the last commit until the server restarts. Checks the join handle
    /// directly — not just the `thread_died` flag, which is only set when a send observes
    /// `Disconnected` — so death is visible even while a healthy watcher suppresses the
    /// request-triggered sends that would otherwise be the first to notice.
    pub fn is_dead(&self) -> bool {
        self.status.thread_died.load(Ordering::Acquire)
            || self
                .join_handle
                .as_ref()
                .is_some_and(|handle| handle.is_finished())
    }

    /// The last background refresh error, if any.
    pub fn last_error(&self) -> Option<String> {
        self.status.last_error.lock().unwrap().clone()
    }

    /// Clone of the command channel sender for the filesystem watcher. The watcher thread
    /// holds this clone, so it MUST be dropped (watcher shut down) before this handle is
    /// dropped — the indexer's `recv()` loop ends only when ALL senders are gone
    /// (guaranteed by `McpServer`'s field declaration order: `watcher` before `indexer`).
    pub fn command_sender(&self) -> SyncSender<IndexCommand> {
        self.sender
            .as_ref()
            .expect("command_sender called after IndexerHandle drop began")
            .clone()
    }
}

impl Drop for IndexerHandle {
    fn drop(&mut self) {
        // Close the channel so the recv loop exits, then wait for the thread to finish its
        // current pass. The client is already gone here, so the join is just clean shutdown;
        // tantivy commits are atomic, so even an abrupt exit would not corrupt the index.
        drop(self.sender.take());
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Spawn the background indexer, taking ownership of `engine` (the single tantivy writer).
/// The initial indexing pass runs immediately on the new thread; the server serves requests
/// against the current reader snapshot meanwhile.
pub fn spawn_indexer(mut engine: TantivySearchEngine) -> IndexerHandle {
    let (sender, receiver) = sync_channel::<IndexCommand>(1);
    let status = Arc::new(IndexerStatus::default());
    let snapshot: Arc<Mutex<CodemapSnapshot>> = Arc::new(Mutex::new(Arc::new(Vec::new())));

    let thread_status = Arc::clone(&status);
    let thread_snapshot = Arc::clone(&snapshot);
    let join_handle = std::thread::Builder::new()
        .name("codemap-indexer".to_string())
        .spawn(move || {
            // Initial pass: hydrate from a warm on-disk index or build it from scratch.
            run_refresh_pass(&mut engine, &thread_status, &thread_snapshot, true);
            thread_status
                .initial_index_done
                .store(true, Ordering::Release);
            // Then serve refresh requests until the channel is closed (server shutdown —
            // the recv loop ends only once ALL senders, including the watcher's clone,
            // have dropped).
            while let Ok(command) = receiver.recv() {
                match command {
                    IndexCommand::Refresh => {
                        run_refresh_pass(&mut engine, &thread_status, &thread_snapshot, false);
                    }
                    IndexCommand::RefreshPaths(paths) => {
                        run_paths_pass(&mut engine, &thread_status, &thread_snapshot, &paths);
                    }
                }
            }
        })
        .expect("failed to spawn codemap-indexer thread");

    IndexerHandle {
        sender: Some(sender),
        status,
        snapshot,
        join_handle: Some(join_handle),
    }
}

/// One refresh pass: incremental reindex, then republish the codemap snapshot when the
/// index changed (or on the initial pass, to hydrate from a warm on-disk index).
fn run_refresh_pass(
    engine: &mut TantivySearchEngine,
    status: &IndexerStatus,
    snapshot: &Mutex<CodemapSnapshot>,
    is_initial_pass: bool,
) {
    let result = engine.index_files_changed(&["."]);
    publish_pass_result(engine, status, snapshot, result, is_initial_pass);
}

/// One path-scoped pass: incremental reindex/delete of just the watcher event paths,
/// then republish the codemap snapshot when the index changed.
fn run_paths_pass(
    engine: &mut TantivySearchEngine,
    status: &IndexerStatus,
    snapshot: &Mutex<CodemapSnapshot>,
    paths: &[std::path::PathBuf],
) {
    let result = engine.refresh_paths(paths);
    publish_pass_result(engine, status, snapshot, result, false);
}

/// Record a pass result on the shared status and republish the codemap snapshot when the
/// index changed (or on the initial pass, to hydrate from a warm on-disk index).
fn publish_pass_result(
    engine: &TantivySearchEngine,
    status: &IndexerStatus,
    snapshot: &Mutex<CodemapSnapshot>,
    result: Result<bool, String>,
    force_publish: bool,
) {
    match result {
        Ok(changed) => {
            *status.last_error.lock().unwrap() = None;
            if changed || force_publish {
                let files = engine.load_extracted_files();
                *snapshot.lock().unwrap() = Arc::new(files);
            }
        }
        Err(e) => {
            tracing::warn!("background index refresh failed: {}", e);
            *status.last_error.lock().unwrap() = Some(e);
        }
    }
}
