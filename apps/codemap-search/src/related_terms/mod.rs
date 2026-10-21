//! Opt-in sense-dataset lookup; importing this module performs no I/O.
pub(crate) mod evidence;
pub mod fetch;
pub mod lookup;
pub mod normalize;
pub(crate) mod overlay;
pub mod prepare;
pub(crate) mod runtime;
mod syntax;

#[cfg(test)]
mod tests;

pub use lookup::{LookupOptions, LookupResult, RelatedTermsLookup, ResolvedInput, SenseResult};
pub use prepare::{prepare_dataset, PreparedDataset, INDEX_VERSION};
