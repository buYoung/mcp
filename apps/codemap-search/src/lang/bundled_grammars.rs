//! Grammar fixes bundled with the binary; provenance is in vendor/grammar-sources.json.
use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_groovy() -> *const ();
    fn tree_sitter_zsh() -> *const ();
    fn tree_sitter_kotlin() -> *const ();
}

// The generated native functions return static Tree-sitter language descriptors.
pub(crate) const GROOVY: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_groovy) };
pub(crate) const ZSH: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_zsh) };
pub(crate) const KOTLIN: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_kotlin) };
