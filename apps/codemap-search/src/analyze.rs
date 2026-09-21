//! Local index inspection and SQLite-backed MCP usage analytics.
mod cli;
mod index_report;
pub(crate) mod options;
mod reads_report;
mod report;
mod storage;
mod table;

use std::path::Path;

pub use cli::AnalyzeCommand;
pub(crate) use report::compact;
pub(crate) use storage::CallRecorder;

/// Internal measurement metadata, never included in the MCP response envelope.
pub struct FileObservation {
    pub path: String,
    /// Rendered per-file result block, before response-wide credential masking.
    pub result_bytes: u64,
}

/// No source parsing or index refresh. The only persistent write is expired-record removal.
pub(crate) fn collect(root: &Path, options: &options::Options) -> anyhow::Result<report::Report> {
    anyhow::ensure!(
        root.is_dir(),
        "Workspace is not a directory: {}",
        root.display()
    );
    let root = root.canonicalize()?;
    let now_ms = storage::now_ms();
    let database_path = root.join(storage::DATABASE_PATH);
    // Enforce retention even when only the index section is requested.
    let database = storage::open_existing(&database_path)?;
    if let Some(database) = &database {
        storage::purge_expired(database, now_ms)?;
    }
    match options.target {
        options::Target::Index => {
            let config = crate::config::get();
            index_report::collect(&root, &root.join(&config.index_path), options)
        }
        options::Target::Reads => reads_report::collect(database.as_ref(), now_ms, options),
    }
}
