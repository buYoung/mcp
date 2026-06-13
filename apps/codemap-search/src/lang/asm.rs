//! Assembly (GAS / AT&T and Intel syntax) language spec.

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{LanguageSpec, NameDecision};

// Assembly (GAS / AT&T and Intel syntax): node kinds verified against tree-sitter-asm 0.24.0
// node-types.json and an empirical parse-tree dump. Key shapes:
//   - label: has a `name` field (word) for named labels, or an `ident` child for local labels.
//     Labels are the primary symbol kind — they define functions and branch targets.
//   - meta: has `kind` field (meta_ident). `.globl` / `.global` directives export a symbol.
//     `.macro` defines a macro (emitted as fn). `.equ` and `=` constants produce ERROR nodes
//     in the parser — skip them (the grammar does not model them reliably).
//   - `const` node: models `name = value` assignment — emitted as kind const.
//   - `string` node (child of `meta`): `.asciz`/`.ascii`/`.string` data directives produce a
//     `string` child confirmed in node-types.json. Captured as `@literal.string` for BM25.
// Export detection: a label is exported iff its name appears in a preceding `.globl`/`.global`
// meta directive anywhere in the file (pre-pass via `collect_asm_globl_names`).
const ASM_QUERY_STR: &str = r#"
;; Labels: branch targets and function entry points.
(label) @symbol.asmfn

;; Macro definitions: `.macro name`.
(meta
  kind: (meta_ident) @meta_kind) @symbol.asmfn

;; Const assignments: `NAME = VALUE` (tree-sitter-asm `const` node).
(const
  name: (word) @symbol.name) @symbol.const

;; String data directives: `.asciz "hello"` / `.ascii "..."` / `.string "..."` —
;; the `string` node is a direct child of the `meta` node (node-types.json confirmed).
(meta
  (string) @literal.string)
"#;

fn get_asm_query() -> &'static Query {
    static ASM_QUERY: OnceLock<Query> = OnceLock::new();
    ASM_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_asm::LANGUAGE.into(), ASM_QUERY_STR)
            .expect("Failed to compile ASM query")
    })
}

/// ASM pre-pass: collect all symbol names that appear as arguments to `.globl` / `.global`
/// directives (case-insensitive match on the meta_ident kind text). These are the exported
/// symbols. The grammar models `.globl name` as `meta { kind: meta_ident, ident child }`.
fn collect_asm_globl_names(root: Node, source: &[u8], out: &mut HashSet<String>) {
    let mut cursor = root.walk();
    let mut to_visit: Vec<Node> = vec![root];
    while let Some(node) = to_visit.pop() {
        if node.kind() == "meta" {
            if let Some(kind_node) = node.child_by_field_name("kind") {
                if let Ok(kind_text) = kind_node.utf8_text(source) {
                    let lower = kind_text.to_ascii_lowercase();
                    if lower == ".globl" || lower == ".global" {
                        // The argument is an ident child (confirmed via parse-tree dump).
                        for i in 0..node.child_count() {
                            let child = node.child(i as u32).unwrap();
                            if child.kind() == "ident" {
                                // The ident may contain a reg child (reg { word }) —
                                // walk to the innermost word to get the plain name text.
                                out.insert(asm_ident_to_name(child, source));
                                break;
                            }
                        }
                    }
                }
            }
        }
        // Push all children; no need to recurse into subtrees more than 1 level since
        // the grammar's root `program` has all directives and labels as direct children.
        for i in (0..node.child_count()).rev() {
            to_visit.push(node.child(i as u32).unwrap());
        }
        // Avoid infinite loop warning — cursor is used for WalkBuilder compatibility.
        let _ = &mut cursor;
    }
}

/// ASM: flatten an `ident` node to its text string. An `ident` may wrap a `reg` which
/// wraps a `word`; we just take the text of the whole subtree's first word-like leaf.
fn asm_ident_to_name(ident_node: Node, source: &[u8]) -> String {
    // Try the full text of the ident node first (most compact).
    if let Ok(text) = ident_node.utf8_text(source) {
        return text.trim().to_string();
    }
    String::new()
}

/// ASM: extract the name from a `label` node. A named label has a `name` field (word);
/// a local label (e.g. `.L1:`) has an `ident` child. Returns `None` on unexpected shape.
fn asm_label_name(label_node: Node, source: &[u8]) -> Option<String> {
    // First try the `name` field (simple word labels).
    if let Some(name) = label_node.child_by_field_name("name") {
        return name.utf8_text(source).ok().map(|t| t.trim().to_string());
    }
    // Fall back to the first `ident` child (local labels).
    for i in 0..label_node.child_count() {
        let child = label_node.child(i as u32).unwrap();
        if child.kind() == "ident" {
            return child.utf8_text(source).ok().map(|t| t.trim().to_string());
        }
    }
    None
}

pub(crate) struct AsmSpec;

impl LanguageSpec for AsmSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_asm::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_asm_query()
    }

    fn collect_exported_names(&self, root: Node, source: &[u8], out: &mut HashSet<String>) {
        // ASM: pre-pass to collect all `.globl`/`.global` exported symbol names so the
        // per-label export check is a simple set lookup (O(1)) during the match loop.
        collect_asm_globl_names(root, source, out);
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        node: Node,
        _kind: &str,
        _ext: &str,
        source: &[u8],
        asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        if capture_name != "symbol.asmfn" {
            return None;
        }

        // ASM: only emit `symbol.asmfn` matches where the meta kind is `.macro`; skip all
        // other `meta` node matches (`.globl`, etc.).
        if node.kind() == "meta" {
            match asm_meta_kind_text {
                Some(k) if k == ".macro" => {} // keep: emit this macro
                _ => return Some(NameDecision::Skip), // discard: not a .macro
            }
        }

        if node.kind() == "label" {
            // ASM label: name is the `name` field or first `ident` child.
            match asm_label_name(node, source) {
                Some(n) => {
                    // Skip assembler-local labels — they are never public API:
                    //   `.L`-prefixed labels are compiler-generated locals (GAS convention).
                    //   Purely numeric labels (e.g. `1:`) are temporary branch targets.
                    if n.starts_with(".L") || n.chars().all(|c| c.is_ascii_digit()) {
                        return Some(NameDecision::Skip);
                    }
                    Some(NameDecision::Name(n))
                }
                None => Some(NameDecision::Skip),
            }
        } else if node.kind() == "meta" {
            // `.macro name` — the first `ident` child is the macro name.
            let mut macro_name = None;
            for i in 0..node.child_count() {
                let child = node.child(i as u32).unwrap();
                if child.kind() == "ident" {
                    if let Ok(text) = child.utf8_text(source) {
                        macro_name = Some(text.trim().to_string());
                        break;
                    }
                }
            }
            match macro_name {
                Some(n) if !n.is_empty() => Some(NameDecision::Name(n)),
                _ => Some(NameDecision::Skip),
            }
        } else {
            // Other `symbol.asmfn` node kinds: no special handling here.
            None
        }
    }

    fn is_test(
        &self,
        _node: Node,
        _name: &str,
        _kind: &str,
        file_path: &str,
        _source: &[u8],
        _comments_text: &str,
    ) -> bool {
        // C/C++/ASM: use path-only test detection (no framework-specific attribute parsing).
        super::path_indicates_test(file_path)
    }

    fn is_exported(
        &self,
        _node: Node,
        name: &str,
        _kind: &str,
        _source: &[u8],
        exported_names: &HashSet<String>,
    ) -> bool {
        // ASM: a label is exported iff its name appears in a `.globl` directive collected
        // in the pre-pass.
        exported_names.contains(name)
    }

    // is_deprecated uses the trait default (docstring `@deprecated` marker).
}
