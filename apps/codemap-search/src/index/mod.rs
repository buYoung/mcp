//! `index` — the coupled index subsystem: Tantivy schema + write path ([`engine`]), BM25
//! search + ranking heuristics ([`ranking`]), the background indexer thread ([`indexer`]),
//! the filesystem watcher ([`watcher`]), and the engine lifecycle supervisor ([`supervisor`])
//! that owns the searcher/indexer/watcher handles and drives auto-restart + refresh fallback.
//!
//! Submodule items are re-exported here as their single canonical path (`crate::index::X`);
//! the watcher/indexer ranking/engine internals are not addressed by their submodule paths.

mod engine;
mod indexer;
mod ranking;
mod supervisor;
mod watcher;

pub use engine::{
    SearchEngine, SearchQueryContext, SearchRankingSignal, SearchResult, SearcherHandle,
    TantivySearchEngine,
};
pub use indexer::{spawn_indexer, CodemapSnapshot, IndexCommand, IndexerHandle, IndexerStatus};
pub use supervisor::EngineSupervisor;
pub use watcher::{spawn_watcher, WatcherHandle, WatcherStatus};
