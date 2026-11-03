(
  (instruction
    kind: (word) @asm.call.kind) @nav.call
  (#match? @asm.call.kind "^(call|callq|bl|blx|jal|jalr)$")
)

(
  (meta
    kind: (meta_ident) @asm.import.kind) @nav.import
  (#eq? @asm.import.kind ".include")
)

; The grammar reads a preprocessed file's `#include` as a comment.
(
  (line_comment) @nav.import
  (#match? @nav.import "^#include[ \t]")
)

; NASM `%include "file"`: the grammar leaves `%` out of an `include` instruction.
(
  (instruction
    kind: (word) @asm.import.kind
    (string)) @nav.import
  (#eq? @asm.import.kind "include")
)
