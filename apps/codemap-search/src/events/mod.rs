//! Indexed static event routes, independent of direct-call and collection hints.
mod inputs;
pub(crate) use inputs::EventInputs;
mod extract;
mod index;
mod javascript;
mod rules;
pub(crate) use index::EventIndex;
mod model;
pub use model::*;

pub const SOURCE_BYTES_PER_FILE: usize = 512 * 1024;
pub const SOURCE_BYTES_PER_SNAPSHOT: usize = 64 * 1024 * 1024;
pub const SOURCE_FILES_PER_SNAPSHOT: usize = 4096;
pub const ENDPOINTS_PER_FILE: usize = 256;
pub const ENDPOINTS_PER_QUERY: usize = 128;
pub const ENDPOINTS_PER_SNAPSHOT: usize = 8192;
pub const CANDIDATES_PER_QUERY: usize = 512;
pub const SOURCE_FILES_PER_QUERY: usize = 128;
pub const SOURCE_BYTES_PER_QUERY: usize = 4 * 1024 * 1024;

pub(crate) fn eligible(path: &str) -> bool {
    let path = std::path::Path::new(path);
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts")
    ) || matches!(
        path.file_name().and_then(|s| s.to_str()),
        Some("Cargo.toml" | "package.json" | "tauri.conf.json")
    )
}

pub(crate) fn config_stamp() -> String {
    let cfg = crate::config::get();
    format!(
        "{:?}|{:?}|{:?}|{}",
        cfg.event_navigation, cfg.analysis_target_os, cfg.excluded_directories, cfg.use_git_exclude
    )
}
