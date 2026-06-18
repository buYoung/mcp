//! Engine supervision: owns the read-side searcher handle plus the background indexer and
//! filesystem watcher, and keeps them alive across the server's lifetime. Auto-restart of a
//! dead indexer and the request-triggered refresh fallback live here (not in the MCP server)
//! because they manipulate index-subsystem types and encode the subsystem's drop contract.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{spawn_indexer, spawn_watcher, CodemapSnapshot, TantivySearchEngine};
use super::{IndexerHandle, SearchResult, SearcherHandle, WatcherHandle, WatcherStatus};

/// Cap on automatic indexer restarts per server process: enough to absorb sporadic
/// failures, finite so a deterministically-crashing pass (e.g. a parser bug tripped by
/// one specific file on every walk) cannot respawn-loop forever. Past the cap the
/// existing "results are frozen" notice stands and a server restart is required.
const MAX_INDEXER_RESTART_ATTEMPTS: u32 = 5;

/// Owns the live index subsystem for the MCP server: a read-only [`SearcherHandle`], the
/// background [`IndexerHandle`], and the optional filesystem [`WatcherHandle`], plus the
/// supervision state (restart-attempt counter, refresh-debounce instant, shared watcher
/// health flag).
pub struct EngineSupervisor {
    // Read-only search handle over the committed index (cloned Arc-backed reader). Indexing
    // happens off-thread, so the request loop never blocks on it.
    searcher: SearcherHandle,
    // Filesystem watcher (None when `watch = false` or the watch failed to start). Field
    // order is load-bearing: struct fields drop in declaration order, and the watcher
    // thread holds a sender clone into the indexer channel — `watcher` MUST be declared
    // (and thus dropped) BEFORE `indexer`, whose drop joins a recv loop that only ends
    // once ALL senders are gone. The opposite order deadlocks on shutdown.
    watcher: Option<WatcherHandle>,
    // Background indexer: fire-and-forget refresh trigger, warming/error status, and the
    // current codemap snapshot consumed by `overview`.
    indexer: IndexerHandle,
    // Watcher health flag: while healthy, the watcher keeps the index current and the
    // request-triggered refresh below is suppressed entirely. Stays unhealthy forever
    // when `watch = false` or the watch failed to start, which preserves the lazy path.
    watcher_status: Arc<WatcherStatus>,
    // Instant of the last refresh trigger. Within `config::index_staleness_ms` we skip
    // re-triggering so a burst of search/overview calls enqueues at most one refresh; the
    // indexer's own mtime diff keeps each pass incremental. A single field now suffices
    // because search and overview share one background refresh.
    last_refresh_trigger: Option<Instant>,
    // Automatic indexer restarts performed so far (see `ensure_alive`).
    indexer_restart_attempts: u32,
}

impl EngineSupervisor {
    pub fn new(
        searcher: SearcherHandle,
        watcher: Option<WatcherHandle>,
        indexer: IndexerHandle,
        watcher_status: Arc<WatcherStatus>,
    ) -> Self {
        Self {
            searcher,
            watcher,
            indexer,
            watcher_status,
            last_refresh_trigger: None,
            indexer_restart_attempts: 0,
        }
    }

    /// Auto-recover a dead indexer thread (config `indexer_auto_restart`, default true):
    /// rebuild the engine, respawn the indexer, and re-attach the watcher, so one panic
    /// does not freeze results for the rest of the session. Runs at most once per
    /// request, only when death is actually observed, and never past
    /// [`MAX_INDEXER_RESTART_ATTEMPTS`]. `TantivySearchEngine::new` rebuilds a corrupt
    /// index directory, so a crash caused by index corruption heals instead of recurring.
    pub fn ensure_alive(&mut self) {
        if !crate::config::get().indexer_auto_restart || !self.indexer.is_dead() {
            return;
        }
        if self.indexer_restart_attempts >= MAX_INDEXER_RESTART_ATTEMPTS {
            return; // exhausted — the dead-indexer notice keeps reporting frozen results
        }
        self.indexer_restart_attempts += 1;
        tracing::warn!(
            "indexer thread died — auto-restarting ({}/{})",
            self.indexer_restart_attempts,
            MAX_INDEXER_RESTART_ATTEMPTS
        );

        // Tear the watcher down FIRST: its thread holds a sender clone into the dead
        // channel, and its drop flips the shared health flag off — done before the
        // respawn below so it cannot clobber the new watcher's healthy=true.
        self.watcher = None;

        let engine = match TantivySearchEngine::new(&crate::config::get().index_path) {
            Ok(engine) => engine,
            Err(e) => {
                tracing::warn!("indexer auto-restart failed to reopen the index: {e}");
                return; // next request retries (attempt already counted)
            }
        };
        self.searcher = engine.searcher_handle();
        // Old IndexerHandle drops on assignment: its thread is already dead, so the
        // join returns immediately — no shutdown-order hazard here.
        self.indexer = spawn_indexer(engine);

        if crate::config::get().watch {
            if let Ok(cwd) = std::env::current_dir() {
                self.watcher = spawn_watcher(
                    &cwd,
                    self.indexer.command_sender(),
                    Arc::clone(&self.watcher_status),
                );
            }
        }
    }

    /// Enqueue a background index refresh unless one was already triggered within the
    /// staleness window. Fire-and-forget — never blocks the request on indexing. Shared by
    /// search and overview, which both serve the indexer's published snapshot.
    ///
    /// While the filesystem watcher is healthy this is a no-op: the watcher already keeps
    /// the index current, so suppressing the request-triggered full walk here is what
    /// actually removes the per-request O(repo) walk during active use. The fallback below
    /// runs only when the watcher is absent (`watch = false`), failed to start, or died.
    pub fn trigger_refresh(&mut self) {
        // A dead indexer disarms the suppression: falling through lets `trigger_refresh`
        // observe the disconnected channel and raise the "results are frozen" notice,
        // which a healthy-looking watcher would otherwise mask indefinitely.
        if self.watcher_status.is_healthy() && !self.indexer.is_dead() {
            return;
        }
        let staleness = Duration::from_millis(crate::config::get().index_staleness_ms);
        let is_fresh = self
            .last_refresh_trigger
            .is_some_and(|t| t.elapsed() < staleness);
        if !is_fresh {
            self.indexer.trigger_refresh();
            self.last_refresh_trigger = Some(Instant::now());
        }
    }

    /// BM25 search over the current committed index snapshot (read-only).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        self.searcher.search(query, limit)
    }

    /// The current codemap snapshot the indexer publishes (cheap `Arc` clone).
    pub fn codemap_snapshot(&self) -> CodemapSnapshot {
        self.indexer.codemap_snapshot()
    }

    /// True if the background indexer thread has stopped (panicked or exited).
    pub fn is_dead(&self) -> bool {
        self.indexer.is_dead()
    }

    /// True until the initial background indexing pass completes.
    pub fn is_warming(&self) -> bool {
        self.indexer.is_warming()
    }

    /// The last background refresh error, if any.
    pub fn last_error(&self) -> Option<String> {
        self.indexer.last_error()
    }
}
