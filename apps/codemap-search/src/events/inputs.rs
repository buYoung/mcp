use super::EventInput;
use std::collections::{BTreeMap, HashMap};

/// Deterministic, bounded source population for one immutable index generation.
#[derive(Default)]
pub(crate) struct EventInputs {
    sources: BTreeMap<String, String>,
    bytes: usize,
    pub considered: usize,
    pub unavailable: usize,
    pub unavailable_details: BTreeMap<String, String>,
}
impl EventInputs {
    pub fn insert(&mut self, path: String, input: Option<EventInput>) {
        if !super::eligible(&path) {
            return;
        }
        self.considered += 1;
        let reason = input
            .as_ref()
            .and_then(|input| input.unavailable_reason.clone());
        let Some(source) = input
            .and_then(|i| i.source)
            .filter(|s| s.len() <= super::SOURCE_BYTES_PER_FILE)
        else {
            self.unavailable += 1;
            self.record_unavailable(
                path,
                reason.unwrap_or_else(|| {
                    "source input was not captured for this index generation".into()
                }),
            );
            return;
        };
        self.bytes += source.len();
        self.sources.insert(path, source);
        while self.sources.len() > super::SOURCE_FILES_PER_SNAPSHOT
            || self.bytes > super::SOURCE_BYTES_PER_SNAPSHOT
        {
            if let Some((path, source)) = self.sources.pop_last() {
                self.bytes -= source.len();
                self.unavailable += 1;
                self.record_unavailable(
                    path,
                    format!(
                        "snapshot source budget exceeded ({} files / {} bytes)",
                        super::SOURCE_FILES_PER_SNAPSHOT,
                        super::SOURCE_BYTES_PER_SNAPSHOT
                    ),
                );
            }
        }
    }
    fn record_unavailable(&mut self, path: String, reason: String) {
        self.unavailable_details.insert(path, reason);
        while self.unavailable_details.len() > 64 {
            self.unavailable_details.pop_last();
        }
    }
    pub fn into_sources(self) -> HashMap<String, String> {
        self.sources.into_iter().collect()
    }
}
