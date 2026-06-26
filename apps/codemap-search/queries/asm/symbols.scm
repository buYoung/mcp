
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
