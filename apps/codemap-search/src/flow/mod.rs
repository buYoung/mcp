//! Persisted source summaries retain index compatibility and freshness digests.
//! Value relationship evaluation/rendering is not part of navigation requests.
#[cfg(test)]
mod evaluate;
mod extract;
mod index;
mod model;
#[cfg(test)]
mod outcome;
#[cfg(test)]
mod render;
#[cfg(test)]
mod tests;

pub(crate) use extract::collect;
pub(crate) use index::FlowIndex;
pub(crate) use index::IndexedFlowStore;
pub(crate) use model::*;

/// Internal evaluator tests share accounting between isolated function summaries.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RequestBudget {
    deadline: Option<std::time::Instant>,
    operations: usize,
    source_bytes: usize,
    values: usize,
    steps: usize,
    files: std::collections::HashMap<String, bool>,
    shown: std::collections::BTreeMap<String, String>,
    shown_diagnostics: std::collections::BTreeSet<String>,
    work_limit: Option<&'static str>,
    has_omissions: bool,
}

pub(crate) const NODES_PER_FILE: usize = 4096;
pub(crate) const FUNCTIONS_PER_FILE: usize = 256;
#[cfg(test)]
pub(crate) const NODES_PER_QUERY: usize = 4096;
#[cfg(test)]
pub(crate) const FILES_PER_QUERY: usize = 24;
#[cfg(test)]
pub(crate) const CALL_DEPTH: usize = 8;
// Unit suites run concurrent, unoptimized parsers. Check the deadline explicitly
// in the budget test rather than making semantic assertions depend on CPU load.
#[cfg(test)]
pub(crate) const QUERY_TIME_MS: u64 = 5_000;

pub(crate) fn supports(language: &str) -> bool {
    matches!(
        language,
        "typescript"
            | "javascript"
            | "rust"
            | "go"
            | "python"
            | "java"
            | "csharp"
            | "kotlin"
            | "swift"
            | "dart"
            | "scala"
            | "groovy"
            | "php"
            | "ruby"
            | "powershell"
            | "c"
            | "cpp"
            | "lua"
            | "bash"
            | "zsh"
    )
}
