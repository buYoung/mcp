//! Source-grounded declaration/implementation relationships, separate from runtime dispatch.
mod extract;
mod index;
mod model;
mod relations;
mod render;
mod resolve;
#[cfg(test)]
mod tests;

pub(crate) use extract::collect;
pub(crate) use index::ImplementationIndex;
use index::*;
pub use model::ImplementationFile;
pub(crate) use model::*;

pub(crate) const TYPES_PER_FILE: usize = 256;
pub(crate) const METHODS_PER_FILE: usize = 2048;
pub(crate) const CALLS_PER_FILE: usize = 2048;
pub(crate) const TYPES_PER_SNAPSHOT: usize = 32_768;
pub(crate) const METHODS_PER_SNAPSHOT: usize = 131_072;
pub(crate) const CANDIDATES_PER_QUERY: usize = 512;
pub(crate) const OUTPUT_ROWS_PER_QUERY: usize = 128;
pub(crate) const INHERITANCE_DEPTH: usize = 32;

/// Only development languages with declaration-level inheritance/conformance participate.
/// Components enter through their existing JS/TS script masks, never their markup.
pub(crate) fn supports(language: &str) -> bool {
    matches!(
        language,
        "typescript"
            | "javascript"
            | "java"
            | "csharp"
            | "kotlin"
            | "scala"
            | "groovy"
            | "swift"
            | "dart"
            | "php"
            | "python"
            | "ruby"
            | "powershell"
            | "cpp"
            | "rust"
            | "go"
    )
}

pub(crate) fn family(language: &str) -> &str {
    match language {
        "typescript" | "javascript" => "script",
        _ => language,
    }
}
