//! `index` — the coupled index subsystem: Tantivy schema + write path ([`engine`]), BM25
//! search + ranking heuristics ([`ranking`]), recall of query phrases stated in one indexed
//! docstring or literal outside the ranked results ([`phrase_recall`]), the background indexer
//! thread ([`indexer`]), the filesystem watcher ([`watcher`]), and the engine lifecycle
//! supervisor ([`supervisor`]) that owns the searcher/indexer/watcher handles and drives
//! auto-restart + refresh fallback.
//!
//! Submodule items are re-exported here as their single canonical path (`crate::index::X`);
//! the watcher/indexer ranking/engine internals are not addressed by their submodule paths.

pub mod admission;
mod engine;
mod indexer;
mod path_role;
mod phrase_recall;
pub(crate) use phrase_recall::{PhraseAnchor, PhraseMatch};
mod preprocess;
pub use preprocess::MacroExpander;
mod ranking;
mod segment_files;
mod supervisor;
mod watcher;

pub use engine::{
    SearchEngine, SearchQueryContext, SearchRankingSignal, SearchResult, SearcherHandle,
    TantivySearchEngine,
};
pub use indexer::{
    spawn_dormant_indexer, spawn_indexer, CodemapSnapshot, IndexCommand, IndexerHandle,
    IndexerStatus, PublishedIndexSnapshot, StaticCollectionRecord,
};
pub(crate) use indexer::{SearchWithPhrases, SnapshotSource};
pub use supervisor::EngineSupervisor;
pub use watcher::{spawn_watcher, WatcherHandle, WatcherStatus};
