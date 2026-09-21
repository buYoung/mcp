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
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

use super::TantivySearchEngine;
use crate::parser::{CodeRange, ExtractedFile, StaticCollectionEdge};
use std::collections::{BTreeSet, HashMap, HashSet};

/// A result can expose at most this many producer and consumer candidates for one collection
/// key. The stable `(path, range)` record order makes truncation deterministic.
const STATIC_COLLECTION_RECORDS_PER_KEY_AND_KIND_MAX: usize = 64;
const INDEXER_RETRY_DELAY_MS: u64 = 1_000;

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

impl IndexCommand {
    fn merge(self, incoming: Self) -> Self {
        match (self, incoming) {
            (Self::RefreshPaths(mut paths), Self::RefreshPaths(incoming)) => {
                paths.extend(incoming);
                paths.sort();
                paths.dedup();
                if paths.len() > super::watcher::FULL_WALK_PATH_THRESHOLD {
                    Self::Refresh
                } else {
                    Self::RefreshPaths(paths)
                }
            }
            _ => Self::Refresh,
        }
    }
}

/// Shared, lock-light status the server reads to annotate responses.
#[derive(Default)]
pub struct IndexerStatus {
    /// Set once an initial generation is successfully published. Until then search/overview
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

/// One immutable generation of derived index data. The indexer replaces this whole value only
/// after a successful pass, keeping codemap, static collection records, and declaration evidence
/// in lock-step for every search.
#[derive(Debug, Clone)]
pub struct PublishedIndexSnapshot {
    codemap: CodemapSnapshot,
    event_index: crate::events::EventIndex,
    implementation_index: crate::implementations::ImplementationIndex,
    flow_index: crate::flow::FlowIndex,
    workspace_catalog: crate::codemap::WorkspaceCatalog,
    source_mtimes: HashMap<String, u64>,
    records: Vec<StaticCollectionRecord>,
    records_by_path: HashMap<String, Vec<usize>>,
    records_by_collection: HashMap<(String, String), Vec<usize>>,
    type_declaration_ranges_by_path: HashMap<(String, String), Vec<CodeRange>>,
    type_declaration_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct StaticCollectionRecord {
    pub file_path: String,
    pub edge: StaticCollectionEdge,
}

impl PublishedIndexSnapshot {
    pub(crate) fn empty() -> Self {
        Self::from_files_and_edges(Vec::new())
    }

    pub(crate) fn from_files_and_edges(
        files_and_edges: Vec<(ExtractedFile, Vec<StaticCollectionEdge>)>,
    ) -> Self {
        Self::from_files_and_edges_with_events(
            files_and_edges,
            crate::events::EventInputs::default(),
        )
    }

    pub(crate) fn from_files_and_edges_with_events(
        files_and_edges: Vec<(ExtractedFile, Vec<StaticCollectionEdge>)>,
        event_inputs: crate::events::EventInputs,
    ) -> Self {
        Self::from_files_and_edges_with_flow(files_and_edges, event_inputs, None, None)
    }

    pub(crate) fn from_files_and_edges_with_flow(
        files_and_edges: Vec<(ExtractedFile, Vec<StaticCollectionEdge>)>,
        event_inputs: crate::events::EventInputs,
        flow_store: Option<crate::flow::IndexedFlowStore>,
        cache_directory: Option<&std::path::Path>,
    ) -> Self {
        const STATIC_COLLECTION_EDGES_PER_FILE_MAX: usize = 256;
        let mut files = Vec::with_capacity(files_and_edges.len());
        let mut records = Vec::new();
        let mut records_by_path: HashMap<String, Vec<usize>> = HashMap::new();
        let mut records_by_collection: HashMap<(String, String), Vec<usize>> = HashMap::new();
        let mut type_declaration_ranges_by_path: HashMap<_, Vec<CodeRange>> = HashMap::new();
        let mut type_declaration_counts = HashMap::new();

        for (file, edges) in files_and_edges {
            let file_path = file.file_path.clone();
            for symbol in &file.symbols {
                if matches!(
                    symbol.kind.as_str(),
                    "class" | "struct" | "interface" | "record" | "object" | "enum" | "union"
                ) {
                    type_declaration_ranges_by_path
                        .entry((file_path.clone(), symbol.name.clone()))
                        .or_default()
                        .push(symbol.range.clone());
                    *type_declaration_counts
                        .entry(symbol.name.clone())
                        .or_insert(0) += 1;
                }
            }
            for edge in edges.into_iter().take(STATIC_COLLECTION_EDGES_PER_FILE_MAX) {
                let index = records.len();
                let collection = (
                    edge.collection_owner_type.clone(),
                    edge.collection_field.clone(),
                );
                records.push(StaticCollectionRecord {
                    file_path: file_path.clone(),
                    edge,
                });
                records_by_path
                    .entry(file_path.clone())
                    .or_default()
                    .push(index);
                records_by_collection
                    .entry(collection)
                    .or_default()
                    .push(index);
            }
            files.push(file);
        }
        files.sort_by(|left, right| left.file_path.cmp(&right.file_path));
        records.sort_by(|left, right| {
            left.file_path
                .cmp(&right.file_path)
                .then(left.edge.range.start_line.cmp(&right.edge.range.start_line))
                .then(left.edge.range.start_col.cmp(&right.edge.range.start_col))
        });
        // Record sorting changes offsets, so rebuild only the compact index vectors.
        records_by_path.clear();
        records_by_collection.clear();
        for (index, record) in records.iter().enumerate() {
            records_by_path
                .entry(record.file_path.clone())
                .or_default()
                .push(index);
            records_by_collection
                .entry((
                    record.edge.collection_owner_type.clone(),
                    record.edge.collection_field.clone(),
                ))
                .or_default()
                .push(index);
        }

        let catalog_started = std::time::Instant::now();
        let workspace_catalog = crate::codemap::WorkspaceCatalog::new(&files);
        tracing::debug!(
            elapsed_ms = catalog_started.elapsed().as_secs_f64() * 1000.0,
            file_count = files.len(),
            "published workspace catalog"
        );
        let event_index = match cache_directory {
            Some(directory) => {
                crate::events::EventIndex::build_with_cache(&files, event_inputs, Some(directory))
            }
            None => crate::events::EventIndex::build(&files, event_inputs),
        };
        let files = Arc::new(files);
        let implementation_index = crate::implementations::ImplementationIndex::build(
            Arc::clone(&files),
            event_index.indexed_sources(),
        );
        let flow_index = if let Some(store) = flow_store {
            crate::flow::FlowIndex::indexed(Arc::clone(&files), store, event_index.shared_sources())
        } else {
            crate::flow::FlowIndex::build(
                Arc::clone(&files),
                Default::default(),
                event_index.shared_sources(),
            )
        };
        Self {
            codemap: files,
            event_index,
            implementation_index,
            flow_index,
            workspace_catalog,
            source_mtimes: HashMap::new(),
            records,
            records_by_path,
            records_by_collection,
            type_declaration_ranges_by_path,
            type_declaration_counts,
        }
    }

    pub(crate) fn events(&self) -> &crate::events::EventIndex {
        &self.event_index
    }

    pub(crate) fn implementations(&self) -> &crate::implementations::ImplementationIndex {
        &self.implementation_index
    }
    pub(crate) fn flows(&self) -> &crate::flow::FlowIndex {
        &self.flow_index
    }

    pub fn codemap(&self) -> CodemapSnapshot {
        Arc::clone(&self.codemap)
    }

    pub(crate) fn workspace_catalog(&self) -> &crate::codemap::WorkspaceCatalog {
        &self.workspace_catalog
    }

    pub(crate) fn with_source_mtimes(mut self, source_mtimes: HashMap<String, u64>) -> Self {
        self.source_mtimes = source_mtimes;
        self
    }

    /// Existing nanosecond source stamps from the same committed documents as the codemap.
    pub(crate) fn source_mtime(&self, path: &str) -> Option<u64> {
        self.source_mtimes.get(path).copied()
    }

    pub fn records_for_result_paths<'a>(
        &'a self,
        result_paths: &HashSet<&str>,
    ) -> Vec<&'a StaticCollectionRecord> {
        let mut collections = BTreeSet::new();
        for path in result_paths {
            let Some(path_records) = self.records_by_path.get(*path) else {
                continue;
            };
            for index in path_records {
                let record = &self.records[*index];
                collections.insert((
                    record.edge.collection_owner_type.clone(),
                    record.edge.collection_field.clone(),
                ));
            }
        }
        let mut selected = Vec::new();
        for collection in collections {
            let Some(collection_records) = self.records_by_collection.get(&collection) else {
                continue;
            };
            let mut result_producers = 0;
            let mut result_consumers = 0;
            let mut counterpart_producers = 0;
            let mut counterpart_consumers = 0;
            for index in collection_records {
                let record = &self.records[*index];
                let is_result_path = result_paths.contains(record.file_path.as_str());
                let count = match (is_result_path, record.edge.kind) {
                    (true, crate::parser::StaticCollectionEdgeKind::Producer) => {
                        &mut result_producers
                    }
                    (true, crate::parser::StaticCollectionEdgeKind::Consumer) => {
                        &mut result_consumers
                    }
                    (false, crate::parser::StaticCollectionEdgeKind::Producer) => {
                        &mut counterpart_producers
                    }
                    (false, crate::parser::StaticCollectionEdgeKind::Consumer) => {
                        &mut counterpart_consumers
                    }
                };
                if *count < STATIC_COLLECTION_RECORDS_PER_KEY_AND_KIND_MAX {
                    selected.push(*index);
                    *count += 1;
                }
            }
        }
        selected.sort_unstable();
        selected
            .into_iter()
            .map(|index| &self.records[index])
            .collect()
    }

    pub fn unique_type_declaration_range_in_path(
        &self,
        path: &str,
        type_name: &str,
    ) -> Option<&CodeRange> {
        let ranges = self
            .type_declaration_ranges_by_path
            .get(&(path.to_string(), type_name.to_string()))?;
        (ranges.len() == 1).then(|| &ranges[0])
    }

    pub fn type_declaration_count(&self, type_name: &str) -> usize {
        self.type_declaration_counts
            .get(type_name)
            .copied()
            .unwrap_or_default()
    }
}

/// Server-side handle to the background indexer: a fire-and-forget refresh trigger, shared
/// status, the current codemap snapshot, and the thread join handle (joined on drop).
pub struct IndexerHandle {
    sender: Option<SyncSender<IndexCommand>>,
    pub status: Arc<IndexerStatus>,
    snapshot: Arc<Mutex<Arc<PublishedIndexSnapshot>>>,
    generation_gate: Arc<RwLock<()>>,
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
        self.snapshot.lock().unwrap().codemap()
    }

    /// The complete immutable index generation used by collection relation rendering.
    pub fn published_snapshot(&self) -> Arc<PublishedIndexSnapshot> {
        self.snapshot.lock().unwrap().clone()
    }

    pub fn search_with_context(
        &self,
        searcher: &super::SearcherHandle,
        query: &str,
        limit: usize,
        context: &super::SearchQueryContext,
    ) -> Result<(Vec<super::SearchResult>, Arc<PublishedIndexSnapshot>), String> {
        self.search_with_context_in_scope(searcher, query, limit, context, None)
    }

    pub(crate) fn search_with_context_in_scope(
        &self,
        searcher: &super::SearcherHandle,
        query: &str,
        limit: usize,
        context: &super::SearchQueryContext,
        workspace_scope: Option<&str>,
    ) -> Result<(Vec<super::SearchResult>, Arc<PublishedIndexSnapshot>), String> {
        let _generation_guard = self.generation_gate.read().unwrap();
        let results =
            searcher.search_with_context_in_scope(query, limit, context, workspace_scope)?;
        Ok((results, self.snapshot.lock().unwrap().clone()))
    }

    /// True until the initial background generation is successfully published.
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
    /// (guaranteed by [`super::EngineSupervisor`]'s field declaration order: `watcher`
    /// before `indexer`).
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
    let snapshot: Arc<Mutex<Arc<PublishedIndexSnapshot>>> =
        Arc::new(Mutex::new(Arc::new(PublishedIndexSnapshot::empty())));
    let generation_gate = Arc::new(RwLock::new(()));

    let thread_status = Arc::clone(&status);
    let thread_snapshot = Arc::clone(&snapshot);
    let thread_generation_gate = Arc::clone(&generation_gate);
    let join_handle = std::thread::Builder::new()
        .name("codemap-indexer".to_string())
        .spawn(move || {
            // Initial pass: hydrate from a warm on-disk index or build it from scratch.
            let is_initial_successful = run_refresh_pass(
                &mut engine,
                &thread_status,
                &thread_snapshot,
                &thread_generation_gate,
                true,
            );
            let mut pending = (!is_initial_successful).then_some(IndexCommand::Refresh);
            // Then serve refresh requests until the channel is closed (server shutdown —
            // the recv loop ends only once ALL senders, including the watcher's clone,
            // have dropped). Failed passes retry even while a healthy watcher suppresses
            // request-triggered refreshes. Keep failed paths until they are committed;
            // contention alone must not promote ordinary edits to a full walk.
            loop {
                let should_retry = pending.is_some();
                let command = if let Some(pending_command) = pending.take() {
                    let command = match receiver
                        .recv_timeout(std::time::Duration::from_millis(INDEXER_RETRY_DELAY_MS))
                    {
                        Ok(command) => pending_command.merge(command),
                        Err(RecvTimeoutError::Timeout) => pending_command,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    if engine.is_waiting_for_writer && !engine.is_writer_available() {
                        pending = Some(command);
                        continue;
                    }
                    command
                } else {
                    match receiver.recv() {
                        Ok(command) => command,
                        Err(_) => break,
                    }
                };
                let is_successful = match &command {
                    IndexCommand::Refresh => run_refresh_pass(
                        &mut engine,
                        &thread_status,
                        &thread_snapshot,
                        &thread_generation_gate,
                        should_retry,
                    ),
                    IndexCommand::RefreshPaths(paths) => run_paths_pass(
                        &mut engine,
                        &thread_status,
                        &thread_snapshot,
                        &thread_generation_gate,
                        paths,
                        should_retry,
                    ),
                };
                if !is_successful {
                    pending = Some(command);
                }
            }
        })
        .expect("failed to spawn codemap-indexer thread");

    IndexerHandle {
        sender: Some(sender),
        status,
        snapshot,
        generation_gate,
        join_handle: Some(join_handle),
    }
}

/// One refresh pass: incremental reindex, then republish the codemap snapshot when the
/// index changed (or on the initial pass, to hydrate from a warm on-disk index).
fn run_refresh_pass(
    engine: &mut TantivySearchEngine,
    status: &IndexerStatus,
    snapshot: &Mutex<Arc<PublishedIndexSnapshot>>,
    generation_gate: &RwLock<()>,
    should_force_publish: bool,
) -> bool {
    let _config_scope = crate::config::pin_request();
    let result = engine.index_files_changed_deferred(&["."]);
    publish_pass_result(
        engine,
        status,
        snapshot,
        generation_gate,
        result,
        should_force_publish,
    )
}

/// One path-scoped pass: incremental reindex/delete of just the watcher event paths,
/// then republish the codemap snapshot when the index changed.
fn run_paths_pass(
    engine: &mut TantivySearchEngine,
    status: &IndexerStatus,
    snapshot: &Mutex<Arc<PublishedIndexSnapshot>>,
    generation_gate: &RwLock<()>,
    paths: &[std::path::PathBuf],
    should_force_publish: bool,
) -> bool {
    let _config_scope = crate::config::pin_request();
    let result = engine.refresh_paths_deferred(paths);
    publish_pass_result(
        engine,
        status,
        snapshot,
        generation_gate,
        result,
        should_force_publish,
    )
}

/// Record a pass result on the shared status and republish the codemap snapshot when the
/// index changed (or on the initial pass, to hydrate from a warm on-disk index).
fn publish_pass_result(
    engine: &TantivySearchEngine,
    status: &IndexerStatus,
    snapshot: &Mutex<Arc<PublishedIndexSnapshot>>,
    generation_gate: &RwLock<()>,
    result: Result<bool, String>,
    should_force_publish: bool,
) -> bool {
    let publication = (|| -> Result<(), String> {
        if result? || should_force_publish {
            let published_snapshot = engine.load_published_snapshot()?;
            // Build the derived data before taking the short generation lock. Failed
            // reconstruction keeps both the request reader and codemap on the old view.
            let _generation_guard = generation_gate.write().unwrap();
            engine.reload_reader()?;
            *snapshot.lock().unwrap() = Arc::new(published_snapshot);
        }
        Ok(())
    })();
    let mut last_error = status.last_error.lock().unwrap();
    match publication {
        Ok(()) => {
            *last_error = None;
            status.initial_index_done.store(true, Ordering::Release);
            true
        }
        Err(error) => {
            if last_error.as_ref() != Some(&error) {
                if engine.is_waiting_for_writer {
                    tracing::debug!("background index refresh waiting for writer: {}", error);
                } else {
                    tracing::warn!("background index refresh failed: {}", error);
                }
            }
            *last_error = Some(error);
            false
        }
    }
}
