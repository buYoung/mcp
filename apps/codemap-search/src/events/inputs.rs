use super::EventInput;
use std::collections::{BTreeMap, HashMap};

/// Deterministic, bounded source population for one immutable index generation.
#[derive(Default)]
pub(crate) struct EventInputs {
    sources: BTreeMap<String, String>,
    bytes: usize,
    pub considered: usize,
    pub unavailable: usize,
}
impl EventInputs {
    pub fn insert(&mut self, path: String, input: Option<EventInput>) {
        if !super::eligible(&path) {
            return;
        }
        self.considered += 1;
        let Some(source) = input
            .and_then(|i| i.source)
            .filter(|s| s.len() <= super::SOURCE_BYTES_PER_FILE)
        else {
            self.unavailable += 1;
            return;
        };
        self.bytes += source.len();
        self.sources.insert(path, source);
        while self.sources.len() > super::SOURCE_FILES_PER_SNAPSHOT
            || self.bytes > super::SOURCE_BYTES_PER_SNAPSHOT
        {
            if let Some((_, source)) = self.sources.pop_last() {
                self.bytes -= source.len();
                self.unavailable += 1;
            }
        }
    }
    pub fn into_sources(self) -> HashMap<String, String> {
        self.sources.into_iter().collect()
    }
}
