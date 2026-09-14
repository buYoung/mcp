//! Bounded source value relationships. Passing a function is not evidence of execution.
mod evaluate;
mod extract;
mod index;
mod model;
mod render;
#[cfg(test)]
mod tests;

pub(crate) use extract::collect;
pub(crate) use index::FlowIndex;
pub(crate) use index::IndexedFlowStore;
pub(crate) use model::*;

/// File-grouped live output shares the original query work limits. Each file has
/// its own value identities, while validated inputs and consumed work are reused.
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
}

impl RequestBudget {
    fn has_work(&mut self) -> bool {
        if self.work_limit.is_none() {
            self.work_limit = if self
                .deadline
                .is_some_and(|deadline| std::time::Instant::now() > deadline)
            {
                Some("value-flow time budget reached")
            } else if self.operations > NODES_PER_QUERY {
                Some("value-flow node budget reached")
            } else if self.values + 1 >= NODES_PER_QUERY {
                Some("value-flow value budget reached")
            } else if self.steps >= NODES_PER_QUERY {
                Some("value-flow relationship budget reached")
            } else {
                None
            };
        }
        self.work_limit.is_none()
    }

    pub(crate) fn notice(&self) -> String {
        self.work_limit.map_or_else(String::new, |reason| format!(
            "[Partial value analysis: {reason}. This limit is shared across returned files; additional value relationships may be missing.]\n"
        ))
    }
}

pub(crate) const NODES_PER_FILE: usize = 4096;
pub(crate) const FUNCTIONS_PER_FILE: usize = 256;
pub(crate) const NODES_PER_QUERY: usize = 4096;
pub(crate) const FILES_PER_QUERY: usize = 24;
pub(crate) const CALL_DEPTH: usize = 8;
#[cfg(not(test))]
pub(crate) const QUERY_TIME_MS: u64 = 100;
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
